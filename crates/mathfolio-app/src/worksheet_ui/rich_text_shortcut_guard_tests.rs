//! Shortcut routing must not admit/parse a source for unrelated input.
use super::*;

fn key(pressed: bool, modifiers: egui::Modifiers) -> egui::Event {
    egui::Event::Key {
        key: egui::Key::B,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers,
    }
}

fn focused_route(events: Vec<egui::Event>) -> (WorksheetApp, Vec<egui::Event>) {
    let ctx = egui::Context::default();
    let mut app = WorksheetApp::default();
    let id = app
        .document
        .add_text(0., 0., "source not requested by unrelated input");
    app.document.regions[0].source_region = Some("1".into());
    // Deliberately not a ZIP. An attempted admission leaves a cached error,
    // making otherwise invisible unnecessary context preparation observable.
    app.document.native_archive = Some(vec![1, 2, 3]);
    app.selected = Some(id);
    let mut remaining = Vec::new();
    let output = ctx.run_ui(
        egui::RawInput {
            events,
            ..Default::default()
        },
        |ui| {
            ui.memory_mut(|memory| memory.request_focus(egui::Id::new(("worksheet-text", id))));
            app.rich_text_shortcuts(ui.ctx());
            remaining = ui.input(|input| input.events.clone());
        },
    );
    output.drop_without_applying_deltas();
    (app, remaining)
}

#[test]
fn unrelated_focused_text_input_does_not_prepare_a_formatting_context() {
    for events in [
        vec![],
        vec![egui::Event::PointerMoved(egui::pos2(20., 30.))],
        vec![egui::Event::Ime(egui::ImeEvent::Preedit {
            text: "조합".into(),
            active_range_chars: Some(0..2),
        })],
        vec![key(false, egui::Modifiers::COMMAND)],
        vec![key(true, egui::Modifiers::NONE)],
        vec![key(true, egui::Modifiers::COMMAND | egui::Modifiers::SHIFT)],
        vec![key(true, egui::Modifiers::COMMAND | egui::Modifiers::ALT)],
    ] {
        let (app, remaining) = focused_route(events.clone());
        assert!(
            app.rich_text.package.is_none(),
            "unrelated input admitted a package: {events:?}"
        );
        assert!(app.rich_text.editors.is_empty());
        assert_eq!(remaining, events);
        assert!(app.undo.is_empty() && app.redo.is_empty() && !app.dirty);
        assert!(app.last_executed_regions.is_empty());
    }
}

#[test]
fn exact_format_key_still_prepares_live_context_and_keeps_ineligible_event_unconsumed() {
    let events = vec![key(true, egui::Modifiers::COMMAND)];
    let (app, remaining) = focused_route(events.clone());
    assert!(app.rich_text.package.as_ref().is_some_and(Result::is_err));
    assert!(app.rich_text.editors.values().all(Result::is_err));
    assert_eq!(remaining, events);
    assert!(app.undo.is_empty() && app.redo.is_empty() && !app.dirty);
    assert!(app.last_executed_regions.is_empty());
}
