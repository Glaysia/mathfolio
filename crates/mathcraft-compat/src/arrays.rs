//! Independent numeric array operations. Contracts and limitations are recorded
//! in docs/analysis/0005-array-semantics.md; original code is never executed.
use crate::{Fault, Quantity, Result, Value};
use num_complex::Complex64;

const MAX_ELEMENTS: usize = 16_384;
const MAX_PRODUCTS: usize = 8_388_608;

/// Numeric indexed assignment grows to include the addressed cell. Keep the
/// original zero-fill dimension even after cells acquire heterogeneous units.
/// An imported mixed matrix has no recovered fill basis; overwriting existing
/// cells is safe, but expanding that representation is explicitly unsupported.
pub(crate) struct IndexedArray {
    rows: usize,
    cols: usize,
    elements: Vec<Quantity>,
    zero_dimension: Option<[f64; 7]>,
}
impl IndexedArray {
    pub(crate) fn new(value: Option<Value>, known_dimension: Option<[f64; 7]>) -> Result<Self> {
        if let Some(value @ Value::Matrix { .. }) = value {
            array(&value)?;
            let Value::Matrix {
                rows,
                cols,
                elements,
            } = value
            else {
                unreachable!()
            };
            let zero_dimension = known_dimension.or_else(|| {
                elements
                    .first()
                    .filter(|first| elements.iter().all(|q| q.dimension == first.dimension))
                    .map(|q| q.dimension)
            });
            Ok(Self {
                rows,
                cols,
                elements,
                zero_dimension,
            })
        } else {
            Ok(Self {
                rows: 0,
                cols: 0,
                elements: vec![],
                zero_dimension: None,
            })
        }
    }
    pub(crate) fn put(&mut self, indices: &[usize], value: Quantity) -> Result<()> {
        let value = finite(value)?;
        let (row, col) = match indices {
            [row] if self.cols <= 1 => (*row, 0),
            [col] if self.rows == 1 => (0, *col),
            [_] => {
                return Err(Fault::unsupported(
                    "A single assignment index requires a vector",
                ));
            }
            [row, col] => (*row, *col),
            _ => {
                return Err(Fault::unsupported(
                    "Indexed definitions require one or two indices",
                ));
            }
        };
        let rows = self.rows.max(
            row.checked_add(1)
                .ok_or_else(|| Fault::unsupported("Array size overflow"))?,
        );
        let cols = self.cols.max(
            col.checked_add(1)
                .ok_or_else(|| Fault::unsupported("Array size overflow"))?,
        );
        let count = rows
            .checked_mul(cols)
            .filter(|count| *count <= MAX_ELEMENTS)
            .ok_or_else(|| Fault::unsupported("Indexed array element budget exceeded"))?;
        if self.elements.is_empty() {
            self.zero_dimension = Some(value.dimension);
        }
        if rows != self.rows || cols != self.cols {
            let dimension=self.zero_dimension.ok_or_else(||Fault::unsupported("Expansion of an imported mixed-unit array needs an established zero-fill dimension"))?;
            let zero = Quantity {
                value: num_complex::Complex64::new(0., 0.),
                dimension,
            };
            if cols == self.cols {
                self.elements.resize(count, zero);
            } else {
                let mut expanded = vec![zero; count];
                for r in 0..self.rows {
                    expanded[r * cols..r * cols + self.cols]
                        .copy_from_slice(&self.elements[r * self.cols..(r + 1) * self.cols]);
                }
                self.elements = expanded;
            }
            self.rows = rows;
            self.cols = cols;
        }
        self.elements[row * self.cols + col] = value;
        Ok(())
    }
    pub(crate) fn finish(self) -> (Value, Option<[f64; 7]>) {
        (
            Value::Matrix {
                rows: self.rows,
                cols: self.cols,
                elements: self.elements,
            },
            self.zero_dimension,
        )
    }

    /// The shipped MatVal setters copy a prefix block and retain the rest of
    /// the destination. Column assignment accepts only a column vector;
    /// setrow delegates to setblock, so a matrix RHS spans successive rows.
    pub(crate) fn put_slice(
        &mut self,
        column: bool,
        offset: usize,
        value: &Value,
        known_dimension: Option<[f64; 7]>,
    ) -> Result<()> {
        let scalar_element;
        let (height, width, elements) = match value {
            Value::Scalar { quantity } => {
                scalar_element = [finite(*quantity)?];
                (1, 1, scalar_element.as_slice())
            }
            _ => array(value)?,
        };
        if column && width != 1 {
            return Err(Fault::unsupported(
                "Column assignment requires a column vector or scalar",
            ));
        }
        let (top, left) = if column { (0, offset) } else { (offset, 0) };
        let rows = self.rows.max(
            top.checked_add(height)
                .ok_or_else(|| Fault::unsupported("Array size overflow"))?,
        );
        let cols = self.cols.max(
            left.checked_add(width)
                .ok_or_else(|| Fault::unsupported("Array size overflow"))?,
        );
        let count = rows
            .checked_mul(cols)
            .filter(|count| *count <= MAX_ELEMENTS)
            .ok_or_else(|| Fault::unsupported("Indexed array element budget exceeded"))?;
        if self.elements.is_empty() {
            self.zero_dimension = known_dimension.or_else(|| {
                elements
                    .first()
                    .filter(|first| elements.iter().all(|q| q.dimension == first.dimension))
                    .map(|q| q.dimension)
            });
        }
        if rows != self.rows || cols != self.cols {
            let fills_every_cell = top == 0 && left == 0 && rows == height && cols == width;
            let dimension = match self.zero_dimension {
                Some(dimension) => dimension,
                None if fills_every_cell => [0.; 7], // All allocation cells are overwritten below.
                None => {
                    return Err(Fault::unsupported(
                        "Expansion of a mixed-unit array needs an established zero-fill dimension",
                    ));
                }
            };
            let mut expanded = vec![
                Quantity {
                    value: Complex64::new(0., 0.),
                    dimension
                };
                count
            ];
            for row in 0..self.rows {
                super::check_cancelled()?;
                expanded[row * cols..row * cols + self.cols]
                    .copy_from_slice(&self.elements[row * self.cols..(row + 1) * self.cols]);
            }
            self.elements = expanded;
            self.rows = rows;
            self.cols = cols;
        }
        for row in 0..height {
            super::check_cancelled()?;
            let start = (top + row) * self.cols + left;
            self.elements[start..start + width]
                .copy_from_slice(&elements[row * width..(row + 1) * width]);
        }
        Ok(())
    }
}

pub(crate) fn index_offset(index: Quantity, origin: Quantity) -> Result<usize> {
    if !index.is_scalar()
        || index.value.im != 0.
        || !index.value.re.is_finite()
        || index.value.re.fract() != 0.
    {
        return Err(Fault::unsupported("Non-integer/unit-bearing matrix index"));
    }
    let offset = index.value.re - origin.value.re;
    if offset < 0. || offset >= MAX_ELEMENTS as f64 {
        return Err(Fault::unsupported("Matrix index outside implemented range"));
    }
    Ok(offset as usize)
}

fn core(error: String) -> Fault {
    if error.contains("dimension") {
        Fault::new("bad%_dimensions", error)
    } else if error.to_lowercase().contains("zero") {
        Fault::new("divide_by_zero", error)
    } else {
        Fault::unsupported(error)
    }
}
fn finite(q: Quantity) -> Result<Quantity> {
    super::check_cancelled()?;
    if q.value.re.is_finite() && q.value.im.is_finite() && q.dimension.iter().all(|d| d.is_finite())
    {
        Ok(q)
    } else {
        Err(Fault::unsupported("Nonfinite array arithmetic result"))
    }
}
fn matrix(rows: usize, cols: usize, elements: Vec<Quantity>) -> Result<Value> {
    if rows == 0
        || cols == 0
        || rows.checked_mul(cols) != Some(elements.len())
        || elements.len() > MAX_ELEMENTS
    {
        return Err(Fault::unsupported("Invalid array shape or element budget"));
    }
    for q in &elements {
        finite(*q)?;
    }
    Ok(Value::Matrix {
        rows,
        cols,
        elements,
    })
}
fn array(v: &Value) -> Result<(usize, usize, &[Quantity])> {
    match v {
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
        _ => Err(Fault::unsupported(
            "Expected a finite numeric matrix; ranges are not matrices",
        )),
    }
}
fn scalar(v: &Value) -> Result<Quantity> {
    match v {
        Value::Scalar { quantity } => finite(*quantity),
        _ => Err(Fault::unsupported("Expected scalar argument")),
    }
}
fn integer(q: Quantity) -> Result<i64> {
    finite(q)?;
    if !q.is_scalar()
        || q.value.im != 0.
        || q.value.re.fract() != 0.
        || q.value.re.abs() > i32::MAX as f64
    {
        return Err(Fault::unsupported("Expected a real dimensionless integer"));
    }
    Ok(q.value.re as i64)
}
fn unary_map(v: &Value, f: impl Fn(Quantity) -> Result<Quantity>) -> Result<Value> {
    match v {
        Value::Scalar { quantity } => Ok(Value::scalar(f(*quantity)?)),
        _ => {
            let (r, c, e) = array(v)?;
            matrix(r, c, e.iter().copied().map(f).collect::<Result<_>>()?)
        }
    }
}
fn scalar_op(op: &str, a: Quantity, b: Quantity) -> Result<Quantity> {
    finite(a)?;
    finite(b)?;
    match op {
        "plus" => a.add(b, false),
        "minus" => a.add(b, true),
        "mult" | "scale" => a.multiply(b, false),
        "div" => a.multiply(b, true),
        "pow" => a.power(b),
        _ => {
            return Err(Fault::unsupported(format!(
                "Unknown scalar/array operator {op}"
            )));
        }
    }
    .map_err(core)
}
fn mapped(op: &str, a: &Value, b: &Value) -> Result<Value> {
    match (a, b) {
        (Value::Scalar { quantity: a }, Value::Scalar { quantity: b }) => {
            Ok(Value::scalar(scalar_op(op, *a, *b)?))
        }
        (Value::Scalar { quantity: a }, _) => unary_map(b, |b| scalar_op(op, *a, b)),
        (_, Value::Scalar { quantity: b }) => unary_map(a, |a| scalar_op(op, a, *b)),
        _ => {
            let (ar, ac, ae) = array(a)?;
            let (br, bc, be) = array(b)?;
            if (ar, ac) != (br, bc) {
                return Err(Fault::unsupported(
                    "Array elementwise operands need equal shapes",
                ));
            }
            matrix(
                ar,
                ac,
                ae.iter()
                    .zip(be)
                    .map(|(a, b)| scalar_op(op, *a, *b))
                    .collect::<Result<_>>()?,
            )
        }
    }
}

/// Ordinary multiplication is a dot product for two column vectors, matrix
/// multiplication for conformable matrices, and scalar mapping otherwise.
pub fn binary(op: &str, left: &Value, right: &Value) -> Result<Value> {
    match op {
        "plus" | "minus" => mapped(op, left, right),
        "mult" | "scale" => {
            if matches!(left, Value::Scalar { .. }) || matches!(right, Value::Scalar { .. }) {
                mapped(op, left, right)
            } else {
                multiply(left, right, true)
            }
        }
        "div" => {
            // The shipped div implementation checks a square right operand
            // before scalar/array mapping, including scalar / square-matrix.
            if let Value::Matrix { rows, cols, .. } = right
                && rows == cols
            {
                return binary("mult", left, &inverse(right)?);
            }
            if matches!(left, Value::Scalar { .. }) || matches!(right, Value::Scalar { .. }) {
                return mapped(op, left, right);
            }
            let (ar, ac, _) = array(left)?;
            let (br, bc, _) = array(right)?;
            if (ar, ac) == (br, bc) {
                mapped(op, left, right)
            } else {
                Err(Fault::unsupported("Array division has incompatible shapes"))
            }
        }
        "pow" => power(left, right),
        _ => Err(Fault::unsupported(format!("Unknown array operator {op}"))),
    }
}

fn add_terms(a: Option<Quantity>, b: Quantity) -> Result<Quantity> {
    // Do not invent a dimensionless initial zero for a physical dot product.
    match a {
        Some(a) => a.add(b, false).map_err(core),
        None => finite(b),
    }
}
fn multiply(left: &Value, right: &Value, allow_dot: bool) -> Result<Value> {
    let (ar, ac, a) = array(left)?;
    let (br, bc, b) = array(right)?;
    if allow_dot && ac == 1 && bc == 1 && ar == br {
        let mut sum = None;
        for (a, b) in a.iter().zip(b) {
            let mut b = *b;
            b.value = b.value.conj();
            sum = Some(add_terms(sum, a.multiply(b, false).map_err(core)?)?);
        }
        return Ok(Value::scalar(sum.unwrap()));
    }
    if ac != br {
        return Err(Fault::unsupported(
            "Matrix product requires matching inner dimensions",
        ));
    }
    if ar.checked_mul(bc).is_none_or(|n| n > MAX_ELEMENTS)
        || ar
            .checked_mul(ac)
            .and_then(|n| n.checked_mul(bc))
            .is_none_or(|n| n > MAX_PRODUCTS)
    {
        return Err(Fault::unsupported(
            "Matrix multiplication work budget exceeded",
        ));
    }
    let mut elements = Vec::with_capacity(ar * bc);
    for r in 0..ar {
        for c in 0..bc {
            let mut sum = None;
            for k in 0..ac {
                sum = Some(add_terms(
                    sum,
                    a[r * ac + k].multiply(b[k * bc + c], false).map_err(core)?,
                )?);
            }
            elements.push(sum.unwrap());
        }
    }
    if ar == 1 && bc == 1 {
        Ok(Value::scalar(elements[0]))
    } else {
        matrix(ar, bc, elements)
    }
}

fn uniform_square(v: &Value) -> Result<(usize, Vec<Complex64>, [f64; 7])> {
    let (r, c, e) = array(v)?;
    if r != c {
        return Err(Fault::unsupported(
            "This operation requires a square matrix",
        ));
    }
    let dimension = e[0].dimension;
    if e.iter().any(|q| !q.same_dimension(&e[0])) {
        return Err(Fault::unsupported(
            "Mixed-dimensional matrix factorization is not implemented",
        ));
    }
    Ok((r, e.iter().map(|q| q.value).collect(), dimension))
}
fn pivot(a: &[Complex64], n: usize, k: usize) -> usize {
    (k..n)
        .max_by(|&r, &s| a[r * n + k].norm().total_cmp(&a[s * n + k].norm()))
        .unwrap()
}
fn determinant(v: &Value) -> Result<Value> {
    let (n, mut a, dimension) = uniform_square(v)?;
    let mut result = Complex64::new(1., 0.);
    for k in 0..n {
        super::check_cancelled()?;
        let p = pivot(&a, n, k);
        if a[p * n + k].norm() == 0. {
            result = Complex64::new(0., 0.);
            break;
        }
        if p != k {
            for c in 0..n {
                a.swap(k * n + c, p * n + c);
            }
            result = -result;
        }
        let diagonal = a[k * n + k];
        result *= diagonal;
        for r in k + 1..n {
            let factor = a[r * n + k] / diagonal;
            for c in k + 1..n {
                let change = factor * a[k * n + c];
                a[r * n + c] -= change;
            }
        }
    }
    Ok(Value::scalar(finite(Quantity {
        value: result,
        dimension: dimension.map(|d| d * n as f64),
    })?))
}
fn inverse(v: &Value) -> Result<Value> {
    let (n, mut a, dimension) = uniform_square(v)?;
    let mut inv = vec![Complex64::new(0., 0.); n * n];
    for i in 0..n {
        inv[i * n + i] = Complex64::new(1., 0.);
    }
    for k in 0..n {
        super::check_cancelled()?;
        let p = pivot(&a, n, k);
        if a[p * n + k].norm() == 0. {
            return Err(Fault::unsupported("Singular matrix has no inverse"));
        }
        if p != k {
            for c in 0..n {
                a.swap(k * n + c, p * n + c);
                inv.swap(k * n + c, p * n + c);
            }
        }
        let diagonal = a[k * n + k];
        for c in 0..n {
            a[k * n + c] /= diagonal;
            inv[k * n + c] /= diagonal;
        }
        for r in 0..n {
            if r != k {
                let factor = a[r * n + k];
                for c in 0..n {
                    let change = factor * a[k * n + c];
                    let inv_change = factor * inv[k * n + c];
                    a[r * n + c] -= change;
                    inv[r * n + c] -= inv_change;
                }
            }
        }
    }
    matrix(
        n,
        n,
        inv.into_iter()
            .map(|value| Quantity {
                value,
                dimension: dimension.map(|d| -d),
            })
            .collect(),
    )
}

/// Solve A*x=b for square or overdetermined full-column-rank systems.
///
/// Prime routes square systems to *gesv and rectangular systems to *gelss.
/// This independent implementation uses reorthogonalized modified Gram-Schmidt
/// followed by triangular back substitution. It preserves the observed b/a
/// physical dimension without claiming LAPACK bit-for-bit parity.
fn lsolve(a: &Value, b: &Value) -> Result<Value> {
    let (rows, cols, a) = compatible(a)?;
    let (b_rows, rhs_cols, b) = compatible(b)?;
    if rows != b_rows {
        return Err(Fault::unsupported(
            "lsolve requires equal row counts for coefficient and right-hand matrices",
        ));
    }
    if rows < cols {
        return Err(Fault::unsupported(
            "Underdetermined lsolve systems are outside the implemented subset",
        ));
    }
    if rows
        .checked_mul(cols)
        .and_then(|work| work.checked_mul(cols.saturating_add(rhs_cols).saturating_add(2)))
        .is_none_or(|work| work > MAX_PRODUCTS)
    {
        return Err(Fault::unsupported("lsolve work budget exceeded"));
    }

    let mut q = vec![Complex64::new(0., 0.); rows * cols];
    let mut r = vec![Complex64::new(0., 0.); cols * cols];
    let mut maximum_column_norm = 0_f64;
    for column in 0..cols {
        super::check_cancelled()?;
        let mut norm = 0_f64;
        for row in 0..rows {
            norm = norm.hypot(a[row * cols + column].value.norm());
        }
        maximum_column_norm = maximum_column_norm.max(norm);
    }
    let rank_threshold = f64::EPSILON * 64. * rows.max(cols) as f64 * maximum_column_norm;

    for column in 0..cols {
        super::check_cancelled()?;
        let mut work = (0..rows)
            .map(|row| a[row * cols + column].value)
            .collect::<Vec<_>>();
        // A second pass substantially reduces loss of orthogonality while
        // retaining a small, deterministic implementation.
        for _ in 0..2 {
            for previous in 0..column {
                let projection = (0..rows)
                    .map(|row| q[row * cols + previous].conj() * work[row])
                    .sum::<Complex64>();
                r[previous * cols + column] += projection;
                for row in 0..rows {
                    work[row] -= q[row * cols + previous] * projection;
                }
            }
        }
        let norm = work
            .iter()
            .fold(0_f64, |norm, value| norm.hypot(value.norm()));
        if !norm.is_finite() || norm <= rank_threshold {
            return Err(Fault::unsupported(
                "lsolve coefficient matrix does not have full column rank",
            ));
        }
        r[column * cols + column] = Complex64::new(norm, 0.);
        for row in 0..rows {
            q[row * cols + column] = work[row] / norm;
        }
    }

    let mut projected = vec![Complex64::new(0., 0.); cols * rhs_cols];
    for row in 0..cols {
        for rhs in 0..rhs_cols {
            projected[row * rhs_cols + rhs] = (0..rows)
                .map(|source| q[source * cols + row].conj() * b[source * rhs_cols + rhs].value)
                .sum();
        }
    }
    let mut solution = vec![Complex64::new(0., 0.); cols * rhs_cols];
    for rhs in 0..rhs_cols {
        for row in (0..cols).rev() {
            let tail = (row + 1..cols)
                .map(|column| r[row * cols + column] * solution[column * rhs_cols + rhs])
                .sum::<Complex64>();
            solution[row * rhs_cols + rhs] =
                (projected[row * rhs_cols + rhs] - tail) / r[row * cols + row];
        }
    }
    let dimension = std::array::from_fn(|index| b[0].dimension[index] - a[0].dimension[index]);
    matrix(
        cols,
        rhs_cols,
        solution
            .into_iter()
            .map(|value| Quantity { value, dimension })
            .collect(),
    )
}

/// Diagonalize a finite real symmetric matrix with cyclic Jacobi rotations.
///
/// Prime's static wrapper selects the LAPACK symmetric path only when every
/// mirrored pair is exactly equal. This independent implementation preserves
/// that boundary and returns descending eigenvalues with matching eigenvector
/// columns. General real and complex matrices remain outside this subset.
struct SymmetricEigensystem {
    size: usize,
    values: Vec<f64>,
    vectors: Vec<f64>,
    dimension: [f64; 7],
}

fn real_symmetric_eigendecomposition(value: &Value) -> Result<SymmetricEigensystem> {
    let (n, complex, dimension) = uniform_square(value)?;
    if n > 32 {
        return Err(Fault::unsupported(
            "Symmetric eigendecomposition currently limited to 32 by 32 matrices",
        ));
    }
    if complex.iter().any(|value| value.im != 0.) {
        return Err(Fault::unsupported(
            "Complex Hermitian and general eigendecomposition is not implemented",
        ));
    }
    let mut matrix = complex.iter().map(|value| value.re).collect::<Vec<_>>();
    for row in 0..n {
        for column in row + 1..n {
            if matrix[row * n + column] != matrix[column * n + row] {
                return Err(Fault::unsupported(
                    "General nonsymmetric eigendecomposition is not implemented",
                ));
            }
        }
    }

    let input_scale = matrix.iter().map(|value| value.abs()).fold(0., f64::max);
    let mut vectors = vec![0.; n * n];
    for diagonal in 0..n {
        vectors[diagonal * n + diagonal] = 1.;
    }
    if input_scale == 0. {
        return Ok(SymmetricEigensystem {
            size: n,
            values: vec![0.; n],
            vectors,
            dimension,
        });
    }
    for value in &mut matrix {
        *value /= input_scale;
    }
    let original = matrix.clone();
    let tolerance = f64::EPSILON * 64. * n as f64;
    for _ in 0..64 {
        super::check_cancelled()?;
        let mut changed = false;
        for p in 0..n {
            for q in p + 1..n {
                let apq = matrix[p * n + q];
                if apq.abs() <= tolerance {
                    continue;
                }
                changed = true;
                let app = matrix[p * n + p];
                let aqq = matrix[q * n + q];
                let tau = (aqq - app) / (2. * apq);
                let tangent = if tau >= 0. { 1. } else { -1. } / (tau.abs() + tau.hypot(1.));
                let cosine = 1. / (1. + tangent * tangent).sqrt();
                let sine = tangent * cosine;
                for k in 0..n {
                    if k != p && k != q {
                        let akp = matrix[k * n + p];
                        let akq = matrix[k * n + q];
                        let p_new = cosine * akp - sine * akq;
                        let q_new = sine * akp + cosine * akq;
                        matrix[k * n + p] = p_new;
                        matrix[p * n + k] = p_new;
                        matrix[k * n + q] = q_new;
                        matrix[q * n + k] = q_new;
                    }
                    let vkp = vectors[k * n + p];
                    let vkq = vectors[k * n + q];
                    vectors[k * n + p] = cosine * vkp - sine * vkq;
                    vectors[k * n + q] = sine * vkp + cosine * vkq;
                }
                matrix[p * n + p] = app - tangent * apq;
                matrix[q * n + q] = aqq + tangent * apq;
                matrix[p * n + q] = 0.;
                matrix[q * n + p] = 0.;
            }
        }
        if !changed {
            break;
        }
    }
    let mut maximum_off_diagonal = 0_f64;
    for row in 0..n {
        for column in row + 1..n {
            maximum_off_diagonal = maximum_off_diagonal.max(matrix[row * n + column].abs());
        }
    }
    if maximum_off_diagonal > tolerance {
        return Err(Fault::unsupported(
            "Symmetric eigendecomposition did not converge",
        ));
    }

    let mut order = (0..n).collect::<Vec<_>>();
    order.sort_by(|&left, &right| matrix[right * n + right].total_cmp(&matrix[left * n + left]));
    let eigenvalues = order
        .iter()
        .map(|&source| matrix[source * n + source] * input_scale)
        .collect::<Vec<_>>();
    let mut sorted_vectors = Vec::with_capacity(n * n);
    for row in 0..n {
        for &source in &order {
            sorted_vectors.push(vectors[row * n + source]);
        }
    }

    // Refuse a plausible-looking result when the bounded independent solver
    // has not produced eigenpairs with a small normalized residual.
    let residual_limit = f64::EPSILON * 1024. * n as f64;
    for (column, &eigenvalue) in eigenvalues.iter().enumerate() {
        let normalized_eigenvalue = eigenvalue / input_scale;
        let residual = (0..n)
            .map(|row| {
                let applied = (0..n)
                    .map(|k| original[row * n + k] * sorted_vectors[k * n + column])
                    .sum::<f64>();
                (applied - normalized_eigenvalue * sorted_vectors[row * n + column]).abs()
            })
            .fold(0., f64::max);
        if !residual.is_finite() || residual > residual_limit {
            return Err(Fault::unsupported(
                "Symmetric eigendecomposition residual check failed",
            ));
        }
    }
    Ok(SymmetricEigensystem {
        size: n,
        values: eigenvalues,
        vectors: sorted_vectors,
        dimension,
    })
}

fn eigenvalues(value: &Value) -> Result<Value> {
    let SymmetricEigensystem {
        size,
        values,
        dimension,
        ..
    } = real_symmetric_eigendecomposition(value)?;
    matrix(
        size,
        1,
        values
            .into_iter()
            .map(|value| Quantity {
                value: Complex64::new(value, 0.),
                dimension,
            })
            .collect(),
    )
}

fn eigenvectors(value: &Value) -> Result<Value> {
    let SymmetricEigensystem { size, vectors, .. } = real_symmetric_eigendecomposition(value)?;
    matrix(
        size,
        size,
        vectors.into_iter().map(Quantity::scalar).collect(),
    )
}
fn identity(n: usize) -> Result<Value> {
    if n == 0 || n.checked_mul(n).is_none_or(|n| n > MAX_ELEMENTS) {
        return Err(Fault::unsupported(
            "Identity size must be a positive integer within the array budget",
        ));
    }
    let mut e = vec![Quantity::scalar(0.); n * n];
    for i in 0..n {
        e[i * n + i] = Quantity::scalar(1.);
    }
    matrix(n, n, e)
}
fn power(left: &Value, right: &Value) -> Result<Value> {
    if matches!(left, Value::Scalar { .. }) && matches!(right, Value::Scalar { .. }) {
        return mapped("pow", left, right);
    }
    if let Value::Matrix { cols: 1, .. } = left
        && matches!(right, Value::Scalar { .. })
    {
        return mapped("pow", left, right);
    }
    if matches!(left, Value::Scalar { .. }) && matches!(right, Value::Matrix { cols: 1, .. }) {
        return mapped("pow", left, right);
    }
    let (r, c, _) = array(left)?;
    if r != c {
        return Err(Fault::unsupported("Matrix power needs a square matrix"));
    }
    let exponent = integer(scalar(right)?)?;
    if exponent.unsigned_abs() > 1024 {
        return Err(Fault::unsupported("Matrix exponent budget exceeded"));
    }
    let mut result = identity(r)?;
    if exponent == 0 {
        return Ok(result);
    }
    let mut base = if exponent < 0 {
        inverse(left)?
    } else {
        left.clone()
    };
    let mut exponent = exponent.unsigned_abs();
    while exponent > 0 {
        super::check_cancelled()?;
        if exponent & 1 != 0 {
            result = multiply(&result, &base, false)?;
        }
        exponent >>= 1;
        if exponent > 0 {
            base = multiply(&base, &base, false)?;
        }
    }
    Ok(result)
}

fn compatible(v: &Value) -> Result<(usize, usize, &[Quantity])> {
    let (r, c, e) = array(v)?;
    if e.iter().any(|q| !q.same_dimension(&e[0])) {
        return Err(Fault::new(
            "bad%_dimensions",
            "Array norm requires compatible units",
        ));
    }
    Ok((r, c, e))
}
fn norm(v: &Value, kind: &str) -> Result<Value> {
    let (r, c, e) = compatible(v)?;
    if kind == "norm" && c != 1 {
        return Err(Fault::unsupported("norm requires a column vector"));
    }
    if kind != "norm" && r != c {
        return Err(Fault::unsupported("Matrix norm requires a square matrix"));
    }
    let value = match kind {
        "norm" | "norme" => e.iter().fold(0_f64, |sum, q| sum.hypot(q.value.norm())),
        "norm1" => (0..c)
            .map(|col| (0..r).map(|row| e[row * c + col].value.norm()).sum::<f64>())
            .fold(0., f64::max),
        "normi" => (0..r)
            .map(|row| (0..c).map(|col| e[row * c + col].value.norm()).sum::<f64>())
            .fold(0., f64::max),
        "norm2" => spectral_norm(r, e)?,
        _ => return Err(Fault::unsupported("Unknown array norm")),
    };
    Ok(Value::scalar(finite(Quantity {
        value: Complex64::new(value, 0.),
        dimension: e[0].dimension,
    })?))
}

// Cyclic Jacobi on the real representation of A* A. Convergence is checked;
// exceeding a bounded work/convergence budget produces an error, not an estimate.
fn spectral_norm(n: usize, e: &[Quantity]) -> Result<f64> {
    if n > 32 {
        return Err(Fault::unsupported(
            "Spectral norm currently limited to 32 by 32 matrices",
        ));
    }
    let input_scale = e
        .iter()
        .map(|q| q.value.re.hypot(q.value.im))
        .fold(0., f64::max);
    if input_scale == 0. {
        return Ok(0.);
    }
    let size = n * 2;
    let mut gram = vec![0.; size * size];
    for i in 0..n {
        for j in 0..n {
            let mut value = Complex64::new(0., 0.);
            for k in 0..n {
                value +=
                    (e[k * n + i].value / input_scale).conj() * (e[k * n + j].value / input_scale);
            }
            if !value.re.is_finite() || !value.im.is_finite() {
                return Err(Fault::unsupported("Nonfinite spectral norm intermediate"));
            }
            gram[i * size + j] = value.re;
            gram[i * size + j + n] = -value.im;
            gram[(i + n) * size + j] = value.im;
            gram[(i + n) * size + j + n] = value.re;
        }
    }
    for _ in 0..64 {
        super::check_cancelled()?;
        let scale = (0..size)
            .map(|i| gram[i * size + i].abs())
            .fold(0., f64::max);
        let tolerance = scale * 32. * f64::EPSILON;
        let mut changed = false;
        for p in 0..size {
            for q in p + 1..size {
                let apq = gram[p * size + q];
                if apq.abs() <= tolerance {
                    continue;
                }
                changed = true;
                let app = gram[p * size + p];
                let aqq = gram[q * size + q];
                let tau = (aqq - app) / (2. * apq);
                let t = if tau >= 0. { 1. } else { -1. } / (tau.abs() + tau.hypot(1.));
                let cosine = 1. / (1. + t * t).sqrt();
                let sine = t * cosine;
                for k in 0..size {
                    if k != p && k != q {
                        let akp = gram[k * size + p];
                        let akq = gram[k * size + q];
                        let p_new = cosine * akp - sine * akq;
                        let q_new = sine * akp + cosine * akq;
                        gram[k * size + p] = p_new;
                        gram[p * size + k] = p_new;
                        gram[k * size + q] = q_new;
                        gram[q * size + k] = q_new;
                    }
                }
                gram[p * size + p] = app - t * apq;
                gram[q * size + q] = aqq + t * apq;
                gram[p * size + q] = 0.;
                gram[q * size + p] = 0.;
            }
        }
        if !changed {
            return Ok((0..size)
                .map(|i| gram[i * size + i])
                .fold(0., f64::max)
                .sqrt()
                * input_scale);
        }
    }
    Err(Fault::unsupported(
        "Spectral norm Jacobi iteration did not converge",
    ))
}

pub fn unary(op: &str, value: &Value) -> Result<Value> {
    match op {
        // Original Percent applies Efi.Matrix.mult(x, 0.01); Degree lowers to
        // op_mult(x, mc_deg). Both preserve each cell's complex value and units.
        "percent" | "degree_op" => unary_map(value, |q| {
            let factor = if op == "percent" {
                0.01
            } else {
                std::f64::consts::PI / 180.
            };
            finite(q)?;
            finite(q.multiply(Quantity::scalar(factor), false).map_err(core)?)
        }),
        "neg" => unary_map(value, |mut q| {
            q.value = -q.value;
            finite(q)
        }),
        "conjugate" => unary_map(value, |mut q| {
            q.value = q.value.conj();
            finite(q)
        }),
        "transpose" => {
            let (r, c, e) = array(value)?;
            matrix(
                c,
                r,
                (0..c)
                    .flat_map(|col| (0..r).map(move |row| e[row * c + col]))
                    .collect(),
            )
        }
        // These presentation operators all route to Efi.Matrix.abs. Named
        // det()/norm() have different square/vector contracts below.
        "determinant" | "norm" => unary("absval", value),
        "absval" => match value {
            Value::Scalar { .. } => unary_map(value, |mut q| {
                q.value = Complex64::new(q.value.norm(), 0.);
                finite(q)
            }),
            _ => {
                let (r, c, _) = array(value)?;
                if c == 1 {
                    norm(value, "norm")
                } else if r == c {
                    determinant(value)
                } else {
                    Err(Fault::unsupported(
                        "Absolute/determinant operator needs a column vector or square matrix",
                    ))
                }
            }
        },
        _ => Err(Fault::unsupported(format!(
            "Unknown array unary operator {op}"
        ))),
    }
}

/// Original EfiMatrix.values.op_polar checks real scalar/column-vector angles
/// and nonnegative real magnitudes, then uses ordinary multiplication:
/// magnitude * exp(i * angle). An arrow outside this operation instead maps
/// scalar applications, so two vectors do not have the same semantics there.
pub fn polar(magnitude: &Value, angle: &Value) -> Result<Value> {
    let real = |q: Quantity, nonnegative: bool| {
        finite(q)?;
        if !q.is_scalar() {
            return Err(Fault::new(
                "bad%_dimensions%_unitless",
                "Polar operands must be dimensionless",
            ));
        }
        if q.value.im != 0. {
            return Err(Fault::new(
                "invalid_argument",
                "Polar operands must be real",
            ));
        }
        if nonnegative && q.value.re < 0. {
            return Err(Fault::new(
                "invalid_argument",
                "Polar magnitude must be non-negative",
            ));
        }
        Ok(q)
    };
    // Native check_real_v visits the angle's cells before the vector guard.
    match angle {
        Value::Scalar { quantity } => {
            real(*quantity, false)?;
        }
        _ => {
            let (_, cols, cells) = array(angle)?;
            for q in cells {
                real(*q, false)?;
            }
            if cols != 1 {
                return Err(Fault::new(
                    "invalid_argument",
                    "Polar angle must be a scalar or column vector",
                ));
            }
        }
    }
    match magnitude {
        Value::Scalar { quantity } => {
            real(*quantity, true)?;
        }
        _ => {
            let (_, _, cells) = array(magnitude)?;
            for q in cells {
                real(*q, true)?;
            }
        }
    }
    let phasor = unary_map(angle, |q| {
        let value = Complex64::new(0., q.value.re).exp();
        finite(Quantity {
            value,
            ..Quantity::scalar(0.)
        })
    })?;
    binary("mult", magnitude, &phasor)
}

/// Native row/column operators preserve the selected cells' units and complex
/// values, and return a vector with the corresponding orientation. This is not
/// a named function alias: its index depends on SYSTEM ORIGIN.
pub fn row_or_column(op: &str, value: &Value, index: Quantity, origin: Quantity) -> Result<Value> {
    let (rows, cols, elements) = array(value)?;
    let offset = index_offset(index, origin)?;
    match op {
        "matrow" if offset < rows => matrix(
            1,
            cols,
            elements[offset * cols..(offset + 1) * cols].to_vec(),
        ),
        "matcol" if offset < cols => matrix(
            rows,
            1,
            (0..rows).map(|row| elements[row * cols + offset]).collect(),
        ),
        "matrow" | "matcol" => Err(Fault::unsupported(
            "Matrix row/column index outside implemented range",
        )),
        _ => Err(Fault::unsupported(format!(
            "Unknown row/column operator {op}"
        ))),
    }
}

/// The native cross-product contract is two uniform, three-element column
/// vectors. Complex multiplication is bilinear, without conjugating an input.
pub fn cross_product(left: &Value, right: &Value) -> Result<Value> {
    let (lr, lc, a) = compatible(left)?;
    let (rr, rc, b) = compatible(right)?;
    if (lr, lc) != (3, 1) || (rr, rc) != (3, 1) {
        return Err(Fault::unsupported(
            "Cross product requires two three-element column vectors",
        ));
    }
    let mut result = Vec::with_capacity(3);
    for (i, j) in [(1, 2), (2, 0), (0, 1)] {
        super::check_cancelled()?;
        let positive = a[i].multiply(b[j], false).map_err(core)?;
        let negative = a[j].multiply(b[i], false).map_err(core)?;
        result.push(finite(positive.add(negative, true).map_err(core)?)?);
    }
    matrix(3, 1, result)
}

/// Apply one already identified operation to scalar cells. The caller walks
/// the expression and supplies the operation; evaluating a matrix operation
/// first and mapping its result would have different semantics.
pub(crate) fn vectorize(
    arguments: &[Value],
    mut operation: impl FnMut(&[Value]) -> Result<Value>,
) -> Result<Value> {
    let mut shape = None;
    for value in arguments {
        match value {
            Value::Scalar { quantity } => {
                finite(*quantity)?;
            }
            Value::Matrix { .. } => {
                let (rows, cols, _) = array(value)?;
                if shape.is_some_and(|old| old != (rows, cols)) {
                    return Err(Fault::unsupported(
                        "Vectorized arrays must have identical rows and columns",
                    ));
                }
                shape = Some((rows, cols));
            }
            _ => {
                return Err(Fault::unsupported(
                    "Vectorization requires numeric scalars or matrices; nested/range values are not supported",
                ));
            }
        }
    }
    let Some((rows, cols)) = shape else {
        return operation(arguments);
    };
    let mut point = vec![Value::scalar(Quantity::scalar(0.)); arguments.len()];
    let mut result = Vec::with_capacity(rows * cols);
    // The original MatVal.vectorize visits columns, then rows. Preserve that
    // operation/error order while storing the independent result row-major.
    result.resize(rows * cols, Quantity::scalar(0.));
    for col in 0..cols {
        for row in 0..rows {
            super::check_cancelled()?;
            let index = row * cols + col;
            for (slot, argument) in point.iter_mut().zip(arguments) {
                let quantity = match argument {
                    Value::Scalar { quantity } => *quantity,
                    Value::Matrix { elements, .. } => elements[index],
                    _ => unreachable!(),
                };
                *slot = Value::scalar(quantity);
            }
            let value = operation(&point)?;
            result[index] = finite(value.as_quantity().map_err(|_| {
                Fault::unsupported("Vectorized operation returned a nested/non-scalar result")
            })?)?;
        }
    }
    matrix(rows, cols, result)
}

pub fn is_builtin(name: &str) -> bool {
    matches!(
        name,
        "abs"
            | "rows"
            | "cols"
            | "length"
            | "last"
            | "identity"
            | "augment"
            | "stack"
            | "submatrix"
            | "det"
            | "norm"
            | "norm1"
            | "norm2"
            | "norme"
            | "normi"
            | "diag"
            | "tr"
            | "lsolve"
            | "eigenvals"
            | "eigenvecs"
    )
}

/// Dispatch after checking user bindings with the same labelled identity.
/// Only last/submatrix consult SYSTEM ORIGIN; other functions ignore it.
pub fn call(name: &str, args: &[Value], origin: Quantity) -> Result<Value> {
    match (name, args) {
        ("abs", [v]) => {
            let mut q = scalar(v)?;
            q.value = Complex64::new(q.value.norm(), 0.);
            Ok(Value::scalar(finite(q)?))
        }
        ("rows", [v]) => Ok(Value::scalar(Quantity::scalar(array(v)?.0 as f64))),
        ("cols", [v]) => Ok(Value::scalar(Quantity::scalar(array(v)?.1 as f64))),
        ("length" | "last", [v]) => {
            let (r, c, _) = array(v)?;
            if c != 1 {
                return Err(Fault::unsupported(
                    "Vector size function requires a column vector",
                ));
            }
            let n = if name == "last" {
                r as i64 - 1 + integer(origin)?
            } else {
                r as i64
            };
            Ok(Value::scalar(Quantity::scalar(n as f64)))
        }
        ("identity", [n]) => {
            let n = integer(scalar(n)?)?;
            if n <= 0 {
                return Err(Fault::unsupported("Identity size must be positive"));
            }
            identity(n as usize)
        }
        ("transpose", [v]) => unary("transpose", v),
        ("det", [v]) => {
            if array(v)?.2.iter().any(|q| !q.is_scalar()) {
                return Err(Fault::new(
                    "bad%_dimensions%_unitless",
                    "Named det requires a dimensionless matrix in the shipped MPL interface",
                ));
            }
            determinant(v)
        }
        ("inverse", [v]) => inverse(v),
        ("lsolve", [a, b]) => lsolve(a, b),
        ("eigenvals", [v]) => eigenvalues(v),
        ("eigenvecs", [v]) => eigenvectors(v),
        (kind @ ("norm" | "norm1" | "norm2" | "norme" | "normi"), [v]) => norm(v, kind),
        ("augment" | "stack", vs) if !vs.is_empty() => join(vs, name == "augment"),
        ("submatrix", [v, ir, jr, ic, jc]) => submatrix(
            v,
            [scalar(ir)?, scalar(jr)?, scalar(ic)?, scalar(jc)?],
            origin,
        ),
        ("diag", [v]) => {
            let (r, c, e) = array(v)?;
            if c == 1 {
                // Off-diagonal numeric zero retains the common physical unit.
                if e.iter().any(|q| !q.same_dimension(&e[0])) {
                    return Err(Fault::unsupported(
                        "Mixed-dimensional diag construction is not implemented",
                    ));
                }
                let mut zero = e[0];
                zero.value = Complex64::new(0., 0.);
                let mut elements = vec![
                    zero;
                    r.checked_mul(r).filter(|n| *n <= MAX_ELEMENTS).ok_or_else(
                        || Fault::unsupported("diag array budget exceeded")
                    )?
                ];
                for i in 0..r {
                    elements[i * r + i] = e[i];
                }
                matrix(r, r, elements)
            } else if r == c {
                matrix(r, 1, (0..r).map(|i| e[i * c + i]).collect())
            } else {
                Err(Fault::unsupported(
                    "diag expects a column vector or square matrix",
                ))
            }
        }
        ("tr", [v]) => {
            let (r, c, e) = array(v)?;
            if r != c {
                return Err(Fault::unsupported("Trace requires a square matrix"));
            }
            let mut sum = None;
            for i in 0..r {
                sum = Some(add_terms(sum, e[i * c + i])?);
            }
            Ok(Value::scalar(sum.unwrap()))
        }
        _ => Err(Fault::unsupported(format!(
            "Unknown array function or argument count: {name}/{}",
            args.len()
        ))),
    }
}

fn join(vs: &[Value], horizontal: bool) -> Result<Value> {
    let mut parts = Vec::with_capacity(vs.len());
    for v in vs {
        parts.push(match v {
            Value::Scalar { quantity } => (1, 1, std::slice::from_ref(quantity)),
            _ => array(v)?,
        });
    }
    let (rows, cols) = if horizontal {
        let rows = parts[0].0;
        if parts.iter().any(|(r, _, _)| *r != rows) {
            return Err(Fault::unsupported("augment requires equal row counts"));
        }
        (
            rows,
            parts
                .iter()
                .try_fold(0_usize, |a, (_, c, _)| a.checked_add(*c))
                .ok_or_else(|| Fault::unsupported("augment size overflow"))?,
        )
    } else {
        let cols = parts[0].1;
        if parts.iter().any(|(_, c, _)| *c != cols) {
            return Err(Fault::unsupported("stack requires equal column counts"));
        }
        (
            parts
                .iter()
                .try_fold(0_usize, |a, (r, _, _)| a.checked_add(*r))
                .ok_or_else(|| Fault::unsupported("stack size overflow"))?,
            cols,
        )
    };
    if rows.checked_mul(cols).is_none_or(|n| n > MAX_ELEMENTS) {
        return Err(Fault::unsupported("Joined array budget exceeded"));
    }
    let mut result = Vec::with_capacity(rows * cols);
    if horizontal {
        for row in 0..rows {
            for (_, c, e) in &parts {
                result.extend_from_slice(&e[row * c..(row + 1) * c]);
            }
        }
    } else {
        for (_, _, e) in parts {
            result.extend_from_slice(e);
        }
    }
    matrix(rows, cols, result)
}
fn submatrix(v: &Value, bounds: [Quantity; 4], origin: Quantity) -> Result<Value> {
    let (r, c, e) = array(v)?;
    let origin = integer(origin)?;
    let mut indices = [0_i64; 4];
    for (index, bound) in indices.iter_mut().zip(bounds) {
        *index = integer(bound)? - origin;
    }
    let [ir, jr, ic, jc] = indices;
    if ir < 0
        || jr < 0
        || ic < 0
        || jc < 0
        || ir >= r as i64
        || jr >= r as i64
        || ic >= c as i64
        || jc >= c as i64
    {
        return Err(Fault::unsupported("submatrix index outside array bounds"));
    }
    let rows = (jr - ir).unsigned_abs() as usize + 1;
    let cols = (jc - ic).unsigned_abs() as usize + 1;
    let rstep = if jr >= ir { 1 } else { -1 };
    let cstep = if jc >= ic { 1 } else { -1 };
    let mut result = Vec::with_capacity(rows * cols);
    for row in 0..rows {
        for col in 0..cols {
            result.push(
                e[(ir + row as i64 * rstep) as usize * c + (ic + col as i64 * cstep) as usize],
            );
        }
    }
    matrix(rows, cols, result)
}

#[cfg(test)]
mod vectorization_tests {
    use super::*;
    #[test]
    fn vectorized_callbacks_use_native_column_order_and_mid_map_stop_never_publishes_partial_output()
     {
        let input = Value::Matrix {
            rows: 2,
            cols: 2,
            elements: [1., 2., 3., 4.].map(Quantity::scalar).to_vec(),
        };
        let before = input.numeric_json();
        let control = crate::CalculationControl::default();
        let mut visited = Vec::new();
        let stopped = control.run(|| {
            vectorize(std::slice::from_ref(&input), |point| {
                visited.push(point[0].as_quantity()?.value.re);
                if visited.len() == 2 {
                    control.cancel();
                }
                Ok(point[0].clone())
            })
        });
        assert!(stopped.unwrap_err().is_cancelled());
        assert_eq!(visited, [1., 3.]);
        assert_eq!(input.numeric_json(), before);
        let completed =
            vectorize(std::slice::from_ref(&input), |point| Ok(point[0].clone())).unwrap();
        assert_eq!(completed.numeric_json(), before);
    }
}
