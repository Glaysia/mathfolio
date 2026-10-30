//! Bounded NUMERIC candidate fusion from E0493/0494 and the Bin/Oct/Hex input
//! option. Existing typed based source is distinct from discovering new suffix
//! input. This is not the complete native value factory/partial parser.
use mathfolio_editor::{
    BinaryOp, Direction, Editor, InsertionMode, NodeId, NodeKind, NumericBase, PortableTree,
    Selection,
};

fn suffix(ch: char) -> Option<NumericBase> {
    match ch {
        'b' => Some(NumericBase::Binary),
        'o' => Some(NumericBase::Octal),
        'h' => Some(NumericBase::Hexadecimal),
        _ => None,
    }
}

fn normal_value_context(editor: &Editor, target: NodeId) -> bool {
    if editor.caret().node != target
        || editor.caret().insertion != InsertionMode::Normal
        || editor.selection() != Selection::None
        || editor.typeover_target().is_some()
    {
        return false;
    }
    // APPLY callee parent repair is outside this adapter. This includes a
    // value nested in an Index used as a callee, rather than only direct ids.
    let mut current = target;
    while let Some(parent) = editor.arena().get(current).and_then(|n| n.parent()) {
        let Some(node) = editor.arena().get(parent) else {
            return false;
        };
        if matches!(node.kind(), NodeKind::Call { .. }) && node.children().first() == Some(&current)
        {
            return false;
        }
        current = parent;
    }
    true
}

fn at_end(editor: &Editor, text: &str) -> bool {
    let caret = editor.caret();
    if let Some(range) = caret.value_range {
        range.anchor == range.focus && range.focus == text.encode_utf16().count()
    } else {
        caret.caret_side == Direction::Right
    }
}

/// Called before the ordinary character/implicit Scale route. None delegates
/// to that route; Some(false) means a recognized candidate hit a source budget.
/// No fallback mutation occurs here for a rejected candidate or unknown mode.
pub(super) fn input_suffix(editor: &mut Editor, ch: char) -> Option<bool> {
    let target = editor.caret().node;
    if !normal_value_context(editor, target) {
        return None;
    }
    let node = editor.arena().get(target)?;
    // E0491/0492: at a NUMERIC right edge the next value is inserted
    // adjacent to it. A decimal prefix is retained and the old suffix moves
    // to the new value (2b + 3 => 2, b3). Limit the parent-binding adapter to
    // the captured isolated value/Evaluate operand, not arbitrary containers.
    if let NodeKind::BasedNumber { literal, base } = node.kind()
        && at_end(editor, literal)
        && ch.is_ascii_hexdigit()
        && let Some(digits) = base.digits(literal)
        && digits.bytes().all(|b| b.is_ascii_digit())
        && (target == editor.root()
            || node.parent().is_some_and(|parent| {
                parent == editor.root()
                    && editor.arena().get(parent).is_some_and(|p| {
                        p.kind() == &NodeKind::Binary(BinaryOp::Evaluate)
                            && p.children().first() == Some(&target)
                    })
            }))
    {
        let decimal = NodeKind::Number(digits.into());
        let tail = format!("{}{ch}", base.suffix());
        let length = tail.encode_utf16().count();
        let replacement = PortableTree {
            kind: NodeKind::Binary(BinaryOp::Scale),
            children: vec![
                PortableTree {
                    kind: decimal.clone(),
                    children: vec![],
                },
                PortableTree {
                    kind: NodeKind::Identifier(tail.clone()),
                    children: vec![],
                },
            ],
        };
        let candidate = if target == editor.root() {
            replacement
        } else {
            let Ok(mut source) = editor.to_portable() else {
                return Some(false);
            };
            source.children[0] = replacement;
            source
        };
        // The Evaluate result-side subtree can already occupy the whole node
        // budget. Reject the complete candidate before touching Undo or caret.
        if candidate.validate().is_err() {
            return Some(false);
        }
        editor.edit_group(|editor| {
            editor
                .set_atom(target, decimal)
                .expect("captured numeric atom");
            editor.insert_binary(BinaryOp::Scale);
            let tail_id = editor.caret().node;
            editor
                .set_atom(tail_id, NodeKind::Identifier(tail))
                .expect("new adjacent atom");
            editor
                .place_text_caret(tail_id, length)
                .expect("new tail offset");
        });
        return Some(true);
    }
    if !editor.bin_oct_hex_mode() {
        return None;
    }
    let base = suffix(ch)?;
    match node.kind() {
        NodeKind::Number(digits) if at_end(editor, digits) => {
            if !base.accepts_lexeme(digits) {
                return None;
            }
            let Ok(kind) = NodeKind::based_number(digits.clone(), base) else {
                return Some(false);
            };
            let length = digits.encode_utf16().count() + 1;
            if editor.set_atom(target, kind).is_err() {
                return Some(false);
            }
            let _ = editor.place_text_caret(target, length);
            Some(true)
        }
        // An unfinished hex lexeme such as 0a32 is currently represented as
        // Scale(Number("0"), Identifier("a32")). Only that direct, plain pair
        // has a captured positive. No labels or literal suffixes are discarded.
        NodeKind::Identifier(tail) if base == NumericBase::Hexadecimal && at_end(editor, tail) => {
            if !tail
                .as_bytes()
                .first()
                .is_some_and(|b| matches!(b, b'a'..=b'f' | b'A'..=b'F'))
            {
                return None;
            }
            let parent = node.parent()?;
            let scale = editor.arena().get(parent)?;
            if scale.kind() != &NodeKind::Binary(BinaryOp::Scale)
                || scale.children().len() != 2
                || scale.children()[1] != target
            {
                return None;
            }
            let NodeKind::Number(prefix) = editor.arena().get(scale.children()[0])?.kind() else {
                return None;
            };
            let bytes = prefix.len().checked_add(tail.len())?;
            if bytes.saturating_add(1) > mathfolio_editor::MAX_ATOM_BYTES {
                return Some(false);
            }
            let mut digits = String::with_capacity(bytes);
            digits.push_str(prefix);
            digits.push_str(tail);
            if !base.accepts_lexeme(&digits) {
                return None;
            }
            let kind = NodeKind::based_number(digits, base).ok()?;
            let length = kind.based_digits()?.encode_utf16().count() + 1;
            let replacement = PortableTree {
                kind: kind.clone(),
                children: vec![],
            };
            // Replacing a two-leaf Scale by one validated leaf cannot increase
            // node/depth budgets. The first real mutation checkpoints the old
            // caret before temporary subtree selection, keeping Undo exact.
            editor.edit_group(|editor| {
                editor
                    .set_atom(target, kind)
                    .expect("captured plain atom still exists");
                editor
                    .select_subtree(parent)
                    .expect("captured direct Scale still exists");
                editor
                    .replace_selection(&replacement)
                    .expect("validated leaf replaces captured Scale");
                let _ = editor.place_text_caret(editor.caret().node, length);
            });
            Some(true)
        }
        _ => None,
    }
}

/// Classify a proposed edit without mutating source. Existing BasedNumber has
/// an explicit type independently of the current document input mode. New
/// Number discovery remains mode-gated and excludes partial selections.
/// None is an unsupported classification. In particular the caller must not
/// turn a rejected existing BasedNumber edit into a forged generic identifier.
pub(super) fn edited_kind(editor: &Editor, target: NodeId, text: &str) -> Option<NodeKind> {
    if !normal_value_context(editor, target) {
        return None;
    }
    let current = editor.arena().get(target)?.kind();
    let existing = matches!(current, NodeKind::BasedNumber { .. });
    if !existing {
        let NodeKind::Number(old) = current else {
            return None;
        };
        if !editor.bin_oct_hex_mode() || !at_end(editor, old) {
            return None;
        }
    }
    if text.is_empty() {
        return existing.then_some(NodeKind::Placeholder);
    }
    if let Some(base) = text.chars().last().and_then(suffix)
        && let Some(digits) = base.digits(text)
    {
        return NodeKind::based_number(digits, base).ok();
    }
    if existing && text.parse::<f64>().is_ok_and(f64::is_finite) {
        return Some(NodeKind::Number(text.into()));
    }
    // e.g. removing h from 0a32h requires Number/Name reparse. Do not invent
    // a single atom, erase a label, or pretend the complete parser is present.
    None
}

#[cfg(test)]
#[path = "radix_input_tests.rs"]
mod tests;
