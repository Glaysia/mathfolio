use super::super::{field_expression, fields};
use super::*;
use eframe::egui::TextBuffer;
use mathcraft_document::canvas::{PlotAxis, PlotSource, PlotTraceAppearance};
use mathcraft_editor::{BinaryOp, Direction};
fn source() -> PlotSpec {
    PlotSpec {
        source: PlotSource::axis_lists(
            vec!["x".into()],
            vec!["x".into(), "2*x".into()],
            vec![PlotTraceAppearance::default(); 2],
        )
        .unwrap(),
        ..Default::default()
    }
}
fn typed(name: &str, label: &str) -> PortableTree {
    PortableTree {
        kind: NodeKind::AnnotatedIdentifier {
            name: name.into(),
            labels: label.into(),
        },
        children: vec![],
    }
}
fn key(key: egui::Key, mut modifiers: egui::Modifiers) -> egui::Event {
    if modifiers.ctrl {
        modifiers.command = true;
    }
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    }
}
fn frame(
    ctx: &egui::Context,
    source: &PlotSpec,
    edit: &mut InlinePlotEdit,
    field: AxisField,
    focus: bool,
    events: Vec<egui::Event>,
) -> (egui::Response, Result<bool, String>) {
    let mut result = None;
    let mut error = None;
    let output = ctx.run_ui(
        egui::RawInput {
            events,
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1200., 1000.),
            )),
            ..Default::default()
        },
        |ui| {
            edit.prepare_fields(source);
            let id = edit.scaling_id(ui, field);
            let active = focus
                || edit
                    .scaling_widget_id(field)
                    .is_some_and(|id| ctx.memory(|m| m.has_focus(id)));
            edit.scaling_focus_changed(ctx, active.then_some(field));
            edit.scaling_editor(source, field).unwrap();
            if active {
                ui.input(|input| edit.observe_scaling_ime(field, &input.events));
            }
            let response = ui
                .push_id(id, |ui| {
                    let editor = edit.scaling_editor(source, field).unwrap();
                    editor.edit_group(|editor| {
                        let revision = editor.arena().revision();
                        let response =
                            crate::formula::formula_ui_styled(ui, editor, None, focus, None, 11.);
                        if editor.arena().revision() != revision {
                            editor.auto_label_scaling_identifiers();
                        }
                        response
                    })
                })
                .inner
                .response;
            edit.record_scaling_widget(field, response.id);
            error = Some(edit.stage_scaling(source, field));
            result = Some(response);
        },
    );
    output.drop_without_applying_deltas();
    (result.unwrap(), error.unwrap())
}
#[test]
fn retained_typed_editor_idle_preserves_raw_metadata_caret_and_has_no_plot_clone() {
    let mut source = source();
    let field = fields(&source)[1];
    let value = PlotScaling::from_tree(PortableTree {
        kind: NodeKind::NativeIdentifier {
            name: "V".into(),
            labels: "VARIABLE".into(),
            contextual: Some(true),
            subscript: Some("dc𝛼".into()),
        },
        children: vec![],
    })
    .unwrap();
    source
        .set_axis_scaling(PlotAxis::Y, 1, value.clone())
        .unwrap();
    let original = source.clone();
    let mut edit = InlinePlotEdit::default();
    edit.prepare_fields(&source);
    let editor = edit.scaling_editor(&source, field).unwrap();
    let root = editor.root();
    editor.place_text_caret(root, 0).unwrap();
    let rev = editor.arena().revision();
    let caret = editor.caret();
    for _ in 0..20 {
        let editor = edit.scaling_editor(&source, field).unwrap();
        assert_eq!(editor.arena().revision(), rev);
        assert_eq!(editor.caret(), caret);
        assert!(!edit.stage_scaling(&source, field).unwrap());
    }
    assert_eq!(source, original);
    assert!(!edit.has_changes());
    assert_eq!(edit.draft_creations, 0);
    assert_eq!(scaling(&source, field).unwrap(), &value);
}
#[test]
fn expression_and_two_typed_scalings_form_one_lazy_aggregate_and_take_retains_editor_caret() {
    let source = source();
    let original = source.clone();
    let mut edit = InlinePlotEdit::default();
    let fields = edit.prepare_fields(&source);
    edit.field_buffer(&source, fields[0]).replace_with("3*x");
    for (field, label) in [(fields[0], "UNIT"), (fields[1], "VARIABLE")] {
        let editor = edit.scaling_editor(&source, field).unwrap();
        editor.load_portable(&typed("m", label)).unwrap();
        assert!(edit.stage_scaling(&source, field).unwrap());
    }
    assert_eq!(edit.draft_creations, 1);
    assert_eq!(source, original);
    assert_eq!(
        field_expression(edit.draft().unwrap(), fields[0]),
        Some("3*x")
    );
    let caret = edit.scaling_editor(&source, fields[1]).unwrap().caret();
    let candidate = edit.take().unwrap();
    assert!(!edit.has_changes());
    assert_eq!(
        edit.scaling_editor(&candidate, fields[1]).unwrap().caret(),
        caret
    );
    let scales: Vec<_> = candidate
        .equation_sources(false)
        .map(|e| e.scaling.unwrap().as_tree().clone())
        .collect();
    assert_eq!(scales, vec![typed("m", "UNIT"), typed("m", "VARIABLE")]);
}
#[test]
fn failed_scaling_stage_blocks_first_edit_and_existing_draft_commit_until_repaired() {
    for prior in [false, true] {
        let source = source();
        let mut edit = InlinePlotEdit::default();
        let field = edit.prepare_fields(&source)[0];
        if prior {
            edit.field_buffer(&source, field).replace_with("confirmed");
        }
        let before = edit.draft().cloned();
        let editor = edit.scaling_editor(&source, field).unwrap();
        editor.set_expressions_only(false);
        editor.select_subtree(editor.root()).unwrap();
        editor
            .replace_selection(&PortableTree {
                kind: NodeKind::Binary(BinaryOp::Define),
                children: vec![typed("bad", "VARIABLE"), typed("m", "UNIT")],
            })
            .unwrap();
        assert!(edit.stage_scaling(&source, field).is_err());
        assert!(edit.has_changes());
        assert!(edit.validation_error().is_some());
        assert!(edit.take().is_none());
        assert_eq!(edit.draft(), before.as_ref());
        let editor = edit.scaling_editor(&source, field).unwrap();
        assert!(editor.undo());
        editor.set_expressions_only(true);
        assert!(!edit.stage_scaling(&source, field).unwrap());
        assert!(edit.validation_error().is_none());
        assert_eq!(edit.has_changes(), prior);
        if prior {
            assert!(edit.take().is_some());
        } else {
            assert!(edit.take().is_none());
        }
    }
}
#[test]
fn actual_literal_subscript_and_label_keys_keep_typed_source_without_affecting_other_slots() {
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    let mut source = source();
    let field = fields(&source)[1];
    source
        .set_axis_scaling(
            PlotAxis::Y,
            1,
            PlotScaling::from_tree(typed("V", "UNIT")).unwrap(),
        )
        .unwrap();
    let original = source.clone();
    let mut edit = InlinePlotEdit::default();
    frame(&ctx, &source, &mut edit, field, true, vec![])
        .1
        .unwrap();
    frame(&ctx, &source, &mut edit, field, false, vec![])
        .1
        .unwrap();
    let (_, changed) = frame(
        &ctx,
        &source,
        &mut edit,
        field,
        false,
        vec![key(egui::Key::Minus, egui::Modifiers::CTRL)],
    );
    assert!(changed.unwrap()); // Empty literal suffix is typed source.
    let (_, changed) = frame(
        &ctx,
        &source,
        &mut edit,
        field,
        false,
        vec![egui::Event::Text("dc𝛼".into())],
    );
    assert!(changed.unwrap());
    let value = scaling(edit.draft().unwrap(), field)
        .unwrap()
        .as_ref()
        .unwrap();
    assert!(
        matches!(&value.as_tree().kind,NodeKind::NativeIdentifier{name,labels,subscript:Some(suffix),..}if name=="V"&&labels=="UNIT"&&suffix=="dc𝛼"),
        "{:?}",
        value.as_tree()
    );
    frame(
        &ctx,
        &source,
        &mut edit,
        field,
        false,
        vec![key(egui::Key::Q, egui::Modifiers::CTRL)],
    )
    .1
    .unwrap();
    let value = scaling(edit.draft().unwrap(), field)
        .unwrap()
        .as_ref()
        .unwrap();
    assert!(
        matches!(&value.as_tree().kind,NodeKind::NativeIdentifier{labels,subscript:Some(suffix),..}if labels=="CONSTANT"&&suffix=="dc𝛼")
    );
    assert_eq!(source, original);
    assert!(
        scaling(edit.draft().unwrap(), fields(&source)[0])
            .unwrap()
            .is_none()
    );
    assert_eq!(edit.draft_creations, 1);
}
#[test]
#[allow(deprecated)]
fn formula_preedit_is_not_user_source_and_blur_clears_only_composition() {
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    let source = source();
    let field = fields(&source)[1];
    let original = source.clone();
    let mut edit = InlinePlotEdit::default();
    frame(&ctx, &source, &mut edit, field, true, vec![])
        .1
        .unwrap();
    frame(&ctx, &source, &mut edit, field, false, vec![])
        .1
        .unwrap();
    let (_, staged) = frame(
        &ctx,
        &source,
        &mut edit,
        field,
        false,
        vec![
            egui::Event::Ime(egui::ImeEvent::Enabled),
            egui::Event::Ime(egui::ImeEvent::Preedit {
                text: "단위".into(),
                active_range_chars: None,
            }),
        ],
    );
    assert!(!staged.unwrap());
    assert!(edit.is_composing());
    assert!(!edit.has_changes());
    assert!(edit.take().is_none());
    let (_, staged) = frame(
        &ctx,
        &source,
        &mut edit,
        field,
        false,
        vec![key(egui::Key::Enter, egui::Modifiers::NONE)],
    );
    assert!(!staged.unwrap());
    assert_eq!(source, original);
    let (_, staged) = frame(
        &ctx,
        &source,
        &mut edit,
        field,
        false,
        vec![egui::Event::Ime(egui::ImeEvent::Commit("m".into()))],
    );
    assert!(staged.unwrap());
    assert!(!edit.is_composing());
    let confirmed = edit.draft().unwrap().clone();
    frame(
        &ctx,
        &source,
        &mut edit,
        field,
        false,
        vec![egui::Event::Ime(egui::ImeEvent::Preedit {
            text: "temporary".into(),
            active_range_chars: None,
        })],
    )
    .1
    .unwrap();
    assert!(edit.is_composing());
    edit.scaling_focus_changed(&ctx, None);
    assert!(!edit.is_composing());
    assert_eq!(edit.draft(), Some(&confirmed));
    assert_eq!(source, original);
}
#[test]
fn traversal_has_expression_then_scaling_in_source_order_and_no_synthetic_wrap() {
    let source = source();
    let fields = fields(&source);
    let items = traversal_fields(&fields);
    assert_eq!(items.len(), 6);
    for (pair, field) in items.as_chunks::<2>().0.iter().zip(&fields) {
        assert_eq!(
            pair[0],
            PlotField {
                axis: *field,
                part: PlotFieldPart::Expression
            }
        );
        assert_eq!(
            pair[1],
            PlotField {
                axis: *field,
                part: PlotFieldPart::Scaling
            }
        );
    }
    assert!(!items[0].axis.x);
    assert!(items.last().unwrap().axis.x);
    assert_eq!(items.last().unwrap().part, PlotFieldPart::Scaling);
}
#[test]
fn identity_reorder_retires_uncommitted_scaling_without_moving_it_to_another_equation() {
    let mut source = source();
    let mut edit = InlinePlotEdit::default();
    let field = edit.prepare_fields(&source)[1];
    let editor = edit.scaling_editor(&source, field).unwrap();
    editor.load_portable(&typed("m", "UNIT")).unwrap();
    assert!(edit.stage_scaling(&source, field).unwrap());
    source.axis_lists_mut().unwrap().y.swap(0, 1);
    edit.prepare_fields(&source);
    assert!(!edit.has_changes());
    let editor = edit.scaling_editor(&source, field).unwrap();
    assert!(matches!(
        editor.to_portable().unwrap().kind,
        NodeKind::Placeholder
    ));
    assert!(source.equation_sources(false).all(|e| e.scaling.is_none()));
    editor.place_caret(editor.root(), Direction::Right).unwrap();
    assert!(!edit.stage_scaling(&source, field).unwrap());
}

#[test]
fn actual_numeric_unit_input_defaults_only_missing_label_and_one_undo_clears_both() {
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    let source = source();
    let original = source.clone();
    let field = fields(&source)[1];
    let mut edit = InlinePlotEdit::default();
    frame(&ctx, &source, &mut edit, field, true, vec![])
        .1
        .unwrap();
    frame(&ctx, &source, &mut edit, field, false, vec![])
        .1
        .unwrap();
    let (_, changed) = frame(
        &ctx,
        &source,
        &mut edit,
        field,
        false,
        vec![egui::Event::Text("0.1m".into())],
    );
    assert!(changed.unwrap());
    let value = scaling(edit.draft().unwrap(), field)
        .unwrap()
        .as_ref()
        .unwrap();
    let tree = value.as_tree();
    assert_eq!(tree.kind, NodeKind::Binary(BinaryOp::Scale));
    assert!(matches!(&tree.children[0].kind, NodeKind::Number(value) if value=="0.1"));
    assert!(
        matches!(&tree.children[1].kind, NodeKind::AnnotatedIdentifier { name, labels }
        if name=="m" && labels=="UNIT")
    );
    assert_eq!(source, original);
    assert_eq!(edit.draft_creations, 1);
    let editor = edit.scaling_editor(&source, field).unwrap();
    assert!(editor.undo());
    assert!(matches!(
        editor.to_portable().unwrap().kind,
        NodeKind::Placeholder
    ));
    assert!(edit.stage_scaling(&source, field).unwrap());
    assert!(!edit.has_changes());
    assert!(edit.take().is_none());
}

#[test]
fn legacy_actual_pair_scalings_are_independent_and_preserve_inactive_fallbacks() {
    use mathcraft_document::canvas::PlotTraceSpec;
    let mut source = PlotSpec {
        source: PlotSource::legacy(
            "fallback_x",
            "fallback_y",
            vec![
                PlotTraceSpec {
                    x_expression: "a".into(),
                    y_expression: "b".into(),
                    ..Default::default()
                },
                PlotTraceSpec {
                    x_expression: "c".into(),
                    y_expression: "d".into(),
                    ..Default::default()
                },
            ],
        ),
        ..Default::default()
    };
    let original = source.clone();
    let mut edit = InlinePlotEdit::default();
    let authored = edit.prepare_fields(&source);
    assert_eq!(authored.len(), 4);
    for (index, field) in authored.into_iter().enumerate() {
        let tree = typed(
            &format!("unit{index}"),
            if index % 2 == 0 { "UNIT" } else { "VARIABLE" },
        );
        edit.scaling_editor(&source, field)
            .unwrap()
            .load_portable(&tree)
            .unwrap();
        assert!(edit.stage_scaling(&source, field).unwrap());
    }
    assert_eq!(edit.draft_creations, 1);
    assert_eq!(source, original);
    source = edit.take().unwrap();
    for (index, field) in fields(&source).into_iter().enumerate() {
        let tree = scaling(&source, field).unwrap().as_ref().unwrap().as_tree();
        assert_eq!(
            tree,
            &typed(
                &format!("unit{index}"),
                if index % 2 == 0 { "UNIT" } else { "VARIABLE" }
            )
        );
    }
    let PlotSource::Legacy {
        x_expression,
        y_expression,
        x_scaling,
        y_scaling,
        traces,
    } = &source.source
    else {
        panic!("legacy source changed mode");
    };
    assert_eq!(x_expression, "fallback_x");
    assert_eq!(y_expression, "fallback_y");
    assert!(x_scaling.is_none() && y_scaling.is_none());
    assert_eq!(traces[0].x_expression, "a");
    assert_eq!(traces[1].y_expression, "d");
}
