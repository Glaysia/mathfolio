use super::*;
use mathfolio_editor::SourceMatrixDisplay;

fn editor() -> Editor {
    Editor::from_portable(&PortableTree {
        kind: NodeKind::Matrix {
            rows: 4,
            cols: 4,
            display: Some(SourceMatrixDisplay::from_parts(None, Some([2, 2])).unwrap()),
        },
        children: (0..16)
            .map(|i| PortableTree {
                kind: NodeKind::Number(i.to_string()),
                children: vec![],
            })
            .collect(),
    })
    .unwrap()
}
fn key(key: Key, modifiers: egui::Modifiers) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    }
}
fn frame(ctx: &egui::Context, editor: &mut Editor, events: Vec<egui::Event>) -> bool {
    let mut changed = false;
    let mut evaluation_requested = false;
    let output = ctx.run_ui(
        egui::RawInput {
            events,
            ..Default::default()
        },
        |ui| {
            let response = formula_ui_with_actions(ui, editor, None, true);
            changed = response.response.changed();
            evaluation_requested = response.evaluation_requested;
        },
    );
    output.drop_without_applying_deltas();
    assert!(!evaluation_requested);
    changed
}
fn context() -> egui::Context {
    let context = egui::Context::default();
    crate::configure(&context);
    context
}
fn offset(editor: &Editor) -> Option<[u32; 2]> {
    let NodeKind::Matrix {
        display: Some(display),
        ..
    } = editor.arena().get(editor.root()).unwrap().kind()
    else {
        panic!()
    };
    display.offset()
}

#[test]
fn visible_layout_keeps_full_indices_and_separate_result_geometry() {
    let context = context();
    let editor = editor();
    let cells = editor
        .arena()
        .get(editor.root())
        .unwrap()
        .children()
        .to_vec();
    let mut seen = vec![];
    let mut result_geometry = true;
    let mut hidden_hit = false;
    let mut visible_hit = false;
    let output = context.run_ui(egui::RawInput::default(), |ui| {
        let layout = retained_layout(ui, &editor, None, FONT_SIZE, None);
        seen = layout.children.iter().map(|(_, cell)| cell.node).collect();
        result_geometry = matrix_viewport::find(&layout, Pos2::ZERO).is_some();
        let mut hits = vec![];
        collect_hits(&layout, Pos2::ZERO, &mut hits);
        hidden_hit = hits.iter().any(|hit| hit.node == cells[2]);
        visible_hit = hits.iter().any(|hit| hit.node == cells[5]);
    });
    output.drop_without_applying_deltas();
    assert_eq!(seen, vec![cells[0], cells[1], cells[4], cells[5]]);
    assert!(!result_geometry);
    assert!(!hidden_hit);
    assert!(visible_hit);
    assert_eq!(
        matrix_cell_corners(editor.arena(), cells[0], cells[5]),
        Some((editor.root(), 0, 5))
    );
}

#[test]
fn three_vertical_keys_in_one_batch_use_fresh_hits_and_create_source_undo() {
    let context = context();
    let mut editor = editor();
    let original = editor.to_portable().unwrap();
    let cells = editor
        .arena()
        .get(editor.root())
        .unwrap()
        .children()
        .to_vec();
    frame(&context, &mut editor, vec![]);
    assert!(frame(
        &context,
        &mut editor,
        vec![key(Key::ArrowDown, egui::Modifiers::NONE); 3]
    ));
    assert_eq!(editor.caret().node, cells[12]);
    assert_eq!(offset(&editor), Some([2, 0]));
    assert_eq!(editor.arena().get(editor.root()).unwrap().children(), cells);
    assert!(editor.undo());
    assert_eq!(offset(&editor), Some([1, 0]));
    assert!(editor.undo());
    assert!(editor.to_portable().unwrap() == original);
    assert!(editor.redo());
    assert!(editor.redo());
    assert_eq!(editor.caret().node, cells[12]);
}

#[test]
fn tab_horizontal_pan_and_shift_tab_keep_full_row_major_addresses() {
    let context = context();
    let mut editor = editor();
    let cells = editor
        .arena()
        .get(editor.root())
        .unwrap()
        .children()
        .to_vec();
    frame(&context, &mut editor, vec![]);
    assert!(frame(
        &context,
        &mut editor,
        vec![key(Key::Tab, egui::Modifiers::NONE); 2]
    ));
    assert_eq!(editor.caret().node, cells[2]);
    assert_eq!(offset(&editor), Some([0, 1]));
    frame(
        &context,
        &mut editor,
        vec![key(Key::Tab, egui::Modifiers::SHIFT); 2],
    );
    assert_eq!(editor.caret().node, cells[0]);
    assert_eq!(offset(&editor), Some([0, 0]));
}

#[test]
fn hidden_horizontal_arrow_target_pans_then_remains_editable() {
    let context = context();
    let mut editor = editor();
    let cells = editor
        .arena()
        .get(editor.root())
        .unwrap()
        .children()
        .to_vec();
    editor.place_caret(cells[1], Direction::Right).unwrap();
    frame(&context, &mut editor, vec![]);
    assert!(frame(
        &context,
        &mut editor,
        vec![key(Key::ArrowRight, egui::Modifiers::NONE)]
    ));
    assert_eq!(editor.caret().node, cells[2]);
    assert_eq!(offset(&editor), Some([0, 1]));
    let source = editor.to_portable().unwrap();
    assert!(frame(
        &context,
        &mut editor,
        vec![egui::Event::Text("9".into())]
    ));
    let changed = editor.to_portable().unwrap();
    assert!(changed.children[0] == source.children[0]);
    assert!(changed.children[2] != source.children[2]);
}

#[test]
fn idle_reuses_clipped_layout_without_source_edit() {
    let context = context();
    let mut editor = editor();
    let before = editor.to_portable().unwrap();
    let mut retained = None;
    let mut same = true;
    for _ in 0..2 {
        let output = context.run_ui(egui::RawInput::default(), |ui| {
            let layout = retained_layout(ui, &editor, None, FONT_SIZE, None);
            if let Some(previous) = &retained {
                same &= Arc::ptr_eq(previous, &layout);
            }
            retained = Some(layout);
        });
        output.drop_without_applying_deltas();
    }
    assert!(editor.to_portable().unwrap() == before);
    assert!(same);
    assert!(!editor.undo());
}

#[test]
fn visible_pointer_hit_addresses_full_cell_and_hidden_cells_have_no_hit() {
    let context = context();
    let mut editor = editor();
    let cells = editor
        .arena()
        .get(editor.root())
        .unwrap()
        .children()
        .to_vec();
    let before = editor.to_portable().unwrap();
    let mut pointer = Pos2::ZERO;
    let output = context.run_ui(egui::RawInput::default(), |ui| {
        let response = formula_ui_with_actions(ui, &mut editor, None, true);
        let layout = retained_layout(ui, &editor, None, 11. * 96. / 72., None);
        let hits = retained_hits(
            ui,
            &layout,
            response.response.rect.min + vec2(PADDING, PADDING),
        );
        pointer = hits
            .iter()
            .find(|hit| hit.node == cells[5])
            .unwrap()
            .rect
            .center();
    });
    output.drop_without_applying_deltas();
    frame(
        &context,
        &mut editor,
        vec![
            egui::Event::PointerMoved(pointer),
            egui::Event::PointerButton {
                pos: pointer,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
            egui::Event::PointerButton {
                pos: pointer,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    assert_eq!(editor.caret().node, cells[5]);
    assert!(editor.to_portable().unwrap() == before);
    assert!(!editor.undo());
}

#[test]
fn vertical_preserves_fraction_in_cell_before_full_row_step() {
    let context = context();
    let mut source = editor().to_portable().unwrap();
    source.kind = NodeKind::Matrix {
        rows: 4,
        cols: 4,
        display: Some(SourceMatrixDisplay::from_parts(None, Some([1, 1])).unwrap()),
    };
    source.children[0] = PortableTree {
        kind: NodeKind::Fraction,
        children: vec![
            PortableTree {
                kind: NodeKind::Number("1".into()),
                children: vec![],
            },
            PortableTree {
                kind: NodeKind::Number("2".into()),
                children: vec![],
            },
        ],
    };
    let mut editor = Editor::from_portable(&source).unwrap();
    let cells = editor
        .arena()
        .get(editor.root())
        .unwrap()
        .children()
        .to_vec();
    let denominator = editor.arena().get(cells[0]).unwrap().children()[1];
    frame(&context, &mut editor, vec![]);
    assert!(!frame(
        &context,
        &mut editor,
        vec![key(Key::ArrowDown, egui::Modifiers::NONE)]
    ));
    assert_eq!(editor.caret().node, denominator);
    assert!(editor.to_portable().unwrap() == source);
    assert!(frame(
        &context,
        &mut editor,
        vec![key(Key::ArrowDown, egui::Modifiers::NONE)]
    ));
    assert_eq!(editor.caret().node, cells[4]);
    assert_eq!(offset(&editor), Some([1, 0]));
}
