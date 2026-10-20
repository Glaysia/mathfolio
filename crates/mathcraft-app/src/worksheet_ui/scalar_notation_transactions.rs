//! Scalar notation commands across commit, calculation, and file boundaries.
use super::*;

fn n(value: &str) -> PortableTree {
    PortableTree {
        kind: NodeKind::Number(value.into()),
        children: vec![],
    }
}
fn unary(kind: NodeKind, child: PortableTree) -> PortableTree {
    PortableTree {
        kind,
        children: vec![child],
    }
}
fn binary(op: BinaryOp, a: PortableTree, b: PortableTree) -> PortableTree {
    PortableTree {
        kind: NodeKind::Binary(op),
        children: vec![a, b],
    }
}
fn evaluated(tree: PortableTree) -> PortableTree {
    binary(
        BinaryOp::Evaluate,
        tree,
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
fn scalar_notation_ribbon_commands_commit_on_enter_and_undo_with_one_region() {
    for (action, kind) in [
        (RibbonAction::Percent, NodeKind::Percent),
        (RibbonAction::Degree, NodeKind::Degree),
        (RibbonAction::Polar, NodeKind::Binary(BinaryOp::Polar)),
    ] {
        let mut app = WorksheetApp::default();
        let id = add(&mut app, 20., n("2"));
        app.insertion = None;
        app.selected = Some(id);
        let editor = app.editors.get_mut(&id).unwrap();
        editor.place_caret(editor.root(), Direction::Right).unwrap();
        let ctx = egui::Context::default();
        crate::configure(&ctx);
        app.ribbon_command(&ctx, action);
        assert_eq!(app.editors[&id].to_portable().unwrap().kind, kind);
        assert!(app.uncommitted_math.contains(&id));
        assert!(app.last_executed_regions.is_empty());
        assert_eq!(app.undo.len(), 1);
        assert!(matches!(app.undo[0].document, DocumentHistory::Region(_)));
        if action == RibbonAction::Polar {
            app.apply_editor_command(|editor| {
                formula::input_text_with_actions(editor, "90°");
            });
        }
        tests::frame(&mut app, &ctx, vec![]);
        tests::frame(&mut app, &ctx, vec![tests::enter()]);
        assert!(!app.uncommitted_math.contains(&id));
        let CanvasContent::Math(tree) = &app.document.regions[0].content else {
            panic!()
        };
        assert_eq!(tree.kind, kind);
    }
}

#[test]
fn scalar_notation_ribbon_does_not_replace_matrix_typeover() {
    for action in [
        RibbonAction::Percent,
        RibbonAction::Degree,
        RibbonAction::Polar,
    ] {
        let mut app = WorksheetApp::default();
        let source = PortableTree {
            kind: NodeKind::Matrix {
                rows: 2,
                cols: 1,
                display: None,
            },
            children: vec![n("1"), n("2")],
        };
        let id = add(&mut app, 20., source.clone());
        app.insertion = None;
        app.selected = Some(id);
        let editor = app.editors.get_mut(&id).unwrap();
        editor.select_matrix_cells(editor.root(), 0, 1);
        app.ribbon_command(&egui::Context::default(), action);
        assert_eq!(app.editors[&id].to_portable().unwrap(), source);
        assert!(!app.dirty && app.undo.is_empty() && app.uncommitted_math.is_empty());
    }
}

#[test]
#[ignore = "Writes scalar notation fixtures to an explicit output directory"]
fn scalar_notation_save_native_export_reopen_probe() {
    let output = PathBuf::from(
        std::env::var_os("MATHCRAFT_SCALAR_NOTATION_OUTPUT")
            .expect("MATHCRAFT_SCALAR_NOTATION_OUTPUT"),
    );
    std::fs::create_dir_all(&output).unwrap();
    let mut app = WorksheetApp::default();
    let percent = add(&mut app, 20., evaluated(unary(NodeKind::Percent, n("50"))));
    let degree = add(&mut app, 130., evaluated(unary(NodeKind::Degree, n("90"))));
    let polar = add(
        &mut app,
        240.,
        evaluated(binary(
            BinaryOp::Polar,
            n("2"),
            unary(NodeKind::Degree, n("90")),
        )),
    );
    let product = add(
        &mut app,
        350.,
        evaluated(binary(
            BinaryOp::Multiply,
            n("200"),
            unary(NodeKind::Percent, n("10")),
        )),
    );
    let sine = add(
        &mut app,
        460.,
        evaluated(PortableTree {
            kind: NodeKind::Call { argument_count: 1 },
            children: vec![
                PortableTree {
                    kind: NodeKind::Identifier("sin".into()),
                    children: vec![],
                },
                unary(NodeKind::Degree, n("90")),
            ],
        }),
    );
    let volts = PortableTree {
        kind: NodeKind::NativeIdentifier {
            name: "V".into(),
            labels: "UNIT".into(),
            contextual: Some(false),
            subscript: None,
        },
        children: vec![],
    };
    // The recovered scalar polar overload is unitless. Attach the physical
    // unit to the completed phasor rather than widening that overload.
    let physical = add(
        &mut app,
        570.,
        evaluated(binary(
            BinaryOp::Scale,
            binary(BinaryOp::Polar, n("2"), unary(NodeKind::Degree, n("90"))),
            volts,
        )),
    );
    let matrix = add(
        &mut app,
        680.,
        evaluated(unary(
            NodeKind::Percent,
            PortableTree {
                kind: NodeKind::Matrix {
                    rows: 2,
                    cols: 2,
                    display: None,
                },
                children: ["10", "20", "30", "40"].into_iter().map(n).collect(),
            },
        )),
    );
    let verify = |app: &WorksheetApp| {
        assert_eq!(app.results.len(), 7);
        assert!(
            app.results.values().all(|r| r.error.is_none()),
            "{:?}",
            app.results
        );
        for (id, re, im) in [
            (percent, 0.5, 0.),
            (degree, std::f64::consts::FRAC_PI_2, 0.),
            (polar, 0., 2.),
            (product, 20., 0.),
            (sine, 1., 0.),
            (physical, 0., 2.),
        ] {
            let Some(Value::Scalar { quantity }) = &app.results[&id].value else {
                panic!("{:?}", app.results[&id])
            };
            assert!(
                (quantity.value.re - re).abs() < 1e-12 && (quantity.value.im - im).abs() < 1e-12,
                "{:?}",
                quantity
            );
            assert_eq!(
                quantity.dimension,
                if id == physical {
                    mathcraft_core::unit("V").unwrap().dimension
                } else {
                    [0.; 7]
                }
            );
        }
        let Some(Value::Matrix {
            rows,
            cols,
            elements,
        }) = &app.results[&matrix].value
        else {
            panic!()
        };
        assert_eq!((*rows, *cols), (2, 2));
        for (q, expected) in elements.iter().zip([0.1, 0.2, 0.3, 0.4]) {
            assert!((q.value.re - expected).abs() < 1e-12);
        }
    };
    app.recalculate();
    verify(&app);
    app.persist_calculation_cache();
    let saved = output.join("scalar-notation.mathcraft");
    let native = output.join("scalar-notation.mcdx");
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
        7
    );
    let mut imported = WorksheetApp::default();
    imported.open(&native);
    assert_eq!(imported.document.regions.len(), 7);
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
        .save(&output.join("scalar-notation-reimported.mathcraft"))
        .unwrap();
    println!(
        "SCALAR_NOTATION_NATIVE regions=7 saved_query_displays=7 saved_reopen_execution=0 native_recalculate_errors=0"
    );
}
