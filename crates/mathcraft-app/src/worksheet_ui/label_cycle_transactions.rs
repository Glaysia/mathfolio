//! Original label-cycle state across calculation and native persistence.
use super::*;

fn atom(kind: NodeKind) -> PortableTree {
    PortableTree {
        kind,
        children: vec![],
    }
}
fn identifier(name: &str, label: &str) -> PortableTree {
    atom(NodeKind::AnnotatedIdentifier {
        name: name.into(),
        labels: label.into(),
    })
}
fn binary(op: BinaryOp, left: PortableTree, right: PortableTree) -> PortableTree {
    PortableTree {
        kind: NodeKind::Binary(op),
        children: vec![left, right],
    }
}
fn evaluation(value: PortableTree) -> PortableTree {
    binary(BinaryOp::Evaluate, value, atom(NodeKind::Placeholder))
}
fn no_label(name: &str, suffix: Option<&str>) -> PortableTree {
    let tree = match suffix {
        Some(text) => atom(NodeKind::NativeIdentifier {
            name: name.into(),
            labels: "KEYWORD".into(),
            contextual: Some(false),
            subscript: Some(text.into()),
        }),
        None => identifier(name, "KEYWORD"),
    };
    let mut editor = Editor::from_portable(&tree).unwrap();
    assert!(editor.cycle_identifier_label());
    editor.to_portable().unwrap()
}
fn add(app: &mut WorksheetApp, tree: PortableTree) -> u64 {
    let id = app
        .document
        .add_math(20., app.document.regions.len() as f64 * 100. + 20.);
    app.document.regions.last_mut().unwrap().content = CanvasContent::Math(tree);
    id
}

#[test]
#[ignore = "Writes label-cycle native fixtures to an explicit output directory"]
fn label_cycle_save_native_export_reopen_probe() {
    let output = PathBuf::from(
        std::env::var_os("MATHCRAFT_LABEL_CYCLE_OUTPUT").expect("MATHCRAFT_LABEL_CYCLE_OUTPUT"),
    );
    std::fs::create_dir_all(&output).unwrap();
    let mut app = WorksheetApp::default();
    add(
        &mut app,
        binary(
            BinaryOp::Define,
            identifier("m", "VARIABLE"),
            atom(NodeKind::Number("7".into())),
        ),
    );
    let variable = add(&mut app, evaluation(no_label("m", None)));
    let physical = add(&mut app, evaluation(identifier("m", "UNIT")));
    let sine = add(
        &mut app,
        evaluation(PortableTree {
            kind: NodeKind::Call { argument_count: 1 },
            children: vec![no_label("sin", None), atom(NodeKind::Number("0".into()))],
        }),
    );
    let system = add(&mut app, evaluation(no_label("ORIGIN", None)));
    let suffix = atom(NodeKind::NativeIdentifier {
        name: "m".into(),
        labels: "VARIABLE".into(),
        contextual: None,
        subscript: Some("dc".into()),
    });
    add(
        &mut app,
        binary(BinaryOp::Define, suffix, atom(NodeKind::Number("4".into()))),
    );
    let literal = add(&mut app, evaluation(no_label("m", Some("dc"))));
    let verify = |app: &WorksheetApp| {
        assert!(
            app.results.values().all(|row| row.error.is_none()),
            "{:?}",
            app.results
        );
        for (id, expected) in [
            (variable, 7.),
            (physical, 1.),
            (sine, 0.),
            (system, 0.),
            (literal, 4.),
        ] {
            let Some(Value::Scalar { quantity }) = &app.results[&id].value else {
                panic!("{:?}", app.results[&id].value)
            };
            assert_eq!(quantity.value.re, expected);
            assert_eq!(quantity.value.im, 0.);
        }
        let Some(Value::Scalar { quantity }) = &app.results[&physical].value else {
            unreachable!()
        };
        assert_eq!(
            quantity.dimension,
            mathcraft_core::unit("m").unwrap().dimension
        );
    };
    app.recalculate();
    verify(&app);
    app.persist_calculation_cache();
    let saved = output.join("label-cycle.mathcraft");
    let native = output.join("label-cycle.mcdx");
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
        5
    );
    assert_eq!(reopened.document.regions, app.document.regions);
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
    for id in [variable, sine, system, literal] {
        let CanvasContent::Math(tree) = &imported
            .document
            .regions
            .iter()
            .find(|r| r.id == id)
            .unwrap()
            .content
        else {
            unreachable!()
        };
        let fragment = worksheet_math::tree_to_mathml(tree).unwrap();
        assert!(fragment.contains("labels=\"*\""), "{fragment}");
        assert!(
            !fragment.contains("label-is-contextual=\"true\""),
            "{fragment}"
        );
    }
    imported.recalculate();
    verify(&imported);
    imported.persist_calculation_cache();
    imported
        .document
        .save(&output.join("label-cycle-reimported.mathcraft"))
        .unwrap();
    println!(
        "LABEL_CYCLE_NATIVE regions=7 saved_query_displays=5 saved_reopen_execution=0 native_recalculate_errors=0"
    );
}
