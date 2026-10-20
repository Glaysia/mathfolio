//! Proposed real egui event routing checks; no target application or GUI.
use super::*;

fn frame(
    editor: &mut Editor,
    ctx: &egui::Context,
    key: Option<(Key, egui::Modifiers)>,
) -> (bool, bool) {
    let mut result = None;
    let events = key
        .map(|(key, modifiers)| egui::Event::Key {
            key,
            physical_key: Some(key),
            pressed: true,
            repeat: false,
            modifiers,
        })
        .into_iter()
        .collect();
    ctx.run_ui(
        egui::RawInput {
            events,
            ..Default::default()
        },
        |ui| {
            let drawn = formula_ui_with_actions(ui, editor, None, true);
            result = Some((drawn.response.changed(), drawn.evaluation_requested));
        },
    )
    .drop_without_applying_deltas();
    result.unwrap()
}

fn setup() -> (Editor, egui::Context) {
    let mut editor = Editor::from_portable(&PortableTree {
        kind: NodeKind::Number("1234".into()),
        children: vec![],
    })
    .unwrap();
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    frame(&mut editor, &ctx, None);
    editor.place_text_caret(editor.root(), 2).unwrap();
    (editor, ctx)
}

fn command_shift() -> egui::Modifiers {
    egui::Modifiers {
        shift: true,
        ..egui::Modifiers::COMMAND
    }
}

#[test]
fn command_shift_arrows_are_reserved_without_collapsing_or_moving_text_caret() {
    for key in [Key::ArrowLeft, Key::ArrowRight] {
        let (mut editor, ctx) = setup();
        let source = editor.to_portable().unwrap();
        let before = editor.caret();
        let selection = editor.selection();
        let revision = editor.arena().revision();
        assert_eq!(
            frame(&mut editor, &ctx, Some((key, command_shift()))),
            (false, false)
        );
        assert_eq!(editor.caret(), before);
        assert_eq!(editor.selection(), selection);
        assert_eq!(editor.to_portable().unwrap(), source);
        assert_eq!(editor.arena().revision(), revision);
        assert!(!editor.undo());
    }
}

#[test]
fn plain_command_arrows_still_move_to_atom_extremes() {
    for (key, offset) in [(Key::ArrowLeft, 0), (Key::ArrowRight, 4)] {
        let (mut editor, ctx) = setup();
        let source = editor.to_portable().unwrap();
        let revision = editor.arena().revision();
        assert_eq!(
            frame(&mut editor, &ctx, Some((key, egui::Modifiers::COMMAND))),
            (false, false)
        );
        assert_eq!(editor.caret().value_range.unwrap().focus, offset);
        assert_eq!(editor.to_portable().unwrap(), source);
        assert_eq!(editor.arena().revision(), revision);
    }
}

#[test]
fn plain_shift_arrows_keep_existing_character_selection_path() {
    for (key, focus) in [(Key::ArrowLeft, 1), (Key::ArrowRight, 3)] {
        let (mut editor, ctx) = setup();
        let source = editor.to_portable().unwrap();
        let shift = egui::Modifiers {
            shift: true,
            ..egui::Modifiers::NONE
        };
        assert_eq!(frame(&mut editor, &ctx, Some((key, shift))), (false, false));
        let range = editor.caret().value_range.unwrap();
        assert_eq!((range.anchor, range.focus), (2, focus));
        assert_eq!(editor.to_portable().unwrap(), source);
    }
}

#[test]
fn reserved_chord_preserves_structural_selection_and_redo() {
    let (mut editor, ctx) = setup();
    assert!(editor.insert_typed_operator(NodeKind::Binary(BinaryOp::Add)));
    let redo_source = editor.to_portable().unwrap();
    assert!(editor.undo());
    let source = editor.to_portable().unwrap();
    editor.select_subtree(editor.root()).unwrap();
    let before = editor.caret();
    let selected = editor.selection();
    let revision = editor.arena().revision();
    assert_eq!(
        frame(&mut editor, &ctx, Some((Key::ArrowRight, command_shift()))),
        (false, false)
    );
    assert_eq!(editor.caret(), before);
    assert_eq!(editor.selection(), selected);
    assert_eq!(editor.arena().revision(), revision);
    assert_eq!(editor.to_portable().unwrap(), source);
    assert!(editor.redo());
    assert_eq!(editor.to_portable().unwrap(), redo_source);
}

#[test]
fn reserved_chord_does_not_misroute_empty_matrix_cell_to_horizontal_navigation() {
    let mut editor = Editor::from_portable(&PortableTree {
        kind: NodeKind::Matrix {
            rows: 2,
            cols: 2,
            display: None,
        },
        children: (0..4)
            .map(|_| PortableTree {
                kind: NodeKind::Placeholder,
                children: vec![],
            })
            .collect(),
    })
    .unwrap();
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    frame(&mut editor, &ctx, None);
    let before = editor.caret();
    let source = editor.to_portable().unwrap();
    let revision = editor.arena().revision();
    for key in [Key::ArrowLeft, Key::ArrowRight] {
        assert_eq!(
            frame(&mut editor, &ctx, Some((key, command_shift()))),
            (false, false)
        );
        assert_eq!(editor.caret(), before);
        assert_eq!(editor.to_portable().unwrap(), source);
        assert_eq!(editor.arena().revision(), revision);
    }
}
