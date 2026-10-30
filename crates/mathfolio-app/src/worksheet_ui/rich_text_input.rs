//! Source-owned Text editing and explicit range-format commands.
use super::*;
use mathfolio_document::rich_text_source::{
    DipSize, FontStyle, OriginalTextAuthority, TextDecorations, TextFormat, TextSelection,
    ValidatedRichText, Weight,
};
use rich_text_edit_map::{GlobalSelection, RunAffinity};
use rich_text_layout::{Replacement, RichTextLayout};

#[cfg(test)]
#[path = "rich_text_shortcut_guard_tests.rs"]
mod shortcut_guard_tests;

mod select_all_input;

pub(super) struct RichEditor {
    pub source: ValidatedRichText,
    authority: Arc<OriginalTextAuthority>,
    base_style: mathfolio_document::native_visual::TextStyle,
    pub(super) paragraph_alignment:
        Option<mathfolio_document::rich_text_source::ParagraphAlignmentInfo>,
    layout: RichTextLayout,
    affinity: RunAffinity,
    select_all: select_all_input::SelectAllInput,
}

#[derive(Default)]
pub(super) struct State {
    package: Option<Result<Arc<NativePackage>, String>>,
    pub editors: HashMap<u64, Result<RichEditor, String>>,
    pub authored_editors: HashMap<u64, Result<authored_text_input::AuthoredEditor, String>>,
    pub creation_seed:
        Option<Result<Arc<mathfolio_document::rich_text_source::AuthoredTextSeed>, String>>,
    pub retained_creation: Option<mathfolio_document::rich_text_source::AuthoredCreationSnapshot>,
    pub(super) authored_failed: Option<authored_text_input::FailedBinding>,
    #[cfg(test)]
    pub(super) authored_admission_attempts: usize,
    retained_bold: bool,
    retained_italic: bool,
    retained_underline: bool,
    target: Option<(u64, usize, usize, usize)>,
    context: Option<ribbon::TextFormatContext>,
    serial: u64,
}

pub(super) fn global(range: egui::text::CCursorRange) -> GlobalSelection {
    GlobalSelection {
        anchor: range.secondary.index.0,
        focus: range.primary.index.0,
    }
}

impl RichEditor {
    fn new(
        source: ValidatedRichText,
        authority: Arc<OriginalTextAuthority>,
        base_style: mathfolio_document::native_visual::TextStyle,
    ) -> Result<Self, String> {
        let visual =
            mathfolio_document::native_visual::read_flow_xml(source.xaml(), base_style.clone())?;
        let layout = RichTextLayout::from_validated(&visual, source.plain_text())?;
        let paragraph_alignment = source.paragraph_alignment_info().ok();
        Ok(Self {
            source,
            authority,
            base_style,
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

    /// An explicit ordinary Enter, never inferred from a pasted/IME newline.
    /// Prepare source, geometry, budget and the following Run before publishing.
    pub fn split_paragraph(
        &mut self,
        cursor: egui::text::CCursorRange,
        headroom: usize,
    ) -> Result<
        (
            mathfolio_document::rich_text_source::RichTextWire,
            egui::text::CCursorRange,
        ),
        String,
    > {
        let caret = global(cursor);
        if caret.anchor != caret.focus {
            return Err("Paragraph split requires a collapsed caret".into());
        }
        let owner = self
            .source
            .run_char_ranges()
            .find(|run| run.global_start < caret.focus && caret.focus < run.global_end)
            .ok_or("Paragraph split currently requires a caret inside one nonempty Run")?;
        let local = owner.start_in_paragraph + caret.focus - owner.global_start;
        let split = self.source.split_paragraph_at_caret(TextSelection {
            paragraph: owner.paragraph,
            anchor: local,
            focus: local,
        })?;
        let mut next = Self::new(
            split.patch.source,
            self.authority.clone(),
            self.base_style.clone(),
        )?;
        if next
            .source
            .plain_text()
            .len()
            .saturating_add(next.source.wire().source_bytes())
            > headroom
        {
            return Err("Canvas text exceeds total 8 MiB budget".into());
        }
        next.affinity
            .remember_run(&next.source, split.following_run)?;
        let wire = next.source.wire().clone();
        let cursor = egui::text::CCursorRange::one(egui::text::CCursor::new(caret.focus + 1));
        *self = next;
        Ok((wire, cursor))
    }

    /// This pure draft projection cannot publish IME text into source/history.
    pub fn layout(
        &self,
        ui: &egui::Ui,
        buffer: &str,
        width: f32,
        before: egui::text::CCursorRange,
    ) -> Result<Arc<egui::Galley>, String> {
        let replacement = if let Some(owner) =
            self.select_all
                .preview_owner(&self.source, global(before), buffer)
        {
            Some(Replacement {
                old_bytes: 0..self.source.plain_text().len(),
                inserted_style: *self
                    .layout
                    .run_styles
                    .get(owner)
                    .ok_or("Missing native SelectAll insertion style")?,
            })
        } else if buffer == self.source.plain_text() {
            None
        } else {
            let before = global(before);
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
            let after = GlobalSelection {
                anchor: caret,
                focus: caret,
            };
            let plan = rich_text_edit_map::plan_committed(
                &self.source,
                before,
                after,
                buffer,
                &self.affinity,
            )?
            .ok_or("Changed draft has no source-owned replacement")?;
            Some(Replacement {
                old_bytes: plan.old_bytes,
                inserted_style: *self
                    .layout
                    .run_styles
                    .get(plan.run_ordinal)
                    .ok_or("Missing Run style")?,
            })
        };
        self.layout
            .project(buffer, replacement)?
            .layout(ui, buffer, width)
    }

    pub fn commit(
        &mut self,
        text: &str,
        before: egui::text::CCursorRange,
        after: egui::text::CCursorRange,
        headroom: usize,
    ) -> Result<Option<mathfolio_document::rich_text_source::RichTextWire>, String> {
        // Affinity mutates only on the candidate. Layout errors leave the live
        // editor, source and caret owner intact.
        let mut affinity = self.affinity.clone();
        let Some(edit) = rich_text_edit_map::apply_committed(
            &self.source,
            global(before),
            global(after),
            text,
            &mut affinity,
        )?
        else {
            return Ok(None);
        };
        debug_assert!(edit.old_bytes.end <= self.source.plain_text().len());
        debug_assert_eq!(edit.after_selection, global(after));
        let mut next = Self::new(
            edit.patch.source,
            self.authority.clone(),
            self.base_style.clone(),
        )?;
        if text.len().saturating_add(next.source.wire().source_bytes()) > headroom {
            return Err("Canvas text exceeds total 8 MiB budget".into());
        }
        next.affinity = affinity;
        let wire = next.source.wire().clone();
        *self = next;
        Ok(Some(wire))
    }

    pub(super) fn selection(&self, range: egui::text::CCursorRange) -> Option<TextSelection> {
        let s = global(range);
        let start = s.anchor.min(s.focus);
        let end = s.anchor.max(s.focus);
        if start == end {
            return None;
        }
        self.layout.paragraphs.iter().find_map(|paragraph| {
            let chars = self.source.plain_text()[paragraph.bytes.clone()]
                .chars()
                .count();
            (paragraph.scalar_start <= start && end <= paragraph.scalar_start + chars).then(|| {
                TextSelection {
                    paragraph: paragraph.index,
                    anchor: s.anchor - paragraph.scalar_start,
                    focus: s.focus - paragraph.scalar_start,
                }
            })
        })
    }
}

impl State {
    pub(super) fn affinity(&self, id: u64) -> Option<RunAffinity> {
        if let Some(Ok(editor)) = self.authored_editors.get(&id) {
            return Some(editor.affinity.clone());
        }
        self.editors
            .get(&id)?
            .as_ref()
            .ok()
            .map(|editor| editor.affinity.clone())
    }
    pub(super) fn invalidate_target(&mut self) {
        self.target = None;
        self.context = None;
    }

    fn read_source(
        &mut self,
        document: &CanvasDocument,
        id: u64,
    ) -> Result<(ValidatedRichText, Arc<OriginalTextAuthority>), String> {
        let region = document
            .regions
            .iter()
            .find(|r| r.id == id)
            .ok_or("Missing Text owner")?;
        let CanvasContent::Text(plain) = &region.content else {
            return Err("Expected Text owner".into());
        };
        let package = self.package.get_or_insert_with(|| {
            NativePackage::from_bytes(
                document
                    .native_archive
                    .as_deref()
                    .ok_or("Missing original rich Text archive")?,
            )
            .map(Arc::new)
        });
        let package = package.as_ref().map_err(Clone::clone)?;
        let owner = region
            .rich_text
            .as_ref()
            .map(|wire| wire.origin().identity().owner_region_id.as_str())
            .or(region.source_region.as_deref())
            .ok_or("Missing native Text owner")?;
        let authority = mathfolio_document::export::read_native_text_authority(package, owner)?;
        let source = if let Some(wire) = region.rich_text.as_deref() {
            ValidatedRichText::from_wire(wire.clone(), authority.clone(), plain)?
        } else {
            ValidatedRichText::from_original(authority.clone(), plain)?
        };
        Ok((source, authority))
    }

    fn editor(
        &mut self,
        document: &CanvasDocument,
        id: u64,
        visuals: &NativeVisuals,
    ) -> Option<&mut Result<RichEditor, String>> {
        let region = document.regions.iter().find(|r| r.id == id)?;
        let CanvasContent::Text(plain) = &region.content else {
            return None;
        };
        // Newly authored plain Text keeps its existing full multiline path.
        if region.rich_text.is_none() && region.source_region.is_none() {
            return None;
        }
        if region.rich_text.is_none() && document.native_archive.is_none() {
            return None;
        }
        if let Some(Ok(current)) = self.editors.get(&id) {
            let same = region.rich_text.as_deref().map_or_else(
                || current.source.xaml() == current.authority.original_xaml(),
                |wire| wire == current.source.wire(),
            ) && current.source.plain_text() == plain;
            if !same {
                self.editors.remove(&id);
                self.target = None;
                self.context = None;
            }
        }
        if !self.editors.contains_key(&id) {
            // Only the active owner retains parsing/layout/affinity state.
            // Readonly/print visuals have their own bounded document cache.
            self.editors.clear();
            let value = self
                .read_source(document, id)
                .and_then(|(source, authority)| {
                    let owner = &authority.anchor().identity().owner_region_id;
                    let base = visuals
                        .regions
                        .get(owner)
                        .and_then(|v| v.as_ref().ok())
                        .and_then(|v| v.document.as_ref())
                        .map(|d| d.style.clone())
                        .unwrap_or_default();
                    RichEditor::new(source, authority, base)
                });
            self.editors.insert(id, value);
        }
        self.editors.get_mut(&id)
    }
}

impl WorksheetApp {
    pub(super) fn clipboard_rich_text(
        &mut self,
        id: u64,
    ) -> Result<Option<Box<mathfolio_document::rich_text_source::RichTextWire>>, String> {
        let region = self
            .document
            .regions
            .iter()
            .find(|r| r.id == id)
            .ok_or("Missing Text region")?;
        if !matches!(region.content, CanvasContent::Text(_))
            || (region.rich_text.is_none()
                && (region.source_region.is_none() || self.document.native_archive.is_none()))
        {
            return Ok(None);
        }
        if let Some(wire) = &region.rich_text {
            return Ok(Some(wire.clone()));
        }
        let (source, _) = self.rich_text.read_source(&self.document, id)?;
        Ok(Some(Box::new(source.wire().clone())))
    }

    pub(super) fn validate_rich_text_append(
        &mut self,
        candidate: &CanvasRegion,
    ) -> Result<(), String> {
        self.rich_text.validate_append_to(&self.document, candidate)
    }

    pub(super) fn restore_rich_text_affinity(&mut self, id: u64, affinity: &RunAffinity) {
        self.prepare_authored_text(id);
        if let Some(Ok(editor)) = self.rich_text.authored_editors.get_mut(&id) {
            editor.affinity.restore_if_bound(&editor.source, affinity);
            return;
        }
        if let Some(Ok(editor)) = self
            .rich_text
            .editor(&self.document, id, &self.native_visuals)
        {
            editor.affinity.restore_if_bound(&editor.source, affinity);
        }
    }
    pub(super) fn prepare_rich_text(&mut self) {
        if let Some(id) = self.selected.or(self.focus_new) {
            self.rich_text
                .editor(&self.document, id, &self.native_visuals);
            self.prepare_authored_text(id);
        }
    }

    pub(super) fn rich_text_context(&mut self) -> Option<ribbon::TextFormatContext> {
        let result = self.rich_text_context_inner();
        if result.is_none() {
            self.rich_text.target = None;
            self.rich_text.context = None;
        }
        result
    }

    fn rich_text_context_inner(&mut self) -> Option<ribbon::TextFormatContext> {
        self.prepare_rich_text();
        if self.text_composition.is_some() {
            return None;
        }
        let id = self.selected?;
        let range = *self.text_cursors.get(&id)?;
        let editor = self.rich_text.format_editor(id)?;
        let target = (
            id,
            editor.source_pointer(),
            range.primary.index.0,
            range.secondary.index.0,
        );
        if self.rich_text.target == Some(target) {
            return self.rich_text.context;
        }
        let selection = editor.selection(range)?;
        let format = editor.selection_format(selection).ok()?;
        let underline_enabled = editor
            .selection_text_decorations_supported(selection)
            .ok()?;
        if let Some(weight) = format.weight {
            self.rich_text.retained_bold = weight == Weight::Bold;
        }
        if let Some(style) = format.style {
            self.rich_text.retained_italic = style == FontStyle::Italic;
        }
        if let Some(decoration) = format.text_decorations {
            self.rich_text.retained_underline = decoration == TextDecorations::Underline;
        }
        self.rich_text.serial = self.rich_text.serial.wrapping_add(1);
        let context = ribbon::TextFormatContext {
            enabled: true,
            selection_key: self.rich_text.serial,
            retained_bold: self.rich_text.retained_bold,
            retained_italic: self.rich_text.retained_italic,
            retained_underline: self.rich_text.retained_underline,
            underline_enabled,
            // Selection and source/geometry were admitted above. Explicit
            // collection assignment is separate from reading/toggling it.
            underline_set_enabled: true,
            font_points: format
                .size_dip
                .and_then(|s| s.as_str().parse::<f64>().ok())
                .map(|dip| (dip / 96. * 72. * 10.).round() / 10.),
            color_rgba: format.foreground_argb.map(|[a, r, g, b]| [r, g, b, a]),
        };
        self.rich_text.target = Some(target);
        self.rich_text.context = Some(context);
        Some(context)
    }

    pub(super) fn rich_text_command(&mut self, ctx: &egui::Context, action: RibbonAction) {
        let (key, format) = match action {
            RibbonAction::SetTextBold {
                value,
                selection_key,
            } => (
                selection_key,
                TextFormat {
                    weight: Some(if value { Weight::Bold } else { Weight::Normal }),
                    ..Default::default()
                },
            ),
            RibbonAction::SetTextUnderline {
                value,
                selection_key,
            } => (
                selection_key,
                TextFormat {
                    text_decorations: Some(if value {
                        TextDecorations::Underline
                    } else {
                        TextDecorations::None
                    }),
                    ..Default::default()
                },
            ),
            RibbonAction::SetTextItalic {
                value,
                selection_key,
            } => (
                selection_key,
                TextFormat {
                    style: Some(if value {
                        FontStyle::Italic
                    } else {
                        FontStyle::Normal
                    }),
                    ..Default::default()
                },
            ),
            RibbonAction::SetTextColor {
                rgba: [r, g, b, a],
                selection_key,
            } => (
                selection_key,
                TextFormat {
                    foreground_argb: Some([a, r, g, b]),
                    ..Default::default()
                },
            ),
            RibbonAction::SetTextFontSize {
                points,
                selection_key,
            } => {
                if !points.is_finite()
                    || !(1. ..=1638.).contains(&points)
                    || (points * 10. - (points * 10.).round()).abs() > 1e-8
                {
                    return;
                }
                let Ok(size) = DipSize::parse((points / 72. * 96.).to_string()) else {
                    return;
                };
                (
                    selection_key,
                    TextFormat {
                        size_dip: Some(size),
                        ..Default::default()
                    },
                )
            }
            _ => return,
        };
        let Some(current) = self.rich_text_context() else {
            return;
        };
        if current.selection_key != key
            || (format.text_decorations.is_some() && !current.underline_set_enabled)
        {
            return;
        }
        let id = self.selected.unwrap();
        let range = self.text_cursors[&id];
        if self.rich_text.authored_editors.contains_key(&id) {
            self.authored_format_command(ctx, id, range, &format);
            return;
        }
        if let Some(Ok(editor)) = self.rich_text.editors.get_mut(&id) {
            editor.retire_select_all();
        }
        let editor = self.rich_text.editors.get(&id).unwrap().as_ref().unwrap();
        let selection = editor.selection(range).unwrap();
        let result = editor
            .source
            .set_selection_format(selection, &format)
            .and_then(|patch| {
                patch
                    .map(|patch| {
                        RichEditor::new(
                            patch.source,
                            editor.authority.clone(),
                            editor.base_style.clone(),
                        )
                    })
                    .transpose()
            });
        let next = match result {
            Ok(Some(mut next)) => {
                let _ = next
                    .affinity
                    .remember_selection(&next.source, global(range));
                next
            }
            Ok(None) => return,
            Err(error) => {
                self.status = format!("텍스트 서식: {error}");
                return;
            }
        };
        let Some(region) = self.document.regions.iter().find(|r| r.id == id) else {
            return;
        };
        let mut candidate = region.clone();
        candidate.rich_text = Some(Box::new(next.source.wire().clone()));
        let checked = self
            .document
            .validate_text_replace_budget(&candidate)
            .and_then(|_| {
                let package = self
                    .rich_text
                    .package
                    .as_ref()
                    .ok_or("Missing rich Text authority")?
                    .as_ref()
                    .map_err(Clone::clone)?;
                mathfolio_document::export::validate_rich_text_replacement(
                    &self.document,
                    package,
                    &candidate,
                )
                .map(|_| ())
            });
        if let Err(error) = checked {
            self.status = format!("텍스트 서식: {error}");
            return;
        }
        let before = self.snapshot_with_document(DocumentHistory::Region(Box::new(region.clone())));
        let wire = next.source.wire().clone();
        self.document
            .regions
            .iter_mut()
            .find(|r| r.id == id)
            .unwrap()
            .rich_text = Some(Box::new(wire));
        self.rich_text.editors.insert(id, Ok(next));
        self.remember(before);
        self.mark_dirty();
        self.status.clear();
        self.refresh_rich_text_visual(id);
        self.region_heights.remove(&id);
        self.region_widths.remove(&id);
        self.focus_new = Some(id);
        ctx.request_repaint();
    }

    pub(super) fn rich_text_shortcuts(&mut self, ctx: &egui::Context) {
        let Some(id) = self.selected else {
            return;
        };
        if !self
            .document
            .regions
            .iter()
            .any(|r| r.id == id && matches!(r.content, CanvasContent::Text(_)))
            || !ctx.memory(|m| m.has_focus(egui::Id::new(("worksheet-text", id))))
        {
            return;
        }
        // Inspect actual keys before context. Ctrl+U is an egui destructive
        // paragraph-delete command: reserve it only for this worksheet Text
        // owner, even when collapsed/unknown/IME makes formatting unsupported.
        // Ineligible B/I deliberately remain unconsumed as before.
        let keys = ctx.input(|input| {
            input
                .events
                .iter()
                .filter_map(|event| match event {
                    egui::Event::Key {
                        key: key @ (egui::Key::B | egui::Key::I | egui::Key::U),
                        pressed: true,
                        modifiers,
                        ..
                    } if modifiers.matches_exact(egui::Modifiers::COMMAND) => Some(*key),
                    _ => None,
                })
                .collect::<Vec<_>>()
        });
        for key in keys {
            let underline = key == egui::Key::U;
            if underline
                && !ctx.input_mut(|input| consume_exact_key(input, egui::Modifiers::COMMAND, key))
            {
                continue;
            }
            let Some(context) = self.rich_text_context() else {
                continue;
            };
            if underline && !context.underline_enabled {
                continue;
            }
            if !underline
                && !ctx.input_mut(|input| consume_exact_key(input, egui::Modifiers::COMMAND, key))
            {
                continue;
            }
            let Some(editor) = self.rich_text.format_editor(id) else {
                continue;
            };
            let Some(selection) = editor.selection(self.text_cursors[&id]) else {
                continue;
            };
            let Ok(format) = editor.selection_format(selection) else {
                continue;
            };
            let action = match key {
                egui::Key::U => RibbonAction::SetTextUnderline {
                    // Known mixed None/Underline follows keyboard Unset->Underline;
                    // toolbar mixed retains its previous checked value separately.
                    value: format.text_decorations != Some(TextDecorations::Underline),
                    selection_key: context.selection_key,
                },
                egui::Key::B => RibbonAction::SetTextBold {
                    value: format.weight != Some(Weight::Bold),
                    selection_key: context.selection_key,
                },
                egui::Key::I => RibbonAction::SetTextItalic {
                    value: format.style != Some(FontStyle::Italic),
                    selection_key: context.selection_key,
                },
                _ => unreachable!(),
            };
            self.rich_text_command(ctx, action);
        }
    }

    pub(super) fn refresh_rich_text_visual(&mut self, id: u64) {
        self.rich_text.invalidate_target();
        self.paragraph_alignment.invalidate();
        let Some(region) = self.document.regions.iter().find(|r| r.id == id) else {
            return;
        };
        if region.authored_text.is_some() {
            self.refresh_authored_text_visual(id);
            return;
        }
        if region.rich_text.is_none() {
            Arc::make_mut(&mut self.native_visuals).remove_rich_text_overlay(id);
            return;
        }
        self.rich_text
            .editor(&self.document, id, &self.native_visuals);
        // Editable geometry is narrower than readonly native presentation.
        // A centered/line-spaced paragraph still keeps its source-bound visual.
        let source = if let Some(Ok(editor)) = self.rich_text.editors.get(&id) {
            Ok(editor.source.clone())
        } else {
            self.rich_text
                .read_source(&self.document, id)
                .map(|(source, _)| source)
        };
        if let Err(error) = source.and_then(|source| {
            Arc::make_mut(&mut self.native_visuals).update_rich_text(region, &source)
        }) {
            self.status = format!("텍스트 서식: {error}");
        }
    }
}

#[path = "rich_text_input/paragraph_alignment.rs"]
mod paragraph_alignment;

impl State {
    /// Validate against the progressively built document without changing live source.
    pub(super) fn validate_append_to(
        &mut self,
        document: &CanvasDocument,
        candidate: &CanvasRegion,
    ) -> Result<(), String> {
        document.validate_text_append_budget(candidate)?;
        if candidate.authored_text.is_some() {
            candidate.validated_authored_text()?;
            return Ok(());
        }
        if candidate.rich_text.is_none() {
            return Ok(());
        }
        let package = self.package.get_or_insert_with(|| {
            NativePackage::from_bytes(
                document
                    .native_archive
                    .as_deref()
                    .ok_or("Missing original rich Text archive")?,
            )
            .map(Arc::new)
        });
        mathfolio_document::export::validate_rich_text_append(
            document,
            package.as_ref().map_err(Clone::clone)?,
            candidate,
        )?;
        Ok(())
    }
}
