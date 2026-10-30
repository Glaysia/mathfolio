//! Actual SelectAll provenance, distinct from a full-range source selection.
//! Ordinary text uses the DECLARED closed authored application input profile.
//! This does not observe Windows InputLanguageManager or infer its culture.
use super::*;
use mathfolio_document::rich_text_source::{
    CrossRunTypingContext, RunCharRange, TextFormat, TypingFlowDirection,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ClosedOwnInputProfile {
    language: String,
    flow: TypingFlowDirection,
}
impl ClosedOwnInputProfile {
    fn for_authored_creation(source: &AuthoredRichText) -> Self {
        Self {
            // Existing creation policy explicitly stores proofing_language.
            // Retaining that language for ordinary authored text is our policy,
            // not a measurement of the OS's current keyboard/input culture.
            language: source.seed().spec().proofing_language.clone(),
            flow: TypingFlowDirection::LeftToRight,
        }
    }
}

struct Pending {
    owner: u64,
    source: AuthoredRichText,
    first: RunCharRange,
    format: TextFormat,
    profile: ClosedOwnInputProfile,
    selection: Option<GlobalSelection>,
    ready: Option<String>,
}
#[derive(Default)]
pub(super) struct SelectAllInput {
    pending: Option<Pending>,
}

pub(in crate::worksheet_ui) enum Delivery<'a> {
    SelectAll,
    Text(&'a str),
    Benign,
    Retire,
}
pub(in crate::worksheet_ui) fn delivery(events: &[egui::Event]) -> Delivery<'_> {
    let mut admitted = Delivery::Benign;
    for event in events {
        match event {
            egui::Event::Key { pressed: false, .. }
            | egui::Event::ModifiersChanged(_)
            | egui::Event::PointerMoved(_)
            | egui::Event::PointerGone => {}
            egui::Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } if (modifiers.is_none() || modifiers.matches_exact(egui::Modifiers::SHIFT))
                && (*key == egui::Key::Space
                    || (egui::Key::Colon..=egui::Key::Z).contains(key)) => {}
            egui::Event::Key {
                key: egui::Key::A,
                pressed: true,
                modifiers,
                ..
            } if modifiers.matches_exact(egui::Modifiers::COMMAND)
                && matches!(admitted, Delivery::Benign) =>
            {
                admitted = Delivery::SelectAll;
            }
            egui::Event::Text(text) if !text.is_empty() && matches!(admitted, Delivery::Benign) => {
                admitted = Delivery::Text(text);
            }
            // Even identical IME/Paste strings, pointer-down/navigation, formatting
            // chords and multiple editing events cannot acquire this token.
            _ => return Delivery::Retire,
        }
    }
    admitted
}

impl WorksheetApp {
    // Main command routing may consume keys before the field is drawn. Never
    // let such a command leave an old SelectAll permission behind.
    pub(in crate::worksheet_ui) fn retire_authored_select_all_inputs(&mut self) {
        // Historical entrypoint now retires both source authority variants.
        for editor in self
            .rich_text
            .editors
            .values_mut()
            .filter_map(|e| e.as_mut().ok())
        {
            editor.retire_select_all();
        }
        for editor in self
            .rich_text
            .authored_editors
            .values_mut()
            .filter_map(|e| e.as_mut().ok())
        {
            editor.retire_select_all();
        }
    }
    pub(in crate::worksheet_ui) fn retire_authored_select_all_before_commands(
        &mut self,
        ctx: &egui::Context,
    ) {
        // Undrawn owners cannot run the field's focus callback. Read optional
        // runtime state only; no XML, source clone or text scan on idle frames.
        for (id, editor) in &mut self.rich_text.editors {
            if let Ok(editor) = editor
                && editor.has_pending_select_all()
                && !ctx.memory(|m| m.has_focus(plain_text_input::widget_id(*id)))
            {
                editor.retire_select_all();
            }
        }
        for (id, editor) in &mut self.rich_text.authored_editors {
            if let Ok(editor) = editor
                && editor.select_all.pending.is_some()
                && !ctx.memory(|m| m.has_focus(plain_text_input::widget_id(*id)))
            {
                editor.retire_select_all();
            }
        }
        if self.pending.is_some() || ctx.input(|i| matches!(delivery(&i.events), Delivery::Retire))
        {
            self.retire_authored_select_all_inputs();
        }
    }
}

impl SelectAllInput {
    pub(super) fn clear(&mut self) {
        self.pending = None;
    }
    pub(super) fn ready(&self) -> bool {
        self.pending.as_ref().is_some_and(|p| p.ready.is_some())
    }
    fn bound(p: &Pending, owner: u64, source: &AuthoredRichText) -> bool {
        p.owner == owner && p.source.wire().same_source_version(source.wire())
    }
    pub(super) fn begin(
        &mut self,
        owner: u64,
        source: &AuthoredRichText,
        before: GlobalSelection,
        events: &[egui::Event],
        owns_focus: bool,
        composing: bool,
    ) -> bool {
        if !owns_focus || composing {
            self.clear();
            return false;
        }
        match delivery(events) {
            Delivery::SelectAll => {
                self.clear();
                let length = source.plain_text().chars().count();
                let mut runs = source.run_char_ranges();
                let Some(first) = runs.find(|run| run.global_start < run.global_end) else {
                    return true; // Actual SelectAll still clears old affinity.
                };
                if first.paragraph != 0 || runs.any(|run| run.paragraph != 0) || length == 0 {
                    return true;
                }
                let Ok(format) = source.selection_format(TextSelection {
                    paragraph: 0,
                    anchor: first.start_in_paragraph,
                    focus: first.start_in_paragraph + 1,
                }) else {
                    return true;
                };
                self.pending = Some(Pending {
                    owner,
                    source: source.clone(), // all immutable payload/index Arcs
                    first,
                    format,
                    profile: ClosedOwnInputProfile::for_authored_creation(source),
                    selection: None,
                    ready: None,
                });
                true
            }
            Delivery::Text(text) => {
                let accepted = self.pending.as_ref().is_some_and(|p| {
                    Self::bound(p, owner, source)
                        && p.selection == Some(before)
                        && p.ready.is_none()
                        && p.profile == ClosedOwnInputProfile::for_authored_creation(source)
                });
                if accepted {
                    self.pending.as_mut().unwrap().ready = Some(text.to_owned());
                } else {
                    self.clear();
                }
                false
            }
            Delivery::Benign => {
                if self.pending.as_ref().is_some_and(|p| {
                    !Self::bound(p, owner, source)
                        || p.selection.is_some_and(|selection| selection != before)
                        || p.ready.is_some()
                }) {
                    self.clear();
                }
                false
            }
            Delivery::Retire => {
                self.clear();
                false
            }
        }
    }
    pub(super) fn observe_selection(
        &mut self,
        owner: u64,
        source: &AuthoredRichText,
        after: Option<GlobalSelection>,
    ) {
        let Some(p) = &self.pending else { return };
        if p.selection.is_some() {
            return;
        }
        let length = source.plain_text().chars().count();
        if !Self::bound(p, owner, source)
            || after.is_none_or(|s| s.anchor.min(s.focus) != 0 || s.anchor.max(s.focus) != length)
        {
            self.clear();
            return;
        }
        self.pending.as_mut().unwrap().selection = after;
    }
    pub(super) fn preview_owner(
        &self,
        source: &AuthoredRichText,
        before: GlobalSelection,
        buffer: &str,
    ) -> Option<usize> {
        let p = self.pending.as_ref()?;
        (p.source.wire().same_source_version(source.wire())
            && p.selection == Some(before)
            && p.ready.as_deref() == Some(buffer))
        .then_some(p.first.run_ordinal)
    }
    fn take(&mut self) -> Option<Pending> {
        self.pending.take().filter(|p| p.ready.is_some())
    }
}

impl AuthoredEditor {
    pub(in crate::worksheet_ui) fn retire_select_all(&mut self) {
        self.select_all.clear();
    }
    pub(super) fn begin_select_all_delivery(
        &mut self,
        owner: u64,
        before: GlobalSelection,
        events: &[egui::Event],
        owns_focus: bool,
        composing: bool,
    ) {
        if self
            .select_all
            .begin(owner, &self.source, before, events, owns_focus, composing)
        {
            // WPF SelectAll explicitly clears springload. Do not preserve a
            // restricted Backspace grant or a prior ordinary Run owner.
            self.affinity.clear();
        }
    }
    pub(super) fn commit_select_all(
        &mut self,
        owner: u64,
        text: &str,
        before: GlobalSelection,
        after: GlobalSelection,
        headroom: usize,
    ) -> Result<Option<AuthoredTextWire>, String> {
        let proof = self
            .select_all
            .take()
            .ok_or("Missing live SelectAll Text delivery")?;
        let text_input = proof.ready.as_deref().unwrap();
        let expected_caret = text_input.chars().count();
        if !SelectAllInput::bound(&proof, owner, &self.source)
            || proof.selection != Some(before)
            || text != text_input
            || after
                != (GlobalSelection {
                    anchor: expected_caret,
                    focus: expected_caret,
                })
        {
            return Err("Stale/mismatched SelectAll replacement".into());
        }
        let patch = self.source.replace_after_select_all(
            TextSelection {
                paragraph: 0,
                anchor: before.anchor,
                focus: before.focus,
            },
            text,
            CrossRunTypingContext {
                insertion_owner: proof.first.run_ordinal,
                expected_format: &proof.format,
                input_language: &proof.profile.language,
                input_flow_direction: proof.profile.flow,
                has_retained_springload: false, // actual SelectAll proved above
            },
        )?;
        let mut next = Self::checked(patch.source, headroom)?;
        let insertion_owner = patch
            .insertion_owner
            .ok_or("Missing SelectAll insertion owner")?;
        next.affinity.remember_run(&next.source, insertion_owner)?;
        if next.source.wire() == self.source.wire() {
            // Consume the proof/establish explicit owner, no visible-noop Undo.
            self.affinity = next.affinity;
            return Ok(None);
        }
        let wire = next.source.wire().clone();
        *self = next;
        Ok(Some(wire))
    }
}

impl Editor<'_> {
    pub(in crate::worksheet_ui) fn begin_select_all_delivery(
        &mut self,
        owner: u64,
        before: egui::text::CCursorRange,
        events: &[egui::Event],
        owns_focus: bool,
        composing: bool,
    ) {
        if let Self::Native(editor) = self {
            editor.begin_select_all_delivery(
                owner,
                rich_text_input::global(before),
                events,
                owns_focus,
                composing,
            );
        }
        if let Self::Authored(editor) = self {
            editor.begin_select_all_delivery(
                owner,
                rich_text_input::global(before),
                events,
                owns_focus,
                composing,
            );
        }
    }
    pub(in crate::worksheet_ui) fn observe_select_all_selection(
        &mut self,
        owner: u64,
        after: Option<egui::text::CCursorRange>,
    ) {
        if let Self::Native(editor) = self {
            editor.observe_select_all_selection(owner, after.map(rich_text_input::global));
        }
        if let Self::Authored(editor) = self {
            editor.select_all.observe_selection(
                owner,
                &editor.source,
                after.map(rich_text_input::global),
            );
        }
    }
    pub(in crate::worksheet_ui) fn has_select_all_replacement(&self) -> bool {
        match self {
            Self::Authored(editor) => editor.select_all.ready(),
            Self::Native(editor) => editor.has_select_all_replacement(),
        }
    }
}

#[cfg(test)]
#[path = "select_all_input_tests.rs"]
mod tests;
