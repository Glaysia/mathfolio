//! Source-bound single-Paragraph commands. No idle source/XML/ZIP work.
use super::*;
use mathfolio_document::rich_text_source::{ParagraphAlignment, ParagraphAlignmentInfo};
use rich_text_edit_map::{RetainedSource, SourceView};

#[derive(Default)]
pub(super) struct State {
    // Both runtime contexts keep their own serial/target; B/I/U state unchanged.
    target: Option<(u64, usize, usize, usize)>,
    context: Option<ribbon::ParagraphAlignmentContext>,
    serial: u64,
    // Holding the exact admitted Arc prevents pointer reuse from resurrecting
    // an old target. Clone only on target changes, never on stable idle frames.
    bound: Option<RetainedSource>,
}
impl State {
    pub fn invalidate(&mut self) {
        self.target = None;
        self.context = None;
        self.bound = None;
    }
}

fn cached(app: &WorksheetApp, id: u64) -> Option<(usize, ParagraphAlignmentInfo)> {
    if let Some(Ok(editor)) = app.rich_text.authored_editors.get(&id) {
        return editor
            .paragraph_alignment
            .map(|info| (editor.source.xaml().as_ptr() as usize, info));
    }
    let editor = app.rich_text.editors.get(&id)?.as_ref().ok()?;
    editor
        .paragraph_alignment
        .map(|info| (editor.source.xaml().as_ptr() as usize, info))
}
fn bound_matches(app: &WorksheetApp, id: u64, bound: &RetainedSource) -> bool {
    if let Some(Ok(e)) = app.rich_text.authored_editors.get(&id) {
        return e.source.matches_owner(bound);
    }
    app.rich_text
        .editors
        .get(&id)
        .and_then(|e| e.as_ref().ok())
        .is_some_and(|e| e.source.matches_owner(bound))
}
fn retain(app: &WorksheetApp, id: u64) -> Option<RetainedSource> {
    if let Some(Ok(e)) = app.rich_text.authored_editors.get(&id) {
        return Some(e.source.retained_owner());
    }
    Some(
        app.rich_text
            .editors
            .get(&id)?
            .as_ref()
            .ok()?
            .source
            .retained_owner(),
    )
}

impl WorksheetApp {
    /// Only existing admitted runtime editors are queried here. Ordinary page
    /// paint/preparation populates those; never do native admission from chrome.
    pub(super) fn paragraph_alignment_context(
        &mut self,
    ) -> Option<ribbon::ParagraphAlignmentContext> {
        if self.pending.is_some() || self.text_composition.is_some() {
            self.paragraph_alignment.invalidate();
            return None;
        }
        let Some(id) = self.selected else {
            self.paragraph_alignment.invalidate();
            return None;
        };
        if !self
            .document
            .regions
            .iter()
            .any(|r| r.id == id && matches!(r.content, CanvasContent::Text(_)))
        {
            self.paragraph_alignment.invalidate();
            return None;
        }
        let Some(range) = self.text_cursors.get(&id).copied() else {
            self.paragraph_alignment.invalidate();
            return None;
        };
        let Some((source, info)) = cached(self, id) else {
            self.paragraph_alignment.invalidate();
            return None;
        };
        if range.primary.index.0 > info.scalar_len() || range.secondary.index.0 > info.scalar_len()
        {
            self.paragraph_alignment.invalidate();
            return None;
        }
        let target = (id, source, range.secondary.index.0, range.primary.index.0);
        if self.paragraph_alignment.target == Some(target)
            && self
                .paragraph_alignment
                .bound
                .as_ref()
                .is_some_and(|b| bound_matches(self, id, b))
        {
            return self.paragraph_alignment.context;
        }
        self.paragraph_alignment.serial = self.paragraph_alignment.serial.wrapping_add(1);
        let context = ribbon::ParagraphAlignmentContext {
            selection_key: self.paragraph_alignment.serial,
            current: info.alignment(),
        };
        self.paragraph_alignment.bound = retain(self, id);
        self.paragraph_alignment.target = Some(target);
        self.paragraph_alignment.context = Some(context);
        Some(context)
    }

    pub(super) fn paragraph_alignment_command(
        &mut self,
        ctx: &egui::Context,
        value: ParagraphAlignment,
        key: u64,
    ) -> Result<bool, String> {
        let current = self
            .paragraph_alignment_context()
            .ok_or("No supported Paragraph alignment target")?;
        if current.selection_key != key {
            return Err("Stale Paragraph alignment command".into());
        }
        let id = self.selected.ok_or("Missing Text owner")?;
        // A format command is not the ordinary Text following SelectAll.
        if let Some(Ok(editor)) = self.rich_text.authored_editors.get_mut(&id) {
            editor.retire_select_all();
        }
        let range = *self
            .text_cursors
            .get(&id)
            .ok_or("Missing paragraph cursor")?;
        let region = self
            .document
            .regions
            .iter()
            .find(|r| r.id == id)
            .ok_or("Missing Text region")?;
        enum Next {
            Native(rich_text_input::RichEditor),
            Authored(authored_text_input::AuthoredEditor),
        }
        let next = if let Some(Ok(editor)) = self.rich_text.authored_editors.get(&id) {
            let Some(next) = editor.aligned(range, value)? else {
                return Ok(false);
            };
            self.document
                .validate_authored_text_edit_budget(id, &next.source)?;
            Next::Authored(next)
        } else {
            let editor = self
                .rich_text
                .editors
                .get(&id)
                .ok_or("Missing rich Text editor")?
                .as_ref()
                .map_err(Clone::clone)?;
            let Some(next) = editor.aligned(range, value)? else {
                return Ok(false);
            };
            let mut candidate = region.clone();
            candidate.rich_text = Some(Box::new(next.source.wire().clone()));
            self.document.validate_text_replace_budget(&candidate)?;
            self.rich_text
                .validate_alignment_replacement(&self.document, &candidate)?;
            Next::Native(next)
        };
        // All fallible admission/budget/layout work is done before history and
        // document mutation. Source/plain/cursor changes are one transaction.
        let before = self.snapshot_with_document(DocumentHistory::Region(Box::new(region.clone())));
        let region = self
            .document
            .regions
            .iter_mut()
            .find(|r| r.id == id)
            .unwrap();
        match next {
            Next::Native(editor) => {
                region.rich_text = Some(Box::new(editor.source.wire().clone()));
                self.rich_text.editors.insert(id, Ok(editor));
            }
            Next::Authored(editor) => {
                region.authored_text = Some(Box::new(editor.source.wire().clone()));
                self.rich_text.authored_editors.insert(id, Ok(editor));
            }
        }
        self.remember(before);
        self.mark_dirty();
        self.status.clear();
        self.paragraph_alignment.invalidate();
        self.refresh_rich_text_visual(id);
        self.region_heights.remove(&id);
        self.region_widths.remove(&id);
        self.focus_new = Some(id);
        ctx.request_repaint();
        Ok(true)
    }

    pub(super) fn paragraph_alignment_shortcuts(&mut self, ctx: &egui::Context) {
        // Cheap event/focus guards precede all source context work.
        let keys = ctx.input(|input| {
            input
                .events
                .iter()
                .filter_map(|event| match event {
                    egui::Event::Key {
                        key,
                        pressed: true,
                        modifiers,
                        ..
                    } if modifiers.matches_exact(egui::Modifiers::COMMAND) => match key {
                        egui::Key::L => Some((*key, ParagraphAlignment::Left)),
                        egui::Key::E => Some((*key, ParagraphAlignment::Center)),
                        egui::Key::R => Some((*key, ParagraphAlignment::Right)),
                        egui::Key::J => Some((*key, ParagraphAlignment::Justify)),
                        _ => None,
                    },
                    _ => None,
                })
                .collect::<Vec<_>>()
        });
        if keys.is_empty() {
            return;
        }
        let Some(id) = self.selected else {
            return;
        };
        if !ctx.memory(|m| m.has_focus(egui::Id::new(("worksheet-text", id))))
            || self.text_composition.is_some()
        {
            return;
        }
        for (key, value) in keys {
            let Some(context) = self.paragraph_alignment_context() else {
                continue;
            };
            if !ctx.input_mut(|i| consume_exact_key(i, egui::Modifiers::COMMAND, key)) {
                continue;
            }
            if let Err(error) = self.paragraph_alignment_command(ctx, value, context.selection_key)
            {
                self.status = format!("문단 정렬: {error}");
            }
        }
    }
}
