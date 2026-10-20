//! Included as child of authored.rs, preserving its private admission boundary.
use super::super::paragraph_alignment::{ParagraphAlignment, changed_source, check_single};
use super::*;

impl AuthoredRichText {
    /// Local block source only; does not mutate seed/current creation defaults.
    pub fn set_paragraph_alignment(
        &self,
        selection: TextSelection,
        alignment: ParagraphAlignment,
    ) -> Result<Option<AuthoredTextPatch>> {
        // Generated seed authority fixes Root TextAlignment=Left and P_OPEN.
        // Restore only our canonical local override; this is an authored policy,
        // not permission to remove original native Paragraph metadata.
        let current = if alignment == ParagraphAlignment::Left {
            let effective = check_single(self.xaml(), &self.index, selection, true)?;
            let paragraph = &self.index.paragraph_xml[0];
            let opening = &self.xaml()[paragraph.range.start..paragraph.opening_end];
            if opening == P_OPEN {
                return Ok(None);
            }
            let current = effective.ok_or("Missing generated alignment authority")?;
            let expected = format!("<Paragraph TextAlignment=\"{}\">", current.as_str());
            if opening != expected {
                return Err("Only canonical authored Paragraph alignment can restore Left".into());
            }
            let mut source = self.xaml().to_owned();
            source.replace_range(paragraph.range.start..paragraph.opening_end, P_OPEN);
            source
        } else {
            let Some(current) =
                changed_source(self.xaml(), &self.index, selection, alignment, true)?
            else {
                return Ok(None);
            };
            current
        };
        let mut wire = self.wire.as_ref().clone();
        wire.current_xaml = current.into();
        wire.derived = Default::default();
        let parsed = Arc::new(parse_source(&wire.current_xaml, &self.seed.keys)?);
        let source = Self::prepared(wire, self.seed.clone(), parsed)?;
        Ok(Some(AuthoredTextPatch {
            source,
            selection,
            following_run: None,
        }))
    }
}
