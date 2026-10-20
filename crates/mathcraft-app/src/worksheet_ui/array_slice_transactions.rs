//! Native array-slice definitions at the worksheet command/cache boundary.
use super::*;

fn n(value: &str) -> PortableTree {
    PortableTree {
        kind: NodeKind::Number(value.into()),
        children: vec![],
    }
}
fn var(value: &str) -> PortableTree {
    PortableTree {
        kind: NodeKind::Identifier(value.into()),
        children: vec![],
    }
}
fn node(kind: NodeKind, children: Vec<PortableTree>) -> PortableTree {
    PortableTree { kind, children }
}
fn binary(op: BinaryOp, a: PortableTree, b: PortableTree) -> PortableTree {
    node(NodeKind::Binary(op), vec![a, b])
}
fn matrix(rows: usize, cols: usize, values: &[&str]) -> PortableTree {
    node(
        NodeKind::Matrix {
            rows,
            cols,
            display: None,
        },
        values.iter().map(|value| n(value)).collect(),
    )
}
fn eval(value: PortableTree) -> PortableTree {
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
fn assert_matrix(app: &WorksheetApp, id: u64, rows: usize, cols: usize, expected: &[f64]) {
    assert!(app.results[&id].error.is_none(), "{:?}", app.results[&id]);
    let Some(Value::Matrix {
        rows: actual_rows,
        cols: actual_cols,
        elements,
    }) = &app.results[&id].value
    else {
        panic!("{:?}", app.results[&id])
    };
    assert_eq!((*actual_rows, *actual_cols), (rows, cols));
    assert_eq!(
        elements.iter().map(|q| q.value.re).collect::<Vec<_>>(),
        expected
    );
}

#[test]
fn slice_definition_is_a_draft_until_enter_and_undo_remains_region_local() {
    let mut app = WorksheetApp::default();
    add(
        &mut app,
        20.,
        binary(
            BinaryOp::Define,
            var("mat"),
            matrix(2, 2, &["1", "2", "3", "4"]),
        ),
    );
    let slice = add(
        &mut app,
        180.,
        node(NodeKind::Column, vec![var("mat"), n("0")]),
    );
    let query = add(&mut app, 300., eval(var("mat")));
    app.recalculate();
    assert_matrix(&app, query, 2, 2, &[1., 2., 3., 4.]);
    app.last_executed_regions.clear();
    app.insertion = None;
    app.selected = Some(slice);
    let editor = app.editors.get_mut(&slice).unwrap();
    editor.place_caret(editor.root(), Direction::Right).unwrap();
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    app.ribbon_command(&ctx, RibbonAction::Define);
    app.apply_editor_command(|editor| {
        editor
            .replace_selection(&matrix(2, 1, &["9", "8"]))
            .unwrap();
    });
    assert!(app.uncommitted_math.contains(&slice));
    assert!(app.last_executed_regions.is_empty());
    assert_matrix(&app, query, 2, 2, &[1., 2., 3., 4.]);
    assert_eq!(app.undo.len(), 2);
    assert!(
        app.undo
            .iter()
            .all(|entry| matches!(entry.document, DocumentHistory::Region(_)))
    );
    tests::frame(&mut app, &ctx, vec![]);
    tests::frame(&mut app, &ctx, vec![tests::enter()]);
    assert!(app.uncommitted_math.is_empty());
    assert_matrix(&app, query, 2, 2, &[9., 2., 8., 4.]);
    app.undo();
    assert!(
        app.results[&query].error.is_some(),
        "incomplete RHS must not revive an earlier binding"
    );
    app.undo();
    assert_matrix(&app, query, 2, 2, &[1., 2., 3., 4.]);
    app.redo();
    app.redo();
    assert_matrix(&app, query, 2, 2, &[9., 2., 8., 4.]);
}

#[test]
fn invalid_slice_draft_still_poison_its_named_binding_and_preserves_global_identity() {
    for kind in [NodeKind::Row, NodeKind::Column] {
        for global in [false, true] {
            let mut app = WorksheetApp::default();
            let name = if global { "globmat" } else { "mat" };
            if !global {
                add(
                    &mut app,
                    20.,
                    binary(
                        BinaryOp::Define,
                        var(name),
                        matrix(2, 2, &["1", "2", "3", "4"]),
                    ),
                );
            }
            let query = add(&mut app, 100., eval(var(name)));
            let declaration = if global {
                BinaryOp::GlobalDefine
            } else {
                BinaryOp::Define
            };
            let bad = node(
                NodeKind::Binary(declaration),
                vec![node(kind.clone(), vec![var(name), n("0")]), n("NaN")],
            );
            let id = app.document.add_math(20., if global { 1000. } else { 60. });
            app.document.regions.last_mut().unwrap().content = CanvasContent::Math(bad.clone());
            let portable_fault = fault_statement(&bad);
            assert!(portable_fault.contains(if global { "globalDefine" } else { "define" }));
            assert!(portable_fault.contains(name));
            let independent = add(&mut app, 1100., eval(binary(BinaryOp::Add, n("2"), n("3"))));
            app.recalculate();
            let error = app.results[&query]
                .error
                .as_ref()
                .expect("failed slice must not expose old matrix");
            assert_eq!(
                error.previous_region.as_deref(),
                Some(id.to_string().as_str())
            );
            assert!(
                matches!(&app.results[&independent].value,Some(Value::Scalar {quantity}) if quantity.value.re==5.)
            );
        }
    }
}

#[test]
fn raw_slice_fault_recovers_exact_base_identifier_metadata() {
    for definition in ["define", "globalDefine"] {
        for operator in ["matrow", "matcol"] {
            let raw = format!(
                "<ml:{definition} xmlns:ml='http://schemas.mathsoft.com/math50'><ml:apply><ml:{operator}/><ml:id labels='VARIABLE' label-is-contextual='false'>mat</ml:id><ml:real>0</ml:real></ml:apply><ml:unsupported/></ml:{definition}>"
            );
            let fault = raw_fault_statement(&raw);
            let xml = roxmltree::Document::parse(&fault).unwrap();
            assert_eq!(xml.root_element().tag_name().name(), definition);
            let lhs = xml.root_element().first_element_child().unwrap();
            assert_eq!(lhs.tag_name().name(), "id");
            assert_eq!(lhs.text(), Some("mat"));
            assert_eq!(lhs.attribute("labels"), Some("VARIABLE"));
            assert_eq!(lhs.attribute("label-is-contextual"), Some("false"));
        }
    }
}

#[test]
#[ignore = "Writes array slice fixtures to an explicit output directory"]
fn array_slice_save_native_export_reopen_probe() {
    let output = PathBuf::from(
        std::env::var_os("MATHCRAFT_ARRAY_SLICE_OUTPUT").expect("MATHCRAFT_ARRAY_SLICE_OUTPUT"),
    );
    std::fs::create_dir_all(&output).unwrap();
    let mut app = WorksheetApp::default();
    add(
        &mut app,
        20.,
        binary(
            BinaryOp::Define,
            var("mat"),
            matrix(2, 2, &["1", "2", "3", "4"]),
        ),
    );
    add(
        &mut app,
        100.,
        binary(BinaryOp::Define, var("oldmat"), var("mat")),
    );
    let column = add(
        &mut app,
        180.,
        node(NodeKind::Column, vec![var("mat"), n("1")]),
    );
    app.insertion = None;
    app.selected = Some(column);
    let editor = app.editors.get_mut(&column).unwrap();
    editor.place_caret(editor.root(), Direction::Right).unwrap();
    app.ribbon_command(&egui::Context::default(), RibbonAction::Define);
    app.apply_editor_command(|editor| {
        editor
            .replace_selection(&matrix(2, 1, &["9", "8"]))
            .unwrap();
    });
    let after_column = add(&mut app, 260., eval(var("mat")));
    add(
        &mut app,
        360.,
        binary(
            BinaryOp::Define,
            node(NodeKind::Row, vec![var("mat"), n("0")]),
            matrix(1, 2, &["7", "6"]),
        ),
    );
    let after_row = add(&mut app, 440., eval(var("mat")));
    let alias = add(&mut app, 540., eval(var("oldmat")));
    let global = add(&mut app, 640., eval(var("globalmat")));
    let program = add(
        &mut app,
        760.,
        eval(node(
            NodeKind::Program { statement_count: 4 },
            vec![
                node(
                    NodeKind::LocalDefine,
                    vec![var("localmat"), matrix(2, 2, &["3", "4", "5", "6"])],
                ),
                node(
                    NodeKind::LocalDefine,
                    vec![
                        node(NodeKind::Column, vec![var("localmat"), n("1")]),
                        matrix(2, 1, &["10", "20"]),
                    ],
                ),
                node(
                    NodeKind::LocalDefine,
                    vec![
                        node(NodeKind::Row, vec![var("localmat"), n("0")]),
                        matrix(1, 2, &["9", "8"]),
                    ],
                ),
                var("localmat"),
            ],
        )),
    );
    let global_definition = add(
        &mut app,
        1200.,
        node(NodeKind::Row, vec![var("globalmat"), n("0")]),
    );
    app.selected = Some(global_definition);
    let editor = app.editors.get_mut(&global_definition).unwrap();
    editor.place_caret(editor.root(), Direction::Right).unwrap();
    app.ribbon_command(&egui::Context::default(), RibbonAction::GlobalDefine);
    app.apply_editor_command(|editor| {
        editor
            .replace_selection(&matrix(1, 2, &["11", "12"]))
            .unwrap();
    });
    add(
        &mut app,
        1400.,
        binary(BinaryOp::Define, var("ORIGIN"), n("1")),
    );
    add(
        &mut app,
        1500.,
        binary(
            BinaryOp::Define,
            node(NodeKind::Column, vec![var("mat"), n("1")]),
            matrix(2, 1, &["2", "4"]),
        ),
    );
    let origin = add(&mut app, 1600., eval(var("mat")));
    let verify = |app: &WorksheetApp| {
        assert_eq!(app.results.len(), 13);
        assert!(
            app.results.values().all(|r| r.error.is_none()),
            "{:?}",
            app.results
        );
        for (id, rows, cols, expected) in [
            (after_column, 2, 2, &[1., 9., 3., 8.][..]),
            (after_row, 2, 2, &[7., 6., 3., 8.][..]),
            (alias, 2, 2, &[1., 2., 3., 4.][..]),
            (global, 1, 2, &[11., 12.][..]),
            (program, 2, 2, &[9., 8., 5., 20.][..]),
            (origin, 2, 2, &[2., 6., 4., 8.][..]),
        ] {
            assert_matrix(app, id, rows, cols, expected);
        }
    };
    app.recalculate();
    verify(&app);
    app.persist_calculation_cache();
    let saved = output.join("array-slices.mathcraft");
    let native = output.join("array-slices.mcdx");
    app.document.save(&saved).unwrap();
    mathcraft_document::export::export_mcdx(&app.document, &native).unwrap();
    let mut reopened = WorksheetApp::default();
    reopened.open(&saved);
    assert!(reopened.last_executed_regions.is_empty() && reopened.pending_regions.is_empty());
    assert_eq!(
        reopened
            .saved_results
            .values()
            .filter(|r| r.display.is_some())
            .count(),
        6
    );
    let mut imported = WorksheetApp::default();
    imported.open(&native);
    assert_eq!(imported.document.regions.len(), 13);
    assert!(
        imported
            .document
            .regions
            .iter()
            .all(|r| matches!(r.content, CanvasContent::Math(_)))
    );
    imported.recalculate();
    verify(&imported);
    imported.persist_calculation_cache();
    imported
        .document
        .save(&output.join("array-slices-reimported.mathcraft"))
        .unwrap();
    println!(
        "ARRAY_SLICES_NATIVE regions=13 saved_query_displays=6 saved_reopen_execution=0 native_recalculate_errors=0"
    );
}
