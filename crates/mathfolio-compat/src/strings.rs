//! Independent bounded scalar string functions from installed CIL. Evidence:
//! artifacts/rea/string-engine-next-contract-20261019.{md,json}.
//! Text is inert. UTF-16 indices are not UTF-8 bytes or grapheme indices.
use super::{Fault, Quantity, Result, Value};
use roxmltree::Node;
pub const MAX_STRING_BYTES: usize = 1024 * 1024;
pub fn is_implemented(name: &str) -> bool {
    matches!(name, "concat" | "strlen" | "substr" | "search" | "str2num")
}
pub fn is_builtin(name: &str) -> bool {
    matches!(
        name,
        "concat" | "strlen" | "substr" | "search" | "str2num" | "num2str"
    )
}
pub fn validate(value: &str) -> Result<()> {
    if value.len() > MAX_STRING_BYTES {
        return Err(Fault::unsupported("String value exceeds 1 MiB budget"));
    }
    if value
        .chars()
        .any(|c| !matches!(c as u32,9|10|13|0x20..=0xD7FF|0xE000..=0xFFFD|0x10000..=0x10FFFF))
    {
        return Err(Fault::unsupported(
            "String contains an XML-invalid character",
        ));
    }
    Ok(())
}
pub(super) fn parse_literal(node: Node<'_, '_>) -> Result<String> {
    if node.attributes().any(|a| {
        a.namespace() != Some("http://www.w3.org/XML/1998/namespace")
            || a.name() != "space"
            || a.value() != "preserve"
    }) {
        return Err(Fault::unsupported("Unsupported string literal metadata"));
    }
    if node.children().any(|n| !n.is_text()) {
        return Err(Fault::unsupported(
            "Structured string literal is unsupported",
        ));
    }
    let value = node.children().filter_map(|n| n.text()).collect::<String>();
    validate(&value)?;
    Ok(value)
}
fn invalid(message: impl Into<String>) -> Fault {
    Fault::new("invalid_argument", message)
}
fn text(value: &Value, argument: usize) -> Result<&str> {
    match value {
        Value::String { value } => {
            validate(value)?;
            Ok(value)
        }
        _ => Err(invalid(format!("must be string (argument {argument})"))),
    }
}
fn integer(value: &Value, argument: usize) -> Result<i32> {
    let Value::Scalar { quantity } = value else {
        return Err(invalid(format!("must be integer (argument {argument})")));
    };
    let n = quantity.value.re;
    if !quantity.is_scalar()
        || quantity.value.im != 0.
        || !n.is_finite()
        || n.fract() != 0.
        || n < i32::MIN as f64
        || n > i32::MAX as f64
    {
        return Err(invalid(format!(
            "must be dimensionless 32-bit integer (argument {argument})"
        )));
    }
    Ok(n as i32)
}
fn string(value: String) -> Result<Value> {
    validate(&value)?;
    Ok(Value::String {
        value: value.into(),
    })
}
fn number(value: usize) -> Value {
    Value::scalar(Quantity::scalar(value as f64))
}
fn offset(value: &Value, origin: i32, argument: usize) -> Result<i64> {
    Ok(i64::from(integer(value, argument)?) - i64::from(origin))
}
fn byte_at_utf16(value: &str, units: usize) -> Result<usize> {
    let mut seen = 0;
    for (byte, c) in value.char_indices() {
        if seen == units {
            return Ok(byte);
        }
        seen += c.len_utf16();
        if seen > units {
            return Err(Fault::unsupported(
                "Substring splits a UTF-16 surrogate pair",
            ));
        }
    }
    if seen == units {
        Ok(value.len())
    } else {
        Err(invalid("index out of bounds (argument 2)"))
    }
}
/// origin is already zero unless the explicit strings-use-origin setting is on.
pub fn call(name: &str, args: &[Value], origin: i32) -> Result<Value> {
    super::check_cancelled()?;
    match (name, args) {
        ("concat", values) if !values.is_empty() => {
            let mut bytes = 0usize;
            for (i, v) in values.iter().enumerate() {
                bytes = bytes
                    .checked_add(text(v, i + 1)?.len())
                    .ok_or_else(|| Fault::unsupported("String concatenation overflow"))?;
                if bytes > MAX_STRING_BYTES {
                    return Err(Fault::unsupported(
                        "String concatenation exceeds 1 MiB budget",
                    ));
                }
            }
            let mut result = String::with_capacity(bytes);
            for (i, v) in values.iter().enumerate() {
                result.push_str(text(v, i + 1)?)
            }
            string(result)
        }
        ("strlen", [value]) => Ok(number(text(value, 1)?.encode_utf16().count())),
        ("substr", [value, start, count]) => {
            let count = integer(count, 3)?;
            let start = offset(start, origin, 2)?;
            let value = text(value, 1)?;
            if count < 0 {
                return Err(invalid("must be non-negative (argument 3)"));
            }
            let len = value.encode_utf16().count();
            if start < 0 || start > len as i64 {
                return Err(invalid("index out of bounds (argument 2)"));
            }
            let start = start as usize;
            let end = start + (count as usize).min(len - start);
            if end == start {
                return string(String::new());
            }
            let a = byte_at_utf16(value, start)?;
            let b = byte_at_utf16(value, end)?;
            string(value[a..b].into())
        }
        ("search", [value, needle, start]) => {
            let start = offset(start, origin, 3)?;
            let needle = text(needle, 2)?;
            let value = text(value, 1)?;
            // Native IndexOf(string,int) has culture-sensitive collation. ASCII
            // is the initial positive subset; do not invent Unicode byte find.
            if !value.is_ascii() || !needle.is_ascii() {
                return Err(Fault::unsupported(
                    "Non-ASCII string search collation is not implemented",
                ));
            }
            if start < 0 || start > value.len() as i64 {
                return Err(invalid("index out of bounds (argument 3)"));
            }
            let start = start as usize;
            let found = value[start..]
                .find(needle)
                .map(|n| (n + start) as i64)
                .unwrap_or(-1);
            Ok(Value::scalar(Quantity::scalar(
                (found + i64::from(origin)) as f64,
            )))
        }
        ("str2num", [value]) => {
            let value = text(value, 1)?;
            let compact = value.chars().filter(|c| c.is_ascii_whitespace()).count();
            let clean = if compact == 0 {
                std::borrow::Cow::Borrowed(value)
            } else {
                std::borrow::Cow::Owned(
                    value
                        .chars()
                        .filter(|c| !c.is_ascii_whitespace())
                        .collect::<String>(),
                )
            };
            // Explicit decimal subset only. Never parse units or worksheet code.
            if !clean.is_ascii()
                || clean.is_empty()
                || clean
                    .bytes()
                    .any(|b| !b.is_ascii_digit() && !matches!(b, b'+' | b'-' | b'.' | b'e' | b'E'))
            {
                return Err(Fault::unsupported(
                    "str2num supports finite invariant decimal strings only",
                ));
            }
            let n = clean
                .parse::<f64>()
                .map_err(|_| invalid("invalid argument (argument 1)"))?;
            if !n.is_finite() {
                return Err(Fault::unsupported("Nonfinite str2num is unsupported"));
            }
            Ok(Value::scalar(Quantity::scalar(n)))
        }
        ("num2str", [_]) => Err(Fault::unsupported(
            "num2str formatting/culture contract is not implemented",
        )),
        _ => Err(invalid("String function argument count")),
    }
}
