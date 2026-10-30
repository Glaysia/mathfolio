//! Bounded source-owned edit mapping; not native WPF affinity reconstruction.
//! Product import becomes mathfolio_document::rich_text_source::{...}.
use mathfolio_document::rich_text_source::{
    AuthoredRichText, AuthoredTextPatch, RunCharRange, TextPatch, TextSelection, ValidatedRichText,
};

/// Borrowed index access shared by two distinct provenance types. No conversion
/// to a native anchor, XML reparse or per-call iterator allocation occurs here.
pub trait SourceView {
    fn plain_text(&self) -> &str;
    fn run_char_ranges(&self) -> impl Iterator<Item = RunCharRange> + '_;
    fn retained_owner(&self) -> RetainedSource;
    fn matches_owner(&self, source: &RetainedSource) -> bool;
}

#[derive(Clone, Debug)]
pub enum RetainedSource {
    Native(ValidatedRichText),
    Authored(AuthoredRichText),
}
impl SourceView for ValidatedRichText {
    fn plain_text(&self) -> &str {
        self.plain_text()
    }
    fn run_char_ranges(&self) -> impl Iterator<Item = RunCharRange> + '_ {
        self.run_char_ranges()
    }
    fn retained_owner(&self) -> RetainedSource {
        RetainedSource::Native(self.clone())
    }
    fn matches_owner(&self, source: &RetainedSource) -> bool {
        matches!(source, RetainedSource::Native(bound) if
            bound.xaml().as_ptr() == self.xaml().as_ptr()
            && bound.xaml().len() == self.xaml().len()
            && bound.wire().origin() == self.wire().origin())
    }
}
impl SourceView for AuthoredRichText {
    fn plain_text(&self) -> &str {
        self.plain_text()
    }
    fn run_char_ranges(&self) -> impl Iterator<Item = RunCharRange> + '_ {
        self.run_char_ranges()
    }
    fn retained_owner(&self) -> RetainedSource {
        RetainedSource::Authored(self.clone())
    }
    fn matches_owner(&self, source: &RetainedSource) -> bool {
        matches!(source, RetainedSource::Authored(bound) if
            bound.xaml().as_ptr() == self.xaml().as_ptr()
            && bound.xaml().len() == self.xaml().len()
            && bound.seed().fingerprint() == self.seed().fingerprint())
    }
}
use std::ops::Range;

type Result<T> = std::result::Result<T, String>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlobalSelection {
    pub anchor: usize,
    pub focus: usize,
}
impl GlobalSelection {
    fn range(self) -> Range<usize> {
        self.anchor.min(self.focus)..self.anchor.max(self.focus)
    }
    fn collapsed(self) -> bool {
        self.anchor == self.focus
    }
}

/// Runtime only, shallow Arc/index owner. Do not serialize into source/history.
/// Explicit pointer/navigation/owner/history changes clear it. Save does not.
#[derive(Clone, Debug, Default)]
pub struct RunAffinity {
    binding: Option<(RetainedSource, usize)>,
    // Runtime-only use restriction, copied by existing affinity/history snapshots.
    backspace_only: bool,
}
impl RunAffinity {
    pub fn clear(&mut self) {
        self.binding = None;
        self.backspace_only = false;
    }
    /// Owner-history/IME checkpoint restoration is explicit and source-bound.
    /// Plain caret equality is not sufficient to resurrect a stale owner.
    pub fn restore_if_bound(&mut self, source: &impl SourceView, snapshot: &Self) -> bool {
        if snapshot.owner(source).is_some() {
            *self = snapshot.clone();
            true
        } else {
            self.clear();
            false
        }
    }
    pub fn owner(&self, source: &impl SourceView) -> Option<usize> {
        self.binding.as_ref().and_then(|(bound, ordinal)| {
            let same = source.matches_owner(bound);
            same.then_some(*ordinal)
        })
    }
    pub fn remember_run(&mut self, source: &impl SourceView, ordinal: usize) -> Result<()> {
        if !source
            .run_char_ranges()
            .any(|run| run.run_ordinal == ordinal)
        {
            return Err("Unknown affinity Run".into());
        }
        self.binding = Some((source.retained_owner(), ordinal));
        self.backspace_only = false;
        Ok(())
    }
    pub fn remember_backspace_run(
        &mut self,
        source: &AuthoredRichText,
        ordinal: usize,
    ) -> Result<()> {
        self.remember_run(source, ordinal)?;
        self.backspace_only = true;
        Ok(())
    }
    pub fn is_backspace_continuation(&self) -> bool {
        self.backspace_only && self.binding.is_some()
    }
    pub fn retire_backspace_continuation(&mut self) {
        if self.backspace_only {
            self.clear();
        }
    }
    /// After formatting, only an explicit selection contained in one resulting
    /// Run establishes affinity. Multi-Run formatting does not invent an owner.
    pub fn remember_selection(
        &mut self,
        source: &impl SourceView,
        selected: GlobalSelection,
    ) -> Result<()> {
        let range = selected.range();
        if range.is_empty() {
            return Err("An explicit nonempty Run selection is required".into());
        }
        let owner = resolve_owner(source, range, None)?;
        self.remember_run(source, owner.run_ordinal)
    }
}

/// Prepared before touching source/history. Byte range is in OLD exact plain
/// text, not XML; inserted borrows the NEW TextEdit buffer. Caller maps the Run
/// ordinal to its prepared layout style index, never guesses from glyph color.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunEditPlan<'a> {
    /// The original first Run owns single-Run insertion or the retained
    /// prefix of a cross-Run deletion. It is not post-deletion affinity.
    pub run_ordinal: usize,
    pub global_range: Range<usize>,
    pub old_bytes: Range<usize>,
    pub local_range: Range<usize>,
    pub inserted: &'a str,
    pub cross_run_deletion: Option<TextSelection>,
}

#[derive(Clone, Debug)]
pub struct AppliedRunEdit {
    pub patch: TextPatch,
    pub run_ordinal: usize,
    #[cfg_attr(not(test), allow(dead_code))] // Kept for exact mapping contract oracles.
    pub old_global_range: Range<usize>,
    pub old_bytes: Range<usize>,
    /// Actual TextEdit selection; do not collapse a native TextEdit Undo range.
    pub after_selection: GlobalSelection,
}

/// Read-only plan. IME preview may use the plan for a projection only; it must
/// NEVER call apply_committed. Before selection must be the committed/
/// composition-origin selection, not a transient preedit cursor. Planning
/// changes neither affinity nor source/history.
pub fn plan_committed<'a>(
    source: &impl SourceView,
    before: GlobalSelection,
    after: GlobalSelection,
    new_text: &'a str,
    affinity: &RunAffinity,
) -> Result<Option<RunEditPlan<'a>>> {
    let old = source.plain_text();
    let old_len = old.chars().count();
    let new_len = new_text.chars().count();
    if before.anchor > old_len
        || before.focus > old_len
        || after.anchor > new_len
        || after.focus > new_len
    {
        return Err("TextEdit selection is outside exact source/plain bounds".into());
    }
    if old == new_text {
        return Ok(None);
    }
    let original_selection = before.range();
    // Prefer the actual old selection/caret. A plain longest-common-prefix
    // diff would misattribute insertion among repeated chars at styled edges.
    let anchored = inserted_for_range(old, new_text, original_selection.clone())
        .filter(|inserted| {
            after.collapsed() && after.focus == original_selection.start + inserted.chars().count()
        })
        .map(|inserted| (original_selection, inserted));
    let deletion =
        if anchored.is_none() && before.collapsed() && after.collapsed() && new_len < old_len {
            let range = if after.focus < before.focus {
                after.focus..before.focus
            } else {
                before.focus
                    ..before
                        .focus
                        .checked_add(old_len - new_len)
                        .ok_or("Selection overflow")?
            };
            inserted_for_range(old, new_text, range.clone())
                .filter(|text| text.is_empty())
                .map(|inserted| (range, inserted))
        } else {
            None
        };
    let (range, inserted) = if let Some(edit) = anchored.or(deletion) {
        edit
    } else {
        minimal_diff(old, new_text, old_len, new_len)?
    };
    let (owner, cross_run_deletion) =
        match resolve_owner(source, range.clone(), affinity.owner(source)) {
            Ok(owner) => (owner, None),
            Err(error) if inserted.is_empty() => {
                let (owner, selection) =
                    cross_run_deletion_selection(source, &range).ok_or(error)?;
                (owner, Some(selection))
            }
            Err(error) => return Err(error),
        };
    let old_bytes = scalar_bytes(old, range.clone())?;
    Ok(Some(RunEditPlan {
        run_ordinal: owner.run_ordinal,
        local_range: if cross_run_deletion.is_some() {
            0..0 // Only the explicit paragraph selection is used for this kind.
        } else {
            (range.start - owner.global_start)..(range.end - owner.global_start)
        },
        global_range: range,
        old_bytes,
        inserted,
        cross_run_deletion,
    }))
}

/// Deletion has no future insertion owner: positive character overlap permits
/// exact Run edges, but separators/cross-Paragraph ranges remain unsupported.
fn cross_run_deletion_selection(
    source: &impl SourceView,
    range: &Range<usize>,
) -> Option<(RunCharRange, TextSelection)> {
    if range.is_empty() {
        return None;
    }
    let mut overlaps = source
        .run_char_ranges()
        .filter(|run| run.global_start < range.end && range.start < run.global_end);
    let first = overlaps.next()?;
    let last = overlaps.last().unwrap_or(first);
    if first.paragraph != last.paragraph || first.run_ordinal >= last.run_ordinal {
        return None;
    }
    let mut paragraph = source
        .run_char_ranges()
        .filter(|run| run.paragraph == first.paragraph);
    let start = paragraph.next()?.global_start;
    let end = paragraph.last().unwrap_or(first).global_end;
    if range.start < start || range.end > end {
        return None;
    }
    Some((
        first,
        TextSelection {
            paragraph: first.paragraph,
            anchor: first.start_in_paragraph + range.start - first.global_start,
            focus: last.start_in_paragraph + range.end - last.global_start,
        },
    ))
}

/// Atomic source + affinity candidate. Caller installs patch source/plain and
/// one owner-history entry only after success. Unsupported edits stay errors;
/// there is no generic plain fallback that drops carrier metadata.
pub fn apply_committed(
    source: &ValidatedRichText,
    before: GlobalSelection,
    after: GlobalSelection,
    new_text: &str,
    affinity: &mut RunAffinity,
) -> Result<Option<AppliedRunEdit>> {
    let Some(plan) = plan_committed(source, before, after, new_text, affinity)? else {
        return Ok(None);
    };
    let patch = if let Some(selection) = plan.cross_run_deletion {
        let deleted = source.delete_cross_run_selection(selection)?;
        TextPatch {
            source: deleted.source,
            selection: deleted.selection,
        }
    } else {
        source
            .replace_in_run(
                plan.run_ordinal,
                plan.local_range.start,
                plan.local_range.end,
                plan.inserted,
            )?
            .ok_or("Changed TextEdit buffer unexpectedly produced no source edit")?
    };
    if patch.source.plain_text() != new_text {
        return Err("Run-owned edit does not reproduce exact TextEdit buffer".into());
    }
    // No fallible work after changing affinity. Successful Run edit keeps its
    // owner even if the resulting caret is also the next Run's start.
    let result = AppliedRunEdit {
        run_ordinal: plan.run_ordinal,
        old_global_range: plan.global_range,
        old_bytes: plan.old_bytes,
        after_selection: after,
        patch,
    };
    affinity.binding = plan
        .cross_run_deletion
        .is_none()
        .then(|| (result.patch.source.retained_owner(), result.run_ordinal));
    affinity.backspace_only = false;
    Ok(Some(result))
}

#[derive(Clone, Debug)]
pub struct AppliedAuthoredRunEdit {
    pub patch: AuthoredTextPatch,
    pub run_ordinal: Option<usize>,
}

/// Authored source retains its own seed provenance throughout the same pure
/// selection/diff mapping. Failed proof leaves the caller's affinity intact.
pub fn apply_authored_committed(
    source: &AuthoredRichText,
    before: GlobalSelection,
    after: GlobalSelection,
    new_text: &str,
    affinity: &mut RunAffinity,
) -> Result<Option<AppliedAuthoredRunEdit>> {
    let Some(plan) = plan_committed(source, before, after, new_text, affinity)? else {
        return Ok(None);
    };
    let patch = if let Some(selection) = plan.cross_run_deletion {
        let deleted = source.delete_cross_run_selection(selection)?;
        AuthoredTextPatch {
            source: deleted.source,
            selection: deleted.selection,
            following_run: None,
        }
    } else {
        source
            .replace_in_run(
                plan.run_ordinal,
                plan.local_range.start,
                plan.local_range.end,
                plan.inserted,
            )?
            .ok_or("Changed authored buffer produced no source edit")?
    };
    if patch.source.plain_text() != new_text {
        return Err("Authored Run edit does not reproduce exact TextEdit buffer".into());
    }
    let run_ordinal = if plan.cross_run_deletion.is_some() {
        affinity.clear();
        None
    } else {
        affinity.remember_run(&patch.source, plan.run_ordinal)?;
        Some(plan.run_ordinal)
    };
    Ok(Some(AppliedAuthoredRunEdit { patch, run_ordinal }))
}

fn resolve_owner(
    source: &impl SourceView,
    range: Range<usize>,
    hint: Option<usize>,
) -> Result<RunCharRange> {
    let candidates: Vec<_> = source
        .run_char_ranges()
        .filter(|run| run.global_start <= range.start && range.end <= run.global_end)
        .collect();
    if !range.is_empty() {
        return match candidates.as_slice() {
            [owner] => Ok(*owner),
            _ => Err("Cross-Run/paragraph replacement is unsupported".into()),
        };
    }
    if let Some(owner) = candidates.iter().find(|run| Some(run.run_ordinal) == hint) {
        return Ok(*owner);
    }
    if let Some(owner) = candidates
        .iter()
        .find(|run| run.global_start < range.start && range.start < run.global_end)
    {
        return Ok(*owner);
    }
    // Independent bounded policy: paragraph start owns first Run, paragraph
    // end owns last Run. No wrap/no internal left-vs-right style guess.
    if let Some(candidate) = candidates.first() {
        let mut paragraph = source
            .run_char_ranges()
            .filter(|run| run.paragraph == candidate.paragraph);
        let first = paragraph.next().ok_or("Missing Paragraph Run")?;
        let last = paragraph.last().unwrap_or(first);
        if first.global_start == last.global_end && first.run_ordinal != last.run_ordinal {
            return Err("Multiple empty Run owners require explicit affinity".into());
        }
        if range.start == first.global_start {
            return Ok(first);
        }
        if range.start == last.global_end {
            return Ok(last);
        }
    }
    Err("Internal Run boundary requires explicit retained affinity".into())
}

/// A command may capture only a uniquely owned selection/caret. It must not
/// invent a style at an ambiguous internal boundary after navigation.
pub fn selection_owner(
    source: &impl SourceView,
    selection: GlobalSelection,
    affinity: &RunAffinity,
) -> Result<usize> {
    resolve_owner(source, selection.range(), affinity.owner(source)).map(|run| run.run_ordinal)
}

fn scalar_bytes(text: &str, range: Range<usize>) -> Result<Range<usize>> {
    if range.start > range.end {
        return Err("Invalid scalar range".into());
    }
    let mut start = None;
    let mut end = None;
    for (scalar, (byte, _)) in text
        .char_indices()
        .chain(std::iter::once((text.len(), '\0')))
        .enumerate()
    {
        if scalar == range.start {
            start = Some(byte);
        }
        if scalar == range.end {
            end = Some(byte);
            break;
        }
    }
    Ok(start.ok_or("Scalar range starts outside text")?
        ..end.ok_or("Scalar range ends outside text")?)
}
fn inserted_for_range<'a>(old: &str, new: &'a str, range: Range<usize>) -> Option<&'a str> {
    let bytes = scalar_bytes(old, range).ok()?;
    new.strip_prefix(&old[..bytes.start])?
        .strip_suffix(&old[bytes.end..])
}
fn minimal_diff<'a>(
    old: &str,
    new: &'a str,
    old_len: usize,
    new_len: usize,
) -> Result<(Range<usize>, &'a str)> {
    let prefix = old
        .chars()
        .zip(new.chars())
        .take_while(|(a, b)| a == b)
        .count();
    let suffix = old
        .chars()
        .rev()
        .zip(new.chars().rev())
        .take((old_len - prefix).min(new_len - prefix))
        .take_while(|(a, b)| a == b)
        .count();
    let range = prefix..(old_len - suffix);
    let inserted = &new[scalar_bytes(new, prefix..(new_len - suffix))?];
    Ok((range, inserted))
}

#[cfg(test)]
#[path = "rich_text_edit_map_tests.rs"]
mod tests;
