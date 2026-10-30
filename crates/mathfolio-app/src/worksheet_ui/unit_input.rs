//! Units-menu insertion preserves the UNIT identity and implicit SCALE source.
//! Candidate validation keeps a rejected command out of the real Undo journal.
use mathfolio_editor::{
    BinaryOp, Direction, Editor, InsertionMode, NodeKind, PortableTree, Selection, TreeError,
};

fn apply(editor: &mut Editor, unit: &PortableTree) -> Result<(), TreeError> {
    if editor.typeover_target().is_some() {
        return editor
            .replace_typeover_tree(unit, Direction::Right)
            .then_some(())
            .ok_or(TreeError::InvalidTree("unsupported unit Typeover repair"));
    }
    if editor
        .caret()
        .value_range
        .is_some_and(|range| range.anchor != range.focus)
    {
        return editor
            .set_atom(editor.caret().node, unit.kind.clone())
            .map_err(|_| TreeError::InvalidTree("unit selection is not an atom"));
    }
    if editor.selection() == Selection::None
        && editor.arena().get(editor.caret().node).unwrap().kind() != &NodeKind::Placeholder
    {
        editor.insert_infix(BinaryOp::Scale);
    }
    editor.replace_selection(unit)
}

pub(super) fn insert(editor: &mut Editor, name: &str) -> bool {
    if editor.caret().insertion != InsertionMode::Normal || editor.is_inside_string_literal() {
        return false;
    }
    // APPLY callee repair and partial formatted-string insertion need the
    // native parent parser. Do not create a computed callee or erase a range.
    let mut current = editor.typeover_target().unwrap_or(editor.caret().node);
    while let Some(parent) = editor.arena().get(current).and_then(|node| node.parent()) {
        let node = editor.arena().get(parent).unwrap();
        if matches!(node.kind(), NodeKind::Call { .. }) && node.children().first() == Some(&current)
        {
            return false;
        }
        current = parent;
    }
    let atom_length = match editor.arena().get(editor.caret().node).unwrap().kind() {
        NodeKind::Number(text)
        | NodeKind::Identifier(text)
        | NodeKind::AnnotatedIdentifier { name: text, .. }
        | NodeKind::NativeIdentifier {
            name: text,
            subscript: None,
            ..
        }
        | NodeKind::BasedNumber { literal: text, .. } => Some(text.encode_utf16().count()),
        _ => None,
    };
    if let Some(range) = editor.caret().value_range
        && range.anchor != range.focus
    {
        if atom_length != Some(range.anchor.max(range.focus)) || range.anchor.min(range.focus) != 0
        {
            return false;
        }
    } else if editor.selection() == Selection::None
        && editor.typeover_target().is_none()
        && editor.arena().get(editor.caret().node).unwrap().kind() != &NodeKind::Placeholder
    {
        if let Some(range) = editor.caret().value_range {
            if atom_length != Some(range.focus) {
                return false;
            }
        } else if editor.caret().caret_side != Direction::Right {
            return false;
        }
    }
    let unit = PortableTree {
        kind: NodeKind::AnnotatedIdentifier {
            name: name.into(),
            labels: "UNIT".into(),
        },
        children: vec![],
    };
    let mut candidate = editor.clone_without_history();
    if apply(&mut candidate, &unit).is_err() || candidate.to_portable().is_err() {
        return false;
    }
    editor.edit_group(|editor| apply(editor, &unit).expect("validated unit insertion"));
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    fn number(text: &str) -> PortableTree {
        PortableTree {
            kind: NodeKind::Number(text.into()),
            children: vec![],
        }
    }

    #[test]
    fn unit_insertion_uses_scale_and_keeps_operand_identity_and_single_undo() {
        let source = number("20");
        let mut editor = Editor::from_portable(&source).unwrap();
        let original = editor.root();
        editor.place_text_caret(original, 2).unwrap();
        let caret = editor.caret();
        assert!(insert(&mut editor, "°C"));
        let tree = editor.to_portable().unwrap();
        assert_eq!(tree.kind, NodeKind::Binary(BinaryOp::Scale));
        assert_eq!(tree.children[0], source);
        assert_eq!(
            tree.children[1].kind,
            NodeKind::AnnotatedIdentifier {
                name: "°C".into(),
                labels: "UNIT".into()
            }
        );
        assert_eq!(
            editor.arena().get(editor.root()).unwrap().children()[0],
            original
        );
        assert!(editor.undo());
        assert_eq!(editor.to_portable().unwrap(), source);
        assert_eq!(editor.caret(), caret);
        assert!(!editor.undo());
        assert!(editor.redo());
        assert_eq!(editor.to_portable().unwrap(), tree);
    }

    #[test]
    fn unit_insertion_over_complete_tree_budget_is_atomic_and_preserves_redo() {
        let mut cell = number("0");
        for _ in 0..3 {
            cell = PortableTree {
                kind: NodeKind::Group,
                children: vec![cell],
            };
        }
        let source = PortableTree {
            kind: NodeKind::Binary(BinaryOp::Evaluate),
            children: vec![
                number("20"),
                PortableTree {
                    kind: NodeKind::Matrix {
                        rows: 1,
                        cols: 4095,
                        display: None,
                    },
                    children: vec![cell; 4095],
                },
            ],
        };
        let mut editor = Editor::from_portable(&source).unwrap();
        let left = editor.arena().get(editor.root()).unwrap().children()[0];
        editor
            .set_atom(left, NodeKind::Number("21".into()))
            .unwrap();
        assert!(editor.undo());
        editor.place_text_caret(left, 2).unwrap();
        let caret = editor.caret();
        let revision = editor.arena().revision();
        assert!(!insert(&mut editor, "m"));
        assert_eq!(editor.to_portable().unwrap(), source);
        assert_eq!(editor.caret(), caret);
        assert_eq!(editor.arena().revision(), revision);
        assert!(editor.redo());
        assert_eq!(
            editor.arena().get(left).unwrap().kind(),
            &NodeKind::Number("21".into())
        );
    }

    #[test]
    fn whole_output_unit_selection_replaces_and_fraction_typeover_uses_existing_repair() {
        let unit = PortableTree {
            kind: NodeKind::AnnotatedIdentifier {
                name: "K".into(),
                labels: "UNIT".into(),
            },
            children: vec![],
        };
        let source = PortableTree {
            kind: NodeKind::Binary(BinaryOp::Evaluate),
            children: vec![number("20"), unit],
        };
        let mut editor = Editor::from_portable(&source).unwrap();
        let right = editor.arena().get(editor.root()).unwrap().children()[1];
        editor.place_text_selection(right, 0, 1).unwrap();
        let caret = editor.caret();
        assert!(insert(&mut editor, "°F"));
        assert!(
            matches!(&editor.to_portable().unwrap().children[1].kind, NodeKind::AnnotatedIdentifier { name, labels } if name == "°F" && labels == "UNIT")
        );
        assert!(editor.undo());
        assert_eq!(editor.to_portable().unwrap(), source);
        assert_eq!(editor.caret(), caret);
        let fraction = PortableTree {
            kind: NodeKind::Fraction,
            children: vec![number("1"), number("2")],
        };
        let mut editor = Editor::from_portable(&fraction).unwrap();
        editor.place_caret(editor.root(), Direction::Right).unwrap();
        assert!(!editor.delete_structure(true));
        assert_eq!(editor.typeover_target(), Some(editor.root()));
        assert!(insert(&mut editor, "m"));
        assert!(matches!(
            editor.to_portable().unwrap().kind,
            NodeKind::AnnotatedIdentifier { .. }
        ));
        assert!(editor.undo());
        assert_eq!(editor.to_portable().unwrap(), fraction);
        assert_eq!(editor.typeover_target(), Some(editor.root()));
    }

    #[test]
    fn partial_value_and_callee_edits_are_atomic_rejections() {
        let source = PortableTree {
            kind: NodeKind::Call { argument_count: 1 },
            children: vec![
                PortableTree {
                    kind: NodeKind::Identifier("func".into()),
                    children: vec![],
                },
                number("20"),
            ],
        };
        let mut editor = Editor::from_portable(&source).unwrap();
        let callee = editor.arena().get(editor.root()).unwrap().children()[0];
        editor.place_text_caret(callee, 4).unwrap();
        let caret = editor.caret();
        let revision = editor.arena().revision();
        assert!(!insert(&mut editor, "m"));
        assert_eq!(editor.to_portable().unwrap(), source);
        assert_eq!(editor.caret(), caret);
        assert_eq!(editor.arena().revision(), revision);
        let argument = editor.arena().get(editor.root()).unwrap().children()[1];
        editor.place_text_selection(argument, 0, 1).unwrap();
        let caret = editor.caret();
        assert!(!insert(&mut editor, "m"));
        assert_eq!(editor.to_portable().unwrap(), source);
        assert_eq!(editor.caret(), caret);
        assert!(!editor.undo());
    }

    #[test]
    fn blank_area_unit_command_never_creates_an_outside_worksheet_region() {
        use crate::worksheet_ui::{WorksheetApp, area_ui::AreaEditor};
        for loaded in [false, true] {
            let mut app = WorksheetApp::default();
            let xml = mathfolio_document::area_authoring::empty_area_xml(100.).unwrap();
            let id = app.document.add_opaque(20., 20., xml.clone(), "Area");
            app.selected = Some(id);
            app.editing_region = Some(id);
            if loaded {
                app.area_editors.insert(id, AreaEditor::new(&xml).unwrap());
            }
            let before = app.document.clone();
            app.insert_unit("m");
            assert_eq!(app.document, before);
            assert_eq!(app.selected, Some(id));
            assert!(app.undo.is_empty());
            assert!(app.last_executed_regions.is_empty());
        }
    }
}
