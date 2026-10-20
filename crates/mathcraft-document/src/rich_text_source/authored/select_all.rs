//! Explicit ordinary typing after a verified live SelectAll. This method does
//! not establish input provenance from serialized source or (0, length) alone.
//! Native pointer/character evidence and independent wrapper policy: README.
use super::*;

impl AuthoredRichText {
    /// Closed authored, one nonempty Paragraph, complete selection only.
    /// Caller proves actual SelectAll and ordinary Text delivery. The explicit
    /// live context is checked again; native/default/springload inference is not
    /// accepted. Existing strict interior cross-Run APIs remain unchanged.
    pub fn replace_after_select_all(
        &self,
        selection: TextSelection,
        text: &str,
        context: CrossRunTypingContext<'_>,
    ) -> Result<CrossRunPatch<Self>> {
        if self.index.paragraphs.len() != 1 || selection.paragraph != 0 {
            return Err("SelectAll typing supports one authored Paragraph".into());
        }
        let length = self.index.paragraphs[0].chars().count();
        if length == 0 || selection.sorted() != (0..length) {
            return Err("SelectAll typing requires the complete nonempty Paragraph".into());
        }
        if text.is_empty()
            || text.len() > MAX_SOURCE_BYTES
            || text
                .chars()
                .any(|c| c < ' ' || matches!(c, '\u{FFFE}' | '\u{FFFF}'))
        {
            return Err("Unsupported SelectAll text/control/size".into());
        }
        let (owner_id, owner) = self
            .index
            .runs
            .iter()
            .enumerate()
            .find(|(_, run)| run.boundaries.len() > 1)
            .ok_or("Missing normalized nonempty Run")?;
        if context.insertion_owner != owner_id || context.has_retained_springload {
            return Err("SelectAll normalized owner/springload mismatch".into());
        }
        let actual = self.selection_format(TextSelection {
            paragraph: 0,
            anchor: owner.scalar_start,
            focus: owner.scalar_start + 1,
        })?;
        if actual.weight.is_none()
            || actual.style.is_none()
            || actual.size_dip.is_none()
            || actual.foreground_argb.is_none()
            || actual.text_decorations.is_none()
            || &actual != context.expected_format
        {
            return Err("SelectAll complete effective character format mismatch".into());
        }
        // prepared/prove_generated_source already bound the exact root xml:lang
        // to the validated seed and forbade Run/Paragraph language/flow changes.
        // This closed scaffold has no FlowDirection override (declared LTR).
        // Reuse that receipt, not a new XML parse nor native catalog inference.
        if !self
            .seed
            .spec
            .proofing_language
            .eq_ignore_ascii_case(context.input_language)
            || context.input_flow_direction != TypingFlowDirection::LeftToRight
        {
            return Err("SelectAll declared input language/flow differs".into());
        }
        // Compute escaped cost before constructing output. The source's normal
        // from-prepared proof still checks aggregate own-wire/seed JSON budget.
        let inserted_bytes = text.chars().try_fold(0usize, |n, c| {
            n.checked_add(match c {
                '&' => 5,
                '<' | '>' => 4,
                _ => c.len_utf8(),
            })
            .ok_or("SelectAll escaped size overflow")
        })?;
        let removed = self.index.runs.iter().try_fold(0usize, |n, run| {
            n.checked_add(run.closing_start - run.opening_end)
                .ok_or("SelectAll removed size overflow")
        })?;
        let size = self
            .xaml()
            .len()
            .checked_sub(removed)
            .and_then(|n| n.checked_add(inserted_bytes))
            .filter(|n| *n <= MAX_SOURCE_BYTES)
            .ok_or("SelectAll XML budget exceeded")?;
        let mut xml = String::with_capacity(size);
        let mut copied = 0;
        for (ordinal, run) in self.index.runs.iter().enumerate() {
            xml.push_str(&self.xaml()[copied..run.opening_end]);
            if ordinal == owner_id {
                for c in text.chars() {
                    match c {
                        '&' => xml.push_str("&amp;"),
                        '<' => xml.push_str("&lt;"),
                        '>' => xml.push_str("&gt;"),
                        _ => xml.push(c),
                    }
                }
            }
            copied = run.closing_start;
        }
        xml.push_str(&self.xaml()[copied..]);
        let source = self.changed(xml)?;
        let caret = text.chars().count();
        Ok(CrossRunPatch {
            source,
            selection: TextSelection {
                paragraph: 0,
                anchor: caret,
                focus: caret,
            },
            insertion_owner: Some(owner_id),
        })
    }
}

#[cfg(test)]
#[path = "select_all_tests.rs"]
mod tests;
