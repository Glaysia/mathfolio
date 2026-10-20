use super::*;
use crate::plots::{self, PlotData, TraceOutcome};
use mathcraft_editor::{Direction, NodeKind, PortableTree};
fn tree(kind: NodeKind, children: Vec<PortableTree>) -> PortableTree {
    PortableTree { kind, children }
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
struct Harness {
    ctx: egui::Context,
    spec: PlotSpec,
    original: PlotSpec,
    edit: InlinePlotEdit,
    data: Result<PlotData, String>,
    fields: Vec<(AxisField, egui::Id)>,
}
impl Harness {
    fn new(spec: PlotSpec) -> Self {
        let ctx = egui::Context::default();
        crate::configure(&ctx);
        let mut h = Self {
            ctx,
            original: spec.clone(),
            spec,
            edit: InlinePlotEdit::default(),
            data: Ok(PlotData {
                traces: vec![TraceOutcome::Incomplete],
            }),
            fields: vec![],
        };
        h.frame(true, vec![]);
        h.frame(false, vec![]);
        h
    }
    fn frame(&mut self, focus: bool, events: Vec<egui::Event>) -> plots::DrawResponse {
        let mut response = None;
        let data_before = self.data.as_ref().unwrap().to_saved();
        let output = self.ctx.run_ui(
            egui::RawInput {
                events,
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1800., 1600.),
                )),
                ..Default::default()
            },
            |ui| {
                response = Some(plots::draw(
                    ui,
                    &mut self.spec,
                    &mut self.edit,
                    &self.data,
                    focus,
                ));
                self.fields = fields(self.edit.draft().unwrap_or(&self.spec))
                    .into_iter()
                    .map(|field| {
                        (
                            field,
                            self.edit
                                .expression_widget_id(field)
                                .unwrap_or_else(|| self.edit.field_id(ui, field)),
                        )
                    })
                    .collect();
            },
        );
        output.drop_without_applying_deltas();
        assert_eq!(self.spec, self.original);
        assert_eq!(self.data.as_ref().unwrap().to_saved(), data_before);
        response.unwrap()
    }
    fn focus_y(&mut self, index: usize) -> AxisField {
        let (field, id) = *self
            .fields
            .iter()
            .find(|(f, _)| !f.x && f.index == index)
            .unwrap();
        self.ctx.memory_mut(|m| m.request_focus(id));
        self.frame(false, vec![]);
        assert!(self.ctx.memory(|m| m.has_focus(id)));
        field
    }
    fn value(&self, field: AxisField) -> &PlotExpression {
        source(self.edit.draft().unwrap_or(&self.spec), field).unwrap()
    }
    fn y_count(&self) -> usize {
        self.edit
            .draft()
            .unwrap_or(&self.spec)
            .axis_lists()
            .unwrap()
            .y
            .len()
    }
}
fn typed(tree: PortableTree) -> PlotSpec {
    let mut spec = PlotSpec::typed_blank_xy();
    spec.set_axis_expression(
        mathcraft_document::canvas::PlotAxis::Y,
        0,
        PlotExpression::from_tree(tree).unwrap(),
    )
    .unwrap();
    spec
}
#[test]
fn idle_and_focus_preserve_typed_labels_context_suffix_without_scaling_default() {
    let value = tree(
        NodeKind::NativeIdentifier {
            name: "V".into(),
            labels: "VARIABLE".into(),
            subscript: Some("dc𝛼".into()),
            contextual: Some(true),
        },
        vec![],
    );
    let mut h = Harness::new(typed(value.clone()));
    let field = h.focus_y(0);
    for _ in 0..12 {
        h.frame(false, vec![]);
    }
    assert_eq!(
        h.edit
            .expression_editor(&h.spec, field)
            .unwrap()
            .to_portable()
            .unwrap(),
        value
    );
    assert!(!h.edit.has_changes());
    assert_eq!(h.edit.draft_creations, 0);
    let mut plain = Harness::new(PlotSpec::typed_blank_xy());
    let field = plain.focus_y(0);
    plain.frame(false, vec![egui::Event::Text("m".into())]);
    assert_eq!(
        plain.value(field).as_tree().unwrap().kind,
        NodeKind::Identifier("m".into()),
        "first math never applies scaling UNIT defaults"
    );
    assert!(
        plain
            .frame(false, vec![key(egui::Key::Enter, egui::Modifiers::NONE)])
            .commit_requested
    );
}
#[test]
fn first_math_call_and_matrix_edit_trees_preserve_their_source_metadata() {
    let callee = tree(
        NodeKind::NativeIdentifier {
            name: "f".into(),
            labels: "FUNCTION".into(),
            subscript: Some("q".into()),
            contextual: None,
        },
        vec![],
    );
    let value = tree(
        NodeKind::Call { argument_count: 1 },
        vec![callee.clone(), tree(NodeKind::Number("2".into()), vec![])],
    );
    let mut h = Harness::new(typed(value));
    let field = h.focus_y(0);
    let editor = h.edit.expression_editor(&h.spec, field).unwrap();
    let arg = editor.arena().get(editor.root()).unwrap().children()[1];
    editor.place_text_caret(arg, 1).unwrap();
    h.frame(false, vec![egui::Event::Text("3".into())]);
    let edited = h.value(field).as_tree().unwrap();
    assert_eq!(edited.children[0], callee);
    assert_eq!(edited.children[1].kind, NodeKind::Number("23".into()));
    let value = tree(
        NodeKind::Matrix {
            rows: 1,
            cols: 1,
            display: None,
        },
        vec![tree(NodeKind::Placeholder, vec![])],
    );
    let mut h = Harness::new(typed(value));
    let field = h.focus_y(0);
    let editor = h.edit.expression_editor(&h.spec, field).unwrap();
    let child = editor.arena().get(editor.root()).unwrap().children()[0];
    editor.place_caret(child, Direction::Left).unwrap();
    h.frame(false, vec![key(egui::Key::Enter, egui::Modifiers::SHIFT)]);
    assert_eq!(h.y_count(), 1);
    assert!(matches!(
        h.value(field).as_tree().unwrap().kind,
        NodeKind::Matrix {
            rows: 2,
            cols: 1,
            display: None
        }
    ));
}
#[test]
fn typed_slot_creation_empty_removal_and_mixed_history_keep_trees_and_ids() {
    let mut h = Harness::new(PlotSpec::typed_blank_xy());
    h.focus_y(0);
    h.frame(false, vec![key(egui::Key::Enter, egui::Modifiers::SHIFT)]);
    assert_eq!(h.y_count(), 2);
    let field = h.focus_y(1);
    assert!(h.value(field).as_tree().is_some());
    assert!(h.value(field).is_exact_placeholder());
    let id = field.id;
    h.frame(false, vec![egui::Event::Text("a".into())]);
    h.frame(
        false,
        vec![key(egui::Key::Backspace, egui::Modifiers::NONE)],
    );
    assert!(h.value(field).is_exact_placeholder());
    h.frame(false, vec![key(egui::Key::Z, egui::Modifiers::CTRL)]);
    assert_eq!(h.y_count(), 2);
    assert_eq!(
        h.value(field).as_tree().unwrap().kind,
        NodeKind::Identifier("a".into())
    );
    h.frame(false, vec![key(egui::Key::Z, egui::Modifiers::CTRL)]);
    assert!(h.value(field).is_exact_placeholder());
    h.frame(false, vec![key(egui::Key::Z, egui::Modifiers::CTRL)]);
    assert_eq!(h.y_count(), 1);
    h.frame(false, vec![key(egui::Key::Y, egui::Modifiers::CTRL)]);
    assert_eq!(h.y_count(), 2);
    let field = h.focus_y(1);
    assert_eq!(field.id, id);
    h.frame(false, vec![key(egui::Key::Delete, egui::Modifiers::NONE)]);
    assert_eq!(h.y_count(), 1);
}
#[test]
fn typed_preedit_is_inert_and_commit_confirmation_enter_does_not_publish() {
    let mut h = Harness::new(PlotSpec::typed_blank_xy());
    let field = h.focus_y(0);
    h.frame(
        false,
        vec![egui::Event::Ime(egui::ImeEvent::Preedit {
            text: "ㅎ".into(),
            active_range_chars: None,
        })],
    );
    assert!(h.edit.is_composing());
    assert!(!h.edit.has_changes());
    let response = h.frame(
        false,
        vec![
            egui::Event::Ime(egui::ImeEvent::Commit("한".into())),
            key(egui::Key::Enter, egui::Modifiers::NONE),
        ],
    );
    assert!(!response.commit_requested);
    assert!(!h.edit.is_composing());
    assert_eq!(
        h.value(field).as_tree().unwrap().kind,
        NodeKind::Identifier("한".into())
    );
    assert!(
        h.frame(false, vec![key(egui::Key::Enter, egui::Modifiers::NONE)])
            .commit_requested
    );
}
#[test]
fn legacy_focus_and_idle_never_promote_to_typed_or_rewrite_old_wire() {
    let spec = PlotSpec::blank_xy();
    let wire = serde_json::to_string(&spec).unwrap();
    let mut h = Harness::new(spec);
    h.focus_y(0);
    for _ in 0..8 {
        h.frame(false, vec![]);
    }
    assert!(!h.edit.has_changes());
    assert_eq!(serde_json::to_string(&h.spec).unwrap(), wire);
    assert!(h.spec.primary_y_expression().as_legacy_text().is_some());
}

#[test]
fn typed_sixteen_slots_stay_lazy_and_surviving_ids_keep_widget_ownership_after_insert() {
    let mut spec = PlotSpec::typed_blank_xy();
    for index in 1..16 {
        spec.insert_typed_axis_equation(
            mathcraft_document::canvas::PlotAxis::Y,
            index,
            tree(NodeKind::Identifier(format!("v{index}")), vec![]),
            Some(mathcraft_document::canvas::PlotTraceAppearance::default()),
        )
        .unwrap();
    }
    let mut h = Harness::new(spec);
    let field = h.focus_y(15);
    let widget = h.edit.expression_widget_id(field).unwrap();
    let before = h.edit.field_journal_cost();
    for _ in 0..8 {
        h.frame(false, vec![]);
    }
    assert_eq!(h.edit.field_journal_cost(), before);
    assert_eq!(h.edit.draft_creations, 0);
    assert!(!h.edit.has_changes());
    assert_eq!(h.edit.expression_widget_id(field), Some(widget));
    // A separate smaller owner can insert before an existing equation. Global
    // equation-derived widget scope survives the changed row ordinal.
    let mut spec = typed(tree(NodeKind::Identifier("a".into()), vec![]));
    spec.insert_typed_axis_equation(
        mathcraft_document::canvas::PlotAxis::Y,
        1,
        tree(NodeKind::Identifier("b".into()), vec![]),
        Some(mathcraft_document::canvas::PlotTraceAppearance::default()),
    )
    .unwrap();
    let mut h = Harness::new(spec);
    let surviving = h.focus_y(1);
    let widget = h.edit.expression_widget_id(surviving).unwrap();
    let first = h.focus_y(0);
    let editor = h.edit.expression_editor(&h.spec, first).unwrap();
    editor.move_to_expression_extreme(false);
    h.frame(false, vec![key(egui::Key::Enter, egui::Modifiers::SHIFT)]);
    assert_eq!(h.y_count(), 3);
    let moved = h
        .fields
        .iter()
        .find(|(f, _)| f.id == surviving.id && !f.x)
        .unwrap()
        .0;
    assert_eq!(moved.index, 2);
    assert_eq!(h.edit.expression_widget_id(moved), Some(widget));
}
#[test]
fn typed_invalid_stage_blocks_owner_commit_and_cancel_keeps_original_source() {
    let mut h = Harness::new(PlotSpec::typed_blank_xy());
    let field = h.focus_y(0);
    h.frame(false, vec![egui::Event::Text("2=".into())]);
    assert!(h.edit.validation_error().is_some());
    assert!(h.edit.has_changes());
    assert!(h.edit.take().is_none());
    assert!(h.value(field).is_exact_placeholder());
    h.edit.cancel();
    assert!(!h.edit.has_changes());
    assert_eq!(h.spec, h.original);
}

#[test]
fn typed_expression_and_scaling_tab_order_share_one_lazy_draft_without_label_leak() {
    let mut h = Harness::new(PlotSpec::typed_blank_xy());
    let y = h.focus_y(0);
    h.frame(false, vec![egui::Event::Text("m".into())]);
    h.frame(false, vec![key(egui::Key::Tab, egui::Modifiers::NONE)]);
    let yscale = h.edit.scaling_widget_id(y).unwrap();
    assert!(h.ctx.memory(|m| m.has_focus(yscale)));
    h.frame(false, vec![egui::Event::Text("m".into())]);
    h.frame(false, vec![key(egui::Key::Tab, egui::Modifiers::NONE)]);
    let x = h.fields.iter().find(|(f, _)| f.x).unwrap().0;
    let xid = h.edit.expression_widget_id(x).unwrap();
    assert!(h.ctx.memory(|m| m.has_focus(xid)));
    h.frame(false, vec![egui::Event::Text("x".into())]);
    h.frame(false, vec![key(egui::Key::Tab, egui::Modifiers::NONE)]);
    let xscale = h.edit.scaling_widget_id(x).unwrap();
    assert!(h.ctx.memory(|m| m.has_focus(xscale)));
    h.frame(false, vec![egui::Event::Text("s".into())]);
    assert_eq!(
        h.value(y).as_tree().unwrap().kind,
        NodeKind::Identifier("m".into())
    );
    assert_eq!(
        h.value(x).as_tree().unwrap().kind,
        NodeKind::Identifier("x".into())
    );
    let draft = h.edit.draft().unwrap().axis_lists().unwrap();
    assert!(
        matches!(&draft.y[0].scaling.as_ref().unwrap().as_tree().kind,NodeKind::AnnotatedIdentifier{labels,..} if labels=="UNIT")
    );
    assert_eq!(h.edit.draft_creations, 1);
    h.frame(false, vec![key(egui::Key::Tab, egui::Modifiers::SHIFT)]);
    assert!(h.ctx.memory(|m| m.has_focus(xid)));
    assert!(
        h.frame(false, vec![key(egui::Key::Enter, egui::Modifiers::NONE)])
            .commit_requested
    );
}

#[test]
fn same_id_external_typed_replacement_retires_preedit_focus_and_late_commit() {
    let mut h = Harness::new(PlotSpec::typed_blank_xy());
    let field = h.focus_y(0);
    let old = h.edit.expression_widget_id(field).unwrap();
    h.frame(
        false,
        vec![egui::Event::Ime(egui::ImeEvent::Preedit {
            text: "discard".into(),
            active_range_chars: None,
        })],
    );
    assert!(h.edit.is_composing());
    h.spec
        .set_axis_expression(
            mathcraft_document::canvas::PlotAxis::Y,
            0,
            PlotExpression::from_tree(tree(NodeKind::Identifier("replacement".into()), vec![]))
                .unwrap(),
        )
        .unwrap();
    h.original = h.spec.clone();
    h.frame(
        false,
        vec![egui::Event::Ime(egui::ImeEvent::Commit("late".into()))],
    );
    assert!(!h.edit.is_composing());
    assert!(!h.edit.has_changes());
    assert_ne!(h.edit.expression_widget_id(field), Some(old));
    assert!(!h.ctx.memory(|m| m.has_focus(old)));
    assert_eq!(
        h.value(field).as_tree().unwrap().kind,
        NodeKind::Identifier("replacement".into())
    );
    let generation = h.edit.expression_generation(field).unwrap();
    h.edit.cancel();
    h.frame(false, vec![]);
    assert!(h.edit.expression_generation(field).unwrap() > generation);
}

#[test]
fn typed_native_history_before_slot_boundary_and_owner_redo_do_not_duplicate() {
    let mut h = Harness::new(PlotSpec::typed_blank_xy());
    let first = h.focus_y(0);
    h.frame(false, vec![egui::Event::Text("q".into())]);
    h.frame(false, vec![key(egui::Key::Enter, egui::Modifiers::SHIFT)]);
    assert_eq!(h.y_count(), 2);
    let second = h.focus_y(1);
    h.frame(false, vec![egui::Event::Text("a".into())]);
    h.frame(false, vec![key(egui::Key::Z, egui::Modifiers::CTRL)]);
    assert!(h.value(second).is_exact_placeholder());
    h.frame(false, vec![key(egui::Key::Z, egui::Modifiers::CTRL)]);
    assert_eq!(h.y_count(), 1);
    assert_eq!(
        h.value(first).as_tree().unwrap().kind,
        NodeKind::Identifier("q".into())
    );
    h.frame(false, vec![key(egui::Key::Z, egui::Modifiers::CTRL)]);
    assert!(h.value(first).is_exact_placeholder());
    h.frame(false, vec![key(egui::Key::Y, egui::Modifiers::CTRL)]);
    assert_eq!(
        h.value(first).as_tree().unwrap().kind,
        NodeKind::Identifier("q".into())
    );
    assert_eq!(h.y_count(), 1);
    h.frame(false, vec![key(egui::Key::Y, egui::Modifiers::CTRL)]);
    assert_eq!(h.y_count(), 2);
    h.frame(false, vec![key(egui::Key::Y, egui::Modifiers::CTRL)]);
    let second = h.focus_y(1);
    assert_eq!(
        h.value(second).as_tree().unwrap().kind,
        NodeKind::Identifier("a".into())
    );
}
#[test]
fn typed_literal_subscript_and_label_commands_keep_formatted_source() {
    let mut h = Harness::new(PlotSpec::typed_blank_xy());
    let field = h.focus_y(0);
    h.frame(false, vec![egui::Event::Text("V".into())]);
    h.frame(false, vec![key(egui::Key::Minus, egui::Modifiers::CTRL)]);
    h.frame(false, vec![egui::Event::Text("dc𝛼".into())]);
    h.frame(false, vec![key(egui::Key::Minus, egui::Modifiers::CTRL)]);
    assert!(
        matches!(&h.value(field).as_tree().unwrap().kind,NodeKind::NativeIdentifier{name,subscript:Some(suffix),labels,..} if name=="V" && suffix=="dc𝛼" && labels!="UNIT")
    );
    h.frame(false, vec![key(egui::Key::Q, egui::Modifiers::CTRL)]);
    assert!(
        matches!(&h.value(field).as_tree().unwrap().kind,NodeKind::NativeIdentifier{name,subscript:Some(suffix),labels,..} if name=="V" && suffix=="dc𝛼" && labels=="VARIABLE")
    );
}
