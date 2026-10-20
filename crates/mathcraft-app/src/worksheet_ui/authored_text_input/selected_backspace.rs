//! Artifact candidate: one admitted selected Backspace delivery, authored only.
//! WPF 5be0/0198..0201 captures Start format before deleting selection. This
//! private, single-use token is not inferred from an empty text diff or wire.
use super::*;
use mathcraft_document::rich_text_source::{AuthoredStyleSnapshot, RunCharRange, TextDecorations};

#[derive(Debug)]
pub(in crate::worksheet_ui) struct SelectedBackspace {
    owner: u64,
    source: AuthoredRichText,
    before: GlobalSelection,
    first: RunCharRange,
    format: AuthoredStyleSnapshot,
    decorations: TextDecorations,
    full_paragraph: bool,
}

impl SelectedBackspace {
    /// Read before TextEdit consumes events. No idle source clone/parse. The
    /// retained source is shallow and held for this call only, not a frame flag.
    pub(super) fn capture(
        owner: u64,
        source: &AuthoredRichText,
        before: GlobalSelection,
        events: &[egui::Event],
        owns_focus: bool,
        composition_delivery: bool,
    ) -> Option<Self> {
        if !owns_focus || composition_delivery || !single_backspace(events) {
            return None;
        }
        let start = before.anchor.min(before.focus);
        let end = before.anchor.max(before.focus);
        if start == end {
            return None;
        }
        // A full observed selection plus this actual Backspace is enough:
        // WPF normalizes every nonempty selected range, not only Ctrl+A.
        // No source wire/cold empty state alone can enter this event-gated API.
        let full_paragraph = start == 0
            && end == source.plain_text().chars().count()
            && source.run_char_ranges().all(|run| run.paragraph == 0)
            // Existing unique-Run edits already have their own commit path.
            // A captured token is accepted only for actual cross-Run deletion.
            && source.run_char_ranges().filter(|run| run.global_start < run.global_end).take(2).count() == 2;
        let first = if full_paragraph {
            // Forward-normalized Start.Parent skips leading empty Runs.
            source
                .run_char_ranges()
                .find(|run| run.global_start < run.global_end)?
        } else {
            // Preserve the original strict-interior branch exactly.
            let first = source
                .run_char_ranges()
                .find(|run| run.global_start < start && start < run.global_end)?;
            let last = source
                .run_char_ranges()
                .find(|run| run.global_start < end && end < run.global_end)?;
            if first.paragraph != last.paragraph || first.run_ordinal >= last.run_ordinal {
                return None;
            }
            first
        };
        // These getters cover the complete closed authored character model:
        // immutable seed key/family/background/language plus the four explicit
        // Run properties and closed fifth decoration. No native defaults.
        let format = source.creation_format_for_run(first.run_ordinal).ok()?;
        let decorations = source
            .creation_text_decorations_for_run(first.run_ordinal)
            .ok()?;
        Some(Self {
            owner,
            source: source.clone(),
            before,
            first,
            format,
            decorations,
            full_paragraph,
        })
    }

    pub(super) fn validate_before(
        &self,
        owner: u64,
        source: &AuthoredRichText,
        before: GlobalSelection,
    ) -> Result<(), String> {
        if self.owner != owner
            || self.before != before
            || !source.wire().same_source_version(self.source.wire())
        {
            return Err("Stale selected Backspace source/owner/selection".into());
        }
        Ok(())
    }

    /// Called only after candidate source/plain, layout and aggregate budget
    /// admission. Failure changes only an unpublished candidate editor.
    pub(super) fn rebind_after_checked(
        self,
        next: &mut AuthoredEditor,
        text: &str,
        after: GlobalSelection,
    ) -> Result<(), String> {
        let start = self.before.anchor.min(self.before.focus);
        let end = self.before.anchor.max(self.before.focus);
        if after
            != (GlobalSelection {
                anchor: start,
                focus: start,
            })
            || next.source.plain_text() != text
            || !text.chars().eq(self
                .source
                .plain_text()
                .chars()
                .take(start)
                .chain(self.source.plain_text().chars().skip(end)))
        {
            return Err("Delivery did not produce the captured selected deletion".into());
        }
        let run = next
            .source
            .run_char_ranges()
            .find(|run| run.run_ordinal == self.first.run_ordinal)
            .ok_or("Captured authored Run no longer survives")?;
        if run.paragraph != self.first.paragraph
            || run.global_start != self.first.global_start
            || run.global_end != start
            || (!self.full_paragraph && run.global_start == run.global_end)
            || !Arc::ptr_eq(next.source.seed(), self.source.seed())
            || next.source.creation_format_for_run(run.run_ordinal)? != self.format
            || next
                .source
                .creation_text_decorations_for_run(run.run_ordinal)?
                != self.decorations
        {
            return Err("Surviving authored Run does not represent captured format".into());
        }
        if self.full_paragraph {
            // Full deletion may leave every body empty. Prove ALL exact raw
            // wrapper/root/Paragraph bytes, not only the captured Run's format.
            if !text.is_empty() || empty_run_bodies(self.source.xaml())? != next.source.xaml() {
                return Err("Full Backspace changed source outside Run bodies".into());
            }
        }
        // The source command already preserved every wrapper and seed proof.
        // This owner represents the captured Backspace format, not a general
        // left/right rule for Delete, Cut, IME or pointer navigation.
        next.affinity
            .remember_backspace_run(&next.source, run.run_ordinal)
    }
}

/// Full-Backspace command-only proof, never an idle/layout/typing parser. Both
/// inputs were already admitted authored sources. Keep the old raw metadata
/// exactly while removing only direct Run bodies, including entity lexemes.
fn empty_run_bodies(source: &str) -> Result<String, String> {
    let document = roxmltree::Document::parse(source).map_err(|e| e.to_string())?;
    let mut out = String::with_capacity(source.len());
    let mut copied = 0;
    for run in document.descendants().filter(|n| {
        n.has_tag_name((
            "http://schemas.microsoft.com/winfx/2006/xaml/presentation",
            "Run",
        ))
    }) {
        let range = run.range();
        let raw = &source[range.clone()];
        let mut quote = None;
        let opening = raw
            .bytes()
            .position(|byte| {
                if let Some(active) = quote {
                    if byte == active {
                        quote = None;
                    }
                    false
                } else if byte == b'\'' || byte == b'"' {
                    quote = Some(byte);
                    false
                } else {
                    byte == b'>'
                }
            })
            .ok_or("Missing admitted Run opening")?
            + range.start
            + 1;
        let closing = range
            .end
            .checked_sub("</Run>".len())
            .ok_or("Missing admitted Run closing")?;
        if copied > range.start || opening > closing || &source[closing..range.end] != "</Run>" {
            return Err("Unsupported admitted Run wrapper".into());
        }
        out.push_str(&source[copied..opening]);
        copied = closing;
    }
    out.push_str(&source[copied..]);
    Ok(out)
}

pub(super) fn retire_nonordinary_delivery(
    editor: &mut AuthoredEditor,
    events: &[egui::Event],
    owns_focus: bool,
    composing: bool,
) {
    if owns_focus
        && editor.affinity.is_backspace_continuation()
        && retires_continuation(events, composing)
    {
        editor.affinity.retire_backspace_continuation();
    }
}

fn retires_continuation(events: &[egui::Event], composing: bool) -> bool {
    composing
        || events.iter().any(|event| match event {
            egui::Event::Ime(_)
            | egui::Event::Paste(_)
            | egui::Event::Cut
            | egui::Event::WindowFocused(false) => true,
            egui::Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } => {
                matches!(key, egui::Key::Backspace | egui::Key::Delete | egui::Key::Enter
                | egui::Key::Tab | egui::Key::ArrowLeft | egui::Key::ArrowRight
                | egui::Key::ArrowUp | egui::Key::ArrowDown | egui::Key::Home
                | egui::Key::End | egui::Key::PageUp | egui::Key::PageDown)
                // Save itself is nonediting and explicitly preserves affinity.
                // Other command/alt chords cannot inherit this narrow grant.
                || ((modifiers.command || modifiers.alt || modifiers.ctrl)
                    && !(*key == egui::Key::S && modifiers.command && !modifiers.alt))
            }
            _ => false,
        })
}

fn single_backspace(events: &[egui::Event]) -> bool {
    let mut pressed = false;
    for event in events {
        match event {
            egui::Event::Key {
                key: egui::Key::Backspace,
                pressed: true,
                modifiers,
                ..
            } if *modifiers == egui::Modifiers::NONE && !pressed => pressed = true,
            egui::Event::Key { pressed: false, .. } => {}
            // Even movement/pointer/focus/clipboard/IME input stays outside this
            // shortest subset. Key releases alone cannot grant a proof.
            _ => return false,
        }
    }
    pressed
}

#[cfg(test)]
#[path = "selected_backspace_tests.rs"]
mod tests;
