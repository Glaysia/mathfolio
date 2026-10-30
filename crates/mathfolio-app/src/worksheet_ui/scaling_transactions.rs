//! Original SCALE input, Ctrl+J operand preservation and native persistence.
use super::*;

fn node(kind: NodeKind, children: Vec<PortableTree>) -> PortableTree {
    PortableTree { kind, children }
}
fn number(n: i32) -> PortableTree {
    node(NodeKind::Number(n.to_string()), vec![])
}
fn name(n: &str) -> PortableTree {
    node(NodeKind::Identifier(n.into()), vec![])
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
fn program(lines: Vec<PortableTree>) -> PortableTree {
    node(
        NodeKind::Program {
            statement_count: lines.len(),
        },
        lines,
    )
}
fn local(n: &str, value: PortableTree) -> PortableTree {
    node(NodeKind::LocalDefine, vec![name(n), value])
}
fn key(key: egui::Key, modifiers: egui::Modifiers) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers,
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
fn add(app: &mut WorksheetApp, y: f64, tree: PortableTree) -> u64 {
    let id = app.document.add_math(20., y);
    app.document.regions.last_mut().unwrap().content = CanvasContent::Math(tree.clone());
    app.editors
        .insert(id, Editor::from_portable(&tree).unwrap());
    id
}
fn fixture(tree: PortableTree) -> (WorksheetApp, egui::Context, u64) {
    let mut app = WorksheetApp::default();
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    let id = add(&mut app, 20., tree);
    app.selected = Some(id);
    app.editing_region = Some(id);
    app.focus_new = Some(id);
    frame(&mut app, &ctx, vec![]);
    (app, ctx, id)
}
fn scalar(app: &WorksheetApp, id: u64, expected: f64) -> mathfolio_core::Quantity {
    let row = &app.results[&id];
    assert!(row.error.is_none(), "{row:?}");
    let Some(Value::Scalar { quantity }) = &row.value else {
        panic!("{row:?}")
    };
    assert_eq!(quantity.value.re, expected);
    assert_eq!(quantity.value.im, 0.);
    *quantity
}
fn enter_boundary(app: &mut WorksheetApp, ctx: &egui::Context, id: u64) {
    let editor = app.editors.get_mut(&id).unwrap();
    editor.place_caret(editor.root(), Direction::Right).unwrap();
    frame(app, ctx, vec![key(egui::Key::Enter, egui::Modifiers::NONE)]);
}

#[test]
fn automatic_number_unit_scale_keeps_committed_result_until_enter() {
    let (mut app, ctx, id) = fixture(eval(number(5)));
    app.recalculate();
    assert!(scalar(&app, id, 5.).is_scalar());
    app.last_executed_regions.clear();
    let editor = app.editors.get_mut(&id).unwrap();
    let operand = editor.arena().get(editor.root()).unwrap().children()[0];
    editor.place_caret(operand, Direction::Right).unwrap();
    frame(&mut app, &ctx, vec![egui::Event::Text("m".into())]);
    assert_eq!(
        app.editors[&id].to_portable().unwrap(),
        eval(binary(BinaryOp::Scale, number(5), name("m")))
    );
    assert!(app.last_executed_regions.is_empty());
    assert!(scalar(&app, id, 5.).is_scalar());
    enter_boundary(&mut app, &ctx, id);
    assert_eq!(
        scalar(&app, id, 5.).dimension,
        mathfolio_core::unit("m").unwrap().dimension
    );
}

#[test]
fn actual_ctrl_shift_u_and_function_rhs_commit_as_a_named_call() {
    let (mut app, ctx, id) = fixture(eval(number(0)));
    app.recalculate();
    scalar(&app, id, 0.);
    app.last_executed_regions.clear();
    let editor = app.editors.get_mut(&id).unwrap();
    let operand = editor.arena().get(editor.root()).unwrap().children()[0];
    editor.place_caret(operand, Direction::Right).unwrap();
    frame(
        &mut app,
        &ctx,
        vec![key(
            egui::Key::U,
            egui::Modifiers {
                shift: true,
                ..egui::Modifiers::COMMAND
            },
        )],
    );
    frame(&mut app, &ctx, vec![egui::Event::Text("cos".into())]);
    assert_eq!(
        app.editors[&id].to_portable().unwrap(),
        eval(binary(BinaryOp::Scale, number(0), name("cos")))
    );
    assert!(app.last_executed_regions.is_empty());
    scalar(&app, id, 0.);
    enter_boundary(&mut app, &ctx, id);
    scalar(&app, id, 1.);
}

fn convert(
    word: &str,
    body: PortableTree,
    outer: impl FnOnce(PortableTree) -> PortableTree,
    headers: Vec<(usize, PortableTree)>,
) -> PortableTree {
    let source = outer(binary(BinaryOp::Scale, name(word), body.clone()));
    let (mut app, ctx, id) = fixture(eval(source.clone()));
    let editor = app.editors.get_mut(&id).unwrap();
    let keyword = editor
        .arena()
        .nodes()
        .find(|(_, n)| n.kind() == &NodeKind::Identifier(word.into()))
        .unwrap()
        .0;
    editor.place_caret(keyword, Direction::Right).unwrap();
    frame(
        &mut app,
        &ctx,
        vec![key(egui::Key::J, egui::Modifiers::COMMAND)],
    );
    let converted = app.editors[&id].to_portable().unwrap();
    assert_ne!(converted, eval(source.clone()));
    let primal = if word == "for" {
        2
    } else if matches!(word, "if" | "while") {
        1
    } else {
        0
    };
    let operator = app.editors[&id]
        .arena()
        .nodes()
        .find(|(_, n)| match word {
            "while" => n.kind() == &NodeKind::While,
            "for" => n.kind() == &NodeKind::For,
            "if" => matches!(n.kind(), NodeKind::If { .. }),
            "try" => n.kind() == &NodeKind::Try,
            "return" => n.kind() == &NodeKind::Return,
            _ => unreachable!(),
        })
        .unwrap()
        .0;
    let native_body = app.editors[&id].arena().get(operator).unwrap().children()[primal];
    let extracted = app.editors[&id].export_subtree(native_body).unwrap();
    assert_eq!(
        extracted,
        if word == "return" || matches!(body.kind, NodeKind::Program { .. }) {
            body
        } else {
            program(vec![body])
        }
    );
    assert!(app.last_executed_regions.is_empty());
    app.undo();
    assert_eq!(app.editors[&id].to_portable().unwrap(), eval(source));
    app.redo();
    assert_eq!(app.editors[&id].to_portable().unwrap(), converted);
    for (index, value) in headers {
        let editor = app.editors.get_mut(&id).unwrap();
        let child = editor.arena().get(operator).unwrap().children()[index];
        editor.select_subtree(child).unwrap();
        app.apply_editor_command(|editor| editor.replace_selection(&value).unwrap());
    }
    app.editors[&id].to_portable().unwrap().children.remove(0)
}

fn control_queries() -> Vec<PortableTree> {
    vec![
        convert(
            "while",
            program(vec![
                local("n", binary(BinaryOp::Add, name("n"), number(1))),
                name("n"),
            ]),
            |value| program(vec![local("n", number(0)), value]),
            vec![(0, binary(BinaryOp::LessThan, name("n"), number(3)))],
        ),
        convert(
            "for",
            program(vec![
                local("total", binary(BinaryOp::Add, name("total"), name("i"))),
                name("total"),
            ]),
            |value| program(vec![local("total", number(0)), value]),
            vec![
                (0, name("i")),
                (
                    1,
                    node(
                        NodeKind::Range { has_step: false },
                        vec![number(1), number(3)],
                    ),
                ),
            ],
        ),
        convert(
            "if",
            number(17),
            |value| program(vec![value]),
            vec![(0, number(1))],
        ),
        convert(
            "try",
            binary(BinaryOp::InlineDivide, number(1), number(0)),
            |value| program(vec![value]),
            vec![(1, program(vec![number(43)]))],
        ),
        convert(
            "return",
            binary(BinaryOp::Scale, number(73), name("V")),
            |value| program(vec![value, number(999)]),
            vec![],
        ),
    ]
}

#[test]
fn actual_ctrl_j_preserves_implicit_rhs_for_five_controls_and_one_undo() {
    let mut app = WorksheetApp::default();
    let ids: Vec<_> = control_queries()
        .into_iter()
        .enumerate()
        .map(|(i, tree)| add(&mut app, 20. + i as f64 * 300., eval(tree)))
        .collect();
    app.recalculate();
    for (id, expected) in ids.iter().zip([3., 6., 17., 43., 73.]) {
        scalar(&app, *id, expected);
    }
    assert_eq!(
        scalar(&app, ids[4], 73.).dimension,
        mathfolio_core::unit("V").unwrap().dimension
    );
}

#[test]
#[ignore = "Explicit SCALE/native/PDF fixture; no GUI or original runtime"]
fn implicit_scale_save_native_export_reopen_probe() {
    let output = PathBuf::from(
        std::env::var_os("MATHFOLIO_IMPLICIT_SCALE_OUTPUT")
            .expect("MATHFOLIO_IMPLICIT_SCALE_OUTPUT"),
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
            binary(BinaryOp::Add, name("x"), number(1)),
        ),
    );
    let mut queries = vec![
        binary(BinaryOp::Scale, number(3), name("m")),
        binary(BinaryOp::Scale, number(0), name("cos")),
        binary(BinaryOp::Scale, number(4), name("f")),
    ];
    queries.extend(control_queries());
    let ids: Vec<_> = queries
        .into_iter()
        .enumerate()
        .map(|(i, tree)| add(&mut app, 220. + i as f64 * 300., eval(tree)))
        .collect();
    let verify = |app: &WorksheetApp| {
        assert_eq!(app.results.len(), 9);
        assert!(
            app.results.values().all(|row| row.error.is_none()),
            "{:?}",
            app.results
        );
        for (id, expected) in ids.iter().zip([3., 1., 5., 3., 6., 17., 43., 73.]) {
            scalar(app, *id, expected);
        }
        assert_eq!(
            scalar(app, ids[0], 3.).dimension,
            mathfolio_core::unit("m").unwrap().dimension
        );
        assert_eq!(
            scalar(app, ids[7], 73.).dimension,
            mathfolio_core::unit("V").unwrap().dimension
        );
    };
    app.recalculate();
    verify(&app);
    app.persist_calculation_cache();
    let saved = output.join("implicit-scale.mathfolio");
    let native = output.join("implicit-scale.mcdx");
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
        8
    );
    let mut imported = WorksheetApp::default();
    imported.open(&native);
    assert_eq!(imported.document.regions.len(), 9);
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
        .save(&output.join("implicit-scale-reimported.mathfolio"))
        .unwrap();
    println!(
        "IMPLICIT_SCALE_NATIVE regions=9 saved_query_displays=8 saved_reopen_execution=0 native_recalculate_errors=0"
    );
}
