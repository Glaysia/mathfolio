//! Static original pair-nary layout contracts, verified without a GUI.
use super::*;

fn atom(kind: NodeKind) -> PortableTree {
    PortableTree {
        kind,
        children: vec![],
    }
}
fn number(value: i32) -> PortableTree {
    atom(NodeKind::Number(value.to_string()))
}
fn program(value: PortableTree) -> PortableTree {
    PortableTree {
        kind: NodeKind::Program { statement_count: 1 },
        children: vec![value],
    }
}
fn chain(alternate: bool) -> PortableTree {
    PortableTree {
        kind: NodeKind::If {
            branch_count: 3,
            has_else: true,
            alternate,
        },
        children: vec![
            number(0),
            program(number(12)),
            number(1),
            program(number(34)),
            number(1),
            program(number(56)),
            program(number(78)),
        ],
    }
}

#[test]
fn native_pair_words_share_test_baselines_and_bodies_are_on_following_indented_rows() {
    let context = egui::Context::default();
    crate::configure(&context);
    let output = context.run_ui(Default::default(), |ui| {
        for scale in [0.75, 1., 2.] {
            for alternate in [false, true] {
                let editor = Editor::from_portable(&chain(alternate)).unwrap();
                let size = FONT_SIZE * scale;
                let layout = measure(ui.painter(), editor.arena(), editor.root(), size);
                let words: Vec<_> = layout.primitives.iter().filter_map(|primitive| match primitive {
                    Primitive::Text(offset, text) => Some((*offset, text)),
                    _ => None,
                }).collect();
                assert_eq!(words.iter().map(|(_, text)| text.text()).collect::<Vec<_>>(), ["if", if alternate { "also if" } else { "else if" }, if alternate { "also if" } else { "else if" }, "else"]);
                assert_eq!(layout.children.len(), 7);
                for (index, (word_offset, word)) in words[..3].iter().enumerate() {
                    let (test_offset, test) = &layout.children[index * 2];
                    let (body_offset, body) = &layout.children[index * 2 + 1];
                    assert!((word_offset.y + text_baseline(word) - test_offset.y - test.baseline).abs() < 0.001);
                    assert!((test_offset.x - word.size().x - 4. * scale).abs() < 0.001);
                    assert!((body_offset.x - 8. * scale).abs() < 0.001);
                    assert!(body_offset.y + 0.001 >= test_offset.y + test.size.y);
                    assert_eq!(body.primitives.iter().filter(|p| matches!(p, Primitive::Line(..))).count(), 2);
                    assert!(!body.primitives.iter().any(|p| matches!(p, Primitive::MinorFence(..))));
                    if index < 2 {
                        assert!(words[index + 1].0.y + 0.001 >= body_offset.y + body.size.y);
                    }
                }
                assert!(layout.children[6].0.y >= words[3].0.y + words[3].1.size().y - 0.001);
                assert!(layout.primitives.iter().any(|p| matches!(p, Primitive::MinorFence(a,b) if a.x==layout.size.x && b.y==layout.size.y)));
                let outer = Editor::from_portable(&program(chain(alternate))).unwrap();
                let outer = measure(ui.painter(), outer.arena(), outer.root(), size);
                assert!(outer.primitives.iter().any(|p| matches!(p, Primitive::MinorFence(..))));
            }
        }
    });
    output.drop_without_applying_deltas();
}

#[test]
fn pair_word_hits_keep_parts_after_parentheses_and_toggle_the_entire_chain_in_one_undo() {
    let context = egui::Context::default();
    crate::configure(&context);
    let output = context.run_ui(Default::default(), |ui| {
        let original = chain(false);
        let mut editor = Editor::from_portable(&original).unwrap();
        let root = editor.root();
        let layout = measure(ui.painter(), editor.arena(), root, FONT_SIZE);
        let original_rect = layout.operator_hits[1].1;
        let layout = visual_parentheses(ui.painter(), layout, FONT_SIZE);
        let mut hits = vec![];
        collect_hits(&layout, pos2(40., 60.), &mut hits);
        for part in 0..4 {
            let expected = hits
                .iter()
                .find(|hit| hit.operator_part == Some(part))
                .unwrap();
            let chosen = hit_node(&hits, expected.rect.center()).unwrap();
            assert_eq!(chosen.operator_part, Some(part));
            assert_eq!(chosen.node, root);
        }
        let middle = hits
            .iter()
            .find(|hit| hit.operator_part == Some(1))
            .unwrap();
        assert!(middle.rect.min.x > original_rect.min.x + 40.);
        editor
            .place_if_operator(middle.node, middle.operator_part.unwrap())
            .unwrap();
        let caret = editor.caret();
        assert!(editor.insert_also_if());
        assert_eq!(editor.to_portable().unwrap(), chain(true));
        assert!(editor.undo());
        assert_eq!(editor.to_portable().unwrap(), original);
        assert_eq!(editor.caret(), caret);
        assert!(editor.redo());
        assert_eq!(editor.to_portable().unwrap(), chain(true));
    });
    output.drop_without_applying_deltas();
}

#[test]
fn actual_pointer_click_selects_the_middle_pair_operator() {
    let context = egui::Context::default();
    crate::configure(&context);
    let mut editor = Editor::from_portable(&chain(false)).unwrap();
    let mut point = Pos2::ZERO;
    let output = context.run_ui(Default::default(), |ui| {
        let response = formula_ui_with_focus(ui, &mut editor, None, true);
        let layout = measure(ui.painter(), editor.arena(), editor.root(), FONT_SIZE);
        let mut hits = vec![];
        collect_hits(
            &layout,
            response.rect.min + vec2(PADDING, PADDING),
            &mut hits,
        );
        point = hits
            .iter()
            .find(|hit| hit.operator_part == Some(1))
            .unwrap()
            .rect
            .center();
    });
    output.drop_without_applying_deltas();
    let output = context.run_ui(
        egui::RawInput {
            events: vec![
                egui::Event::PointerMoved(point),
                egui::Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
                egui::Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            ..Default::default()
        },
        |ui| {
            formula_ui_with_focus(ui, &mut editor, None, true);
        },
    );
    output.drop_without_applying_deltas();
    assert_eq!(editor.caret().node, editor.root());
    assert_eq!(editor.caret().operator_part, Some(1));
    assert!(editor.insert_also_if());
    assert_eq!(editor.to_portable().unwrap(), chain(true));
}

#[test]
fn try_uses_two_original_headers_and_two_stacked_body_rows_without_extra_body_fences() {
    let context = egui::Context::default();
    crate::configure(&context);
    let tree = PortableTree {
        kind: NodeKind::Try,
        children: vec![program(number(17)), program(number(43))],
    };
    let editor = Editor::from_portable(&tree).unwrap();
    let output = context.run_ui(Default::default(), |ui| {
        for scale in [0.75, 1., 2.] {
            let layout = measure(
                ui.painter(),
                editor.arena(),
                editor.root(),
                FONT_SIZE * scale,
            );
            let headers: Vec<_> = layout
                .primitives
                .iter()
                .filter_map(|primitive| match primitive {
                    Primitive::Text(offset, text) => Some((*offset, text)),
                    _ => None,
                })
                .collect();
            assert_eq!(
                headers
                    .iter()
                    .map(|(_, text)| text.text())
                    .collect::<Vec<_>>(),
                ["try", "on error"]
            );
            assert_eq!(layout.children.len(), 2);
            assert!((layout.baseline - text_baseline(headers[0].1)).abs() < 0.001);
            for index in 0..2 {
                let (offset, body) = &layout.children[index];
                assert!((offset.x - 8. * scale).abs() < 0.001);
                assert!((offset.y - headers[index].0.y - headers[index].1.size().y).abs() < 0.001);
                assert_eq!(
                    body.primitives
                        .iter()
                        .filter(|primitive| matches!(primitive, Primitive::Line(..)))
                        .count(),
                    2
                );
                assert!(
                    !body
                        .primitives
                        .iter()
                        .any(|primitive| matches!(primitive, Primitive::MinorFence(..)))
                );
                if index == 0 {
                    assert!((headers[1].0.y - offset.y - body.size.y).abs() < 0.001);
                }
            }
            assert_eq!(
                layout
                    .primitives
                    .iter()
                    .filter(|primitive| matches!(primitive, Primitive::MinorFence(..)))
                    .count(),
                1
            );
            assert!(
                (layout.size.y - layout.children[1].0.y - layout.children[1].1.size.y).abs()
                    < 0.001
            );
        }
    });
    output.drop_without_applying_deltas();
}

#[test]
fn actual_try_header_click_maps_to_native_node_side_and_keeps_operator_part_null() {
    for (index, side) in [(0, Direction::Left), (1, Direction::Right)] {
        let context = egui::Context::default();
        crate::configure(&context);
        let tree = PortableTree {
            kind: NodeKind::Try,
            children: vec![program(number(17)), program(number(43))],
        };
        let mut editor = Editor::from_portable(&tree).unwrap();
        let mut point = Pos2::ZERO;
        context
            .run_ui(Default::default(), |ui| {
                let response = formula_ui_with_focus(ui, &mut editor, None, true);
                let layout = measure(ui.painter(), editor.arena(), editor.root(), FONT_SIZE);
                let mut hits = vec![];
                collect_hits(
                    &layout,
                    response.rect.min + vec2(PADDING, PADDING),
                    &mut hits,
                );
                point = hits
                    .iter()
                    .find(|hit| hit.operator_part == Some(index))
                    .unwrap()
                    .rect
                    .center();
            })
            .drop_without_applying_deltas();
        context
            .run_ui(
                egui::RawInput {
                    events: vec![
                        egui::Event::PointerMoved(point),
                        egui::Event::PointerButton {
                            pos: point,
                            button: egui::PointerButton::Primary,
                            pressed: true,
                            modifiers: egui::Modifiers::NONE,
                        },
                        egui::Event::PointerButton {
                            pos: point,
                            button: egui::PointerButton::Primary,
                            pressed: false,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                    ..Default::default()
                },
                |ui| {
                    formula_ui_with_focus(ui, &mut editor, None, true);
                },
            )
            .drop_without_applying_deltas();
        assert_eq!(editor.caret().node, editor.root());
        assert_eq!(editor.caret().caret_side, Direction::Indeterminate);
        assert_eq!(editor.caret().node_side, side);
        assert_eq!(editor.caret().operator_part, None);
        assert_eq!(editor.to_portable().unwrap(), tree);
        assert!(!editor.undo());
    }
}
