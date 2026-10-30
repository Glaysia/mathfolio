//! Installed formatted-value deletion/fusion. Native offsets count the complete
//! UTF-16 value; the editor stores a literal suffix as a separate text field.
//! Static source contract and representation limits: analysis0127.
use super::grouping_insertion::{grouping_fragment, grouping_text_slice};
use super::*;

pub(super) enum DeletionValueFusion {
    Merged {
        kind: NodeKind,
        offset: usize,
        insertion: InsertionMode,
    },
    Separate,
}

pub(super) fn fusion_value(kind: &NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::Number(_)
            | NodeKind::Identifier(_)
            | NodeKind::AnnotatedIdentifier { .. }
            | NodeKind::NativeIdentifier { .. }
            | NodeKind::Imaginary(_)
            | NodeKind::ImaginarySymbol { .. }
    )
}

// None means unknown/unrepresented metadata or formatting, not failed lexical
// fusion. Separate means the installed factory rejects the concatenation.
pub(super) fn deletion_value_fusion(
    left: &NodeKind,
    right: &NodeKind,
) -> Option<DeletionValueFusion> {
    let l = value(left)?;
    let r = value(right)?;
    // Append adds the right baseline after an existing suffix. The current IR
    // cannot save that interleaved format; moving it before the suffix is wrong.
    if l.suffix.as_ref().is_some_and(|s| !s.is_empty())
        || (l.suffix.is_some() && r.suffix.is_none())
    {
        return None;
    }
    let name = format!("{}{}", l.text, r.text);
    let offset = l.text.encode_utf16().count();
    let kind = if let Some(suffix) = r.suffix.filter(|s| !s.is_empty()) {
        if suffix
            .chars()
            .any(|ch| ch.is_whitespace() || ch.is_control())
        {
            return None;
        }
        if !plain_name(&name) {
            return if name.as_bytes().first().is_some_and(u8::is_ascii_digit) {
                Some(DeletionValueFusion::Separate)
            } else {
                None
            };
        }
        native_name(name, Some(suffix), l.label)
    } else {
        let Some(mut kind) = grouping_fragment(&NodeKind::Identifier(String::new()), name.clone())
        else {
            // Decimal/name concatenations beginning with a digit are certainly
            // invalid unless they may be an unimplemented radix spelling.
            return if name.as_bytes().first().is_some_and(u8::is_ascii_digit)
                && !name.ends_with(['b', 'B', 'o', 'O', 'h', 'H'])
            {
                Some(DeletionValueFusion::Separate)
            } else {
                None
            };
        };
        if let NodeKind::Identifier(name) = kind {
            kind = match l.label {
                Some(label) => NodeKind::AnnotatedIdentifier {
                    name,
                    labels: label,
                },
                None => NodeKind::Identifier(name),
            };
        }
        kind
    };
    Some(DeletionValueFusion::Merged {
        kind,
        offset,
        insertion: InsertionMode::Normal,
    })
}

struct Value {
    text: String,
    suffix: Option<String>,
    label: Option<String>,
}

fn value(kind: &NodeKind) -> Option<Value> {
    let (text, suffix, label) = match kind {
        NodeKind::Identifier(name) => (name.clone(), None, None),
        NodeKind::AnnotatedIdentifier { name, labels } => {
            (name.clone(), None, explicit_label(labels)?)
        }
        NodeKind::NativeIdentifier {
            name,
            labels,
            contextual,
            subscript,
        } => {
            let label = if *contextual == Some(true) {
                None
            } else {
                explicit_label(labels)?
            };
            (name.clone(), subscript.clone(), label)
        }
        NodeKind::Number(text) => {
            if !matches!(
                grouping_fragment(kind, text.clone()),
                Some(NodeKind::Number(_))
            ) {
                return None;
            }
            (text.clone(), None, None)
        }
        NodeKind::Imaginary(text) => (format!("{text}i"), None, None),
        NodeKind::ImaginarySymbol { value, symbol } if matches!(symbol.as_str(), "i" | "j") => {
            (format!("{value}{symbol}"), None, None)
        }
        _ => return None,
    };
    Some(Value {
        text,
        suffix,
        label,
    })
}

fn explicit_label(labels: &str) -> Option<Option<String>> {
    let label = labels.split(' ').next()?;
    Some(Some(
        match label {
            "VARIABLE" | "UNIT" | "CONSTANT" | "FUNCTION" | "SYSTEM" | "KEYWORD" | "*" => label,
            "NONE" => "*",
            _ => return None,
        }
        .into(),
    ))
}

fn native_name(name: String, suffix: Option<String>, label: Option<String>) -> NodeKind {
    let (labels, contextual) = match label {
        Some(label) => (label, None),
        // Native factory creates nullable IdLabel=null and ResolvedIdLabel=None6.
        None => ("*".into(), Some(true)),
    };
    NodeKind::NativeIdentifier {
        name,
        labels,
        contextual,
        subscript: suffix,
    }
}

fn plain_name(text: &str) -> bool {
    matches!(
        grouping_fragment(&NodeKind::Identifier(String::new()), text.into()),
        Some(NodeKind::Identifier(_))
    )
}

impl Editor {
    /// Handle the native complete-value character stage for literal suffixes
    /// and imaginary literals. Some(false) is an atomic unsupported edit;
    /// None is an outward boundary or a value handled by ordinary text entry.
    pub fn delete_formatted_value(&mut self, backwards: bool) -> Option<bool> {
        if self.selection != Selection::None {
            return None;
        }
        let node = self.caret.node;
        let source = self.arena.get(node)?.kind.clone();
        let supported = matches!(
            source,
            NodeKind::NativeIdentifier { .. }
                | NodeKind::AnnotatedIdentifier { .. }
                | NodeKind::Imaginary(_)
                | NodeKind::ImaginarySymbol { .. }
        );
        if !supported {
            return None;
        }
        let Some(raw) = value(&source) else {
            return Some(false);
        };
        let Some((start, end)) = self.formatted_value_range(&source) else {
            return Some(false);
        };
        let base_len = raw.text.encode_utf16().count();
        let suffix_len = raw.suffix.as_deref().unwrap_or("").encode_utf16().count();
        let total = base_len + suffix_len;
        if start == end && ((backwards && start == 0) || (!backwards && end == total)) {
            return None;
        }
        // 0435 computes the mode before mutation at SelectionStart (or the
        // collapsed caret), and 1329 reads its preceding character. Selecting
        // the entire suffix therefore returns to Normal; deleting its final
        // character from a collapsed suffix-end caret keeps Subscript pending.
        let insertion = if start > base_len {
            InsertionMode::Subscript
        } else {
            InsertionMode::Normal
        };
        let (a, b) = if start != end {
            (start, end)
        } else if backwards {
            (start.checked_sub(1)?, start)
        } else {
            (start, start.checked_add(1)?)
        };
        if b > total {
            return Some(false);
        }
        let mut base = raw.text.clone();
        let mut suffix = raw.suffix.clone();
        if a < base_len {
            let stop = b.min(base_len);
            let (Some(prefix), Some(rest)) = (
                grouping_text_slice(&base, 0, a),
                grouping_text_slice(&base, stop, base_len),
            ) else {
                return Some(false);
            };
            base = prefix + &rest;
        }
        if b > base_len {
            let text = suffix.as_deref().unwrap_or("");
            let (Some(prefix), Some(rest)) = (
                grouping_text_slice(text, 0, a.saturating_sub(base_len)),
                grouping_text_slice(text, b - base_len, suffix_len),
            ) else {
                return Some(false);
            };
            suffix = Some(prefix + &rest);
        }
        let suffix = suffix.filter(|text| !text.is_empty());
        let kind = match source {
            NodeKind::AnnotatedIdentifier { labels, .. } => {
                if base.is_empty() {
                    NodeKind::Placeholder
                } else {
                    match grouping_fragment(&NodeKind::Identifier(String::new()), base.clone()) {
                        Some(NodeKind::Identifier(_)) => {
                            NodeKind::AnnotatedIdentifier { name: base, labels }
                        }
                        Some(kind) => kind,
                        None => return Some(false),
                    }
                }
            }
            NodeKind::NativeIdentifier {
                labels, contextual, ..
            } => {
                if base.is_empty() {
                    if suffix.is_some() {
                        return Some(false);
                    }
                    NodeKind::Placeholder
                } else if suffix.is_some() {
                    NodeKind::NativeIdentifier {
                        name: base,
                        labels,
                        contextual,
                        subscript: suffix,
                    }
                } else {
                    match grouping_fragment(&NodeKind::Identifier(String::new()), base.clone()) {
                        Some(NodeKind::Identifier(_)) => NodeKind::NativeIdentifier {
                            name: base,
                            labels,
                            contextual,
                            subscript: None,
                        },
                        Some(kind) => kind,
                        None => return Some(false),
                    }
                }
            }
            NodeKind::Imaginary(_) | NodeKind::ImaginarySymbol { .. } => {
                let Some(kind) = grouping_fragment(&NodeKind::Identifier(String::new()), base)
                else {
                    return Some(false);
                };
                kind
            }
            _ => return None,
        };
        let mut candidate = self.clone_without_history();
        candidate.arena.node_mut(node).kind = kind;
        let Some(caret) = candidate.deletion_formatted_caret(node, a, insertion) else {
            return Some(false);
        };
        candidate.caret = caret;
        candidate.selection = Selection::None;
        candidate.scope = None;
        if candidate.to_portable().is_err() {
            return Some(false);
        }
        self.checkpoint();
        self.arena = candidate.arena;
        self.root = candidate.root;
        self.caret = candidate.caret;
        self.selection = Selection::None;
        self.scope = None;
        Some(true)
    }

    pub(super) fn formatted_value_range(&self, kind: &NodeKind) -> Option<(usize, usize)> {
        let raw = value(kind)?;
        let base = raw.text.encode_utf16().count();
        let total = base + raw.suffix.as_deref().unwrap_or("").encode_utf16().count();
        if self.caret.caret_side == Direction::Left {
            return Some((0, 0));
        }
        if self.caret.caret_side == Direction::Right {
            return Some((total, total));
        }
        let range = self.caret.value_range?;
        let offset = if self.caret.insertion == InsertionMode::Subscript {
            if !matches!(kind, NodeKind::NativeIdentifier { .. }) {
                return None;
            }
            base
        } else {
            0
        };
        let a = offset.checked_add(range.anchor.min(range.focus))?;
        let b = offset.checked_add(range.anchor.max(range.focus))?;
        (b <= total).then_some((a, b))
    }

    pub(super) fn deletion_formatted_boundary(
        &self,
        kind: &NodeKind,
        backwards: bool,
    ) -> Option<bool> {
        if !fusion_value(kind) {
            return None;
        }
        let raw = value(kind)?;
        let total = raw.text.encode_utf16().count()
            + raw.suffix.as_deref().unwrap_or("").encode_utf16().count();
        let (a, b) = self.formatted_value_range(kind)?;
        Some(a == b && a == if backwards { 0 } else { total })
    }

    pub(super) fn deletion_formatted_caret(
        &self,
        node: NodeId,
        offset: usize,
        insertion: InsertionMode,
    ) -> Option<Caret> {
        let kind = &self.arena.get(node)?.kind;
        let mut caret = Caret::at(node, Direction::Indeterminate);
        if *kind == NodeKind::Placeholder {
            return Some(Caret::at(node, Direction::Left));
        }
        match kind {
            NodeKind::NativeIdentifier {
                name, subscript, ..
            } => {
                let base = name.encode_utf16().count();
                let total = base + subscript.as_deref().unwrap_or("").encode_utf16().count();
                if offset > total {
                    return None;
                }
                if insertion == InsertionMode::Subscript && offset >= base {
                    caret.insertion = InsertionMode::Subscript;
                    let local = offset - base;
                    caret.value_range = Some(ValueRange {
                        anchor: local,
                        focus: local,
                    });
                } else if offset <= base {
                    caret.value_range = Some(ValueRange {
                        anchor: offset,
                        focus: offset,
                    });
                } else {
                    return None;
                }
            }
            NodeKind::Imaginary(value) | NodeKind::ImaginarySymbol { value, .. } => {
                let length = value.encode_utf16().count();
                if offset == length + 1 {
                    return Some(Caret::at(node, Direction::Right));
                }
                if offset > length {
                    return None;
                }
                caret.value_range = Some(ValueRange {
                    anchor: offset,
                    focus: offset,
                });
            }
            _ => {
                let text = kind.text()?;
                grouping_text_slice(text, 0, offset)?;
                caret.value_range = Some(ValueRange {
                    anchor: offset,
                    focus: offset,
                });
            }
        }
        Some(caret)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tree(kind: NodeKind, children: Vec<PortableTree>) -> PortableTree {
        PortableTree { kind, children }
    }
    fn native(name: &str, label: &str, contextual: Option<bool>, suffix: Option<&str>) -> NodeKind {
        NodeKind::NativeIdentifier {
            name: name.into(),
            labels: label.into(),
            contextual,
            subscript: suffix.map(str::to_owned),
        }
    }
    fn merged(left: NodeKind, right: NodeKind) -> (NodeKind, usize, InsertionMode) {
        match deletion_value_fusion(&left, &right).expect("supported native fusion") {
            DeletionValueFusion::Merged {
                kind,
                offset,
                insertion,
            } => (kind, offset, insertion),
            DeletionValueFusion::Separate => panic!("expected factory success"),
        }
    }
    #[test]
    fn native_nullable_label_transfer_uses_left_choice_and_not_cached_resolution() {
        let right = native("y", "FUNCTION", Some(false), None);
        for (label, contextual, expected) in [
            ("UNIT", Some(false), Some("UNIT")),
            ("UNIT", None, Some("UNIT")),
            ("UNIT", Some(true), None),
            ("UNKNOWN-CACHE", Some(true), None),
            ("*", None, Some("*")),
            ("NONE", Some(false), Some("*")),
        ] {
            let (kind, offset, insertion) =
                merged(native("x", label, contextual, None), right.clone());
            assert_eq!(
                kind,
                match expected {
                    Some(label) => NodeKind::AnnotatedIdentifier {
                        name: "xy".into(),
                        labels: label.into()
                    },
                    None => NodeKind::Identifier("xy".into()),
                }
            );
            assert_eq!(offset, 1);
            assert_eq!(insertion, InsertionMode::Normal);
        }
        assert!(deletion_value_fusion(&native("x", "UNKNOWN", None, None), &right).is_none());
    }
    #[test]
    fn formatted_append_retains_right_suffix_and_rejects_unrepresentable_left_interleaving() {
        let (kind, offset, insertion) = merged(
            native("𝑥", "UNIT", Some(true), None),
            native("y", "VARIABLE", None, Some("dc")),
        );
        assert_eq!(kind, native("𝑥y", "*", Some(true), Some("dc")));
        assert_eq!(offset, 2);
        assert_eq!(insertion, InsertionMode::Normal);
        let (kind, _, _) = merged(
            native("x", "UNIT", None, None),
            native("y", "FUNCTION", None, Some("dc")),
        );
        assert_eq!(kind, native("xy", "UNIT", None, Some("dc")));
        for right in [
            NodeKind::Number("2".into()),
            NodeKind::Identifier("y".into()),
            native("y", "VARIABLE", None, Some("dc")),
        ] {
            assert!(
                deletion_value_fusion(&native("x", "UNIT", None, Some("rms")), &right).is_none()
            );
        }
    }
    #[test]
    fn imaginary_values_fuse_the_native_symbol_lexeme_instead_of_coefficient_only() {
        let (kind, offset, _) = merged(
            NodeKind::Number("1".into()),
            NodeKind::ImaginarySymbol {
                value: "2".into(),
                symbol: "j".into(),
            },
        );
        assert_eq!(
            kind,
            NodeKind::ImaginarySymbol {
                value: "12".into(),
                symbol: "j".into()
            }
        );
        assert_eq!(offset, 1);
        assert!(matches!(
            deletion_value_fusion(
                &NodeKind::Imaginary("2".into()),
                &NodeKind::Number("3".into())
            ),
            Some(DeletionValueFusion::Separate)
        ));
        let (kind, offset, _) = merged(
            NodeKind::Identifier("x".into()),
            NodeKind::Imaginary("2".into()),
        );
        assert_eq!(kind, NodeKind::Identifier("x2i".into()));
        assert_eq!(offset, 1);
    }
    #[test]
    fn literal_suffix_start_backspace_and_base_end_delete_use_full_native_offset() {
        let source = tree(native("AB", "UNIT", Some(true), Some("dc")), vec![]);
        for backwards in [true, false] {
            let mut e = Editor::from_portable(&source).unwrap();
            let node = e.root();
            if backwards {
                e.place_subscript_selection(node, 0, 0).unwrap();
            } else {
                e.place_text_caret(node, 2).unwrap();
            }
            let before = e.caret();
            assert_eq!(e.delete_formatted_value(backwards), Some(true));
            assert_eq!(
                e.to_portable().unwrap().kind,
                if backwards {
                    native("A", "UNIT", Some(true), Some("dc"))
                } else {
                    native("AB", "UNIT", Some(true), Some("c"))
                }
            );
            assert_eq!(e.caret().insertion, InsertionMode::Normal);
            let position = if backwards { 1 } else { 2 };
            assert_eq!(
                e.caret().value_range,
                Some(ValueRange {
                    anchor: position,
                    focus: position
                })
            );
            let after = e.caret();
            assert!(e.undo());
            assert_eq!(e.to_portable().unwrap(), source);
            assert_eq!(e.caret(), before);
            assert!(e.redo());
            assert_eq!(e.caret(), after);
            assert!(e.validate().is_ok());
        }
    }
    #[test]
    fn explicit_name_character_deletion_retains_labels_and_reclassifies_empty_and_numeric_values() {
        for (name, position, backwards, expected) in [
            (
                "xy",
                2,
                true,
                NodeKind::AnnotatedIdentifier {
                    name: "x".into(),
                    labels: "UNIT".into(),
                },
            ),
            ("x", 1, true, NodeKind::Placeholder),
            ("x12", 0, false, NodeKind::Number("12".into())),
        ] {
            let source = tree(
                NodeKind::AnnotatedIdentifier {
                    name: name.into(),
                    labels: "UNIT".into(),
                },
                vec![],
            );
            let mut e = Editor::from_portable(&source).unwrap();
            e.place_text_caret(e.root(), position).unwrap();
            let before = e.caret();
            assert_eq!(e.delete_formatted_value(backwards), Some(true));
            assert_eq!(e.to_portable().unwrap().kind, expected);
            assert!(e.validate().is_ok());
            assert!(e.undo());
            assert_eq!(e.to_portable().unwrap(), source);
            assert_eq!(e.caret(), before);
            assert!(!e.undo());
        }
    }
    #[test]
    fn selected_entire_suffix_uses_baseline_insertion_and_restores_selection_on_undo() {
        for backwards in [true, false] {
            for (anchor, focus) in [(0, 3), (3, 0)] {
                let source = tree(native("V", "VARIABLE", None, Some("a𝛼")), vec![]);
                let mut e = Editor::from_portable(&source).unwrap();
                let node = e.root();
                e.place_subscript_selection(node, anchor, focus).unwrap();
                let selected = e.caret();
                assert_eq!(e.delete_formatted_value(backwards), Some(true));
                let after = e.caret();
                assert_eq!(
                    e.to_portable().unwrap().kind,
                    native("V", "VARIABLE", None, None)
                );
                assert_eq!(after.insertion, InsertionMode::Normal);
                assert_eq!(
                    after.value_range,
                    Some(ValueRange {
                        anchor: 1,
                        focus: 1
                    })
                );
                assert_eq!(e.editing_text(node), Some("V"));
                assert!(e.undo());
                assert_eq!(e.to_portable().unwrap(), source);
                assert_eq!(e.caret(), selected);
                assert!(!e.undo());
                assert!(e.redo());
                assert_eq!(e.caret(), after);
                assert_eq!(e.delete_formatted_value(true), Some(true));
                assert_eq!(e.to_portable().unwrap().kind, NodeKind::Placeholder);
                assert_eq!(e.caret(), Caret::at(node, Direction::Left));
                assert!(e.undo());
                assert_eq!(
                    e.to_portable().unwrap().kind,
                    native("V", "VARIABLE", None, None)
                );
                assert_eq!(e.caret(), after);
                assert!(e.undo());
                assert_eq!(e.to_portable().unwrap(), source);
                assert_eq!(e.caret(), selected);
                assert_eq!(e.validate(), Ok(()));
            }
        }
    }
    #[test]
    fn last_suffix_backspace_keeps_pending_insertion_without_serializing_empty_span() {
        let source = tree(native("AB", "VARIABLE", None, Some("c")), vec![]);
        let mut e = Editor::from_portable(&source).unwrap();
        let node = e.root();
        e.place_subscript_selection(node, 1, 1).unwrap();
        assert_eq!(e.delete_formatted_value(true), Some(true));
        assert_eq!(
            e.to_portable().unwrap().kind,
            native("AB", "VARIABLE", None, None)
        );
        assert_eq!(e.caret().insertion, InsertionMode::Subscript);
        assert_eq!(
            e.caret().value_range,
            Some(ValueRange {
                anchor: 0,
                focus: 0
            })
        );
        assert_eq!(e.editing_text(node), Some(""));
        e.place_subscript_selection(node, 0, 0).unwrap();
        assert!(e.undo());
        assert_eq!(e.to_portable().unwrap(), source);
    }
    #[test]
    fn imaginary_symbol_is_deleted_at_native_value_end_and_reclassified() {
        for backwards in [true, false] {
            let source = tree(
                NodeKind::ImaginarySymbol {
                    value: "12".into(),
                    symbol: "j".into(),
                },
                vec![],
            );
            let mut e = Editor::from_portable(&source).unwrap();
            let node = e.root();
            if backwards {
                e.place_caret(node, Direction::Right).unwrap();
            } else {
                e.place_text_caret(node, 2).unwrap();
            }
            assert_eq!(e.delete_formatted_value(backwards), Some(true));
            assert_eq!(e.to_portable().unwrap().kind, NodeKind::Number("12".into()));
            assert_eq!(
                e.caret().value_range,
                Some(ValueRange {
                    anchor: 2,
                    focus: 2
                })
            );
            assert!(e.undo());
            assert_eq!(e.to_portable().unwrap(), source);
        }
        let mut e = Editor::from_portable(&tree(NodeKind::Imaginary("2".into()), vec![])).unwrap();
        e.place_text_caret(e.root(), 0).unwrap();
        assert_eq!(e.delete_formatted_value(false), Some(true));
        assert_eq!(
            e.to_portable().unwrap().kind,
            NodeKind::Identifier("i".into())
        );
    }
    #[test]
    fn unsupported_suffix_shape_and_surrogate_unit_removal_preserve_revision_and_redo() {
        for (name, suffix, offset) in [("I", "dc", 0), ("AB", "𝛼", 2)] {
            let source = tree(native(name, "VARIABLE", None, Some(suffix)), vec![]);
            let mut e = Editor::from_portable(&source).unwrap();
            let node = e.root();
            e.set_atom(node, native("Changed", "VARIABLE", None, None))
                .unwrap();
            assert!(e.undo());
            e.place_subscript_selection(node, offset, offset).unwrap();
            let caret = e.caret();
            let revision = e.arena().revision();
            assert_eq!(e.delete_formatted_value(true), Some(false));
            assert_eq!(e.to_portable().unwrap(), source);
            assert_eq!(e.caret(), caret);
            assert_eq!(e.arena().revision(), revision);
            assert!(e.redo());
        }
    }
    #[test]
    fn actual_native_name_operator_deletion_is_one_undo_and_restores_all_raw_metadata() {
        let source = tree(
            NodeKind::Binary(BinaryOp::Add),
            vec![
                tree(native("𝑥", "UNIT", Some(true), None), vec![]),
                tree(native("y", "FUNCTION", Some(false), None), vec![]),
            ],
        );
        for backwards in [false, true] {
            let mut e = Editor::from_portable(&source).unwrap();
            let root = e.root();
            let children = e.arena().get(root).unwrap().children().to_vec();
            let child = children[usize::from(backwards)];
            e.place_text_caret(child, if backwards { 0 } else { 2 })
                .unwrap();
            let before = e.caret();
            assert!(e.delete_structure(backwards));
            assert_eq!(
                e.to_portable().unwrap().kind,
                NodeKind::Identifier("𝑥y".into())
            );
            assert_eq!(
                e.caret().value_range,
                Some(ValueRange {
                    anchor: 2,
                    focus: 2
                })
            );
            let after = e.caret();
            assert!(e.undo());
            assert_eq!(e.to_portable().unwrap(), source);
            assert_eq!(e.caret(), before);
            assert!(!e.undo());
            assert!(e.redo());
            assert_eq!(e.caret(), after);
            assert!(e.validate().is_ok());
        }
    }
    #[test]
    fn actual_right_suffix_fusion_and_literal_end_structural_deletion_preserve_utf16_mode() {
        let source = tree(
            NodeKind::Binary(BinaryOp::Add),
            vec![
                tree(native("𝑥", "UNIT", Some(true), None), vec![]),
                tree(native("y", "VARIABLE", None, Some("dc")), vec![]),
            ],
        );
        let mut e = Editor::from_portable(&source).unwrap();
        let right = e.arena().get(e.root()).unwrap().children()[1];
        e.place_text_caret(right, 0).unwrap();
        assert!(e.delete_structure(true));
        assert_eq!(
            e.to_portable().unwrap().kind,
            native("𝑥y", "*", Some(true), Some("dc"))
        );
        assert_eq!(e.caret().insertion, InsertionMode::Normal);
        assert_eq!(
            e.caret().value_range,
            Some(ValueRange {
                anchor: 2,
                focus: 2
            })
        );
        assert!(e.undo());
        assert_eq!(e.to_portable().unwrap(), source);

        let atom = native("I", "VARIABLE", Some(false), Some("dc"));
        let source = tree(
            NodeKind::Binary(BinaryOp::Add),
            vec![
                tree(atom.clone(), vec![]),
                tree(NodeKind::Placeholder, vec![]),
            ],
        );
        let mut e = Editor::from_portable(&source).unwrap();
        let left = e.arena().get(e.root()).unwrap().children()[0];
        e.place_subscript_selection(left, 2, 2).unwrap();
        let before = e.caret();
        assert_eq!(e.delete_formatted_value(false), None);
        assert!(e.delete_structure(false));
        assert_eq!(e.to_portable().unwrap().kind, atom);
        assert_eq!(e.root(), left);
        assert_eq!(e.caret().caret_side, Direction::Right);
        assert_eq!(e.caret().insertion, InsertionMode::Normal);
        assert!(e.undo());
        assert_eq!(e.to_portable().unwrap(), source);
        assert_eq!(e.caret(), before);
    }
    #[test]
    fn actual_imaginary_fusion_and_invalid_concat_retain_symbol_and_directional_entry() {
        for backwards in [false, true] {
            let source = tree(
                NodeKind::Binary(BinaryOp::Add),
                vec![
                    tree(NodeKind::Number("1".into()), vec![]),
                    tree(
                        NodeKind::ImaginarySymbol {
                            value: "2".into(),
                            symbol: "j".into(),
                        },
                        vec![],
                    ),
                ],
            );
            let mut e = Editor::from_portable(&source).unwrap();
            e.place_operator_caret(e.root(), Direction::Indeterminate)
                .unwrap();
            assert!(e.delete_structure(backwards));
            assert_eq!(
                e.to_portable().unwrap().kind,
                NodeKind::ImaginarySymbol {
                    value: "12".into(),
                    symbol: "j".into()
                }
            );
            assert_eq!(
                e.caret().value_range,
                Some(ValueRange {
                    anchor: 1,
                    focus: 1
                })
            );
            assert!(e.undo());
            assert_eq!(e.to_portable().unwrap(), source);
        }
        let source = tree(
            NodeKind::Binary(BinaryOp::Add),
            vec![
                tree(NodeKind::Imaginary("2".into()), vec![]),
                tree(NodeKind::Number("3".into()), vec![]),
            ],
        );
        let mut e = Editor::from_portable(&source).unwrap();
        let left = e.arena().get(e.root()).unwrap().children()[0];
        e.place_operator_caret(e.root(), Direction::Indeterminate)
            .unwrap();
        assert!(e.delete_structure(false));
        assert_eq!(
            e.to_portable().unwrap().kind,
            NodeKind::Binary(BinaryOp::Scale)
        );
        assert_eq!(e.caret().node, left);
        assert_eq!(e.caret().caret_side, Direction::Right);
    }
    #[test]
    fn unknown_metadata_or_oversized_fusion_never_mutates_history_or_partial_tree() {
        for (left, right) in [
            (
                native("x", "UNSUPPORTED", None, None),
                NodeKind::Identifier("y".into()),
            ),
            (
                NodeKind::Identifier("x".repeat(MAX_ATOM_BYTES / 2 + 1)),
                NodeKind::Identifier("y".repeat(MAX_ATOM_BYTES / 2 + 1)),
            ),
        ] {
            let source = tree(
                NodeKind::Binary(BinaryOp::Add),
                vec![tree(left, vec![]), tree(right, vec![])],
            );
            let mut e = Editor::from_portable(&source).unwrap();
            let child = e.arena().get(e.root()).unwrap().children()[1];
            e.set_atom(child, NodeKind::Identifier("other".into()))
                .unwrap();
            assert!(e.undo());
            e.place_operator_caret(e.root(), Direction::Indeterminate)
                .unwrap();
            let before = e.caret();
            let revision = e.arena().revision();
            assert!(!e.delete_structure(true));
            assert_eq!(e.to_portable().unwrap(), source);
            assert_eq!(e.caret(), before);
            assert_eq!(e.arena().revision(), revision);
            assert!(e.redo());
        }
    }
    #[test]
    fn pending_suffix_with_last_base_character_deletes_to_placeholder_left_as_one_undo() {
        let source = tree(native("I", "VARIABLE", None, None), vec![]);
        let mut e = Editor::from_portable(&source).unwrap();
        let node = e.root();
        e.place_subscript_selection(node, 0, 0).unwrap();
        let before = e.caret();
        assert_eq!(e.delete_formatted_value(true), Some(true));
        assert_eq!(e.to_portable().unwrap().kind, NodeKind::Placeholder);
        assert_eq!(e.caret(), Caret::at(node, Direction::Left));
        assert!(e.undo());
        assert_eq!(e.to_portable().unwrap(), source);
        assert_eq!(e.caret(), before);
        assert!(e.redo());
        assert_eq!(e.caret(), Caret::at(node, Direction::Left));
        assert!(e.validate().is_ok());
    }
}
