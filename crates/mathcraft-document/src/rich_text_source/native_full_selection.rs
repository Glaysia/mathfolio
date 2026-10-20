//! Ordinary Text after controller-proven SelectAll. No implicit format values.
use super::*;

/// Non-serde live caller policy, not recoverable from source or a full range.
/// The caller must prove an actual fresh SelectAll cleared springload, exact
/// owner/source/selection and one ordinary Text delivery (not Paste or IME).
/// A source-language-preserving profile must not be called actual OS culture.
#[derive(Clone, Copy, Debug)]
pub struct NativeFullSelectionTypingContext<'a> {
    pub input_language: &'a str,
    pub input_flow_direction: TypingFlowDirection,
    pub has_retained_springload: bool,
}

impl ValidatedRichText {
    /// Keeps every original current wrapper; only Run bodies are rewritten.
    /// Unknown source properties stay unknown. WPF serializer partition/merge
    /// parity is not claimed. Canvas/runtime admission remains the caller's job.
    pub fn replace_select_all_text(
        &self,
        selection: TextSelection,
        text: &str,
        context: NativeFullSelectionTypingContext<'_>,
    ) -> Result<Option<CrossRunPatch<Self>>> {
        if self.wire.paragraph_origins.is_some()
            || self.index.paragraphs.len() != 1
            || selection.paragraph != 0
            || context.has_retained_springload
            || context.input_flow_direction != TypingFlowDirection::LeftToRight
        {
            return Err("Unsupported native SelectAll source/context".into());
        }
        let length = self.index.paragraphs[0].chars().count();
        if length == 0 || selection.sorted() != (0..length) {
            return Err("Expected full nonempty single-Paragraph selection".into());
        }
        if text.is_empty()
            || text.len() > MAX_SOURCE_BYTES
            || text
                .chars()
                .any(|c| c < ' ' || matches!(c, '\u{FFFE}' | '\u{FFFF}'))
        {
            return Err("Unsupported ordinary SelectAll text/control/size".into());
        }
        let (owner_id, owner) = self
            .index
            .runs
            .iter()
            .enumerate()
            .find(|(_, run)| run.boundaries.len() > 1)
            .ok_or("Missing nonempty normalized insertion Run")?;
        cross_run::require_input_profile(
            self.xaml(),
            owner,
            context.input_language,
            context.input_flow_direction,
            None,
        )?;
        let escaped_bytes = text
            .chars()
            .try_fold(0usize, |size, c| {
                size.checked_add(match c {
                    '&' => 5,
                    '<' | '>' => 4,
                    _ => c.len_utf8(),
                })
            })
            .ok_or("SelectAll escaped byte overflow")?;
        let removed = self
            .index
            .runs
            .iter()
            .try_fold(0usize, |size, run| {
                size.checked_add(run.closing_start - run.opening_end)
            })
            .ok_or("SelectAll body byte overflow")?;
        let size = self
            .xaml()
            .len()
            .checked_sub(removed)
            .and_then(|size| size.checked_add(escaped_bytes))
            .filter(|size| *size <= MAX_SOURCE_BYTES)
            .ok_or("SelectAll source byte budget exceeded")?;
        let escaped = text
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;");
        let mut xml = String::with_capacity(size);
        let mut copied = 0;
        for (id, run) in self.index.runs.iter().enumerate() {
            xml.push_str(&self.xaml()[copied..run.opening_end]);
            if id == owner_id {
                xml.push_str(&escaped);
            }
            copied = run.closing_start;
        }
        xml.push_str(&self.xaml()[copied..]);
        if xml == self.xaml() {
            return Ok(None);
        }
        let mut wire = self.wire.as_ref().clone();
        wire.current_xaml = xml.into();
        wire.visual_fingerprint = Default::default();
        let parsed = Arc::new(parse_source(&wire.current_xaml, &self.authority.keys)?);
        let source = Self::from_prepared(wire, self.authority.clone(), parsed)?;
        let caret = text.chars().count();
        Ok(Some(CrossRunPatch {
            source,
            selection: TextSelection {
                paragraph: 0,
                anchor: caret,
                focus: caret,
            },
            insertion_owner: Some(owner_id),
        }))
    }

    /// Positive export classification witness, not a format-proof fallback.
    /// All original wrappers, count/order and block bytes must be exact; only
    /// original Run bodies may differ. A prior format/split delta returns false.
    pub fn has_original_raw_body_only_delta(&self) -> bool {
        if self.wire.paragraph_origins.is_some()
            || self.index.runs.len() != self.authority.original_index.runs.len()
            || outside_runs(self.xaml(), &self.index)
                != outside_runs(
                    self.authority.original_xaml(),
                    &self.authority.original_index,
                )
        {
            return false;
        }
        let mut changed = false;
        for (current, old) in self
            .index
            .runs
            .iter()
            .zip(&self.authority.original_index.runs)
        {
            let original = self.authority.original_xaml();
            if current.paragraph != old.paragraph
                || self.xaml()[current.range.start..current.opening_end]
                    != original[old.range.start..old.opening_end]
                || self.xaml()[current.closing_start..current.range.end]
                    != original[old.closing_start..old.range.end]
            {
                return false;
            }
            changed |= self.xaml()[current.opening_end..current.closing_start]
                != original[old.opening_end..old.closing_start];
        }
        changed
    }
}

#[cfg(test)]
#[path = "native_full_selection_tests.rs"]
mod tests;
