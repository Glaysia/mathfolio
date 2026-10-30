//! Axis-list authoring policy: source/result installation remains an owner commit.
//! Native command/identity/focus evidence: analysis/0157,0158,0161 and the ignored
//! xy-slot-key-next-plan. This draft transaction is an app integration policy.
use super::{
    AxisField, ExpressionFields, InlinePlotEdit, PlotFieldPart, PlotSpec, ScalingFields, fields,
};
use eframe::egui;
use mathfolio_document::canvas::{PlotAxis, PlotTraceAppearance, PlotTraceStyle};
use mathfolio_editor::Direction;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::plots) enum SlotFocusEdge {
    Start,
    End,
    Preserve,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::plots) struct SlotFocus {
    pub field: AxisField,
    pub part: PlotFieldPart,
    pub edge: SlotFocusEdge,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::plots) struct SlotEditOutcome {
    pub changed: bool,
    pub focus: SlotFocus,
}
pub(super) struct Checkpoint {
    before: Option<PlotSpec>,
    after: PlotSpec,
    before_scaling: ScalingFields,
    after_scaling: ScalingFields,
    before_expressions: ExpressionFields,
    after_expressions: ExpressionFields,
    before_focus: SlotFocus,
    ui_before: Option<super::field_journal::WidgetHistory>,
    ui_after: Option<super::field_journal::WidgetHistory>,
    after_focus: SlotFocus,
}
pub(super) enum Entry {
    Structural(Box<Checkpoint>),
    Field(Box<super::field_journal::FieldCheckpoint>),
}
#[derive(Default)]
pub(super) struct SlotEdits {
    pub(super) undo: Vec<Entry>,
    pub(super) redo: Vec<Entry>,
    pub(super) pending_ui: Option<super::field_journal::WidgetHistory>,
    pub(super) field_widgets: std::collections::HashSet<egui::Id>,
    focus: Option<SlotFocus>,
    pub(super) native_history_command: bool,
    palette_cursor: usize,
    palette_initialized: bool,
    equation_id_floor: u64,
}
impl SlotEdits {
    pub(super) fn clear_history(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.focus = None;
        self.pending_ui = None;
        self.field_widgets.clear();
    }
    pub(super) fn clear_redo(&mut self) {
        if !self.native_history_command {
            self.redo.clear();
        }
    }
    pub(super) fn has_history(&self) -> bool {
        !self.undo.is_empty() || !self.redo.is_empty()
    }
}
fn axis(field: AxisField) -> PlotAxis {
    if field.x { PlotAxis::X } else { PlotAxis::Y }
}
fn locate(spec: &PlotSpec, field: AxisField) -> Result<AxisField, String> {
    fields(spec)
        .into_iter()
        .find(|f| {
            f.x == field.x
                && if field.id.is_some() {
                    f.id == field.id
                } else {
                    f.id.is_none() && f.index == field.index
                }
        })
        .ok_or_else(|| "Stale plot equation identity".into())
}
fn focus(
    spec: &PlotSpec,
    x: bool,
    index: usize,
    part: PlotFieldPart,
    edge: SlotFocusEdge,
) -> SlotFocus {
    SlotFocus {
        field: fields(spec)
            .into_iter()
            .find(|f| f.x == x && f.index == index)
            .expect("validated axis index"),
        part,
        edge,
    }
}
fn appearance(cursor: usize) -> PlotTraceAppearance {
    // Original named WPF brushes, 0157. This counter is session state, not source.
    const COLORS: [[u8; 4]; 7] = [
        [0, 0, 139, 255],
        [0, 0, 0, 255],
        [255, 0, 0, 255],
        [255, 255, 0, 255],
        [255, 165, 0, 255],
        [0, 128, 0, 255],
        [0, 0, 255, 255],
    ];
    PlotTraceAppearance {
        label: String::new(),
        style: PlotTraceStyle {
            color: COLORS[cursor % 7],
            line_width: 1.,
            ..Default::default()
        },
    }
}
impl InlinePlotEdit {
    pub(super) fn ensure_source_baseline(&mut self, source: &PlotSpec) {
        if self.source_baseline.is_none() {
            self.source_baseline = Some(source.source.clone());
        }
    }
    /// Owners must check immediately before installing a draft, even when no
    /// draw occurred after an external edit. Geometry is deliberately separate.
    pub(crate) fn validate_source(&self, source: &PlotSpec) -> Result<(), String> {
        if self
            .source_baseline
            .as_ref()
            .is_some_and(|old| *old != source.source)
        {
            Err("Plot source changed outside its inline draft".into())
        } else {
            Ok(())
        }
    }
    pub(crate) fn palette_cursor(&self) -> usize {
        self.slots.palette_cursor
    }
    pub(crate) fn ensure_palette_cursor(&mut self, minimum: usize) {
        self.slots.palette_cursor = self.slots.palette_cursor.max(minimum);
        self.slots.palette_initialized = true;
    }
    pub(crate) fn equation_id_floor(&self) -> u64 {
        self.slots.equation_id_floor
    }
    pub(crate) fn ensure_equation_id_floor(&mut self, minimum: u64) {
        self.slots.equation_id_floor = self.slots.equation_id_floor.max(minimum);
    }
    pub(in crate::plots) fn take_slot_focus(&mut self) -> Option<SlotFocus> {
        self.slots.focus.take()
    }
    pub(in crate::plots) fn can_undo_slot(&self) -> bool {
        !self.is_composing()
            && self.slots.undo.last().is_some_and(|entry| match entry {
                Entry::Structural(c) => {
                    !self.scaling.has_errors()
                        && !self.expressions.has_errors()
                        && self.draft.as_ref() == Some(&c.after)
                }
                Entry::Field(c) => self.matches_field(&c.after),
            })
    }
    pub(in crate::plots) fn can_redo_slot(&self) -> bool {
        !self.is_composing()
            && self.slots.redo.last().is_some_and(|entry| match entry {
                Entry::Structural(c) => {
                    !self.scaling.has_errors()
                        && !self.expressions.has_errors()
                        && self.draft == c.before
                }
                Entry::Field(c) => self.matches_field(&c.before),
            })
    }
    pub(crate) fn undo_structure(&mut self) -> bool {
        self.undo_slot().is_some()
    }
    pub(crate) fn redo_structure(&mut self) -> bool {
        self.redo_slot().is_some()
    }
    pub(in crate::plots) fn undo_slot(&mut self) -> Option<SlotFocus> {
        if !self.can_undo_slot() {
            return None;
        }
        let entry = self.slots.undo.pop().unwrap();
        let focus = match &entry {
            Entry::Structural(c) => {
                self.draft = c.before.clone();
                self.scaling = c.before_scaling.clone_with_history();
                self.expressions = c.before_expressions.clone_with_history();
                self.slots.pending_ui = c.ui_before.clone();
                c.before_focus
            }
            Entry::Field(c) => {
                self.restore_field(&c.before);
                self.slots.pending_ui = c.before_ui.clone();
                c.before.focus
            }
        };
        self.slots.redo.push(entry);
        self.rebuild_field_boundaries();
        self.slots.focus = Some(focus);
        Some(focus)
    }
    pub(in crate::plots) fn redo_slot(&mut self) -> Option<SlotFocus> {
        if !self.can_redo_slot() {
            return None;
        }
        let entry = self.slots.redo.pop().unwrap();
        let focus = match &entry {
            Entry::Structural(c) => {
                self.draft = Some(c.after.clone());
                self.scaling = c.after_scaling.clone_with_history();
                self.expressions = c.after_expressions.clone_with_history();
                self.slots.pending_ui = c.ui_after.clone();
                c.after_focus
            }
            Entry::Field(c) => {
                self.restore_field(&c.after);
                self.slots.pending_ui = Some(c.after_ui());
                c.after.focus
            }
        };
        self.slots.undo.push(entry);
        self.rebuild_field_boundaries();
        self.slots.focus = Some(focus);
        Some(focus)
    }
    pub(in crate::plots) fn attach_slot_widget_history(
        &mut self,
        ui: &egui::Ui,
        before: SlotFocus,
        before_id: egui::Id,
        after: SlotFocus,
    ) {
        let before_ui = (before.part == PlotFieldPart::Expression
            && !self.expression_is_typed(self.draft.as_ref().unwrap(), before.field))
        .then(|| super::field_journal::WidgetHistory::capture(ui.ctx(), before_id));
        let after_ui = (after.part == PlotFieldPart::Expression
            && !self.expression_is_typed(self.draft.as_ref().unwrap(), after.field))
        .then(|| {
            let id = self.field_id(ui, after.field);
            let text =
                super::field_expression(self.draft.as_ref().unwrap(), after.field).unwrap_or("");
            super::field_journal::WidgetHistory::capture_or_new(ui.ctx(), id, text)
        });
        if let Some(Entry::Structural(c)) = self.slots.undo.last_mut() {
            c.ui_before = before_ui;
            c.ui_after = after_ui;
        }
    }
    fn require_slot_edit(&self, source: &PlotSpec) -> Result<(), String> {
        self.validate_source(source)?;
        if self.is_composing() {
            return Err("Confirm the current composition before editing plot slots".into());
        }
        if let Some(error) = self.validation_error() {
            return Err(error.to_owned());
        }
        Ok(())
    }
    fn install_slot_candidate(
        &mut self,
        source: &PlotSpec,
        candidate: PlotSpec,
        before_focus: SlotFocus,
        after_focus: SlotFocus,
    ) {
        self.ensure_source_baseline(source);
        // Preserve pre-owner editor history once at the first structural boundary.
        // Later structural checkpoints are already covered by the mixed journal.
        let before_scaling = if self
            .slots
            .undo
            .iter()
            .any(|e| matches!(e, Entry::Structural(_)))
        {
            self.scaling.clone()
        } else {
            self.scaling.clone_with_history()
        };
        let before_expressions = if self
            .slots
            .undo
            .iter()
            .any(|e| matches!(e, Entry::Structural(_)))
        {
            self.expressions.clone()
        } else {
            self.expressions.clone_with_history()
        };
        self.scaling.retain_fields(&fields(&candidate));
        self.expressions.retain_fields(&fields(&candidate));
        let mut checkpoint = Checkpoint {
            before: self.draft.clone(),
            after: candidate.clone(),
            before_scaling,
            before_expressions,
            after_expressions: self.expressions.clone(),
            after_scaling: self.scaling.clone(),
            before_focus,
            after_focus,
            ui_before: None,
            ui_after: None,
        };
        if self.draft.is_none() {
            #[cfg(test)]
            {
                self.draft_creations += 1;
            }
        }
        self.draft = Some(candidate);
        // Freeze the new empty editors at the structural boundary. Recreating
        // them on Redo would give a different arena revision and break the
        // following field journal entry even when its source is identical.
        if self.expression_is_typed(source, after_focus.field) {
            let _ = self.expression_editor(source, after_focus.field);
        }
        let _ = self.scaling_editor(source, after_focus.field);
        checkpoint.after_expressions = self.expressions.clone();
        checkpoint.after_scaling = self.scaling.clone();
        if self.slots.undo.len() == 64 {
            self.slots.undo.remove(0);
        } // App history budget.
        self.slots
            .undo
            .push(Entry::Structural(Box::new(checkpoint)));
        self.slots.field_widgets.clear();
        self.slots.redo.clear();
        self.slots.focus = Some(after_focus);
    }
    /// Called only after the focused inner editor declined its row command.
    pub(in crate::plots) fn insert_slot(
        &mut self,
        source: &PlotSpec,
        field: AxisField,
        part: PlotFieldPart,
        expression_at_left_edge: bool,
    ) -> Result<SlotEditOutcome, String> {
        self.require_slot_edit(source)?;
        let current = self.draft.as_ref().unwrap_or(source);
        let field = locate(current, field)?;
        let expression =
            super::field_expression_ref(current, field).ok_or("Missing plot expression")?;
        let typed = expression.as_tree().is_some();
        let index = field.index
            + usize::from(
                part == PlotFieldPart::Scaling
                    || !expression_at_left_edge
                    || expression.is_exact_placeholder(),
            );
        let new_trace = current.axis_insertion_requires_trace(axis(field))?;
        let cursor = if self.slots.palette_initialized {
            self.slots.palette_cursor
        } else {
            current.trace_count()
        };
        let next = if new_trace {
            cursor.checked_add(1).ok_or("Plot style cursor overflow")?
        } else {
            cursor
        };
        let mut candidate = current.clone();
        candidate.promote_axis_lists()?;
        let lists = candidate.axis_lists_mut().unwrap();
        lists.next_equation_id = lists.next_equation_id.max(self.slots.equation_id_floor);
        if typed {
            candidate.insert_typed_axis_equation(
                axis(field),
                index,
                mathfolio_editor::PortableTree {
                    kind: mathfolio_editor::NodeKind::Placeholder,
                    children: vec![],
                },
                new_trace.then(|| appearance(cursor)),
            )?;
        } else {
            candidate.insert_axis_equation(
                axis(field),
                index,
                "",
                new_trace.then(|| appearance(cursor)),
            )?;
        }
        let target = focus(
            &candidate,
            field.x,
            index,
            PlotFieldPart::Expression,
            SlotFocusEdge::Start,
        );
        let before = SlotFocus {
            field,
            part,
            edge: SlotFocusEdge::Preserve,
        };
        let floor = candidate.axis_lists().unwrap().next_equation_id;
        self.install_slot_candidate(source, candidate, before, target);
        self.slots.equation_id_floor = floor;
        self.slots.palette_cursor = next;
        self.slots.palette_initialized = true;
        Ok(SlotEditOutcome {
            changed: true,
            focus: target,
        })
    }
    /// None means nonempty and leaves the key to its ordinary focused editor.
    /// Some(false) consumes the sole empty pair without allocating or migrating.
    pub(in crate::plots) fn remove_empty_slot(
        &mut self,
        source: &PlotSpec,
        field: AxisField,
        part: PlotFieldPart,
        direction: Direction,
    ) -> Result<Option<SlotEditOutcome>, String> {
        self.require_slot_edit(source)?;
        if !matches!(direction, Direction::Left | Direction::Right) {
            return Err("Invalid plot slot deletion direction".into());
        }
        let current = self.draft.as_ref().unwrap_or(source);
        let field = locate(current, field)?;
        if !super::field_is_placeholder(current, field)
            || !super::scaling_field::is_empty(current, field)
        {
            return Ok(None);
        }
        let before = SlotFocus {
            field,
            part,
            edge: SlotFocusEdge::Preserve,
        };
        let count = current.axis_equation_count(axis(field));
        if count == 1 {
            return Ok(Some(SlotEditOutcome {
                changed: false,
                focus: before,
            }));
        }
        let index = if direction == Direction::Left {
            field.index.saturating_sub(1)
        } else {
            field.index.min(count - 2)
        };
        let mut candidate = current.clone();
        candidate.remove_axis_equation(axis(field), field.index)?;
        let target = focus(
            &candidate,
            field.x,
            index,
            if direction == Direction::Left {
                PlotFieldPart::Scaling
            } else {
                PlotFieldPart::Expression
            },
            if direction == Direction::Left {
                SlotFocusEdge::End
            } else {
                SlotFocusEdge::Start
            },
        );
        self.install_slot_candidate(source, candidate, before, target);
        Ok(Some(SlotEditOutcome {
            changed: true,
            focus: target,
        }))
    }
}

#[cfg(test)]
#[path = "slot_edit_tests.rs"]
mod tests;
