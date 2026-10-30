//! Scalar/vector dispatch reconstructed from EfiCore's installed module interface
//! and Efi.Core CIL routing. Algorithms are independently implemented.
//! Evidence and branch restrictions: docs/analysis/0008-engine-catalog.md.
use super::{Fault, Quantity, Result, Value};
use num_complex::Complex64;
use std::f64::consts::{FRAC_PI_2, PI, TAU};

pub const NAMES: &[&str] = &[
    "sin", "cos", "tan", "cot", "sec", "csc", "sinc", "asin", "acos", "atan", "acot", "asec",
    "acsc", "sinh", "cosh", "tanh", "coth", "sech", "csch", "asinh", "acosh", "atanh", "acoth",
    "asech", "acsch", "ln", "log", "exp", "sqrt", "re", "im", "Re", "Im", "conj", "arg", "angle",
    "atan2", "sign", "signum", "csgn", "floor", "ceil", "trunc", "round", "Floor", "Ceil", "Trunc",
    "Round", "mod", "IsScalar", "IsArray", "IsRange", "IsString", "IsNaN",
];

pub fn is_builtin(name: &str) -> bool {
    NAMES.contains(&name)
}

fn invalid(detail: impl Into<String>) -> Fault {
    Fault::new("invalid_argument", detail)
}

pub fn core_error(error: String) -> Fault {
    if error.to_lowercase().contains("zero") {
        Fault::new("divide_by_zero", error)
    } else if error.contains("dimension") {
        Fault::new("bad%_dimensions", error)
    } else {
        invalid(error)
    }
}

fn finite(q: Quantity) -> Result<Quantity> {
    super::check_cancelled()?;
    if q.value.re.is_finite() && q.value.im.is_finite() {
        Ok(q)
    } else {
        Err(invalid("Nonfinite function result"))
    }
}

fn dimensionless(q: Quantity) -> Result<()> {
    if q.is_scalar() {
        Ok(())
    } else {
        Err(Fault::new(
            "bad%_dimensions%_unitless",
            "Argument must be dimensionless",
        ))
    }
}

fn real(q: Quantity) -> Result<f64> {
    if q.value.im == 0. {
        Ok(q.value.re)
    } else {
        Err(invalid("Argument must be real"))
    }
}

fn reciprocal(value: Complex64) -> Result<Complex64> {
    if value == Complex64::new(0., 0.) {
        Err(Fault::new("divide_by_zero", "Reciprocal of zero"))
    } else {
        Ok(Complex64::new(1., 0.) / value)
    }
}

fn rounding(name: &str, a: Quantity, b: Option<Quantity>) -> Result<Quantity> {
    let capital = name.chars().next().is_some_and(char::is_uppercase);
    let (ratio, multiplier) = if capital {
        let b = b.ok_or_else(|| invalid("Two arguments required"))?;
        real(b)?;
        if !a.same_dimension(&b) {
            return Err(Fault::new(
                "bad%_dimensions",
                "Rounding quantum has incompatible units",
            ));
        }
        (a.multiply(b, true).map_err(core_error)?, b)
    } else {
        dimensionless(a)?;
        let places = b.unwrap_or(Quantity::scalar(0.));
        dimensionless(places)?;
        let places = real(places)?;
        if places.fract() != 0. || !(-308. ..=308.).contains(&places) {
            return Err(invalid(
                "Decimal places must be an integer between -308 and 308",
            ));
        }
        if name != "round" && b.is_some() {
            return Err(invalid("Unexpected second argument"));
        }
        let multiplier = Quantity::scalar(10_f64.powf(-places));
        (
            a.multiply(multiplier, true).map_err(core_error)?,
            multiplier,
        )
    };
    let round = |x: f64| match name {
        "floor" | "Floor" => x.floor(),
        "ceil" | "Ceil" => x.ceil(),
        "trunc" | "Trunc" => x.trunc(),
        _ => x.round(),
    };
    let rounded = Quantity {
        value: Complex64::new(round(ratio.value.re), round(ratio.value.im)),
        ..ratio
    };
    rounded.multiply(multiplier, false).map_err(core_error)
}

fn scalar(name: &str, args: &[Quantity]) -> Result<Quantity> {
    let Some(&a) = args.first() else {
        return Err(invalid("Missing function argument"));
    };
    let b = args.get(1).copied();
    let optional = matches!(name, "log" | "round" | "signum");
    let two = matches!(
        name,
        "Floor" | "Ceil" | "Trunc" | "Round" | "mod" | "atan2" | "angle"
    );
    if args.len() > 2 || (two && args.len() != 2) || (!two && !optional && args.len() != 1) {
        return Err(invalid("Function argument count"));
    }
    if matches!(
        name,
        "floor" | "ceil" | "trunc" | "round" | "Floor" | "Ceil" | "Trunc" | "Round"
    ) {
        return finite(rounding(name, a, b)?);
    }
    let z = a.value;
    let one = Complex64::new(1., 0.);
    let q = match name {
        "sqrt" => a.power(Quantity::scalar(0.5)).map_err(core_error)?,
        "re" | "Re" => Quantity {
            value: Complex64::new(z.re, 0.),
            ..a
        },
        "im" | "Im" => Quantity {
            value: Complex64::new(z.im, 0.),
            ..a
        },
        "conj" => Quantity {
            value: z.conj(),
            ..a
        },
        "arg" => {
            if z == Complex64::new(0., 0.) {
                return Err(invalid("Argument of zero"));
            }
            let angle = z.arg();
            Quantity::scalar(if angle <= -PI { PI } else { angle })
        }
        "sign" => Quantity::scalar(if real(a)? == 0. { 0. } else { z.re.signum() }),
        "csgn" => Quantity::scalar(if z == Complex64::new(0., 0.) {
            0.
        } else if z.re > 0. || (z.re == 0. && z.im > 0.) {
            1.
        } else {
            -1.
        }),
        "signum" => {
            let zero_value = b.unwrap_or(Quantity::scalar(1.));
            dimensionless(zero_value)?;
            if z == Complex64::new(0., 0.) {
                zero_value
            } else {
                Quantity {
                    value: z / z.norm(),
                    dimension: [0.; 7],
                }
            }
        }
        "mod" => {
            let b = b.unwrap();
            if !a.same_dimension(&b) {
                return Err(Fault::new(
                    "bad%_dimensions",
                    "Modulo arguments have incompatible units",
                ));
            }
            let x = real(a)?;
            let y = real(b)?;
            if y == 0. {
                return Err(Fault::new("divide_by_zero", "Modulo divisor is zero"));
            }
            Quantity {
                value: Complex64::new(x % y, 0.),
                ..a
            }
        }
        "angle" | "atan2" => {
            let b = b.unwrap();
            if !a.same_dimension(&b) {
                return Err(Fault::new(
                    "bad%_dimensions",
                    "Angle coordinates have incompatible units",
                ));
            }
            let x = real(a)?;
            let y = real(b)?;
            if x == 0. && y == 0. {
                return Err(invalid("Angle at the origin is undefined"));
            }
            // Mathcad atan2(x,y) reverses Rust's y.atan2(x) receiver order.
            let angle = y.atan2(x);
            Quantity::scalar(if name == "angle" {
                angle.rem_euclid(TAU)
            } else if angle <= -PI {
                PI
            } else {
                angle
            })
        }
        _ => {
            dimensionless(a)?;
            if matches!(name, "sin" | "cos" | "tan" | "cot" | "sec" | "csc" | "sinc")
                && z.norm() > 1e12
            {
                return Err(invalid("Trigonometric argument magnitude exceeds 10^12"));
            }
            let value = match name {
                "sin" => z.sin(),
                "cos" => z.cos(),
                "tan" => z.tan(),
                "cot" => reciprocal(z.tan())?,
                "sec" => reciprocal(z.cos())?,
                "csc" => reciprocal(z.sin())?,
                "sinc" => {
                    if z == Complex64::new(0., 0.) {
                        one
                    } else {
                        z.sin() / z
                    }
                }
                "asin" => z.asin(),
                "acos" => z.acos(),
                "atan" => z.atan(),
                "acot" => {
                    if z.im == 0. {
                        Complex64::new(1_f64.atan2(z.re), 0.)
                    } else {
                        Complex64::new(FRAC_PI_2, 0.) - z.atan()
                    }
                }
                "asec" => reciprocal(z)?.acos(),
                "acsc" => reciprocal(z)?.asin(),
                "sinh" => z.sinh(),
                "cosh" => z.cosh(),
                "tanh" => z.tanh(),
                "coth" => reciprocal(z.tanh())?,
                "sech" => reciprocal(z.cosh())?,
                "csch" => reciprocal(z.sinh())?,
                "asinh" => z.asinh(),
                "acosh" => z.acosh(),
                "atanh" => z.atanh(),
                "acoth" => reciprocal(z)?.atanh(),
                "asech" => reciprocal(z)?.acosh(),
                "acsch" => reciprocal(z)?.asinh(),
                "exp" => z.exp(),
                "ln" => {
                    if z == Complex64::new(0., 0.) {
                        return Err(invalid("Logarithm of zero"));
                    }
                    z.ln()
                }
                "log" => {
                    let base = b.unwrap_or(Quantity::scalar(10.));
                    dimensionless(base)?;
                    if z == Complex64::new(0., 0.)
                        || base.value == Complex64::new(0., 0.)
                        || base.value == one
                    {
                        return Err(invalid("Invalid logarithm base or argument"));
                    }
                    z.ln() / base.value.ln()
                }
                _ => {
                    return Err(Fault::unsupported(format!(
                        "Unimplemented scalar function {name}"
                    )));
                }
            };
            Quantity {
                value,
                dimension: [0.; 7],
            }
        }
    };
    finite(q)
}

pub fn factorial(value: &Value) -> Result<Value> {
    map("factorial", std::slice::from_ref(value), |args| {
        let a = args[0];
        dimensionless(a)?;
        let n = real(a)?;
        if n.fract() != 0. || !(0. ..=170.).contains(&n) {
            return Err(Fault::unsupported(
                "Factorial currently requires an integer from 0 to 170",
            ));
        }
        Ok(Quantity::scalar(
            (1..=n as u32).fold(1., |p, n| p * f64::from(n)),
        ))
    })
}

fn map(
    name: &str,
    args: &[Value],
    operation: impl Fn(&[Quantity]) -> Result<Quantity>,
) -> Result<Value> {
    let matrices = matches!(name, "Re" | "Im" | "re" | "im" | "conj");
    let mut shape = None;
    for value in args {
        match value {
            Value::Scalar { .. } => (),
            Value::Matrix { rows, cols, .. } if matrices || *cols == 1 => {
                let current = (*rows, *cols);
                if shape.is_some_and(|s| s != current) {
                    return Err(invalid("Vector arguments must have matching sizes"));
                }
                shape = Some(current);
            }
            _ => {
                return Err(Fault::unsupported(
                    "Function requires a scalar or column vector",
                ));
            }
        }
    }
    let element = |index| {
        args.iter()
            .map(|a| match a {
                Value::Scalar { quantity } => *quantity,
                Value::Matrix { elements, .. } => elements[index],
                _ => unreachable!(),
            })
            .collect::<Vec<_>>()
    };
    if let Some((rows, cols)) = shape {
        Ok(Value::Matrix {
            rows,
            cols,
            elements: (0..rows * cols)
                .map(|i| operation(&element(i)))
                .collect::<Result<_>>()?,
        })
    } else {
        Ok(Value::scalar(operation(&element(0))?))
    }
}

pub fn call(name: &str, args: &[Value]) -> Result<Value> {
    if matches!(
        name,
        "IsScalar" | "IsArray" | "IsRange" | "IsString" | "IsNaN"
    ) {
        let [value] = args else {
            return Err(invalid("Type predicate argument count"));
        };
        let yes = match name {
            "IsScalar" => matches!(value, Value::Scalar { .. }),
            "IsArray" => matches!(value, Value::Matrix { .. }),
            "IsRange" => matches!(value, Value::Range { .. }),
            "IsString" => matches!(value, Value::String { .. }),
            // NaN values are not produced by this engine.
            _ => false,
        };
        return Ok(Value::scalar(Quantity::scalar(if yes { 1. } else { 0. })));
    }
    map(name, args, |arguments| scalar(name, arguments))
}
