//! Ribbon commands use the same operand and draft boundaries as key input.
use super::*;

fn number(text: &str) -> PortableTree {
    PortableTree {
        kind: NodeKind::Number(text.into()),
        children: vec![],
    }
}

fn matrix_command_app() -> (WorksheetApp, u64, u64, PortableTree) {
    let tree = PortableTree {
        kind: NodeKind::Matrix {
            rows: 2,
            cols: 2,
            display: None,
        },
        children: ["1", "2", "3", "4"].map(number).to_vec(),
    };
    let mut app = WorksheetApp::default();
    let id = app.document.add_math(0., 0.);
    app.document.regions[0].content = CanvasContent::Math(tree.clone());
    let mut editor = Editor::from_portable(&tree).unwrap();
    let first = editor.arena().get(editor.root()).unwrap().children()[0];
    editor.place_caret(first, Direction::Right).unwrap();
    app.editors.insert(id, editor);
    let unrelated = app.document.add_math(0., 100.);
    app.document.regions[1].content = CanvasContent::Math(number("7"));
    app.editors
        .insert(unrelated, Editor::from_portable(&number("7")).unwrap());
    app.selected = Some(id);
    app.insertion = None;
    (app, id, unrelated, tree)
}

#[test]
fn matrix_structure_history_preserves_other_editor_and_waits_for_commit() {
    let (mut app, id, unrelated, tree) = matrix_command_app();
    app.recalculate();
    app.last_executed_regions.clear();
    app.matrix_command(RibbonAction::InsertMatrixRow);
    assert_eq!(
        app.editors[&id].to_portable().unwrap().kind,
        NodeKind::Matrix {
            rows: 3,
            cols: 2,
            display: None
        }
    );
    assert_eq!(app.undo.len(), 1);
    assert!(matches!(app.undo[0].document, DocumentHistory::Region(_)));
    assert!(app.uncommitted_math.contains(&id));
    assert!(app.last_executed_regions.is_empty());
    // Another live editor must survive a matrix command's undo/redo.
    let editor = app.editors.get_mut(&unrelated).unwrap();
    editor.place_text_caret(editor.root(), 1).unwrap();
    formula::input_text(editor, "8");
    let live = editor.to_portable().unwrap();
    app.undo();
    assert_eq!(app.editors[&id].to_portable().unwrap(), tree);
    assert_eq!(app.editors[&unrelated].to_portable().unwrap(), live);
    app.redo();
    assert_eq!(
        app.editors[&id].to_portable().unwrap().kind,
        NodeKind::Matrix {
            rows: 3,
            cols: 2,
            display: None
        }
    );
    assert_eq!(app.editors[&unrelated].to_portable().unwrap(), live);
}

#[test]
fn matrix_clear_cells_uses_the_ribbon_route_and_keeps_dimensions() {
    let (mut app, id, _, tree) = matrix_command_app();
    app.recalculate();
    app.last_executed_regions.clear();
    app.ribbon_command(&egui::Context::default(), RibbonAction::ClearCells);
    let cleared = app.editors[&id].to_portable().unwrap();
    assert_eq!(cleared.kind, tree.kind);
    assert_eq!(cleared.children[0].kind, NodeKind::Placeholder);
    assert_eq!(cleared.children[1..], tree.children[1..]);
    assert!(app.uncommitted_math.contains(&id));
    assert!(app.last_executed_regions.is_empty());
    assert_eq!(app.undo.len(), 1);
    app.undo();
    assert_eq!(app.editors[&id].to_portable().unwrap(), tree);
}

#[test]
fn basic_ribbon_input_history_preserves_other_live_editors() {
    for action in [
        RibbonAction::Define,
        RibbonAction::Evaluate,
        RibbonAction::InsertSymbol("+"),
    ] {
        let (mut app, id, unrelated, original) = matrix_command_app();
        app.ribbon_command(&egui::Context::default(), action);
        let changed = app.editors[&id].to_portable().unwrap();
        assert_ne!(changed, original);
        assert_eq!(app.undo.len(), 1);
        assert!(matches!(app.undo[0].document, DocumentHistory::Region(_)));
        let editor = app.editors.get_mut(&unrelated).unwrap();
        editor.place_text_caret(editor.root(), 1).unwrap();
        formula::input_text(editor, "8");
        let live = editor.to_portable().unwrap();
        app.undo();
        assert_eq!(app.editors[&id].to_portable().unwrap(), original);
        assert_eq!(app.editors[&unrelated].to_portable().unwrap(), live);
        app.redo();
        assert_eq!(app.editors[&id].to_portable().unwrap(), changed);
        assert_eq!(app.editors[&unrelated].to_portable().unwrap(), live);
    }
}

#[test]
fn selected_ribbon_target_owns_history_even_with_an_older_focus_request() {
    for action in [
        RibbonAction::Define,
        RibbonAction::Evaluate,
        RibbonAction::InsertSymbol("+"),
        RibbonAction::Factorial,
    ] {
        let (mut app, id, unrelated, original) = matrix_command_app();
        app.focus_new = Some(unrelated);
        app.ribbon_command(&egui::Context::default(), action);
        assert!(
            matches!(&app.undo[0].document, DocumentHistory::Region(region) if region.id == id)
        );
        let changed = app.editors[&id].to_portable().unwrap();
        app.undo();
        assert_eq!(
            app.document.regions[0].content,
            CanvasContent::Math(original)
        );
        app.redo();
        assert_eq!(
            app.document.regions[0].content,
            CanvasContent::Math(changed)
        );
    }
}

#[test]
fn matrix_range_ribbon_clear_and_delete_have_distinct_shape_and_history() {
    let (mut app, id, _, tree) = matrix_command_app();
    let editor = app.editors.get_mut(&id).unwrap();
    assert!(editor.select_matrix_cells(editor.root(), 0, 1));
    app.matrix_command(RibbonAction::ClearCells);
    let cleared = app.editors[&id].to_portable().unwrap();
    assert_eq!(cleared.kind, tree.kind);
    assert!(
        cleared.children[..2]
            .iter()
            .all(|cell| cell.kind == NodeKind::Placeholder)
    );
    assert_eq!(cleared.children[2..], tree.children[2..]);
    app.undo();
    let editor = app.editors.get_mut(&id).unwrap();
    assert!(editor.select_matrix_cells(editor.root(), 0, 1));
    app.matrix_command(RibbonAction::DeleteMatrixRow);
    let deleted = app.editors[&id].to_portable().unwrap();
    assert_eq!(
        deleted.kind,
        NodeKind::Matrix {
            rows: 1,
            cols: 2,
            display: None
        }
    );
    assert_eq!(deleted.children, tree.children[2..]);
    assert!(matches!(
        app.undo.last().unwrap().document,
        DocumentHistory::Region(_)
    ));
    app.undo();
    assert_eq!(app.editors[&id].to_portable().unwrap(), tree);
}

#[test]
fn local_math_format_history_keeps_unrelated_drafts() {
    let (mut app, id, unrelated, _) = matrix_command_app();
    let original_format = app.document.regions[0].format;
    app.change_math_format(|format| format.font_points = 24.);
    assert_eq!(app.undo.len(), 1);
    assert!(matches!(app.undo[0].document, DocumentHistory::Region(_)));
    let editor = app.editors.get_mut(&unrelated).unwrap();
    editor.place_text_caret(editor.root(), 1).unwrap();
    formula::input_text(editor, "8");
    let live = editor.to_portable().unwrap();
    app.undo();
    assert_eq!(app.document.regions[0].format, original_format);
    assert_eq!(app.editors[&unrelated].to_portable().unwrap(), live);
    app.redo();
    assert_eq!(app.document.regions[0].format.font_points, 24.);
    assert_eq!(app.editors[&unrelated].to_portable().unwrap(), live);
    assert_eq!(app.selected, Some(id));
}

fn structural_operator_cases() -> [(RibbonAction, NodeKind); 7] {
    [
        (RibbonAction::Factorial, NodeKind::Factorial),
        (RibbonAction::Transpose, NodeKind::Transpose),
        (RibbonAction::Conjugate, NodeKind::Conjugate),
        (RibbonAction::Absolute, NodeKind::Absolute),
        (RibbonAction::Norm, NodeKind::Determinant),
        (RibbonAction::RowOperator, NodeKind::Row),
        (RibbonAction::ColumnOperator, NodeKind::Column),
    ]
}

#[test]
fn structural_operator_ribbon_preserves_operand_native_structure_and_draft_history() {
    for (action, expected) in structural_operator_cases() {
        let (mut app, id, _, original) = matrix_command_app();
        let editor = app.editors.get_mut(&id).unwrap();
        editor.select_subtree(editor.root()).unwrap();
        app.ribbon_command(&egui::Context::default(), action);
        let tree = app.editors[&id].to_portable().unwrap();
        assert_eq!(tree.kind, expected);
        assert_eq!(tree.children[0], original);
        if matches!(expected, NodeKind::Row | NodeKind::Column) {
            assert_eq!(tree.children[1].kind, NodeKind::Placeholder);
        }
        assert!(app.uncommitted_math.contains(&id));
        assert!(app.last_executed_regions.is_empty());
        assert_eq!(app.undo.len(), 1);
        assert!(matches!(app.undo[0].document, DocumentHistory::Region(_)));
        let native = worksheet_math::tree_to_mathml(&tree).unwrap();
        assert_eq!(worksheet_math::tree_from_mathml(&native).unwrap(), tree);
        app.undo();
        assert_eq!(app.editors[&id].to_portable().unwrap(), original);
        app.redo();
        assert_eq!(app.editors[&id].to_portable().unwrap(), tree);
    }
}

#[test]
fn structural_operator_ribbon_respects_matrix_cell_typeover_guards() {
    for (action, _) in structural_operator_cases() {
        let (mut app, id, _, original) = matrix_command_app();
        let editor = app.editors.get_mut(&id).unwrap();
        assert!(editor.select_matrix_cells(editor.root(), 0, 3));
        app.ribbon_command(&egui::Context::default(), action);
        assert_eq!(app.editors[&id].to_portable().unwrap(), original);
        assert!(app.editors[&id].matrix_selection().is_some());
        assert!(app.undo.is_empty());
        assert!(!app.dirty);
        assert!(app.uncommitted_math.is_empty());
        assert!(app.last_executed_regions.is_empty());
    }
}

#[test]
fn ribbon_index_keeps_the_operand_and_literal_toggle_does_not_commit_a_change() {
    let mut app = WorksheetApp::default();
    let id = app.document.add_math(0., 0.);
    let tree = PortableTree {
        kind: NodeKind::NativeIdentifier {
            name: "voltage".into(),
            labels: "VARIABLE".into(),
            contextual: None,
            subscript: Some("rms".into()),
        },
        children: vec![],
    };
    app.document.regions[0].content = CanvasContent::Math(tree.clone());
    let mut editor = Editor::from_portable(&tree).unwrap();
    editor.move_to_expression_extreme(true);
    app.editors.insert(id, editor);
    app.selected = Some(id);
    app.insertion = None;
    app.apply_editor_command(|editor| {
        editor.toggle_literal_subscript();
    });
    assert!(app.undo.is_empty());
    assert!(!app.dirty);
    assert!(app.uncommitted_math.is_empty());
    app.apply_editor_command(|editor| {
        editor.toggle_literal_subscript();
    });
    app.insert_template(PortableTree {
        kind: NodeKind::Index { indices: 1 },
        children: vec![number("0"), number("0")],
    });
    let indexed = app.editors[&id].to_portable().unwrap();
    assert_eq!(indexed.kind, NodeKind::Index { indices: 1 });
    assert_eq!(indexed.children[0], tree);
    assert_eq!(indexed.children[1].kind, NodeKind::Placeholder);
    assert_eq!(app.undo.len(), 1);
    assert!(matches!(app.undo[0].document, DocumentHistory::Region(_)));
}

#[test]
fn inserting_a_template_from_a_text_selection_undoes_the_new_region() {
    let mut app = WorksheetApp::default();
    let id = app.document.add_text(0., 0., "keep");
    app.selected = Some(id);
    app.insertion = None;
    app.insert_atom(NodeKind::Number("7".into()));
    assert_eq!(app.document.regions.len(), 2);
    assert!(matches!(app.undo[0].document, DocumentHistory::Full(_)));
    app.undo();
    assert_eq!(app.document.regions.len(), 1);
    assert_eq!(app.document.regions[0].id, id);
}

#[test]
fn table_ribbon_templates_atoms_and_labels_stay_in_the_cell_draft() {
    let mut app = WorksheetApp::default();
    app.insert_spec_table();
    let id = app.selected.unwrap();
    let before = app.document.regions[0].content.clone();
    let history = app.undo.len();
    app.insert_atom(NodeKind::Identifier("length".into()));
    app.set_label("VARIABLE");
    app.apply_editor_command(|editor| {
        editor.toggle_literal_subscript();
    });
    app.apply_editor_command(|editor| {
        formula::input_text_with_actions(editor, "nom");
    });
    let tree = app.table_editors[&id]
        .active_editor()
        .unwrap()
        .to_portable()
        .unwrap();
    assert!(
        matches!(tree.kind, NodeKind::NativeIdentifier { subscript: Some(ref suffix), .. } if suffix == "nom")
    );
    assert_eq!(app.document.regions.len(), 1);
    assert_eq!(app.document.regions[0].content, before);
    assert_eq!(app.undo.len(), history);
    app.commit_table_edits(None).unwrap();
    assert_ne!(app.document.regions[0].content, before);
    assert_eq!(app.undo.len(), history + 1);
}

#[test]
fn table_region_clipboard_preserves_structure_with_new_identity_and_undo() {
    let mut app = WorksheetApp::default();
    app.insert_spec_table();
    let original = app.selected.unwrap();
    app.document.regions[0].format.font_points = 23.;
    let ctx = egui::Context::default();
    let copied = ctx.run_ui(egui::RawInput::default(), |_ui| {
        app.copy_region(&ctx, false)
    });
    let text = copied
        .platform_output
        .commands
        .iter()
        .find_map(|command| {
            if let egui::OutputCommand::CopyText(text) = command {
                Some(text.clone())
            } else {
                None
            }
        })
        .unwrap();
    copied.drop_without_applying_deltas();
    let history = app.undo.len();
    assert!(app.paste_region(&text));
    let pasted = app.selected.unwrap();
    assert_ne!(original, pasted);
    assert_eq!(app.document.regions.len(), 2);
    assert_eq!(
        app.document.regions[0].content,
        app.document.regions[1].content
    );
    assert_eq!(app.document.regions[1].format.font_points, 23.);
    assert!(
        app.native_visuals
            .region(&app.document.regions[1])
            .unwrap()
            .is_ok()
    );
    assert!(app.document.regions[1].source_region.is_none());
    assert_eq!(app.undo.len(), history + 1);
    app.undo();
    assert_eq!(app.document.regions.len(), 1);
    app.redo();
    assert_eq!(app.document.regions.len(), 2);
    assert!(
        app.native_visuals
            .region(&app.document.regions[1])
            .unwrap()
            .is_ok()
    );
    app.selected = Some(pasted);
    let removed = app.document.regions[1].clone();
    app.remove_selected();
    assert!(!app.table_editors.contains_key(&pasted));
    assert!(app.native_visuals.region(&removed).is_none());
    app.undo();
    assert!(app.native_visuals.region(&removed).unwrap().is_ok());
}

#[test]
fn invalid_table_region_clipboard_is_rejected_without_creating_an_opaque_region() {
    let mut app = WorksheetApp::default();
    for text in [
        "mathcraft-table:<ws:spec-table/>",
        r#"mathcraft-region:{"Opaque":{"xml":"invalid","kind":"spec-table"}}"#,
    ] {
        assert!(app.paste_region(text));
        assert!(app.document.regions.is_empty());
        assert!(app.undo.is_empty());
    }
}

#[test]
fn unsupported_indexed_native_definition_keeps_the_failed_binding_name() {
    let source = r#"<ml:define xmlns:ml="http://schemas.mathsoft.com/math50" unsupported-setting="true"><ml:apply><ml:indexer/><ml:id labels="VARIABLE">a</ml:id><ml:sequence><ml:real>1</ml:real></ml:sequence></ml:apply><ml:real>7</ml:real></ml:define>"#;
    let fault = raw_fault_statement(source);
    let document = roxmltree::Document::parse(&fault).unwrap();
    let lhs = document
        .root_element()
        .children()
        .find(roxmltree::Node::is_element)
        .unwrap();
    assert_eq!(lhs.tag_name().name(), "id");
    assert_eq!(lhs.text(), Some("a"));
    assert_eq!(lhs.attribute("labels"), Some("VARIABLE"));
    assert!(fault.contains("<ml:unsupported/>") && !fault.contains("indexer"));
    let mut app = WorksheetApp::default();
    let add = |app: &mut WorksheetApp, y, text| {
        let mut editor = Editor::new();
        formula::input_text(&mut editor, text);
        let id = app.document.add_math(0., y);
        app.document
            .regions
            .iter_mut()
            .find(|r| r.id == id)
            .unwrap()
            .content = CanvasContent::Math(editor.to_portable().unwrap());
        id
    };
    add(&mut app, 0., "a:2");
    let native = app.document.add_opaque(0., 100., format!("<region xmlns=\"http://schemas.mathsoft.com/worksheet50\"><math>{source}</math></region>"), "math");
    let dependent = add(&mut app, 200., "a=");
    let independent = add(&mut app, 300., "2+3=");
    app.recalculate();
    assert!(app.results[&native].error.is_some());
    assert!(
        app.results[&dependent].error.is_some(),
        "the previous good a must not revive after an unsupported indexed definition"
    );
    assert!(
        matches!(app.results[&independent].value, Some(Value::Scalar { quantity }) if quantity.value.re == 5.)
    );
}

/// Cross the editor/worksheet/export/import/cache boundaries without a window.
#[test]
#[ignore = "Writes new indexed-definition fixtures to an explicit output directory"]
fn indexed_definition_save_native_export_reopen_probe() {
    let output = PathBuf::from(
        std::env::var_os("MATHCRAFT_INDEXED_OUTPUT").expect("MATHCRAFT_INDEXED_OUTPUT"),
    );
    std::fs::create_dir_all(&output).unwrap();
    let fragments = [
        r#"<ml:define><ml:apply><ml:indexer/><ml:id>v</ml:id><ml:real>2</ml:real></ml:apply><ml:apply><ml:scale/><ml:real>5</ml:real><ml:id labels="UNIT">m</ml:id></ml:apply></ml:define>"#,
        r#"<ml:eval><ml:id>v</ml:id><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>"#,
        r#"<ml:define><ml:apply><ml:indexer/><ml:id>M</ml:id><ml:sequence><ml:real>1</ml:real><ml:real>2</ml:real></ml:sequence></ml:apply><ml:real>13</ml:real></ml:define>"#,
        r#"<ml:eval><ml:id>M</ml:id><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>"#,
        r#"<ml:define><ml:id>r</ml:id><ml:matrix rows="1" cols="3"><ml:real>1</ml:real><ml:real>2</ml:real><ml:real>3</ml:real></ml:matrix></ml:define>"#,
        r#"<ml:define><ml:apply><ml:indexer/><ml:id>r</ml:id><ml:real>1</ml:real></ml:apply><ml:apply><ml:plus/><ml:real>1</ml:real><ml:imag>3</ml:imag></ml:apply></ml:define>"#,
        r#"<ml:eval><ml:id>r</ml:id><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>"#,
    ];
    let mut app = WorksheetApp::default();
    let mut ids = Vec::new();
    for (index, fragment) in fragments.iter().enumerate() {
        let xml = format!(
            "<context xmlns:ml=\"http://schemas.mathsoft.com/math50\">{fragment}</context>"
        );
        let tree = worksheet_math::tree_from_mathml(&xml).unwrap();
        let id = app.document.add_math(0., index as f64 * 40.);
        app.document.regions.last_mut().unwrap().content = CanvasContent::Math(tree);
        ids.push(id);
    }
    let verify = |app: &WorksheetApp| {
        assert_eq!(app.results.len(), 7);
        assert!(
            app.results.values().all(|row| row.error.is_none()),
            "{:?}",
            app.results
        );
        let Some(Value::Matrix {
            rows,
            cols,
            elements,
        }) = &app.results[&ids[1]].value
        else {
            panic!()
        };
        assert_eq!((*rows, *cols), (3, 1));
        assert_eq!(
            elements.iter().map(|q| q.value.re).collect::<Vec<_>>(),
            [0., 0., 5.]
        );
        assert!(
            elements
                .iter()
                .all(|q| q.dimension == mathcraft_core::unit("m").unwrap().dimension)
        );
        let Some(Value::Matrix {
            rows,
            cols,
            elements,
        }) = &app.results[&ids[3]].value
        else {
            panic!()
        };
        assert_eq!((*rows, *cols), (2, 3));
        assert_eq!(elements[5].value.re, 13.);
        assert!(elements[..5].iter().all(|q| q.value.re == 0.));
        let Some(Value::Matrix {
            rows,
            cols,
            elements,
        }) = &app.results[&ids[6]].value
        else {
            panic!()
        };
        assert_eq!((*rows, *cols), (1, 3));
        assert_eq!(elements[1].value.re, 1.);
        assert_eq!(elements[1].value.im, 3.);
    };
    app.recalculate();
    verify(&app);
    app.persist_calculation_cache();
    let saved = output.join("indexed.mathcraft");
    let native = output.join("indexed.mcdx");
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
        3
    );
    let mut imported = WorksheetApp::default();
    imported.open(&native);
    assert_eq!(imported.document.regions.len(), 7, "{}", imported.status);
    assert!(
        imported
            .document
            .regions
            .iter()
            .all(|r| matches!(r.content, CanvasContent::Math(_)))
    );
    imported.recalculate();
    verify(&imported);
    println!(
        "INDEXED_NATIVE regions=7 query_displays=3 saved_reopen_execution=0 native_recalculate_errors=0"
    );
}

/// Save/export a matrix altered through the same rectangle/ribbon transaction
/// used by the headless widget. No original runtime or desktop is involved.
#[test]
#[ignore = "Writes matrix-selection fixtures to an explicit output directory"]
fn matrix_selection_save_native_export_reopen_probe() {
    let output = PathBuf::from(
        std::env::var_os("MATHCRAFT_MATRIX_OUTPUT").expect("MATHCRAFT_MATRIX_OUTPUT"),
    );
    std::fs::create_dir_all(&output).unwrap();
    let fragments = [
        // Native MathML stores matrix elements in column-major order.
        r#"<ml:define><ml:id labels="VARIABLE">M</ml:id><ml:matrix rows="3" cols="3"><ml:real>1</ml:real><ml:real>4</ml:real><ml:real>7</ml:real><ml:real>2</ml:real><ml:real>5</ml:real><ml:real>8</ml:real><ml:real>3</ml:real><ml:real>6</ml:real><ml:real>9</ml:real></ml:matrix></ml:define>"#,
        r#"<ml:eval><ml:id labels="VARIABLE">M</ml:id><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>"#,
    ];
    let mut app = WorksheetApp::default();
    let mut ids = Vec::new();
    for (index, fragment) in fragments.iter().enumerate() {
        let tree = worksheet_math::tree_from_mathml(&format!(
            "<context xmlns:ml=\"http://schemas.mathsoft.com/math50\">{fragment}</context>"
        ))
        .unwrap();
        let id = app.document.add_math(0., index as f64 * 120.);
        app.document.regions.last_mut().unwrap().content = CanvasContent::Math(tree.clone());
        app.editors
            .insert(id, Editor::from_portable(&tree).unwrap());
        ids.push(id);
    }
    app.recalculate();
    app.last_executed_regions.clear();
    app.selected = Some(ids[0]);
    app.insertion = None;
    let original = app.editors[&ids[0]].to_portable().unwrap();
    let editor = app.editors.get_mut(&ids[0]).unwrap();
    let matrix = editor.arena().get(editor.root()).unwrap().children()[1];
    assert!(editor.select_matrix_cells(matrix, 3, 5));
    let copied = editor.copy_matrix_selection().unwrap();
    assert_eq!(
        copied.kind,
        NodeKind::Matrix {
            rows: 1,
            cols: 3,
            display: None
        }
    );
    assert_eq!(copied.children, ["4", "5", "6"].map(number));
    app.matrix_command(RibbonAction::ClearCells);
    assert!(app.last_executed_regions.is_empty());
    app.undo();
    assert_eq!(app.editors[&ids[0]].to_portable().unwrap(), original);
    let editor = app.editors.get_mut(&ids[0]).unwrap();
    let matrix = editor.arena().get(editor.root()).unwrap().children()[1];
    assert!(editor.select_matrix_cells(matrix, 3, 5));
    app.matrix_command(RibbonAction::DeleteMatrixRow);
    assert!(app.uncommitted_math.contains(&ids[0]));
    let Some(Value::Matrix { rows, cols, .. }) = &app.results[&ids[1]].value else {
        panic!()
    };
    assert_eq!(
        (*rows, *cols),
        (3, 3),
        "draft must retain the prior committed answer"
    );
    app.commit_math_edits();
    app.recalculate();
    let verify = |app: &WorksheetApp| {
        assert!(app.results.values().all(|row| row.error.is_none()));
        let Some(Value::Matrix {
            rows,
            cols,
            elements,
        }) = &app.results[&ids[1]].value
        else {
            panic!()
        };
        assert_eq!((*rows, *cols), (2, 3));
        assert_eq!(
            elements.iter().map(|q| q.value.re).collect::<Vec<_>>(),
            [1., 2., 3., 7., 8., 9.]
        );
    };
    verify(&app);
    app.persist_calculation_cache();
    let saved = output.join("matrix.mathcraft");
    let native = output.join("matrix.mcdx");
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
        1
    );
    let mut imported = WorksheetApp::default();
    imported.open(&native);
    assert_eq!(imported.document.regions.len(), 2);
    assert!(
        imported
            .document
            .regions
            .iter()
            .all(|region| matches!(region.content, CanvasContent::Math(_)))
    );
    imported.recalculate();
    verify(&imported);
    println!(
        "MATRIX_NATIVE regions=2 shape=2x3 saved_query_displays=1 saved_reopen_execution=0 native_recalculate_errors=0"
    );
}

/// Exercise operator commands through native storage and calculated caches.
#[test]
#[ignore = "Writes structural-operator fixtures to an explicit output directory"]
fn structural_operators_save_native_export_reopen_probe() {
    let output = PathBuf::from(
        std::env::var_os("MATHCRAFT_OPERATORS_OUTPUT").expect("MATHCRAFT_OPERATORS_OUTPUT"),
    );
    std::fs::create_dir_all(&output).unwrap();
    let matrix = |rows, cols, values: &[&str]| PortableTree {
        kind: NodeKind::Matrix {
            rows,
            cols,
            display: None,
        },
        children: values.iter().map(|value| number(value)).collect(),
    };
    let rectangular = || matrix(2, 3, &["1", "2", "3", "4", "5", "6"]);
    let square = || matrix(2, 2, &["1", "2", "3", "4"]);
    let cases = [
        (RibbonAction::Factorial, number("5")),
        (RibbonAction::Transpose, rectangular()),
        (
            RibbonAction::Conjugate,
            PortableTree {
                kind: NodeKind::Binary(BinaryOp::Add),
                children: vec![
                    number("2"),
                    PortableTree {
                        kind: NodeKind::Imaginary("3".into()),
                        children: vec![],
                    },
                ],
            },
        ),
        (RibbonAction::Absolute, square()),
        (RibbonAction::Norm, square()),
        (RibbonAction::RowOperator, rectangular()),
        (RibbonAction::ColumnOperator, rectangular()),
    ];
    let mut app = WorksheetApp::default();
    let mut ids = Vec::new();
    for (index, (action, operand)) in cases.into_iter().enumerate() {
        let id = app.document.add_math(20., index as f64 * 110. + 20.);
        app.document.regions.last_mut().unwrap().content = CanvasContent::Math(operand.clone());
        let mut editor = Editor::from_portable(&operand).unwrap();
        editor.select_subtree(editor.root()).unwrap();
        app.editors.insert(id, editor);
        app.selected = Some(id);
        app.insertion = None;
        app.ribbon_command(&egui::Context::default(), action);
        if matches!(
            action,
            RibbonAction::RowOperator | RibbonAction::ColumnOperator
        ) {
            app.apply_editor_command(|editor| {
                formula::input_text_with_actions(editor, "1");
            });
        }
        app.apply_editor_command(|editor| {
            editor.insert_evaluation();
        });
        ids.push(id);
    }
    let verify = |app: &WorksheetApp| {
        assert_eq!(app.results.len(), 7);
        assert!(
            app.results.values().all(|row| row.error.is_none()),
            "{:?}",
            app.results
        );
        let scalar = |index: usize| {
            let Some(Value::Scalar { quantity }) = &app.results[&ids[index]].value else {
                panic!()
            };
            quantity.value
        };
        assert_eq!(scalar(0).re, 120.);
        assert_eq!((scalar(2).re, scalar(2).im), (2., -3.));
        assert!((scalar(3).re + 2.).abs() < 1e-12);
        assert!((scalar(4).re + 2.).abs() < 1e-12);
        for (index, shape, expected) in [
            (1, (3, 2), vec![1., 4., 2., 5., 3., 6.]),
            (5, (1, 3), vec![4., 5., 6.]),
            (6, (2, 1), vec![2., 5.]),
        ] {
            let Some(Value::Matrix {
                rows,
                cols,
                elements,
            }) = &app.results[&ids[index]].value
            else {
                panic!()
            };
            assert_eq!((*rows, *cols), shape);
            assert_eq!(
                elements.iter().map(|q| q.value.re).collect::<Vec<_>>(),
                expected
            );
        }
    };
    app.recalculate();
    verify(&app);
    app.persist_calculation_cache();
    let saved = output.join("operators.mathcraft");
    let native = output.join("operators.mcdx");
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
            .all(|region| matches!(region.content, CanvasContent::Math(_)))
    );
    imported.recalculate();
    verify(&imported);
    imported.persist_calculation_cache();
    imported
        .document
        .save(&output.join("operators-reimported.mathcraft"))
        .unwrap();
    println!(
        "OPERATORS_NATIVE regions=7 saved_query_displays=7 saved_reopen_execution=0 native_recalculate_errors=0"
    );
}
