//! Command queue regressions; no app, dialogs, disk writes or calculation.
use super::*;

fn text(value: &str) -> Event {
    Event::Text(value.into())
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
fn deliver(queue: &mut PlotInputQueue, events: Vec<Event>, active: bool) -> (Vec<Event>, bool) {
    let mut raw = RawInput {
        events,
        ..Default::default()
    };
    let more = queue.prepare_editor_input(&mut raw, false, active, false);
    (raw.events, more)
}
fn commands() -> Vec<Event> {
    vec![
        key(Key::S, Modifiers::COMMAND, true),
        key(Key::S, Modifiers::COMMAND | Modifiers::SHIFT, true),
        key(Key::O, Modifiers::COMMAND, true),
        key(Key::N, Modifiers::COMMAND, true),
        key(Key::P, Modifiers::COMMAND, true),
        key(Key::Z, Modifiers::COMMAND, true),
        key(Key::Y, Modifiers::COMMAND, true),
        key(Key::F5, Modifiers::NONE, true),
        key(Key::F5, Modifiers::CTRL, true),
    ]
}

#[test]
fn plain_prefix_app_command_suffix_are_ordered_before_new_native_input() {
    for command in commands() {
        let mut queue = PlotInputQueue::default();
        assert_eq!(
            deliver(
                &mut queue,
                vec![text("before"), command.clone(), text("after")],
                true
            ),
            (vec![text("before")], true)
        );
        // Focus may change: stored modes and FIFO still retain the old command.
        assert_eq!(
            deliver(&mut queue, vec![text("newer")], false),
            (vec![command], true)
        );
        assert_eq!(
            deliver(&mut queue, vec![], false),
            (vec![text("after")], true)
        );
        assert_eq!(
            deliver(&mut queue, vec![], false),
            (vec![text("newer")], false)
        );
        assert!(queue.is_empty());
    }
}

#[test]
fn ordinary_plain_editing_and_complete_click_are_not_split() {
    let mut queue = PlotInputQueue::default();
    let input = vec![
        text("a"),
        key(Key::Backspace, Modifiers::NONE, true),
        key(Key::Enter, Modifiers::NONE, true),
        text("b"),
        key(Key::Delete, Modifiers::NONE, true),
        key(Key::Tab, Modifiers::SHIFT, true),
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
    assert_eq!(deliver(&mut queue, input.clone(), true), (input, false));
}

#[test]
fn releases_extra_modifiers_and_unrelated_shortcuts_do_not_admit_plain_queue() {
    for command in [
        key(Key::S, Modifiers::COMMAND, false),
        key(Key::S, Modifiers::COMMAND | Modifiers::ALT, true),
        key(Key::S, Modifiers::SHIFT, true),
        key(Key::O, Modifiers::COMMAND | Modifiers::SHIFT, true),
        key(Key::Z, Modifiers::COMMAND | Modifiers::SHIFT, true),
        key(Key::F5, Modifiers::SHIFT, true),
        key(Key::A, Modifiers::COMMAND, true),
    ] {
        let mut queue = PlotInputQueue::default();
        let input = vec![text("a"), command, text("b")];
        assert_eq!(deliver(&mut queue, input.clone(), true), (input, false));
    }
    for command in commands() {
        let mut queue = PlotInputQueue::default();
        let input = vec![text("a"), command, text("b")];
        assert_eq!(deliver(&mut queue, input.clone(), false), (input, false));
    }
}

#[test]
fn ime_publication_boundaries_survive_following_save_command_and_text() {
    let mut queue = PlotInputQueue::default();
    let ime = vec![
        Event::Ime(eframe::egui::ImeEvent::Preedit {
            text: "하".into(),
            active_range_chars: None,
        }),
        Event::Ime(eframe::egui::ImeEvent::Commit("한".into())),
    ];
    let save = key(Key::S, Modifiers::COMMAND, true);
    let input = ime
        .iter()
        .cloned()
        .chain([save.clone(), text("tail")])
        .collect();
    assert_eq!(deliver(&mut queue, input, true), (ime, true));
    assert_eq!(deliver(&mut queue, vec![], true), (vec![save], true));
    assert_eq!(
        deliver(&mut queue, vec![], true),
        (vec![text("tail")], false)
    );
}

#[test]
fn repeated_commands_and_benign_releases_keep_exact_fifo_without_reclassification() {
    let mut queue = PlotInputQueue::default();
    let first = key(Key::S, Modifiers::COMMAND, true);
    let release = key(Key::S, Modifiers::COMMAND, false);
    let hover = Event::PointerMoved(eframe::egui::pos2(7., 9.));
    let second = key(Key::Z, Modifiers::COMMAND, true);
    let input = vec![
        text("a"),
        first.clone(),
        release.clone(),
        hover.clone(),
        text("b"),
        second.clone(),
        text("c"),
    ];
    let expected = input.clone();
    let (mut actual, mut more) = deliver(&mut queue, input, true);
    assert_eq!(actual, vec![text("a")]);
    let mut count = 0;
    while more {
        let (events, pending) = deliver(&mut queue, vec![], false);
        if count == 0 {
            assert_eq!(events, vec![first.clone(), release.clone(), hover.clone()]);
        }
        actual.extend(events);
        more = pending;
        count += 1;
        assert!(count < 10);
    }
    assert_eq!(actual, expected);
    assert_eq!(count, 4);
}

#[test]
fn fresh_frame_metadata_is_not_replayed_and_reset_discards_old_document_tail() {
    let mut queue = PlotInputQueue::default();
    let save = key(Key::S, Modifiers::COMMAND, true);
    assert_eq!(
        deliver(&mut queue, vec![text("a"), save.clone(), text("b")], true),
        (vec![text("a")], true)
    );
    let screen =
        eframe::egui::Rect::from_min_size(eframe::egui::Pos2::ZERO, eframe::egui::vec2(800., 600.));
    let mut raw = RawInput {
        events: vec![],
        time: Some(19.25),
        predicted_dt: 0.125,
        screen_rect: Some(screen),
        focused: false,
        ..Default::default()
    };
    assert!(queue.prepare_editor_input(&mut raw, false, false, false));
    assert_eq!(raw.events, vec![save]);
    assert_eq!(raw.time, Some(19.25));
    assert_eq!(raw.predicted_dt, 0.125);
    assert_eq!(raw.screen_rect, Some(screen));
    assert!(!raw.focused);
    queue.clear();
    assert!(queue.is_empty());
    assert_eq!(
        deliver(&mut queue, vec![text("new document")], true),
        (vec![text("new document")], false)
    );
}
