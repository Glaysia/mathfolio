//! Candidate: one direct Paragraph only. Coordinates are source Unicode scalars.
//! Command/admission helpers may parse; do not call them in an idle toolbar loop.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParagraphAlignment {
    Left,
    Center,
    Right,
    Justify,
}
impl ParagraphAlignment {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Left => "Left",
            Self::Center => "Center",
            Self::Right => "Right",
            Self::Justify => "Justify",
        }
    }
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "Left" => Ok(Self::Left),
            "Center" => Ok(Self::Center),
            "Right" => Ok(Self::Right),
            "Justify" => Ok(Self::Justify),
            _ => Err("Unsupported Paragraph.TextAlignment".into()),
        }
    }
}

pub(super) fn check_single(
    source: &str,
    index: &SourceIndex,
    selection: TextSelection,
    generated_ltr: bool,
) -> Result<Option<ParagraphAlignment>> {
    if index.paragraph_xml.len() != 1 || selection.paragraph != 0 {
        return Err("Alignment requires exactly one direct Paragraph".into());
    }
    let length = index.paragraphs[0].chars().count();
    if selection.anchor > length || selection.focus > length {
        return Err("Invalid alignment selection scalar range".into());
    }
    let doc = parse_xml(source)?;
    let root = doc.root_element();
    let paragraph = root
        .children()
        .find(Node::is_element)
        .ok_or("Missing Paragraph")?;
    if !paragraph.has_tag_name((XAML, "Paragraph"))
        || paragraph.range() != index.paragraph_xml[0].range
    {
        return Err("Stale alignment source index".into());
    }
    let mut effective = None;
    let mut explicit_ltr = false;
    for node in [root, paragraph] {
        if let Some(direction) = node.attribute("FlowDirection") {
            if direction != "LeftToRight" {
                return Err("RTL/unknown paragraph flow is unsupported".into());
            }
            explicit_ltr = true;
        }
        if let Some(value) = node.attribute("TextAlignment") {
            effective = Some(ParagraphAlignment::parse(value)?);
        }
    }
    if !explicit_ltr && !generated_ltr {
        return Err("Imported paragraph alignment requires explicit LTR source".into());
    }
    // Missing imported source remains unknown; no catalog/default inference.
    Ok(effective)
}

fn paragraph_opening<'a>(source: &'a str, index: &SourceIndex) -> Result<&'a str> {
    let paragraph = index.paragraph_xml.first().ok_or("Missing Paragraph")?;
    source
        .get(paragraph.range.start..paragraph.opening_end)
        .ok_or("Stale Paragraph range".into())
}

fn strip_alignment(opening: &str) -> Result<(String, bool)> {
    let attrs = raw_attributes(opening)?;
    let mut out = opening.to_owned();
    let mut found = false;
    for attr in attrs
        .iter()
        .rev()
        .filter(|attr| attr.name == "TextAlignment")
    {
        if found {
            return Err("Duplicate Paragraph.TextAlignment".into());
        }
        ParagraphAlignment::parse(attr.value)?; // canonical lexical subset
        out.replace_range(attr.full_range.clone(), "");
        found = true;
    }
    Ok((out, found))
}

fn masked_skeleton(
    source: &str,
    index: &SourceIndex,
    generated_ltr: bool,
) -> Result<(String, bool)> {
    check_single(
        source,
        index,
        TextSelection {
            paragraph: 0,
            anchor: 0,
            focus: 0,
        },
        generated_ltr,
    )?;
    let paragraph = &index.paragraph_xml[0];
    let (open, present) = strip_alignment(paragraph_opening(source, index)?)?;
    // Build in original coordinates: never splice source then reuse old ranges.
    let mut out = String::new();
    out.push_str(&source[..paragraph.range.start]);
    out.push_str(&open);
    let mut last = paragraph.opening_end;
    for run in &index.runs {
        if run.range.start < last || run.range.end > paragraph.closing_start {
            return Err("Invalid alignment Run range".into());
        }
        out.push_str(&source[last..run.range.start]);
        last = run.range.end;
    }
    out.push_str(&source[last..]);
    Ok((out, present))
}

/// Preserve existing no-change/wide source behavior first. Only the differing
/// single-Paragraph alignment case receives the additional closed permission.
pub(super) fn same_block_skeleton(
    old: &str,
    old_index: &SourceIndex,
    new: &str,
    new_index: &SourceIndex,
) -> Result<bool> {
    if outside_runs(old, old_index) == outside_runs(new, new_index) {
        return Ok(true);
    }
    let (old_masked, old_present) = masked_skeleton(old, old_index, false)?;
    let (new_masked, new_present) = masked_skeleton(new, new_index, false)?;
    if old_present && !new_present {
        return Err("Original explicit Paragraph.TextAlignment removed".into());
    }
    Ok(old_masked == new_masked)
}

/// Authored scaffold permission is local to its only Paragraph, not seed/root.
pub(super) fn authored_block_skeleton(source: &str, index: &SourceIndex) -> Result<String> {
    if index.paragraph_xml.len() == 1 {
        return masked_skeleton(source, index, true).map(|value| value.0);
    }
    // Preserve older multiple-Paragraph generated sources. They gain no new
    // alignment permission, so split-after-alignment fails atomically for now.
    Ok(outside_runs(source, index))
}

pub(super) fn changed_source(
    source: &str,
    index: &SourceIndex,
    selection: TextSelection,
    desired: ParagraphAlignment,
    generated_ltr: bool,
) -> Result<Option<String>> {
    if check_single(source, index, selection, generated_ltr)? == Some(desired) {
        return Ok(None);
    }
    let paragraph = &index.paragraph_xml[0];
    let opening = paragraph_opening(source, index)?;
    let attrs = raw_attributes(opening)?;
    let mut next_opening = opening.to_owned();
    if let Some(attr) = attrs.iter().find(|attr| attr.name == "TextAlignment") {
        ParagraphAlignment::parse(attr.value)?;
        next_opening.replace_range(attr.value_range.clone(), desired.as_str());
    } else {
        // Insert before original trailing whitespace, preserving its bytes.
        let mut at = opening.len() - 1;
        while at > 0 && opening.as_bytes()[at - 1].is_ascii_whitespace() {
            at -= 1;
        }
        next_opening.insert_str(at, &format!(" TextAlignment=\"{}\"", desired.as_str()));
    }
    let extra = next_opening.len().saturating_sub(opening.len());
    if source
        .len()
        .checked_add(extra)
        .is_none_or(|size| size > MAX_SOURCE_BYTES)
    {
        return Err("Aligned source budget exceeded".into());
    }
    let mut next = source.to_owned();
    next.replace_range(paragraph.range.start..paragraph.opening_end, &next_opening);
    Ok(Some(next))
}

impl ValidatedRichText {
    /// One direct Paragraph; a collapsed selection changes the current block.
    /// Plain text, Run affinity coordinates and native authority stay unchanged.
    pub fn set_paragraph_alignment(
        &self,
        selection: TextSelection,
        alignment: ParagraphAlignment,
    ) -> Result<Option<TextPatch>> {
        if self.wire.paragraph_origins.is_some() {
            return Err("Mapped/split Paragraph alignment is not supported".into());
        }
        let Some(current) = changed_source(self.xaml(), &self.index, selection, alignment, false)?
        else {
            return Ok(None);
        };
        let mut wire = self.wire.as_ref().clone();
        wire.current_xaml = current.into();
        wire.visual_fingerprint = std::sync::OnceLock::new();
        // Existing admission checks the full XML, plain mirror, source/provenance
        // budgets, exact native anchor, keys and unrelated source metadata.
        let next = Self::from_wire(wire, self.authority.clone(), self.plain_text())?;
        Ok(Some(TextPatch {
            source: next,
            selection,
        }))
    }
}

#[cfg(test)]
#[path = "paragraph_alignment_tests.rs"]
mod tests;

#[path = "paragraph_alignment_info.rs"]
pub(super) mod alignment_info;
pub use alignment_info::ParagraphAlignmentInfo;
