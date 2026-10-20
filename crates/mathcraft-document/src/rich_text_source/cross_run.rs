//! Artifact candidate. Direct Run bodies only; no WPF serializer-shape claim.
//! Evidence and controller obligations are in this candidate's README.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TypingFlowDirection {
    LeftToRight,
    RightToLeft,
}
impl TypingFlowDirection {
    fn as_str(self) -> &'static str {
        match self {
            Self::LeftToRight => "LeftToRight",
            Self::RightToLeft => "RightToLeft",
        }
    }
}

/// Trusted live controller inputs, NOT deserializable source authority.
/// The controller must know whether a springload exists; source XML cannot
/// recover WPF's retained typing state. A retained state is not supported here.
#[derive(Clone, Copy, Debug)]
pub struct CrossRunTypingContext<'a> {
    pub insertion_owner: usize,
    pub expected_format: &'a TextFormat,
    pub input_language: &'a str,
    pub input_flow_direction: TypingFlowDirection,
    pub has_retained_springload: bool,
}

#[derive(Clone, Debug)]
pub struct CrossRunPatch<T> {
    pub source: T,
    pub selection: TextSelection,
    /// Valid only in this returned source. Deletion deliberately does not
    /// choose future typing affinity (Delete and Backspace differ in WPF).
    pub insertion_owner: Option<usize>,
}

pub(super) struct PreparedCrossRun {
    pub xml: String,
    pub selection: TextSelection,
    pub insertion_owner: Option<usize>,
}

impl ValidatedRichText {
    /// Same Paragraph, selected characters overlap distinct Runs. Deletion may
    /// include exact Run/Paragraph boundaries; typing remains strictly interior.
    /// Every wrapper/style key/unselected raw byte is retained. Empty bodies
    /// are deliberate source-preserving policy, not captured WPF output.
    pub fn delete_cross_run_selection(
        &self,
        selection: TextSelection,
    ) -> Result<CrossRunPatch<Self>> {
        self.apply_cross_run(selection, "", None)
    }

    /// Ordinary committed typing only. Plain paste and IME transaction
    /// semantics have separate native paths and are not admitted by this API.
    pub fn replace_cross_run_selection(
        &self,
        selection: TextSelection,
        text: &str,
        context: CrossRunTypingContext<'_>,
    ) -> Result<CrossRunPatch<Self>> {
        if text.is_empty() {
            return Err("Use explicit selection deletion".into());
        }
        self.apply_cross_run(selection, text, Some(context))
    }

    fn apply_cross_run(
        &self,
        selection: TextSelection,
        text: &str,
        context: Option<CrossRunTypingContext<'_>>,
    ) -> Result<CrossRunPatch<Self>> {
        let extra = paragraph_origin::additional_wire_bytes(self.wire.paragraph_origins.as_deref());
        let prepared = prepare_cross_run(
            self.xaml(),
            &self.index,
            selection,
            text,
            context,
            None,
            None,
            extra,
        )?;
        let mut wire = self.wire.as_ref().clone();
        wire.current_xaml = prepared.xml.into();
        wire.visual_fingerprint = Default::default();
        let index = Arc::new(parse_source(&wire.current_xaml, &self.authority.keys)?);
        let source = Self::from_prepared(wire, self.authority.clone(), index)?;
        Ok(CrossRunPatch {
            source,
            selection: prepared.selection,
            insertion_owner: prepared.insertion_owner,
        })
    }
}

/// Shared by native authority and the independently validated authored seed.
/// No source mutation occurs until the caller validates the returned candidate.
#[allow(clippy::too_many_arguments)]
pub(super) fn prepare_cross_run(
    xml: &str,
    index: &SourceIndex,
    selection: TextSelection,
    text: &str,
    context: Option<CrossRunTypingContext<'_>>,
    decoration_default: Option<TextDecorations>,
    flow_default: Option<&str>,
    extra_source_bytes: usize,
) -> Result<PreparedCrossRun> {
    if text.len() > MAX_SOURCE_BYTES
        || text
            .chars()
            .any(|c| c < ' ' || matches!(c, '\u{FFFE}' | '\u{FFFF}'))
    {
        return Err("Unsupported cross-Run control/newline/size".into());
    }
    let selected = selection.sorted();
    if selected.is_empty() || selection.paragraph >= index.paragraphs.len() {
        return Err("Expected nonempty same-Paragraph selection".into());
    }
    let paragraph_len = index.paragraphs[selection.paragraph].chars().count();
    if selected.end > paragraph_len {
        return Err("Selection exceeds Paragraph scalar bounds".into());
    }
    // Typing still requires unambiguous interior endpoint ownership. Empty
    // replacement needs only character overlap, never a future typing owner.
    let (first_id, first, last_id) = if context.is_some() {
        let containing = |position: usize| {
            index.runs.iter().enumerate().find(|(_, run)| {
                let end = run.scalar_start + run.boundaries.len() - 1;
                run.paragraph == selection.paragraph
                    && run.scalar_start < position
                    && position < end
            })
        };
        let (first_id, first) = containing(selected.start).ok_or("Start must be inside a Run")?;
        let (last_id, _) = containing(selected.end).ok_or("End must be inside a Run")?;
        (first_id, first, last_id)
    } else {
        let mut overlaps = index.runs.iter().enumerate().filter(|(_, run)| {
            let end = run.scalar_start + run.boundaries.len() - 1;
            run.paragraph == selection.paragraph
                && run.scalar_start < selected.end
                && selected.start < end
        });
        let (first_id, first) = overlaps.next().ok_or("No selected Run characters")?;
        let last_id = overlaps.next_back().map_or(first_id, |(id, _)| id);
        (first_id, first, last_id)
    };
    if first_id >= last_id {
        return Err("Expected selection crossing distinct Runs".into());
    }
    if let Some(context) = context {
        if context.insertion_owner != first_id || context.has_retained_springload {
            return Err("Unsupported insertion owner/retained springload".into());
        }
        let owner_point = TextSelection {
            paragraph: selection.paragraph,
            anchor: selected.start,
            focus: selected.start + 1,
        };
        let actual =
            selected_index_format_with_decoration_default(index, owner_point, decoration_default)?;
        if actual.weight.is_none()
            || actual.style.is_none()
            || actual.size_dip.is_none()
            || actual.foreground_argb.is_none()
            || actual.text_decorations.is_none()
            || &actual != context.expected_format
        {
            return Err("Missing/different effective insertion format".into());
        }
        require_input_context(xml, first, context, flow_default)?;
    } else if !text.is_empty() {
        return Err("Typing needs explicit live insertion context".into());
    }
    let escaped = text
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    let mut edits = Vec::with_capacity(last_id - first_id + 1);
    for ordinal in first_id..=last_id {
        let run = &index.runs[ordinal];
        if run.paragraph != selection.paragraph {
            return Err("Cross-Paragraph replacement unsupported".into());
        }
        let len = run.boundaries.len() - 1;
        let lo = selected.start.saturating_sub(run.scalar_start).min(len);
        let hi = selected.end.saturating_sub(run.scalar_start).min(len);
        if lo < hi {
            edits.push((
                run.opening_end + run.boundaries[lo]..run.opening_end + run.boundaries[hi],
                if ordinal == first_id {
                    escaped.as_str()
                } else {
                    ""
                },
            ));
        }
    }
    let removed = edits.iter().map(|(range, _)| range.len()).sum::<usize>();
    let size = xml
        .len()
        .checked_sub(removed)
        .and_then(|n| n.checked_add(escaped.len()))
        .ok_or("Cross-Run size overflow")?;
    if size
        .checked_add(extra_source_bytes)
        .is_none_or(|n| n > MAX_SOURCE_BYTES)
    {
        return Err("Cross-Run aggregate source budget exceeded".into());
    }
    let mut output = String::with_capacity(size);
    let mut copied = 0;
    for (range, replacement) in edits {
        output.push_str(&xml[copied..range.start]);
        output.push_str(replacement);
        copied = range.end;
    }
    output.push_str(&xml[copied..]);
    let caret = selected.start + text.chars().count();
    Ok(PreparedCrossRun {
        xml: output,
        selection: TextSelection {
            paragraph: selection.paragraph,
            anchor: caret,
            focus: caret,
        },
        insertion_owner: context.map(|_| first_id),
    })
}

fn require_input_context(
    xml: &str,
    owner: &IndexedRun,
    context: CrossRunTypingContext<'_>,
    flow_default: Option<&str>,
) -> Result<()> {
    require_input_profile(
        xml,
        owner,
        context.input_language,
        context.input_flow_direction,
        flow_default,
    )
}

pub(super) fn require_input_profile(
    xml: &str,
    owner: &IndexedRun,
    input_language: &str,
    input_flow_direction: TypingFlowDirection,
    flow_default: Option<&str>,
) -> Result<()> {
    // Command-only XML inspection; never called by idle/render/query getters.
    // It resolves exact inherited xml:lang and FlowDirection, NOT catalog data.
    let doc = parse_xml(xml)?;
    let node = doc
        .descendants()
        .find(|node| node.is_element() && node.range() == owner.range)
        .ok_or("Insertion Run source mismatch")?;
    let language = node
        .ancestors()
        .filter(Node::is_element)
        .find_map(|n| n.attribute((XML, "lang")))
        .ok_or("Unknown input language")?;
    let flow = node
        .ancestors()
        .filter(Node::is_element)
        .find_map(|n| n.attribute("FlowDirection"))
        .or(flow_default)
        .ok_or("Unknown input flow direction")?;
    if input_language.is_empty()
        || input_language.len() > 64
        || !input_language
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        || !language.eq_ignore_ascii_case(input_language)
        || flow != input_flow_direction.as_str()
    {
        return Err("Input language/flow direction differs from source owner".into());
    }
    Ok(())
}

#[cfg(test)]
#[path = "cross_run_tests.rs"]
mod tests;
