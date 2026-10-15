//! Independent numeric statistics. Evidence and deliberate subset limits:
//! docs/analysis/0009-statistics-semantics.md. No vendor code is executed.
use crate::{Fault, Quantity, Result, Value};
use num_complex::Complex64;

const MAX_ELEMENTS: usize = 16_384;

fn bad(detail: &str) -> Fault {
    Fault::unsupported(detail)
}
fn finite(q: Quantity) -> Result<Quantity> {
    if q.value.re.is_finite() && q.value.im.is_finite() && q.dimension.iter().all(|d| d.is_finite())
    {
        Ok(q)
    } else {
        Err(bad("Statistics require finite numeric data"))
    }
}
fn array(value: &Value) -> Result<(usize, usize, &[Quantity])> {
    match value {
        Value::Matrix {
            rows,
            cols,
            elements,
        } if *rows > 0
            && *cols > 0
            && rows.checked_mul(*cols) == Some(elements.len())
            && elements.len() <= MAX_ELEMENTS =>
        {
            for q in elements {
                finite(*q)?;
            }
            Ok((*rows, *cols, elements))
        }
        _ => Err(bad(
            "Expected finite numeric array; ranges are not matrices",
        )),
    }
}
fn data(args: &[Value]) -> Result<Vec<Quantity>> {
    let mut result = Vec::new();
    for value in args {
        match value {
            Value::Scalar { quantity } => result.push(finite(*quantity)?),
            _ => result.extend_from_slice(array(value)?.2),
        }
        if result.len() > MAX_ELEMENTS {
            return Err(bad("Statistics element budget exceeded"));
        }
    }
    let first = result
        .first()
        .ok_or_else(|| bad("Statistics require nonempty arguments"))?;
    if result.iter().any(|q| !q.same_dimension(first)) {
        return Err(Fault::new(
            "bad%_dimensions",
            "Statistical data dimensions differ",
        ));
    }
    Ok(result)
}
fn mean(data: &[Quantity]) -> Complex64 {
    // Divide each value before summation to avoid overflow for large constants.
    data.iter().map(|q| q.value / data.len() as f64).sum()
}
fn output(value: Complex64, dimension: [f64; 7]) -> Result<Value> {
    Ok(Value::scalar(finite(Quantity { value, dimension })?))
}
fn real(data: &[Quantity]) -> Result<()> {
    if data.iter().any(|q| q.value.im != 0.) {
        Err(bad("This statistic requires real data"))
    } else {
        Ok(())
    }
}
fn variance(data: &[Quantity], sample: bool) -> Result<f64> {
    let center = mean(data);
    Ok(data
        .iter()
        .map(|q| (q.value - center).norm_sqr())
        .sum::<f64>()
        // Shipped var_imp IL_165..177 uses 1 when sample size is <=1.
        / (data.len() - usize::from(sample && data.len() > 1)) as f64)
}

pub fn is_builtin(name: &str) -> bool {
    matches!(
        name,
        "mean"
            | "gmean"
            | "hmean"
            | "median"
            | "var"
            | "Var"
            | "stdev"
            | "Stdev"
            | "cvar"
            | "corr"
            | "min"
            | "max"
            | "sort"
            | "reverse"
            | "Rank"
    )
}

/// Dispatch after checking explicitly labelled user function bindings. `covar`
/// is deliberately absent: the original name denotes signal correlation.
pub fn call(name: &str, args: &[Value]) -> Result<Value> {
    if matches!(name, "sort" | "reverse" | "Rank") {
        let [value] = args else {
            return Err(bad("Array ordering function requires one argument"));
        };
        let (rows, cols, elements) = array(value)?;
        if name != "reverse" && cols != 1 {
            return Err(bad("Sort and Rank require a column vector in this subset"));
        }
        let mut elements = elements.to_vec();
        if name == "reverse" {
            // Native Efi.Matrix::reverse returns a one-row matrix unchanged.
            for row in 0..rows / 2 {
                for col in 0..cols {
                    elements.swap(row * cols + col, (rows - 1 - row) * cols + col);
                }
            }
        } else {
            data(args)?;
            if name == "sort" {
                elements.sort_by(|a, b| a.value.re.total_cmp(&b.value.re));
            } else {
                let mut order = (0..elements.len()).collect::<Vec<_>>();
                order.sort_by(|&a, &b| elements[a].value.re.total_cmp(&elements[b].value.re));
                let mut ranks = vec![Quantity::scalar(0.); elements.len()];
                let mut begin = 0;
                while begin < order.len() {
                    let mut end = begin + 1;
                    while end < order.len()
                        && elements[order[begin]].value.re == elements[order[end]].value.re
                    {
                        end += 1;
                    }
                    let rank = (begin + 1 + end) as f64 / 2.;
                    for &index in &order[begin..end] {
                        ranks[index] = Quantity::scalar(rank);
                    }
                    begin = end;
                }
                elements = ranks;
            }
        }
        return Ok(Value::Matrix {
            rows,
            cols,
            elements,
        });
    }
    if matches!(name, "cvar" | "corr") {
        let [a, b] = args else {
            return Err(bad("Covariance/correlation require two arrays"));
        };
        let (ar, ac, a) = array(a)?;
        let (br, bc, b) = array(b)?;
        if (ar, ac) != (br, bc) {
            return Err(bad("Covariance/correlation array shapes differ"));
        }
        // Conjugation side in the native kernel is not established yet.
        // Do not silently invent a complex covariance convention.
        real(a)?;
        real(b)?;
        data(&[args[0].clone()])?;
        data(&[args[1].clone()])?;
        let am = mean(a);
        let bm = mean(b);
        let covariance = a
            .iter()
            .zip(b)
            .map(|(a, b)| (a.value.re - am.re) * (b.value.re - bm.re))
            .sum::<f64>()
            / a.len() as f64;
        if name == "corr" {
            let denominator = (variance(a, false)? * variance(b, false)?).sqrt();
            if denominator == 0. {
                return Err(Fault::new(
                    "divide_by_zero",
                    "Correlation of constant data is undefined",
                ));
            }
            return output(Complex64::new(covariance / denominator, 0.), [0.; 7]);
        }
        return output(
            Complex64::new(covariance, 0.),
            std::array::from_fn(|i| a[0].dimension[i] + b[0].dimension[i]),
        );
    }
    if !is_builtin(name) {
        return Err(bad("Unknown statistical function"));
    }
    let values = data(args)?;
    let dimension = values[0].dimension;
    let value = match name {
        "mean" => mean(&values),
        "gmean" | "hmean" => {
            real(&values)?;
            if values.iter().any(|q| q.value.re <= 0.) {
                return Err(bad("Geometric/harmonic mean require positive real data"));
            }
            let result = if name == "gmean" {
                (values
                    .iter()
                    .map(|q| q.value.re.ln() / values.len() as f64)
                    .sum::<f64>())
                .exp()
            } else {
                values.len() as f64 / values.iter().map(|q| 1. / q.value.re).sum::<f64>()
            };
            Complex64::new(result, 0.)
        }
        "median" => {
            real(&values)?;
            let mut sorted = values.iter().map(|q| q.value.re).collect::<Vec<_>>();
            sorted.sort_by(f64::total_cmp);
            let n = sorted.len();
            Complex64::new(
                if n % 2 == 0 {
                    sorted[n / 2 - 1] / 2. + sorted[n / 2] / 2.
                } else {
                    sorted[n / 2]
                },
                0.,
            )
        }
        "min" | "max" => {
            let choose = |a: f64, b: f64| if name == "min" { a.min(b) } else { a.max(b) };
            values
                .iter()
                .map(|q| q.value)
                .reduce(|a, b| Complex64::new(choose(a.re, b.re), choose(a.im, b.im)))
                .unwrap()
        }
        "var" | "Var" | "stdev" | "Stdev" => {
            let variance = variance(&values, matches!(name, "Var" | "Stdev"))?;
            return if matches!(name, "stdev" | "Stdev") {
                output(Complex64::new(variance.sqrt(), 0.), dimension)
            } else {
                output(Complex64::new(variance, 0.), dimension.map(|d| d * 2.))
            };
        }
        _ => return Err(bad("Unsupported statistic")),
    };
    output(value, dimension)
}
