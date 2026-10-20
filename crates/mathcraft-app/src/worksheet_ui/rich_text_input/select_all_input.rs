//! Native authority variant of the existing exact SelectAll delivery policy.
use super::*;
use crate::worksheet_ui::authored_text_input::{
    SelectAllDelivery as Delivery, select_all_delivery as delivery,
};
use mathcraft_document::rich_text_source::{
    NativeFullSelectionTypingContext, RunCharRange, TypingFlowDirection,
};
use rich_text_edit_map::{RetainedSource, SourceView};

struct SourcePreservingInputProfile {
    language: String,
}
impl SourcePreservingInputProfile {
    fn capture(source: &ValidatedRichText, first: RunCharRange) -> Result<Self, String> {
        // Source already passed native admission. This is command-time derived
        // metadata, not a second source authority and not OS input culture.
        let doc = roxmltree::Document::parse(source.xaml()).map_err(|e| e.to_string())?;
        let node = doc
            .descendants()
            .filter(|n| {
                n.has_tag_name((
                    "http://schemas.microsoft.com/winfx/2006/xaml/presentation",
                    "Run",
                ))
            })
            .nth(first.run_ordinal)
            .ok_or("Missing native insertion Run")?;
        let language = node
            .ancestors()
            .filter(|n| n.is_element())
            .find_map(|n| n.attribute(("http://www.w3.org/XML/1998/namespace", "lang")))
            .ok_or("Unknown native source input language")?;
        let flow = node
            .ancestors()
            .filter(|n| n.is_element())
            .find_map(|n| n.attribute("FlowDirection"))
            .ok_or("Unknown native source input direction")?;
        if flow != "LeftToRight"
            || language.is_empty()
            || language.len() > 64
            || !language
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'-')
        {
            return Err("Unsupported native source-preserving input profile".into());
        }
        Ok(Self {
            language: language.to_owned(),
        })
    }
}
struct Pending {
    owner: u64,
    source: RetainedSource,
    first: RunCharRange,
    length: usize,
    profile: SourcePreservingInputProfile,
    selection: Option<GlobalSelection>,
    ready: Option<String>,
}
#[derive(Default)]
pub(super) struct SelectAllInput {
    pending: Option<Pending>,
    #[cfg(test)]
    profile_reads: usize,
}
impl SelectAllInput {
    pub(super) fn clear(&mut self) {
        self.pending = None;
    }
    fn bound(p: &Pending, owner: u64, source: &ValidatedRichText) -> bool {
        p.owner == owner && source.matches_owner(&p.source)
    }
    fn ready(&self) -> bool {
        self.pending.as_ref().is_some_and(|p| p.ready.is_some())
    }
    fn begin(
        &mut self,
        owner: u64,
        source: &ValidatedRichText,
        before: GlobalSelection,
        events: &[egui::Event],
        focus: bool,
        composing: bool,
    ) -> bool {
        if !focus || composing {
            self.clear();
            return false;
        }
        match delivery(events) {
            Delivery::SelectAll => {
                self.clear();
                let mut runs = source.run_char_ranges();
                let Some(first) = runs.find(|r| r.global_start < r.global_end) else {
                    return true;
                };
                if first.paragraph != 0 || runs.any(|r| r.paragraph != 0) {
                    return true;
                }
                #[cfg(test)]
                {
                    self.profile_reads += 1;
                }
                let Ok(profile) = SourcePreservingInputProfile::capture(source, first) else {
                    return true;
                };
                self.pending = Some(Pending {
                    owner,
                    source: source.retained_owner(),
                    first,
                    length: source.plain_text().chars().count(),
                    profile,
                    selection: None,
                    ready: None,
                });
                true
            }
            Delivery::Text(text) => {
                if text.len() <= mathcraft_document::rich_text_source::MAX_SOURCE_BYTES
                    && self.pending.as_ref().is_some_and(|p| {
                        Self::bound(p, owner, source)
                            && p.selection == Some(before)
                            && p.ready.is_none()
                    })
                {
                    self.pending.as_mut().unwrap().ready = Some(text.to_owned());
                } else {
                    self.clear();
                }
                false
            }
            Delivery::Benign => {
                if self.pending.as_ref().is_some_and(|p| {
                    !Self::bound(p, owner, source)
                        || p.selection.is_some_and(|s| s != before)
                        || p.ready.is_some()
                }) {
                    self.clear();
                }
                false
            }
            Delivery::Retire => {
                self.clear();
                false
            }
        }
    }
    fn observe(&mut self, owner: u64, source: &ValidatedRichText, after: Option<GlobalSelection>) {
        let Some(p) = &self.pending else {
            return;
        };
        if p.selection.is_some() {
            return;
        }
        if !Self::bound(p, owner, source)
            || after.is_none_or(|s| s.anchor.min(s.focus) != 0 || s.anchor.max(s.focus) != p.length)
        {
            self.clear();
            return;
        }
        self.pending.as_mut().unwrap().selection = after;
    }
    pub(super) fn preview_owner(
        &self,
        source: &ValidatedRichText,
        before: GlobalSelection,
        buffer: &str,
    ) -> Option<usize> {
        let p = self.pending.as_ref()?;
        (source.matches_owner(&p.source)
            && p.selection == Some(before)
            && p.ready.as_deref() == Some(buffer))
        .then_some(p.first.run_ordinal)
    }
}

impl RichEditor {
    pub(in crate::worksheet_ui) fn retire_select_all(&mut self) {
        self.select_all.clear();
    }
    pub(in crate::worksheet_ui) fn has_pending_select_all(&self) -> bool {
        self.select_all.pending.is_some()
    }
    pub(in crate::worksheet_ui) fn has_select_all_replacement(&self) -> bool {
        self.select_all.ready()
    }
    #[cfg(test)]
    pub(in crate::worksheet_ui) fn select_all_profile_reads(&self) -> usize {
        self.select_all.profile_reads
    }
    pub(in crate::worksheet_ui) fn begin_select_all_delivery(
        &mut self,
        owner: u64,
        before: GlobalSelection,
        events: &[egui::Event],
        focus: bool,
        composing: bool,
    ) {
        if self
            .select_all
            .begin(owner, &self.source, before, events, focus, composing)
        {
            self.affinity.clear();
        }
    }
    pub(in crate::worksheet_ui) fn observe_select_all_selection(
        &mut self,
        owner: u64,
        after: Option<GlobalSelection>,
    ) {
        self.select_all.observe(owner, &self.source, after);
    }
    pub(in crate::worksheet_ui) fn commit_select_all(
        &mut self,
        owner: u64,
        text: &str,
        before: GlobalSelection,
        after: GlobalSelection,
        headroom: usize,
    ) -> Result<Option<mathcraft_document::rich_text_source::RichTextWire>, String> {
        let proof = self
            .select_all
            .pending
            .take()
            .filter(|p| p.ready.is_some())
            .ok_or("Missing native SelectAll Text delivery")?;
        let inserted = proof.ready.as_deref().unwrap();
        let caret = inserted.chars().count();
        if !SelectAllInput::bound(&proof, owner, &self.source)
            || proof.selection != Some(before)
            || text != inserted
            || after
                != (GlobalSelection {
                    anchor: caret,
                    focus: caret,
                })
        {
            return Err("Stale native SelectAll Text replacement".into());
        }
        let patch = self.source.replace_select_all_text(
            TextSelection {
                paragraph: 0,
                anchor: before.anchor,
                focus: before.focus,
            },
            text,
            NativeFullSelectionTypingContext {
                input_language: &proof.profile.language,
                input_flow_direction: TypingFlowDirection::LeftToRight,
                has_retained_springload: false,
            },
        )?;
        let Some(patch) = patch else {
            let mut affinity = self.affinity.clone();
            affinity.remember_run(&self.source, proof.first.run_ordinal)?;
            self.affinity = affinity;
            return Ok(None);
        };
        // Match the exporter admission before publishing a same-plain source.
        // A previous format/split can invalidate the original raw-body witness;
        // it must then satisfy the existing strict format proof, never a catch
        // that silently reclassifies an unsupported delta as content.
        if patch.source.plain_text() == self.authority.original_plain_text()
            && !patch.source.has_original_raw_body_only_delta()
        {
            patch.source.prove_format_only()?;
        }
        let insertion_owner = patch
            .insertion_owner
            .ok_or("Missing native SelectAll owner")?;
        let mut next = Self::new(
            patch.source,
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
        next.affinity.remember_run(&next.source, insertion_owner)?;
        let wire = next.source.wire().clone();
        *self = next;
        Ok(Some(wire))
    }
}
