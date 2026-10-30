//! Mixed owner history only after structural authoring. Field entries retain one
//! edited value/caret, never a full plot or a per-key native undo stack.
use super::scaling_field::{self, ScalingSlot};
use super::slot_edit::Entry;
use super::{AxisField, InlinePlotEdit, PlotFieldPart, PlotSpec, SlotFocus, SlotFocusEdge};
use eframe::egui::{
    self,
    text::{CCursor, CCursorRange, CharIndex},
};

#[derive(Clone)]
pub(super) struct WidgetHistory {
    id: egui::Id,
    cursor: Option<CCursorRange>,
    undoer: Option<egui::util::undoer::Undoer<(CCursorRange, String)>>,
}
impl WidgetHistory {
    pub(super) fn capture(ctx: &egui::Context, id: egui::Id) -> Self {
        let state = egui::text_edit::TextEditState::load(ctx, id).unwrap_or_default();
        Self {
            id,
            cursor: state.cursor.char_range(),
            undoer: Some(state.undoer()),
        }
    }
    pub(super) fn capture_or_new(ctx: &egui::Context, id: egui::Id, _text: &str) -> Self {
        Self::capture(ctx, id)
    }
    fn cursor(ctx: &egui::Context, id: egui::Id) -> Self {
        Self {
            id,
            cursor: egui::text_edit::TextEditState::load(ctx, id)
                .and_then(|s| s.cursor.char_range()),
            undoer: None,
        }
    }
    fn install(&self, ctx: &egui::Context) {
        let mut state = egui::text_edit::TextEditState::load(ctx, self.id).unwrap_or_default();
        state.cursor.set_char_range(self.cursor);
        state.set_undoer(self.undoer.clone().unwrap_or_default());
        state.store(ctx, self.id);
    }
}
pub(super) enum Value {
    Expression(String),
    TypedExpression {
        source: mathfolio_document::canvas::PlotExpression,
        slot: Box<super::expression_field::ExpressionSlot>,
    },
    Scaling {
        source: Option<mathfolio_document::canvas::PlotScaling>,
        slot: Box<ScalingSlot>,
    },
}
pub(super) struct FieldSnapshot {
    pub(super) focus: SlotFocus,
    id: egui::Id,
    value: Value,
    ui: WidgetHistory,
}
pub(super) struct FieldCheckpoint {
    pub(super) before: FieldSnapshot,
    pub(super) after: FieldSnapshot,
    pub(super) before_ui: Option<WidgetHistory>,
}
impl FieldCheckpoint {
    pub(super) fn after_ui(&self) -> WidgetHistory {
        self.after.ui.clone()
    }
}
pub(in crate::plots) struct PendingField {
    before: FieldSnapshot,
    boundary: Option<WidgetHistory>,
}
impl InlinePlotEdit {
    fn field_snapshot(
        &self,
        ctx: &egui::Context,
        source: &PlotSpec,
        field: AxisField,
        part: PlotFieldPart,
        id: egui::Id,
    ) -> Option<FieldSnapshot> {
        let spec = self.draft.as_ref().unwrap_or(source);
        let value = match part {
            PlotFieldPart::Expression => {
                if let Some(value) =
                    super::expression_field::source(spec, field).filter(|v| v.as_tree().is_some())
                {
                    Value::TypedExpression {
                        source: value.clone(),
                        slot: Box::new(self.expressions.snapshot_field(field)?),
                    }
                } else {
                    Value::Expression(super::field_expression(spec, field)?.to_owned())
                }
            }
            PlotFieldPart::Scaling => Value::Scaling {
                source: scaling_field::scaling(spec, field)?.clone(),
                slot: Box::new(self.scaling.snapshot_field(field)?),
            },
        };
        Some(FieldSnapshot {
            focus: SlotFocus {
                field,
                part,
                edge: SlotFocusEdge::Preserve,
            },
            id,
            value,
            ui: WidgetHistory::cursor(ctx, id),
        })
    }
    pub(in crate::plots) fn begin_field_command(
        &mut self,
        ui: &egui::Ui,
        source: &PlotSpec,
        field: AxisField,
        part: PlotFieldPart,
        id: egui::Id,
    ) -> Option<PendingField> {
        if self.slots.undo.is_empty() || !ui.memory(|m| m.has_focus(id)) {
            return None;
        }
        let (potential, commit, preedit) = ui.input(|i| {
            let mut potential = false;
            let mut commit = false;
            let mut preedit = false;
            for e in &i.events {
                match e {
                    egui::Event::Text(_) | egui::Event::Paste(_) | egui::Event::Cut => {
                        potential = true
                    }
                    egui::Event::Ime(egui::ImeEvent::Commit(_)) => {
                        potential = true;
                        commit = true;
                    }
                    egui::Event::Ime(egui::ImeEvent::Preedit { .. }) => preedit = true,
                    egui::Event::Key {
                        key, pressed: true, ..
                    } if !matches!(
                        key,
                        egui::Key::ArrowLeft
                            | egui::Key::ArrowRight
                            | egui::Key::ArrowUp
                            | egui::Key::ArrowDown
                            | egui::Key::Tab
                            | egui::Key::Enter
                            | egui::Key::Escape
                    ) =>
                    {
                        potential = true
                    }
                    _ => (),
                }
            }
            (potential, commit, preedit)
        });
        if !potential || (self.is_composing() || preedit) && !commit {
            return None;
        }
        let mut before = self.field_snapshot(ui.ctx(), source, field, part, id)?;
        if part == PlotFieldPart::Expression
            && commit
            && let Some(saved) = &self.composition_checkpoint
            && saved.field == field
        {
            before.value = Value::Expression(saved.text.clone());
            before.ui.cursor = Some(CCursorRange::one(CCursor::new(CharIndex(saved.caret))));
        }
        let boundary =
            (!self.slots.field_widgets.contains(&id)).then(|| WidgetHistory::capture(ui.ctx(), id));
        #[cfg(test)]
        {
            self.field_snapshots += 1;
        }
        Some(PendingField { before, boundary })
    }
    pub(in crate::plots) fn finish_field_command(
        &mut self,
        ui: &egui::Ui,
        source: &PlotSpec,
        pending: Option<PendingField>,
    ) {
        let Some(p) = pending else {
            return;
        };
        let same = match &p.before.value {
            Value::Expression(_) => self.matches_field(&p.before),
            Value::TypedExpression {
                source: saved,
                slot,
            } => {
                super::expression_field::source(
                    self.draft.as_ref().unwrap_or(source),
                    p.before.focus.field,
                ) == Some(saved)
                    && self
                        .expressions
                        .same_field_value(p.before.focus.field, slot)
            }
            Value::Scaling {
                source: saved,
                slot,
            } => {
                scaling_field::scaling(self.draft.as_ref().unwrap_or(source), p.before.focus.field)
                    == Some(saved)
                    && self.scaling.same_field_value(p.before.focus.field, slot)
            }
        };
        if same {
            return;
        }
        let f = p.before.focus;
        let Some(after) = self.field_snapshot(ui.ctx(), source, f.field, f.part, p.before.id)
        else {
            return;
        };
        let before_ui = p.boundary.or_else(|| Some(p.before.ui.clone()));
        self.slots.field_widgets.insert(p.before.id);
        self.slots.redo.clear();
        if self.slots.undo.len() == 64 {
            self.slots.undo.remove(0);
        }
        self.slots.undo.push(Entry::Field(Box::new(FieldCheckpoint {
            before: p.before,
            after,
            before_ui,
        })));
    }
    pub(super) fn matches_field(&self, saved: &FieldSnapshot) -> bool {
        let Some(spec) = self.draft.as_ref() else {
            return false;
        };
        match &saved.value {
            Value::Expression(text) => {
                super::field_expression(spec, saved.focus.field) == Some(text.as_str())
            }
            Value::TypedExpression { source, slot } => {
                super::expression_field::source(spec, saved.focus.field) == Some(source)
                    && self.expressions.matches_field(saved.focus.field, slot)
            }
            Value::Scaling { source, slot } => {
                scaling_field::scaling(spec, saved.focus.field) == Some(source)
                    && self.scaling.matches_field(saved.focus.field, slot)
            }
        }
    }
    pub(super) fn restore_field(&mut self, saved: &FieldSnapshot) {
        let Some(spec) = self.draft.as_mut() else {
            return;
        };
        match &saved.value {
            Value::Expression(text) => {
                if let Some(v) = super::field_expression_mut(spec, saved.focus.field) {
                    v.clone_from(text);
                }
            }
            Value::TypedExpression { source, slot } => {
                if let Some(v) = super::expression_field::source_mut(spec, saved.focus.field) {
                    v.clone_from(source);
                }
                self.expressions.restore_field(saved.focus.field, slot);
            }
            Value::Scaling { source, slot } => {
                if let Some(v) = scaling_field::scaling_mut(spec, saved.focus.field) {
                    v.clone_from(source);
                }
                self.scaling.restore_field(saved.focus.field, slot);
            }
        }
    }
    pub(super) fn rebuild_field_boundaries(&mut self) {
        self.slots.field_widgets.clear();
        for e in self.slots.undo.iter().rev() {
            match e {
                Entry::Structural(_) => break,
                Entry::Field(c) => {
                    self.slots.field_widgets.insert(c.before.id);
                }
            }
        }
    }
    #[cfg(test)]
    pub(in crate::plots) fn field_journal_cost(&self) -> (usize, usize) {
        (self.field_snapshots, self.slots.undo.len())
    }
    pub(in crate::plots) fn observe_native_history_command(&mut self, ui: &egui::Ui) {
        self.slots.native_history_command=ui.input(|i|i.events.iter().any(|event|matches!(event,egui::Event::Key{key:egui::Key::Z|egui::Key::Y,pressed:true,modifiers,..} if modifiers.command&&!modifiers.shift&&!modifiers.alt)));
    }
    pub(in crate::plots) fn apply_pending_widget_history(&mut self, ctx: &egui::Context) {
        if let Some(saved) = self.slots.pending_ui.take() {
            saved.install(ctx);
        }
    }
}
