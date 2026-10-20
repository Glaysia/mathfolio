//! Original text parentheses across draft/commit and native persistence.
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
fn binary(op: BinaryOp, left: PortableTree, right: PortableTree) -> PortableTree {
    node(NodeKind::Binary(op), vec![left, right])
}
fn eval(value: PortableTree) -> PortableTree {
    binary(
        BinaryOp::Evaluate,
        value,
        node(NodeKind::Placeholder, vec![]),
    )
}
fn typed(input: &str) -> PortableTree {
    let mut editor = Editor::new();
    for ch in input.chars() {
        assert!(
            !crate::formula::input_text_with_actions(&mut editor, &ch.to_string())
                .evaluation_requested
        );
    }
    editor.to_portable().unwrap()
}
fn typed_with_right(prefix: &str, suffix: &str) -> PortableTree {
    let mut editor = Editor::new();
    crate::formula::input_text(&mut editor, prefix);
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    ctx.run_ui(Default::default(), |ui| {
        crate::formula::formula_ui_with_actions(ui, &mut editor, None, true);
    })
    .drop_without_applying_deltas();
    ctx.run_ui(
        egui::RawInput {
            events: vec![key(egui::Key::ArrowRight)],
            ..Default::default()
        },
        |ui| {
            let response = crate::formula::formula_ui_with_actions(ui, &mut editor, None, true);
            assert!(!response.response.changed() && !response.evaluation_requested);
        },
    )
    .drop_without_applying_deltas();
    crate::formula::input_text(&mut editor, suffix);
    editor.to_portable().unwrap()
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
fn scalar(app: &WorksheetApp, id: u64, value: f64) -> mathcraft_core::Quantity {
    let result = &app.results[&id];
    assert!(result.error.is_none(), "{result:?}");
    let Some(Value::Scalar { quantity }) = &result.value else {
        panic!("{result:?}")
    };
    assert_eq!(quantity.value.re, value, "{result:?}");
    assert_eq!(quantity.value.im, 0.);
    *quantity
}
fn edit(app: &mut WorksheetApp, ctx: &egui::Context, id: u64) {
    app.selected = Some(id);
    app.editing_region = Some(id);
    app.focus_new = Some(id);
    frame(app, ctx, vec![]);
}
fn commit(app: &mut WorksheetApp, ctx: &egui::Context, id: u64) {
    let editor = app.editors.get_mut(&id).unwrap();
    editor.place_caret(editor.root(), Direction::Right).unwrap();
    frame(app, ctx, vec![key(egui::Key::Enter)]);
}

#[test]
fn actual_parenthesis_and_number_events_preserve_old_result_until_enter_and_undo() {
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    let mut app = WorksheetApp::default();
    let source = eval(number("2"));
    let id = add(&mut app, 20., source.clone());
    app.recalculate();
    scalar(&app, id, 2.);
    app.last_executed_regions.clear();
    edit(&mut app, &ctx, id);
    let editor = app.editors.get_mut(&id).unwrap();
    let operand = editor.arena().get(editor.root()).unwrap().children()[0];
    editor.place_caret(operand, Direction::Right).unwrap();
    for text in ["(", "3", ")"] {
        frame(&mut app, &ctx, vec![egui::Event::Text(text.into())]);
        assert!(app.last_executed_regions.is_empty());
        scalar(&app, id, 2.);
    }
    assert_eq!(app.editors[&id].to_portable().unwrap(), eval(typed("2(3)")));
    assert!(app.uncommitted_math.contains(&id));
    commit(&mut app, &ctx, id);
    scalar(&app, id, 6.);
    assert!(!app.uncommitted_math.contains(&id));
    app.undo();
    assert_eq!(
        app.editors[&id].to_portable().unwrap(),
        eval(binary(
            BinaryOp::Scale,
            number("2"),
            node(NodeKind::Group, vec![node(NodeKind::Placeholder, vec![])])
        ))
    );
    app.redo();
    // The inverse snapshot was captured after leaving the equation. Redo
    // restores its committed source/result and may leave the editor lazy.
    assert_eq!(
        app.document.regions[0].content,
        CanvasContent::Math(eval(typed("2(3)")))
    );
    scalar(&app, id, 6.);
}

#[test]
fn actual_named_call_edit_keeps_old_value_and_function_binding_until_commit() {
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    let mut app = WorksheetApp::default();
    add(
        &mut app,
        20.,
        binary(
            BinaryOp::Define,
            node(
                NodeKind::Function { parameter_count: 1 },
                vec![name("f"), name("x")],
            ),
            binary(BinaryOp::Add, name("x"), number("1")),
        ),
    );
    let id = add(&mut app, 220., eval(typed("f(2)")));
    app.recalculate();
    scalar(&app, id, 3.);
    app.last_executed_regions.clear();
    edit(&mut app, &ctx, id);
    let editor = app.editors.get_mut(&id).unwrap();
    let operand = editor.arena().get(editor.root()).unwrap().children()[0];
    editor.select_subtree(operand).unwrap();
    app.apply_editor_command(|editor| editor.replace_selection(&name("f")).unwrap());
    let editor = app.editors.get_mut(&id).unwrap();
    let operand = editor.arena().get(editor.root()).unwrap().children()[0];
    editor.place_caret(operand, Direction::Right).unwrap();
    for text in ["(", "4", ")"] {
        frame(&mut app, &ctx, vec![egui::Event::Text(text.into())]);
        scalar(&app, id, 3.);
        assert!(app.last_executed_regions.is_empty());
    }
    commit(&mut app, &ctx, id);
    scalar(&app, id, 5.);
}

#[test]
#[ignore = "Explicit parentheses/native/PDF integration fixture; no GUI or original runtime"]
fn parenthesis_save_native_export_reopen_probe() {
    let output = PathBuf::from(
        std::env::var_os("MATHCRAFT_PARENTHESIS_OUTPUT").expect("MATHCRAFT_PARENTHESIS_OUTPUT"),
    );
    std::fs::create_dir_all(&output).unwrap();
    let mut app = WorksheetApp::default();
    add(
        &mut app,
        20.,
        binary(
            BinaryOp::Define,
            node(
                NodeKind::Function { parameter_count: 1 },
                vec![name("f"), name("x")],
            ),
            binary(BinaryOp::Add, name("x"), number("1")),
        ),
    );
    let cases = [
        ("2(3)", 6.),
        ("(2+3)(4)", 20.),
        ("2(3+4)", 14.),
        ("3(2m)", 6.),
        ("sqrt(4)", 2.),
        ("abs(-3)", 3.),
        ("f(4)", 5.),
        ("2+3(4)", 14.),
        ("2*3(4)", 24.),
        ("2^3(2)", 64.),
        ("2/3(4)", 1. / 6.),
        ("2^3+4", 128.),
        ("2/3+4", 2. / 7.),
        ("(2^3)+4", 12.),
    ];
    let ids: Vec<_> = cases
        .iter()
        .enumerate()
        .map(|(i, (text, _))| {
            // The last source is produced with an actual ArrowRight before
            // ')'; the same plain text stays in the exponent in Prime.
            let value = if *text == "(2^3)+4" {
                typed_with_right("(2^3", ")+4")
            } else {
                typed(text)
            };
            add(&mut app, 220. + i as f64 * 220., eval(value))
        })
        .collect();
    let verify = |app: &WorksheetApp| {
        assert_eq!(app.results.len(), 15);
        assert!(
            app.results.values().all(|row| row.error.is_none()),
            "{:?}",
            app.results
        );
        for ((_, expected), id) in cases.iter().zip(&ids) {
            scalar(app, *id, *expected);
        }
        assert_eq!(
            scalar(app, ids[3], 6.).dimension,
            mathcraft_core::unit("m").unwrap().dimension
        );
    };
    app.recalculate();
    verify(&app);
    app.persist_calculation_cache();
    let saved = output.join("parenthesis.mathcraft");
    let native = output.join("parenthesis.mcdx");
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
        14
    );
    let mut imported = WorksheetApp::default();
    imported.open(&native);
    assert_eq!(imported.document.regions.len(), 15);
    assert!(
        imported
            .document
            .regions
            .iter()
            .all(|region| matches!(region.content, CanvasContent::Math(_)))
    );
    imported.recalculate();
    verify(&imported);
    imported.persist_calculation_cache();
    imported
        .document
        .save(&output.join("parenthesis-reimported.mathcraft"))
        .unwrap();
    println!(
        "PARENTHESIS_NATIVE regions=15 saved_query_displays=14 saved_reopen_execution=0 native_recalculate_errors=0"
    );
}
