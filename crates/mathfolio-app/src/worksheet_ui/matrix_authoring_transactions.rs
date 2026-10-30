//! Default matrix creation, actual cell authoring, draft calculation and native persistence.
use super::*;

fn node(kind: NodeKind, children: Vec<PortableTree>) -> PortableTree {
    PortableTree { kind, children }
}
fn number(text: &str) -> PortableTree {
    node(NodeKind::Number(text.into()), vec![])
}
fn blank() -> PortableTree {
    node(NodeKind::Placeholder, vec![])
}
fn name(text: &str) -> PortableTree {
    node(NodeKind::Identifier(text.into()), vec![])
}
fn binary(op: BinaryOp, left: PortableTree, right: PortableTree) -> PortableTree {
    node(NodeKind::Binary(op), vec![left, right])
}
fn matrix(rows: usize, cols: usize, values: &[&str]) -> PortableTree {
    node(
        NodeKind::Matrix {
            rows,
            cols,
            display: None,
        },
        values.iter().map(|s| number(s)).collect(),
    )
}
fn key(key: egui::Key, mut modifiers: egui::Modifiers) -> egui::Event {
    modifiers.command |= modifiers.ctrl;
    egui::Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers,
    }
}
fn text(value: &str) -> egui::Event {
    egui::Event::Text(value.into())
}
fn formula_event(ctx: &egui::Context, editor: &mut Editor, event: egui::Event) {
    ctx.run_ui(
        egui::RawInput {
            events: vec![event],
            ..Default::default()
        },
        |ui| {
            let response = crate::formula::formula_ui_with_actions(ui, editor, None, true);
            assert!(response.response.changed());
            assert!(!response.evaluation_requested);
        },
    )
    .drop_without_applying_deltas();
    assert!(editor.validate().is_ok());
}
fn formula_start(source: &PortableTree) -> (egui::Context, Editor) {
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    let mut editor = Editor::from_portable(source).unwrap();
    ctx.run_ui(Default::default(), |ui| {
        crate::formula::formula_ui_with_actions(ui, &mut editor, None, true);
    })
    .drop_without_applying_deltas();
    (ctx, editor)
}
fn authored_array() -> PortableTree {
    let (ctx, mut e) = formula_start(&blank());
    formula_event(&ctx, &mut e, key(egui::Key::M, egui::Modifiers::CTRL));
    assert_eq!(
        e.to_portable().unwrap(),
        node(
            NodeKind::Matrix {
                rows: 1,
                cols: 1,
                display: None
            },
            vec![blank()]
        )
    );
    formula_event(&ctx, &mut e, text("1"));
    // Shift+Space appends a column. Tab at its last cell appends a row.
    formula_event(&ctx, &mut e, key(egui::Key::Space, egui::Modifiers::SHIFT));
    formula_event(&ctx, &mut e, text("2"));
    formula_event(&ctx, &mut e, key(egui::Key::Tab, egui::Modifiers::NONE));
    formula_event(&ctx, &mut e, text("3"));
    let cell = e.caret().node;
    // Next-cell Tab changes navigation only, so inspect the destination separately.
    ctx.run_ui(
        egui::RawInput {
            events: vec![key(egui::Key::Tab, egui::Modifiers::NONE)],
            ..Default::default()
        },
        |ui| {
            assert!(
                !crate::formula::formula_ui_with_actions(ui, &mut e, None, true)
                    .response
                    .changed()
            );
        },
    )
    .drop_without_applying_deltas();
    assert_ne!(e.caret().node, cell);
    formula_event(&ctx, &mut e, text("4"));
    let expected = matrix(2, 2, &["1", "2", "3", "4"]);
    assert_eq!(e.to_portable().unwrap(), expected);
    expected
}
fn add(app: &mut WorksheetApp, y: f64, tree: PortableTree) -> u64 {
    let id = app.document.add_math(20., y);
    app.document.regions.last_mut().unwrap().content = CanvasContent::Math(tree.clone());
    app.editors
        .insert(id, Editor::from_portable(&tree).unwrap());
    id
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
fn assert_matrix(app: &WorksheetApp, id: u64, shape: (usize, usize), values: &[f64]) {
    let row = &app.results[&id];
    assert!(row.error.is_none(), "{row:?}");
    let Some(Value::Matrix {
        rows,
        cols,
        elements,
    }) = &row.value
    else {
        panic!("{row:?}")
    };
    assert_eq!((*rows, *cols), shape);
    assert_eq!(
        elements.iter().map(|q| q.value.re).collect::<Vec<_>>(),
        values
    );
    assert!(elements.iter().all(|q| q.value.im == 0.));
}

#[test]
fn ribbon_matrix_matches_ctrl_m_default_and_keeps_definition_result_until_enter() {
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    let mut app = WorksheetApp::default();
    let id = add(
        &mut app,
        20.,
        binary(BinaryOp::Define, name("A"), number("12")),
    );
    let query = add(
        &mut app,
        220.,
        binary(BinaryOp::Evaluate, name("A"), blank()),
    );
    app.recalculate();
    app.selected = Some(id);
    app.editing_region = Some(id);
    app.focus_new = Some(id);
    frame(&mut app, &ctx, vec![]);
    let editor = app.editors.get_mut(&id).unwrap();
    let rhs = editor.arena().get(editor.root()).unwrap().children()[1];
    editor.place_text_selection(rhs, 0, 2).unwrap();
    app.last_executed_regions.clear();
    app.ribbon_command(&ctx, crate::ribbon::RibbonAction::Matrix);
    let expected = binary(BinaryOp::Define, name("A"), matrix(1, 1, &["12"]));
    assert_eq!(app.editors[&id].to_portable().unwrap(), expected);
    assert!(app.last_executed_regions.is_empty());
    assert!(app.uncommitted_math.contains(&id));
    assert!(
        matches!(&app.results[&query].value, Some(Value::Scalar { quantity }) if quantity.value.re == 12.)
    );
    let editor = app.editors.get_mut(&id).unwrap();
    editor.place_caret(editor.root(), Direction::Right).unwrap();
    frame(
        &mut app,
        &ctx,
        vec![key(egui::Key::Enter, egui::Modifiers::NONE)],
    );
    assert_matrix(&app, query, (1, 1), &[12.]);
    assert!(!app.uncommitted_math.contains(&id));
    app.undo();
    assert_eq!(
        app.document.regions[0].content,
        CanvasContent::Math(binary(BinaryOp::Define, name("A"), number("12")))
    );
    app.redo();
    assert_eq!(
        app.document.regions[0].content,
        CanvasContent::Math(expected)
    );
    assert_matrix(&app, query, (1, 1), &[12.]);
}

fn fixture() -> (WorksheetApp, [u64; 4]) {
    let a = authored_array();
    let (ctx, mut e) = formula_start(&number("12"));
    e.place_text_selection(e.root(), 0, 2).unwrap();
    formula_event(&ctx, &mut e, key(egui::Key::M, egui::Modifiers::CTRL));
    let b = e.to_portable().unwrap();
    assert_eq!(b, matrix(1, 1, &["12"]));
    let (ctx, mut e) = formula_start(&number("2"));
    e.place_text_caret(e.root(), 1).unwrap();
    formula_event(&ctx, &mut e, key(egui::Key::M, egui::Modifiers::CTRL));
    formula_event(&ctx, &mut e, text("3"));
    let c = e.to_portable().unwrap();
    assert_eq!(
        c,
        binary(BinaryOp::Scale, number("2"), matrix(1, 1, &["3"]))
    );
    let mut app = WorksheetApp::default();
    for (index, (label, rhs)) in [("A", a), ("B", b), ("C", c)].into_iter().enumerate() {
        add(
            &mut app,
            20. + index as f64 * 150.,
            binary(BinaryOp::Define, name(label), rhs),
        );
    }
    let mut ids = [0; 4];
    for (index, expression) in [
        name("A"),
        name("B"),
        name("C"),
        node(
            NodeKind::Index { indices: 2 },
            vec![name("A"), number("0"), number("1")],
        ),
    ]
    .into_iter()
    .enumerate()
    {
        ids[index] = add(
            &mut app,
            500. + index as f64 * 150.,
            binary(BinaryOp::Evaluate, expression, blank()),
        );
    }
    (app, ids)
}
fn verify(app: &WorksheetApp, ids: [u64; 4]) {
    assert_eq!(app.document.regions.len(), 7);
    assert_eq!(app.results.len(), 7);
    assert!(
        app.results.values().all(|r| r.error.is_none()),
        "{:?}",
        app.results
    );
    assert_matrix(app, ids[0], (2, 2), &[1., 2., 3., 4.]);
    assert_matrix(app, ids[1], (1, 1), &[12.]);
    assert_matrix(app, ids[2], (1, 1), &[6.]);
    assert!(
        matches!(&app.results[&ids[3]].value, Some(Value::Scalar { quantity }) if quantity.value.re == 2. && quantity.value.im == 0.)
    );
}
#[test]
fn actual_matrix_creation_fill_expansion_and_scalar_adjacency_match_value_oracles() {
    let (mut app, ids) = fixture();
    app.recalculate();
    verify(&app, ids);
}
#[test]
#[ignore = "Explicit original matrix-authoring native/PDF fixture; no GUI or original runtime"]
fn matrix_authoring_save_native_export_reopen_probe() {
    let output = PathBuf::from(
        std::env::var_os("MATHFOLIO_MATRIX_AUTHORING_OUTPUT")
            .expect("MATHFOLIO_MATRIX_AUTHORING_OUTPUT"),
    );
    std::fs::create_dir_all(&output).unwrap();
    let (mut app, ids) = fixture();
    app.recalculate();
    verify(&app, ids);
    app.persist_calculation_cache();
    let saved = output.join("matrix-authoring.mathfolio");
    let native = output.join("matrix-authoring.mcdx");
    app.document.save(&saved).unwrap();
    mathfolio_document::export::export_mcdx(&app.document, &native).unwrap();
    let mut reopened = WorksheetApp::default();
    reopened.open(&saved);
    assert_eq!(reopened.document.regions.len(), 7);
    assert!(reopened.last_executed_regions.is_empty() && reopened.pending_regions.is_empty());
    assert_eq!(
        reopened
            .saved_results
            .values()
            .filter(|r| r.display.is_some())
            .count(),
        4
    );
    for id in ids {
        assert!(
            reopened
                .saved_results
                .get(&id)
                .is_some_and(|r| r.display.is_some())
        );
    }
    let mut imported = WorksheetApp::default();
    imported.open(&native);
    imported.recalculate();
    verify(&imported, ids);
    imported.persist_calculation_cache();
    imported
        .document
        .save(&output.join("matrix-authoring-reimported.mathfolio"))
        .unwrap();
    println!(
        "MATRIX_AUTHORING_NATIVE regions=7 saved_query_displays=4 saved_reopen_execution=0 saved_reopen_pending=0 native_recalculate_errors=0"
    );
}
