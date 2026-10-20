//! CPU-only widget input. No native application, window or screenshot is used.
use super::*;

fn number(value: usize) -> PortableTree {
    PortableTree {
        kind: NodeKind::Number(value.to_string()),
        children: vec![],
    }
}
fn matrix(rows: usize, cols: usize) -> PortableTree {
    PortableTree {
        kind: NodeKind::Matrix {
            rows,
            cols,
            display: None,
        },
        children: (1..=rows * cols).map(number).collect(),
    }
}
fn editor() -> Editor {
    Editor::from_portable(&PortableTree {
        kind: NodeKind::Binary(BinaryOp::Add),
        children: vec![matrix(3, 3), number(100)],
    })
    .unwrap()
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
fn key(key: Key, modifiers: egui::Modifiers) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers,
    }
}

#[test]
fn matrix_drag_copy_delete_preserves_outside_operand_and_one_undo_restores_it() {
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    let mut editor = editor();
    let before = editor.to_portable().unwrap();
    let root = editor.root();
    let matrix = editor.arena().get(root).unwrap().children()[0];
    let mut points = [Pos2::ZERO; 2];
    ctx.run_ui(egui::RawInput::default(), |ui| {
        let response = formula_ui_with_actions(ui, &mut editor, None, true).response;
        let layout = measure(ui.painter(), editor.arena(), root, FONT_SIZE);
        let mut hits = vec![];
        collect_hits(
            &layout,
            response.rect.min + vec2(PADDING, PADDING),
            &mut hits,
        );
        let cells = editor.arena().get(matrix).unwrap().children();
        points = [0, 4].map(|index| {
            hits.iter()
                .find(|hit| hit.node == cells[index])
                .unwrap()
                .rect
                .center()
        });
    })
    .drop_without_applying_deltas();
    for events in [
        vec![
            egui::Event::PointerMoved(points[0]),
            egui::Event::PointerButton {
                pos: points[0],
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ],
        vec![egui::Event::PointerMoved(points[1])],
        vec![egui::Event::PointerButton {
            pos: points[1],
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }],
    ] {
        send(&ctx, &mut editor, events).drop_without_applying_deltas();
    }
    let selection = editor.matrix_selection().expect("dragged matrix rectangle");
    assert_eq!(
        (selection.matrix, selection.anchor, selection.focus),
        (matrix, 0, 4)
    );
    assert_eq!(editor.to_portable().unwrap(), before);
    let output = send(&ctx, &mut editor, vec![egui::Event::Copy]);
    let copy = output
        .platform_output
        .commands
        .iter()
        .find_map(|command| {
            if let egui::OutputCommand::CopyText(text) = command {
                Some(text)
            } else {
                None
            }
        })
        .expect("Copy produces structural clipboard");
    let copied: PortableTree =
        serde_json::from_str(copy.strip_prefix(TREE_CLIPBOARD_PREFIX).unwrap()).unwrap();
    assert_eq!(
        copied.kind,
        NodeKind::Matrix {
            rows: 2,
            cols: 2,
            display: None
        }
    );
    assert_eq!(
        copied.children,
        vec![number(1), number(2), number(4), number(5)]
    );
    send(
        &ctx,
        &mut editor,
        vec![key(Key::Delete, egui::Modifiers::NONE)],
    )
    .drop_without_applying_deltas();
    let edited = editor.to_portable().unwrap();
    assert_eq!(edited.children[1], number(100));
    assert_eq!(
        edited.children[0].kind,
        NodeKind::Matrix {
            rows: 3,
            cols: 3,
            display: None
        }
    );
    for index in [0, 1, 3, 4] {
        assert_eq!(
            edited.children[0].children[index].kind,
            NodeKind::Placeholder
        );
    }
    for index in [2, 5, 6, 7, 8] {
        assert_eq!(
            edited.children[0].children[index],
            before.children[0].children[index]
        );
    }
    assert!(editor.undo());
    assert_eq!(editor.to_portable().unwrap(), before);
    assert!(!editor.undo());
}

#[test]
fn matrix_range_paste_and_typed_text_are_native_noops_without_undo_or_evaluation() {
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    let mut editor = editor();
    let before = editor.to_portable().unwrap();
    let matrix_id = editor.arena().get(editor.root()).unwrap().children()[0];
    assert!(editor.select_matrix_cells(matrix_id, 0, 4));
    let clipboard = format!(
        "{TREE_CLIPBOARD_PREFIX}{}",
        serde_json::to_string(&matrix(2, 2)).unwrap()
    );
    for event in [
        egui::Event::Paste(clipboard),
        egui::Event::Paste("9+7=".into()),
        egui::Event::Text("x+2=".into()),
        key(Key::ArrowLeft, egui::Modifiers::SHIFT),
        key(Key::ArrowRight, egui::Modifiers::SHIFT),
        key(Key::Equals, egui::Modifiers::COMMAND),
        key(Key::M, egui::Modifiers::COMMAND),
    ] {
        ctx.run_ui(
            egui::RawInput {
                events: vec![event],
                ..Default::default()
            },
            |ui| {
                let response = formula_ui_with_actions(ui, &mut editor, None, true);
                assert!(!response.evaluation_requested);
                assert!(
                    !response.response.changed(),
                    "native no-op must not mark a worksheet draft dirty"
                );
            },
        )
        .drop_without_applying_deltas();
        assert_eq!(editor.to_portable().unwrap(), before);
        assert!(editor.matrix_selection().is_some());
    }
    assert!(!editor.undo());
}

#[test]
fn transformed_nested_matrix_drag_cut_preserves_its_outer_cell_and_siblings() {
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    let mut outer = matrix(2, 2);
    outer.children[0] = matrix(2, 2);
    let mut editor = Editor::from_portable(&outer).unwrap();
    let original = editor.to_portable().unwrap();
    let outer_id = editor.root();
    let inner = editor.arena().get(outer_id).unwrap().children()[0];
    let layer = egui::LayerId::new(
        egui::Order::Background,
        egui::Id::new("matrix-zoomed-layer"),
    );
    let transform = egui::emath::TSTransform::from_translation(vec2(201., 117.))
        * egui::emath::TSTransform::from_scaling(2.);
    let mut points = [Pos2::ZERO; 2];
    ctx.run_ui(egui::RawInput::default(), |ui| {
        ctx.set_sublayer(ui.layer_id(), layer);
        ctx.set_transform_layer(layer, transform);
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("matrix-layer-content")
                .layer_id(layer)
                .max_rect(Rect::from_min_size(pos2(17., 23.), vec2(500., 500.))),
        );
        let response = formula_ui_with_actions(&mut child, &mut editor, None, true).response;
        let layout = measure(child.painter(), editor.arena(), outer_id, FONT_SIZE);
        let mut hits = vec![];
        collect_hits(
            &layout,
            response.rect.min + vec2(PADDING, PADDING),
            &mut hits,
        );
        let cells = editor.arena().get(inner).unwrap().children();
        points = [0, 3].map(|index| {
            transform
                * hits
                    .iter()
                    .find(|hit| hit.node == cells[index])
                    .unwrap()
                    .rect
                    .center()
        });
    })
    .drop_without_applying_deltas();
    let run = |editor: &mut Editor, events| {
        ctx.run_ui(
            egui::RawInput {
                events,
                ..Default::default()
            },
            |ui| {
                ctx.set_sublayer(ui.layer_id(), layer);
                ctx.set_transform_layer(layer, transform);
                let mut child = ui.new_child(
                    egui::UiBuilder::new()
                        .id_salt("matrix-layer-content")
                        .layer_id(layer)
                        .max_rect(Rect::from_min_size(pos2(17., 23.), vec2(500., 500.))),
                );
                assert!(
                    !formula_ui_with_actions(&mut child, editor, None, true).evaluation_requested
                );
            },
        )
    };
    for events in [
        vec![
            egui::Event::PointerMoved(points[0]),
            egui::Event::PointerButton {
                pos: points[0],
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ],
        vec![egui::Event::PointerMoved(points[1])],
        vec![egui::Event::PointerButton {
            pos: points[1],
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }],
    ] {
        run(&mut editor, events).drop_without_applying_deltas();
    }
    assert_eq!(editor.matrix_selection().unwrap().matrix, inner);
    assert_eq!(editor.to_portable().unwrap(), original);
    let output = run(&mut editor, vec![egui::Event::Cut]);
    let text = output
        .platform_output
        .commands
        .iter()
        .find_map(|command| {
            if let egui::OutputCommand::CopyText(text) = command {
                Some(text)
            } else {
                None
            }
        })
        .unwrap();
    let copied: PortableTree =
        serde_json::from_str(text.strip_prefix(TREE_CLIPBOARD_PREFIX).unwrap()).unwrap();
    assert_eq!(copied, original.children[0]);
    let changed = editor.to_portable().unwrap();
    assert_eq!(
        changed.kind,
        NodeKind::Matrix {
            rows: 2,
            cols: 2,
            display: None
        }
    );
    assert_eq!(
        changed.children[0].kind,
        NodeKind::Matrix {
            rows: 1,
            cols: 1,
            display: None
        }
    );
    assert_eq!(changed.children[0].children[0].kind, NodeKind::Placeholder);
    assert_eq!(changed.children[1..], original.children[1..]);
    assert!(editor.undo());
    assert_eq!(editor.to_portable().unwrap(), original);
    assert!(!editor.undo());
}

#[test]
fn matrix_backspace_uses_axis_removal_while_clear_keeps_shape_and_enter_is_parent_owned() {
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    let mut editor = editor();
    let original = editor.to_portable().unwrap();
    let matrix = editor.arena().get(editor.root()).unwrap().children()[0];
    editor.select_matrix_cells(matrix, 0, 7);
    send(
        &ctx,
        &mut editor,
        vec![key(Key::Backspace, egui::Modifiers::NONE)],
    )
    .drop_without_applying_deltas();
    let changed = editor.to_portable().unwrap();
    assert_eq!(
        changed.children[0].kind,
        NodeKind::Matrix {
            rows: 3,
            cols: 1,
            display: None
        }
    );
    assert_eq!(
        changed.children[0].children,
        vec![number(3), number(6), number(9)]
    );
    assert_eq!(changed.children[1], number(100));
    assert!(editor.undo());
    assert_eq!(editor.to_portable().unwrap(), original);
    editor.select_matrix_cells(matrix, 0, 7);
    assert!(editor.clear_matrix_cells());
    let changed = editor.to_portable().unwrap();
    assert_eq!(
        changed.children[0].kind,
        NodeKind::Matrix {
            rows: 3,
            cols: 3,
            display: None
        }
    );
    for index in [0, 1, 3, 4, 6, 7] {
        assert_eq!(
            changed.children[0].children[index].kind,
            NodeKind::Placeholder
        );
    }
    let mut remaining = false;
    ctx.run_ui(
        egui::RawInput {
            events: vec![key(Key::Enter, egui::Modifiers::NONE)],
            ..Default::default()
        },
        |ui| {
            let response = formula_ui_with_actions(ui, &mut editor, None, true);
            assert!(!response.evaluation_requested);
            assert!(!response.response.changed());
            remaining = ui.input(|input| {
                input.events.iter().any(|event| {
                    matches!(
                        event,
                        egui::Event::Key {
                            key: Key::Enter,
                            ..
                        }
                    )
                })
            });
        },
    )
    .drop_without_applying_deltas();
    assert!(remaining, "Enter commits at the worksheet boundary");
    assert!(editor.undo());
    assert_eq!(editor.to_portable().unwrap(), original);
}

#[test]
fn matrix_corner_resolution_uses_the_deepest_shared_matrix() {
    let mut outer = matrix(2, 2);
    outer.children[0] = matrix(2, 2);
    let editor = Editor::from_portable(&outer).unwrap();
    let outer = editor.root();
    let children = editor.arena().get(outer).unwrap().children();
    let inner = children[0];
    let nested = editor.arena().get(inner).unwrap().children();
    assert_eq!(
        matrix_cell_corners(editor.arena(), nested[0], nested[3]),
        Some((inner, 0, 3))
    );
    assert_eq!(
        matrix_cell_corners(editor.arena(), nested[0], children[3]),
        Some((outer, 0, 3))
    );
    assert_eq!(
        matrix_cell_corners(editor.arena(), nested[0], nested[0]),
        None
    );
}
