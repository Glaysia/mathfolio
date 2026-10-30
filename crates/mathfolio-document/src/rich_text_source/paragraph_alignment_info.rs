//! Derived read-only metadata from an already admitted immutable source/index.
//! Not serde, not a mutation/authority receipt; construction stays private.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParagraphAlignmentInfo {
    alignment: Option<ParagraphAlignment>,
    scalar_len: usize,
}
impl ParagraphAlignmentInfo {
    pub fn alignment(self) -> Option<ParagraphAlignment> {
        self.alignment
    }
    pub fn scalar_len(self) -> usize {
        self.scalar_len
    }
}

pub(in crate::rich_text_source) fn source_info(
    source: &str,
    index: &SourceIndex,
    generated_ltr: bool,
) -> Result<ParagraphAlignmentInfo> {
    if index.paragraph_xml.len() != 1 {
        return Err("Alignment requires exactly one direct Paragraph".into());
    }
    let paragraph = &index.paragraph_xml[0];
    // Existing admitted grammar permits direct Paragraph children only (no
    // comments/CDATA/PI/nonempty text between root and first child). XML prelude
    // and BOM may precede the root. A raw '<' cannot occur in an XML attr value.
    let prefix = source
        .get(..paragraph.range.start)
        .ok_or("Stale Paragraph prefix")?;
    let root_start = prefix.rfind('<').ok_or("Missing root opening")?;
    let root_end = tag_end(source.as_bytes(), root_start)?;
    if root_end != paragraph.range.start {
        return Err("Unsupported root/Paragraph source gap".into());
    }
    let root = source.get(root_start..root_end).ok_or("Stale root range")?;
    let para = source
        .get(paragraph.range.start..paragraph.opening_end)
        .ok_or("Stale Paragraph opening")?;
    let mut alignment = None;
    let mut ltr = false;
    for opening in [root, para] {
        let attrs = raw_attributes(opening)?;
        for attr in &attrs {
            match attr.name {
                "FlowDirection" => {
                    if attr.value != "LeftToRight" {
                        return Err("RTL/unknown flow unsupported".into());
                    }
                    ltr = true;
                }
                "TextAlignment" => alignment = Some(ParagraphAlignment::parse(attr.value)?),
                _ => {}
            }
        }
    }
    if !ltr && !generated_ltr {
        return Err("Imported alignment requires explicit LTR source".into());
    }
    let run = index.runs.last().ok_or("Missing direct Run")?;
    if run.paragraph != 0 {
        return Err("Stale single-Paragraph Run index".into());
    }
    let scalar_len = run
        .scalar_start
        .checked_add(
            run.boundaries
                .len()
                .checked_sub(1)
                .ok_or("Missing scalar boundary")?,
        )
        .ok_or("Scalar count overflow")?;
    Ok(ParagraphAlignmentInfo {
        alignment,
        scalar_len,
    })
}

impl ValidatedRichText {
    /// O(root-opening + Paragraph-opening) metadata. No XML/ZIP parse, source
    /// clone, text scan or native catalog/default inference. Cache in Editor.
    pub fn paragraph_alignment_info(&self) -> Result<ParagraphAlignmentInfo> {
        if self.wire.paragraph_origins.is_some() {
            return Err("Mapped Paragraph alignment unsupported".into());
        }
        source_info(self.xaml(), &self.index, false)
    }
}

#[cfg(test)]
#[path = "paragraph_alignment_info_tests.rs"]
mod tests;
