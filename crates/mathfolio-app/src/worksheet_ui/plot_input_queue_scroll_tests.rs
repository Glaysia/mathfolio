use super::*;

fn wheel(n: usize) -> Event {
    Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: egui::vec2(0., -(n as f32 + 1.)),
        phase: egui::TouchPhase::Move,
        modifiers: Modifiers::NONE,
    }
}

use eframe::egui;

fn deliver(queue: &mut PlotInputQueue, events: Vec<Event>) -> (Vec<Event>, bool, bool) {
    let mut raw = RawInput {
        events,
        time: Some(12.5),
        ..Default::default()
    };
    let more = queue.prepare_editor_input(&mut raw, false, true, true);
    assert_eq!(raw.time, Some(12.5));
    (raw.events, more, queue.rich_ime_confirmation())
}

#[test]
fn repeated_rich_text_wheel_packets_do_not_accumulate_frame_backlog() {
    let mut queue = PlotInputQueue::default();
    for packet in 0..4 {
        let mut events = vec![Event::PointerMoved(egui::pos2(600., 450.))];
        events.extend((packet * 24..(packet + 1) * 24).map(wheel));
        assert_eq!(deliver(&mut queue, events.clone()), (events, false, false));
        assert!(queue.is_empty());
    }
}

#[test]
fn rich_wheel_run_keeps_ime_enter_and_click_publication_boundaries() {
    let mut queue = PlotInputQueue::default();
    let ime = Event::Ime(egui::ImeEvent::Commit("한".into()));
    let enter = Event::Key {
        key: Key::Enter,
        physical_key: Some(Key::Enter),
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    };
    let click = Event::PointerButton {
        pos: egui::pos2(600., 450.),
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    };
    let wheels: Vec<_> = (0..12).map(wheel).collect();
    let text = Event::Text("X".into());
    let mut events = vec![ime.clone()];
    events.extend(wheels.clone());
    events.extend([enter.clone(), click.clone(), text.clone()]);
    assert_eq!(deliver(&mut queue, events), (vec![ime], true, false));
    assert_eq!(deliver(&mut queue, vec![]), (wheels, true, false));
    // The wheel run still separates native IME Commit from Enter: the old
    // confirmation provenance is retained, rather than inventing a new tag.
    assert_eq!(deliver(&mut queue, vec![]), (vec![enter], true, false));
    assert_eq!(deliver(&mut queue, vec![]), (vec![click], true, false));
    assert_eq!(deliver(&mut queue, vec![]), (vec![text], false, false));
    assert!(queue.is_empty());
}
