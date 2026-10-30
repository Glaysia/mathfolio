//! Real CPU input through draw; no calculator, original runtime or GUI.
use super::*;
use inline_edit::{AxisField, PlotFieldPart};
use mathfolio_document::canvas::{PlotAxis, PlotScaling, PlotTraceAppearance};
use mathfolio_editor::Direction;

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
fn source(nx: usize, ny: usize) -> PlotSpec {
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
fn scale(tree: PortableTree) -> Option<PlotScaling> {
    PlotScaling::from_tree(tree).unwrap()
}
fn tree(kind: NodeKind, children: Vec<PortableTree>) -> PortableTree {
    PortableTree { kind, children }
}
struct Harness {
    ctx: egui::Context,
    spec: PlotSpec,
    original: PlotSpec,
    edit: InlinePlotEdit,
    data: Result<PlotData, String>,
    saved: String,
    ids: Vec<(AxisField, PlotFieldPart, egui::Id)>,
}
impl Harness {
    fn new(spec: PlotSpec) -> Self {
        let ctx = egui::Context::default();
        crate::configure(&ctx);
        let data = PlotData {
            traces: vec![TraceOutcome::Ready(PlotTraceData {
                points: vec![[0., 1.], [1., 3.]].into(),
                presentation: None,
                implicit_domain: false,
                label: "retained".into(),
                style: PlotTraceStyle::default(),
            })],
        };
        let saved = data.to_saved();
        let mut h = Self {
            ctx,
            original: spec.clone(),
            spec,
            edit: InlinePlotEdit::default(),
            data: Ok(data),
            saved,
            ids: vec![],
        };
        h.render(true, vec![]);
        h.frame(vec![]);
        h
    }
    fn render(&mut self, focus: bool, events: Vec<egui::Event>) -> DrawResponse {
        let mut drawn = None;
        let output = self.ctx.run_ui(
            egui::RawInput {
                events,
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(2200., 1600.),
                )),
                ..Default::default()
            },
            |ui| {
                drawn = Some(draw(ui, &mut self.spec, &mut self.edit, &self.data, focus));
                self.ids = inline_edit::fields(self.edit.draft().unwrap_or(&self.spec))
                    .into_iter()
                    .flat_map(|f| {
                        let mut ids =
                            vec![(f, PlotFieldPart::Expression, self.edit.field_id(ui, f))];
                        if let Some(id) = self.edit.scaling_widget_id(f) {
                            ids.push((f, PlotFieldPart::Scaling, id));
                        }
                        ids
                    })
                    .collect();
            },
        );
        output.drop_without_applying_deltas();
        assert_eq!(self.spec, self.original, "input must not install source");
        assert_eq!(
            self.data.as_ref().unwrap().to_saved(),
            self.saved,
            "input must not change retained curves"
        );
        drawn.unwrap()
    }
    fn frame(&mut self, events: Vec<egui::Event>) -> DrawResponse {
        self.render(false, events)
    }
    fn focus(&mut self, x: bool, index: usize, part: PlotFieldPart) -> (AxisField, egui::Id) {
        let (field, _, id) = *self
            .ids
            .iter()
            .find(|(f, p, _)| f.x == x && f.index == index && *p == part)
            .unwrap();
        self.ctx.memory_mut(|m| m.request_focus(id));
        self.frame(vec![]);
        assert!(self.ctx.memory(|m| m.has_focus(id)));
        (field, id)
    }
    fn caret(&self, id: egui::Id, anchor: usize, focus: usize) {
        let mut state = egui::text_edit::TextEditState::load(&self.ctx, id).unwrap();
        state.cursor.set_char_range(Some(egui::text::CCursorRange {
            primary: egui::text::CCursor::new(focus),
            secondary: egui::text::CCursor::new(anchor),
            h_pos: None,
        }));
        state.store(&self.ctx, id);
    }
    fn count_y(&self) -> usize {
        self.edit
            .draft()
            .unwrap_or(&self.spec)
            .axis_equation_count(PlotAxis::Y)
    }
}

#[test]
fn shift_enter_uses_expression_focus_edge_but_scaling_always_adds_after() {
    for (part, at, empty, want) in [
        (PlotFieldPart::Expression, 0, false, 0),
        (PlotFieldPart::Expression, 2, false, 1),
        (PlotFieldPart::Expression, 0, true, 1),
        (PlotFieldPart::Scaling, 0, false, 1),
    ] {
        let mut spec = source(1, 2);
        if empty {
            spec.axis_lists_mut().unwrap().y[0]
                .source
                .as_legacy_text_mut()
                .unwrap()
                .clear();
        }
        let mut h = Harness::new(spec);
        let (field, id) = h.focus(false, 0, part);
        if part == PlotFieldPart::Expression {
            h.caret(id, at, at);
        } else {
            let e = h.edit.scaling_editor(&h.spec, field).unwrap();
            e.place_caret(e.root(), Direction::Left).unwrap();
        }
        let response = h.frame(vec![key(egui::Key::Enter, egui::Modifiers::SHIFT)]);
        assert!(response.expression_changed);
        assert!(!response.commit_requested);
        assert_eq!(h.count_y(), 3);
        let pending = h.edit.draft().unwrap().axis_lists().unwrap();
        assert!(pending.y[want].source.is_exact_placeholder() && pending.y[want].scaling.is_none());
        let (created, _, new_id) = *h
            .ids
            .iter()
            .find(|(f, p, _)| !f.x && f.index == want && *p == PlotFieldPart::Expression)
            .unwrap();
        assert_ne!(created.id, field.id);
        assert!(h.ctx.memory(|m| m.has_focus(new_id)));
        assert_eq!(response.response.id, new_id);
        let caret = egui::text_edit::TextEditState::load(&h.ctx, new_id)
            .unwrap()
            .cursor
            .char_range()
            .unwrap();
        assert_eq!(caret.primary.index.0, 0);
    }
}
#[test]
fn selection_direction_uses_primary_focus_not_sorted_min_with_supplementary_text() {
    for (anchor, focus, want) in [(0, 2, 1), (2, 0, 0), (0, 1, 1)] {
        let mut spec = source(1, 1);
        spec.axis_lists_mut().unwrap().y[0].source = "𝛼x".into();
        let mut h = Harness::new(spec);
        let (_, id) = h.focus(false, 0, PlotFieldPart::Expression);
        h.caret(id, anchor, focus);
        h.frame(vec![key(egui::Key::Enter, egui::Modifiers::SHIFT)]);
        let list = &h.edit.draft().unwrap().axis_lists().unwrap().y;
        assert!(list[want].source.is_exact_placeholder());
        assert_eq!(list[1 - want].source.as_legacy_text().unwrap(), "𝛼x");
    }
}
#[test]
fn empty_pair_delete_uses_neighbor_scaling_end_or_expression_start() {
    for delete in [false, true] {
        let mut spec = source(1, 3);
        spec.axis_lists_mut().unwrap().y[1]
            .source
            .as_legacy_text_mut()
            .unwrap()
            .clear();
        spec.axis_lists_mut().unwrap().y[0].scaling = scale(tree(
            NodeKind::AnnotatedIdentifier {
                name: "meter".into(),
                labels: "UNIT".into(),
            },
            vec![],
        ));
        let mut h = Harness::new(spec);
        h.focus(false, 1, PlotFieldPart::Scaling);
        let response = h.frame(vec![key(
            if delete {
                egui::Key::Delete
            } else {
                egui::Key::Backspace
            },
            egui::Modifiers::NONE,
        )]);
        assert!(response.expression_changed);
        assert_eq!(h.count_y(), 2);
        let (target, part, id) = *h
            .ids
            .iter()
            .find(|(f, p, _)| {
                !f.x && f.index == if delete { 1 } else { 0 }
                    && *p
                        == if delete {
                            PlotFieldPart::Expression
                        } else {
                            PlotFieldPart::Scaling
                        }
            })
            .unwrap();
        assert!(h.ctx.memory(|m| m.has_focus(id)));
        assert_eq!(response.response.id, id);
        if part == PlotFieldPart::Expression {
            assert_eq!(
                egui::text_edit::TextEditState::load(&h.ctx, id)
                    .unwrap()
                    .cursor
                    .char_range()
                    .unwrap()
                    .primary
                    .index
                    .0,
                0
            );
        } else {
            assert_eq!(
                h.edit
                    .scaling_editor(&h.spec, target)
                    .unwrap()
                    .caret()
                    .value_range
                    .unwrap()
                    .focus,
                5
            );
        }
    }
}
#[test]
fn first_nonempty_key_edits_text_only_and_unique_placeholder_consumes_without_history() {
    let mut spec = source(1, 2);
    spec.axis_lists_mut().unwrap().y[1].source = "a".into();
    let mut h = Harness::new(spec);
    let (_, id) = h.focus(false, 1, PlotFieldPart::Expression);
    h.caret(id, 1, 1);
    h.frame(vec![key(egui::Key::Backspace, egui::Modifiers::NONE)]);
    assert_eq!(h.count_y(), 2);
    assert!(
        h.edit.draft().unwrap().axis_lists().unwrap().y[1]
            .source
            .is_exact_placeholder()
    );
    assert!(!h.edit.can_undo_slot());
    h.frame(vec![key(egui::Key::Backspace, egui::Modifiers::NONE)]);
    assert_eq!(h.count_y(), 1);
    assert!(h.edit.can_undo_slot());
    let mut spec = source(1, 1);
    spec.axis_lists_mut().unwrap().y[0]
        .source
        .as_legacy_text_mut()
        .unwrap()
        .clear();
    let mut h = Harness::new(spec);
    h.focus(false, 0, PlotFieldPart::Scaling);
    for k in [egui::Key::Backspace, egui::Key::Delete] {
        let r = h.frame(vec![key(k, egui::Modifiers::NONE)]);
        assert!(!r.expression_changed);
        assert_eq!(h.count_y(), 1);
        assert!(!h.edit.has_changes());
        assert!(!h.edit.can_undo_slot());
        assert_eq!(h.edit.palette_cursor(), 0);
    }
}
#[test]
fn command_z_y_restore_local_topology_and_infinity_is_not_redo() {
    let mut h = Harness::new(source(1, 1));
    h.focus(false, 0, PlotFieldPart::Expression);
    h.frame(vec![key(egui::Key::Enter, egui::Modifiers::SHIFT)]);
    let after = h.edit.draft().unwrap().clone();
    let cursor = h.edit.palette_cursor();
    let floor = h.edit.equation_id_floor();
    h.frame(vec![key(egui::Key::Z, egui::Modifiers::CTRL)]);
    assert_eq!(h.count_y(), 1);
    assert!(h.edit.draft().is_none());
    assert!(h.edit.can_redo_slot());
    h.frame(vec![key(egui::Key::Y, egui::Modifiers::CTRL)]);
    assert_eq!(h.edit.draft(), Some(&after));
    assert_eq!(h.edit.palette_cursor(), cursor);
    assert_eq!(h.edit.equation_id_floor(), floor);
    h.frame(vec![key(egui::Key::Z, egui::Modifiers::CTRL)]);
    h.focus(false, 0, PlotFieldPart::Scaling);
    h.frame(vec![key(
        egui::Key::Z,
        egui::Modifiers {
            ctrl: true,
            shift: true,
            command: true,
            ..Default::default()
        },
    )]);
    assert_eq!(h.count_y(), 1);
    assert_eq!(h.edit.palette_cursor(), cursor);
    assert!(h.edit.validation_error().is_none());
    assert!(
        serde_json::to_string(h.edit.draft().unwrap())
            .unwrap()
            .contains('∞')
    );
    assert!(
        !h.edit.can_redo_slot(),
        "new text command retires structural redo"
    );
}
#[test]
fn matrix_scaling_keeps_inner_row_priority_and_source_budget_failure_never_adds_axis() {
    for (rows, cols, accepted) in [(1, 1, true), (31, 33, false)] {
        let mut spec = source(1, 1);
        spec.axis_lists_mut().unwrap().y[0].scaling = scale(tree(
            NodeKind::Matrix {
                rows,
                cols,
                display: None,
            },
            vec![tree(NodeKind::Placeholder, vec![]); rows * cols],
        ));
        let mut h = Harness::new(spec);
        let (field, _) = h.focus(false, 0, PlotFieldPart::Scaling);
        let editor = h.edit.scaling_editor(&h.spec, field).unwrap();
        let cell = editor.arena().get(editor.root()).unwrap().children()[0];
        editor.place_caret(cell, Direction::Indeterminate).unwrap();
        h.frame(vec![key(egui::Key::Enter, egui::Modifiers::SHIFT)]);
        assert_eq!(h.count_y(), 1);
        assert!(!h.edit.can_undo_slot());
        if accepted {
            assert!(matches!(
                h.edit.draft().unwrap().axis_lists().unwrap().y[0]
                    .scaling
                    .as_ref()
                    .unwrap()
                    .as_tree()
                    .kind,
                NodeKind::Matrix {
                    rows: 2,
                    cols: 1,
                    display: None
                }
            ));
        } else {
            assert!(h.edit.validation_error().is_some());
            assert!(h.edit.draft().is_none());
        }
    }
}
#[test]
#[allow(deprecated)]
fn modified_enter_and_preedit_do_not_add_axis_slots() {
    for modifiers in [
        egui::Modifiers::NONE,
        egui::Modifiers::CTRL,
        egui::Modifiers::ALT,
        egui::Modifiers {
            shift: true,
            ctrl: true,
            ..Default::default()
        },
    ] {
        let mut h = Harness::new(source(1, 1));
        h.focus(false, 0, PlotFieldPart::Expression);
        h.frame(vec![key(egui::Key::Enter, modifiers)]);
        assert_eq!(h.count_y(), 1);
        assert!(!h.edit.can_undo_slot());
    }
    for part in [PlotFieldPart::Expression, PlotFieldPart::Scaling] {
        let mut h = Harness::new(source(1, 1));
        h.focus(false, 0, part);
        h.frame(vec![
            egui::Event::Ime(egui::ImeEvent::Enabled),
            egui::Event::Ime(egui::ImeEvent::Preedit {
                text: "단위".into(),
                active_range_chars: None,
            }),
        ]);
        assert!(h.edit.is_composing(), "preedit {part:?}");
        h.frame(vec![key(egui::Key::Enter, egui::Modifiers::SHIFT)]);
        assert_eq!(h.count_y(), 1);
        assert!(!h.edit.can_undo_slot());
        assert!(h.edit.is_composing(), "after ShiftEnter {part:?}");
    }
}

#[test]
fn backspace_neighbor_scaling_end_includes_the_unicode_literal_suffix() {
    let mut spec = source(1, 2);
    spec.axis_lists_mut().unwrap().y[1]
        .source
        .as_legacy_text_mut()
        .unwrap()
        .clear();
    spec.axis_lists_mut().unwrap().y[0].scaling = scale(tree(
        NodeKind::NativeIdentifier {
            name: "V".into(),
            labels: "UNIT".into(),
            contextual: None,
            subscript: Some("dc𝛼".into()),
        },
        vec![],
    ));
    let mut h = Harness::new(spec);
    h.focus(false, 1, PlotFieldPart::Expression);
    h.frame(vec![key(egui::Key::Backspace, egui::Modifiers::NONE)]);
    let (field, _, id) = *h
        .ids
        .iter()
        .find(|(f, p, _)| !f.x && f.index == 0 && *p == PlotFieldPart::Scaling)
        .unwrap();
    assert!(h.ctx.memory(|m| m.has_focus(id)));
    let e = h.edit.scaling_editor(&h.spec, field).unwrap();
    let caret = e.caret();
    assert!(
        caret.caret_side == Direction::Right
            || (caret.insertion == mathfolio_editor::InsertionMode::Subscript
                && caret.value_range.is_some_and(|r| r.focus == 4)),
        "end must include full suffix, actual={caret:?}"
    );
}
#[test]
fn unsupported_call_row_context_never_falls_through_to_axis_insertion() {
    let mut spec = source(1, 1);
    spec.axis_lists_mut().unwrap().y[0].scaling = scale(tree(
        NodeKind::Call { argument_count: 1 },
        vec![
            tree(NodeKind::Identifier("f".into()), vec![]),
            tree(NodeKind::Placeholder, vec![]),
        ],
    ));
    let mut h = Harness::new(spec);
    let (field, _) = h.focus(false, 0, PlotFieldPart::Scaling);
    let e = h.edit.scaling_editor(&h.spec, field).unwrap();
    let child = e.arena().get(e.root()).unwrap().children()[1];
    e.place_caret(child, Direction::Indeterminate).unwrap();
    h.frame(vec![key(egui::Key::Enter, egui::Modifiers::SHIFT)]);
    assert_eq!(h.count_y(), 1);
    assert!(!h.edit.can_undo_slot());
    assert!(!h.edit.has_changes());
}

#[test]
fn scaling_source_blocks_empty_expression_removal_and_first_scaling_delete_only_clears_value() {
    let mut spec = source(1, 2);
    spec.axis_lists_mut().unwrap().y[1]
        .source
        .as_legacy_text_mut()
        .unwrap()
        .clear();
    spec.axis_lists_mut().unwrap().y[1].scaling = scale(tree(
        NodeKind::AnnotatedIdentifier {
            name: "m".into(),
            labels: "UNIT".into(),
        },
        vec![],
    ));
    let mut h = Harness::new(spec);
    h.focus(false, 1, PlotFieldPart::Expression);
    h.frame(vec![key(egui::Key::Backspace, egui::Modifiers::NONE)]);
    assert_eq!(h.count_y(), 2);
    assert!(!h.edit.has_changes());
    let (field, _) = h.focus(false, 1, PlotFieldPart::Scaling);
    let editor = h.edit.scaling_editor(&h.spec, field).unwrap();
    editor.place_text_caret(editor.root(), 1).unwrap();
    h.frame(vec![key(egui::Key::Backspace, egui::Modifiers::NONE)]);
    assert_eq!(h.count_y(), 2);
    assert!(!h.edit.can_undo_slot());
    assert!(
        h.edit.draft().unwrap().axis_lists().unwrap().y[1]
            .scaling
            .is_none()
    );
    h.frame(vec![key(egui::Key::Backspace, egui::Modifiers::NONE)]);
    assert_eq!(h.count_y(), 1);
    assert!(h.edit.can_undo_slot());
}

#[test]
fn maximum_axis_key_failure_is_inert_and_keeps_focus_for_correction() {
    let mut h = Harness::new(source(1, 16));
    let (_, id) = h.focus(false, 0, PlotFieldPart::Expression);
    h.caret(id, 2, 2);
    let palette = h.edit.palette_cursor();
    let floor = h.edit.equation_id_floor();
    let response = h.frame(vec![key(egui::Key::Enter, egui::Modifiers::SHIFT)]);
    assert!(
        response
            .input_error
            .as_ref()
            .is_some_and(|error| error.contains("16"))
    );
    assert!(!response.expression_changed);
    assert!(!response.commit_requested);
    assert_eq!(h.count_y(), 16);
    assert!(!h.edit.has_changes());
    assert!(!h.edit.can_undo_slot());
    assert_eq!(h.edit.palette_cursor(), palette);
    assert_eq!(h.edit.equation_id_floor(), floor);
    assert!(h.ctx.memory(|m| m.has_focus(id)));
}

#[test]
fn undo_after_typing_and_deleting_same_value_restores_field_before_removing_slot() {
    for part in [PlotFieldPart::Expression, PlotFieldPart::Scaling] {
        let mut h = Harness::new(source(1, 1));
        h.focus(false, 0, PlotFieldPart::Expression);
        h.frame(vec![key(egui::Key::Enter, egui::Modifiers::SHIFT)]);
        let (field, _) = h.focus(false, 1, part);
        let cursor = h.edit.palette_cursor();
        let floor = h.edit.equation_id_floor();
        h.frame(vec![egui::Event::Text("a".into())]);
        h.frame(vec![key(egui::Key::Backspace, egui::Modifiers::NONE)]);
        assert_eq!(h.count_y(), 2);
        h.frame(vec![key(egui::Key::Z, egui::Modifiers::CTRL)]);
        assert_eq!(
            h.count_y(),
            2,
            "first Undo restores the deleted character: {part:?}"
        );
        if part == PlotFieldPart::Expression {
            assert_eq!(
                inline_edit::field_expression(h.edit.draft().unwrap(), field),
                Some("a")
            );
        } else {
            assert!(
                h.edit.draft().unwrap().axis_lists().unwrap().y[1]
                    .scaling
                    .is_some()
            );
        }
        h.frame(vec![key(egui::Key::Z, egui::Modifiers::CTRL)]);
        assert_eq!(h.count_y(), 2);
        if part == PlotFieldPart::Expression {
            assert_eq!(
                inline_edit::field_expression(h.edit.draft().unwrap(), field),
                Some("")
            );
        } else {
            assert!(
                h.edit.draft().unwrap().axis_lists().unwrap().y[1]
                    .scaling
                    .is_none()
            );
        }
        h.frame(vec![key(egui::Key::Z, egui::Modifiers::CTRL)]);
        assert_eq!(h.count_y(), 1);
        h.frame(vec![key(egui::Key::Y, egui::Modifiers::CTRL)]);
        assert_eq!(h.count_y(), 2);
        h.frame(vec![key(egui::Key::Y, egui::Modifiers::CTRL)]);
        assert_eq!(h.count_y(), 2);
        h.frame(vec![key(egui::Key::Y, egui::Modifiers::CTRL)]);
        assert_eq!(h.count_y(), 2);
        assert_eq!(h.edit.palette_cursor(), cursor);
        assert_eq!(h.edit.equation_id_floor(), floor);
    }
}

#[test]
fn mixed_journal_returns_to_prestruct_native_text_and_scaling_undo() {
    for part in [PlotFieldPart::Expression, PlotFieldPart::Scaling] {
        let mut h = Harness::new(source(1, 1));
        let (original, id) = h.focus(false, 0, part);
        if part == PlotFieldPart::Expression {
            h.caret(id, 2, 2);
        }
        h.frame(vec![egui::Event::Text("q".into())]);
        // The field command predates structural history and belongs to the
        // existing TextEdit/editor Undo stack, not the mixed owner journal.
        h.frame(vec![key(egui::Key::Enter, egui::Modifiers::SHIFT)]);
        h.frame(vec![egui::Event::Text("a".into())]);
        h.frame(vec![key(egui::Key::Z, egui::Modifiers::CTRL)]);
        assert_eq!(h.count_y(), 2);
        h.frame(vec![key(egui::Key::Z, egui::Modifiers::CTRL)]);
        assert_eq!(h.count_y(), 1);
        h.frame(vec![key(egui::Key::Z, egui::Modifiers::CTRL)]);
        assert_eq!(h.count_y(), 1);
        let current = h.edit.draft().unwrap_or(&h.spec);
        if part == PlotFieldPart::Expression {
            assert_eq!(inline_edit::field_expression(current, original), Some("y0"));
        } else {
            assert!(current.axis_lists().unwrap().y[0].scaling.is_none());
        }
        // Native Redo precedes the structural Redo; neither layer duplicates
        // the other when crossing back over the owner boundary.
        h.frame(vec![key(egui::Key::Y, egui::Modifiers::CTRL)]);
        assert_eq!(h.count_y(), 1);
        let current = h.edit.draft().unwrap_or(&h.spec);
        if part == PlotFieldPart::Expression {
            assert_eq!(
                inline_edit::field_expression(current, original),
                Some("y0q")
            );
        } else {
            assert!(current.axis_lists().unwrap().y[0].scaling.is_some());
        }
        h.frame(vec![key(egui::Key::Y, egui::Modifiers::CTRL)]);
        assert_eq!(h.count_y(), 2);
        h.frame(vec![key(egui::Key::Y, egui::Modifiers::CTRL)]);
        assert_eq!(h.count_y(), 2);
    }
}

#[test]
fn mixed_field_unicode_replacement_restores_each_command_and_caret_once() {
    let mut h = Harness::new(source(1, 1));
    h.focus(false, 0, PlotFieldPart::Expression);
    h.frame(vec![key(egui::Key::Enter, egui::Modifiers::SHIFT)]);
    let (field, id) = h.focus(false, 1, PlotFieldPart::Expression);
    h.frame(vec![egui::Event::Text("a𝛼z".into())]);
    h.caret(id, 1, 2);
    h.frame(vec![egui::Event::Text("β".into())]);
    assert_eq!(
        inline_edit::field_expression(h.edit.draft().unwrap(), field),
        Some("aβz")
    );
    h.frame(vec![key(egui::Key::Z, egui::Modifiers::CTRL)]);
    assert_eq!(
        inline_edit::field_expression(h.edit.draft().unwrap(), field),
        Some("a𝛼z")
    );
    let cursor = egui::text_edit::TextEditState::load(&h.ctx, id)
        .unwrap()
        .cursor
        .char_range()
        .unwrap();
    assert_eq!((cursor.secondary.index.0, cursor.primary.index.0), (1, 2));
    h.frame(vec![key(egui::Key::Z, egui::Modifiers::CTRL)]);
    assert_eq!(h.count_y(), 2);
    assert_eq!(
        inline_edit::field_expression(h.edit.draft().unwrap(), field),
        Some("")
    );
    h.frame(vec![key(egui::Key::Z, egui::Modifiers::CTRL)]);
    assert_eq!(h.count_y(), 1);
}

#[test]
fn field_journal_idle_and_navigation_do_not_snapshot_and_commands_obey_64_budget() {
    let mut h = Harness::new(source(1, 1));
    h.focus(false, 0, PlotFieldPart::Expression);
    h.frame(vec![key(egui::Key::Enter, egui::Modifiers::SHIFT)]);
    h.focus(false, 1, PlotFieldPart::Expression);
    let before = h.edit.field_journal_cost();
    for _ in 0..8 {
        h.frame(vec![]);
        h.frame(vec![key(egui::Key::ArrowLeft, egui::Modifiers::NONE)]);
    }
    assert_eq!(h.edit.field_journal_cost(), before);
    for _ in 0..70 {
        h.frame(vec![egui::Event::Text("a".into())]);
    }
    let cost = h.edit.field_journal_cost();
    assert_eq!(cost.0, before.0 + 70);
    assert_eq!(cost.1, 64);
    for _ in 0..64 {
        h.frame(vec![key(egui::Key::Z, egui::Modifiers::CTRL)]);
    }
    assert_eq!(
        h.count_y(),
        2,
        "evicted structural checkpoints cannot be replayed"
    );
    assert_eq!(h.edit.field_journal_cost().1, 0);
}

#[test]
fn committed_ime_is_one_field_command_without_preedit_history() {
    let mut h = Harness::new(source(1, 1));
    h.focus(false, 0, PlotFieldPart::Expression);
    h.frame(vec![key(egui::Key::Enter, egui::Modifiers::SHIFT)]);
    let (field, _) = h.focus(false, 1, PlotFieldPart::Expression);
    let before = h.edit.field_journal_cost();
    h.frame(vec![egui::Event::Ime(egui::ImeEvent::Preedit {
        text: "ㅎ".into(),
        active_range_chars: None,
    })]);
    h.frame(vec![egui::Event::Ime(egui::ImeEvent::Preedit {
        text: "하".into(),
        active_range_chars: None,
    })]);
    assert_eq!(h.edit.field_journal_cost(), before);
    h.frame(vec![egui::Event::Ime(egui::ImeEvent::Commit("한".into()))]);
    assert_eq!(
        inline_edit::field_expression(h.edit.draft().unwrap(), field),
        Some("한")
    );
    assert_eq!(h.edit.field_journal_cost().1, before.1 + 1);
    h.frame(vec![key(egui::Key::Z, egui::Modifiers::CTRL)]);
    assert_eq!(
        inline_edit::field_expression(h.edit.draft().unwrap(), field),
        Some("")
    );
    assert_eq!(h.count_y(), 2);
    h.frame(vec![key(egui::Key::Z, egui::Modifiers::CTRL)]);
    assert_eq!(h.count_y(), 1);
}
