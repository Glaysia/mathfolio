//! Validated current Paragraph -> immutable original Paragraph mapping.
//! Routing map is untrusted. Exact wrappers/gaps and the old metadata proof
//! are checked after folding cloned wrappers. No merge/LineBreak/empty split.
use super::*;
use serde::de::{Error as _, SeqAccess, Visitor};

pub(super) const MAX_PARAGRAPHS: usize = 1024;

pub(super) fn deserialize_origins<'de, D>(
    deserializer: D,
) -> std::result::Result<Option<Arc<[u16]>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct Origins;
    impl<'de> Visitor<'de> for Origins {
        type Value = Option<Arc<[u16]>>;
        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("a nonempty bounded paragraph-origin array, not null")
        }
        fn visit_seq<A: SeqAccess<'de>>(
            self,
            mut seq: A,
        ) -> std::result::Result<Self::Value, A::Error> {
            if seq.size_hint().is_some_and(|n| n > MAX_PARAGRAPHS) {
                return Err(A::Error::custom("Paragraph origin budget exceeded"));
            }
            let mut values = Vec::with_capacity(seq.size_hint().unwrap_or(0).min(MAX_PARAGRAPHS));
            while let Some(value) = seq.next_element::<u16>()? {
                if values.len() == MAX_PARAGRAPHS {
                    return Err(A::Error::custom("Paragraph origin budget exceeded"));
                }
                values.push(value);
            }
            if values.is_empty() {
                return Err(A::Error::custom("Empty origin map"));
            }
            Ok(Some(values.into()))
        }
    }
    deserializer.deserialize_seq(Origins)
}

/// Additional compact JSON field bytes. None keeps the old cost exactly.
pub(super) fn additional_wire_bytes(origins: Option<&[u16]>) -> usize {
    let Some(origins) = origins else {
        return 0;
    };
    b",\"paragraph_origins\":[]".len()
        + origins.len().saturating_sub(1)
        + origins
            .iter()
            .map(|n| match n {
                0..=9 => 1,
                10..=99 => 2,
                100..=999 => 3,
                1000..=9999 => 4,
                _ => 5,
            })
            .sum::<usize>()
}

#[derive(Clone, Debug)]
pub struct ParagraphSplitPatch {
    pub patch: TextPatch,
    pub following_run: usize,
}

pub(super) fn checked_map(origins: &[u16], original: usize, current: usize) -> Result<Vec<usize>> {
    if original == 0
        || original > MAX_PARAGRAPHS
        || current > MAX_PARAGRAPHS
        || current <= original
        || origins.len() != current
        || origins.first().copied() != Some(0)
        || origins.last().copied().map(usize::from) != Some(original - 1)
        || origins
            .windows(2)
            .any(|w| w[1] < w[0] || u32::from(w[1]) > u32::from(w[0]) + 1)
    {
        return Err("Invalid/uncovered/reordered/redundant Paragraph origin map".into());
    }
    let mut counts = vec![0usize; original];
    for origin in origins {
        let count = counts
            .get_mut(usize::from(*origin))
            .ok_or("Unknown original Paragraph")?;
        *count += 1;
    }
    if counts.contains(&0) {
        return Err("Original Paragraph omitted".into());
    }
    Ok(counts)
}

/// Called ONLY for present provenance. It does not authorize source parsing,
/// origin/version binding or format-only caching; those remain separate.
pub(super) fn prove_mapped_metadata(source: &ValidatedRichText) -> Result<()> {
    let origins = source
        .wire
        .paragraph_origins
        .as_deref()
        .ok_or("Missing origins")?;
    let original_text = &source.authority.original_xaml;
    let original_index = &source.authority.original_index;
    let counts = checked_map(
        origins,
        original_index.paragraphs.len(),
        source.index.paragraphs.len(),
    )?;
    let old = &original_index.paragraph_xml;
    let current = &source.index.paragraph_xml;
    if current.len() != origins.len() || old.len() != counts.len() {
        return Err("Paragraph span/map mismatch".into());
    }
    if source.xaml()[..current[0].range.start] != original_text[..old[0].range.start]
        || source.xaml()[current.last().unwrap().range.end..]
            != original_text[old.last().unwrap().range.end..]
    {
        return Err("Original root/prefix/suffix changed".into());
    }
    for (i, p) in current.iter().enumerate() {
        let origin = usize::from(origins[i]);
        if source.xaml()[p.range.start..p.opening_end]
            != original_text[old[origin].range.start..old[origin].opening_end]
            || source.xaml()[p.closing_start..p.range.end]
                != original_text[old[origin].closing_start..old[origin].range.end]
        {
            return Err("Mapped Paragraph wrapper attributes/QName changed".into());
        }
        if counts[origin] > 1 && source.index.paragraphs[i].is_empty() {
            return Err("Empty split-derived Paragraph unsupported".into());
        }
        if i == 0 {
            continue;
        }
        let gap = &source.xaml()[current[i - 1].range.end..p.range.start];
        if origins[i - 1] == origins[i] {
            if !gap.is_empty() {
                return Err("Extra XML/gap between clones".into());
            }
            let right_first = source.index.runs.partition_point(|r| r.paragraph < i);
            let left_last = right_first.checked_sub(1).ok_or("Missing left Run")?;
            let left = source.index.runs.get(left_last).ok_or("Missing left Run")?;
            let right = source
                .index
                .runs
                .get(right_first)
                .ok_or("Missing right Run")?;
            if left.paragraph != i - 1
                || right.paragraph != i
                || non_format_opening(&source.xaml()[left.range.start..left.opening_end])?
                    != non_format_opening(&source.xaml()[right.range.start..right.opening_end])?
                || source.xaml()[left.closing_start..left.range.end]
                    != source.xaml()[right.closing_start..right.range.end]
            {
                return Err("Split boundary Run ownership metadata changed".into());
            }
        } else {
            let previous = origin.checked_sub(1).ok_or("Unordered original boundary")?;
            if gap != &original_text[old[previous].range.end..old[origin].range.start] {
                return Err("Original inter-Paragraph gap changed".into());
            }
        }
    }
    // Fold exactly once, O(source) temporary memory. Never enumerate possible
    // origin assignments or try O(paragraphs * full-source) clones.
    let mut folded = String::with_capacity(source.xaml().len());
    folded.push_str(&original_text[..old[0].range.start]);
    let mut folded_runs = Vec::with_capacity(source.index.runs.len());
    let mut run_cursor = 0;
    let mut cursor = 0;
    for (origin, count) in counts.iter().enumerate() {
        folded.push_str(&original_text[old[origin].range.start..old[origin].opening_end]);
        for (offset, p) in current[cursor..cursor + *count].iter().enumerate() {
            let current_paragraph = cursor + offset;
            let body_start = folded.len();
            let first_run = run_cursor;
            while let Some(run) = source
                .index
                .runs
                .get(run_cursor)
                .filter(|r| r.paragraph == current_paragraph)
            {
                if run.range.start < p.opening_end
                    || run.range.end > p.closing_start
                    || run.range.start > run.range.end
                {
                    return Err("Run outside cached Paragraph body".into());
                }
                let start = body_start + (run.range.start - p.opening_end);
                let end = body_start + (run.range.end - p.opening_end);
                folded_runs.push(start..end);
                run_cursor += 1;
            }
            if first_run == run_cursor {
                return Err("Missing cached Paragraph Runs".into());
            }
            // Copy ALL body bytes, including admitted empty CDATA/non-Run gaps.
            folded.push_str(&source.xaml()[p.opening_end..p.closing_start]);
        }
        folded.push_str(&original_text[old[origin].closing_start..old[origin].range.end]);
        cursor += *count;
        if origin + 1 < old.len() {
            folded.push_str(&original_text[old[origin].range.end..old[origin + 1].range.start]);
        }
    }
    folded.push_str(&original_text[old.last().unwrap().range.end..]);
    if run_cursor != source.index.runs.len() {
        return Err("Unmapped cached Run".into());
    }
    if outside_run_ranges(&folded, folded_runs) != outside_runs(original_text, original_index)
        || remapped_metadata_groups(source, origins)?
            != metadata_groups(original_text, original_index)?
    {
        return Err("Folded original metadata/block/key proof failed".into());
    }
    Ok(())
}

/// Same old metadata key function, with only the validated Paragraph index
/// remapped to its original origin. Adjacent equality is checked AFTER remap;
/// distinct original paragraph groups therefore never coalesce.
fn remapped_metadata_groups(
    source: &ValidatedRichText,
    origins: &[u16],
) -> Result<Vec<(usize, String, String)>> {
    let mut groups = Vec::new();
    for run in &source.index.runs {
        let origin = usize::from(*origins.get(run.paragraph).ok_or("Missing Run origin")?);
        let group = (
            origin,
            non_format_opening(&source.xaml()[run.range.start..run.opening_end])?,
            source.xaml()[run.closing_start..run.range.end].to_owned(),
        );
        if groups.last() != Some(&group) {
            groups.push(group);
        }
    }
    Ok(groups)
}

impl ValidatedRichText {
    pub fn split_paragraph_at_caret(&self, caret: TextSelection) -> Result<ParagraphSplitPatch> {
        if caret.anchor != caret.focus || caret.paragraph >= self.index.paragraphs.len() {
            return Err("Expected supported collapsed Paragraph caret".into());
        }
        let (ordinal, run) = self
            .index
            .runs
            .iter()
            .enumerate()
            .find(|(_, run)| {
                run.paragraph == caret.paragraph
                    && run.scalar_start < caret.focus
                    && caret.focus < run.scalar_start + run.boundaries.len() - 1
            })
            .ok_or("Caret must be strictly inside one nonempty Run")?;
        let paragraph = &self.index.paragraph_xml[caret.paragraph];
        let run_opening = &self.xaml()[run.range.start..run.opening_end];
        let run_closing = &self.xaml()[run.closing_start..run.range.end];
        let added = run_opening
            .len()
            .checked_add(run_closing.len())
            .and_then(|n| n.checked_add(paragraph.opening_end - paragraph.range.start))
            .and_then(|n| n.checked_add(paragraph.range.end - paragraph.closing_start))
            .ok_or("Split size overflow")?;
        let capacity = self
            .xaml()
            .len()
            .checked_add(added)
            .filter(|n| *n <= MAX_SOURCE_BYTES)
            .ok_or("Split source budget exceeded")?;
        let mut origins: Vec<u16> = self
            .wire
            .paragraph_origins
            .as_deref()
            .map(|v| v.to_vec())
            .unwrap_or_else(|| {
                (0..self.index.paragraphs.len())
                    .map(|i| u16::try_from(i).expect("validated paragraph budget"))
                    .collect()
            });
        if origins.len() >= MAX_PARAGRAPHS {
            return Err("Paragraph origin budget exceeded".into());
        }
        origins.insert(caret.paragraph + 1, origins[caret.paragraph]);
        capacity
            .checked_add(additional_wire_bytes(Some(&origins)))
            .filter(|n| *n <= MAX_SOURCE_BYTES)
            .ok_or("Split source plus provenance budget exceeded")?;
        let cut = run.opening_end + run.boundaries[caret.focus - run.scalar_start];
        let mut current = String::with_capacity(capacity);
        current.push_str(&self.xaml()[..cut]);
        current.push_str(run_closing);
        current.push_str(&self.xaml()[paragraph.closing_start..paragraph.range.end]);
        current.push_str(&self.xaml()[paragraph.range.start..paragraph.opening_end]);
        current.push_str(run_opening);
        current.push_str(&self.xaml()[cut..]);
        let mut wire = self.wire.as_ref().clone();
        wire.current_xaml = current.into();
        wire.paragraph_origins = Some(origins.into());
        wire.visual_fingerprint = std::sync::OnceLock::new();
        let parsed = Arc::new(parse_source(&wire.current_xaml, &self.authority.keys)?);
        let source = Self::from_prepared(wire, self.authority.clone(), parsed)?;
        Ok(ParagraphSplitPatch {
            patch: TextPatch {
                source,
                selection: TextSelection {
                    paragraph: caret.paragraph + 1,
                    anchor: 0,
                    focus: 0,
                },
            },
            following_run: ordinal + 1,
        })
    }
}

#[cfg(test)]
#[path = "paragraph_origin_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "paragraph_origin_oracle_tests.rs"]
mod oracle_tests;
