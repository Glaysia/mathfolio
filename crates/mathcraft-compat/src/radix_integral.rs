//! Independent bounded radix decoder; original source parser is not recovered.
//! Used for explicit native radix literals, independently of editor entry mode.
//! Pure exact unsigned-integral subset; no name/unit/worksheet evaluation.
//! Original fractional, signed-in-text, overflow and rounding rules unresolved.
use std::fmt;
/// Independent lexical resource cap, not an original Mathcad digit limit.
pub const MAX_RADIX_SOURCE_BYTES: usize = 65_536;
/// Independent exact-f64 capability cap, not an original Mathcad value limit.
pub const MAX_EXACT_INTEGER: u64 = 1_u64 << 53;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnsupportedRadix {
    Base,
    SourceBudget,
    Empty,
    Digit,
    ExactIntegerBoundary,
}
impl fmt::Display for UnsupportedRadix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Base => "Only explicit base 2, 8 and 16 integral literals are supported",
            Self::SourceBudget => "Radix literal exceeds independent source byte budget",
            Self::Empty => "Empty radix literal is unsupported",
            Self::Digit => "Radix literal requires unsigned ASCII integral digits; fractions, signs and separators are unsupported",
            Self::ExactIntegerBoundary => "Radix integer exceeds independent exact-f64 boundary",
        })
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RadixIntegral<'a> {
    digits: &'a str,
    base: u8,
    integer: u64,
}
impl<'a> RadixIntegral<'a> {
    pub fn digits(self) -> &'a str {
        self.digits
    }
    pub fn base(self) -> u8 {
        self.base
    }
    pub fn integer(self) -> u64 {
        self.integer
    }
    pub fn exact_f64(self) -> f64 {
        self.integer as f64
    }
}
/// Preserve the borrowed lexeme (case/leadingzeros) independently from value.
/// Explicit10/absentbase stay in the existing decimal parser; do not route them
/// here, strip their attrs, or reinterpret arbitrarytext as a decimal fallback.
pub fn decode_unsigned_integral(
    digits: &str,
    base: u8,
) -> Result<RadixIntegral<'_>, UnsupportedRadix> {
    if !matches!(base, 2 | 8 | 16) {
        return Err(UnsupportedRadix::Base);
    }
    if digits.len() > MAX_RADIX_SOURCE_BYTES {
        return Err(UnsupportedRadix::SourceBudget);
    }
    if digits.is_empty() {
        return Err(UnsupportedRadix::Empty);
    }
    // Validate the complete lexical field before arithmetic: a late invalid
    // character must not become an arbitrary partially consumed integer.
    let digit = |byte: u8| -> Option<u64> {
        let n = match byte {
            b'0'..=b'9' => byte - b'0',
            b'a'..=b'f' => byte - b'a' + 10,
            b'A'..=b'F' => byte - b'A' + 10,
            _ => return None,
        };
        (n < base).then_some(u64::from(n))
    };
    if digits.bytes().any(|b| digit(b).is_none()) {
        return Err(UnsupportedRadix::Digit);
    }
    let mut integer = 0_u64;
    for byte in digits.bytes() {
        // Long leading zero runs never overflow or trigger a significant-digit
        // heuristic. Check each exact arithmetic step, not lexical digit count.
        integer = integer
            .checked_mul(u64::from(base))
            .and_then(|n| n.checked_add(digit(byte)?))
            .filter(|n| *n <= MAX_EXACT_INTEGER)
            .ok_or(UnsupportedRadix::ExactIntegerBoundary)?;
    }
    Ok(RadixIntegral {
        digits,
        base,
        integer,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lexical_identity_is_separate_from_exact_value() {
        for (text, base, expected) in [
            ("001101", 2, 13),
            ("00032", 8, 26),
            ("0a32", 16, 2610),
            ("2B3c", 16, 11068),
        ] {
            let result = decode_unsigned_integral(text, base).unwrap();
            assert_eq!(result.digits(), text);
            assert_eq!(result.base(), base);
            assert_eq!(result.integer(), expected);
            assert_eq!(result.exact_f64(), expected as f64);
            assert_eq!(result.digits().as_ptr(), text.as_ptr());
        }
    }
    #[test]
    fn exact_boundary_and_next_value_are_distinct_without_rounding() {
        for (text, base) in [
            (format!("1{}", "0".repeat(53)), 2),
            ("400000000000000000".into(), 8),
            ("20000000000000".into(), 16),
        ] {
            assert_eq!(
                decode_unsigned_integral(&text, base).unwrap().integer(),
                MAX_EXACT_INTEGER
            );
        }
        for (text, base) in [
            (format!("1{}1", "0".repeat(52)), 2),
            ("400000000000000001".into(), 8),
            ("20000000000001".into(), 16),
        ] {
            assert_eq!(
                decode_unsigned_integral(&text, base),
                Err(UnsupportedRadix::ExactIntegerBoundary)
            );
        }
    }
    #[test]
    fn long_leading_zeros_do_not_create_a_false_numeric_overflow() {
        let text = format!("{}1", "0".repeat(MAX_RADIX_SOURCE_BYTES - 1));
        assert_eq!(decode_unsigned_integral(&text, 2).unwrap().integer(), 1);
        let oversized = format!("0{text}");
        assert_eq!(
            decode_unsigned_integral(&oversized, 2),
            Err(UnsupportedRadix::SourceBudget)
        );
    }
    #[test]
    fn whole_input_must_be_unsigned_ascii_integral_digits() {
        assert_eq!(
            decode_unsigned_integral("", 2),
            Err(UnsupportedRadix::Empty)
        );
        for (text, base) in [
            ("2", 2),
            ("8", 8),
            ("g", 16),
            ("1.1", 2),
            ("-1", 2),
            ("+1", 16),
            ("1_0", 2),
            (" 1", 2),
            ("1b", 2),
            ("0x10", 16),
            ("１", 2),
            ("1i", 2),
            ("1\0", 2),
        ] {
            assert_eq!(
                decode_unsigned_integral(text, base),
                Err(UnsupportedRadix::Digit),
                "{text:?}/{base}"
            );
        }
        for base in [0, 3, 10, 32, 255] {
            assert_eq!(
                decode_unsigned_integral("10", base),
                Err(UnsupportedRadix::Base)
            );
        }
    }
}
