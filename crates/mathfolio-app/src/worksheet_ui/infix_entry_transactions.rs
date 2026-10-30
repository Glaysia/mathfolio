//! Typed native operators across draft/commit and native persistence.
use super::*;

fn number(value: &str) -> PortableTree {
    PortableTree {
        kind: NodeKind::Number(value.into()),
        children: vec![],
    }
}
fn binary(op: BinaryOp, left: PortableTree, right: PortableTree) -> PortableTree {
    PortableTree {
        kind: NodeKind::Binary(op),
        children: vec![left, right],
    }
}
fn eval(value: PortableTree) -> PortableTree {
    binary(
        BinaryOp::Evaluate,
        value,
        PortableTree {
            kind: NodeKind::Placeholder,
            children: vec![],
        },
    )
}
fn add(app: &mut WorksheetApp, y: f64, tree: PortableTree) -> u64 {
    let id = app.document.add_math(20., y);
    app.document.regions.last_mut().unwrap().content = CanvasContent::Math(tree.clone());
    app.editors
        .insert(id, Editor::from_portable(&tree).unwrap());
    id
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
fn enter() -> egui::Event {
    egui::Event::Key {
        key: egui::Key::Enter,
        physical_key: Some(egui::Key::Enter),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }
}

#[test]
fn actual_middle_operator_text_keeps_saved_result_until_enter_and_restores_with_history() {
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    let mut app = WorksheetApp::default();
    let id = add(&mut app, 20., eval(number("1234")));
    app.recalculate();
    app.last_executed_regions.clear();
    app.selected = Some(id);
    app.editing_region = Some(id);
    app.focus_new = Some(id);
    frame(&mut app, &ctx, vec![]);
    let editor = app.editors.get_mut(&id).unwrap();
    let operand = editor.arena().get(editor.root()).unwrap().children()[0];
    editor.place_text_caret(operand, 2).unwrap();
    frame(&mut app, &ctx, vec![egui::Event::Text("+".into())]);
    let changed = eval(binary(BinaryOp::Add, number("12"), number("34")));
    assert_eq!(app.editors[&id].to_portable().unwrap(), changed);
    assert!(app.last_executed_regions.is_empty() && app.uncommitted_math.contains(&id));
    scalar(&app, id, 1234.);
    let editor = app.editors.get_mut(&id).unwrap();
    editor.place_caret(editor.root(), Direction::Right).unwrap();
    frame(&mut app, &ctx, vec![enter()]);
    scalar(&app, id, 46.);
    assert!(!app.uncommitted_math.contains(&id));
    app.undo();
    assert_eq!(
        app.editors[&id].to_portable().unwrap(),
        eval(number("1234"))
    );
    app.redo();
    assert_eq!(
        app.document.regions[0].content,
        CanvasContent::Math(changed)
    );
    scalar(&app, id, 46.);
}

#[test]
fn actual_partial_operator_edit_updates_dependent_regions_only_after_enter() {
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    let mut app = WorksheetApp::default();
    let name = PortableTree {
        kind: NodeKind::Identifier("a".into()),
        children: vec![],
    };
    let definition = add(
        &mut app,
        20.,
        binary(BinaryOp::Define, name.clone(), number("1234")),
    );
    let query = add(
        &mut app,
        220.,
        eval(binary(BinaryOp::Multiply, name.clone(), number("2"))),
    );
    app.recalculate();
    scalar(&app, query, 2468.);
    app.last_executed_regions.clear();
    app.selected = Some(definition);
    app.editing_region = Some(definition);
    app.focus_new = Some(definition);
    frame(&mut app, &ctx, vec![]);
    let editor = app.editors.get_mut(&definition).unwrap();
    let rhs = editor.arena().get(editor.root()).unwrap().children()[1];
    editor.place_text_selection(rhs, 1, 3).unwrap();
    frame(&mut app, &ctx, vec![egui::Event::Text("+".into())]);
    assert_eq!(
        app.editors[&definition].to_portable().unwrap(),
        binary(
            BinaryOp::Define,
            name,
            binary(BinaryOp::Add, number("1"), number("4"))
        )
    );
    assert!(app.last_executed_regions.is_empty());
    scalar(&app, query, 2468.);
    let editor = app.editors.get_mut(&definition).unwrap();
    editor.place_caret(editor.root(), Direction::Right).unwrap();
    frame(&mut app, &ctx, vec![enter()]);
    scalar(&app, query, 10.);
}

#[test]
fn actual_control_minus_exit_allows_an_operator_after_the_complete_literal_name() {
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    let mut app = WorksheetApp::default();
    let id = add(
        &mut app,
        20.,
        PortableTree {
            kind: NodeKind::Identifier("I".into()),
            children: vec![],
        },
    );
    app.selected = Some(id);
    app.editing_region = Some(id);
    app.focus_new = Some(id);
    frame(&mut app, &ctx, vec![]);
    let editor = app.editors.get_mut(&id).unwrap();
    editor.place_text_caret(editor.root(), 1).unwrap();
    let zoom = app.zoom;
    let modifiers = egui::Modifiers {
        ctrl: true,
        command: true,
        ..Default::default()
    };
    let minus = || egui::Event::Key {
        key: egui::Key::Minus,
        physical_key: Some(egui::Key::Minus),
        pressed: true,
        repeat: false,
        modifiers,
    };
    frame(&mut app, &ctx, vec![minus()]);
    frame(&mut app, &ctx, vec![egui::Event::Text("dc".into())]);
    frame(&mut app, &ctx, vec![minus()]);
    frame(&mut app, &ctx, vec![egui::Event::Text("+123".into())]);
    let tree = app.editors[&id].to_portable().unwrap();
    assert_eq!(tree.kind, NodeKind::Binary(BinaryOp::Add));
    assert!(
        matches!(&tree.children[0].kind, NodeKind::NativeIdentifier { name, subscript: Some(suffix), .. } if name == "I" && suffix == "dc")
    );
    assert_eq!(tree.children[1], number("123"));
    assert_eq!(app.zoom, zoom);
    assert!(app.last_executed_regions.is_empty());
}

#[test]
#[ignore = "Explicit infix/native/PDF integration fixture; no GUI or original runtime"]
fn infix_entry_save_native_export_reopen_probe() {
    let output = PathBuf::from(
        std::env::var_os("MATHFOLIO_INFIX_ENTRY_OUTPUT").expect("MATHFOLIO_INFIX_ENTRY_OUTPUT"),
    );
    std::fs::create_dir_all(&output).unwrap();
    let mut app = WorksheetApp::default();
    let cases = [
        ("1234", 2, 2, "+", 46.),
        ("1234", 1, 3, "+", 5.),
        ("23", 1, 1, "^", 8.),
        ("24", 1, 1, "/", 0.5),
        ("23", 1, 1, "*", 6.),
        ("23", 1, 1, "-", -1.),
        ("23", 1, 1, "<", 1.),
    ];
    let ids: Vec<_> = cases
        .iter()
        .enumerate()
        .map(|(i, (source, start, end, text, _))| {
            let mut editor = Editor::from_portable(&number(source)).unwrap();
            editor
                .place_text_selection(editor.root(), *start, *end)
                .unwrap();
            assert!(crate::formula::input_text(&mut editor, text));
            add(
                &mut app,
                20. + i as f64 * 150.,
                eval(editor.to_portable().unwrap()),
            )
        })
        .collect();
    let verify = |app: &WorksheetApp| {
        assert_eq!(app.results.len(), cases.len());
        for ((_, _, _, _, expected), id) in cases.iter().zip(&ids) {
            scalar(app, *id, *expected);
        }
    };
    app.recalculate();
    verify(&app);
    app.persist_calculation_cache();
    let saved = output.join("infix-entry.mathfolio");
    let native = output.join("infix-entry.mcdx");
    app.document.save(&saved).unwrap();
    mathfolio_document::export::export_mcdx(&app.document, &native).unwrap();
    let mut reopened = WorksheetApp::default();
    reopened.open(&saved);
    assert!(reopened.last_executed_regions.is_empty() && reopened.pending_regions.is_empty());
    assert_eq!(
        reopened
            .saved_results
            .values()
            .filter(|row| row.display.is_some())
            .count(),
        cases.len()
    );
    let mut imported = WorksheetApp::default();
    imported.open(&native);
    assert_eq!(imported.document.regions.len(), cases.len());
    imported.recalculate();
    verify(&imported);
    imported.persist_calculation_cache();
    imported
        .document
        .save(&output.join("infix-entry-reimported.mathfolio"))
        .unwrap();
    println!(
        "INFIX_ENTRY_NATIVE regions=7 saved_query_displays=7 saved_reopen_execution=0 native_recalculate_errors=0"
    );
}
