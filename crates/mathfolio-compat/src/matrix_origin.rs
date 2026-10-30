//! Live-result metadata only. Installed provenance: matrix-result-origin-next-20261020.
//! Scope reading is the already-used array index policy, never an extra query.
//! Original arbitrary source-assignment callback ordering remains unproven;
//! this precisely records the actual Rust execution scope instead.
use super::*;

pub(super) fn capture(scope: &Scope, row: &RegionEvaluation) -> Option<i32> {
    if row.error.is_some()
        || !matches!(row.value.as_ref(), Some(Value::Matrix { .. }))
            && !matches!(row.display_value.as_ref(), Some(Value::Matrix { .. }))
    {
        return None;
    }
    // Scope.origin checks label-sensitive stored binding, scalar/unitless/real,
    // finite integral admission. It never evaluates a variable or builtin call.
    let n = scope.origin().ok()?.value.re;
    if n < i32::MIN as f64 || n > i32::MAX as f64 {
        return None;
    }
    Some(n as i32)
}

#[cfg(test)]
mod tests;
