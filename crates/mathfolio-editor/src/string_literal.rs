//! STRING is an explicit literal value, not an identifier or a text region.
//! E: descriptor0bbb/0303..038a; presenter063d disables gestures only at an
//! inner value caret. Writer118d stores text without its displayed quote glyphs.
//! Static ledger: artifacts/rea/string-value-next-contract-20261019.md.
use super::*;
use unicode_general_category::{GeneralCategory, get_general_category};

/// Stored native/portable text has the XML 1.0 character boundary. This is
/// deliberately separate from the narrower original editor authoring regex.
pub(super) fn xml_text_valid(text: &str) -> bool {
    text.chars().all(|ch| {
        matches!(ch, '\t' | '\n' | '\r') || (ch >= ' ' && !matches!(ch, '\u{fffe}' | '\u{ffff}'))
    })
}

fn authoring_text_valid(text: &str) -> bool {
    // Original ^\P{C}*\z. This dependency's Unicode16 category table is an
    // implementation boundary, not proof of .NET's exact Unicode version.
    // Controls0a0a dispatches UTF16 Char values individually. Supplementary
    // input would pass through isolated surrogate characters, excluded by C;
    // raw imported supplementary values remain losslessly representable.
    text.chars().all(|ch| {
        ch.len_utf16() == 1
            && !matches!(
                get_general_category(ch),
                GeneralCategory::Control
                    | GeneralCategory::Format
                    | GeneralCategory::PrivateUse
                    | GeneralCategory::Surrogate
                    | GeneralCategory::Unassigned
            )
    }) && xml_text_valid(text)
}

fn byte_offset(text: &str, offset: usize) -> Option<usize> {
    if offset == 0 {
        return Some(0);
    }
    let mut units = 0;
    for (index, ch) in text.char_indices() {
        units += ch.len_utf16();
        if units == offset {
            return Some(index + ch.len_utf8());
        }
        if units > offset {
            return None;
        }
    }
    None
}

impl Editor {
    /// Native presenter063d tests STRING + inner CaretPosition.HasValue before
    /// disabling all math character gestures. Whole-scope selection is separate.
    pub fn is_inside_string_literal(&self) -> bool {
        self.string_literal_range().is_some()
    }

    fn string_literal_range(&self) -> Option<(&str, ValueRange)> {
        if self.selection != Selection::None
            || self.typeover.is_some()
            || self.caret.caret_side != Direction::Indeterminate
        {
            return None;
        }
        let NodeKind::StringLiteral(text) = &self.arena.get(self.caret.node)?.kind else {
            return None;
        };
        let range = self.caret.value_range.unwrap_or(ValueRange {
            anchor: 0,
            focus: 0,
        });
        byte_offset(text, range.anchor)?;
        byte_offset(text, range.focus)?;
        Some((text, range))
    }

    /// The double-quote gesture creates STRING in a blank slot, or converts a
    /// plain Normal value at its current text position (008d/026b). Structured
    /// names and unknown adjacent/Scope repair remain atomic unsupported paths.
    /// Inside an existing literal, a quote is real text, never a closing command.
    pub fn insert_string_literal(&mut self) -> bool {
        if let Some(changed) = self.insert_literal_text("\"") {
            return changed;
        }
        if self.selection != Selection::None || self.typeover.is_some() {
            return false;
        }
        let Some(node) = self.arena.get(self.caret.node) else {
            return false;
        };
        let (text, offset) = match &node.kind {
            NodeKind::Placeholder => (String::new(), 0),
            NodeKind::Identifier(text)
            | NodeKind::Number(text)
            | NodeKind::AnnotatedIdentifier { name: text, .. }
            | NodeKind::NativeIdentifier {
                name: text,
                subscript: None,
                ..
            } if self.caret.caret_side == Direction::Indeterminate => {
                let range = self.caret.value_range.unwrap_or(ValueRange {
                    anchor: 0,
                    focus: 0,
                });
                if range.anchor != range.focus || byte_offset(text, range.focus).is_none() {
                    return false;
                }
                (text.clone(), range.focus)
            }
            _ => return false,
        };
        if text.len() > MAX_ATOM_BYTES || !authoring_text_valid(&text) {
            return false;
        }
        let id = self.caret.node;
        self.checkpoint();
        self.arena.node_mut(id).kind = NodeKind::StringLiteral(text);
        self.caret = Caret::at(id, Direction::Indeterminate);
        self.caret.value_range = Some(ValueRange {
            anchor: offset,
            focus: offset,
        });
        self.selection = Selection::None;
        self.scope = None;
        true
    }

    /// Some means the inner literal owns this text, including invalid/no-op
    /// input. A caller must not fall through to its normal operator parser.
    /// Value replacement and caret use native UTF16 offsets (05e4); source text
    /// remains literal even when it looks numeric or contains operator glyphs.
    pub fn insert_literal_text(&mut self, inserted: &str) -> Option<bool> {
        let (text, range) = self.string_literal_range()?;
        if !authoring_text_valid(inserted) {
            return Some(false);
        }
        let start = range.anchor.min(range.focus);
        let end = range.anchor.max(range.focus);
        let a = byte_offset(text, start)?;
        let b = byte_offset(text, end)?;
        let size = a.checked_add(inserted.len())?.checked_add(text.len() - b)?;
        if size > MAX_ATOM_BYTES {
            return Some(false);
        }
        let mut next = String::with_capacity(size);
        next.push_str(&text[..a]);
        next.push_str(inserted);
        next.push_str(&text[b..]);
        if next == text {
            return Some(false);
        }
        let position = start + inserted.encode_utf16().count();
        self.install_literal_text(next, position);
        Some(true)
    }

    /// Native0435 removes the selected range or one UTF16 unit and retains the
    /// STRING descriptor, including an empty value (preferred factory02e4).
    /// Rust's scalar boundaries cannot represent deleting half a surrogate pair;
    /// for imported supplementary text one whole scalar is removed instead.
    /// Unproved outward quote-deletion is left unchanged, not forced to PH.
    pub fn delete_literal_text(&mut self, backwards: bool) -> Option<bool> {
        let (text, range) = self.string_literal_range()?;
        let mut start = range.anchor.min(range.focus);
        let mut end = range.anchor.max(range.focus);
        if start == end {
            let position = byte_offset(text, start)?;
            if backwards {
                let Some(ch) = text[..position].chars().next_back() else {
                    return Some(false);
                };
                start -= ch.len_utf16();
            } else {
                let Some(ch) = text[position..].chars().next() else {
                    return Some(false);
                };
                end += ch.len_utf16();
            }
        }
        let a = byte_offset(text, start)?;
        let b = byte_offset(text, end)?;
        let mut next = String::with_capacity(text.len() - (b - a));
        next.push_str(&text[..a]);
        next.push_str(&text[b..]);
        self.install_literal_text(next, start);
        Some(true)
    }

    /// Literal value movement keeps its distinct outer edges separate from
    /// internal offsets. Native STRING explicitly enables both edges; Rust
    /// moves across complete scalar boundaries rather than lone surrogates.
    /// None leaves ordinary parent/neighbor navigation to the host.
    pub fn move_literal_caret(&mut self, direction: Direction, selecting: bool) -> Option<bool> {
        if self.selection != Selection::None || self.typeover.is_some() {
            return None;
        }
        let NodeKind::StringLiteral(text) = &self.arena.get(self.caret.node)?.kind else {
            return None;
        };
        if !matches!(direction, Direction::Left | Direction::Right) {
            return None;
        }
        let length = text.encode_utf16().count();
        let old = self.caret;
        let mut range = old.value_range.unwrap_or(ValueRange {
            anchor: 0,
            focus: 0,
        });
        match old.caret_side {
            Direction::Left if direction == Direction::Right => {
                range = ValueRange {
                    anchor: 0,
                    focus: 0,
                };
            }
            Direction::Right if direction == Direction::Left => {
                range = ValueRange {
                    anchor: length,
                    focus: length,
                };
            }
            Direction::Indeterminate => {
                let position = byte_offset(text, range.focus)?;
                let next = if !selecting && range.anchor != range.focus {
                    Some(if direction == Direction::Left {
                        range.anchor.min(range.focus)
                    } else {
                        range.anchor.max(range.focus)
                    })
                } else if direction == Direction::Left {
                    text[..position]
                        .chars()
                        .next_back()
                        .map(|ch| range.focus - ch.len_utf16())
                } else {
                    text[position..]
                        .chars()
                        .next()
                        .map(|ch| range.focus + ch.len_utf16())
                };
                if let Some(next) = next {
                    range.focus = next;
                    if !selecting {
                        range.anchor = next;
                    }
                } else if selecting {
                    return Some(false);
                } else {
                    self.caret = Caret::at(old.node, direction);
                    self.scope = None;
                    return Some(self.caret != old);
                }
            }
            _ => return None,
        }
        self.caret = Caret::at(old.node, Direction::Indeterminate);
        self.caret.value_range = Some(range);
        self.scope = None;
        Some(self.caret != old)
    }

    fn install_literal_text(&mut self, text: String, offset: usize) {
        self.checkpoint();
        let id = self.caret.node;
        self.arena.node_mut(id).kind = NodeKind::StringLiteral(text);
        self.caret = Caret::at(id, Direction::Indeterminate);
        self.caret.value_range = Some(ValueRange {
            anchor: offset,
            focus: offset,
        });
        self.selection = Selection::None;
        self.scope = None;
    }
}

#[cfg(test)]
#[path = "string_literal_tests.rs"]
mod tests;
