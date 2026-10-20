//! A borrowed TextEdit buffer keeps plot source committed until its owner
//! accepts a draft. No parsing, calculation, curve copying or history lives here.
use super::PlotSpec;
use eframe::egui::{TextBuffer, text::CharIndex};
use mathcraft_document::canvas::PlotSource;
use std::{any::TypeId, ops::Range};
#[path = "inline_edit/scaling_field.rs"]
mod scaling_field;
use scaling_field::ScalingFields;
#[path = "inline_edit/expression_field.rs"]
mod expression_field;
use expression_field::ExpressionFields;
#[path = "inline_edit/field_journal.rs"]
mod field_journal;
#[path = "inline_edit/slot_edit.rs"]
mod slot_edit;
pub(super) use scaling_field::{PlotField, PlotFieldPart, traversal_fields};
use slot_edit::SlotEdits;
pub(super) use slot_edit::{SlotEditOutcome, SlotFocus, SlotFocusEdge};

#[derive(Default)]
pub(crate) struct InlinePlotEdit {
    draft: Option<PlotSpec>,
    source_baseline: Option<PlotSource>,
    committed_fields: Vec<AxisField>,
    slots: SlotEdits,
    scaling: ScalingFields,
    expressions: ExpressionFields,
    expression_widget_generation: u64,
    fields: Vec<AxisField>,
    legacy_sources: Option<LegacySources>,
    source_initialized: bool,
    legacy_revision: u64,
    primary_ids: [Option<AxisField>; 2],
    composing: bool,
    composition_checkpoint: Option<CompositionCheckpoint>,
    #[cfg(test)]
    draft_creations: usize,
    #[cfg(test)]
    field_snapshots: usize,
}

struct CompositionCheckpoint {
    field: AxisField,
    text: String,
    cancelled: bool,
    caret: usize,
}

pub(super) struct CompositionAbort {
    pub field: AxisField,
    pub caret: usize,
    pub changed: bool,
}

impl InlinePlotEdit {
    pub(crate) fn has_changes(&self) -> bool {
        self.draft.is_some() || self.scaling.has_errors() || self.expressions.has_errors()
    }
    pub(crate) fn take(&mut self) -> Option<PlotSpec> {
        if self.validation_error().is_some() || self.is_composing() {
            return None;
        }
        if let Some(draft) = &self.draft {
            self.scaling.accept(draft);
            self.expressions.accept(draft);
        }
        self.composing = false;
        self.composition_checkpoint = None;
        if let Some(draft) = &self.draft {
            self.legacy_sources = LegacySources::capture(draft);
            self.source_initialized = true;
        }
        self.source_baseline = None;
        self.slots.clear_history();
        if let Some(draft) = &self.draft {
            self.committed_fields = fields(draft);
        }
        self.draft.take()
    }
    pub(crate) fn cancel(&mut self) {
        self.draft = None;
        self.source_baseline = None;
        self.slots.clear_history();
        self.scaling.clear();
        self.expressions.clear();
        self.composing = false;
        self.composition_checkpoint = None;
    }
    pub(crate) fn draft(&self) -> Option<&PlotSpec> {
        self.draft.as_ref()
    }
    pub(crate) fn is_composing(&self) -> bool {
        self.composing || self.scaling.is_composing() || self.expressions.is_composing()
    }

    pub(super) fn prepare_fields(&mut self, source: &PlotSpec) -> Vec<AxisField> {
        let committed_fields = fields(source);
        let same_source_kind = match (&self.legacy_sources, &source.source) {
            (
                Some(old),
                PlotSource::Legacy {
                    x_expression,
                    y_expression,
                    traces,
                    x_scaling,
                    y_scaling,
                },
            ) => {
                old.fallback_x == *x_expression
                    && old.fallback_y == *y_expression
                    && old.traces == *traces
                    && old.x_scaling == *x_scaling
                    && old.y_scaling == *y_scaling
            }
            (None, PlotSource::AxisLists(_)) => true,
            _ => false,
        };
        let stale_draft = self.validate_source(source).is_err();
        let changed_identity =
            !self.committed_fields.is_empty() && self.committed_fields != committed_fields;
        if stale_draft || changed_identity {
            self.cancel();
        }
        if self.source_initialized && !same_source_kind {
            // Legacy has no equation identities. Exact committed-source change
            // (including inactive fallback fields/styles) retires ordinal state.
            self.cancel();
            self.legacy_revision = self.legacy_revision.wrapping_add(1);
        }
        if !self.source_initialized || !same_source_kind {
            self.legacy_sources = LegacySources::capture(source);
        }
        self.source_initialized = true;
        self.committed_fields = committed_fields;
        self.expressions
            .retain_typed_sources(self.draft.as_ref().unwrap_or(source));
        let fields = fields(self.draft.as_ref().unwrap_or(source));
        for x in [false, true] {
            let primary = &mut self.primary_ids[usize::from(x)];
            if primary.is_none() {
                *primary = fields.iter().find(|f| f.x == x).copied();
            }
        }
        if self.fields != fields {
            self.scaling.retain_fields(&fields);
            self.expressions.retain_fields(&fields);
            self.fields = fields.clone();
        }
        fields
    }
    pub(super) fn field_id(&self, ui: &eframe::egui::Ui, field: AxisField) -> eframe::egui::Id {
        if field.id.is_none() && self.legacy_revision > 0 {
            return ui.make_persistent_id((
                "plot_legacy_revision",
                self.legacy_revision,
                field.x,
                field.index,
            ));
        }
        if self.primary_ids[usize::from(field.x)].is_some_and(|primary| {
            primary.x == field.x
                && match field.id {
                    Some(id) => primary.id == Some(id),
                    None => primary == field,
                }
        }) || field.id.is_none() && field.index == 0
        {
            ui.make_persistent_id(if field.x { "plot_x" } else { "plot_y" })
        } else {
            ui.make_persistent_id((
                "plot_axis_slot",
                field.x,
                field.id,
                if field.id.is_some() { 0 } else { field.index },
            ))
        }
    }
    pub(super) fn begin_ime(&mut self, source: &PlotSpec, field: AxisField, caret: Option<usize>) {
        if self.composition_checkpoint.is_none() {
            let text = field_expression(self.draft.as_ref().unwrap_or(source), field).unwrap_or("");
            self.composition_checkpoint = Some(CompositionCheckpoint {
                field,
                text: text.to_owned(),
                cancelled: false,
                caret: caret
                    .unwrap_or_else(|| text.chars().count())
                    .min(text.chars().count()),
            });
        }
    }
    pub(super) fn ime_field(&self) -> Option<AxisField> {
        self.composition_checkpoint
            .as_ref()
            .map(|saved| saved.field)
    }
    pub(super) fn ime_cancelled(&self) -> bool {
        self.composition_checkpoint
            .as_ref()
            .is_some_and(|saved| saved.cancelled)
    }
    pub(super) fn abort_ime(&mut self, source: &PlotSpec) -> Option<CompositionAbort> {
        self.composing = false;
        let saved = self.composition_checkpoint.take()?;
        let changed =
            field_expression(self.draft.as_ref().unwrap_or(source), saved.field)? != saved.text;
        self.field_buffer(source, saved.field)
            .replace_with(&saved.text);
        self.normalize(source);
        Some(CompositionAbort {
            field: saved.field,
            caret: saved.caret,
            changed,
        })
    }
    pub(super) fn finish_ime(&mut self) {
        if !self.composing {
            self.composition_checkpoint = None;
        }
    }

    // Current egui no longer emits Enabled/Disabled, but retained backend/test
    // events may still contain them. Explicit Disabled must release this guard.
    #[allow(deprecated)]
    pub(super) fn observe_ime(&mut self, events: &[eframe::egui::Event]) -> bool {
        use eframe::egui::{Event, ImeEvent};
        let mut composition_key = self.composing;
        for event in events {
            if let Event::Ime(event) = event {
                match event {
                    ImeEvent::Preedit { text, .. } => {
                        self.composing = !text.is_empty();
                        if let Some(saved) = &mut self.composition_checkpoint {
                            saved.cancelled = text.is_empty();
                        }
                        composition_key |= self.composing;
                    }
                    ImeEvent::Commit(_) => {
                        self.composing = false;
                        if let Some(saved) = &mut self.composition_checkpoint {
                            saved.cancelled = false;
                        }
                        composition_key = true;
                    }
                    ImeEvent::Disabled => {
                        self.composing = false;
                        if let Some(saved) = &mut self.composition_checkpoint {
                            saved.cancelled = true;
                        }
                    }
                    _ => (),
                }
            }
        }
        composition_key
    }

    #[cfg(test)]
    pub(super) fn buffer<'a>(&'a mut self, source: &'a PlotSpec, x: bool) -> AxisBuffer<'a> {
        self.field_buffer(source, AxisField::primary(source, x))
    }
    pub(super) fn field_buffer<'a>(
        &'a mut self,
        source: &'a PlotSpec,
        field: AxisField,
    ) -> AxisBuffer<'a> {
        AxisBuffer {
            edit: self,
            source,
            field,
        }
    }
    pub(super) fn normalize(&mut self, source: &PlotSpec) {
        if self.draft.as_ref() == Some(source) {
            self.draft = None;
            if !self.slots.has_history() {
                self.source_baseline = None;
            }
        }
    }
    pub(super) fn dimensions(&mut self, width: f32, height: f32) -> bool {
        self.draft
            .as_mut()
            .is_none_or(|draft| draft.set_outer_size(width, height).is_ok())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) struct AxisField {
    pub x: bool,
    pub index: usize,
    pub id: Option<u64>,
}
impl AxisField {
    #[cfg(test)]
    fn primary(spec: &PlotSpec, x: bool) -> Self {
        let id = spec
            .axis_lists()
            .and_then(|a| if x { a.x.first() } else { a.y.first() })
            .map(|e| e.id);
        Self { x, index: 0, id }
    }
}
struct LegacySources {
    fallback_x: String,
    fallback_y: String,
    x_scaling: Option<mathcraft_document::canvas::PlotScaling>,
    y_scaling: Option<mathcraft_document::canvas::PlotScaling>,
    traces: Vec<super::PlotTraceSpec>,
}
impl LegacySources {
    fn capture(spec: &PlotSpec) -> Option<Self> {
        let PlotSource::Legacy {
            x_expression,
            y_expression,
            traces,
            x_scaling,
            y_scaling,
        } = &spec.source
        else {
            return None;
        };
        Some(Self {
            fallback_x: x_expression.clone(),
            fallback_y: y_expression.clone(),
            traces: traces.clone(),
            x_scaling: x_scaling.clone(),
            y_scaling: y_scaling.clone(),
        })
    }
}
pub(super) fn fields(spec: &PlotSpec) -> Vec<AxisField> {
    [false, true]
        .into_iter()
        .flat_map(|x| {
            let n = spec.axis_lists().map_or_else(
                || spec.trace_count(),
                |a| if x { a.x.len() } else { a.y.len() },
            );
            (0..n).map(move |index| AxisField {
                x,
                index,
                id: spec
                    .axis_lists()
                    .map(|a| if x { a.x[index].id } else { a.y[index].id }),
            })
        })
        .collect()
}
pub(super) fn field_expression_ref(
    spec: &PlotSpec,
    field: AxisField,
) -> Option<mathcraft_document::canvas::PlotExpressionRef<'_>> {
    use mathcraft_document::canvas::PlotExpressionRef;
    if let Some(value) = expression_field::source(spec, field) {
        return Some(value.as_ref());
    }
    field_expression(spec, field).map(PlotExpressionRef::LegacyText)
}
pub(super) fn field_is_placeholder(spec: &PlotSpec, field: AxisField) -> bool {
    field_expression_ref(spec, field).is_some_and(|v| v.is_exact_placeholder())
}
pub(super) fn field_expression(spec: &PlotSpec, field: AxisField) -> Option<&str> {
    match &spec.source {
        PlotSource::AxisLists(a) => (if field.x { &a.x } else { &a.y })
            .iter()
            .find(|e| Some(e.id) == field.id)
            .and_then(|e| e.source.as_legacy_text()),
        PlotSource::Legacy {
            x_expression,
            y_expression,
            traces,
            ..
        } => {
            if traces.is_empty() {
                (field.index == 0).then_some(if field.x { x_expression } else { y_expression })
            } else {
                traces.get(field.index).map(|t| {
                    if field.x {
                        t.x_expression.as_str()
                    } else {
                        t.y_expression.as_str()
                    }
                })
            }
        }
    }
}
fn field_expression_mut(spec: &mut PlotSpec, field: AxisField) -> Option<&mut String> {
    match &mut spec.source {
        PlotSource::AxisLists(a) => (if field.x { &mut a.x } else { &mut a.y })
            .iter_mut()
            .find(|e| Some(e.id) == field.id)
            .and_then(|e| match &mut e.source {
                mathcraft_document::canvas::PlotExpression::LegacyText(text) => Some(text),
                _ => None,
            }),
        PlotSource::Legacy {
            x_expression,
            y_expression,
            traces,
            ..
        } => {
            if traces.is_empty() {
                (field.index == 0).then_some(if field.x { x_expression } else { y_expression })
            } else {
                traces.get_mut(field.index).map(|t| {
                    if field.x {
                        &mut t.x_expression
                    } else {
                        &mut t.y_expression
                    }
                })
            }
        }
    }
}
pub(super) struct AxisBuffer<'a> {
    edit: &'a mut InlinePlotEdit,
    source: &'a PlotSpec,
    field: AxisField,
}
#[cfg(test)]
fn expression(spec: &PlotSpec, x: bool) -> &str {
    if x {
        spec.primary_x_expression().as_legacy_text().unwrap()
    } else {
        spec.primary_y_expression().as_legacy_text().unwrap()
    }
}
impl AxisBuffer<'_> {
    fn mutable(&mut self) -> &mut String {
        self.edit.ensure_source_baseline(self.source);
        self.edit.slots.clear_redo();
        if self.edit.draft.is_none() {
            self.edit.draft = Some(self.source.clone());
            #[cfg(test)]
            {
                self.edit.draft_creations += 1;
            }
        }
        field_expression_mut(self.edit.draft.as_mut().unwrap(), self.field)
            .expect("validated plot source slot")
    }
}
impl TextBuffer for AxisBuffer<'_> {
    fn is_mutable(&self) -> bool {
        true
    }
    fn as_str(&self) -> &str {
        field_expression(self.edit.draft.as_ref().unwrap_or(self.source), self.field).unwrap_or("")
    }
    fn insert_text(&mut self, text: &str, index: CharIndex) -> usize {
        if text.is_empty() {
            return 0;
        }
        TextBuffer::insert_text(self.mutable(), text, index)
    }
    fn delete_char_range(&mut self, range: Range<CharIndex>) {
        if range.start >= range.end || range.start.0 >= self.as_str().chars().count() {
            return;
        }
        TextBuffer::delete_char_range(self.mutable(), range);
    }
    fn replace_with(&mut self, text: &str) {
        if self.as_str() != text {
            TextBuffer::replace_with(self.mutable(), text);
        }
    }
    fn type_id(&self) -> TypeId {
        TypeId::of::<AxisBuffer<'static>>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn borrowed_idle_and_noop_mutations_never_create_a_draft() {
        let spec = PlotSpec::default();
        let mut edit = InlinePlotEdit::default();
        for _ in 0..20 {
            let mut buffer = edit.buffer(&spec, true);
            assert_eq!(
                buffer.as_str().as_ptr(),
                spec.primary_x_expression()
                    .as_legacy_text()
                    .unwrap()
                    .as_ptr()
            );
            assert_eq!(buffer.insert_text("", CharIndex(0)), 0);
            buffer.delete_char_range(CharIndex(0)..CharIndex(0));
            buffer.delete_char_range(CharIndex(8)..CharIndex(10));
            buffer.replace_with("x");
        }
        assert!(!edit.has_changes());
        assert_eq!(edit.draft_creations, 0);
    }
    #[test]
    fn unicode_edit_clones_once_and_keeps_committed_metadata_and_other_traces_inert() {
        let mut spec = PlotSpec::default();
        spec.promote_legacy_trace();
        *spec.primary_x_expression_mut().unwrap() = "α𝛼z".into();
        spec.legacy_traces_mut()
            .unwrap()
            .push(super::super::PlotTraceSpec {
                y_expression: "cos(t)".into(),
                ..Default::default()
            });
        let original = spec.clone();
        let mut edit = InlinePlotEdit::default();
        {
            let mut buffer = edit.buffer(&spec, true);
            buffer.delete_char_range(CharIndex(1)..CharIndex(2));
            assert_eq!(buffer.as_str(), "αz");
            assert_eq!(buffer.insert_text("한𝛽", CharIndex(1)), 2);
            assert_eq!(buffer.as_str(), "α한𝛽z");
        }
        edit.buffer(&spec, false).replace_with("sin(");
        assert_eq!(edit.draft_creations, 1);
        assert_eq!(spec, original);
        assert_eq!(
            edit.draft().unwrap().resolved_traces().unwrap()[1],
            original.resolved_traces().unwrap()[1]
        );
        let candidate = edit.take().unwrap();
        assert_eq!(
            candidate.primary_x_expression().as_legacy_text().unwrap(),
            "α한𝛽z"
        );
        assert_eq!(
            candidate.primary_y_expression().as_legacy_text().unwrap(),
            "sin("
        );
        assert!(!edit.has_changes());
    }
    #[test]
    fn returning_to_source_or_cancel_drops_draft_and_resize_keeps_latest_dimensions() {
        let mut spec = PlotSpec::default();
        let mut edit = InlinePlotEdit::default();
        edit.buffer(&spec, true).replace_with("t");
        edit.buffer(&spec, true).replace_with("x");
        edit.normalize(&spec);
        assert!(!edit.has_changes());
        edit.buffer(&spec, false).replace_with("cos(x)");
        spec.set_outer_size(500., 320.).unwrap();
        assert!(edit.dimensions(spec.width, spec.height));
        let pending = edit.draft().unwrap();
        assert_eq!((pending.width, pending.height), (500., 320.));
        assert_eq!(
            pending.primary_y_expression().as_legacy_text().unwrap(),
            "cos(x)"
        );
        assert_eq!(
            spec.primary_y_expression().as_legacy_text().unwrap(),
            "sin(x)"
        );
        edit.cancel();
        assert!(!edit.has_changes());
        assert!(edit.take().is_none());
    }

    fn frame(
        ctx: &eframe::egui::Context,
        spec: &mut PlotSpec,
        edit: &mut InlinePlotEdit,
        focus: bool,
        events: Vec<eframe::egui::Event>,
    ) -> super::super::DrawResponse {
        use eframe::egui;
        let mut response = None;
        let output = ctx.run_ui(
            egui::RawInput {
                events,
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1000., 800.),
                )),
                ..Default::default()
            },
            |ui| {
                response = Some(super::super::draw(
                    ui,
                    spec,
                    edit,
                    &Err("Retained curve".into()),
                    focus,
                ));
            },
        );
        output.drop_without_applying_deltas();
        response.unwrap()
    }
    fn key(key: eframe::egui::Key, modifiers: eframe::egui::Modifiers) -> eframe::egui::Event {
        eframe::egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }
    }
    #[test]
    fn actual_text_and_plain_enter_stage_source_and_keep_focus_without_implicit_commit() {
        use eframe::egui;
        let ctx = egui::Context::default();
        crate::configure(&ctx);
        let mut source = PlotSpec::default();
        let original = source.clone();
        let mut edit = InlinePlotEdit::default();
        frame(&ctx, &mut source, &mut edit, true, vec![]);
        frame(&ctx, &mut source, &mut edit, false, vec![]);
        let typed = frame(
            &ctx,
            &mut source,
            &mut edit,
            false,
            vec![egui::Event::Text("+2".into())],
        );
        assert!(typed.expression_changed);
        assert!(!typed.commit_requested);
        assert_eq!(source, original);
        let draft = edit.draft().unwrap().clone();
        assert_ne!(
            draft.primary_y_expression().as_legacy_text().unwrap(),
            original.primary_y_expression().as_legacy_text().unwrap()
        );
        let enter = frame(
            &ctx,
            &mut source,
            &mut edit,
            false,
            vec![key(egui::Key::Enter, egui::Modifiers::NONE)],
        );
        assert!(enter.commit_requested);
        assert!(!enter.expression_changed);
        assert!(ctx.memory(|memory| memory.has_focus(enter.response.id)));
        assert_eq!(edit.draft(), Some(&draft));
        assert_eq!(source, original);
        assert_eq!(edit.draft_creations, 1);
    }

    #[test]
    fn shared_primary_axis_edit_stays_draft_then_updates_each_resolved_trace() {
        use eframe::egui;
        use mathcraft_document::canvas::{PlotSource, PlotTraceAppearance};

        for x in [true, false] {
            let source_axes = if x {
                (vec!["x".into()], vec!["2".into(), "3".into()])
            } else {
                (vec!["1".into(), "2".into()], vec!["5".into()])
            };
            let mut source = PlotSpec {
                source: PlotSource::axis_lists(
                    source_axes.0,
                    source_axes.1,
                    vec![PlotTraceAppearance::default(); 2],
                )
                .unwrap(),
                ..Default::default()
            };
            let original = source.clone();
            let ctx = egui::Context::default();
            crate::configure(&ctx);
            let mut edit = InlinePlotEdit::default();
            frame(&ctx, &mut source, &mut edit, true, vec![]);
            let mut focused = frame(&ctx, &mut source, &mut edit, false, vec![]);
            for _ in 0..if x {
                2 * source.axis_lists().unwrap().y.len()
            } else {
                0
            } {
                focused = frame(
                    &ctx,
                    &mut source,
                    &mut edit,
                    false,
                    vec![key(egui::Key::Tab, egui::Modifiers::NONE)],
                );
            }
            let mut state =
                egui::text_edit::TextEditState::load(&ctx, focused.response.id).unwrap();
            state
                .cursor
                .set_char_range(Some(egui::text::CCursorRange::two(
                    egui::text::CCursor::new(0),
                    egui::text::CCursor::new(expression(&source, x).chars().count()),
                )));
            state.store(&ctx, focused.response.id);
            let replacement = if x { "t" } else { "7" };
            let typed = frame(
                &ctx,
                &mut source,
                &mut edit,
                false,
                vec![egui::Event::Text(replacement.into())],
            );
            assert!(typed.expression_changed);
            assert!(!typed.commit_requested);
            assert_eq!(source, original);
            assert_eq!(edit.draft_creations, 1);
            for trace in edit.draft().unwrap().resolved_traces().unwrap() {
                assert_eq!(
                    if x {
                        trace.x_expression
                    } else {
                        trace.y_expression
                    },
                    replacement
                );
            }
            let enter = frame(
                &ctx,
                &mut source,
                &mut edit,
                false,
                vec![key(egui::Key::Enter, egui::Modifiers::NONE)],
            );
            assert!(enter.commit_requested);
            assert_eq!(source, original);
            let committed = edit.take().unwrap();
            let lists = committed.axis_lists().unwrap();
            let original_lists = original.axis_lists().unwrap();
            assert_eq!(lists.next_equation_id, original_lists.next_equation_id);
            assert_eq!(lists.appearances, original_lists.appearances);
            assert_eq!(
                lists
                    .x
                    .iter()
                    .chain(&lists.y)
                    .map(|e| e.id)
                    .collect::<Vec<_>>(),
                original_lists
                    .x
                    .iter()
                    .chain(&original_lists.y)
                    .map(|e| e.id)
                    .collect::<Vec<_>>()
            );
            let data = super::super::evaluate(&committed, &[], 999, 0., 100.).unwrap();
            assert_eq!(data.traces.len(), 2);
            if x {
                assert_eq!(
                    data.traces[0].curve().unwrap().points.first(),
                    Some(&[-10., 2.])
                );
                assert_eq!(
                    data.traces[1].curve().unwrap().points.last(),
                    Some(&[10., 3.])
                );
                assert!(
                    data.traces
                        .iter()
                        .all(|trace| trace.curve().unwrap().points.len() == 500)
                );
            } else {
                assert_eq!(data.traces[0].curve().unwrap().points, vec![[1., 7.]]);
                assert_eq!(data.traces[1].curve().unwrap().points, vec![[2., 7.]]);
            }
        }
    }
    #[test]
    fn modified_enter_does_not_request_commit_and_escape_cancels_only_the_draft() {
        use eframe::egui;
        let ctx = egui::Context::default();
        crate::configure(&ctx);
        let mut source = PlotSpec::default();
        let original = source.clone();
        let mut edit = InlinePlotEdit::default();
        frame(&ctx, &mut source, &mut edit, true, vec![]);
        frame(&ctx, &mut source, &mut edit, false, vec![]);
        frame(
            &ctx,
            &mut source,
            &mut edit,
            false,
            vec![egui::Event::Text("+2".into())],
        );
        let response = frame(
            &ctx,
            &mut source,
            &mut edit,
            false,
            vec![key(egui::Key::Enter, egui::Modifiers::SHIFT)],
        );
        assert!(!response.commit_requested);
        // Re-establish field focus if the underlying single-line widget elected
        // to release it; the plot handler never overrides modified Enter.
        frame(&ctx, &mut source, &mut edit, true, vec![]);
        frame(&ctx, &mut source, &mut edit, false, vec![]);
        let escaped = frame(
            &ctx,
            &mut source,
            &mut edit,
            false,
            vec![key(egui::Key::Escape, egui::Modifiers::NONE)],
        );
        assert!(!escaped.commit_requested);
        assert!(escaped.expression_changed);
        assert!(!edit.has_changes());
        assert_eq!(source, original);
    }

    #[test]
    fn escape_after_late_focus_transfer_does_not_cancel_another_plot() {
        use eframe::egui;
        let ctx = egui::Context::default();
        crate::configure(&ctx);
        let mut first = PlotSpec::default();
        let mut second = PlotSpec::default();
        let mut first_edit = InlinePlotEdit::default();
        let mut second_edit = InlinePlotEdit::default();
        first_edit.buffer(&first, false).replace_with("first_draft");
        second_edit
            .buffer(&second, false)
            .replace_with("second_draft");
        let mut render = |focus_first, focus_second, events| {
            let output = ctx.run_ui(
                egui::RawInput {
                    events,
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1000., 1000.),
                    )),
                    ..Default::default()
                },
                |ui| {
                    ui.push_id("first", |ui| {
                        super::super::draw(
                            ui,
                            &mut first,
                            &mut first_edit,
                            &Err("saved".into()),
                            focus_first,
                        );
                    });
                    ui.push_id("second", |ui| {
                        super::super::draw(
                            ui,
                            &mut second,
                            &mut second_edit,
                            &Err("saved".into()),
                            focus_second,
                        );
                    });
                },
            );
            output.drop_without_applying_deltas();
        };
        render(true, false, vec![]);
        // The first field has already rendered when the later plot receives
        // focus. Its local last-draw boolean would be stale on the next frame.
        render(false, true, vec![]);
        render(
            false,
            false,
            vec![key(egui::Key::Escape, egui::Modifiers::NONE)],
        );
        assert!(first_edit.has_changes());
        assert!(!second_edit.has_changes());
        assert_eq!(
            first_edit
                .draft()
                .unwrap()
                .primary_y_expression()
                .as_legacy_text()
                .unwrap(),
            "first_draft"
        );
    }

    #[test]
    fn inward_axis_tab_moves_focus_in_the_same_frame_and_keeps_the_aggregate_draft() {
        use eframe::egui;
        let ctx = egui::Context::default();
        crate::configure(&ctx);
        let mut source = PlotSpec::default();
        let original = source.clone();
        let mut edit = InlinePlotEdit::default();
        frame(&ctx, &mut source, &mut edit, true, vec![]);
        frame(&ctx, &mut source, &mut edit, false, vec![]);
        let y = frame(
            &ctx,
            &mut source,
            &mut edit,
            false,
            vec![egui::Event::Text("+2".into())],
        )
        .response
        .id;
        let forward = frame(
            &ctx,
            &mut source,
            &mut edit,
            false,
            vec![key(egui::Key::Tab, egui::Modifiers::NONE)],
        );
        assert_eq!(
            Some(forward.response.id),
            edit.scaling_widget_id(fields(&source)[0])
        );
        assert_ne!(forward.response.id, y);
        assert!(!forward.commit_requested);
        assert!(!forward.expression_changed);
        assert!(ctx.memory(|memory| memory.has_focus(forward.response.id)));
        assert!(edit.has_changes());
        let reverse = frame(
            &ctx,
            &mut source,
            &mut edit,
            false,
            vec![key(egui::Key::Tab, egui::Modifiers::SHIFT)],
        );
        assert_eq!(reverse.response.id, y);
        assert!(!reverse.commit_requested);
        assert!(!reverse.expression_changed);
        assert!(ctx.memory(|memory| memory.has_focus(y)));
        assert!(edit.has_changes());
        assert_eq!(source, original);
    }

    #[test]
    #[allow(deprecated)] // Exercise the explicit legacy backend Enabled event.
    fn actual_ime_preedit_enter_is_not_owner_commit_and_final_commit_precedes_later_enter() {
        use eframe::egui;
        let ctx = egui::Context::default();
        crate::configure(&ctx);
        let mut source = PlotSpec::default();
        let original = source.clone();
        let mut edit = InlinePlotEdit::default();
        frame(&ctx, &mut source, &mut edit, true, vec![]);
        frame(&ctx, &mut source, &mut edit, false, vec![]);
        frame(
            &ctx,
            &mut source,
            &mut edit,
            false,
            vec![egui::Event::Ime(egui::ImeEvent::Enabled)],
        );
        assert!(!edit.has_changes());
        frame(
            &ctx,
            &mut source,
            &mut edit,
            false,
            vec![egui::Event::Ime(egui::ImeEvent::Preedit {
                text: "한".into(),
                active_range_chars: None,
            })],
        );
        assert!(edit.is_composing());
        assert!(edit.has_changes());
        assert_eq!(source, original);
        let confirm = frame(
            &ctx,
            &mut source,
            &mut edit,
            false,
            vec![key(egui::Key::Enter, egui::Modifiers::NONE)],
        );
        assert!(!confirm.commit_requested);
        assert!(edit.is_composing());
        assert!(ctx.memory(|memory| memory.has_focus(confirm.response.id)));
        let committed = frame(
            &ctx,
            &mut source,
            &mut edit,
            false,
            vec![
                egui::Event::Ime(egui::ImeEvent::Commit("한글".into())),
                key(egui::Key::Enter, egui::Modifiers::NONE),
            ],
        );
        assert!(!committed.commit_requested);
        assert!(!edit.is_composing());
        assert!(
            edit.draft()
                .unwrap()
                .primary_y_expression()
                .as_legacy_text()
                .unwrap()
                .contains("한글")
        );
        assert_eq!(source, original);
        let enter = frame(
            &ctx,
            &mut source,
            &mut edit,
            false,
            vec![key(egui::Key::Enter, egui::Modifiers::NONE)],
        );
        assert!(enter.commit_requested);
        assert_eq!(source, original);
        assert_eq!(edit.draft_creations, 1);
        edit.observe_ime(&[egui::Event::Ime(egui::ImeEvent::Preedit {
            text: "조합".into(),
            active_range_chars: None,
        })]);
        edit.cancel();
        assert!(!edit.is_composing());
        assert!(!edit.has_changes());
        edit.observe_ime(&[egui::Event::Ime(egui::ImeEvent::Preedit {
            text: "조합".into(),
            active_range_chars: None,
        })]);
        assert!(edit.is_composing());
        edit.observe_ime(&[egui::Event::Ime(egui::ImeEvent::Disabled)]);
        assert!(!edit.is_composing());
    }

    #[test]
    #[allow(deprecated)] // Include the explicit legacy backend cancellation.
    fn late_focus_transfer_and_cancellation_restore_only_unconfirmed_axis_composition() {
        use eframe::egui;
        for cancellation in [
            egui::ImeEvent::Disabled,
            egui::ImeEvent::Preedit {
                text: String::new(),
                active_range_chars: None,
            },
        ] {
            let ctx = egui::Context::default();
            crate::configure(&ctx);
            let mut first = PlotSpec::default();
            let first_source = first.clone();
            let mut second = PlotSpec::default();
            let mut a = InlinePlotEdit::default();
            let mut b = InlinePlotEdit::default();
            a.buffer(&first, true).replace_with("confirmed_x");
            let mut render = |focus_first, focus_second, events| {
                let output = ctx.run_ui(
                    egui::RawInput {
                        events,
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(1000., 1000.),
                        )),
                        ..Default::default()
                    },
                    |ui| {
                        ui.push_id("first", |ui| {
                            super::super::draw(
                                ui,
                                &mut first,
                                &mut a,
                                &Err("saved".into()),
                                focus_first,
                            );
                        });
                        ui.push_id("second", |ui| {
                            super::super::draw(
                                ui,
                                &mut second,
                                &mut b,
                                &Err("saved".into()),
                                focus_second,
                            );
                        });
                    },
                );
                output.drop_without_applying_deltas();
                (
                    a.is_composing(),
                    a.draft().map(|spec| expression(spec, false).to_owned()),
                )
            };
            render(true, false, vec![]);
            let confirmed = render(false, false, vec![egui::Event::Text("+2".into())])
                .1
                .unwrap();
            assert!(
                render(
                    false,
                    false,
                    vec![egui::Event::Ime(egui::ImeEvent::Preedit {
                        text: "미확정".into(),
                        active_range_chars: None
                    })]
                )
                .0
            );
            // Another control takes focus after the first has already drawn.
            render(false, true, vec![]);
            let restored = render(false, false, vec![egui::Event::Ime(cancellation)]);
            assert!(!restored.0);
            assert_eq!(restored.1.as_deref(), Some(confirmed.as_str()));
            render(false, false, vec![egui::Event::Text("new_owner".into())]);
            assert_eq!(expression(a.draft().unwrap(), true), "confirmed_x");
            assert_eq!(expression(a.draft().unwrap(), false), confirmed);
            assert!(expression(b.draft().unwrap(), false).contains("new_owner"));
            assert!(!a.is_composing());
            assert_eq!(first, first_source);
            assert_eq!(a.draft_creations, 1);
        }
    }

    #[test]
    #[allow(deprecated)] // An explicit backend cancellation must not delete text.
    fn cancelling_middle_preedit_restores_confirmed_tail_and_a_safe_caret() {
        use eframe::egui;
        let ctx = egui::Context::default();
        crate::configure(&ctx);
        let mut source = PlotSpec {
            source: mathcraft_document::canvas::PlotSource::single("x", "abcd+1"),
            ..Default::default()
        };
        let mut edit = InlinePlotEdit::default();
        frame(&ctx, &mut source, &mut edit, true, vec![]);
        let focused = frame(&ctx, &mut source, &mut edit, false, vec![])
            .response
            .id;
        let mut state = egui::text_edit::TextEditState::load(&ctx, focused).unwrap();
        state
            .cursor
            .set_char_range(Some(egui::text::CCursorRange::one(
                egui::text::CCursor::new(2),
            )));
        state.store(&ctx, focused);
        frame(
            &ctx,
            &mut source,
            &mut edit,
            false,
            vec![egui::Event::Ime(egui::ImeEvent::Preedit {
                text: "미확정".into(),
                active_range_chars: None,
            })],
        );
        assert!(edit.is_composing());
        frame(
            &ctx,
            &mut source,
            &mut edit,
            false,
            vec![egui::Event::Ime(egui::ImeEvent::Disabled)],
        );
        assert!(!edit.is_composing());
        assert!(!edit.has_changes());
        frame(
            &ctx,
            &mut source,
            &mut edit,
            false,
            vec![egui::Event::Ime(egui::ImeEvent::Preedit {
                text: String::new(),
                active_range_chars: None,
            })],
        );
        frame(
            &ctx,
            &mut source,
            &mut edit,
            false,
            vec![egui::Event::Text("Z".into())],
        );
        assert_eq!(
            edit.draft()
                .unwrap()
                .primary_y_expression()
                .as_legacy_text()
                .unwrap(),
            "abZcd+1"
        );
        assert_eq!(
            source.primary_y_expression().as_legacy_text().unwrap(),
            "abcd+1"
        );
    }
    fn select_all(ctx: &eframe::egui::Context, id: eframe::egui::Id, len: usize) {
        use eframe::egui;
        let mut state = egui::text_edit::TextEditState::load(ctx, id).unwrap();
        state
            .cursor
            .set_char_range(Some(egui::text::CCursorRange::two(
                egui::text::CCursor::new(0),
                egui::text::CCursor::new(len),
            )));
        state.store(ctx, id);
    }
    fn many_source(legacy: bool, n: usize) -> PlotSpec {
        use mathcraft_document::canvas::{PlotSource, PlotTraceAppearance, PlotTraceSpec};
        let x: Vec<_> = (0..n).map(|i| (i + 1).to_string()).collect();
        let y: Vec<_> = (0..n).map(|i| (i + 20).to_string()).collect();
        PlotSpec {
            source: if legacy {
                PlotSource::legacy(
                    "unused_x",
                    "unused_y",
                    x.into_iter()
                        .zip(y)
                        .map(|(x_expression, y_expression)| PlotTraceSpec {
                            x_expression,
                            y_expression,
                            ..Default::default()
                        })
                        .collect(),
                )
            } else {
                PlotSource::axis_lists(x, y, vec![PlotTraceAppearance::default(); n]).unwrap()
            },
            ..Default::default()
        }
    }
    #[test]
    fn every_axis_and_legacy_pair_has_independent_text_and_ordered_internal_tabs() {
        use eframe::egui;
        for legacy in [false, true] {
            let ctx = egui::Context::default();
            crate::configure(&ctx);
            let mut source = many_source(legacy, 3);
            let original = source.clone();
            let mut edit = InlinePlotEdit::default();
            frame(&ctx, &mut source, &mut edit, true, vec![]);
            let mut current = frame(&ctx, &mut source, &mut edit, false, vec![]);
            let authored = fields(&source);
            let mut visited = Vec::new();
            for (index, field) in authored.iter().copied().enumerate() {
                assert!(ctx.memory(|m| m.has_focus(current.response.id)));
                assert!(!visited.contains(&current.response.id));
                visited.push(current.response.id);
                select_all(
                    &ctx,
                    current.response.id,
                    field_expression(&source, field).unwrap().chars().count(),
                );
                let text = (100 + index).to_string();
                current = frame(
                    &ctx,
                    &mut source,
                    &mut edit,
                    false,
                    vec![egui::Event::Text(text.clone())],
                );
                assert!(current.expression_changed);
                assert!(!current.commit_requested);
                assert_eq!(source, original);
                assert_eq!(
                    field_expression(edit.draft().unwrap(), field),
                    Some(text.as_str())
                );
                for untouched in authored.iter().skip(index + 1) {
                    assert_eq!(
                        field_expression(edit.draft().unwrap(), *untouched),
                        field_expression(&original, *untouched)
                    );
                }
                if index + 1 < authored.len() {
                    for _ in 0..2 {
                        current = frame(
                            &ctx,
                            &mut source,
                            &mut edit,
                            false,
                            vec![key(egui::Key::Tab, egui::Modifiers::NONE)],
                        );
                    }
                    assert!(!current.commit_requested);
                }
            }
            for expected in visited.iter().rev().skip(1) {
                for _ in 0..2 {
                    current = frame(
                        &ctx,
                        &mut source,
                        &mut edit,
                        false,
                        vec![key(egui::Key::Tab, egui::Modifiers::SHIFT)],
                    );
                }
                assert_eq!(current.response.id, *expected);
                assert!(!current.commit_requested);
            }
            let enter = frame(
                &ctx,
                &mut source,
                &mut edit,
                false,
                vec![key(egui::Key::Enter, egui::Modifiers::NONE)],
            );
            assert!(enter.commit_requested);
            assert_eq!(source, original);
            assert_eq!(edit.draft_creations, 1);
            let candidate = edit.take().unwrap();
            let traces = candidate.resolved_traces().unwrap();
            assert_eq!(
                traces
                    .iter()
                    .map(|t| t.y_expression.as_str())
                    .collect::<Vec<_>>(),
                vec!["100", "101", "102"]
            );
            assert_eq!(
                traces
                    .iter()
                    .map(|t| t.x_expression.as_str())
                    .collect::<Vec<_>>(),
                vec!["103", "104", "105"]
            );
            let calculated = super::super::evaluate(&candidate, &[], 999, 0., 100.).unwrap();
            for (i, trace) in calculated.traces.iter().enumerate() {
                assert_eq!(
                    &*trace.curve().unwrap().points,
                    &[[(103 + i) as f64, (100 + i) as f64]]
                );
            }
        }
    }
    #[test]
    fn reordered_axis_identity_keeps_its_caret_but_never_installs_a_stale_draft() {
        use eframe::egui;
        let ctx = egui::Context::default();
        crate::configure(&ctx);
        let mut source = many_source(false, 3);
        let mut edit = InlinePlotEdit::default();
        frame(&ctx, &mut source, &mut edit, true, vec![]);
        let first = frame(&ctx, &mut source, &mut edit, false, vec![]);
        let first_id = source.axis_lists().unwrap().y[0].id;
        select_all(&ctx, first.response.id, 2);
        frame(
            &ctx,
            &mut source,
            &mut edit,
            false,
            vec![egui::Event::Text("α𝛼".into())],
        );
        assert!(edit.has_changes());
        source.axis_lists_mut().unwrap().y.swap(0, 2);
        let reordered = source.clone();
        let current = frame(&ctx, &mut source, &mut edit, false, vec![]);
        assert_eq!(current.response.id, first.response.id);
        assert!(!edit.has_changes());
        select_all(&ctx, current.response.id, 2);
        frame(
            &ctx,
            &mut source,
            &mut edit,
            false,
            vec![egui::Event::Text("77".into())],
        );
        let pending = edit.draft().unwrap().axis_lists().unwrap();
        assert_eq!(
            pending
                .y
                .iter()
                .find(|e| e.id == first_id)
                .unwrap()
                .source
                .as_legacy_text()
                .unwrap(),
            "77"
        );
        assert_eq!(pending.y[0].source.as_legacy_text().unwrap(), "22");
        assert_eq!(source, reordered);
    }
    #[test]
    #[allow(deprecated)]
    fn secondary_slot_ime_retains_confirmed_edits_and_aborts_when_its_identity_is_removed() {
        use eframe::egui;
        let ctx = egui::Context::default();
        crate::configure(&ctx);
        let mut source = many_source(false, 3);
        let original = source.clone();
        let mut edit = InlinePlotEdit::default();
        frame(&ctx, &mut source, &mut edit, true, vec![]);
        frame(&ctx, &mut source, &mut edit, false, vec![]);
        frame(
            &ctx,
            &mut source,
            &mut edit,
            false,
            vec![key(egui::Key::Tab, egui::Modifiers::NONE)],
        );
        let second = frame(
            &ctx,
            &mut source,
            &mut edit,
            false,
            vec![key(egui::Key::Tab, egui::Modifiers::NONE)],
        );
        select_all(&ctx, second.response.id, 2);
        frame(
            &ctx,
            &mut source,
            &mut edit,
            false,
            vec![egui::Event::Text("confirmed".into())],
        );
        frame(
            &ctx,
            &mut source,
            &mut edit,
            false,
            vec![
                egui::Event::Ime(egui::ImeEvent::Enabled),
                egui::Event::Ime(egui::ImeEvent::Preedit {
                    text: "한𝛼".into(),
                    active_range_chars: None,
                }),
            ],
        );
        assert!(edit.is_composing());
        assert_eq!(
            edit.ime_field().unwrap().id,
            Some(source.axis_lists().unwrap().y[1].id)
        );
        let enter = frame(
            &ctx,
            &mut source,
            &mut edit,
            false,
            vec![key(egui::Key::Enter, egui::Modifiers::NONE)],
        );
        assert!(!enter.commit_requested);
        assert_eq!(source, original);
        frame(
            &ctx,
            &mut source,
            &mut edit,
            false,
            vec![egui::Event::Ime(egui::ImeEvent::Commit("완료".into()))],
        );
        assert!(!edit.is_composing());
        assert!(
            edit.draft().unwrap().axis_lists().unwrap().y[1]
                .source
                .as_legacy_text()
                .unwrap()
                .contains("완료")
        );
        let enter = frame(
            &ctx,
            &mut source,
            &mut edit,
            false,
            vec![key(egui::Key::Enter, egui::Modifiers::NONE)],
        );
        assert!(enter.commit_requested);
        frame(
            &ctx,
            &mut source,
            &mut edit,
            false,
            vec![egui::Event::Ime(egui::ImeEvent::Preedit {
                text: "discard".into(),
                active_range_chars: None,
            })],
        );
        assert!(edit.is_composing());
        let lists = source.axis_lists_mut().unwrap();
        let removed = lists.y[1].id;
        lists.y.remove(1);
        frame(&ctx, &mut source, &mut edit, false, vec![]);
        assert!(!edit.is_composing());
        assert!(!edit.has_changes());
        assert!(
            !source
                .axis_lists()
                .unwrap()
                .y
                .iter()
                .any(|e| e.id == removed)
        );
        frame(
            &ctx,
            &mut source,
            &mut edit,
            false,
            vec![egui::Event::Ime(egui::ImeEvent::Disabled)],
        );
        assert!(!edit.has_changes());
    }
    #[test]
    fn all_sixteen_slots_idle_outside_outer_height_stay_source_and_curve_inert() {
        use eframe::egui;
        let ctx = egui::Context::default();
        crate::configure(&ctx);
        let mut source = many_source(false, 16);
        let original = source.clone();
        let data = super::super::evaluate(&source, &[], 999, 0., 100.).unwrap();
        let before = data.to_saved();
        let mut edit = InlinePlotEdit::default();
        for _ in 0..3 {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1200., 2400.),
                    )),
                    ..Default::default()
                },
                |ui| {
                    let response =
                        super::super::draw(ui, &mut source, &mut edit, &Ok(data.clone()), false);
                    assert!(response.response.rect.height() > source.height);
                    assert!(!response.expression_changed);
                    assert!(!response.commit_requested);
                    assert!(!response.resize_started);
                },
            );
            output.textures_delta.clear();
            for primitive in ctx.tessellate(output.shapes, output.pixels_per_point) {
                if let egui::epaint::Primitive::Mesh(mesh) = primitive.primitive {
                    assert!(
                        mesh.vertices
                            .iter()
                            .all(|v| v.pos.x.is_finite() && v.pos.y.is_finite())
                    );
                }
            }
        }
        assert_eq!(source, original);
        assert!(!edit.has_changes());
        assert_eq!(edit.draft_creations, 0);
        assert_eq!(data.to_saved(), before);
    }
    #[test]
    fn external_legacy_replacement_and_model_switch_retire_old_ordinal_ime_state() {
        use eframe::egui;
        let ctx = egui::Context::default();
        crate::configure(&ctx);
        let mut source = many_source(true, 2);
        let mut edit = InlinePlotEdit::default();
        frame(&ctx, &mut source, &mut edit, true, vec![]);
        frame(&ctx, &mut source, &mut edit, false, vec![]);
        frame(
            &ctx,
            &mut source,
            &mut edit,
            false,
            vec![egui::Event::Ime(egui::ImeEvent::Preedit {
                text: "stale".into(),
                active_range_chars: None,
            })],
        );
        assert!(edit.is_composing());
        source.legacy_traces_mut().unwrap().swap(0, 1);
        let replacement = source.clone();
        frame(&ctx, &mut source, &mut edit, false, vec![]);
        assert!(!edit.has_changes());
        assert!(!edit.is_composing());
        frame(
            &ctx,
            &mut source,
            &mut edit,
            false,
            vec![egui::Event::Ime(egui::ImeEvent::Commit(
                "wrong_owner".into(),
            ))],
        );
        assert_eq!(source, replacement);
        assert!(!edit.has_changes());
        let previous_generation = edit.legacy_revision;
        source = many_source(false, 2);
        frame(&ctx, &mut source, &mut edit, false, vec![]);
        assert!(edit.legacy_revision > previous_generation);
        let previous_generation = edit.legacy_revision;
        source = many_source(true, 2);
        frame(&ctx, &mut source, &mut edit, false, vec![]);
        assert!(edit.legacy_revision > previous_generation);
        assert!(!edit.has_changes());
    }
    #[test]
    fn outbound_tabs_use_surrounding_widgets_instead_of_wrapping_the_plot() {
        use eframe::egui;
        let ctx = egui::Context::default();
        crate::configure(&ctx);
        let mut source = many_source(false, 2);
        let mut edit = InlinePlotEdit::default();
        let mut outside_before = String::from("before");
        let mut outside_after = String::from("after");
        let mut render = |focus: bool, events: Vec<egui::Event>| {
            let mut ids = None;
            let output = ctx.run_ui(
                egui::RawInput {
                    events,
                    ..Default::default()
                },
                |ui| {
                    let before = ui.text_edit_singleline(&mut outside_before).id;
                    let current = super::super::draw(
                        ui,
                        &mut source,
                        &mut edit,
                        &Err("Retained".into()),
                        focus,
                    );
                    let after = ui.text_edit_singleline(&mut outside_after).id;
                    ids = Some((before, current.response.id, after));
                },
            );
            output.drop_without_applying_deltas();
            ids.unwrap()
        };
        render(true, vec![]);
        let first = render(false, vec![]);
        render(false, vec![key(egui::Key::Tab, egui::Modifiers::SHIFT)]);
        // egui's outbound reverse traversal applies id_next_frame.
        render(false, vec![]);
        assert!(
            ctx.memory(|m| m.has_focus(first.0)),
            "focus={:?} expected={:?}",
            ctx.memory(|m| m.focused()),
            first.0
        );
        ctx.memory_mut(|m| m.request_focus(first.1));
        render(false, vec![]);
        for _ in 0..7 {
            render(false, vec![key(egui::Key::Tab, egui::Modifiers::NONE)]);
        }
        let last = render(false, vec![]);
        assert_ne!(last.1, first.1);
        render(false, vec![key(egui::Key::Tab, egui::Modifiers::NONE)]);
        // The existing resize handle is a normal focusable egui control after
        // the final X row. It is not an axis-list wrap or a source edit.
        if !ctx.memory(|m| m.has_focus(first.2)) {
            assert_ne!(ctx.memory(|m| m.focused()), Some(first.1));
            assert_ne!(ctx.memory(|m| m.focused()), Some(last.1));
            render(false, vec![key(egui::Key::Tab, egui::Modifiers::NONE)]);
        }
        assert!(
            ctx.memory(|m| m.has_focus(first.2)),
            "focus={:?} expected={:?}",
            ctx.memory(|m| m.focused()),
            first.2
        );
        assert!(!edit.has_changes());
    }
    #[test]
    fn resize_keeps_precise_native_graph_source_and_rejects_invalid_draft_dimensions() {
        use mathcraft_document::canvas::{PlotGraphSize, PlotNativeGeometry};
        let source = PlotSpec {
            native_geometry: Some(Box::new(PlotNativeGeometry {
                requested_width: Some(420.000003),
                requested_height: Some(260.000001),
                actual_width: Some(420.),
                actual_height: Some(260.),
                graph_size: Some(PlotGraphSize {
                    width: 35.75975582,
                    height: 99.5000001,
                }),
            })),
            ..Default::default()
        };
        let mut edit = InlinePlotEdit::default();
        edit.buffer(&source, false).replace_with("cos(x)");
        let before = edit.draft().unwrap().clone();
        assert!(!edit.dimensions(f32::NAN, 260.));
        assert_eq!(edit.draft(), Some(&before));
        assert!(edit.dimensions(480., 260.));
        let pending = edit.draft().unwrap();
        let geometry = pending.native_geometry.as_ref().unwrap();
        assert_eq!(
            geometry.graph_size,
            source.native_geometry.as_ref().unwrap().graph_size
        );
        assert_eq!(geometry.requested_width, Some(480.));
        assert_eq!(geometry.requested_height, Some(260.000001));
        assert_eq!(
            (geometry.actual_width, geometry.actual_height),
            (None, None)
        );
        assert_eq!(
            pending.primary_y_expression().as_legacy_text().unwrap(),
            "cos(x)"
        );
        assert_eq!(
            source.primary_y_expression().as_legacy_text().unwrap(),
            "sin(x)"
        );
    }
    #[test]
    fn measured_rows_fit_normal_outer_bounds_and_print_keeps_the_same_order() {
        use eframe::egui;
        fn text_rects(shape: &egui::Shape, out: &mut Vec<(String, egui::Rect)>) {
            match shape {
                egui::Shape::Text(text) => out.push((
                    text.galley.text().into(),
                    egui::Rect::from_min_size(text.pos, text.galley.size()),
                )),
                egui::Shape::Vec(v) => {
                    for shape in v {
                        text_rects(shape, out)
                    }
                }
                _ => (),
            }
        }
        for print in [false, true] {
            let ctx = egui::Context::default();
            crate::configure(&ctx);
            let mut source = many_source(false, 3);
            let before = source.clone();
            let mut edit = InlinePlotEdit::default();
            let mut bounds = None;
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1200., 1200.),
                    )),
                    ..Default::default()
                },
                |ui| {
                    let top = ui.cursor().min;
                    if print {
                        super::super::draw_print(ui, &source, &Err("body_marker".into()));
                        bounds = Some(egui::Rect::from_min_max(top, ui.min_rect().max));
                    } else {
                        bounds = Some(
                            super::super::draw(
                                ui,
                                &mut source,
                                &mut edit,
                                &Err("body_marker".into()),
                                false,
                            )
                            .response
                            .rect,
                        );
                    }
                },
            );
            output.textures_delta.clear();
            let mut text = Vec::new();
            for shape in &output.shapes {
                text_rects(&shape.shape, &mut text);
            }
            let bounds = bounds.unwrap();
            assert!(
                (bounds.height() - source.height).abs() < 1.,
                "print={print} bounds={bounds:?}"
            );
            let names = ["20", "21", "22", "body_marker", "1", "2", "3"];
            let mut bottom = bounds.top();
            for name in names {
                let rect = text.iter().find(|(label, _)| label == name).unwrap().1;
                assert!(rect.top() >= bottom, "print={print} name={name}");
                assert!(
                    bounds.contains_rect(rect),
                    "print={print} name={name} rect={rect:?} bounds={bounds:?}"
                );
                bottom = rect.bottom();
            }
            assert_eq!(source, before);
            assert!(!edit.has_changes());
            output.drop_without_applying_deltas();
        }
    }
}
