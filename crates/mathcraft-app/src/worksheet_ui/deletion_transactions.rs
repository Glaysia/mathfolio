//! Native structural deletion through actual editor events and commit boundaries.
use super::*;

fn node(kind: NodeKind, children: Vec<PortableTree>) -> PortableTree {
    PortableTree { kind, children }
}
fn number(value: &str) -> PortableTree {
    node(NodeKind::Number(value.into()), vec![])
}
fn name(value: &str) -> PortableTree {
    node(NodeKind::Identifier(value.into()), vec![])
}
fn definitions(app: &mut WorksheetApp) {
    for (index, (label, value)) in [("x", "2"), ("y", "3"), ("xy", "7")]
        .into_iter()
        .enumerate()
    {
        add(
            app,
            20. + index as f64 * 140.,
            node(
                NodeKind::Binary(BinaryOp::Define),
                vec![name(label), number(value)],
            ),
        );
    }
}
fn blank() -> PortableTree {
    node(NodeKind::Placeholder, vec![])
}
fn eval(value: PortableTree) -> PortableTree {
    node(NodeKind::Binary(BinaryOp::Evaluate), vec![value, blank()])
}
fn add(app: &mut WorksheetApp, y: f64, tree: PortableTree) -> u64 {
    let id = app.document.add_math(20., y);
    app.document.regions.last_mut().unwrap().content = CanvasContent::Math(tree.clone());
    app.editors
        .insert(id, Editor::from_portable(&tree).unwrap());
    id
}
fn key(key: egui::Key) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }
}
fn frame(app: &mut WorksheetApp, ctx: &egui::Context, events: Vec<egui::Event>) {
    ctx.run_ui(
        egui::RawInput {
            events,
            screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1200., 900.))),
            ..Default::default()
        },
        |ui| {
            app.global_editing_shortcuts(ui.ctx());
            app.document_history_shortcuts(ui.ctx());
            app.region_clipboard_shortcuts(ui.ctx());
            app.page_ui(ui);
        },
    )
    .drop_without_applying_deltas();
}
fn scalar(app: &WorksheetApp, id: u64, expected: f64) {
    let result = &app.results[&id];
    assert!(result.error.is_none(), "{result:?}");
    let Some(Value::Scalar { quantity }) = &result.value else {
        panic!("{result:?}")
    };
    assert_eq!(quantity.value.re, expected);
    assert_eq!(quantity.value.im, 0.);
}

#[test]
fn actual_deletion_between_integer_operands_commits_only_on_enter_and_undo_restores_both() {
    for action in [egui::Key::Backspace, egui::Key::Delete] {
        let ctx = egui::Context::default();
        crate::configure(&ctx);
        let mut app = WorksheetApp::default();
        let source = eval(node(
            NodeKind::Binary(BinaryOp::Add),
            vec![number("12"), number("34")],
        ));
        let id = add(&mut app, 20., source.clone());
        app.recalculate();
        scalar(&app, id, 46.);
        app.last_executed_regions.clear();
        app.selected = Some(id);
        app.editing_region = Some(id);
        app.focus_new = Some(id);
        frame(&mut app, &ctx, vec![]);
        let editor = app.editors.get_mut(&id).unwrap();
        let sum = editor.arena().get(editor.root()).unwrap().children()[0];
        let target = editor.arena().get(sum).unwrap().children()
            [usize::from(action == egui::Key::Backspace)];
        editor
            .place_text_caret(target, if action == egui::Key::Backspace { 0 } else { 2 })
            .unwrap();
        frame(&mut app, &ctx, vec![key(action)]);
        assert_eq!(
            app.editors[&id].to_portable().unwrap(),
            eval(number("1234"))
        );
        assert!(app.last_executed_regions.is_empty() && app.uncommitted_math.contains(&id));
        scalar(&app, id, 46.);
        let editor = app.editors.get_mut(&id).unwrap();
        editor.place_caret(editor.root(), Direction::Right).unwrap();
        frame(&mut app, &ctx, vec![key(egui::Key::Enter)]);
        scalar(&app, id, 1234.);
        assert!(!app.uncommitted_math.contains(&id));
        app.undo();
        assert_eq!(app.editors[&id].to_portable().unwrap(), source);
        app.redo();
        assert_eq!(
            app.document.regions[0].content,
            CanvasContent::Math(eval(number("1234")))
        );
        scalar(&app, id, 1234.);
    }
}

#[test]
fn actual_name_operator_deletion_references_the_merged_name_only_after_enter() {
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    let mut app = WorksheetApp::default();
    definitions(&mut app);
    let id = add(
        &mut app,
        440.,
        eval(node(
            NodeKind::Binary(BinaryOp::Add),
            vec![name("x"), name("y")],
        )),
    );
    app.recalculate();
    scalar(&app, id, 5.);
    app.last_executed_regions.clear();
    app.selected = Some(id);
    app.editing_region = Some(id);
    app.focus_new = Some(id);
    frame(&mut app, &ctx, vec![]);
    let editor = app.editors.get_mut(&id).unwrap();
    let sum = editor.arena().get(editor.root()).unwrap().children()[0];
    editor
        .place_operator_caret(sum, Direction::Indeterminate)
        .unwrap();
    frame(&mut app, &ctx, vec![key(egui::Key::Backspace)]);
    assert_eq!(app.editors[&id].to_portable().unwrap(), eval(name("xy")));
    assert!(app.last_executed_regions.is_empty() && app.uncommitted_math.contains(&id));
    scalar(&app, id, 5.);
    let editor = app.editors.get_mut(&id).unwrap();
    editor.place_caret(editor.root(), Direction::Right).unwrap();
    frame(&mut app, &ctx, vec![key(egui::Key::Enter)]);
    scalar(&app, id, 7.);
    assert!(!app.uncommitted_math.contains(&id));
}

enum Position {
    Text(usize, usize),
    Placeholder(usize),
    Operator(Direction),
    Boundary(Direction),
}
fn deleted(source: PortableTree, position: Position, action: egui::Key) -> PortableTree {
    let mut editor = Editor::from_portable(&source).unwrap();
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    ctx.run_ui(Default::default(), |ui| {
        crate::formula::formula_ui_with_actions(ui, &mut editor, None, true);
    })
    .drop_without_applying_deltas();
    match position {
        Position::Text(slot, offset) => {
            let child = editor.arena().get(editor.root()).unwrap().children()[slot];
            editor.place_text_caret(child, offset).unwrap();
        }
        Position::Placeholder(slot) => {
            let child = editor.arena().get(editor.root()).unwrap().children()[slot];
            editor.place_caret(child, Direction::Indeterminate).unwrap();
        }
        Position::Operator(side) => editor.place_operator_caret(editor.root(), side).unwrap(),
        Position::Boundary(side) => editor.place_caret(editor.root(), side).unwrap(),
    }
    ctx.run_ui(
        egui::RawInput {
            events: vec![key(action)],
            ..Default::default()
        },
        |ui| {
            let response = crate::formula::formula_ui_with_actions(ui, &mut editor, None, true);
            assert!(response.response.changed() && !response.evaluation_requested);
        },
    )
    .drop_without_applying_deltas();
    editor.to_portable().unwrap()
}

#[test]
#[ignore = "Explicit deletion/native/PDF integration fixture; no GUI or original runtime"]
fn structural_deletion_save_native_export_reopen_probe() {
    let output = PathBuf::from(
        std::env::var_os("MATHCRAFT_STRUCTURAL_DELETION_OUTPUT")
            .expect("MATHCRAFT_STRUCTURAL_DELETION_OUTPUT"),
    );
    std::fs::create_dir_all(&output).unwrap();
    let mut app = WorksheetApp::default();
    definitions(&mut app);
    let cases = [
        (
            node(
                NodeKind::Binary(BinaryOp::Add),
                vec![number("12"), number("34")],
            ),
            Position::Text(1, 0),
            egui::Key::Backspace,
            "1234",
        ),
        (
            node(
                NodeKind::Binary(BinaryOp::Add),
                vec![number("12"), number("34")],
            ),
            Position::Text(0, 2),
            egui::Key::Delete,
            "1234",
        ),
        (
            node(NodeKind::Fraction, vec![number("12"), number("34")]),
            Position::Operator(Direction::Indeterminate),
            egui::Key::Delete,
            "1234",
        ),
        (
            node(NodeKind::Power, vec![number("5"), number("2")]),
            Position::Operator(Direction::Indeterminate),
            egui::Key::Backspace,
            "52",
        ),
        (
            node(NodeKind::Group, vec![number("12")]),
            Position::Operator(Direction::Left),
            egui::Key::Delete,
            "12",
        ),
        (
            node(NodeKind::Group, vec![number("34")]),
            Position::Boundary(Direction::Right),
            egui::Key::Backspace,
            "34",
        ),
        (
            node(NodeKind::Fraction, vec![blank(), number("34")]),
            Position::Placeholder(0),
            egui::Key::Delete,
            "34",
        ),
        (
            node(NodeKind::Power, vec![number("12"), blank()]),
            Position::Placeholder(1),
            egui::Key::Backspace,
            "12",
        ),
    ];
    let mut ids = vec![];
    let mut values = vec![];
    for (index, (source, position, action, expected)) in cases.into_iter().enumerate() {
        let result = deleted(source, position, action);
        assert_eq!(result, number(expected));
        ids.push(add(&mut app, 440. + index as f64 * 140., eval(result)));
        values.push(expected.parse::<f64>().unwrap());
    }
    for (index, (source, expected_tree, value)) in [
        (
            node(
                NodeKind::Binary(BinaryOp::Add),
                vec![number("1.2"), number("3")],
            ),
            number("1.23"),
            1.23,
        ),
        (
            node(
                NodeKind::Binary(BinaryOp::Add),
                vec![number("1.2"), number("3.4")],
            ),
            node(
                NodeKind::Binary(BinaryOp::Scale),
                vec![number("1.2"), number("3.4")],
            ),
            4.08,
        ),
        (
            node(NodeKind::Binary(BinaryOp::Add), vec![name("x"), name("y")]),
            name("xy"),
            7.,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let result = deleted(
            source,
            Position::Operator(Direction::Indeterminate),
            egui::Key::Backspace,
        );
        assert_eq!(result, expected_tree);
        ids.push(add(&mut app, 1560. + index as f64 * 140., eval(result)));
        values.push(value);
    }
    let verify = |app: &WorksheetApp| {
        assert_eq!(app.results.len(), 14);
        for (&id, &value) in ids.iter().zip(&values) {
            scalar(app, id, value);
        }
    };
    app.recalculate();
    verify(&app);
    app.persist_calculation_cache();
    let saved = output.join("structural-deletion.mathcraft");
    let native = output.join("structural-deletion.mcdx");
    app.document.save(&saved).unwrap();
    mathcraft_document::export::export_mcdx(&app.document, &native).unwrap();
    let mut reopened = WorksheetApp::default();
    reopened.open(&saved);
    assert!(reopened.last_executed_regions.is_empty() && reopened.pending_regions.is_empty());
    assert_eq!(
        reopened
            .saved_results
            .values()
            .filter(|row| row.display.is_some())
            .count(),
        11
    );
    let mut imported = WorksheetApp::default();
    imported.open(&native);
    assert_eq!(imported.document.regions.len(), 14);
    imported.recalculate();
    verify(&imported);
    imported.persist_calculation_cache();
    imported
        .document
        .save(&output.join("structural-deletion-reimported.mathcraft"))
        .unwrap();
    println!(
        "STRUCTURAL_DELETION_NATIVE regions=14 saved_query_displays=11 saved_reopen_execution=0 native_recalculate_errors=0"
    );
}
