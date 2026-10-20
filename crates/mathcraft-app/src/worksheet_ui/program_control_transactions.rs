//! Ctrl+J, ribbon insertion, commit/Undo and native control flow.
use super::*;

fn atom(kind: NodeKind) -> PortableTree {
    PortableTree {
        kind,
        children: vec![],
    }
}
fn number(n: i32) -> PortableTree {
    atom(NodeKind::Number(n.to_string()))
}
fn name(n: &str) -> PortableTree {
    atom(NodeKind::Identifier(n.into()))
}
fn node(kind: NodeKind, children: Vec<PortableTree>) -> PortableTree {
    PortableTree { kind, children }
}
fn program(lines: Vec<PortableTree>) -> PortableTree {
    node(
        NodeKind::Program {
            statement_count: lines.len(),
        },
        lines,
    )
}
fn binary(op: BinaryOp, left: PortableTree, right: PortableTree) -> PortableTree {
    node(NodeKind::Binary(op), vec![left, right])
}
fn eval(value: PortableTree) -> PortableTree {
    binary(BinaryOp::Evaluate, value, atom(NodeKind::Placeholder))
}
fn local(n: &str, value: PortableTree) -> PortableTree {
    node(NodeKind::LocalDefine, vec![name(n), value])
}
fn add_to(n: &str, value: PortableTree) -> PortableTree {
    local(n, binary(BinaryOp::Add, name(n), value))
}
fn control(word: &str) -> PortableTree {
    let mut editor = Editor::from_portable(&name(word)).unwrap();
    assert!(editor.convert_program_keyword());
    editor.to_portable().unwrap()
}
fn conditional(test: PortableTree, statement: PortableTree) -> PortableTree {
    node(
        NodeKind::If {
            has_else: false,
            branch_count: 1,
            alternate: false,
        },
        vec![test, program(vec![statement])],
    )
}
fn for_loop(n: &str, end: i32, body: Vec<PortableTree>) -> PortableTree {
    node(
        NodeKind::For,
        vec![
            name(n),
            node(
                NodeKind::Range { has_step: false },
                vec![number(1), number(end)],
            ),
            program(body),
        ],
    )
}
fn sum_loop(word: &str) -> PortableTree {
    program(vec![
        local("total", number(0)),
        for_loop(
            "i",
            5,
            vec![
                conditional(binary(BinaryOp::Equal, name("i"), number(3)), control(word)),
                add_to("total", name("i")),
            ],
        ),
        name("total"),
    ])
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
fn fixture(tree: PortableTree) -> (WorksheetApp, egui::Context, u64) {
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    let mut app = WorksheetApp::default();
    let id = app.document.add_math(20., 20.);
    app.document.regions.last_mut().unwrap().content = CanvasContent::Math(tree.clone());
    app.editors
        .insert(id, Editor::from_portable(&tree).unwrap());
    app.selected = Some(id);
    app.editing_region = Some(id);
    app.focus_new = Some(id);
    frame(&mut app, &ctx, vec![]);
    (app, ctx, id)
}
#[track_caller]
fn scalar(app: &WorksheetApp, id: u64, expected: f64) {
    assert!(app.results[&id].error.is_none(), "{:?}", app.results[&id]);
    let Some(Value::Scalar { quantity }) = &app.results[&id].value else {
        panic!("{:?}", app.results[&id])
    };
    assert_eq!(quantity.value.re, expected);
    assert_eq!(quantity.value.im, 0.);
}

#[test]
fn actual_ctrl_j_uses_exact_modifiers_and_native_keyword_shape_with_one_undo() {
    for (word, expected) in [
        ("break", NodeKind::Break),
        ("continue", NodeKind::Continue),
        ("return", NodeKind::Return),
        ("for", NodeKind::For),
        ("while", NodeKind::While),
        ("try", NodeKind::Try),
        (
            "if",
            NodeKind::If {
                has_else: false,
                branch_count: 1,
                alternate: false,
            },
        ),
    ] {
        let (mut app, ctx, id) = fixture(name(word));
        let before = app.document.regions[0].content.clone();
        frame(
            &mut app,
            &ctx,
            vec![key(
                egui::Key::J,
                egui::Modifiers {
                    shift: true,
                    ..egui::Modifiers::COMMAND
                },
            )],
        );
        frame(
            &mut app,
            &ctx,
            vec![key(
                egui::Key::J,
                egui::Modifiers {
                    alt: true,
                    ..egui::Modifiers::COMMAND
                },
            )],
        );
        assert_eq!(app.document.regions[0].content, before);
        frame(
            &mut app,
            &ctx,
            vec![key(egui::Key::J, egui::Modifiers::COMMAND)],
        );
        let tree = app.editors[&id].to_portable().unwrap();
        assert_eq!(tree.kind, expected);
        assert!(app.last_executed_regions.is_empty());
        if tree.children.is_empty() {
            assert_eq!(app.editors[&id].caret().caret_side, Direction::Right);
        } else {
            assert_eq!(
                app.editors[&id]
                    .arena()
                    .get(app.editors[&id].caret().node)
                    .unwrap()
                    .kind(),
                &NodeKind::Placeholder
            );
            assert!(!format!("{tree:?}").contains(word));
        }
        app.undo();
        assert_eq!(app.document.regions[0].content, before);
        app.redo();
        assert_eq!(app.editors[&id].to_portable().unwrap(), tree);
    }
}

#[test]
fn empty_slot_ribbon_insertion_and_inward_deletion_are_undoable_without_a_loop_gate() {
    for (action, kind) in [
        (RibbonAction::Break, NodeKind::Break),
        (RibbonAction::Continue, NodeKind::Continue),
        (RibbonAction::Try, NodeKind::Try),
    ] {
        let (mut app, ctx, id) = fixture(atom(NodeKind::Placeholder));
        app.ribbon_command(&ctx, action);
        assert_eq!(app.editors[&id].to_portable().unwrap().kind, kind);
        frame(
            &mut app,
            &ctx,
            vec![key(egui::Key::Backspace, egui::Modifiers::NONE)],
        );
        assert_eq!(
            app.editors[&id].to_portable().unwrap().kind,
            NodeKind::Placeholder
        );
        app.undo();
        assert_eq!(app.editors[&id].to_portable().unwrap().kind, kind);
        app.redo();
        assert_eq!(
            app.editors[&id].to_portable().unwrap().kind,
            NodeKind::Placeholder
        );
    }
}

#[test]
fn changed_loop_control_keeps_result_until_boundary_enter_and_undo_restores_it() {
    let (mut app, ctx, id) = fixture(eval(sum_loop("continue")));
    app.recalculate();
    scalar(&app, id, 12.);
    app.last_executed_regions.clear();
    let target = app.editors[&id]
        .arena()
        .nodes()
        .find(|(_, n)| n.kind() == &NodeKind::Continue)
        .unwrap()
        .0;
    app.editors
        .get_mut(&id)
        .unwrap()
        .select_subtree(target)
        .unwrap();
    app.apply_editor_command(|editor| {
        editor.replace_selection(&name("break")).unwrap();
    });
    frame(
        &mut app,
        &ctx,
        vec![key(egui::Key::J, egui::Modifiers::COMMAND)],
    );
    scalar(&app, id, 12.);
    assert!(app.last_executed_regions.is_empty());
    let root = app.editors[&id].root();
    app.editors
        .get_mut(&id)
        .unwrap()
        .place_caret(root, Direction::Right)
        .unwrap();
    frame(
        &mut app,
        &ctx,
        vec![key(egui::Key::Enter, egui::Modifiers::NONE)],
    );
    scalar(&app, id, 3.);
    app.undo();
    assert!(
        app.editors[&id]
            .arena()
            .nodes()
            .any(|(_, n)| matches!(n.kind(),NodeKind::Identifier(word) if word=="break"))
    );
    app.undo();
    // Undo while an equation owns editing is itself a draft restoration.
    scalar(&app, id, 3.);
    let root = app.editors[&id].root();
    app.editors
        .get_mut(&id)
        .unwrap()
        .place_caret(root, Direction::Right)
        .unwrap();
    frame(
        &mut app,
        &ctx,
        vec![key(egui::Key::Enter, egui::Modifiers::NONE)],
    );
    scalar(&app, id, 12.);
    app.redo();
    scalar(&app, id, 12.);
    // The final inverse snapshot was taken after leaving the equation.
    app.redo();
    scalar(&app, id, 3.);
}

fn conditional_chain(alternate: bool, all_false: bool) -> PortableTree {
    program(vec![
        local("total", number(0)),
        node(
            NodeKind::If {
                branch_count: 3,
                has_else: true,
                alternate,
            },
            vec![
                number(i32::from(!all_false)),
                program(vec![add_to("total", number(1))]),
                number(i32::from(!all_false)),
                program(vec![add_to("total", number(10))]),
                number(i32::from(!all_false)),
                program(vec![add_to("total", number(100))]),
                program(vec![local("total", number(43))]),
            ],
        ),
        name("total"),
    ])
}

#[test]
fn original_if_and_separate_branch_ribbon_commands_are_atomic_and_keep_adjacent_statements() {
    let (mut app, ctx, id) = fixture(program(vec![atom(NodeKind::Placeholder), number(999)]));
    let first = app.editors[&id]
        .arena()
        .get(app.editors[&id].root())
        .unwrap()
        .children()[0];
    app.editors
        .get_mut(&id)
        .unwrap()
        .place_caret(first, Direction::Right)
        .unwrap();
    app.ribbon_command(&ctx, RibbonAction::If);
    let created = app.editors[&id].to_portable().unwrap();
    assert_eq!(
        created.children[0].kind,
        NodeKind::If {
            branch_count: 1,
            has_else: false,
            alternate: false
        }
    );
    assert_eq!(created.children[1], number(999));
    let pair = app.editors[&id]
        .arena()
        .get(app.editors[&id].root())
        .unwrap()
        .children()[0];
    app.editors
        .get_mut(&id)
        .unwrap()
        .place_caret(pair, Direction::Right)
        .unwrap();
    app.ribbon_command(&ctx, RibbonAction::ElseIf);
    let extended = app.editors[&id].to_portable().unwrap();
    assert_eq!(
        extended.children[0].kind,
        NodeKind::If {
            branch_count: 2,
            has_else: false,
            alternate: false
        }
    );
    app.undo();
    assert_eq!(app.editors[&id].to_portable().unwrap(), created);
    app.redo();
    assert_eq!(app.editors[&id].to_portable().unwrap(), extended);
    app.editors
        .get_mut(&id)
        .unwrap()
        .place_caret(pair, Direction::Right)
        .unwrap();
    app.ribbon_command(&ctx, RibbonAction::Else);
    let terminal = app.editors[&id].to_portable().unwrap();
    assert_eq!(
        terminal.children[0].kind,
        NodeKind::If {
            branch_count: 2,
            has_else: true,
            alternate: false
        }
    );
    app.editors
        .get_mut(&id)
        .unwrap()
        .place_caret(pair, Direction::Right)
        .unwrap();
    app.ribbon_command(&ctx, RibbonAction::Else);
    assert_eq!(app.editors[&id].to_portable().unwrap(), terminal);
    app.undo();
    assert_eq!(app.editors[&id].to_portable().unwrap(), extended);
}

#[test]
fn actual_ctrl_j_attaches_branch_keywords_at_the_original_adjacent_blank_boundary() {
    for word in ["else", "elseif", "alsoif"] {
        let original = program(vec![
            conditional(number(0), number(12)),
            name(word),
            number(999),
        ]);
        let (mut app, ctx, id) = fixture(original.clone());
        let target = app.editors[&id]
            .arena()
            .get(app.editors[&id].root())
            .unwrap()
            .children()[1];
        app.editors
            .get_mut(&id)
            .unwrap()
            .place_text_caret(target, 2)
            .unwrap();
        frame(
            &mut app,
            &ctx,
            vec![key(egui::Key::J, egui::Modifiers::COMMAND)],
        );
        let edited = app.editors[&id].to_portable().unwrap();
        assert_eq!(edited.children.len(), 2);
        assert_eq!(edited.children[1], number(999));
        assert_eq!(
            edited.children[0].kind,
            NodeKind::If {
                branch_count: if word == "else" { 1 } else { 2 },
                has_else: word == "else",
                alternate: word == "alsoif"
            }
        );
        assert!(app.last_executed_regions.is_empty());
        app.undo();
        assert_eq!(app.editors[&id].to_portable().unwrap(), original);
        app.redo();
        assert_eq!(app.editors[&id].to_portable().unwrap(), edited);
    }
}

#[test]
fn changing_the_whole_chain_mode_is_draft_until_enter_and_retains_results_through_undo() {
    let (mut app, ctx, id) = fixture(eval(conditional_chain(false, false)));
    app.recalculate();
    scalar(&app, id, 1.);
    app.last_executed_regions.clear();
    let pair = app.editors[&id]
        .arena()
        .nodes()
        .find(|(_, n)| matches!(n.kind(), NodeKind::If { .. }))
        .unwrap()
        .0;
    app.editors
        .get_mut(&id)
        .unwrap()
        .place_if_operator(pair, 1)
        .unwrap();
    app.ribbon_command(&ctx, RibbonAction::AlsoIf);
    scalar(&app, id, 1.);
    assert!(app.last_executed_regions.is_empty());
    let root = app.editors[&id].root();
    app.editors
        .get_mut(&id)
        .unwrap()
        .place_caret(root, Direction::Right)
        .unwrap();
    frame(
        &mut app,
        &ctx,
        vec![key(egui::Key::Enter, egui::Modifiers::NONE)],
    );
    scalar(&app, id, 111.);
    app.undo();
    scalar(&app, id, 111.);
    let root = app.editors[&id].root();
    app.editors
        .get_mut(&id)
        .unwrap()
        .place_caret(root, Direction::Right)
        .unwrap();
    frame(
        &mut app,
        &ctx,
        vec![key(egui::Key::Enter, egui::Modifiers::NONE)],
    );
    scalar(&app, id, 1.);
}

#[test]
#[ignore = "Writes conditional-chain native fixtures to an explicit output directory"]
fn conditional_branch_save_native_export_reopen_probe() {
    let output = PathBuf::from(
        std::env::var_os("MATHCRAFT_CONDITIONAL_BRANCH_OUTPUT")
            .expect("MATHCRAFT_CONDITIONAL_BRANCH_OUTPUT"),
    );
    std::fs::create_dir_all(&output).unwrap();
    let mut app = WorksheetApp::default();
    let returned = node(
        NodeKind::If {
            branch_count: 3,
            has_else: false,
            alternate: true,
        },
        vec![
            number(0),
            program(vec![number(10)]),
            number(1),
            program(vec![node(
                NodeKind::Return,
                vec![binary(BinaryOp::Multiply, number(73), name("V"))],
            )]),
            number(1),
            program(vec![number(999)]),
        ],
    );
    let broken = node(
        NodeKind::If {
            branch_count: 2,
            has_else: false,
            alternate: false,
        },
        vec![
            number(0),
            program(vec![number(0)]),
            number(1),
            program(vec![control("break")]),
        ],
    );
    let trees = vec![
        conditional_chain(false, false),
        conditional_chain(true, false),
        conditional_chain(false, true),
        program(vec![returned, number(9999)]),
        program(vec![
            local("total", number(7)),
            for_loop("i", 3, vec![broken, local("total", number(999))]),
            name("total"),
        ]),
    ];
    let mut ids = vec![];
    for (index, tree) in trees.into_iter().enumerate() {
        let id = app.document.add_math(20., 20. + index as f64 * 320.);
        app.document.regions.last_mut().unwrap().content = CanvasContent::Math(eval(tree));
        ids.push(id);
    }
    let verify = |app: &WorksheetApp| {
        assert_eq!(app.results.len(), 5);
        for (id, expected) in ids.iter().zip([1., 111., 43., 73., 7.]) {
            scalar(app, *id, expected);
        }
        let Some(Value::Scalar { quantity }) = &app.results[&ids[3]].value else {
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
    let saved = output.join("conditional-branches.mathcraft");
    let native = output.join("conditional-branches.mcdx");
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
    let mut imported = WorksheetApp::default();
    imported.open(&native);
    assert_eq!(imported.document.regions.len(), 5);
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
        .save(&output.join("conditional-branches-reimported.mathcraft"))
        .unwrap();
    println!(
        "CONDITIONAL_BRANCH_NATIVE regions=5 saved_query_displays=5 saved_reopen_execution=0 native_recalculate_errors=0"
    );
}

#[test]
#[ignore = "Writes program-control native fixtures to an explicit output directory"]
fn program_control_save_native_export_reopen_probe() {
    let output = PathBuf::from(
        std::env::var_os("MATHCRAFT_PROGRAM_CONTROL_OUTPUT")
            .expect("MATHCRAFT_PROGRAM_CONTROL_OUTPUT"),
    );
    std::fs::create_dir_all(&output).unwrap();
    let mut app = WorksheetApp::default();
    let mut trees = vec![sum_loop("continue"), sum_loop("break")];
    trees.push(program(vec![
        local("i", number(0)),
        local("total", number(0)),
        node(
            NodeKind::While,
            vec![
                binary(BinaryOp::LessThan, name("i"), number(6)),
                program(vec![
                    add_to("i", number(1)),
                    conditional(
                        binary(BinaryOp::Equal, name("i"), number(2)),
                        control("continue"),
                    ),
                    conditional(
                        binary(BinaryOp::Equal, name("i"), number(5)),
                        control("break"),
                    ),
                    add_to("total", name("i")),
                ]),
            ],
        ),
        name("total"),
    ]));
    trees.push(program(vec![
        local("total", number(0)),
        for_loop(
            "i",
            3,
            vec![for_loop(
                "j",
                3,
                vec![
                    conditional(
                        binary(BinaryOp::Equal, name("j"), number(2)),
                        control("break"),
                    ),
                    add_to("total", number(1)),
                ],
            )],
        ),
        name("total"),
    ]));
    let mut ret = control("return");
    ret.children[0] = binary(BinaryOp::Multiply, number(7), name("V"));
    trees.push(program(vec![
        node(NodeKind::While, vec![number(1), program(vec![ret])]),
        number(999),
    ]));
    let mut ids = vec![];
    for (i, tree) in trees.into_iter().enumerate() {
        let id = app.document.add_math(20., i as f64 * 240. + 20.);
        app.document.regions.last_mut().unwrap().content = CanvasContent::Math(eval(tree));
        ids.push(id);
    }
    let verify = |app: &WorksheetApp| {
        assert_eq!(app.results.len(), 5);
        for (id, value) in ids.iter().zip([12., 3., 8., 3., 7.]) {
            scalar(app, *id, value);
        }
        let Some(Value::Scalar { quantity }) = &app.results[ids.last().unwrap()].value else {
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
    let saved = output.join("program-control.mathcraft");
    let native = output.join("program-control.mcdx");
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
        5
    );
    let mut imported = WorksheetApp::default();
    imported.open(&native);
    assert_eq!(imported.document.regions.len(), 5);
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
        .save(&output.join("program-control-reimported.mathcraft"))
        .unwrap();
    println!(
        "PROGRAM_CONTROL_NATIVE regions=5 saved_query_displays=5 saved_reopen_execution=0 native_recalculate_errors=0"
    );
}

#[test]
fn scoped_special_ribbon_keeps_the_selected_expression_in_the_original_primal_body() {
    let source = binary(
        BinaryOp::Multiply,
        number(17),
        atom(NodeKind::NativeIdentifier {
            name: "V".into(),
            labels: "UNIT".into(),
            contextual: Some(false),
            subscript: Some("r😀".into()),
        }),
    );
    for (action, expected_kind, body_index) in [
        (
            RibbonAction::If,
            NodeKind::If {
                has_else: false,
                branch_count: 1,
                alternate: false,
            },
            1,
        ),
        (RibbonAction::WhileLoop, NodeKind::While, 1),
        (RibbonAction::ForLoop, NodeKind::For, 2),
        (RibbonAction::Try, NodeKind::Try, 0),
    ] {
        for side in [Direction::Left, Direction::Right] {
            let original = eval(source.clone());
            let (mut app, ctx, id) = fixture(original.clone());
            let editor = app.editors.get_mut(&id).unwrap();
            let target = editor.arena().get(editor.root()).unwrap().children()[0];
            editor.select_subtree_from(target, side).unwrap();
            let before_caret = editor.caret();
            let before_selection = editor.selection();
            app.ribbon_command(&ctx, action);

            let editor = &app.editors[&id];
            let actual = editor.to_portable().unwrap();
            let control = &actual.children[0];
            assert_eq!(control.kind, expected_kind);
            assert_eq!(control.children[body_index], program(vec![source.clone()]));
            assert_eq!(editor.export_subtree(target).unwrap(), source);
            let body = editor.arena().get(target).unwrap().parent().unwrap();
            assert_eq!(editor.arena().get(body).unwrap().children(), &[target]);
            assert_eq!(editor.selection(), mathcraft_editor::Selection::None);
            assert!(app.uncommitted_math.contains(&id));
            assert!(app.last_executed_regions.is_empty());

            app.undo();
            assert_eq!(app.editors[&id].to_portable().unwrap(), original);
            assert_eq!(app.editors[&id].caret(), before_caret);
            assert_eq!(app.editors[&id].selection(), before_selection);
            app.redo();
            assert_eq!(app.editors[&id].to_portable().unwrap(), actual);
        }
    }
}

#[test]
fn scoped_special_ribbon_reuses_an_existing_program_as_the_body_without_double_wrapping() {
    let source = program(vec![local("q", number(17)), name("q")]);
    for (action, body_index) in [
        (RibbonAction::If, 1),
        (RibbonAction::WhileLoop, 1),
        (RibbonAction::ForLoop, 2),
        (RibbonAction::Try, 0),
    ] {
        let (mut app, ctx, id) = fixture(eval(source.clone()));
        let editor = app.editors.get_mut(&id).unwrap();
        let target = editor.arena().get(editor.root()).unwrap().children()[0];
        let source_children = editor.arena().get(target).unwrap().children().to_vec();
        editor
            .select_subtree_from(target, Direction::Right)
            .unwrap();
        app.ribbon_command(&ctx, action);
        let editor = &app.editors[&id];
        assert_eq!(
            editor.to_portable().unwrap().children[0].children[body_index],
            source
        );
        assert_eq!(
            editor.arena().get(target).unwrap().children(),
            source_children
        );
        let wrapper = editor.arena().get(target).unwrap().parent().unwrap();
        assert_eq!(
            editor.arena().get(wrapper).unwrap().children()[body_index],
            target
        );
        assert!(app.last_executed_regions.is_empty());
    }
}

#[test]
fn whole_value_text_selection_replaces_the_value_with_a_blank_control_instead_of_scoped_wrapping() {
    for (action, word) in [
        (RibbonAction::If, "if"),
        (RibbonAction::WhileLoop, "while"),
        (RibbonAction::ForLoop, "for"),
        (RibbonAction::Try, "try"),
        (RibbonAction::Return, "return"),
        (RibbonAction::Break, "break"),
        (RibbonAction::Continue, "continue"),
    ] {
        for (anchor, focus) in [(0, 2), (2, 0)] {
            let (mut app, ctx, id) = fixture(number(17));
            let editor = app.editors.get_mut(&id).unwrap();
            editor
                .place_text_selection(editor.root(), anchor, focus)
                .unwrap();
            let old_caret = editor.caret();
            app.ribbon_command(&ctx, action);
            assert_eq!(app.editors[&id].to_portable().unwrap(), control(word));
            assert!(app.last_executed_regions.is_empty());
            app.undo();
            assert_eq!(app.editors[&id].to_portable().unwrap(), number(17));
            assert_eq!(app.editors[&id].caret(), old_caret);
        }
    }
}

#[test]
fn return_ribbon_obeys_the_original_scope_direction_and_keeps_rejected_drafts_unchanged() {
    for side in [Direction::Left, Direction::Right] {
        let original = program(vec![number(73), number(999)]);
        let (mut app, ctx, id) = fixture(original.clone());
        let editor = app.editors.get_mut(&id).unwrap();
        let target = editor.arena().get(editor.root()).unwrap().children()[0];
        editor.select_subtree_from(target, side).unwrap();
        let old_caret = editor.caret();
        let old_selection = editor.selection();
        let old_history = (app.undo.len(), app.redo.len());
        app.ribbon_command(&ctx, RibbonAction::Return);
        if side == Direction::Left {
            assert_eq!(
                app.editors[&id].to_portable().unwrap(),
                program(vec![node(NodeKind::Return, vec![number(73)]), number(999)])
            );
            app.undo();
        } else {
            assert_eq!((app.undo.len(), app.redo.len()), old_history);
            assert!(!app.uncommitted_math.contains(&id));
        }
        assert_eq!(app.editors[&id].to_portable().unwrap(), original);
        assert_eq!(app.editors[&id].caret(), old_caret);
        assert_eq!(app.editors[&id].selection(), old_selection);
        assert!(app.last_executed_regions.is_empty());
    }
}

#[test]
fn scoped_if_authoring_keeps_the_previous_result_until_boundary_enter() {
    let (mut app, ctx, id) = fixture(eval(program(vec![number(17)])));
    app.recalculate();
    scalar(&app, id, 17.);
    app.last_executed_regions.clear();
    let editor = app.editors.get_mut(&id).unwrap();
    let body = editor.arena().get(editor.root()).unwrap().children()[0];
    let target = editor.arena().get(body).unwrap().children()[0];
    editor
        .select_subtree_from(target, Direction::Right)
        .unwrap();
    app.ribbon_command(&ctx, RibbonAction::If);
    frame(&mut app, &ctx, vec![egui::Event::Text("0".into())]);
    scalar(&app, id, 17.);
    assert!(app.last_executed_regions.is_empty());

    let editor = app.editors.get_mut(&id).unwrap();
    let conditional = editor
        .arena()
        .nodes()
        .find(|(_, entry)| matches!(entry.kind(), NodeKind::If { .. }))
        .unwrap()
        .0;
    editor.place_caret(conditional, Direction::Right).unwrap();
    app.ribbon_command(&ctx, RibbonAction::Else);
    frame(&mut app, &ctx, vec![egui::Event::Text("43".into())]);
    scalar(&app, id, 17.);
    assert!(app.last_executed_regions.is_empty());
    let editor = app.editors.get_mut(&id).unwrap();
    editor.place_caret(editor.root(), Direction::Right).unwrap();
    frame(
        &mut app,
        &ctx,
        vec![key(egui::Key::Enter, egui::Modifiers::NONE)],
    );
    scalar(&app, id, 43.);
    assert!(!app.uncommitted_math.contains(&id));
    assert_eq!(app.last_executed_regions.len(), 1);
}

#[test]
fn actual_space_scope_and_ribbon_while_preserve_the_value_in_the_body_and_restore_scope_on_undo() {
    let original = eval(number(17));
    let (mut app, ctx, id) = fixture(original.clone());
    let editor = app.editors.get_mut(&id).unwrap();
    let target = editor.arena().get(editor.root()).unwrap().children()[0];
    editor.place_text_caret(target, 2).unwrap();
    frame(
        &mut app,
        &ctx,
        vec![
            key(egui::Key::Space, egui::Modifiers::NONE),
            egui::Event::Text(" ".into()),
        ],
    );
    assert_eq!(
        app.editors[&id].selection(),
        mathcraft_editor::Selection::Subtree(target)
    );
    let scoped_caret = app.editors[&id].caret();
    app.ribbon_command(&ctx, RibbonAction::WhileLoop);
    assert_eq!(
        app.editors[&id].to_portable().unwrap().children[0],
        node(
            NodeKind::While,
            vec![atom(NodeKind::Placeholder), program(vec![number(17)])]
        )
    );
    app.undo();
    assert_eq!(app.editors[&id].to_portable().unwrap(), original);
    assert_eq!(
        app.editors[&id].selection(),
        mathcraft_editor::Selection::Subtree(target)
    );
    assert_eq!(app.editors[&id].caret(), scoped_caret);
    frame(
        &mut app,
        &ctx,
        vec![
            key(egui::Key::Space, egui::Modifiers::NONE),
            egui::Event::Text(" ".into()),
        ],
    );
    assert_eq!(
        app.editors[&id].selection(),
        mathcraft_editor::Selection::Subtree(app.editors[&id].root())
    );
    assert!(app.last_executed_regions.is_empty());
}

fn scoped_ribbon_program(
    action: RibbonAction,
    word: &str,
    original: PortableTree,
    statement: usize,
    side: Direction,
    headers: Vec<(usize, PortableTree)>,
) -> PortableTree {
    let (mut app, ctx, id) = fixture(eval(original));
    let editor = app.editors.get_mut(&id).unwrap();
    let body = editor.arena().get(editor.root()).unwrap().children()[0];
    let target = editor.arena().get(body).unwrap().children()[statement];
    editor.select_subtree_from(target, side).unwrap();
    app.ribbon_command(&ctx, action);
    let expected_kind = control(word).kind;
    let operator = app.editors[&id]
        .arena()
        .nodes()
        .find(|(_, node)| node.kind() == &expected_kind)
        .unwrap()
        .0;
    for (index, value) in headers {
        let editor = app.editors.get_mut(&id).unwrap();
        let child = editor.arena().get(operator).unwrap().children()[index];
        editor.select_subtree(child).unwrap();
        app.apply_editor_command(|editor| editor.replace_selection(&value).unwrap());
    }
    assert!(app.last_executed_regions.is_empty());
    app.editors[&id].to_portable().unwrap().children.remove(0)
}

#[test]
#[ignore = "Explicit scoped ribbon/native/PDF integration fixture; no GUI or original runtime"]
fn program_insertion_save_native_export_reopen_probe() {
    let output = PathBuf::from(
        std::env::var_os("MATHCRAFT_PROGRAM_INSERTION_OUTPUT")
            .expect("MATHCRAFT_PROGRAM_INSERTION_OUTPUT"),
    );
    std::fs::create_dir_all(&output).unwrap();
    let trees = vec![
        scoped_ribbon_program(
            RibbonAction::WhileLoop,
            "while",
            program(vec![
                local("n", number(0)),
                add_to("n", number(1)),
                name("n"),
            ]),
            1,
            Direction::Right,
            vec![(0, binary(BinaryOp::LessThan, name("n"), number(3)))],
        ),
        scoped_ribbon_program(
            RibbonAction::ForLoop,
            "for",
            program(vec![
                local("total", number(0)),
                add_to("total", name("i")),
                name("total"),
            ]),
            1,
            Direction::Left,
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
        scoped_ribbon_program(
            RibbonAction::If,
            "if",
            program(vec![number(17)]),
            0,
            Direction::Right,
            vec![(0, number(1))],
        ),
        scoped_ribbon_program(
            RibbonAction::Try,
            "try",
            program(vec![division_error()]),
            0,
            Direction::Right,
            vec![(1, program(vec![number(43)]))],
        ),
        scoped_ribbon_program(
            RibbonAction::Return,
            "return",
            program(vec![
                binary(BinaryOp::Multiply, number(73), name("V")),
                number(999),
            ]),
            0,
            Direction::Left,
            vec![],
        ),
    ];
    let mut app = WorksheetApp::default();
    let mut ids = vec![];
    for (index, tree) in trees.into_iter().enumerate() {
        let id = app.document.add_math(20., 20. + index as f64 * 300.);
        app.document.regions.last_mut().unwrap().content = CanvasContent::Math(eval(tree));
        ids.push(id);
    }
    let verify = |app: &WorksheetApp| {
        assert_eq!(app.results.len(), 5);
        for (id, expected) in ids.iter().zip([3., 6., 17., 43., 73.]) {
            scalar(app, *id, expected);
        }
        let Some(Value::Scalar { quantity }) = &app.results[&ids[4]].value else {
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
    let saved = output.join("program-insertion.mathcraft");
    let native = output.join("program-insertion.mcdx");
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
    let mut imported = WorksheetApp::default();
    imported.open(&native);
    assert_eq!(imported.document.regions.len(), 5);
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
        .save(&output.join("program-insertion-reimported.mathcraft"))
        .unwrap();
    println!(
        "PROGRAM_INSERTION_NATIVE regions=5 saved_query_displays=5 saved_reopen_execution=0 native_recalculate_errors=0"
    );
}

fn try_tree(first: Vec<PortableTree>, second: Vec<PortableTree>) -> PortableTree {
    node(NodeKind::Try, vec![program(first), program(second)])
}
fn division_error() -> PortableTree {
    binary(BinaryOp::InlineDivide, number(1), number(0))
}
fn try_loop(word: &str) -> PortableTree {
    program(vec![
        local("total", number(0)),
        for_loop(
            "i",
            5,
            vec![
                try_tree(
                    vec![binary(
                        BinaryOp::InlineDivide,
                        number(1),
                        binary(BinaryOp::Subtract, name("i"), number(3)),
                    )],
                    vec![control(word)],
                ),
                add_to("total", name("i")),
            ],
        ),
        name("total"),
    ])
}

#[test]
fn try_ribbon_is_one_empty_template_command_and_error_fallback_waits_for_enter() {
    let (mut app, ctx, id) = fixture(atom(NodeKind::Placeholder));
    app.ribbon_command(&ctx, RibbonAction::Try);
    let template = app.editors[&id].to_portable().unwrap();
    assert_eq!(template.kind, NodeKind::Try);
    assert_eq!(template.children.len(), 2);
    app.undo();
    assert_eq!(
        app.editors[&id].to_portable().unwrap(),
        atom(NodeKind::Placeholder)
    );
    app.redo();
    assert_eq!(app.editors[&id].to_portable().unwrap(), template);

    let (mut app, ctx, id) = fixture(eval(try_tree(vec![number(17)], vec![number(43)])));
    app.recalculate();
    scalar(&app, id, 17.);
    app.last_executed_regions.clear();
    let target = app.editors[&id]
        .arena()
        .nodes()
        .find(|(_, node)| node.kind() == &NodeKind::Number("17".into()))
        .unwrap()
        .0;
    app.editors
        .get_mut(&id)
        .unwrap()
        .select_subtree(target)
        .unwrap();
    app.apply_editor_command(|editor| {
        editor.replace_selection(&division_error()).unwrap();
    });
    scalar(&app, id, 17.);
    assert!(app.last_executed_regions.is_empty());
    let root = app.editors[&id].root();
    app.editors
        .get_mut(&id)
        .unwrap()
        .place_caret(root, Direction::Right)
        .unwrap();
    frame(
        &mut app,
        &ctx,
        vec![key(egui::Key::Enter, egui::Modifiers::NONE)],
    );
    scalar(&app, id, 43.);
    app.undo();
    scalar(&app, id, 43.);
    let root = app.editors[&id].root();
    app.editors
        .get_mut(&id)
        .unwrap()
        .place_caret(root, Direction::Right)
        .unwrap();
    frame(
        &mut app,
        &ctx,
        vec![key(egui::Key::Enter, egui::Modifiers::NONE)],
    );
    scalar(&app, id, 17.);
}

#[test]
fn deleting_the_on_error_header_merges_bodies_but_keeps_results_until_enter() {
    let original = eval(try_tree(vec![number(17)], vec![number(43)]));
    let (mut app, ctx, id) = fixture(original.clone());
    app.recalculate();
    scalar(&app, id, 17.);
    app.last_executed_regions.clear();
    let target = app.editors[&id]
        .arena()
        .nodes()
        .find(|(_, node)| node.kind() == &NodeKind::Try)
        .unwrap()
        .0;
    app.editors
        .get_mut(&id)
        .unwrap()
        .place_try_operator(target, Direction::Right)
        .unwrap();
    frame(
        &mut app,
        &ctx,
        vec![key(egui::Key::Backspace, egui::Modifiers::NONE)],
    );
    let merged = app.editors[&id].to_portable().unwrap();
    assert_eq!(merged.children[0], program(vec![number(17), number(43)]));
    assert!(app.last_executed_regions.is_empty());
    scalar(&app, id, 17.);
    let root = app.editors[&id].root();
    app.editors
        .get_mut(&id)
        .unwrap()
        .place_caret(root, Direction::Right)
        .unwrap();
    frame(
        &mut app,
        &ctx,
        vec![key(egui::Key::Enter, egui::Modifiers::NONE)],
    );
    scalar(&app, id, 43.);
    app.undo();
    assert_eq!(app.editors[&id].to_portable().unwrap(), original);
    scalar(&app, id, 43.);
    let root = app.editors[&id].root();
    app.editors
        .get_mut(&id)
        .unwrap()
        .place_caret(root, Direction::Right)
        .unwrap();
    frame(
        &mut app,
        &ctx,
        vec![key(egui::Key::Enter, egui::Modifiers::NONE)],
    );
    scalar(&app, id, 17.);
}

#[test]
fn fresh_ctrl_j_try_can_be_deleted_at_its_initial_input_without_selecting_an_outer_node() {
    for key_ in [egui::Key::Backspace, egui::Key::Delete] {
        let original = name("try");
        let (mut app, ctx, id) = fixture(original.clone());
        frame(
            &mut app,
            &ctx,
            vec![key(egui::Key::J, egui::Modifiers::COMMAND)],
        );
        let inserted = app.editors[&id].to_portable().unwrap();
        assert_eq!(inserted.kind, NodeKind::Try);
        assert_eq!(
            app.editors[&id]
                .arena()
                .get(app.editors[&id].caret().node)
                .unwrap()
                .kind(),
            &NodeKind::Placeholder
        );
        frame(&mut app, &ctx, vec![key(key_, egui::Modifiers::NONE)]);
        let expected = if key_ == egui::Key::Backspace {
            atom(NodeKind::Placeholder)
        } else {
            program(vec![
                atom(NodeKind::Placeholder),
                atom(NodeKind::Placeholder),
            ])
        };
        assert_eq!(app.editors[&id].to_portable().unwrap(), expected);
        assert!(app.last_executed_regions.is_empty());
        app.undo();
        assert_eq!(app.editors[&id].to_portable().unwrap(), inserted);
        app.undo();
        assert_eq!(app.editors[&id].to_portable().unwrap(), original);
        app.redo();
        app.redo();
        assert_eq!(app.editors[&id].to_portable().unwrap(), expected);
    }
}

#[test]
#[ignore = "Writes Try/on error native fixtures to an explicit output directory"]
fn try_control_save_native_export_reopen_probe() {
    let output = PathBuf::from(
        std::env::var_os("MATHCRAFT_TRY_CONTROL_OUTPUT").expect("MATHCRAFT_TRY_CONTROL_OUTPUT"),
    );
    std::fs::create_dir_all(&output).unwrap();
    let mut app = WorksheetApp::default();
    let trees = vec![
        try_tree(vec![number(17)], vec![division_error()]),
        try_tree(vec![division_error()], vec![number(43)]),
        try_loop("continue"),
        try_loop("break"),
        program(vec![
            try_tree(
                vec![division_error()],
                vec![node(
                    NodeKind::Return,
                    vec![binary(BinaryOp::Multiply, number(73), name("V"))],
                )],
            ),
            number(999),
        ]),
    ];
    let mut ids = vec![];
    for (index, tree) in trees.into_iter().enumerate() {
        let id = app.document.add_math(20., 20. + index as f64 * 300.);
        app.document.regions.last_mut().unwrap().content = CanvasContent::Math(eval(tree));
        ids.push(id);
    }
    let verify = |app: &WorksheetApp| {
        assert_eq!(app.results.len(), 5);
        for (id, expected) in ids.iter().zip([17., 43., 12., 3., 73.]) {
            scalar(app, *id, expected);
        }
        let Some(Value::Scalar { quantity }) = &app.results[&ids[4]].value else {
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
    let saved = output.join("try-control.mathcraft");
    let native = output.join("try-control.mcdx");
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
    let mut imported = WorksheetApp::default();
    imported.open(&native);
    assert_eq!(imported.document.regions.len(), 5);
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
        .save(&output.join("try-control-reimported.mathcraft"))
        .unwrap();
    println!(
        "TRY_CONTROL_NATIVE regions=5 saved_query_displays=5 saved_reopen_execution=0 native_recalculate_errors=0"
    );
}
