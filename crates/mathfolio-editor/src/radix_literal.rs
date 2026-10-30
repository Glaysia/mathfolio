//! NUMERIC source lexemes from E 0bbb/119c/124f. This does not turn ordinary
//! Number text into radix input or decide engine digit/overflow semantics.
use crate::{NodeKind, TreeError};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum NumericBase {
    Binary,
    Octal,
    Hexadecimal,
}

impl NumericBase {
    pub const fn radix(self) -> u8 {
        match self {
            Self::Binary => 2,
            Self::Octal => 8,
            Self::Hexadecimal => 16,
        }
    }
    pub const fn suffix(self) -> &'static str {
        match self {
            Self::Binary => "b",
            Self::Octal => "o",
            Self::Hexadecimal => "h",
        }
    }
    pub const fn from_radix(radix: u8) -> Option<Self> {
        match radix {
            2 => Some(Self::Binary),
            8 => Some(Self::Octal),
            16 => Some(Self::Hexadecimal),
            _ => None,
        }
    }

    /// Borrow exactly the digits of a complete source literal. A mismatched,
    /// absent or uppercase suffix is not this canonical based representation.
    pub fn digits(self, literal: &str) -> Option<&str> {
        literal
            .strip_suffix(self.suffix())
            .filter(|digits| self.accepts_lexeme(digits))
    }

    /// Lexical admission only: NUMERIC's original decimal/scientific branch
    /// admits digits outside base 2/8. Their computation remains an engine error.
    pub fn accepts_lexeme(self, digits: &str) -> bool {
        if decimal_lexeme(digits) {
            return true;
        }
        if self != Self::Hexadecimal {
            return false;
        }
        let hex = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_hexdigit());
        if let Some(tail) = digits.strip_prefix('.') {
            return hex(tail);
        }
        if !digits.as_bytes().first().is_some_and(u8::is_ascii_digit) {
            return false;
        }
        match digits.split_once('.') {
            Some((whole, fraction)) => hex(whole) && hex(fraction),
            None => hex(digits),
        }
    }
}

fn decimal_lexeme(text: &str) -> bool {
    let mut parts = text.split('E');
    let magnitude = parts.next().unwrap_or("");
    let exponent = parts.next();
    if parts.next().is_some() {
        return false;
    }
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    let mantissa = match magnitude.split_once('.') {
        Some((whole, fraction)) => {
            (whole.is_empty() || digits(whole))
                && (fraction.is_empty() || digits(fraction))
                && !(whole.is_empty() && fraction.is_empty())
        }
        None => digits(magnitude),
    };
    mantissa && exponent.is_none_or(|e| digits(e.strip_prefix(['+', '-']).unwrap_or(e)))
}

impl NodeKind {
    pub fn based_number(digits: impl Into<String>, base: NumericBase) -> Result<Self, TreeError> {
        let digits = digits.into();
        if digits.len().saturating_add(base.suffix().len()) > crate::MAX_ATOM_BYTES {
            return Err(TreeError::TextTooLong);
        }
        if !base.accepts_lexeme(&digits) {
            return Err(TreeError::InvalidNumber);
        }
        let mut literal = digits;
        literal.push_str(base.suffix());
        Ok(Self::BasedNumber { literal, base })
    }

    pub fn based_digits(&self) -> Option<&str> {
        match self {
            Self::BasedNumber { literal, base } => base.digits(literal),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Direction, Editor, PortableTree};

    #[test]
    fn lexical_source_preserves_case_fractions_and_pending_engine_errors() {
        for (base, good) in [
            (
                NumericBase::Binary,
                vec!["001101", "2", "10.01", ".1", "1.E+2"],
            ),
            (NumericBase::Octal, vec!["032", "8", "7.4", "1E-2"]),
            (
                NumericBase::Hexadecimal,
                vec!["0a32", "1.Af", ".F", "0E", "1E+2"],
            ),
        ] {
            for text in good {
                let kind = NodeKind::based_number(text, base).unwrap();
                let tree = PortableTree {
                    kind,
                    children: vec![],
                };
                tree.validate().unwrap();
                assert_eq!(tree.kind.based_digits(), Some(text));
                assert_eq!(
                    tree.kind.text(),
                    Some(format!("{text}{}", base.suffix()).as_str())
                );
                assert_eq!(tree.kind.descriptor_name(), "NUMERIC");
                assert_eq!(tree.kind.precedence(), i32::MAX);
            }
        }
        for bad in [
            "", "+1", "-1", "1b", "1 h", "0x10", "NaN", "Infinity", "1e2", "1..0", "1E", "1E++2",
            "FF", "𝟙",
        ] {
            assert!(
                NodeKind::based_number(bad, NumericBase::Binary).is_err(),
                "{bad}"
            );
        }
        assert!(NodeKind::based_number("FF", NumericBase::Hexadecimal).is_err());
        assert!(NodeKind::based_number("1.A", NumericBase::Octal).is_err());
        assert!(
            NodeKind::based_number("0".repeat(crate::MAX_ATOM_BYTES + 1), NumericBase::Binary)
                .is_err()
        );
    }

    #[test]
    fn wire_caret_selection_and_history_do_not_reinterpret_decimal() {
        assert_eq!(
            serde_json::to_string(&NodeKind::Number("12".into())).unwrap(),
            r#"{"Number":"12"}"#
        );
        let original = PortableTree {
            kind: NodeKind::based_number("0a32", NumericBase::Hexadecimal).unwrap(),
            children: vec![],
        };
        let serialized = serde_json::to_string(&original).unwrap();
        assert_eq!(
            serde_json::from_str::<PortableTree>(&serialized).unwrap(),
            original
        );
        let mut editor = Editor::from_portable(&original).unwrap();
        let root = editor.root();
        // The suffix is an ordinary NUMERIC UTF16 character, not decoration.
        editor.place_text_selection(root, 5, 4).unwrap();
        assert_eq!(editor.caret().value_range.unwrap().anchor, 5);
        editor
            .set_atom(
                root,
                NodeKind::based_number("0AbC", NumericBase::Hexadecimal).unwrap(),
            )
            .unwrap();
        assert!(editor.undo());
        assert_eq!(editor.to_portable().unwrap(), original);
        assert_eq!(editor.caret().value_range.unwrap().focus, 4);
        assert!(editor.redo());
        assert_eq!(editor.caret().caret_side, Direction::Right);
        assert_eq!(editor.to_portable().unwrap().kind.text(), Some("0AbCh"));
    }

    #[test]
    fn complete_literal_requires_matching_lowercase_suffix_and_counts_it_in_budget() {
        let valid = NodeKind::based_number("001", NumericBase::Binary).unwrap();
        assert_eq!(
            serde_json::to_string(&valid).unwrap(),
            r#"{"BasedNumber":{"literal":"001b","base":"Binary"}}"#
        );
        assert_eq!(NumericBase::Binary.digits("001b"), Some("001"));
        assert_eq!(valid.based_digits(), Some("001"));
        assert_eq!(NodeKind::Number("1".into()).based_digits(), None);
        for literal in ["", "b", "1", "1B", "1h", "1bb", "1 b", "-1b"] {
            let forged = PortableTree {
                kind: NodeKind::BasedNumber {
                    literal: literal.into(),
                    base: NumericBase::Binary,
                },
                children: vec![],
            };
            assert!(forged.validate().is_err(), "{literal}");
            assert_eq!(forged.kind.based_digits(), None);
        }
        assert!(
            NodeKind::based_number("0".repeat(crate::MAX_ATOM_BYTES - 1), NumericBase::Binary)
                .is_ok()
        );
        assert_eq!(
            NodeKind::based_number("0".repeat(crate::MAX_ATOM_BYTES), NumericBase::Binary),
            Err(TreeError::TextTooLong)
        );
    }

    #[test]
    fn input_mode_changes_no_source_caret_revision_or_history_and_survives_undo() {
        let mut editor = Editor::new();
        assert!(!editor.bin_oct_hex_mode());
        editor
            .set_atom(editor.root(), NodeKind::Number("1101".into()))
            .unwrap();
        editor.place_text_selection(editor.root(), 4, 1).unwrap();
        let source = editor.to_portable().unwrap();
        let caret = editor.caret();
        let revision = editor.arena().revision();
        let lengths = (editor.undo.len(), editor.redo.len());
        for enabled in [true, true, false, true] {
            editor.set_bin_oct_hex_mode(enabled);
            assert_eq!(editor.bin_oct_hex_mode(), enabled);
            assert_eq!(editor.to_portable().unwrap(), source);
            assert_eq!(editor.caret(), caret);
            assert_eq!(editor.arena().revision(), revision);
            assert_eq!((editor.undo.len(), editor.redo.len()), lengths);
        }
        let cloned = editor.clone_without_history();
        assert!(cloned.bin_oct_hex_mode());
        assert_eq!(cloned.caret(), caret);
        assert!(cloned.undo.is_empty() && cloned.redo.is_empty());
        assert!(editor.undo());
        assert!(editor.bin_oct_hex_mode());
        assert_eq!(editor.to_portable().unwrap().kind, NodeKind::Placeholder);
        let lengths = (editor.undo.len(), editor.redo.len());
        editor.set_bin_oct_hex_mode(false);
        assert_eq!((editor.undo.len(), editor.redo.len()), lengths);
        assert!(editor.redo());
        assert!(!editor.bin_oct_hex_mode());
        assert_eq!(editor.to_portable().unwrap(), source);
    }
}
