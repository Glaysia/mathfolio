//! Headless authoring, native MathML, geometry and input-ownership regressions.
use super::*;

fn number(value: &str) -> PortableTree {
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
fn matrix() -> PortableTree {
    PortableTree {
        kind: NodeKind::Matrix {
            rows: 2,
            cols: 2,
            display: None,
        },
        children: ["1", "2", "3", "4"].into_iter().map(number).collect(),
    }
}
fn key(key: Key, modifiers: egui::Modifiers) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers,
    }
}
fn command_shift() -> egui::Modifiers {
    egui::Modifiers {
        shift: true,
        ..egui::Modifiers::COMMAND
    }
}
fn send(ctx: &egui::Context, editor: &mut Editor, events: Vec<egui::Event>) -> egui::FullOutput {
    ctx.run_ui(
        egui::RawInput {
            events,
            ..Default::default()
        },
        |ui| {
            assert!(!formula_ui_with_actions(ui, editor, None, true).evaluation_requested);
        },
    )
}
fn calculated(editor: &Editor) -> mathcraft_compat::Value {
    let fragment = crate::worksheet_math::tree_to_mathml(&editor.to_portable().unwrap()).unwrap();
    let worksheet = crate::worksheet_math::worksheet_xml(&[(1, 0., 0., fragment)]).unwrap();
    let rows = mathcraft_compat::evaluate_worksheet(&worksheet).unwrap();
    assert!(rows[0].error.is_none(), "{:?}", rows[0].error);
    rows[0].value.clone().expect("calculated value")
}

#[test]
fn factorial_key_respects_precedence_and_one_text_commit_undo() {
    for (source, expected) in [("5!", 120.), ("1+2!", 3.)] {
        let mut editor = Editor::new();
        input_text(&mut editor, source);
        let tree = editor.to_portable().unwrap();
        assert_ne!(tree.kind, NodeKind::Call { argument_count: 1 });
        let outcome = input_text_with_actions(&mut editor, "=");
        assert!(outcome.evaluation_requested);
        let mathcraft_compat::Value::Scalar { quantity } = calculated(&editor) else {
            panic!()
        };
        assert_eq!(quantity.value.re, expected, "{source}");
    }
    let mut editor = Editor::from_portable(&number("5")).unwrap();
    editor.place_caret(editor.root(), Direction::Right).unwrap();
    let before = editor.to_portable().unwrap();
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    send(&ctx, &mut editor, vec![egui::Event::Text("!".into())]).drop_without_applying_deltas();
    assert_eq!(editor.to_portable().unwrap().kind, NodeKind::Factorial);
    assert!(editor.undo());
    assert_eq!(editor.to_portable().unwrap(), before);
    assert!(!editor.undo());
}

#[test]
fn transpose_row_column_and_conjugate_shortcuts_build_operators_not_function_calls() {
    for (shortcut, kind) in [
        (Key::T, NodeKind::Transpose),
        (Key::R, NodeKind::Row),
        (Key::C, NodeKind::Column),
        (Key::Num9, NodeKind::Conjugate),
        (Key::Pipe, NodeKind::Determinant),
        (Key::Backslash, NodeKind::Determinant),
    ] {
        let mut editor = Editor::from_portable(&matrix()).unwrap();
        let before = editor.to_portable().unwrap();
        editor.select_subtree(editor.root()).unwrap();
        let ctx = egui::Context::default();
        crate::configure(&ctx);
        send(&ctx, &mut editor, vec![key(shortcut, command_shift())])
            .drop_without_applying_deltas();
        assert_eq!(editor.to_portable().unwrap().kind, kind);
        assert_eq!(editor.to_portable().unwrap().children[0], before);
        assert!(editor.undo());
        assert_eq!(editor.to_portable().unwrap(), before);
        assert!(!editor.undo());
    }
}

#[test]
fn exact_column_key_inserts_operator_without_overwriting_clipboard() {
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    let mut editor = Editor::from_portable(&matrix()).unwrap();
    editor.select_subtree(editor.root()).unwrap();
    let output = ctx.run_ui(
        egui::RawInput {
            events: vec![
                egui::Event::ModifiersChanged(command_shift()),
                key(Key::C, command_shift()),
            ],
            ..Default::default()
        },
        |ui| {
            let response = formula_ui_with_actions(ui, &mut editor, None, true);
            assert!(response.response.changed());
            assert!(!response.evaluation_requested);
        },
    );
    assert_eq!(editor.to_portable().unwrap().kind, NodeKind::Column);
    assert!(
        !output
            .platform_output
            .commands
            .iter()
            .any(|command| matches!(command, egui::OutputCommand::CopyText(_)))
    );
    output.drop_without_applying_deltas();
}

#[test]
fn copy_event_with_command_shift_remains_copy_for_insert_menu_and_dedicated_copy_keys() {
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    let mut editor = Editor::from_portable(&matrix()).unwrap();
    editor.select_subtree(editor.root()).unwrap();
    let source = editor.to_portable().unwrap();
    let output = ctx.run_ui(
        egui::RawInput {
            events: vec![
                egui::Event::ModifiersChanged(command_shift()),
                egui::Event::Copy,
            ],
            ..Default::default()
        },
        |ui| {
            let response = formula_ui_with_actions(ui, &mut editor, None, true);
            assert!(!response.response.changed());
            assert!(!response.evaluation_requested);
        },
    );
    let copied = output
        .platform_output
        .commands
        .iter()
        .find_map(|command| {
            let egui::OutputCommand::CopyText(text) = command else {
                return None;
            };
            text.strip_prefix(TREE_CLIPBOARD_PREFIX)
        })
        .expect("a Copy event must retain clipboard behavior regardless of held Shift");
    assert_eq!(
        serde_json::from_str::<PortableTree>(copied).unwrap(),
        source
    );
    assert_eq!(editor.to_portable().unwrap(), source);
    assert!(!editor.undo());
    output.drop_without_applying_deltas();
}

#[test]
fn operator_geometry_preserves_source_hits_and_readonly_measurement_and_input() {
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    let mut trees = vec![
        unary(NodeKind::Factorial, number("5")),
        unary(NodeKind::Transpose, matrix()),
        unary(NodeKind::Conjugate, number("2")),
        unary(NodeKind::Determinant, matrix()),
        unary(NodeKind::Absolute, matrix()),
    ];
    trees.extend([NodeKind::Row, NodeKind::Column].map(|kind| PortableTree {
        kind,
        children: vec![matrix(), number("1")],
    }));
    for tree in trees {
        let editor = Editor::from_portable(&tree).unwrap();
        let source = editor.to_portable().unwrap();
        let fragment = crate::worksheet_math::tree_to_mathml(&source).unwrap();
        let reopened = crate::worksheet_math::tree_from_mathml(&fragment).unwrap();
        assert_eq!(reopened, source);
        ctx.run_ui(egui::RawInput {events:vec![egui::Event::Text("9".into()),key(Key::Delete,egui::Modifiers::NONE)],..Default::default()},|ui| {
            let layout=readonly_layout(ui,&editor,None,None,12.);
            assert!(layout.size.is_finite());assert!(layout.size.x>0. && layout.size.y>0.);
            let mut hits=vec![];collect_hits(&layout,Pos2::ZERO,&mut hits);
            assert!(hits.iter().all(|hit|editor.arena().get(hit.node).is_some()));
            for child in editor.arena().get(editor.root()).unwrap().children() {
                assert!(hits.iter().any(|hit|hit.node==*child),"lost operand hit {:?}",source.kind);
            }
            let measured=measured_size_readonly_styled(ui,&editor,None,None,12.);
            let response=formula_readonly_ui_styled(ui,&editor,None,None,12.);
            assert!(!response.changed());assert!(!response.has_focus());
            let delta=response.rect.size()-measured;
            let pixel=ui.ctx().pixels_per_point().recip();
            assert!(delta.x.abs()<=pixel && delta.y.abs()<=pixel);
            assert!(ui.input(|i|i.events.iter().any(|event|matches!(event,egui::Event::Text(_)))));
            match source.kind {
                NodeKind::Conjugate => {
                    let (offset,child)=&layout.children[0];
                    assert!(offset.y>0.);
                    assert!(layout.primitives.iter().any(|p|matches!(p,Primitive::Line(a,b) if (a.y-b.y).abs()<0.001 && a.y<offset.y && b.x-a.x>=child.size.x)));
                }
                NodeKind::Factorial => assert!(layout.primitives.iter().any(|p|matches!(p,Primitive::Text(_,g) if g.text()=="!"))),
                NodeKind::Transpose => {
                    assert!(layout.children[0].0.y>0.);
                    assert!(layout.primitives.iter().any(|p|matches!(p,Primitive::Text(_,g) if g.text()=="T")));
                }
                NodeKind::Absolute => assert_eq!(layout.primitives.iter().filter(|p|matches!(p,Primitive::Line(..))).count(),2),
                NodeKind::Determinant => assert_eq!(layout.primitives.iter().filter(|p|matches!(p,Primitive::Line(..))).count(),4),
                _=>{}
            }
        }).drop_without_applying_deltas();
        assert_eq!(editor.to_portable().unwrap(), source);
    }
}

#[test]
fn native_array_operators_edit_then_calculate_without_becoming_named_calls() {
    let cases = [
        (unary(NodeKind::Transpose, matrix()), vec![1., 3., 2., 4.]),
        (
            PortableTree {
                kind: NodeKind::Row,
                children: vec![matrix(), number("1")],
            },
            vec![3., 4.],
        ),
        (
            PortableTree {
                kind: NodeKind::Column,
                children: vec![matrix(), number("1")],
            },
            vec![2., 4.],
        ),
    ];
    for (tree, expected) in cases {
        let xml = crate::worksheet_math::tree_to_mathml(&tree).unwrap();
        let restored = crate::worksheet_math::tree_from_mathml(&xml).unwrap();
        let mut editor = Editor::from_portable(&restored).unwrap();
        assert!(input_text_with_actions(&mut editor, "=").evaluation_requested);
        let mathcraft_compat::Value::Matrix { elements, .. } = calculated(&editor) else {
            panic!()
        };
        assert_eq!(
            elements.iter().map(|q| q.value.re).collect::<Vec<_>>(),
            expected
        );
    }
    for kind in [NodeKind::Absolute, NodeKind::Determinant] {
        let mut editor = Editor::from_portable(&unary(kind, matrix())).unwrap();
        assert!(input_text_with_actions(&mut editor, "=").evaluation_requested);
        let mathcraft_compat::Value::Scalar { quantity } = calculated(&editor) else {
            panic!()
        };
        assert_eq!(quantity.value.re, -2.);
    }
    let complex = PortableTree {
        kind: NodeKind::Binary(BinaryOp::Add),
        children: vec![
            number("2"),
            PortableTree {
                kind: NodeKind::ImaginarySymbol {
                    value: "3".into(),
                    symbol: "j".into(),
                },
                children: vec![],
            },
        ],
    };
    let mut editor = Editor::from_portable(&unary(NodeKind::Conjugate, complex)).unwrap();
    assert!(input_text_with_actions(&mut editor, "=").evaluation_requested);
    let mathcraft_compat::Value::Scalar { quantity } = calculated(&editor) else {
        panic!()
    };
    assert_eq!((quantity.value.re, quantity.value.im), (2., -3.));
}

#[test]
fn row_and_column_fences_have_original_orientation_and_distinct_source_index_hits() {
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    ctx.run_ui(egui::RawInput::default(), |ui| {
        for kind in [NodeKind::Row, NodeKind::Column] {
            let editor = Editor::from_portable(&PortableTree {
                kind: kind.clone(),
                children: vec![matrix(), number("12")],
            })
            .unwrap();
            let layout = measure(ui.painter(), editor.arena(), editor.root(), FONT_SIZE);
            let (offset, index) = &layout.children[1];
            assert!(offset.x >= layout.children[0].1.size.x);
            assert!(offset.y < layout.children[0].0.y);
            assert_eq!(index.primitives.len(), 4);
            let source_index = editor.arena().get(editor.root()).unwrap().children()[1];
            let mut hits = vec![];
            collect_hits(&layout, Pos2::ZERO, &mut hits);
            let hit = hits.iter().find(|hit| hit.node == source_index).unwrap();
            assert_eq!(hit.atom.as_ref().unwrap().text(), "12");
            let Primitive::Line(a, b) = index.primitives[0] else {
                panic!()
            };
            if kind == NodeKind::Row {
                assert!(b.y < a.y, "upper angle points upward");
                assert!(b.x > a.x);
            } else {
                assert!(b.x < a.x, "left angle points left");
                assert!(b.y > a.y);
            }
        }
    })
    .drop_without_applying_deltas();
}

#[test]
fn matrix_selection_rejects_new_operator_commands_without_dirty_or_undo() {
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    let mut editor = Editor::from_portable(&matrix()).unwrap();
    let root = editor.root();
    editor.select_matrix_cells(root, 0, 3);
    let before = editor.to_portable().unwrap();
    for event in [
        egui::Event::Text("!".into()),
        key(Key::T, command_shift()),
        key(Key::R, command_shift()),
        key(Key::C, command_shift()),
        key(Key::Num9, command_shift()),
        key(Key::Pipe, command_shift()),
    ] {
        ctx.run_ui(
            egui::RawInput {
                events: vec![event],
                ..Default::default()
            },
            |ui| {
                let response = formula_ui_with_actions(ui, &mut editor, None, true);
                assert!(!response.response.changed());
                assert!(!response.evaluation_requested);
            },
        )
        .drop_without_applying_deltas();
        assert_eq!(editor.to_portable().unwrap(), before);
    }
    assert!(!editor.undo());
}

#[test]
fn original_prohibited_nested_script_operands_receive_visual_groups_without_source_mutation() {
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    for (operator, child) in [
        (NodeKind::Transpose, NodeKind::Row),
        (NodeKind::Transpose, NodeKind::Column),
        (NodeKind::Row, NodeKind::Transpose),
        (NodeKind::Row, NodeKind::Column),
        (NodeKind::Column, NodeKind::Row),
    ] {
        let child = if child == NodeKind::Transpose {
            unary(child, matrix())
        } else {
            PortableTree {
                kind: child,
                children: vec![matrix(), number("0")],
            }
        };
        let source = if operator == NodeKind::Transpose {
            unary(operator, child)
        } else {
            PortableTree {
                kind: operator,
                children: vec![child, number("1")],
            }
        };
        let editor = Editor::from_portable(&source).unwrap();
        ctx.run_ui(egui::RawInput::default(), |ui| {
            let layout = measure(ui.painter(), editor.arena(), editor.root(), FONT_SIZE);
            assert!(
                layout.children[0]
                    .1
                    .primitives
                    .iter()
                    .any(|p| matches!(p, Primitive::Line(..))),
                "visual parentheses required for {:?}",
                source
            );
        })
        .drop_without_applying_deltas();
        assert_eq!(editor.to_portable().unwrap(), source);
    }
}

#[test]
fn named_function_calls_remain_separate_from_operator_shortcuts() {
    for name in ["factorial", "transpose", "conj", "matrow", "matcol", "norm"] {
        let mut editor = Editor::new();
        input_text(&mut editor, &format!("{name}("));
        let tree = editor.to_portable().unwrap();
        assert!(matches!(tree.kind, NodeKind::Call { .. }));
        assert!(matches!(&tree.children[0].kind,NodeKind::Identifier(value) if value==name));
    }
}
