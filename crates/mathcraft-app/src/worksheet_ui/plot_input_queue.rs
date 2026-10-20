//! Ordered field-input delivery before egui derives pointer/key/focus state.
//!
//! This queue does not execute commands or infer an owner. The app clears it on
//! document replacement and validates focus/model commands using the cursor
//! stored by the prior segmented field drawing.
//! `prepare` belongs in eframe::App::raw_input_hook, once per native input cycle,
//! not in a multipass paint callback. It changes events only, not window state.
use eframe::egui::{Event, Key, Modifiers, RawInput};
use std::collections::VecDeque;

#[derive(Default)]
pub(super) struct PlotInputQueue {
    pending: VecDeque<QueuedEvent>,
    rich_ime_confirmation: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Unrestricted,
    Plot,
    PlainText,
    RichText,
}

struct QueuedEvent {
    event: Event,
    mode: Mode,
    rich_ime_confirmation: bool,
}

fn exact(modifiers: Modifiers, expected: Modifiers) -> bool {
    modifiers.matches_exact(expected)
}

fn barrier(event: &Event) -> bool {
    match event {
        Event::Key {
            key,
            pressed: true,
            modifiers,
            ..
        } => {
            let plain = exact(*modifiers, Modifiers::NONE);
            let shift = exact(*modifiers, Modifiers::SHIFT);
            let command = exact(*modifiers, Modifiers::COMMAND);
            match key {
                Key::Enter | Key::Tab => plain || shift,
                Key::Backspace | Key::Delete => plain,
                // Cmd+Shift+Z is the original infinity gesture, never redo.
                Key::Z | Key::Y => command,
                Key::F5 => plain || exact(*modifiers, Modifiers::CTRL),
                Key::S => command || exact(*modifiers, Modifiers::COMMAND | Modifiers::SHIFT),
                Key::O | Key::N | Key::P => command,
                _ => false,
            }
        }
        Event::PointerButton { pressed: true, .. } => true,
        _ => false,
    }
}

fn changes_before_command(event: &Event) -> bool {
    // Releases, hover movement and modifier transitions do not edit a field.
    // Keep ordinary complete clicks/key presses in one delivery. Touch remains
    // conservative because it can encode its own selection/gesture transition.
    !matches!(
        event,
        Event::Key { pressed: false, .. }
            | Event::ModifiersChanged(_)
            | Event::PointerMoved(_)
            | Event::PointerGone
            | Event::PointerButton { pressed: false, .. }
    )
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PlainEvent {
    Ordinary,
    Ime,
    SurroundingDeletion,
}

fn plain_event(event: &Event) -> Option<PlainEvent> {
    if !changes_before_command(event) {
        return None;
    }
    Some(match event {
        Event::Ime(eframe::egui::ImeEvent::DeleteSurrounding { .. }) => {
            PlainEvent::SurroundingDeletion
        }
        Event::Ime(_) => PlainEvent::Ime,
        _ => PlainEvent::Ordinary,
    })
}

// Commands handled by the app before the plain TextEdit is drawn. Bare
// editing/navigation keys stay in the existing ordinary text publication.
fn plain_app_command(event: &Event) -> bool {
    let Event::Key {
        key,
        pressed: true,
        modifiers,
        ..
    } = event
    else {
        return false;
    };
    match key {
        Key::S => {
            exact(*modifiers, Modifiers::COMMAND)
                || exact(*modifiers, Modifiers::COMMAND | Modifiers::SHIFT)
        }
        Key::O | Key::N | Key::P | Key::T | Key::Z | Key::Y | Key::L | Key::E | Key::R | Key::J => {
            exact(*modifiers, Modifiers::COMMAND)
        }
        Key::F5 => exact(*modifiers, Modifiers::NONE) || exact(*modifiers, Modifiers::CTRL),
        _ => false,
    }
}

fn plain_app_command_segment_count(events: &[&Event]) -> usize {
    match events.iter().position(|event| plain_app_command(event)) {
        Some(index)
            if events
                .iter()
                .take(index)
                .any(|event| changes_before_command(event)) =>
        {
            index
        }
        Some(index) => {
            index
                + 1
                + events
                    .iter()
                    .skip(index + 1)
                    .take_while(|event| !changes_before_command(event))
                    .count()
        }
        None => events.len(),
    }
}

fn plain_segment_count(events: &[&Event]) -> usize {
    let mut current = None;
    for (index, event) in events.iter().enumerate() {
        let Some(kind) = plain_event(event) else {
            continue;
        };
        if let Some(previous) = current {
            // Surrounding deletion needs its own publication boundary even
            // between two IME updates. Harmless trailing events stay with it.
            if kind != previous || kind == PlainEvent::SurroundingDeletion {
                return index;
            }
        }
        current = Some(kind);
    }
    events.len()
}

fn plot_segment_count(events: &[&Event]) -> usize {
    let at = events.iter().position(|event| barrier(event));
    match at {
        Some(index)
            if matches!(events[index], Event::Key {key: Key::Enter, pressed: true, modifiers, ..} if modifiers.is_none())
                && events.iter().take(index).any(|event| {
                    matches!(event, Event::Ime(eframe::egui::ImeEvent::Commit(_)))
                }) =>
        {
            // Plot Enter accompanying an IME commit confirms composition.
            index
                + 1
                + events
                    .iter()
                    .skip(index + 1)
                    .take_while(|event| !changes_before_command(event))
                    .count()
        }
        Some(index)
            if events
                .iter()
                .take(index)
                .any(|event| changes_before_command(event)) =>
        {
            index
        }
        Some(index) => {
            index
                + 1
                + events
                    .iter()
                    .skip(index + 1)
                    .take_while(|event| !changes_before_command(event))
                    .count()
        }
        None => events.len(),
    }
}

impl PlotInputQueue {
    /// Discard deferred input only. The caller uses this when replacing a
    /// document; old tail events must never be reassigned to a new worksheet.
    pub(super) fn clear(&mut self) {
        self.pending.clear();
        self.rich_ime_confirmation = false;
    }

    /// Original native-batch provenance survives deferred delivery. A later
    /// native Enter must never inherit a confirmation from an earlier batch.
    pub(super) fn rich_ime_confirmation(&self) -> bool {
        self.rich_ime_confirmation
    }

    #[cfg(test)]
    pub(super) fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    /// Move older deferred events before fresh events, then deliver one ordered
    /// segment. A command follows a previously delivered mutation prefix; its
    /// benign releases/hover/modifiers stay with it, later mutations are deferred.
    /// Returns whether repaint is needed to drain
    /// remaining events. No events/strings are cloned, synthesized or dropped.
    ///
    /// Non-plot frames without a backlog pass through completely unchanged.
    /// Once queued, ordering continues even if focus changes out of the plot.
    /// Window/viewport metadata remains fresh; this is not a window-state replay.
    /// No fixed queue cap silently drops accepted OS input: callers must use an
    /// explicit admission/cancellation policy if a backlog limit is required.
    #[cfg_attr(not(test), allow(dead_code))] // Retain the original plot-only API.
    pub(super) fn prepare(&mut self, raw: &mut RawInput, active_plot: bool) -> bool {
        self.prepare_input(raw, active_plot, false)
    }

    /// Share the same FIFO between plot commands and plain-text composition.
    /// Plain input without an app command or IME is unchanged. Mixed input is
    /// split at IME/ordinary transitions, with each surrounding deletion alone,
    /// so the prior field drawing establishes the next segment's real cursor.
    /// Routing mode travels with queued events; changing focus never reclassifies
    /// an old tail as input from the newly focused field. The caller still owns
    /// widget routing and clears this queue when replacing the document.
    pub(super) fn prepare_input(
        &mut self,
        raw: &mut RawInput,
        active_plot: bool,
        active_plain_text: bool,
    ) -> bool {
        self.prepare_editor_input(raw, active_plot, active_plain_text, false)
    }

    pub(super) fn prepare_editor_input(
        &mut self,
        raw: &mut RawInput,
        active_plot: bool,
        active_plain_text: bool,
        active_rich_text: bool,
    ) -> bool {
        self.rich_ime_confirmation = false;
        if self.pending.is_empty()
            && !active_plot
            && !active_rich_text
            && (!active_plain_text
                || !raw
                    .events
                    .iter()
                    .any(|event| matches!(event, Event::Ime(_)) || plain_app_command(event)))
        {
            return false;
        }
        let mode = if active_plot {
            Mode::Plot
        } else if active_rich_text {
            Mode::RichText
        } else if active_plain_text {
            Mode::PlainText
        } else {
            Mode::Unrestricted
        };
        let mut preceding_ime = false;
        self.pending.extend(std::mem::take(&mut raw.events).into_iter().map(|event| {
            let rich_ime_confirmation = mode == Mode::RichText
                && preceding_ime
                && matches!(&event, Event::Key { key: Key::Enter, pressed: true, modifiers, .. } if modifiers.is_none());
            if changes_before_command(&event) {
                preceding_ime = matches!(event, Event::Ime(eframe::egui::ImeEvent::Commit(_) | eframe::egui::ImeEvent::Preedit { .. }));
            } else if matches!(event, Event::Key { key: Key::Enter, pressed: false, .. }) {
                preceding_ime = false;
            }
            QueuedEvent { event, mode, rich_ime_confirmation }
        }));
        if let Some(first) = self.pending.front() {
            let mode = first.mode;
            let events: Vec<_> = self
                .pending
                .iter()
                .take_while(|queued| queued.mode == mode)
                .map(|queued| &queued.event)
                .collect();
            let count = match mode {
                Mode::Unrestricted => events.len(),
                Mode::Plot => plot_segment_count(&events),
                Mode::PlainText => {
                    plain_segment_count(&events).min(plain_app_command_segment_count(&events))
                }
                Mode::RichText => {
                    // One editing/navigation boundary per delivery establishes
                    // the actual caret and source-owned Run before the next
                    // event. Consecutive IME updates retain the existing batch
                    // contract (Commit followed by Preedit is one publication).
                    if events.iter().find_map(|event| plain_event(event)) == Some(PlainEvent::Ime) {
                        plain_segment_count(&events)
                    } else {
                        let first = events
                            .iter()
                            .position(|event| changes_before_command(event));
                        first.map_or(events.len(), |first| {
                            first
                                + 1
                                + events
                                    .iter()
                                    .skip(first + 1)
                                    .take_while(|event| !changes_before_command(event))
                                    .count()
                        })
                    }
                }
            };
            for queued in self.pending.drain(..count) {
                self.rich_ime_confirmation |= queued.rich_ime_confirmation;
                raw.events.push(queued.event);
            }
        }
        !self.pending.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rich_delivery(queue: &mut PlotInputQueue, events: Vec<Event>) -> (Vec<Event>, bool, bool) {
        let mut raw = RawInput {
            events,
            ..Default::default()
        };
        let more = queue.prepare_editor_input(&mut raw, false, true, true);
        (raw.events, more, queue.rich_ime_confirmation())
    }

    #[test]
    fn rich_deferred_enter_keeps_only_its_original_batch_ime_confirmation() {
        let mut queue = PlotInputQueue::default();
        let commit = Event::Ime(eframe::egui::ImeEvent::Commit("한".into()));
        let enter = key(Key::Enter, Modifiers::NONE, true);
        assert_eq!(
            rich_delivery(&mut queue, vec![commit.clone(), enter.clone()]),
            (vec![commit], true, false)
        );
        // A fresh subsequent Enter follows the older tagged confirmation; it
        // must stay a separate untagged input even while the queue drains.
        assert_eq!(
            rich_delivery(&mut queue, vec![enter.clone()]),
            (vec![enter.clone()], true, true)
        );
        assert_eq!(
            rich_delivery(&mut queue, vec![]),
            (vec![enter], false, false)
        );
        assert_eq!(rich_delivery(&mut queue, vec![]), (vec![], false, false));
    }

    #[test]
    fn rich_confirmation_tag_stops_at_an_ordinary_edit_or_enter_release() {
        for boundary in [text("X"), key(Key::Enter, Modifiers::NONE, false)] {
            let mut queue = PlotInputQueue::default();
            let enter = key(Key::Enter, Modifiers::NONE, true);
            let events = vec![preedit("조합"), boundary, enter.clone()];
            let mut delivered = Vec::new();
            let (first, mut more, confirmation) = rich_delivery(&mut queue, events.clone());
            assert!(!confirmation);
            delivered.extend(first);
            while more {
                let (events, pending, confirmation) = rich_delivery(&mut queue, vec![]);
                assert!(!confirmation);
                delivered.extend(events);
                more = pending;
            }
            assert_eq!(delivered, events);
        }
    }

    #[test]
    fn rich_ime_confirmation_does_not_cross_a_native_input_batch_or_document_reset() {
        let mut queue = PlotInputQueue::default();
        let commit = Event::Ime(eframe::egui::ImeEvent::Commit("한".into()));
        let enter = key(Key::Enter, Modifiers::NONE, true);
        assert_eq!(
            rich_delivery(&mut queue, vec![commit.clone()]),
            (vec![commit.clone()], false, false)
        );
        assert_eq!(
            rich_delivery(&mut queue, vec![enter.clone()]),
            (vec![enter.clone()], false, false)
        );
        let _ = rich_delivery(&mut queue, vec![commit, enter.clone()]);
        queue.clear();
        assert_eq!(
            rich_delivery(&mut queue, vec![enter.clone()]),
            (vec![enter], false, false)
        );
    }
    fn key(key: Key, modifiers: Modifiers, pressed: bool) -> Event {
        Event::Key {
            key,
            physical_key: Some(key),
            pressed,
            repeat: false,
            modifiers,
        }
    }
    fn text(value: &str) -> Event {
        Event::Text(value.into())
    }
    fn preedit(value: &str) -> Event {
        Event::Ime(eframe::egui::ImeEvent::Preedit {
            text: value.into(),
            active_range_chars: None,
        })
    }
    fn deliver_modes(
        queue: &mut PlotInputQueue,
        events: Vec<Event>,
        plot: bool,
        plain: bool,
    ) -> (Vec<Event>, bool) {
        let mut raw = RawInput {
            events,
            ..Default::default()
        };
        let more = queue.prepare_input(&mut raw, plot, plain);
        (raw.events, more)
    }
    fn deliver(queue: &mut PlotInputQueue, events: Vec<Event>, active: bool) -> (Vec<Event>, bool) {
        let mut raw = RawInput {
            events,
            ..Default::default()
        };
        let more = queue.prepare(&mut raw, active);
        (raw.events, more)
    }
    #[test]
    fn prefix_command_suffix_use_three_deliveries_and_fresh_input_stays_last() {
        let mut q = PlotInputQueue::default();
        let command = key(Key::Enter, Modifiers::SHIFT, true);
        let input = vec![text("prefix"), command.clone(), text("suffix")];
        assert_eq!(deliver(&mut q, input, true), (vec![text("prefix")], true));
        assert_eq!(
            deliver(&mut q, vec![text("newer")], true),
            (vec![command], true)
        );
        assert_eq!(
            deliver(&mut q, vec![], true),
            (vec![text("suffix"), text("newer")], false)
        );
        assert!(q.is_empty());
    }
    #[test]
    fn keys_modifiers_releases_ime_and_pointer_events_preserve_exact_once_order() {
        let mut q = PlotInputQueue::default();
        let input = vec![
            Event::ModifiersChanged(Modifiers::SHIFT),
            text("α"),
            key(Key::Enter, Modifiers::SHIFT, true),
            key(Key::Enter, Modifiers::SHIFT, false),
            Event::ModifiersChanged(Modifiers::NONE),
            Event::Ime(eframe::egui::ImeEvent::Preedit {
                text: "가".into(),
                active_range_chars: None,
            }),
            Event::Ime(eframe::egui::ImeEvent::Commit("각".into())),
            Event::PointerMoved(eframe::egui::pos2(12., 34.)),
            Event::PointerButton {
                pos: eframe::egui::pos2(12., 34.),
                button: eframe::egui::PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::NONE,
            },
            Event::PointerButton {
                pos: eframe::egui::pos2(12., 34.),
                button: eframe::egui::PointerButton::Primary,
                pressed: false,
                modifiers: Modifiers::NONE,
            },
            key(Key::F5, Modifiers::NONE, true),
        ];
        let expected = input.clone();
        let (mut actual, mut more) = deliver(&mut q, input, true);
        let mut steps = 0;
        while more {
            let (events, pending) = deliver(&mut q, vec![], false);
            actual.extend(events);
            more = pending;
            steps += 1;
            assert!(steps < 20);
        }
        assert_eq!(actual, expected);
    }
    #[test]
    fn every_structural_global_and_pointer_command_is_separated_from_text() {
        for command in [
            key(Key::Enter, Modifiers::SHIFT, true),
            key(Key::Enter, Modifiers::NONE, true),
            key(Key::Backspace, Modifiers::NONE, true),
            key(Key::Delete, Modifiers::NONE, true),
            key(Key::Z, Modifiers::COMMAND, true),
            key(Key::Y, Modifiers::COMMAND, true),
            key(Key::Tab, Modifiers::NONE, true),
            key(Key::Tab, Modifiers::SHIFT, true),
            key(Key::F5, Modifiers::NONE, true),
            key(Key::F5, Modifiers::CTRL, true),
            key(Key::S, Modifiers::COMMAND, true),
            key(Key::S, Modifiers::COMMAND | Modifiers::SHIFT, true),
            key(Key::O, Modifiers::COMMAND, true),
            key(Key::N, Modifiers::COMMAND, true),
            key(Key::P, Modifiers::COMMAND, true),
        ] {
            let mut q = PlotInputQueue::default();
            assert_eq!(
                deliver(&mut q, vec![text("a"), command.clone(), text("b")], true),
                (vec![text("a")], true)
            );
            assert_eq!(deliver(&mut q, vec![], false), (vec![command], true));
            assert_eq!(deliver(&mut q, vec![], false), (vec![text("b")], false));
        }
    }
    #[test]
    fn select_all_and_infinity_are_not_reclassified_and_inactive_input_is_untouched() {
        let events = vec![
            key(Key::A, Modifiers::COMMAND, true),
            text("replacement"),
            key(Key::Z, Modifiers::COMMAND | Modifiers::SHIFT, true),
        ];
        let mut q = PlotInputQueue::default();
        assert_eq!(deliver(&mut q, events.clone(), true), (events, false));
        let events = vec![
            text("a"),
            key(Key::Enter, Modifiers::SHIFT, true),
            text("b"),
        ];
        assert_eq!(deliver(&mut q, events.clone(), false), (events, false));
        assert!(q.is_empty());
    }
    #[test]
    fn modifier_only_prefix_and_keyup_keep_physical_events_and_reset_discards_old_tail() {
        let mut q = PlotInputQueue::default();
        let pressed = Event::Key {
            key: Key::Z,
            physical_key: Some(Key::Y),
            pressed: true,
            repeat: true,
            modifiers: Modifiers {
                ctrl: true,
                command: true,
                ..Modifiers::NONE
            },
        };
        let before = vec![
            key(Key::A, Modifiers::NONE, false),
            Event::ModifiersChanged(Modifiers::COMMAND),
            pressed.clone(),
        ];
        let mut full = before.clone();
        full.push(text("old worksheet"));
        assert_eq!(deliver(&mut q, full, true), (before, true));
        q.clear();
        assert!(q.is_empty());
        assert_eq!(
            deliver(&mut q, vec![text("new worksheet")], false),
            (vec![text("new worksheet")], false)
        );
    }
    #[test]
    fn ordinary_click_hover_and_release_require_no_extra_delivery() {
        let pos = eframe::egui::pos2(12., 34.);
        let input = vec![
            Event::PointerMoved(pos),
            Event::PointerButton {
                pos,
                button: eframe::egui::PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::NONE,
            },
            Event::PointerButton {
                pos,
                button: eframe::egui::PointerButton::Primary,
                pressed: false,
                modifiers: Modifiers::NONE,
            },
            Event::PointerGone,
        ];
        let mut q = PlotInputQueue::default();
        assert_eq!(deliver(&mut q, input.clone(), true), (input, false));
        assert!(q.is_empty());
    }
    #[test]
    fn complete_keypress_keeps_release_and_modifiers_but_stops_before_next_mutation() {
        let input = vec![
            Event::ModifiersChanged(Modifiers::SHIFT),
            key(Key::Enter, Modifiers::SHIFT, true),
            key(Key::Enter, Modifiers::SHIFT, false),
            Event::ModifiersChanged(Modifiers::NONE),
            Event::PointerMoved(eframe::egui::pos2(0., 1.)),
        ];
        let mut q = PlotInputQueue::default();
        assert_eq!(deliver(&mut q, input.clone(), true), (input.clone(), false));
        let mut with_text = input.clone();
        with_text.push(text("new slot"));
        assert_eq!(deliver(&mut q, with_text, true), (input, true));
        assert_eq!(
            deliver(&mut q, vec![], true),
            (vec![text("new slot")], false)
        );
        let touch = Event::Touch {
            device_id: eframe::egui::TouchDeviceId(1),
            id: eframe::egui::TouchId(2),
            phase: eframe::egui::TouchPhase::Start,
            pos: eframe::egui::pos2(0., 1.),
            force: None,
        };
        assert!(changes_before_command(&touch));
    }
    #[test]
    fn ime_commit_keeps_its_confirmation_enter_but_not_a_following_command_or_text() {
        let mut q = PlotInputQueue::default();
        let first = vec![
            Event::Ime(eframe::egui::ImeEvent::Commit("값".into())),
            key(Key::Enter, Modifiers::NONE, true),
            key(Key::Enter, Modifiers::NONE, false),
        ];
        let mut events = first.clone();
        events.extend([text("later"), key(Key::F5, Modifiers::NONE, true)]);
        assert_eq!(deliver(&mut q, events, true), (first, true));
        assert_eq!(deliver(&mut q, vec![], true), (vec![text("later")], true));
        assert_eq!(
            deliver(&mut q, vec![], true),
            (vec![key(Key::F5, Modifiers::NONE, true)], false)
        );
        // A plain-text Enter remains a separate owner command.
        assert_eq!(
            deliver(
                &mut q,
                vec![text("x"), key(Key::Enter, Modifiers::NONE, true)],
                true
            ),
            (vec![text("x")], true)
        );
    }

    #[test]
    fn plain_text_prefix_preedit_cancel_suffix_keep_ordered_publication_boundaries() {
        let mut q = PlotInputQueue::default();
        let prefix = vec![
            key(Key::A, Modifiers::COMMAND, true),
            text("prefix"),
            key(Key::A, Modifiers::COMMAND, false),
        ];
        let composing = vec![
            preedit("하"),
            preedit(""),
            Event::Ime(eframe::egui::ImeEvent::Commit(String::new())),
        ];
        let suffix = vec![text("suffix"), key(Key::ArrowLeft, Modifiers::NONE, true)];
        let input: Vec<_> = prefix
            .iter()
            .chain(&composing)
            .chain(&suffix)
            .cloned()
            .collect();
        assert_eq!(
            deliver_modes(&mut q, input.clone(), false, true),
            (prefix.clone(), true)
        );
        assert_eq!(
            deliver_modes(&mut q, vec![], false, true),
            (composing.clone(), true)
        );
        assert_eq!(
            deliver_modes(&mut q, vec![], false, true),
            (suffix.clone(), false)
        );
        let actual: Vec<_> = prefix.into_iter().chain(composing).chain(suffix).collect();
        assert_eq!(actual, input);
    }

    #[test]
    fn plain_commit_next_preedit_precedes_navigation_but_keeps_benign_keyup() {
        let mut q = PlotInputQueue::default();
        let ime = vec![
            Event::Ime(eframe::egui::ImeEvent::Commit("한".into())),
            preedit("그"),
            key(Key::Enter, Modifiers::NONE, false),
            Event::ModifiersChanged(Modifiers::NONE),
            Event::PointerMoved(eframe::egui::pos2(3., 4.)),
        ];
        let normal = vec![key(Key::ArrowLeft, Modifiers::NONE, true), text("x")];
        let input: Vec<_> = ime.iter().chain(&normal).cloned().collect();
        assert_eq!(deliver_modes(&mut q, input, false, true), (ime, true));
        assert_eq!(deliver_modes(&mut q, vec![], false, true), (normal, false));
    }

    #[test]
    fn surrounding_deletions_each_have_their_own_segment_between_ime_and_text() {
        let mut q = PlotInputQueue::default();
        let first_delete = Event::Ime(eframe::egui::ImeEvent::DeleteSurrounding {
            before_chars: 0,
            after_chars: 1,
        });
        let second_delete = Event::Ime(eframe::egui::ImeEvent::DeleteSurrounding {
            before_chars: 2,
            after_chars: 0,
        });
        let after = vec![
            Event::Ime(eframe::egui::ImeEvent::Commit("글".into())),
            preedit("다"),
        ];
        let segments = [
            vec![preedit("그")],
            vec![first_delete, Event::ModifiersChanged(Modifiers::SHIFT)],
            vec![second_delete],
            after,
            vec![text("tail")],
        ];
        let input: Vec<_> = segments.iter().flatten().cloned().collect();
        let mut actual = vec![];
        for (index, expected) in segments.iter().enumerate() {
            let events = if index == 0 { input.clone() } else { vec![] };
            let (delivered, more) = deliver_modes(&mut q, events, false, true);
            assert_eq!(&delivered, expected);
            assert_eq!(more, index + 1 < segments.len());
            actual.extend(delivered);
        }
        assert_eq!(actual, input);
        assert!(q.is_empty());
    }

    #[test]
    fn ordinary_only_plain_text_and_complete_clicks_need_no_extra_frame() {
        let mut q = PlotInputQueue::default();
        let input = vec![
            key(Key::A, Modifiers::COMMAND, true),
            text("replacement"),
            key(Key::End, Modifiers::SHIFT, true),
            Event::PointerMoved(eframe::egui::pos2(3., 4.)),
            Event::PointerButton {
                pos: eframe::egui::pos2(3., 4.),
                button: eframe::egui::PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::NONE,
            },
            Event::PointerButton {
                pos: eframe::egui::pos2(3., 4.),
                button: eframe::egui::PointerButton::Primary,
                pressed: false,
                modifiers: Modifiers::NONE,
            },
        ];
        assert_eq!(
            deliver_modes(&mut q, input.clone(), false, true),
            (input, false)
        );
        assert!(q.is_empty());
    }

    #[test]
    fn queued_plain_mode_drains_before_fresh_plot_mode_after_focus_changes() {
        let mut q = PlotInputQueue::default();
        let old = vec![text("old prefix"), preedit("하"), text("old suffix")];
        assert_eq!(
            deliver_modes(&mut q, old, false, true),
            (vec![text("old prefix")], true)
        );
        let command = key(Key::Enter, Modifiers::SHIFT, true);
        let fresh = vec![text("plot prefix"), command.clone(), text("new slot")];
        assert_eq!(
            deliver_modes(&mut q, fresh, true, false),
            (vec![preedit("하")], true)
        );
        assert_eq!(
            deliver_modes(&mut q, vec![], false, false),
            (vec![text("old suffix")], true)
        );
        assert_eq!(
            deliver_modes(&mut q, vec![], false, false),
            (vec![text("plot prefix")], true)
        );
        assert_eq!(
            deliver_modes(&mut q, vec![], false, false),
            (vec![command], true)
        );
        assert_eq!(
            deliver_modes(&mut q, vec![], false, false),
            (vec![text("new slot")], false)
        );
    }

    #[test]
    fn shared_queue_keeps_old_plot_mode_then_new_plain_ime_and_fresh_window_metadata() {
        let mut q = PlotInputQueue::default();
        let command = key(Key::Enter, Modifiers::SHIFT, true);
        assert_eq!(
            deliver_modes(
                &mut q,
                vec![text("plot"), command.clone(), text("plot tail")],
                true,
                false
            ),
            (vec![text("plot")], true)
        );
        let plain = vec![preedit("가"), text("plain")];
        let mut raw = RawInput {
            events: plain,
            time: Some(42.),
            focused: false,
            screen_rect: Some(eframe::egui::Rect::from_min_size(
                eframe::egui::pos2(1., 2.),
                eframe::egui::vec2(500., 300.),
            )),
            ..Default::default()
        };
        let expected_rect = raw.screen_rect;
        assert!(q.prepare_input(&mut raw, false, true));
        assert_eq!(raw.events, vec![command]);
        assert_eq!(raw.time, Some(42.));
        assert!(!raw.focused);
        assert_eq!(raw.screen_rect, expected_rect);
        assert_eq!(
            deliver_modes(&mut q, vec![], false, true),
            (vec![text("plot tail")], true)
        );
        assert_eq!(
            deliver_modes(&mut q, vec![], false, true),
            (vec![preedit("가")], true)
        );
        q.clear();
        assert!(q.is_empty());
        assert_eq!(
            deliver_modes(&mut q, vec![text("new document")], false, false),
            (vec![text("new document")], false)
        );
    }
}

#[cfg(test)]
#[path = "plain_app_command_queue_tests.rs"]
mod plain_app_command_queue_tests;
