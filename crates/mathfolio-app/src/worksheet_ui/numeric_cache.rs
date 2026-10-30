//! Inert result data for changing display format without executing input.
use super::*;
use mathfolio_document::canvas::SavedNumericSource;

const MAX_BYTES: usize = 4 * 1024 * 1024;

fn valid(value: &mathfolio_compat::Value) -> bool {
    use mathfolio_compat::Value;
    let quantity = |q: &mathfolio_core::Quantity| {
        q.value.re.is_finite()
            && q.value.im.is_finite()
            && q.dimension.iter().all(|d| d.is_finite())
    };
    match value {
        Value::String { value } => {
            value.len() <= MAX_BYTES
                && value.chars().all(|c| {
                    matches!(c, '\t' | '\n' | '\r')
                        || c >= ' ' && !matches!(c, '\u{fffe}' | '\u{ffff}')
                })
        }
        Value::Scalar { quantity: q } => quantity(q),
        Value::Matrix {
            rows,
            cols,
            elements,
        } => {
            *rows > 0
                && *cols > 0
                && rows.checked_mul(*cols) == Some(elements.len())
                && elements.len() <= 65_536
                && elements.iter().all(quantity)
        }
        Value::Range {
            start, step, end, ..
        } => quantity(start) && quantity(step) && quantity(end),
        Value::RangeValues { elements } => {
            elements.len() <= 65_536 && elements.iter().all(quantity)
        }
        Value::Symbolic { mathml } => mathml.len() <= MAX_BYTES,
    }
}

pub(super) fn source(value: &mathfolio_compat::Value) -> Option<SavedNumericSource> {
    if !valid(value) {
        return None;
    }
    let source = serde_json::to_string(value).ok()?;
    (source.len() <= MAX_BYTES).then_some(SavedNumericSource::EvaluatedValue(source))
}

pub(super) fn display(
    row: &SavedCalculationRow,
    style: &MathFormat,
) -> Option<crate::result_display::FormattedResult> {
    if row.error.is_some() {
        return None;
    }
    let source = row.numeric_source.as_ref()?;
    match source {
        SavedNumericSource::AreaPresentation(_) => None,
        SavedNumericSource::NativeMathml(xml) => (xml.len() <= MAX_BYTES)
            .then(|| crate::result_display::from_cached_native_result(xml, style))
            .flatten(),
        SavedNumericSource::EvaluatedValue(source) => {
            if source.len() > MAX_BYTES {
                return None;
            }
            let value: mathfolio_compat::Value = serde_json::from_str(source).ok()?;
            valid(&value).then(|| crate::result_display::format(&value, style))
        }
    }
}
