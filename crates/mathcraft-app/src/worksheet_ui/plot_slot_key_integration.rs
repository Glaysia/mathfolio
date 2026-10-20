//! Ordered actual-key authoring, curve identities and the controller commit.
use super::*;
use mathcraft_document::canvas::{PlotSource, PlotTraceAppearance};

fn spec(app: &WorksheetApp, id: u64) -> &PlotSpec {
    let CanvasContent::Plot(p) = &app
        .document
        .regions
        .iter()
        .find(|r| r.id == id)
        .unwrap()
        .content
    else {
        panic!()
    };
    p
}
fn key(key: egui::Key, modifiers: egui::Modifiers) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    }
}
fn fixture() -> (WorksheetApp, egui::Context, u64) {
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    let mut app = WorksheetApp {
        automatic_calculation: false,
        ..Default::default()
    };
    let x = app.document.add_math(20., 20.);
    app.document.regions.iter_mut().find(|r| r.id == x).unwrap().content = CanvasContent::Math(worksheet_math::tree_from_mathml("<ml:define><ml:id>x</ml:id><ml:matrix rows='3' cols='1'><ml:real>1</ml:real><ml:real>2</ml:real><ml:real>3</ml:real></ml:matrix></ml:define>").unwrap());
    let id = app.document.add_plot(20., 100.);
    let CanvasContent::Plot(p) = &mut app
        .document
        .regions
        .iter_mut()
        .find(|r| r.id == id)
        .unwrap()
        .content
    else {
        panic!()
    };
    p.source = PlotSource::axis_lists(
        vec!["x".into()],
        vec!["2*x".into(), "5*x".into()],
        vec![PlotTraceAppearance::default(); 2],
    )
    .unwrap();
    app.recalculate();
    app.persist_calculation_cache();
    app.restore_saved_calculations();
    app.last_executed_regions.clear();
    app.selected = Some(id);
    app.focus_new = Some(id);
    app.insertion = None;
    frame(&mut app, &ctx, vec![]);
    frame(&mut app, &ctx, vec![]);
    (app, ctx, id)
}
// Same raw hook and pre-page command order as the production app. No native
// window/frame, file dialog, screenshots or original target code is executed.
fn frame(app: &mut WorksheetApp, ctx: &egui::Context, events: Vec<egui::Event>) {
    let mut events = events;
    loop {
        let mut raw = egui::RawInput {
            events,
            screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1200., 900.))),
            ..Default::default()
        };
        app.prepare_plot_input(ctx, &mut raw);
        let output = ctx.run_ui(raw, |ui| {
            if ui.input_mut(|i| consume_exact_key(i, egui::Modifiers::COMMAND, egui::Key::S)) {
                assert!(app.path.is_some(), "test save must never open a dialog");
                app.save(false);
            }
            app.global_editing_shortcuts(ui.ctx());
            app.document_history_shortcuts(ui.ctx());
            app.page_ui(ui);
        });
        output.drop_without_applying_deltas();
        if app.plot_input_queue.is_empty() {
            break;
        }
        events = vec![];
    }
}
fn insert_batch() -> Vec<egui::Event> {
    vec![
        key(egui::Key::A, egui::Modifiers::COMMAND),
        egui::Event::Text("3*x".into()),
        key(egui::Key::Enter, egui::Modifiers::SHIFT),
        egui::Event::Text("4*x".into()),
    ]
}
fn assert_factors(app: &WorksheetApp, id: u64) {
    let data = app.plot_results[&id].as_ref().unwrap();
    assert_eq!(data.traces.len(), 3);
    assert!(!data.has_pending() && !data.has_failures());
    for (trace, factor) in data.traces.iter().zip([3., 4., 5.]) {
        let curve = trace.curve().unwrap();
        assert_eq!(curve.points.len(), 3);
        assert!(curve.points.iter().all(|p| p[1] == factor * p[0]));
    }
}
#[test]
fn batched_text_insert_text_is_staged_once_and_retains_only_surviving_pair_curves() {
    let (mut app, ctx, id) = fixture();
    let original = app.document.clone();
    let curves = app.plot_results[&id].clone();
    let generation = app.background.submitted_generation();
    frame(&mut app, &ctx, insert_batch());
    assert_eq!(app.document, original);
    assert_eq!(app.plot_results[&id], curves);
    assert!(app.undo.is_empty() && app.last_executed_regions.is_empty());
    assert_eq!(app.background.submitted_generation(), generation);
    let draft = app.inline_plots[&id].draft().unwrap().axis_lists().unwrap();
    assert_eq!(
        draft
            .y
            .iter()
            .map(|e| e.source.as_legacy_text().unwrap())
            .collect::<Vec<_>>(),
        ["3*x", "4*x", "5*x"]
    );
    let old = spec(&app, id).axis_lists().unwrap();
    assert_eq!(draft.y[0].id, old.y[0].id);
    assert_eq!(draft.y[2].id, old.y[1].id);
    assert_ne!(draft.y[1].id, old.y[0].id);
    frame(
        &mut app,
        &ctx,
        vec![key(egui::Key::Enter, egui::Modifiers::NONE)],
    );
    assert_eq!(app.undo.len(), 1);
    assert!(app.last_executed_regions.is_empty());
    let data = app.plot_results[&id].as_ref().unwrap();
    assert!(data.has_pending());
    assert_eq!(data.traces.len(), 3);
    let prior = curves.as_ref().unwrap();
    assert_eq!(data.traces[0].curve(), prior.traces[0].curve());
    assert!(data.traces[1].curve().is_none());
    assert_eq!(data.traces[2].curve(), prior.traces[1].curve());
    app.recalculate();
    assert_factors(&app, id);
}
#[test]
fn resize_commits_structural_draft_with_retained_pair_identity_in_one_history_entry() {
    let (mut app, ctx, id) = fixture();
    let before = spec(&app, id).clone();
    let prior = app.plot_results[&id].clone();
    frame(&mut app, &ctx, insert_batch());
    app.begin_plot_resize(id, before);
    assert_eq!(spec(&app, id).axis_lists().unwrap().y.len(), 3);
    assert_eq!(app.undo.len(), 1);
    assert!(app.last_executed_regions.is_empty());
    let data = app.plot_results[&id].as_ref().unwrap();
    let prior = prior.as_ref().unwrap();
    assert!(data.has_pending());
    assert_eq!(data.traces[0].curve(), prior.traces[0].curve());
    assert!(data.traces[1].curve().is_none());
    assert_eq!(data.traces[2].curve(), prior.traces[1].curve());
    app.undo();
    assert_eq!(spec(&app, id).axis_lists().unwrap().y.len(), 2);
}
#[test]
fn deleted_owner_history_keeps_the_cancelled_equation_id_floor() {
    let (mut app, ctx, id) = fixture();
    frame(
        &mut app,
        &ctx,
        vec![
            key(egui::Key::End, egui::Modifiers::NONE),
            key(egui::Key::Enter, egui::Modifiers::SHIFT),
        ],
    );
    let cancelled = app.inline_plots[&id]
        .draft()
        .unwrap()
        .axis_lists()
        .unwrap()
        .y[1]
        .id;
    frame(
        &mut app,
        &ctx,
        vec![key(egui::Key::Escape, egui::Modifiers::NONE)],
    );
    app.selected = Some(id);
    app.remove_selected();
    assert!(app.plot_equation_floors.contains_key(&id));
    app.undo();
    app.selected = Some(id);
    app.focus_new = Some(id);
    frame(&mut app, &ctx, vec![]);
    frame(&mut app, &ctx, vec![]);
    frame(
        &mut app,
        &ctx,
        vec![
            key(egui::Key::End, egui::Modifiers::NONE),
            key(egui::Key::Enter, egui::Modifiers::SHIFT),
        ],
    );
    assert!(
        app.inline_plots[&id]
            .draft()
            .unwrap()
            .axis_lists()
            .unwrap()
            .y[1]
            .id
            > cancelled
    );
}
#[test]
fn structural_undo_redo_keeps_earlier_text_and_committed_undo_never_reuses_new_id() {
    let (mut app, ctx, id) = fixture();
    frame(
        &mut app,
        &ctx,
        vec![
            key(egui::Key::A, egui::Modifiers::COMMAND),
            egui::Event::Text("3*x".into()),
        ],
    );
    frame(
        &mut app,
        &ctx,
        vec![key(egui::Key::Enter, egui::Modifiers::SHIFT)],
    );
    let first = app.inline_plots[&id].draft().unwrap().clone();
    let first_id = first.axis_lists().unwrap().y[1].id;
    let first_style = first.axis_lists().unwrap().appearances[1].style;
    frame(
        &mut app,
        &ctx,
        vec![key(egui::Key::Z, egui::Modifiers::COMMAND)],
    );
    let draft = app.inline_plots[&id].draft().unwrap().axis_lists().unwrap();
    assert_eq!(draft.y.len(), 2);
    assert_eq!(draft.y[0].source.as_legacy_text(), Some("3*x"));
    frame(
        &mut app,
        &ctx,
        vec![key(egui::Key::Y, egui::Modifiers::COMMAND)],
    );
    assert_eq!(app.inline_plots[&id].draft(), Some(&first));
    frame(
        &mut app,
        &ctx,
        vec![
            egui::Event::Text("4*x".into()),
            key(egui::Key::Enter, egui::Modifiers::NONE),
        ],
    );
    let committed = spec(&app, id).clone();
    assert_eq!(app.undo.len(), 1);
    app.undo();
    assert_eq!(spec(&app, id).axis_lists().unwrap().y.len(), 2);
    app.redo();
    assert_eq!(spec(&app, id), &committed);
    app.undo();
    app.selected = Some(id);
    app.focus_new = Some(id);
    frame(&mut app, &ctx, vec![]);
    frame(&mut app, &ctx, vec![]);
    frame(
        &mut app,
        &ctx,
        vec![
            key(egui::Key::End, egui::Modifiers::NONE),
            key(egui::Key::Enter, egui::Modifiers::SHIFT),
        ],
    );
    let next = app.inline_plots[&id].draft().unwrap().axis_lists().unwrap();
    assert!(next.y[1].id > first_id);
    assert_ne!(next.appearances[1].style.color, first_style.color);
    assert_eq!(app.undo.len(), 0);
    assert!(app.last_executed_regions.is_empty());
}
fn run(output: &Path) {
    std::fs::create_dir_all(output).unwrap();
    let (mut pending, pending_ctx, pending_id) = fixture();
    let mut staged = insert_batch();
    staged.push(key(egui::Key::Enter, egui::Modifiers::NONE));
    frame(&mut pending, &pending_ctx, staged);
    pending.prepare_native_export().unwrap();
    pending
        .document
        .save(&output.join("slot-keys-pending.mathcraft"))
        .unwrap();
    mathcraft_document::export::export_mcdx(
        &pending.document,
        &output.join("slot-keys-pending.mcdx"),
    )
    .unwrap();
    let mut pending_cold = WorksheetApp::default();
    pending_cold.open(&output.join("slot-keys-pending.mcdx"));
    let traces = &pending_cold.plot_results[&pending_id]
        .as_ref()
        .unwrap()
        .traces;
    assert_eq!(traces.len(), 3);
    assert!(traces.iter().all(crate::plots::TraceOutcome::is_pending));
    assert!(traces[1].curve().is_none());
    assert_eq!(traces[0].curve().unwrap().points[1], [2., 4.]);
    assert_eq!(traces[2].curve().unwrap().points[1], [2., 10.]);
    assert!(pending_cold.last_executed_regions.is_empty());
    pending_cold.prepare_native_export().unwrap();
    mathcraft_document::export::export_mcdx(
        &pending_cold.document,
        &output.join("slot-keys-pending-copy.mcdx"),
    )
    .unwrap();
    let (mut app, ctx, id) = fixture();
    app.path = Some(output.join("slot-keys.mathcraft"));
    // Even a single OS batch must apply both text segments before F5/save.
    let mut events = insert_batch();
    events.extend([
        key(egui::Key::F5, egui::Modifiers::NONE),
        key(egui::Key::S, egui::Modifiers::COMMAND),
    ]);
    frame(&mut app, &ctx, events);
    assert_factors(&app, id);
    assert_eq!(app.undo.len(), 1);
    assert!(!app.dirty);
    let committed = spec(&app, id).clone();
    let mut cold = WorksheetApp::default();
    cold.open(&output.join("slot-keys.mathcraft"));
    assert_eq!(spec(&cold, id), &committed);
    assert_factors(&cold, id);
    assert!(cold.last_executed_regions.is_empty());
    cold.prepare_native_export().unwrap();
    mathcraft_document::export::export_mcdx(&cold.document, &output.join("slot-keys.mcdx"))
        .unwrap();
    let mut native = WorksheetApp::default();
    native.open(&output.join("slot-keys.mcdx"));
    assert_factors(&native, id);
    assert!(native.last_executed_regions.is_empty());
    native.prepare_native_export().unwrap();
    mathcraft_document::export::export_mcdx(&native.document, &output.join("slot-keys-copy.mcdx"))
        .unwrap();
    let mut snapshot = native.print_snapshot();
    let plan = snapshot.plan(PageSelection::All, 300).unwrap();
    let pages = printing::render_vector_pages(&plan, |ui, p, t| snapshot.paint(ui, p, t)).unwrap();
    printing::write_pdf(&output.join("slot-keys.pdf"), &pages).unwrap();
    assert!(native.last_executed_regions.is_empty());
    println!(
        "SLOT_KEYS_FLOW real_batch=select_all+text+shift_enter+text+F5+save original_pairing=1x3 history=1 own_native_cold_execution=0 vector_pdf=true"
    );
}
#[test]
fn batched_insert_f5_save_own_native_cold_and_vector_print() {
    let path = std::env::temp_dir().join(format!(
        "mathcraft-slot-keys-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    run(&path);
    for item in std::fs::read_dir(&path).unwrap() {
        std::fs::remove_file(item.unwrap().path()).unwrap();
    }
    std::fs::remove_dir(path).unwrap();
}
#[test]
#[ignore = "Explicit structural-slot actual-key/cold-save/native/vector-PDF probe"]
fn plot_slot_key_save_native_export_reopen_probe() {
    run(&PathBuf::from(
        std::env::var_os("MATHCRAFT_PLOT_SLOT_KEY_OUTPUT").expect("MATHCRAFT_PLOT_SLOT_KEY_OUTPUT"),
    ));
}
