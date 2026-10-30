use super::*;
use eframe::egui::TextBuffer;
use mathfolio_document::canvas::{PlotScaling, PlotSource};
use mathfolio_editor::{BinaryOp, NodeKind, PortableTree};

fn fixture(nx: usize, ny: usize) -> PlotSpec {
    PlotSpec {
        source: PlotSource::axis_lists(
            (0..nx).map(|i| format!("x{i}")).collect(),
            (0..ny).map(|i| format!("y{i}")).collect(),
            vec![PlotTraceAppearance::default(); nx.max(ny)],
        )
        .unwrap(),
        ..Default::default()
    }
}
fn field(spec: &PlotSpec, x: bool, index: usize) -> AxisField {
    fields(spec)
        .into_iter()
        .find(|f| f.x == x && f.index == index)
        .unwrap()
}
fn typed() -> PlotScaling {
    PlotScaling::from_tree(PortableTree {
        kind: NodeKind::NativeIdentifier {
            name: "V".into(),
            labels: "UNIT".into(),
            contextual: Some(true),
            subscript: Some("dc𝛼".into()),
        },
        children: vec![],
    })
    .unwrap()
    .unwrap()
}
fn empty_y(spec: &mut PlotSpec, index: usize) {
    spec.axis_lists_mut().unwrap().y[index]
        .source
        .as_legacy_text_mut()
        .unwrap()
        .clear();
}

#[test]
fn insertion_uses_actual_part_and_edge_without_splitting_and_new_fields_are_editable() {
    for (part, left, empty, index) in [
        (PlotFieldPart::Expression, true, false, 0),
        (PlotFieldPart::Expression, false, false, 1),
        (PlotFieldPart::Scaling, true, false, 1),
        (PlotFieldPart::Expression, true, true, 1),
    ] {
        let mut source = fixture(1, 2);
        if empty {
            empty_y(&mut source, 0);
        }
        let old = source.clone();
        let mut edit = InlinePlotEdit::default();
        edit.prepare_fields(&source);
        let result = edit
            .insert_slot(&source, field(&source, false, 0), part, left)
            .unwrap();
        assert!(result.changed);
        assert_eq!(result.focus.field.index, index);
        assert_eq!(result.focus.part, PlotFieldPart::Expression);
        assert_eq!(result.focus.edge, SlotFocusEdge::Start);
        let rendered = edit.prepare_fields(&source);
        assert_eq!(rendered.len(), 4);
        let id = result.focus.field.id.unwrap();
        let created = edit
            .draft()
            .unwrap()
            .axis_lists()
            .unwrap()
            .y
            .iter()
            .find(|e| e.id == id)
            .unwrap();
        assert!(created.source.is_exact_placeholder() && created.scaling.is_none());
        assert!(matches!(
            edit.scaling_editor(&source, result.focus.field)
                .unwrap()
                .to_portable()
                .unwrap()
                .kind,
            NodeKind::Placeholder
        ));
        edit.field_buffer(&source, result.focus.field)
            .replace_with("new_value");
        assert_eq!(
            super::super::field_expression(edit.draft().unwrap(), result.focus.field),
            Some("new_value")
        );
        assert_eq!(source, old);
        assert_eq!(edit.draft_creations, 1);
    }
}
#[test]
fn empty_removal_selects_scaling_end_or_expression_start_and_last_pair_is_inert() {
    for direction in [Direction::Left, Direction::Right] {
        for index in 0..3 {
            let mut source = fixture(1, 3);
            empty_y(&mut source, index);
            let original = source.clone();
            let mut edit = InlinePlotEdit::default();
            edit.prepare_fields(&source);
            let result = edit
                .remove_empty_slot(
                    &source,
                    field(&source, false, index),
                    PlotFieldPart::Scaling,
                    direction,
                )
                .unwrap()
                .unwrap();
            let expected = if direction == Direction::Left {
                index.saturating_sub(1)
            } else {
                index.min(1)
            };
            assert_eq!(result.focus.field.index, expected);
            assert_eq!(
                (result.focus.part, result.focus.edge),
                if direction == Direction::Left {
                    (PlotFieldPart::Scaling, SlotFocusEdge::End)
                } else {
                    (PlotFieldPart::Expression, SlotFocusEdge::Start)
                }
            );
            assert_eq!(edit.draft().unwrap().axis_equation_count(PlotAxis::Y), 2);
            assert_eq!(source, original);
            assert_eq!(edit.undo_slot().unwrap().part, PlotFieldPart::Scaling);
            assert!(edit.draft().is_none());
            assert!(edit.redo_slot().is_some());
        }
    }
    let source = PlotSpec {
        source: PlotSource::single("", ""),
        ..Default::default()
    };
    let mut edit = InlinePlotEdit::default();
    let f = edit.prepare_fields(&source)[0];
    let outcome = edit
        .remove_empty_slot(&source, f, PlotFieldPart::Expression, Direction::Left)
        .unwrap()
        .unwrap();
    assert!(!outcome.changed);
    assert!(!edit.has_changes());
    assert!(!edit.can_undo_slot());
    assert_eq!(edit.palette_cursor(), 0);
    assert_eq!(edit.draft_creations, 0);
}
#[test]
fn scaling_and_nested_placeholders_are_not_empty_pairs_and_existing_metadata_survives() {
    let mut source = fixture(1, 2);
    empty_y(&mut source, 0);
    source.axis_lists_mut().unwrap().y[0].scaling = Some(typed());
    let mut edit = InlinePlotEdit::default();
    edit.prepare_fields(&source);
    let old = source.clone();
    let f = field(&source, false, 0);
    assert!(
        edit.remove_empty_slot(&source, f, PlotFieldPart::Scaling, Direction::Left)
            .unwrap()
            .is_none()
    );
    edit.insert_slot(&source, f, PlotFieldPart::Scaling, true)
        .unwrap();
    assert_eq!(
        edit.draft().unwrap().axis_lists().unwrap().y[0].scaling,
        Some(typed())
    );
    assert_eq!(source, old);
    let mut nested = source.clone();
    nested.axis_lists_mut().unwrap().y[0].scaling = PlotScaling::from_tree(PortableTree {
        kind: NodeKind::Group,
        children: vec![PortableTree {
            kind: NodeKind::Placeholder,
            children: vec![],
        }],
    })
    .unwrap();
    let mut edit = InlinePlotEdit::default();
    edit.prepare_fields(&nested);
    assert!(
        edit.remove_empty_slot(
            &nested,
            field(&nested, false, 0),
            PlotFieldPart::Scaling,
            Direction::Right
        )
        .unwrap()
        .is_none()
    );
}
#[test]
fn local_undo_redo_keeps_prior_text_allocator_appearance_and_palette_is_not_rewound() {
    let source = fixture(1, 1);
    let mut edit = InlinePlotEdit::default();
    edit.prepare_fields(&source);
    let f = field(&source, false, 0);
    edit.field_buffer(&source, f).replace_with("prior_input");
    let prior = edit.draft().unwrap().clone();
    let created = edit
        .insert_slot(&source, f, PlotFieldPart::Expression, false)
        .unwrap();
    let after = edit.draft().unwrap().clone();
    let cursor = edit.palette_cursor();
    assert_eq!(cursor, 2);
    edit.field_buffer(&source, created.focus.field)
        .replace_with("later");
    assert!(!edit.can_undo_slot());
    edit.field_buffer(&source, created.focus.field)
        .replace_with("");
    assert!(edit.can_undo_slot());
    assert_eq!(edit.undo_slot().unwrap().field.id, f.id);
    assert_eq!(edit.draft(), Some(&prior));
    assert_eq!(edit.palette_cursor(), cursor);
    assert!(edit.can_redo_slot());
    assert_eq!(edit.redo_slot().unwrap(), created.focus);
    assert_eq!(edit.draft(), Some(&after));
    assert_eq!(edit.palette_cursor(), cursor);
    edit.undo_slot();
    edit.field_buffer(&source, f).replace_with("new_branch");
    assert!(!edit.can_redo_slot());
    edit.cancel();
    assert_eq!(edit.palette_cursor(), cursor);
    let replacement = edit
        .insert_slot(&source, f, PlotFieldPart::Expression, false)
        .unwrap();
    assert!(replacement.focus.field.id.unwrap() > created.focus.field.id.unwrap());
    assert_eq!(edit.palette_cursor(), 3);
    assert_eq!(
        edit.draft().unwrap().axis_lists().unwrap().appearances[1]
            .style
            .color,
        [255, 0, 0, 255]
    );
}
#[test]
fn same_id_external_source_change_is_rejected_but_resize_preserves_the_draft() {
    for scaling in [false, true] {
        let source = fixture(1, 1);
        let mut edit = InlinePlotEdit::default();
        let f = edit.prepare_fields(&source)[0];
        edit.field_buffer(&source, f).replace_with("draft");
        let mut external = source.clone();
        if scaling {
            external.axis_lists_mut().unwrap().y[0].scaling = Some(typed());
        } else {
            external.axis_lists_mut().unwrap().y[0].source = "external".into();
        }
        assert!(edit.validate_source(&external).is_err());
        edit.prepare_fields(&external);
        assert!(!edit.has_changes());
        assert!(edit.validate_source(&external).is_ok());
    }
    let mut source = fixture(1, 1);
    let mut edit = InlinePlotEdit::default();
    let f = edit.prepare_fields(&source)[0];
    edit.field_buffer(&source, f).replace_with("draft");
    source.set_outer_size(600., 400.).unwrap();
    assert!(edit.dimensions(600., 400.));
    assert!(edit.validate_source(&source).is_ok());
    edit.prepare_fields(&source);
    assert_eq!(edit.draft().unwrap().height, 400.);
    assert_eq!(
        edit.draft()
            .unwrap()
            .primary_y_expression()
            .as_legacy_text()
            .unwrap(),
        "draft"
    );
}
#[test]
fn structural_failure_preserves_redo_source_palette_and_editor_caret() {
    let source = fixture(1, 1);
    let mut edit = InlinePlotEdit::default();
    let f = edit.prepare_fields(&source)[0];
    edit.scaling_editor(&source, f).unwrap();
    edit.insert_slot(&source, f, PlotFieldPart::Expression, false)
        .unwrap();
    edit.undo_slot();
    assert!(edit.can_redo_slot());
    let cursor = edit.palette_cursor();
    let caret = edit.scaling_editor(&source, f).unwrap().caret();
    let invalid = AxisField {
        id: Some(u64::MAX),
        ..f
    };
    assert!(
        edit.insert_slot(&source, invalid, PlotFieldPart::Expression, false)
            .is_err()
    );
    assert!(edit.draft().is_none());
    assert!(edit.can_redo_slot());
    assert_eq!(edit.palette_cursor(), cursor);
    assert_eq!(edit.scaling_editor(&source, f).unwrap().caret(), caret);
    let capped = fixture(1, 16);
    let mut limited = InlinePlotEdit::default();
    let f = limited.prepare_fields(&capped)[0];
    assert!(
        limited
            .insert_slot(&capped, f, PlotFieldPart::Expression, false)
            .is_err()
    );
    assert!(limited.draft().is_none());
    assert_eq!(limited.palette_cursor(), 0);
    assert_eq!(limited.draft_creations, 0);
    let mut overflow = InlinePlotEdit::default();
    overflow.prepare_fields(&source);
    overflow.ensure_palette_cursor(usize::MAX);
    assert!(
        overflow
            .insert_slot(
                &source,
                field(&source, false, 0),
                PlotFieldPart::Expression,
                false
            )
            .is_err()
    );
    assert!(overflow.draft().is_none());
    assert_eq!(overflow.palette_cursor(), usize::MAX);
}
#[test]
fn legacy_success_migrates_scaling_without_touching_original_and_failure_does_not_migrate() {
    use mathfolio_document::canvas::PlotTraceSpec;
    let source = PlotSpec {
        source: PlotSource::legacy(
            "unused_x",
            "unused_y",
            vec![PlotTraceSpec {
                x_expression: "x".into(),
                y_expression: "y".into(),
                y_scaling: Some(typed()),
                ..Default::default()
            }],
        ),
        ..Default::default()
    };
    let original = source.clone();
    let mut edit = InlinePlotEdit::default();
    let f = edit.prepare_fields(&source)[0];
    let new = edit
        .insert_slot(&source, f, PlotFieldPart::Scaling, false)
        .unwrap();
    assert_eq!(edit.prepare_fields(&source).len(), 3);
    assert_eq!(
        edit.draft().unwrap().axis_lists().unwrap().y[0].scaling,
        Some(typed())
    );
    assert!(matches!(
        edit.scaling_editor(&source, new.focus.field)
            .unwrap()
            .to_portable()
            .unwrap()
            .kind,
        NodeKind::Placeholder
    ));
    assert_eq!(source, original);
    edit.undo_slot();
    assert!(edit.draft().is_none());
    assert_eq!(edit.prepare_fields(&source).len(), 2);
    assert!(edit.can_redo_slot());
}

#[test]
fn baseline_is_lazy_and_id_floor_failure_does_not_consume_style_or_history() {
    let source = fixture(1, 1);
    let mut edit = InlinePlotEdit::default();
    for _ in 0..20 {
        edit.prepare_fields(&source);
        assert!(edit.source_baseline.is_none());
    }
    let field = field(&source, false, 0);
    edit.ensure_equation_id_floor(u64::MAX);
    assert!(
        edit.insert_slot(&source, field, PlotFieldPart::Expression, false)
            .is_err()
    );
    assert!(edit.draft().is_none());
    assert!(edit.source_baseline.is_none());
    assert!(!edit.can_undo_slot());
    assert_eq!(edit.palette_cursor(), 0);
    assert_eq!(edit.equation_id_floor(), u64::MAX);
}

#[test]
fn removed_scaling_cache_is_retired_and_undo_restores_its_exact_caret_and_widget() {
    let source = fixture(1, 1);
    let mut edit = InlinePlotEdit::default();
    let f = edit.prepare_fields(&source)[0];
    let created = edit
        .insert_slot(&source, f, PlotFieldPart::Expression, false)
        .unwrap()
        .focus
        .field;
    edit.prepare_fields(&source);
    let editor = edit.scaling_editor(&source, created).unwrap();
    let root = editor.root();
    editor.place_caret(root, Direction::Left).unwrap();
    let caret = editor.caret();
    let id = eframe::egui::Id::new("new-scale-widget");
    edit.record_scaling_widget(created, id);
    edit.remove_empty_slot(&source, created, PlotFieldPart::Scaling, Direction::Left)
        .unwrap()
        .unwrap();
    assert!(edit.scaling_widget_id(created).is_none());
    edit.undo_slot().unwrap();
    edit.prepare_fields(&source);
    assert_eq!(edit.scaling_widget_id(created), Some(id));
    assert_eq!(
        edit.scaling_editor(&source, created).unwrap().caret(),
        caret
    );
}
#[test]
fn invalid_or_composing_input_blocks_the_whole_structural_transaction() {
    let source = fixture(1, 1);
    let mut edit = InlinePlotEdit::default();
    let f = edit.prepare_fields(&source)[0];
    let editor = edit.scaling_editor(&source, f).unwrap();
    editor.set_expressions_only(false);
    editor.select_subtree(editor.root()).unwrap();
    editor
        .replace_selection(&PortableTree {
            kind: NodeKind::Binary(BinaryOp::Define),
            children: vec![
                PortableTree {
                    kind: NodeKind::Identifier("x".into()),
                    children: vec![],
                },
                PortableTree {
                    kind: NodeKind::Number("3".into()),
                    children: vec![],
                },
            ],
        })
        .unwrap();
    assert!(edit.stage_scaling(&source, f).is_err());
    assert!(
        edit.insert_slot(&source, f, PlotFieldPart::Scaling, false)
            .is_err()
    );
    assert!(edit.draft().is_none());
    assert_eq!(edit.palette_cursor(), 0);
    assert!(!edit.can_undo_slot());
    edit.cancel();
    edit.prepare_fields(&source);
    edit.scaling_editor(&source, f).unwrap();
    edit.observe_scaling_ime(
        f,
        &[eframe::egui::Event::Ime(eframe::egui::ImeEvent::Preedit {
            text: "한".into(),
            active_range_chars: None,
        })],
    );
    assert!(
        edit.insert_slot(&source, f, PlotFieldPart::Scaling, false)
            .is_err()
    );
    assert!(edit.is_composing());
    assert!(edit.draft().is_none());
    assert_eq!(edit.palette_cursor(), 0);
}
