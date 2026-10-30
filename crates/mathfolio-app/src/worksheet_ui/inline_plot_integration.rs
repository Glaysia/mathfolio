//! Actual inline key input through commit, save, native reopen and vector print.
use super::*;
fn run(output: &Path) {
    std::fs::create_dir_all(output).unwrap();
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    let mut app = WorksheetApp {
        automatic_calculation: false,
        ..Default::default()
    };
    let id = app.document.add_plot(20., 20.);
    app.recalculate();
    app.persist_calculation_cache();
    app.restore_saved_calculations();
    app.selected = Some(id);
    app.focus_new = Some(id);
    app.insertion = None;
    let frame = super::super::tests::frame;
    frame(&mut app, &ctx, vec![]);
    frame(&mut app, &ctx, vec![]);
    let original = app.document.clone();
    let points = app.plot_results[&id].clone();
    frame(
        &mut app,
        &ctx,
        vec![
            egui::Event::Key {
                key: egui::Key::A,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::COMMAND,
            },
            egui::Event::Text("2*x".into()),
        ],
    );
    assert_eq!(app.document, original);
    assert_eq!(app.plot_results[&id], points);
    assert!(app.last_executed_regions.is_empty());
    let mut snapshot = app.print_snapshot();
    let plan = snapshot.plan(PageSelection::All, 300).unwrap();
    let pages =
        printing::render_vector_pages(&plan, |ui, page, total| snapshot.paint(ui, page, total))
            .unwrap();
    printing::write_pdf(&output.join("inline-plot-before.pdf"), &pages).unwrap();
    assert_eq!(app.document, original);
    assert!(app.inline_plots[&id].has_changes() && app.last_executed_regions.is_empty());
    frame(&mut app, &ctx, vec![super::super::tests::enter()]);
    assert!(app.pending_regions.contains(&id));
    assert_eq!(app.undo.len(), 1);
    super::super::tests::assert_pending_curves_unchanged(&app.plot_results[&id], &points);
    app.prepare_native_export().unwrap();
    let pending = output.join("inline-plot-pending.mcdx");
    mathfolio_document::export::export_mcdx(&app.document, &pending).unwrap();
    let mut pending_app = WorksheetApp::default();
    pending_app.open(&pending);
    assert!(
        pending_app.pending_regions.contains(&id) && pending_app.last_executed_regions.is_empty()
    );
    let own = output.join("inline-plot.mathfolio");
    app.document.save(&own).unwrap();
    let mut saved = WorksheetApp::default();
    saved.open(&own);
    assert!(saved.pending_regions.contains(&id) && saved.last_executed_regions.is_empty());
    super::super::tests::assert_pending_curves_unchanged(&saved.plot_results[&id], &points);
    saved.recalculate();
    assert!(
        saved.plot_results[&id]
            .as_ref()
            .unwrap()
            .first_curve()
            .unwrap()
            .points
            .iter()
            .all(|point| point[1] == 2. * point[0])
    );
    saved.prepare_native_export().unwrap();
    let native = output.join("inline-plot-edited.mcdx");
    mathfolio_document::export::export_mcdx(&saved.document, &native).unwrap();
    let mut reopened = WorksheetApp::default();
    reopened.open(&native);
    assert!(reopened.pending_regions.is_empty() && reopened.last_executed_regions.is_empty());
    assert!(
        reopened.plot_results[&id]
            .as_ref()
            .unwrap()
            .first_curve()
            .unwrap()
            .points
            .iter()
            .all(|point| point[1] == 2. * point[0])
    );
    let mut snapshot = reopened.print_snapshot();
    let plan = snapshot.plan(PageSelection::All, 300).unwrap();
    let pages =
        printing::render_vector_pages(&plan, |ui, page, total| snapshot.paint(ui, page, total))
            .unwrap();
    printing::write_pdf(&output.join("inline-plot-after.pdf"), &pages).unwrap();
    reopened.persist_calculation_cache();
    reopened
        .document
        .save(&output.join("inline-plot-reimported.mathfolio"))
        .unwrap();
    println!(
        "INLINE_PLOT_FLOW actual_keys=select_all+text+enter typing_source=inert typing_calculation=0 history=1 manual=pending old_points=retained save_reopen=pending calculation=2x native_reopen_execution=0 print_execution=0"
    );
}
#[test]
fn inline_plot_real_keys_manual_cache_native_save_reopen_and_vector_print() {
    let output = std::env::temp_dir().join(format!(
        "mathfolio-inline-plot-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    run(&output);
    for item in std::fs::read_dir(&output).unwrap() {
        std::fs::remove_file(item.unwrap().path()).unwrap();
    }
    std::fs::remove_dir(output).unwrap();
}
#[test]
#[ignore = "Explicit inline plot key/commit/native/PDF probe, headless Rust only"]
fn inline_plot_key_commit_save_native_export_reopen_probe() {
    run(&PathBuf::from(
        std::env::var_os("MATHFOLIO_INLINE_PLOT_OUTPUT").expect("MATHFOLIO_INLINE_PLOT_OUTPUT"),
    ));
}
