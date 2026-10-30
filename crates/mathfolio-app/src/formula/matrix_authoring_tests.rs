//! MATRIX default creation and mode-sensitive insertion through CPU Key/Text.
use super::*;

fn node(kind: NodeKind, children: Vec<PortableTree>) -> PortableTree {
    PortableTree { kind, children }
}
fn number(text: &str) -> PortableTree {
    node(NodeKind::Number(text.into()), vec![])
}
fn matrix(cell: PortableTree) -> PortableTree {
    node(
        NodeKind::Matrix {
            rows: 1,
            cols: 1,
            display: None,
        },
        vec![cell],
    )
}
fn key(key: Key, mut modifiers: egui::Modifiers) -> egui::Event {
    modifiers.command |= modifiers.ctrl;
    egui::Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers,
    }
}
fn event(ctx: &egui::Context, editor: &mut Editor, event: egui::Event) -> bool {
    let mut changed = false;
    ctx.run_ui(
        egui::RawInput {
            events: vec![event],
            ..Default::default()
        },
        |ui| {
            let response = formula_ui_with_actions(ui, editor, None, true);
            changed = response.response.changed();
            assert!(!response.evaluation_requested);
        },
    )
    .drop_without_applying_deltas();
    assert!(editor.validate().is_ok());
    changed
}
fn start(source: PortableTree) -> (egui::Context, Editor) {
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    let mut editor = Editor::from_portable(&source).unwrap();
    ctx.run_ui(Default::default(), |ui| {
        formula_ui_with_actions(ui, &mut editor, None, true);
    })
    .drop_without_applying_deltas();
    (ctx, editor)
}

#[test]
fn ctrl_m_and_blank_bracket_create_one_empty_cell_without_discarding_history() {
    for trigger in [
        key(Key::M, egui::Modifiers::CTRL),
        egui::Event::Text("[".into()),
    ] {
        let (ctx, mut editor) = start(node(NodeKind::Placeholder, vec![]));
        let original = editor.caret();
        assert!(event(&ctx, &mut editor, trigger));
        assert_eq!(
            editor.to_portable().unwrap(),
            matrix(node(NodeKind::Placeholder, vec![]))
        );
        assert_eq!(
            editor.arena().get(editor.caret().node).unwrap().kind(),
            &NodeKind::Placeholder
        );
        assert!(editor.undo());
        assert_eq!(
            editor.to_portable().unwrap(),
            node(NodeKind::Placeholder, vec![])
        );
        assert_eq!(editor.caret(), original);
        assert!(!editor.undo());
        assert!(editor.redo());
        assert!(event(&ctx, &mut editor, egui::Event::Text("7".into())));
        assert_eq!(editor.to_portable().unwrap(), matrix(number("7")));
    }
}

#[test]
fn ctrl_m_full_selection_wraps_and_normal_end_preserves_numeric_operand_as_scale() {
    for full in [true, false] {
        let (ctx, mut editor) = start(number("12"));
        let root = editor.root();
        if full {
            editor.place_text_selection(root, 0, 2).unwrap();
        } else {
            editor.place_text_caret(root, 2).unwrap();
        }
        let before = editor.caret();
        assert!(event(&ctx, &mut editor, key(Key::M, egui::Modifiers::CTRL)));
        let expected = if full {
            matrix(number("12"))
        } else {
            node(
                NodeKind::Binary(BinaryOp::Scale),
                vec![number("12"), matrix(node(NodeKind::Placeholder, vec![]))],
            )
        };
        assert_eq!(editor.to_portable().unwrap(), expected);
        assert!(editor.undo());
        assert_eq!(editor.to_portable().unwrap(), number("12"));
        assert_eq!(editor.caret(), before);
        assert!(editor.redo());
    }
}

#[test]
fn matrix_cells_ctrl_m_is_a_noop_and_nonblank_bracket_keeps_index_command() {
    let source = node(
        NodeKind::Matrix {
            rows: 1,
            cols: 2,
            display: None,
        },
        vec![number("1"), number("2")],
    );
    let (ctx, mut editor) = start(source.clone());
    assert!(editor.select_matrix_cells(editor.root(), 0, 1));
    let selection = editor.matrix_selection();
    let revision = editor.arena().revision();
    assert!(!event(
        &ctx,
        &mut editor,
        key(Key::M, egui::Modifiers::CTRL)
    ));
    assert_eq!(editor.to_portable().unwrap(), source);
    assert_eq!(editor.matrix_selection(), selection);
    assert_eq!(editor.arena().revision(), revision);
    assert!(!editor.undo());
    let (ctx, mut editor) = start(node(NodeKind::Identifier("A".into()), vec![]));
    editor.place_caret(editor.root(), Direction::Right).unwrap();
    assert!(event(&ctx, &mut editor, egui::Event::Text("[".into())));
    assert!(matches!(
        editor.to_portable().unwrap().kind,
        NodeKind::Index { indices: 1 }
    ));
}
