//! Actual native-rich-Text focus and full-App ScrollArea, without a window.
use super::*;

fn native_text_scene() -> (PathBuf, WorksheetApp, egui::Context, Clock) {
    let root = temporary();
    let path = super::super::rich_text_integration_tests::native_fixture(&root);
    let mut app = WorksheetApp {
        automatic_calculation: false,
        ..Default::default()
    };
    app.open(&path);
    assert!(app.status.starts_with("Opened"), "{}", app.status);
    let owner = app.document.regions[0].id;
    app.document.add_text(0., 10_000., "Bottom");
    app.selected = Some(owner);
    app.focus_new = Some(owner);
    app.insertion = None;
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    let mut clock = Clock {
        frame: 0,
        native_ppp: 1.,
    };
    for index in 0..3 {
        frame(&mut app, &ctx, &mut clock, "warm", index, vec![]);
    }
    assert_eq!(app.editing_region, Some(owner));
    assert!(app.rich_text.editors.get(&owner).is_some_and(Result::is_ok));
    (root, app, ctx, clock)
}

#[test]
fn rich_text_wheel_burst_reaches_actual_scroll_area_without_drain_frames() {
    let (root, mut app, ctx, mut clock) = native_text_scene();
    let oracle = ReadOnlyOracle::new(&app);
    let mut events = vec![egui::Event::PointerMoved(pos2(600., 450.))];
    events.extend((0..64).map(|_| egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: vec2(0., -4.),
        phase: egui::TouchPhase::Move,
        modifiers: egui::Modifiers::NONE,
    }));
    let first = frame(&mut app, &ctx, &mut clock, "wheel_burst", 0, events);
    oracle.check(&app);
    assert!(
        app.plot_input_queue.is_empty(),
        "wheel burst queued behind one-event-per-frame publication"
    );
    let mut offset = first.offset.y;
    for index in 0..30 {
        let next = frame(&mut app, &ctx, &mut clock, "settle", index, vec![]);
        offset = next.offset.y;
        oracle.check(&app);
    }
    assert!(
        (offset - 256.).abs() < 1.,
        "whole native wheel distance must reach the ScrollArea: {offset}"
    );
    oracle.finish(&app);
    drop(app);
    drop(ctx);
    for name in ["original-rich-text.mcdx", "generated-plain.mcdx"] {
        std::fs::remove_file(root.join(name)).unwrap();
    }
    std::fs::remove_dir(root).unwrap();
}
