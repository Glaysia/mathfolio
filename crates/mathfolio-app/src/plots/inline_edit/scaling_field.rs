//! Retained expression-only scaling input. User source is always a typed tree;
//! automatic units are deliberately not an input to this adapter (0170/0174).
use super::{AxisField, InlinePlotEdit, fields};
use eframe::egui;
use mathfolio_document::canvas::{PlotScaling, PlotSource, PlotSpec};
use mathfolio_editor::{Editor, NodeKind, PortableTree};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
struct Key {
    x: bool,
    id: Option<u64>,
    index: usize,
}
impl From<AxisField> for Key {
    fn from(field: AxisField) -> Self {
        Self {
            x: field.x,
            id: field.id,
            index: if field.id.is_some() { 0 } else { field.index },
        }
    }
}
pub(super) struct ScalingSlot {
    editor: Editor,
    baseline: Option<PlotScaling>,
    revision: u64,
    widget: Option<egui::Id>,
    composing: bool,
    error: Option<String>,
}
impl Clone for ScalingSlot {
    fn clone(&self) -> Self {
        Self {
            editor: self.editor.clone_without_history(),
            baseline: self.baseline.clone(),
            revision: self.revision,
            widget: self.widget,
            composing: self.composing,
            error: self.error.clone(),
        }
    }
}
#[derive(Clone, Default)]
pub(super) struct ScalingFields {
    slots: HashMap<Key, ScalingSlot>,
}
impl ScalingFields {
    pub(super) fn clear(&mut self) {
        self.slots.clear();
    }
    pub(super) fn has_errors(&self) -> bool {
        self.slots.values().any(|slot| slot.error.is_some())
    }
    pub(super) fn is_composing(&self) -> bool {
        self.slots.values().any(|slot| slot.composing)
    }
    pub(super) fn clone_with_history(&self) -> Self {
        Self {
            slots: self
                .slots
                .iter()
                .map(|(key, slot)| {
                    (
                        *key,
                        ScalingSlot {
                            editor: slot.editor.clone(),
                            baseline: slot.baseline.clone(),
                            revision: slot.revision,
                            widget: slot.widget,
                            composing: slot.composing,
                            error: slot.error.clone(),
                        },
                    )
                })
                .collect(),
        }
    }
    pub(super) fn snapshot_field(&self, field: AxisField) -> Option<ScalingSlot> {
        self.slots.get(&field.into()).cloned()
    }
    pub(super) fn restore_field(&mut self, field: AxisField, saved: &ScalingSlot) {
        let mut slot = saved.clone();
        slot.revision = slot.editor.arena().revision();
        self.slots.insert(field.into(), slot);
    }
    pub(super) fn matches_field(&self, field: AxisField, saved: &ScalingSlot) -> bool {
        self.slots.get(&field.into()).is_some_and(|slot| {
            slot.error == saved.error
                && slot.editor.arena().revision() == saved.editor.arena().revision()
        })
    }
    pub(super) fn same_field_value(&self, field: AxisField, saved: &ScalingSlot) -> bool {
        self.slots.get(&field.into()).is_some_and(|slot| {
            slot.error == saved.error
                && slot.editor.to_portable().ok() == saved.editor.to_portable().ok()
        })
    }
    pub(super) fn retain_fields(&mut self, fields: &[AxisField]) {
        self.slots
            .retain(|key, _| fields.iter().any(|field| Key::from(*field) == *key));
    }
    pub(super) fn accept(&mut self, spec: &PlotSpec) {
        for field in fields(spec) {
            if let Some(slot) = self.slots.get_mut(&field.into()) {
                slot.baseline = scaling(spec, field).and_then(Option::as_ref).cloned();
            }
        }
    }
}
pub(super) fn scaling(spec: &PlotSpec, field: AxisField) -> Option<&Option<PlotScaling>> {
    match &spec.source {
        PlotSource::AxisLists(a) => (if field.x { &a.x } else { &a.y })
            .iter()
            .find(|e| Some(e.id) == field.id)
            .map(|e| &e.scaling),
        PlotSource::Legacy {
            x_scaling,
            y_scaling,
            traces,
            ..
        } => {
            if traces.is_empty() {
                (field.index == 0).then_some(if field.x { x_scaling } else { y_scaling })
            } else {
                traces
                    .get(field.index)
                    .map(|t| if field.x { &t.x_scaling } else { &t.y_scaling })
            }
        }
    }
}
pub(super) fn is_empty(spec: &PlotSpec, field: AxisField) -> bool {
    scaling(spec, field).is_some_and(Option::is_none)
}
pub(super) fn scaling_mut(
    spec: &mut PlotSpec,
    field: AxisField,
) -> Option<&mut Option<PlotScaling>> {
    match &mut spec.source {
        PlotSource::AxisLists(a) => (if field.x { &mut a.x } else { &mut a.y })
            .iter_mut()
            .find(|e| Some(e.id) == field.id)
            .map(|e| &mut e.scaling),
        PlotSource::Legacy {
            x_scaling,
            y_scaling,
            traces,
            ..
        } => {
            if traces.is_empty() {
                (field.index == 0).then_some(if field.x { x_scaling } else { y_scaling })
            } else {
                traces.get_mut(field.index).map(|t| {
                    if field.x {
                        &mut t.x_scaling
                    } else {
                        &mut t.y_scaling
                    }
                })
            }
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::plots) enum PlotFieldPart {
    Expression,
    Scaling,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::plots) struct PlotField {
    pub axis: AxisField,
    pub part: PlotFieldPart,
}
pub(in crate::plots) fn traversal_fields(fields: &[AxisField]) -> Vec<PlotField> {
    fields
        .iter()
        .flat_map(|axis| {
            [
                PlotField {
                    axis: *axis,
                    part: PlotFieldPart::Expression,
                },
                PlotField {
                    axis: *axis,
                    part: PlotFieldPart::Scaling,
                },
            ]
        })
        .collect()
}

impl InlinePlotEdit {
    pub(in crate::plots) fn scaling_id(&self, ui: &egui::Ui, field: AxisField) -> egui::Id {
        self.field_id(ui, field).with("scaling")
    }
    pub(in crate::plots) fn scaling_widget_id(&self, field: AxisField) -> Option<egui::Id> {
        self.scaling
            .slots
            .get(&field.into())
            .and_then(|slot| slot.widget)
    }
    pub(in crate::plots) fn record_scaling_widget(&mut self, field: AxisField, id: egui::Id) {
        if let Some(slot) = self.scaling.slots.get_mut(&field.into()) {
            slot.widget = Some(id);
        }
    }
    pub(crate) fn validation_error(&self) -> Option<&str> {
        self.expressions.error().or_else(|| {
            self.scaling
                .slots
                .values()
                .find_map(|slot| slot.error.as_deref())
        })
    }
    pub(in crate::plots) fn scaling_editor(
        &mut self,
        source: &PlotSpec,
        field: AxisField,
    ) -> Result<&mut Editor, String> {
        let current = self.draft.as_ref().unwrap_or(source);
        let value = scaling(current, field).ok_or("Stale plot scaling identity")?;
        let key = field.into();
        let reset = self
            .scaling
            .slots
            .get(&key)
            .is_none_or(|slot| slot.baseline != *value);
        if reset {
            let tree = value.as_ref().map_or_else(
                || PortableTree {
                    kind: NodeKind::Placeholder,
                    children: vec![],
                },
                |v| v.as_tree().clone(),
            );
            let mut editor = Editor::from_portable(&tree).map_err(|e| e.to_string())?;
            editor.set_expressions_only(true);
            let revision = editor.arena().revision();
            let widget = self.scaling.slots.get(&key).and_then(|s| s.widget);
            self.scaling.slots.insert(
                key,
                ScalingSlot {
                    editor,
                    baseline: value.clone(),
                    revision,
                    widget,
                    composing: false,
                    error: None,
                },
            );
        }
        Ok(&mut self.scaling.slots.get_mut(&key).unwrap().editor)
    }
    pub(in crate::plots) fn stage_scaling(
        &mut self,
        source: &PlotSpec,
        field: AxisField,
    ) -> Result<bool, String> {
        let key = field.into();
        let slot = self
            .scaling
            .slots
            .get_mut(&key)
            .ok_or("Scaling editor was not prepared")?;
        let revision = slot.editor.arena().revision();
        if revision == slot.revision {
            return slot.error.clone().map_or(Ok(false), Err);
        }
        let candidate = slot
            .editor
            .to_portable()
            .map_err(|e| e.to_string())
            .and_then(PlotScaling::from_tree);
        slot.revision = revision;
        let value = match candidate {
            Ok(value) => value,
            Err(error) => {
                slot.error = Some(error.clone());
                return Err(error);
            }
        };
        let Some(current) = scaling(self.draft.as_ref().unwrap_or(source), field) else {
            let error = "Stale plot scaling identity".to_owned();
            self.scaling.slots.get_mut(&key).unwrap().error = Some(error.clone());
            return Err(error);
        };
        if *current == value {
            let slot = self.scaling.slots.get_mut(&key).unwrap();
            slot.baseline = value;
            slot.error = None;
            return Ok(false);
        }
        self.ensure_source_baseline(source);
        let retained = value.clone();
        let result = if let Some(draft) = &mut self.draft {
            let target = scaling_mut(draft, field).ok_or("Stale plot scaling identity")?;
            let previous = std::mem::replace(target, value);
            if let Err(error) = draft.validate() {
                *scaling_mut(draft, field).unwrap() = previous;
                Err(error)
            } else {
                Ok(())
            }
        } else {
            let mut candidate = source.clone();
            *scaling_mut(&mut candidate, field).ok_or("Stale plot scaling identity")? = value;
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
                let slot = self.scaling.slots.get_mut(&key).unwrap();
                slot.baseline = retained;
                slot.error = None;
                self.slots.clear_redo();
                self.normalize(source);
                Ok(true)
            }
            Err(error) => {
                self.scaling.slots.get_mut(&key).unwrap().error = Some(error.clone());
                Err(error)
            }
        }
    }
    /// Call only for this field's actual focused widget. Formula preedit stays
    /// outside the Editor tree; no user-source checkpoint/copy is needed.
    #[allow(deprecated)]
    pub(in crate::plots) fn observe_scaling_ime(
        &mut self,
        field: AxisField,
        events: &[egui::Event],
    ) -> bool {
        let Some(slot) = self.scaling.slots.get_mut(&field.into()) else {
            return false;
        };
        let mut key = slot.composing;
        for event in events {
            if let egui::Event::Ime(event) = event {
                match event {
                    egui::ImeEvent::Preedit { text, .. } => {
                        slot.composing = !text.is_empty();
                        key |= slot.composing;
                    }
                    egui::ImeEvent::Commit(_) => {
                        slot.composing = false;
                        key = true;
                    }
                    egui::ImeEvent::Disabled => slot.composing = false,
                    _ => (),
                }
            }
        }
        key
    }
    pub(in crate::plots) fn scaling_focus_changed(
        &mut self,
        ctx: &egui::Context,
        active: Option<AxisField>,
    ) {
        for (key, slot) in &mut self.scaling.slots {
            if active.map(Key::from) != Some(*key) {
                slot.composing = false;
                if let Some(id) = slot.widget {
                    ctx.data_mut(|data| data.remove::<String>(id.with("ime-preedit")));
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "scaling_field_tests.rs"]
mod tests;
