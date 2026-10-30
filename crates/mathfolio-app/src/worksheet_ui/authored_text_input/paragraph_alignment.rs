use super::*;
use mathfolio_document::rich_text_source::ParagraphAlignment;
impl AuthoredEditor {
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
        let mut next = Self::new(patch.source)?;
        if !self.affinity.is_backspace_continuation()
            && let Some(ordinal) = self.affinity.owner(&self.source)
        {
            next.affinity.remember_run(&next.source, ordinal)?;
        }
        Ok(Some(next))
    }
}
