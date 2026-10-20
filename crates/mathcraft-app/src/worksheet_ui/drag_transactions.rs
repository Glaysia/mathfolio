//! Native-window-free pointer transactions. Widget bounds come from egui's
//! interaction registry rather than a screenshot or assumed screen placement.
use super::*;

fn frame(app: &mut WorksheetApp, ctx: &egui::Context, events: Vec<egui::Event>) {
    let output = ctx.run_ui(
        egui::RawInput {
            events,
            screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1800., 1400.))),
            ..Default::default()
        },
        |ui| {
            app.chrome_ui(ui);
            egui::CentralPanel::default().show(ui, |ui| app.workspace_ui(ui));
        },
    );
    output.drop_without_applying_deltas();
}

fn source(app: &mut WorksheetApp, id: u64, text: &str) {
    let mut editor = Editor::new();
    formula::input_text(&mut editor, text);
    app.document
        .regions
        .iter_mut()
        .find(|r| r.id == id)
        .unwrap()
        .content = CanvasContent::Math(editor.to_portable().unwrap());
    app.editors.insert(id, editor);
}

fn settle(app: &mut WorksheetApp) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while app.background.running {
        app.poll_background_calculation();
        assert!(std::time::Instant::now() < deadline);
        std::thread::yield_now();
    }
}

fn fixture(zoom: f32) -> (WorksheetApp, egui::Context, u64, u64) {
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    let mut app = WorksheetApp {
        zoom,
        automatic_calculation: false,
        ..Default::default()
    };
    let moving = app.document.add_math(75., 75.);
    source(&mut app, moving, "a:=2");
    let other = app.document.add_math(100., 200.);
    source(&mut app, other, "b:=3");
    app.selected = Some(moving);
    app.enable_background_calculation(ctx.clone());
    app.calculation_changed(false);
    settle(&mut app);
    app.dirty = false;
    frame(&mut app, &ctx, vec![]);
    frame(&mut app, &ctx, vec![]);
    (app, ctx, moving, other)
}

fn button(point: egui::Pos2, pressed: bool) -> Vec<egui::Event> {
    vec![
        egui::Event::PointerMoved(point),
        egui::Event::PointerButton {
            pos: point,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        },
    ]
}

fn move_handle(ctx: &egui::Context, id: u64) -> egui::Pos2 {
    let response = ctx.read_response(egui::Id::new(("move", id))).unwrap();
    ctx.layer_transform_to_global(response.layer_id)
        .unwrap_or_default()
        * response.rect.center()
}

fn position(app: &WorksheetApp, id: u64) -> (f64, f64) {
    let region = app.document.regions.iter().find(|r| r.id == id).unwrap();
    (region.left, region.top)
}

fn snap(v: f64, grid: f64) -> f64 {
    let r = v % grid;
    if r > grid / 2. { v + grid - r } else { v - r }
}

#[test]
fn blank_press_does_not_capture_document_history_or_submit_work() {
    let (mut app, ctx, _, _) = fixture(1.);
    let signature = app.document.active_calculation_signature();
    let generation = app.background.submitted_generation();
    for pressed in [true, false] {
        frame(&mut app, &ctx, button(pos2(650., 600.), pressed));
        assert!(app.pointer_history.is_none());
        assert!(app.undo.is_empty());
        assert!(!app.dirty);
        assert_eq!(app.background.submitted_generation(), generation);
        assert_eq!(app.document.active_calculation_signature(), signature);
    }
}

#[test]
fn move_deltas_preview_only_and_drop_commits_exactly_one_transaction_at_both_zooms() {
    for zoom in [1., 2.] {
        let (mut app, ctx, moving, _) = fixture(zoom);
        let initial = position(&app, moving);
        let signature = app.document.active_calculation_signature();
        let generation = app.background.submitted_generation();
        let handle = move_handle(&ctx, moving);
        frame(&mut app, &ctx, button(handle, true));
        let before = app
            .pointer_history
            .as_ref()
            .expect("handle press captures one region");
        assert!(matches!(&before.document, DocumentHistory::Region(region) if region.id == moving));
        assert!(!app.dirty);
        for delta in [vec2(20., 12.), vec2(45., 27.), vec2(80., 54.)] {
            frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(handle + delta)],
            );
            assert!(app.drag_preview.is_some(), "zoom {zoom}");
            assert_eq!(position(&app, moving), initial);
            assert_eq!(app.document.active_calculation_signature(), signature);
            assert!(!app.dirty);
            assert!(app.undo.is_empty());
            assert_eq!(app.background.submitted_generation(), generation);
            assert!(!app.background.running);
        }
        let target = handle + vec2(80., 54.);
        frame(&mut app, &ctx, button(target, false));
        let expected = (
            snap(initial.0 + 80. / f64::from(zoom), app.document.grid),
            snap(initial.1 + 54. / f64::from(zoom), app.document.grid),
        );
        let actual = position(&app, moving);
        assert!(
            (actual.0 - expected.0).abs() < 1e-4,
            "zoom {zoom}: {actual:?} vs {expected:?}"
        );
        assert!(
            (actual.1 - expected.1).abs() < 1e-4,
            "zoom {zoom}: {actual:?} vs {expected:?}"
        );
        assert!(app.drag_preview.is_none());
        assert!(app.dragging.is_none());
        assert!(app.pointer_history.is_none());
        assert!(app.dirty);
        assert_eq!(app.undo.len(), 1);
        assert!(matches!(app.undo[0].document, DocumentHistory::Region(_)));
        assert_eq!(app.background.submitted_generation(), generation + 1);
        assert_ne!(app.document.active_calculation_signature(), signature);
        frame(&mut app, &ctx, vec![]);
        assert_eq!(app.undo.len(), 1);
        assert_eq!(app.background.submitted_generation(), generation + 1);
        settle(&mut app);
    }
}

#[test]
fn move_undo_and_redo_preserve_unrelated_source_and_editor() {
    let (mut app, ctx, moving, other) = fixture(1.);
    let initial = position(&app, moving);
    let handle = move_handle(&ctx, moving);
    frame(&mut app, &ctx, button(handle, true));
    let target = handle + vec2(80., 54.);
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(target)]);
    frame(&mut app, &ctx, button(target, false));
    let moved = position(&app, moving);
    settle(&mut app);
    source(&mut app, other, "b:=47");
    let revision = app.editors[&other].arena().revision();
    app.undo();
    assert_eq!(position(&app, moving), initial);
    assert_eq!(app.editors[&other].arena().revision(), revision);
    assert!(
        matches!(
            app.redo.last().unwrap().document,
            DocumentHistory::Region(_)
        ),
        "the inverse of one moved region must remain a region history"
    );
    source(&mut app, other, "b:=91");
    let other_source = app.editors[&other].to_portable().unwrap();
    let revision = app.editors[&other].arena().revision();
    app.redo();
    assert_eq!(position(&app, moving), moved);
    assert_eq!(app.editors[&other].arena().revision(), revision);
    let CanvasContent::Math(tree) = &app
        .document
        .regions
        .iter()
        .find(|r| r.id == other)
        .unwrap()
        .content
    else {
        panic!()
    };
    assert_eq!(*tree, other_source);
    settle(&mut app);
}

#[test]
fn final_release_position_is_committed_even_without_a_separate_move_frame() {
    for zoom in [1., 2.] {
        let (mut app, ctx, moving, _) = fixture(zoom);
        let initial = position(&app, moving);
        let handle = move_handle(&ctx, moving);
        frame(&mut app, &ctx, button(handle, true));
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(handle + vec2(20., 12.))],
        );
        // The final position arrives together with release, beyond the last
        // rendered preview. Dropping the last input delta would leave it short.
        frame(&mut app, &ctx, button(handle + vec2(80., 54.), false));
        let expected = (
            snap(initial.0 + 80. / f64::from(zoom), app.document.grid),
            snap(initial.1 + 54. / f64::from(zoom), app.document.grid),
        );
        let actual = position(&app, moving);
        assert!(
            (actual.0 - expected.0).abs() < 1e-4 && (actual.1 - expected.1).abs() < 1e-4,
            "zoom {zoom}: release delta lost: {actual:?} vs {expected:?}"
        );
        assert_eq!(app.undo.len(), 1);
        settle(&mut app);
    }
}

#[test]
fn press_and_first_drag_coalesced_in_one_frame_keep_region_history() {
    let (mut app, ctx, moving, _) = fixture(1.);
    let initial = position(&app, moving);
    let generation = app.background.submitted_generation();
    let handle = move_handle(&ctx, moving);
    let mut events = button(handle, true);
    events.push(egui::Event::PointerMoved(handle + vec2(80., 54.)));
    frame(&mut app, &ctx, events);
    assert!(
        app.dragging.is_some(),
        "ordered same-frame press/move must start dragging"
    );
    assert!(
        matches!(&app.pointer_history.as_ref().expect("coalesced start captures history").document,
        DocumentHistory::Region(region) if region.id == moving)
    );
    assert_eq!(position(&app, moving), initial);
    assert_eq!(app.background.submitted_generation(), generation);
    frame(&mut app, &ctx, button(handle + vec2(80., 54.), false));
    assert_ne!(position(&app, moving), initial);
    assert_eq!(app.undo.len(), 1);
    assert!(matches!(app.undo[0].document, DocumentHistory::Region(_)));
    assert_eq!(app.background.submitted_generation(), generation + 1);
    settle(&mut app);
}

#[test]
#[ignore = "CPU-only held-drag workload, no native window"]
fn benchmark_4k_drag_deltas_do_not_prepare_calculation() {
    let (mut app, ctx, moving, _) = fixture(1.);
    for i in 2..4000 {
        let id = app.document.add_math(75., i as f64 * 35.);
        source(&mut app, id, &format!("v{i}:={i}"));
    }
    app.calculation_changed(false);
    settle(&mut app);
    app.dirty = false;
    frame(&mut app, &ctx, vec![]);
    let initial = position(&app, moving);
    let signature = app.document.active_calculation_signature();
    let generation = app.background.submitted_generation();
    let handle = move_handle(&ctx, moving);
    let started = std::time::Instant::now();
    frame(&mut app, &ctx, button(handle, true));
    let press = started.elapsed().as_micros();
    let started = std::time::Instant::now();
    for i in 0..60 {
        let delta = vec2(20. + (i % 20) as f32, 15. + (i % 10) as f32);
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(handle + delta)],
        );
    }
    let deltas = started.elapsed().as_micros();
    assert_eq!(app.document.active_calculation_signature(), signature);
    assert_eq!(app.background.submitted_generation(), generation);
    assert_eq!(position(&app, moving), initial);
    assert!(app.undo.is_empty());
    assert!(!app.dirty);
    let started = std::time::Instant::now();
    frame(&mut app, &ctx, button(handle + vec2(39., 24.), false));
    let drop_time = started.elapsed().as_micros();
    assert_eq!(app.background.submitted_generation(), generation + 1);
    assert_eq!(app.undo.len(), 1);
    settle(&mut app);
    println!(
        "DRAG_TRANSACTION regions=4000 press_us={press} delta_frames=60 deltas_us={deltas} drop_us={drop_time} intermediate_submissions=0 drop_submissions=1"
    );
}
