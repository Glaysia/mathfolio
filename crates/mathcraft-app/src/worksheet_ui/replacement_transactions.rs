//! Actual operator replacement, commit history and native saved-results flow.
use super::*;
fn node(kind: NodeKind, children: Vec<PortableTree>) -> PortableTree {
    PortableTree { kind, children }
}
fn number(value: &str) -> PortableTree {
    node(NodeKind::Number(value.into()), vec![])
}
fn op(kind: NodeKind, a: PortableTree, b: PortableTree) -> PortableTree {
    node(kind, vec![a, b])
}
fn add_tree(a: PortableTree, b: PortableTree) -> PortableTree {
    op(NodeKind::Binary(BinaryOp::Add), a, b)
}
fn eval(value: PortableTree) -> PortableTree {
    op(
        NodeKind::Binary(BinaryOp::Evaluate),
        value,
        node(NodeKind::Placeholder, vec![]),
    )
}
fn add(app: &mut WorksheetApp, y: f64, tree: PortableTree) -> u64 {
    let id = app.document.add_math(20., y);
    app.document.regions.last_mut().unwrap().content = CanvasContent::Math(tree.clone());
    app.editors
        .insert(id, Editor::from_portable(&tree).unwrap());
    id
}
fn scalar(app: &WorksheetApp, id: u64, value: f64) {
    let result = &app.results[&id];
    assert!(result.error.is_none(), "{result:?}");
    let Some(Value::Scalar { quantity }) = &result.value else {
        panic!("{result:?}")
    };
    assert_eq!(quantity.value.re, value);
    assert_eq!(quantity.value.im, 0.);
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

#[test]
fn actual_center_replacement_keeps_saved_result_until_enter_and_restores_source_on_undo() {
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    let mut app = WorksheetApp::default();
    let source = eval(add_tree(number("12"), number("34")));
    let id = add(&mut app, 20., source.clone());
    app.recalculate();
    scalar(&app, id, 46.);
    app.last_executed_regions.clear();
    app.selected = Some(id);
    app.editing_region = Some(id);
    app.focus_new = Some(id);
    frame(&mut app, &ctx, vec![]);
    let editor = app.editors.get_mut(&id).unwrap();
    let target = editor.arena().get(editor.root()).unwrap().children()[0];
    editor
        .place_operator_caret(target, Direction::Indeterminate)
        .unwrap();
    frame(&mut app, &ctx, vec![egui::Event::Text("*".into())]);
    let changed = eval(op(
        NodeKind::Binary(BinaryOp::Multiply),
        number("12"),
        number("34"),
    ));
    assert_eq!(app.editors[&id].to_portable().unwrap(), changed);
    assert!(app.uncommitted_math.contains(&id) && app.last_executed_regions.is_empty());
    scalar(&app, id, 46.);
    let editor = app.editors.get_mut(&id).unwrap();
    editor.place_caret(editor.root(), Direction::Right).unwrap();
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: Some(egui::Key::Enter),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    scalar(&app, id, 408.);
    assert!(!app.uncommitted_math.contains(&id));
    app.undo();
    assert_eq!(app.editors[&id].to_portable().unwrap(), source);
    app.redo();
    assert_eq!(
        app.document.regions[0].content,
        CanvasContent::Math(changed)
    );
    scalar(&app, id, 408.);
}

fn replaced(source: PortableTree, path: &[usize], text: &str) -> PortableTree {
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    let mut editor = Editor::from_portable(&source).unwrap();
    ctx.run_ui(Default::default(), |ui| {
        crate::formula::formula_ui_with_actions(ui, &mut editor, None, true);
    })
    .drop_without_applying_deltas();
    let target = path.iter().fold(editor.root(), |id, slot| {
        editor.arena().get(id).unwrap().children()[*slot]
    });
    editor
        .place_operator_caret(target, Direction::Indeterminate)
        .unwrap();
    ctx.run_ui(
        egui::RawInput {
            events: vec![egui::Event::Text(text.into())],
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
fn fraction_typeover_keeps_result_until_enter_and_undo_restores_the_pending_selection() {
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    let mut app = WorksheetApp::default();
    let source = eval(op(NodeKind::Fraction, number("12"), number("2")));
    let id = add(&mut app, 20., source.clone());
    app.recalculate();
    scalar(&app, id, 6.);
    app.last_executed_regions.clear();
    app.selected = Some(id);
    app.editing_region = Some(id);
    app.focus_new = Some(id);
    frame(&mut app, &ctx, vec![]);
    let editor = app.editors.get_mut(&id).unwrap();
    let target = editor.arena().get(editor.root()).unwrap().children()[0];
    editor.place_caret(target, Direction::Right).unwrap();
    let key = |key| egui::Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    };
    frame(&mut app, &ctx, vec![key(egui::Key::Backspace)]);
    assert_eq!(app.editors[&id].typeover_target(), Some(target));
    assert!(!app.uncommitted_math.contains(&id));
    assert!(app.last_executed_regions.is_empty());
    scalar(&app, id, 6.);
    frame(&mut app, &ctx, vec![egui::Event::Text("24".into())]);
    assert_eq!(app.editors[&id].to_portable().unwrap(), eval(number("24")));
    assert!(app.uncommitted_math.contains(&id) && app.last_executed_regions.is_empty());
    scalar(&app, id, 6.);
    let editor = app.editors.get_mut(&id).unwrap();
    editor.place_caret(editor.root(), Direction::Right).unwrap();
    frame(&mut app, &ctx, vec![key(egui::Key::Enter)]);
    scalar(&app, id, 24.);
    app.undo();
    assert_eq!(app.editors[&id].to_portable().unwrap(), source);
    // Document-level undo restores the pre-commit editor snapshot including mode.
    assert_eq!(app.editors[&id].typeover_target(), Some(target));
    app.redo();
    scalar(&app, id, 24.);
}

#[test]
#[ignore = "Explicit operator replacement/native/PDF integration fixture; no GUI or original runtime"]
fn operator_replacement_save_native_export_reopen_probe() {
    let output = PathBuf::from(
        std::env::var_os("MATHCRAFT_OPERATOR_REPLACEMENT_OUTPUT")
            .expect("MATHCRAFT_OPERATOR_REPLACEMENT_OUTPUT"),
    );
    std::fs::create_dir_all(&output).unwrap();
    let mut app = WorksheetApp::default();
    let cases = [
        (add_tree(number("12"), number("34")), vec![], "*", 408.),
        (add_tree(number("6"), number("3")), vec![], "/", 2.),
        (
            add_tree(add_tree(number("1"), number("2")), number("3")),
            vec![],
            "*",
            7.,
        ),
        (add_tree(number("12"), number("34")), vec![], "-", -22.),
        (
            op(
                NodeKind::Binary(BinaryOp::Multiply),
                number("3"),
                number("4"),
            ),
            vec![],
            "+",
            7.,
        ),
        (
            op(NodeKind::Fraction, number("6"), number("3")),
            vec![],
            "*",
            18.,
        ),
        (
            op(
                NodeKind::Power,
                op(
                    NodeKind::Binary(BinaryOp::Multiply),
                    number("2"),
                    number("3"),
                ),
                number("2"),
            ),
            vec![0],
            "+",
            25.,
        ),
        (
            add_tree(
                op(
                    NodeKind::Binary(BinaryOp::Multiply),
                    number("2"),
                    number("3"),
                ),
                number("4"),
            ),
            vec![],
            "*",
            24.,
        ),
        (add_tree(number("2"), number("3")), vec![], "^", 8.),
        (
            op(NodeKind::Power, number("2"), number("3")),
            vec![],
            "+",
            5.,
        ),
    ];
    let mut expected = vec![];
    for (index, (source, path, text, value)) in cases.into_iter().enumerate() {
        let tree = replaced(source, &path, text);
        expected.push((add(&mut app, 20. + index as f64 * 140., eval(tree)), value));
    }
    let verify = |app: &WorksheetApp| {
        assert_eq!(app.results.len(), 10);
        for &(id, value) in &expected {
            scalar(app, id, value)
        }
    };
    app.recalculate();
    verify(&app);
    app.persist_calculation_cache();
    let saved = output.join("operator-replacement.mathcraft");
    let native = output.join("operator-replacement.mcdx");
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
        10
    );
    let mut imported = WorksheetApp::default();
    imported.open(&native);
    assert_eq!(imported.document.regions.len(), 10);
    imported.recalculate();
    verify(&imported);
    imported.persist_calculation_cache();
    imported
        .document
        .save(&output.join("operator-replacement-reimported.mathcraft"))
        .unwrap();
    println!(
        "OPERATOR_REPLACEMENT_NATIVE regions=10 saved_query_displays=10 saved_reopen_execution=0 native_recalculate_errors=0"
    );
}
