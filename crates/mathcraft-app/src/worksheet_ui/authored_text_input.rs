//! Controller for explicitly authored Text. Imported native anchors stay in
//! rich_text_input; this controller accepts only the separate validated seed.
use super::*;
use mathcraft_document::rich_text_source::{AuthoredRichText, AuthoredTextWire, TextSelection};
use rich_text_edit_map::{GlobalSelection, RunAffinity};
use rich_text_layout::{Replacement, RichTextLayout};

mod select_all_input;
pub(in crate::worksheet_ui) use select_all_input::{
    Delivery as SelectAllDelivery, delivery as select_all_delivery,
};
mod selected_backspace;
use selected_backspace::SelectedBackspace;

pub(super) struct FailedBinding {
    id: u64,
    wire: AuthoredTextWire,
    plain: String,
}

pub(super) struct AuthoredEditor {
    pub source: AuthoredRichText,
    pub(super) paragraph_alignment:
        Option<mathcraft_document::rich_text_source::ParagraphAlignmentInfo>,
    layout: RichTextLayout,
    pub affinity: RunAffinity,
    select_all: select_all_input::SelectAllInput,
}

impl AuthoredEditor {
    pub fn new(source: AuthoredRichText) -> Result<Self, String> {
        let visual = mathcraft_document::native_visual::read_flow_xml(
            source.xaml(),
            mathcraft_document::native_visual::TextStyle::default(),
        )?;
        let layout = RichTextLayout::from_validated(&visual, source.plain_text())?;
        let paragraph_alignment = source.paragraph_alignment_info().ok();
        Ok(Self {
            source,
            layout,
            paragraph_alignment,
            affinity: RunAffinity::default(),
            select_all: Default::default(),
        })
    }

    pub fn navigate(&mut self) {
        self.select_all.clear();
        self.affinity.clear();
    }

    pub fn horizontal_align(&self) -> egui::Align {
        self.layout.horizontal_align()
    }

    pub fn selection(&self, range: egui::text::CCursorRange) -> Option<TextSelection> {
        let s = rich_text_input::global(range);
        let start = s.anchor.min(s.focus);
        let end = s.anchor.max(s.focus);
        if start == end {
            return None;
        }
        self.layout.paragraphs.iter().find_map(|p| {
            let chars = self.source.plain_text()[p.bytes.clone()].chars().count();
            (p.scalar_start <= start && end <= p.scalar_start + chars).then_some(TextSelection {
                paragraph: p.index,
                anchor: s.anchor.saturating_sub(p.scalar_start),
                focus: s.focus.saturating_sub(p.scalar_start),
            })
        })
    }

    pub fn layout(
        &self,
        ui: &egui::Ui,
        buffer: &str,
        width: f32,
        before: egui::text::CCursorRange,
    ) -> Result<Arc<egui::Galley>, String> {
        let replacement = if let Some(owner) =
            self.select_all
                .preview_owner(&self.source, rich_text_input::global(before), buffer)
        {
            Some(Replacement {
                old_bytes: 0..self.source.plain_text().len(),
                inserted_style: *self
                    .layout
                    .run_styles
                    .get(owner)
                    .ok_or("Missing SelectAll insertion style")?,
            })
        } else if buffer == self.source.plain_text() {
            None
        } else {
            let before = rich_text_input::global(before);
            let old_len = self.source.plain_text().chars().count();
            let new_len = buffer.chars().count();
            let removed = before.anchor.abs_diff(before.focus);
            let inserted = new_len
                .checked_add(removed)
                .and_then(|n| n.checked_sub(old_len));
            let backspace = ui.input(|input| {
                input.events.iter().any(|event| {
                    matches!(
                        event,
                        egui::Event::Key {
                            key: egui::Key::Backspace,
                            pressed: true,
                            ..
                        }
                    )
                })
            });
            let caret = if before.anchor == before.focus && new_len < old_len && backspace {
                before.focus.saturating_sub(old_len - new_len)
            } else {
                inserted
                    .map(|n| before.anchor.min(before.focus) + n)
                    .unwrap_or(before.focus.min(new_len))
            };
            let plan = rich_text_edit_map::plan_committed(
                &self.source,
                before,
                GlobalSelection {
                    anchor: caret,
                    focus: caret,
                },
                buffer,
                &self.affinity,
            )?
            .ok_or("Changed authored draft has no Run replacement")?;
            Some(Replacement {
                old_bytes: plan.old_bytes,
                inserted_style: *self
                    .layout
                    .run_styles
                    .get(plan.run_ordinal)
                    .ok_or("Missing authored Run style")?,
            })
        };
        self.layout
            .project(buffer, replacement)?
            .layout(ui, buffer, width)
    }

    fn checked(source: AuthoredRichText, headroom: usize) -> Result<Self, String> {
        if source
            .plain_text()
            .len()
            .saturating_add(source.budget().serialized_wire_bytes())
            > headroom
        {
            return Err("Canvas text exceeds total 8 MiB budget".into());
        }
        Self::new(source)
    }

    pub fn commit(
        &mut self,
        text: &str,
        before: egui::text::CCursorRange,
        after: egui::text::CCursorRange,
        headroom: usize,
    ) -> Result<Option<AuthoredTextWire>, String> {
        self.commit_with_backspace(0, text, before, after, headroom, None)
    }

    pub fn commit_with_backspace(
        &mut self,
        owner: u64,
        text: &str,
        before: egui::text::CCursorRange,
        after: egui::text::CCursorRange,
        headroom: usize,
        proof: Option<SelectedBackspace>,
    ) -> Result<Option<AuthoredTextWire>, String> {
        if self.select_all.ready() {
            return self.commit_select_all(
                owner,
                text,
                rich_text_input::global(before),
                rich_text_input::global(after),
                headroom,
            );
        }
        if let Some(proof) = &proof {
            proof.validate_before(owner, &self.source, rich_text_input::global(before))?;
        }
        let mut affinity = self.affinity.clone();
        let Some(edit) = rich_text_edit_map::apply_authored_committed(
            &self.source,
            rich_text_input::global(before),
            rich_text_input::global(after),
            text,
            &mut affinity,
        )?
        else {
            return Ok(None);
        };
        let mut next = Self::checked(edit.patch.source, headroom)?;
        debug_assert_eq!(affinity.owner(&next.source), edit.run_ordinal);
        next.affinity = affinity;
        if let Some(proof) = proof {
            if edit.run_ordinal.is_some() {
                return Err("Captured Backspace was not a cross-Run deletion".into());
            }
            proof.rebind_after_checked(&mut next, text, rich_text_input::global(after))?;
        }
        let wire = next.source.wire().clone();
        *self = next;
        Ok(Some(wire))
    }

    pub fn split_paragraph(
        &mut self,
        cursor: egui::text::CCursorRange,
        headroom: usize,
    ) -> Result<(AuthoredTextWire, egui::text::CCursorRange), String> {
        let caret = rich_text_input::global(cursor);
        if caret.anchor != caret.focus {
            return Err("Paragraph split requires a collapsed caret".into());
        }
        let owner = self
            .source
            .run_char_ranges()
            .find(|run| run.global_start <= caret.focus && caret.focus <= run.global_end)
            .ok_or("Missing authored paragraph caret")?;
        let local = owner.start_in_paragraph + caret.focus - owner.global_start;
        let split = self.source.split_paragraph_at_caret(TextSelection {
            paragraph: owner.paragraph,
            anchor: local,
            focus: local,
        })?;
        let following = split
            .following_run
            .ok_or("Missing authored following Run")?;
        let mut next = Self::checked(split.source, headroom)?;
        next.affinity.remember_run(&next.source, following)?;
        let wire = next.source.wire().clone();
        *self = next;
        Ok((
            wire,
            egui::text::CCursorRange::one(egui::text::CCursor::new(caret.focus + 1)),
        ))
    }
}

/// One input/IME publication path with explicitly typed source outcomes.
pub(super) enum Editor<'a> {
    Native(&'a mut rich_text_input::RichEditor),
    Authored(&'a mut AuthoredEditor),
}
pub(super) enum Wire {
    Native(Box<mathcraft_document::rich_text_source::RichTextWire>),
    Authored(AuthoredTextWire),
}
impl Wire {
    pub fn install(
        self,
        native: &mut Option<Box<mathcraft_document::rich_text_source::RichTextWire>>,
        authored: &mut Option<Box<AuthoredTextWire>>,
    ) {
        match self {
            Self::Native(wire) => {
                *native = Some(wire);
                *authored = None;
            }
            Self::Authored(wire) => {
                *authored = Some(Box::new(wire));
                *native = None;
            }
        }
    }
}
impl Editor<'_> {
    pub fn plain_text(&self) -> &str {
        match self {
            Self::Native(e) => e.source.plain_text(),
            Self::Authored(e) => e.source.plain_text(),
        }
    }
    pub fn navigate(&mut self) {
        match self {
            Self::Native(e) => e.navigate(),
            Self::Authored(e) => e.navigate(),
        }
    }

    pub fn horizontal_align(&self) -> egui::Align {
        match self {
            Self::Native(e) => e.horizontal_align(),
            Self::Authored(e) => e.horizontal_align(),
        }
    }
    pub fn layout(
        &self,
        ui: &egui::Ui,
        text: &str,
        width: f32,
        before: egui::text::CCursorRange,
    ) -> Result<Arc<egui::Galley>, String> {
        match self {
            Self::Native(e) => e.layout(ui, text, width, before),
            Self::Authored(e) => e.layout(ui, text, width, before),
        }
    }
    pub fn capture_selected_backspace(
        &self,
        owner: u64,
        before: egui::text::CCursorRange,
        events: &[egui::Event],
        owns_focus: bool,
        composing: bool,
    ) -> Option<SelectedBackspace> {
        match self {
            Self::Native(_) => None,
            Self::Authored(editor) => SelectedBackspace::capture(
                owner,
                &editor.source,
                rich_text_input::global(before),
                events,
                owns_focus,
                composing,
            ),
        }
    }

    pub fn observe_backspace_delivery(
        &mut self,
        events: &[egui::Event],
        owns_focus: bool,
        composing: bool,
    ) {
        if let Self::Authored(editor) = self {
            selected_backspace::retire_nonordinary_delivery(editor, events, owns_focus, composing);
        }
    }

    pub fn commit_with_backspace(
        &mut self,
        owner: u64,
        text: &str,
        before: egui::text::CCursorRange,
        after: egui::text::CCursorRange,
        headroom: usize,
        proof: Option<SelectedBackspace>,
    ) -> Result<Option<Wire>, String> {
        if let Self::Native(editor) = self
            && editor.has_select_all_replacement()
        {
            return editor
                .commit_select_all(
                    owner,
                    text,
                    rich_text_input::global(before),
                    rich_text_input::global(after),
                    headroom,
                )
                .map(|wire| wire.map(|wire| Wire::Native(Box::new(wire))));
        }
        if let Self::Authored(editor) = self
            && editor.select_all.ready()
        {
            return editor
                .commit_select_all(
                    owner,
                    text,
                    rich_text_input::global(before),
                    rich_text_input::global(after),
                    headroom,
                )
                .map(|wire| wire.map(Wire::Authored));
        }
        let Some(proof) = proof else {
            return self.commit(text, before, after, headroom);
        };
        match self {
            Self::Native(_) => Err("Authored Backspace proof cannot bind a native owner".into()),
            Self::Authored(editor) => editor
                .commit_with_backspace(owner, text, before, after, headroom, Some(proof))
                .map(|wire| wire.map(Wire::Authored)),
        }
    }

    pub fn commit(
        &mut self,
        text: &str,
        before: egui::text::CCursorRange,
        after: egui::text::CCursorRange,
        headroom: usize,
    ) -> Result<Option<Wire>, String> {
        match self {
            Self::Native(e) => e
                .commit(text, before, after, headroom)
                .map(|v| v.map(|wire| Wire::Native(Box::new(wire)))),
            Self::Authored(e) => e
                .commit(text, before, after, headroom)
                .map(|v| v.map(Wire::Authored)),
        }
    }
    pub fn split_paragraph(
        &mut self,
        cursor: egui::text::CCursorRange,
        headroom: usize,
    ) -> Result<(Wire, egui::text::CCursorRange), String> {
        match self {
            Self::Native(e) => e
                .split_paragraph(cursor, headroom)
                .map(|(v, c)| (Wire::Native(Box::new(v)), c)),
            Self::Authored(e) => e
                .split_paragraph(cursor, headroom)
                .map(|(v, c)| (Wire::Authored(v), c)),
        }
    }
}

pub(super) enum FormatEditor<'a> {
    Native(&'a rich_text_input::RichEditor),
    Authored(&'a AuthoredEditor),
}
impl FormatEditor<'_> {
    pub fn source_pointer(&self) -> usize {
        match self {
            Self::Native(e) => e.source.xaml().as_ptr() as usize,
            Self::Authored(e) => e.source.xaml().as_ptr() as usize,
        }
    }
    pub fn selection(&self, range: egui::text::CCursorRange) -> Option<TextSelection> {
        match self {
            Self::Native(e) => e.selection(range),
            Self::Authored(e) => e.selection(range),
        }
    }
    pub fn selection_text_decorations_supported(
        &self,
        selection: TextSelection,
    ) -> Result<bool, String> {
        match self {
            Self::Native(e) => e.source.selection_text_decorations_supported(selection),
            Self::Authored(e) => e.source.selection_text_decorations_supported(selection),
        }
    }
    pub fn selection_format(
        &self,
        selection: TextSelection,
    ) -> Result<mathcraft_document::rich_text_source::TextFormat, String> {
        match self {
            Self::Native(e) => e.source.selection_format(selection),
            Self::Authored(e) => e.source.selection_format(selection),
        }
    }
}
impl rich_text_input::State {
    pub(super) fn format_editor(&self, id: u64) -> Option<FormatEditor<'_>> {
        if let Some(Ok(e)) = self.authored_editors.get(&id) {
            return Some(FormatEditor::Authored(e));
        }
        self.editors
            .get(&id)?
            .as_ref()
            .ok()
            .map(FormatEditor::Native)
    }
}

impl WorksheetApp {
    /// Materialize a creation seed only on a command. No catalog work occurs
    /// in idle drawing. Current format capture is distinct from mixed buttons.
    fn initial_authored_seed(
        &mut self,
    ) -> Result<Arc<mathcraft_document::rich_text_source::AuthoredTextSeed>, String> {
        if let Some(id) = self.selected {
            self.prepare_authored_text(id);
            self.capture_authored_creation(id);
        }
        if let Some(snapshot) = &self.rich_text.retained_creation {
            return snapshot.materialize();
        }
        self.rich_text
            .creation_seed
            .get_or_insert_with(|| {
                if let Some(bytes) = self.document.native_archive.as_deref() {
                    let package = NativePackage::from_bytes(bytes)?;
                    let presentation = package
                        .original_xml_part("mathcad/settings/presentation.xml")?
                        .ok_or("Missing active Text presentation")?;
                    return text_creation_seed::seed_for_native_body(&presentation);
                }
                // Explicit fresh Mathcraft profile: recovered TextFormat fallback
                // appearance and this app's Korean proofing policy. This is not a
                // captured Prime fresh-empty serialization or a native override.
                use mathcraft_document::rich_text_source::{
                    AuthoredStyleSnapshot, AuthoredTextSeed, AuthoredTextSeedSpec,
                };
                let style = AuthoredStyleSnapshot {
                    key: "Normal".into(),
                    font_family: "Tahoma".into(),
                    size_dip: ((11f64 / 72.) * 96.).to_string(),
                    weight: "Normal".into(),
                    font_style: "Normal".into(),
                    alignment: "Left".into(),
                    foreground_rgba: [0, 0, 0, 255],
                    background_rgba: [255, 255, 255, 0],
                };
                AuthoredTextSeed::new(AuthoredTextSeedSpec {
                    format: style.clone(),
                    catalog: vec![style],
                    proofing_language: "ko-KR".into(),
                    text_decorations: None,
                })
            })
            .clone()
    }

    pub(super) fn insert_authored_text(
        &mut self,
        position: (f64, f64),
        initial: &str,
    ) -> Result<u64, String> {
        let seed = self.initial_authored_seed()?;
        let mut source = AuthoredRichText::empty(seed)?;
        let mut ordinal = 0;
        for (index, line) in initial.split('\n').enumerate() {
            if index != 0 {
                let previous = source
                    .run_char_ranges()
                    .last()
                    .ok_or("Missing initial Text Run")?;
                let split = source.split_paragraph_at_caret(TextSelection {
                    paragraph: previous.paragraph,
                    anchor: previous.end_in_paragraph,
                    focus: previous.end_in_paragraph,
                })?;
                ordinal = split.following_run.ok_or("Missing initial paragraph Run")?;
                source = split.source;
            }
            if !line.is_empty() {
                source = source
                    .replace_in_run(ordinal, 0, 0, line)?
                    .ok_or("Missing initial Text edit")?
                    .source;
            }
        }
        let mut editor = AuthoredEditor::new(source)?;
        editor.affinity.remember_run(&editor.source, ordinal)?;
        let id = self.document.next_id;
        let next_id = id.checked_add(1).ok_or("Text region ID overflow")?;
        let region = CanvasRegion {
            id,
            left: position.0,
            top: position.1,
            content: CanvasContent::Text(editor.source.plain_text().into()),
            source_region: None,
            rich_text: None,
            authored_text: Some(Box::new(editor.source.wire().clone())),
            format: Default::default(),
            calculation_disabled: false,
            calculation_enabled_override: false,
        };
        self.document.validate_text_append_budget(&region)?;
        let before = self.snapshot();
        self.document.next_id = next_id;
        self.document.regions.push(region);
        self.remember(before);
        self.rich_text.authored_editors.clear();
        self.rich_text.editors.clear();
        self.rich_text.authored_editors.insert(id, Ok(editor));
        self.selected = Some(id);
        self.focus_new = Some(id);
        self.reveal = Some(position);
        self.insertion = None;
        self.refresh_rich_text_visual(id);
        Ok(id)
    }

    pub(super) fn prepare_authored_text(&mut self, id: u64) {
        let Some(region) = self.document.regions.iter().find(|r| r.id == id) else {
            self.rich_text.authored_failed = None;
            return;
        };
        let Some(wire) = region.authored_text.as_deref() else {
            self.rich_text.authored_editors.clear();
            self.rich_text.authored_failed = None;
            return;
        };
        let CanvasContent::Text(plain) = &region.content else {
            return;
        };
        let same = self
            .rich_text
            .authored_editors
            .get(&id)
            .is_some_and(|entry| {
                entry.as_ref().is_ok_and(|e| {
                    e.source.wire().same_source_version(wire) && e.source.plain_text() == plain
                })
            });
        let failed_same = region.source_region.is_none()
            && region.rich_text.is_none()
            && self
                .rich_text
                .authored_editors
                .get(&id)
                .is_some_and(Result::is_err)
            && self
                .rich_text
                .authored_failed
                .as_ref()
                .is_some_and(|failed| {
                    failed.id == id
                        && failed.wire.same_source_version(wire)
                        && failed.plain == *plain
                });
        if same || failed_same {
            return;
        }
        self.rich_text.authored_failed = None;
        self.rich_text.authored_editors.clear();
        self.rich_text.editors.clear();
        self.rich_text.invalidate_target();
        #[cfg(test)]
        {
            self.rich_text.authored_admission_attempts += 1;
        }
        let admitted = region
            .validated_authored_text()
            .and_then(|source| source.ok_or_else(|| "Missing authored Text source".to_string()));
        let value = match admitted {
            Ok(source) => {
                // Cache only a display/editability failure AFTER full source
                // admission. Invalid cold DTOs are never trusted by this cache.
                let value = AuthoredEditor::new(source);
                if value.is_err() {
                    self.rich_text.authored_failed = Some(FailedBinding {
                        id,
                        wire: wire.clone(),
                        plain: plain.clone(),
                    });
                }
                value
            }
            Err(error) => Err(error),
        };
        self.rich_text.authored_editors.insert(id, value);
    }

    pub(super) fn authored_format_command(
        &mut self,
        ctx: &egui::Context,
        id: u64,
        range: egui::text::CCursorRange,
        format: &mathcraft_document::rich_text_source::TextFormat,
    ) {
        // Explicit formatting, including a no-op setter, retires SelectAll.
        if let Some(Ok(editor)) = self.rich_text.authored_editors.get_mut(&id) {
            editor.retire_select_all();
        }
        let result = (|| {
            let editor = self
                .rich_text
                .authored_editors
                .get(&id)
                .ok_or("Missing authored editor")?
                .as_ref()
                .map_err(Clone::clone)?;
            let selection = editor
                .selection(range)
                .ok_or("Expected a nonempty authored selection")?;
            let Some(patch) = editor.source.set_selection_format(selection, format)? else {
                return Ok(None);
            };
            let mut next = AuthoredEditor::new(patch.source)?;
            let _ = next
                .affinity
                .remember_selection(&next.source, rich_text_input::global(range));
            self.document
                .validate_authored_text_edit_budget(id, &next.source)?;
            Ok::<_, String>(Some(next))
        })();
        let next = match result {
            Ok(Some(next)) => next,
            Ok(None) => return,
            Err(error) => {
                self.status = format!("텍스트 서식: {error}");
                return;
            }
        };
        let Some(region) = self.document.regions.iter().find(|r| r.id == id) else {
            return;
        };
        let before = self.snapshot_with_document(DocumentHistory::Region(Box::new(region.clone())));
        let wire = next.source.wire().clone();
        self.document
            .regions
            .iter_mut()
            .find(|r| r.id == id)
            .unwrap()
            .authored_text = Some(Box::new(wire));
        self.rich_text.authored_editors.insert(id, Ok(next));
        self.remember(before);
        self.mark_dirty();
        self.status.clear();
        self.refresh_rich_text_visual(id);
        self.region_heights.remove(&id);
        self.region_widths.remove(&id);
        self.focus_new = Some(id);
        ctx.request_repaint();
    }

    pub(super) fn refresh_authored_text_visual(&mut self, id: u64) {
        self.capture_authored_creation(id);
        let source = self
            .document
            .regions
            .iter()
            .find(|r| r.id == id)
            .ok_or_else(|| "Missing authored Text region".to_string())
            .and_then(|region| {
                // Reuse only the exact admitted immutable source version and
                // current plain mirror; provenance/type checks remain local.
                if let CanvasContent::Text(plain) = &region.content
                    && region.source_region.is_none()
                    && region.rich_text.is_none()
                    && let Some(wire) = region.authored_text.as_deref()
                    && let Some(Ok(editor)) = self.rich_text.authored_editors.get(&id)
                    && editor.source.wire().same_source_version(wire)
                    && editor.source.plain_text() == plain
                {
                    return Ok(editor.source.clone());
                }
                region
                    .validated_authored_text()?
                    .ok_or_else(|| "Missing authored Text source".to_string())
            });
        let result = source.and_then(|source| {
            let region = self
                .document
                .regions
                .iter()
                .find(|r| r.id == id)
                .ok_or("Missing authored region")?;
            Arc::make_mut(&mut self.native_visuals).update_authored_text(region, &source)
        });
        if let Err(error) = result {
            self.status = format!("텍스트 서식: {error}");
        }
    }

    fn capture_authored_creation(&mut self, id: u64) {
        let Some(Ok(editor)) = self.rich_text.authored_editors.get(&id) else {
            return;
        };
        let ordinal = editor.affinity.owner(&editor.source).or_else(|| {
            rich_text_edit_map::selection_owner(
                &editor.source,
                rich_text_input::global(*self.text_cursors.get(&id)?),
                &editor.affinity,
            )
            .ok()
        });
        if let Some(snapshot) =
            ordinal.and_then(|run| editor.source.creation_snapshot_for_run(run).ok())
        {
            self.rich_text.retained_creation = Some(snapshot);
        }
    }
}

#[cfg(test)]
#[path = "authored_error_cache_tests.rs"]
mod error_cache_tests;

#[path = "authored_text_input/paragraph_alignment.rs"]
mod paragraph_alignment;
