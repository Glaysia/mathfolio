//! Included below rich_text_input so native editor authority/affinity stay private.
use super::*;
use mathcraft_document::rich_text_source::ParagraphAlignment;
impl RichEditor {
    pub fn aligned(
        &self,
        range: egui::text::CCursorRange,
        value: ParagraphAlignment,
    ) -> Result<Option<Self>, String> {
        let info = self
            .paragraph_alignment
            .ok_or("Paragraph alignment unsupported")?;
        let selection = TextSelection {
            paragraph: 0,
            anchor: range.secondary.index.0,
            focus: range.primary.index.0,
        };
        if selection.anchor > info.scalar_len() || selection.focus > info.scalar_len() {
            return Err("Stale paragraph cursor".into());
        }
        let Some(patch) = self.source.set_paragraph_alignment(selection, value)? else {
            return Ok(None);
        };
        let mut next = Self::new(
            patch.source,
            self.authority.clone(),
            self.base_style.clone(),
        )?;
        if let Some(ordinal) = self.affinity.owner(&self.source) {
            next.affinity.remember_run(&next.source, ordinal)?;
        }
        Ok(Some(next))
    }
}
impl State {
    pub fn validate_alignment_replacement(
        &self,
        document: &CanvasDocument,
        candidate: &CanvasRegion,
    ) -> Result<(), String> {
        let package = self
            .package
            .as_ref()
            .ok_or("Missing native Text authority")?
            .as_ref()
            .map_err(Clone::clone)?;
        mathcraft_document::export::validate_rich_text_replacement(document, package, candidate)
            .map(|_| ())
    }
}
