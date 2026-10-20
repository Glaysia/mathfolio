//! First-math authority is a typed tree. Legacy text never enters this cache.
//! Unlike scaling fields, newly typed names keep the ordinary editor labels.
use super::{AxisField, InlinePlotEdit, fields};
use eframe::egui;
use mathcraft_document::canvas::{PlotExpression, PlotSource, PlotSpec};
use mathcraft_editor::Editor;
use std::collections::HashMap;
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct Key {
    x: bool,
    id: u64,
}
fn key(field: AxisField) -> Option<Key> {
    Some(Key {
        x: field.x,
        id: field.id?,
    })
}
pub(super) struct ExpressionSlot {
    editor: Editor,
    baseline: PlotExpression,
    revision: u64,
    generation: u64,
    widget: Option<egui::Id>,
    composing: bool,
    error: Option<String>,
}
impl Clone for ExpressionSlot {
    fn clone(&self) -> Self {
        Self {
            editor: self.editor.clone_without_history(),
            baseline: self.baseline.clone(),
            revision: self.revision,
            generation: self.generation,
            widget: self.widget,
            composing: self.composing,
            error: self.error.clone(),
        }
    }
}
#[derive(Clone, Default)]
pub(super) struct ExpressionFields {
    slots: HashMap<Key, ExpressionSlot>,
}
impl ExpressionFields {
    pub(super) fn clear(&mut self) {
        self.slots.clear();
    }
    pub(super) fn has_errors(&self) -> bool {
        self.slots.values().any(|s| s.error.is_some())
    }
    pub(super) fn error(&self) -> Option<&str> {
        self.slots.values().find_map(|s| s.error.as_deref())
    }
    pub(super) fn is_composing(&self) -> bool {
        self.slots.values().any(|s| s.composing)
    }
    pub(super) fn retain_fields(&mut self, fields: &[AxisField]) {
        self.slots
            .retain(|k, _| fields.iter().any(|f| key(*f) == Some(*k)));
    }
    pub(super) fn retain_typed_sources(&mut self, spec: &PlotSpec) {
        self.slots.retain(|k, _| {
            source(
                spec,
                AxisField {
                    x: k.x,
                    id: Some(k.id),
                    index: 0,
                },
            )
            .is_some_and(|v| v.as_tree().is_some())
        });
    }
    pub(super) fn accept(&mut self, spec: &PlotSpec) {
        for field in fields(spec) {
            if let Some(slot) = key(field).and_then(|k| self.slots.get_mut(&k))
                && let Some(value) = source(spec, field)
            {
                slot.baseline = value.clone();
            }
        }
    }
    pub(super) fn clone_with_history(&self) -> Self {
        Self {
            slots: self
                .slots
                .iter()
                .map(|(k, s)| {
                    (
                        *k,
                        ExpressionSlot {
                            editor: s.editor.clone(),
                            baseline: s.baseline.clone(),
                            revision: s.revision,
                            generation: s.generation,
                            widget: s.widget,
                            composing: s.composing,
                            error: s.error.clone(),
                        },
                    )
                })
                .collect(),
        }
    }
    pub(super) fn snapshot_field(&self, field: AxisField) -> Option<ExpressionSlot> {
        self.slots.get(&key(field)?).cloned()
    }
    pub(super) fn restore_field(&mut self, field: AxisField, saved: &ExpressionSlot) {
        if let Some(k) = key(field) {
            let mut s = saved.clone();
            s.revision = s.editor.arena().revision();
            self.slots.insert(k, s);
        }
    }
    pub(super) fn matches_field(&self, field: AxisField, saved: &ExpressionSlot) -> bool {
        key(field)
            .and_then(|k| self.slots.get(&k))
            .is_some_and(|s| {
                s.error == saved.error
                    && s.editor.arena().revision() == saved.editor.arena().revision()
            })
    }
    pub(super) fn same_field_value(&self, field: AxisField, saved: &ExpressionSlot) -> bool {
        key(field)
            .and_then(|k| self.slots.get(&k))
            .is_some_and(|s| {
                s.error == saved.error
                    && s.editor.to_portable().ok() == saved.editor.to_portable().ok()
            })
    }
}
pub(super) fn source(spec: &PlotSpec, field: AxisField) -> Option<&PlotExpression> {
    let PlotSource::AxisLists(a) = &spec.source else {
        return None;
    };
    (if field.x { &a.x } else { &a.y })
        .iter()
        .find(|e| Some(e.id) == field.id)
        .map(|e| &e.source)
}
pub(super) fn source_mut(spec: &mut PlotSpec, field: AxisField) -> Option<&mut PlotExpression> {
    let PlotSource::AxisLists(a) = &mut spec.source else {
        return None;
    };
    (if field.x { &mut a.x } else { &mut a.y })
        .iter_mut()
        .find(|e| Some(e.id) == field.id)
        .map(|e| &mut e.source)
}
impl InlinePlotEdit {
    pub(crate) fn widget_generation(&self) -> u64 {
        self.expression_widget_generation
    }
    pub(crate) fn ensure_widget_generation(&mut self, minimum: u64) {
        if minimum > self.expression_widget_generation {
            self.expression_widget_generation = minimum;
            self.expressions.clear();
        }
    }
    pub(crate) fn retire_widgets(&mut self) -> Vec<egui::Id> {
        let ids = self
            .expressions
            .slots
            .values()
            .filter_map(|s| s.widget)
            .collect();
        self.expression_widget_generation = self.expression_widget_generation.saturating_add(1);
        self.expressions.clear();
        ids
    }

    pub(in crate::plots) fn expression_is_typed(&self, spec: &PlotSpec, field: AxisField) -> bool {
        source(self.draft.as_ref().unwrap_or(spec), field).is_some_and(|v| v.as_tree().is_some())
    }
    pub(in crate::plots) fn expression_is_typed_for_field(&self, field: AxisField) -> bool {
        key(field).is_some_and(|k| self.expressions.slots.contains_key(&k))
    }
    pub(in crate::plots) fn expression_generation(&self, field: AxisField) -> Option<u64> {
        key(field)
            .and_then(|k| self.expressions.slots.get(&k))
            .map(|s| s.generation)
    }
    pub(in crate::plots) fn expression_widget_id(&self, field: AxisField) -> Option<egui::Id> {
        key(field)
            .and_then(|k| self.expressions.slots.get(&k))
            .and_then(|s| s.widget)
    }
    pub(in crate::plots) fn record_expression_widget(&mut self, field: AxisField, id: egui::Id) {
        if let Some(s) = key(field).and_then(|k| self.expressions.slots.get_mut(&k)) {
            s.widget = Some(id);
        }
    }
    pub(in crate::plots) fn expression_editor(
        &mut self,
        spec: &PlotSpec,
        field: AxisField,
    ) -> Result<&mut Editor, String> {
        let current = self.draft.as_ref().unwrap_or(spec);
        let value = source(current, field).ok_or("Stale typed plot expression identity")?;
        let k = key(field).ok_or("Missing typed plot expression identity")?;
        let reset = self
            .expressions
            .slots
            .get(&k)
            .is_none_or(|s| s.baseline != *value);
        if reset {
            let tree = value
                .as_tree()
                .ok_or("Legacy plot text is not a typed expression")?;
            let mut editor = Editor::from_portable(tree).map_err(|e| e.to_string())?;
            editor.set_expressions_only(true);
            let revision = editor.arena().revision();
            let generation = self
                .expression_widget_generation
                .checked_add(1)
                .ok_or("Plot expression widget generation exhausted")?;
            self.expression_widget_generation = generation;
            let widget = None;
            self.expressions.slots.insert(
                k,
                ExpressionSlot {
                    editor,
                    baseline: value.clone(),
                    revision,
                    generation,
                    widget,
                    composing: false,
                    error: None,
                },
            );
        }
        Ok(&mut self.expressions.slots.get_mut(&k).unwrap().editor)
    }
    pub(in crate::plots) fn stage_expression(
        &mut self,
        spec: &PlotSpec,
        field: AxisField,
    ) -> Result<bool, String> {
        let k = key(field).ok_or("Missing typed plot expression identity")?;
        let slot = self
            .expressions
            .slots
            .get_mut(&k)
            .ok_or("Typed expression editor was not prepared")?;
        let revision = slot.editor.arena().revision();
        if revision == slot.revision {
            return slot.error.clone().map_or(Ok(false), Err);
        }
        let value = slot
            .editor
            .to_portable()
            .map_err(|e| e.to_string())
            .and_then(PlotExpression::from_tree);
        slot.revision = revision;
        let value = match value {
            Ok(v) => v,
            Err(e) => {
                slot.error = Some(e.clone());
                return Err(e);
            }
        };
        let current = source(self.draft.as_ref().unwrap_or(spec), field)
            .ok_or("Stale typed plot expression identity")?;
        if *current == value {
            let slot = self.expressions.slots.get_mut(&k).unwrap();
            slot.baseline = value;
            slot.error = None;
            return Ok(false);
        }
        self.ensure_source_baseline(spec);
        let retained = value.clone();
        let result = if let Some(draft) = &mut self.draft {
            let old = std::mem::replace(
                source_mut(draft, field).ok_or("Stale typed plot expression identity")?,
                value,
            );
            if let Err(e) = draft.validate() {
                *source_mut(draft, field).unwrap() = old;
                Err(e)
            } else {
                Ok(())
            }
        } else {
            let mut candidate = spec.clone();
            *source_mut(&mut candidate, field).ok_or("Stale typed plot expression identity")? =
                value;
            candidate.validate().map(|()| {
                self.draft = Some(candidate);
                #[cfg(test)]
                {
                    self.draft_creations += 1;
                }
            })
        };
        match result {
            Ok(()) => {
                let slot = self.expressions.slots.get_mut(&k).unwrap();
                slot.baseline = retained;
                slot.error = None;
                self.slots.clear_redo();
                self.normalize(spec);
                Ok(true)
            }
            Err(e) => {
                self.expressions.slots.get_mut(&k).unwrap().error = Some(e.clone());
                Err(e)
            }
        }
    }
    #[allow(deprecated)]
    pub(in crate::plots) fn observe_expression_ime(
        &mut self,
        field: AxisField,
        events: &[egui::Event],
    ) -> bool {
        let Some(slot) = key(field).and_then(|k| self.expressions.slots.get_mut(&k)) else {
            return false;
        };
        let mut guard = slot.composing;
        for event in events {
            if let egui::Event::Ime(event) = event {
                match event {
                    egui::ImeEvent::Preedit { text, .. } => {
                        slot.composing = !text.is_empty();
                        guard |= slot.composing;
                    }
                    egui::ImeEvent::Commit(_) => {
                        slot.composing = false;
                        guard = true;
                    }
                    egui::ImeEvent::Disabled => slot.composing = false,
                    _ => (),
                }
            }
        }
        guard
    }
    pub(in crate::plots) fn expression_focus_changed(
        &mut self,
        ctx: &egui::Context,
        active: Option<AxisField>,
    ) {
        for (k, s) in &mut self.expressions.slots {
            if active.and_then(key) != Some(*k) {
                s.composing = false;
                if let Some(id) = s.widget {
                    ctx.data_mut(|d| d.remove::<String>(id.with("ime-preedit")));
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "expression_field_tests.rs"]
mod tests;
