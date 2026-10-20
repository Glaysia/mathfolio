//! App boundaries for global definitions and explicit vector operations.
use super::*;

fn number(value: &str) -> PortableTree {
    PortableTree {
        kind: NodeKind::Number(value.into()),
        children: vec![],
    }
}
fn variable(name: &str) -> PortableTree {
    PortableTree {
        kind: NodeKind::Identifier(name.into()),
        children: vec![],
    }
}
fn binary(op: BinaryOp, a: PortableTree, b: PortableTree) -> PortableTree {
    PortableTree {
        kind: NodeKind::Binary(op),
        children: vec![a, b],
    }
}
fn evaluation(value: PortableTree) -> PortableTree {
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

#[test]
fn global_ribbon_definition_waits_for_enter_then_updates_the_query_above_it() {
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    let mut app = WorksheetApp::default();
    let query = add(&mut app, 20., evaluation(variable("glob")));
    let definition = add(&mut app, 200., variable("glob"));
    app.insertion = None;
    app.recalculate();
    assert!(app.results[&query].error.is_some());
    app.last_executed_regions.clear();
    app.selected = Some(definition);
    let editor = app.editors.get_mut(&definition).unwrap();
    editor.place_caret(editor.root(), Direction::Right).unwrap();
    app.ribbon_command(&ctx, RibbonAction::GlobalDefine);
    app.apply_editor_command(|editor| {
        formula::input_text_with_actions(editor, "5");
    });
    assert_eq!(
        app.editors[&definition].to_portable().unwrap().kind,
        NodeKind::Binary(BinaryOp::GlobalDefine)
    );
    assert!(app.uncommitted_math.contains(&definition));
    assert!(app.last_executed_regions.is_empty());
    assert!(app.results[&query].error.is_some());
    assert!(
        app.undo
            .iter()
            .all(|entry| matches!(entry.document, DocumentHistory::Region(_)))
    );
    tests::frame(&mut app, &ctx, vec![]);
    tests::frame(&mut app, &ctx, vec![tests::enter()]);
    assert!(app.uncommitted_math.is_empty());
    assert!(
        matches!(&app.results[&query].value, Some(Value::Scalar {quantity}) if quantity.value.re == 5.)
    );
    assert!(!expects_display_result(&app.document.regions[1]));
    assert!(calculatable(&app.document.regions[1]));

    // Global definitions remain part of the prepass when below the visible
    // prefix targeted by Calculate Now.
    app.visible_calculation_end = Some(query);
    app.visible_regions.clear();
    app.visible_regions.insert(query);
    app.calculate_command(CalculationCommand::Now);
    assert!(
        matches!(&app.results[&query].value, Some(Value::Scalar {quantity}) if quantity.value.re == 5.)
    );
}

#[test]
fn invalid_global_draft_poison_retains_its_global_identity_instead_of_old_value() {
    let mut app = WorksheetApp::default();
    let query = add(&mut app, 20., evaluation(variable("glob")));
    let definition = add(
        &mut app,
        200.,
        binary(BinaryOp::GlobalDefine, variable("glob"), number("5")),
    );
    let independent = add(
        &mut app,
        300.,
        evaluation(binary(BinaryOp::Add, number("2"), number("3"))),
    );
    app.insertion = None;
    app.recalculate();
    assert!(
        matches!(&app.results[&query].value, Some(Value::Scalar {quantity}) if quantity.value.re == 5.)
    );
    let editor = app.editors.get_mut(&definition).unwrap();
    let rhs = editor.arena().get(editor.root()).unwrap().children()[1];
    editor
        .set_atom(rhs, NodeKind::Number("NaN".into()))
        .unwrap();
    app.recalculate();
    assert!(app.results[&query].value.is_none());
    let fault = app.results[&query].error.as_ref().unwrap();
    assert_eq!(
        fault.previous_region.as_deref(),
        Some(definition.to_string().as_str())
    );
    assert!(app.results[&definition].error.is_some());
    assert!(
        matches!(&app.results[&independent].value, Some(Value::Scalar {quantity}) if quantity.value.re == 5.)
    );
    let CanvasContent::Math(tree) = &app.document.regions[1].content else {
        panic!()
    };
    let fault = fault_statement(tree);
    assert!(fault.contains("<ml:globalDefine") && !fault.contains("<ml:define"));
}

#[test]
fn new_vector_and_global_commands_do_not_replace_matrix_cell_typeover() {
    for action in [
        RibbonAction::GlobalDefine,
        RibbonAction::Vectorize,
        RibbonAction::CrossProduct,
    ] {
        let mut app = WorksheetApp::default();
        let tree = PortableTree {
            kind: NodeKind::Matrix {
                rows: 2,
                cols: 1,
                display: None,
            },
            children: vec![number("1"), number("2")],
        };
        let id = add(&mut app, 20., tree.clone());
        app.selected = Some(id);
        app.insertion = None;
        let editor = app.editors.get_mut(&id).unwrap();
        editor.select_matrix_cells(editor.root(), 0, 1);
        app.ribbon_command(&egui::Context::default(), action);
        assert_eq!(app.editors[&id].to_portable().unwrap(), tree);
        assert!(app.editors[&id].matrix_selection().is_some());
        assert!(app.undo.is_empty());
        assert!(app.uncommitted_math.is_empty());
    }
}

#[test]
#[ignore = "Writes global/vector native fixtures to an explicit output directory"]
fn global_vector_save_native_export_reopen_probe() {
    let output = PathBuf::from(
        std::env::var_os("MATHCRAFT_GLOBAL_VECTOR_OUTPUT").expect("MATHCRAFT_GLOBAL_VECTOR_OUTPUT"),
    );
    std::fs::create_dir_all(&output).unwrap();
    let mut app = WorksheetApp::default();
    let query = add(&mut app, 20., evaluation(variable("gain")));
    let column = |values: &[&str]| PortableTree {
        kind: NodeKind::Matrix {
            rows: values.len(),
            cols: 1,
            display: None,
        },
        children: values.iter().map(|value| number(value)).collect(),
    };
    add(
        &mut app,
        130.,
        binary(BinaryOp::Define, variable("a"), column(&["1", "2", "3"])),
    );
    add(
        &mut app,
        240.,
        binary(BinaryOp::Define, variable("b"), column(&["4", "5", "6"])),
    );
    let product = || binary(BinaryOp::Multiply, variable("a"), variable("b"));
    let ordinary = add(&mut app, 350., evaluation(product()));
    let vectorized = |app: &mut WorksheetApp, y, expression: PortableTree| {
        let id = add(app, y, expression);
        app.selected = Some(id);
        app.insertion = None;
        let editor = app.editors.get_mut(&id).unwrap();
        editor
            .select_subtree_from(editor.root(), Direction::Right)
            .unwrap();
        app.ribbon_command(&egui::Context::default(), RibbonAction::Vectorize);
        app.apply_editor_command(|editor| {
            editor.insert_evaluation();
        });
        id
    };
    let mapped = vectorized(&mut app, 460., product());
    let cross = add(&mut app, 570., variable("a"));
    app.selected = Some(cross);
    let editor = app.editors.get_mut(&cross).unwrap();
    editor.place_caret(editor.root(), Direction::Right).unwrap();
    app.ribbon_command(&egui::Context::default(), RibbonAction::CrossProduct);
    app.apply_editor_command(|editor| {
        formula::input_text_with_actions(editor, "b");
    });
    app.apply_editor_command(|editor| {
        editor.insert_evaluation();
    });
    add(
        &mut app,
        680.,
        binary(
            BinaryOp::Define,
            PortableTree {
                kind: NodeKind::Function { parameter_count: 1 },
                children: vec![variable("f"), variable("x")],
            },
            binary(BinaryOp::Multiply, variable("x"), variable("gain")),
        ),
    );
    let called = vectorized(
        &mut app,
        790.,
        PortableTree {
            kind: NodeKind::Call { argument_count: 1 },
            children: vec![variable("f"), variable("a")],
        },
    );
    let broadcast = vectorized(
        &mut app,
        900.,
        binary(BinaryOp::Add, variable("a"), variable("gain")),
    );
    let global = add(&mut app, 1600., variable("gain"));
    app.selected = Some(global);
    let editor = app.editors.get_mut(&global).unwrap();
    editor.place_caret(editor.root(), Direction::Right).unwrap();
    app.ribbon_command(&egui::Context::default(), RibbonAction::GlobalDefine);
    app.apply_editor_command(|editor| {
        formula::input_text_with_actions(editor, "3");
    });
    let verify = |app: &WorksheetApp| {
        assert_eq!(app.results.len(), 10);
        assert!(
            app.results.values().all(|row| row.error.is_none()),
            "{:?}",
            app.results
        );
        for (id, expected) in [(query, 3.), (ordinary, 32.)] {
            assert!(
                matches!(&app.results[&id].value, Some(Value::Scalar {quantity}) if quantity.value.re == expected),
                "{:?}",
                app.results[&id]
            );
        }
        for (id, expected) in [
            (mapped, [4., 10., 18.]),
            (cross, [-3., 6., -3.]),
            (called, [3., 6., 9.]),
            (broadcast, [4., 5., 6.]),
        ] {
            let Some(Value::Matrix {
                rows,
                cols,
                elements,
            }) = &app.results[&id].value
            else {
                panic!("{:?}", app.results[&id])
            };
            assert_eq!((*rows, *cols), (3, 1));
            assert_eq!(
                elements
                    .iter()
                    .map(|value| value.value.re)
                    .collect::<Vec<_>>(),
                expected
            );
        }
    };
    app.recalculate();
    verify(&app);
    app.persist_calculation_cache();
    let saved = output.join("global-vector.mathcraft");
    let native = output.join("global-vector.mcdx");
    app.document.save(&saved).unwrap();
    mathcraft_document::export::export_mcdx(&app.document, &native).unwrap();
    let mut reopened = WorksheetApp::default();
    reopened.open(&saved);
    assert!(reopened.last_executed_regions.is_empty());
    assert!(reopened.pending_regions.is_empty(), "{}", reopened.status);
    assert_eq!(
        reopened
            .saved_results
            .values()
            .filter(|row| row.display.is_some())
            .count(),
        6
    );
    let mut imported = WorksheetApp::default();
    imported.open(&native);
    assert_eq!(imported.document.regions.len(), 10);
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
        .save(&output.join("global-vector-reimported.mathcraft"))
        .unwrap();
    println!(
        "GLOBAL_VECTOR_NATIVE regions=10 saved_query_displays=6 saved_reopen_execution=0 native_recalculate_errors=0"
    );
}
