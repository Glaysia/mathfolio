//! Program-local function authoring across commit and native persistence.
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
fn program(lines: Vec<PortableTree>) -> PortableTree {
    node(
        NodeKind::Program {
            statement_count: lines.len(),
        },
        lines,
    )
}
fn function(value: &str, parameters: &[&str]) -> PortableTree {
    node(
        NodeKind::Function {
            parameter_count: parameters.len(),
        },
        std::iter::once(name(value))
            .chain(parameters.iter().map(|parameter| name(parameter)))
            .collect(),
    )
}
fn local(value: &str, parameters: &[&str], body: PortableTree) -> PortableTree {
    node(
        NodeKind::LocalDefine,
        vec![function(value, parameters), body],
    )
}
fn call(value: &str, arguments: Vec<PortableTree>) -> PortableTree {
    let count = arguments.len();
    node(
        NodeKind::Call {
            argument_count: count,
        },
        std::iter::once(name(value)).chain(arguments).collect(),
    )
}
fn evaluation(value: PortableTree) -> PortableTree {
    binary(
        BinaryOp::Evaluate,
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
fn scalar(app: &WorksheetApp, id: u64, expected: f64) {
    let result = &app.results[&id];
    assert!(result.error.is_none(), "{result:?}");
    assert!(
        matches!(&result.value,Some(Value::Scalar {quantity}) if quantity.value.re==expected && quantity.value.im==0.),
        "{result:?}"
    );
}

#[test]
fn local_function_body_changes_wait_for_enter_and_undo_restores_calculation() {
    let tree = evaluation(program(vec![
        local(
            "square",
            &["x"],
            node(NodeKind::Power, vec![name("x"), number("2")]),
        ),
        call("square", vec![number("3")]),
    ]));
    let mut app = WorksheetApp::default();
    let id = add(&mut app, 20., tree);
    app.recalculate();
    scalar(&app, id, 9.);
    app.last_executed_regions.clear();
    app.selected = Some(id);
    app.insertion = None;
    let editor = app.editors.get_mut(&id).unwrap();
    let program_id = editor.arena().get(editor.root()).unwrap().children()[0];
    let definition = editor.arena().get(program_id).unwrap().children()[0];
    let power = editor.arena().get(definition).unwrap().children()[1];
    let exponent = editor.arena().get(power).unwrap().children()[1];
    editor.select_subtree(exponent).unwrap();
    app.apply_editor_command(|editor| {
        editor.replace_selection(&number("3")).unwrap();
    });
    assert!(app.uncommitted_math.contains(&id));
    assert!(app.last_executed_regions.is_empty());
    scalar(&app, id, 9.);
    // Enter inside a program inserts a line; commit from the equation boundary.
    let editor = app.editors.get_mut(&id).unwrap();
    editor.place_caret(editor.root(), Direction::Right).unwrap();
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    tests::frame(&mut app, &ctx, vec![]);
    tests::frame(&mut app, &ctx, vec![tests::enter()]);
    scalar(&app, id, 27.);
    app.undo();
    scalar(&app, id, 9.);
    app.redo();
    scalar(&app, id, 27.);
}

#[test]
#[ignore = "Writes local function fixtures to an explicit output directory"]
fn local_function_save_native_export_reopen_probe() {
    let output = PathBuf::from(
        std::env::var_os("MATHCRAFT_LOCAL_FUNCTION_OUTPUT")
            .expect("MATHCRAFT_LOCAL_FUNCTION_OUTPUT"),
    );
    std::fs::create_dir_all(&output).unwrap();
    let mut app = WorksheetApp::default();
    let square = add(
        &mut app,
        20.,
        evaluation(program(vec![
            local(
                "square",
                &["x"],
                node(NodeKind::Power, vec![name("x"), number("2")]),
            ),
            call("square", vec![number("3")]),
        ])),
    );
    let sum = add(
        &mut app,
        260.,
        evaluation(program(vec![
            local(
                "sum",
                &["x", "y"],
                binary(BinaryOp::Add, name("x"), name("y")),
            ),
            call("sum", vec![number("2"), number("5")]),
        ])),
    );
    let nested = add(
        &mut app,
        500.,
        evaluation(program(vec![
            local(
                "nested",
                &["x"],
                program(vec![
                    node(
                        NodeKind::LocalDefine,
                        vec![name("u"), binary(BinaryOp::Add, name("x"), number("1"))],
                    ),
                    binary(BinaryOp::Multiply, name("u"), number("2")),
                ]),
            ),
            call("nested", vec![number("3")]),
        ])),
    );
    add(
        &mut app,
        820.,
        binary(
            BinaryOp::Define,
            function("outer", &["a"]),
            program(vec![
                local(
                    "helper",
                    &["x"],
                    binary(BinaryOp::Multiply, name("a"), name("x")),
                ),
                call("helper", vec![number("2")]),
            ]),
        ),
    );
    let captured = add(
        &mut app,
        1100.,
        evaluation(call("outer", vec![number("4")])),
    );
    let physical = add(
        &mut app,
        1320.,
        evaluation(program(vec![
            local(
                "double",
                &["x"],
                binary(BinaryOp::Multiply, number("2"), name("x")),
            ),
            call(
                "double",
                vec![binary(BinaryOp::Multiply, number("3"), name("V"))],
            ),
        ])),
    );
    let verify = |app: &WorksheetApp| {
        assert_eq!(app.results.len(), 6);
        assert!(
            app.results.values().all(|result| result.error.is_none()),
            "{:?}",
            app.results
        );
        for (id, value) in [
            (square, 9.),
            (sum, 7.),
            (nested, 8.),
            (captured, 8.),
            (physical, 6.),
        ] {
            scalar(app, id, value);
        }
        let Some(Value::Scalar { quantity }) = &app.results[&physical].value else {
            unreachable!()
        };
        assert_eq!(
            quantity.dimension,
            mathcraft_core::unit("V").unwrap().dimension
        );
    };
    app.recalculate();
    verify(&app);
    app.persist_calculation_cache();
    let saved = output.join("local-functions.mathcraft");
    let native = output.join("local-functions.mcdx");
    app.document.save(&saved).unwrap();
    mathcraft_document::export::export_mcdx(&app.document, &native).unwrap();
    let mut reopened = WorksheetApp::default();
    reopened.open(&saved);
    assert!(reopened.last_executed_regions.is_empty() && reopened.pending_regions.is_empty());
    assert_eq!(
        reopened
            .saved_results
            .values()
            .filter(|result| result.display.is_some())
            .count(),
        5
    );
    let mut imported = WorksheetApp::default();
    imported.open(&native);
    assert_eq!(imported.document.regions.len(), 6);
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
        .save(&output.join("local-functions-reimported.mathcraft"))
        .unwrap();
    println!(
        "LOCAL_FUNCTIONS_NATIVE regions=6 saved_query_displays=5 saved_reopen_execution=0 native_recalculate_errors=0"
    );
}
