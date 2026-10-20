//! Non-primary shared-axis source fields through actual keys and one app commit.
use super::*;
use mathcraft_document::canvas::{PlotSource, PlotTraceAppearance};

fn spec(app: &WorksheetApp, id: u64) -> &PlotSpec {
    let CanvasContent::Plot(spec) = &app
        .document
        .regions
        .iter()
        .find(|r| r.id == id)
        .unwrap()
        .content
    else {
        panic!()
    };
    spec
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
fn run(output: &Path) {
    std::fs::create_dir_all(output).unwrap();
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    let mut app = WorksheetApp {
        automatic_calculation: false,
        ..Default::default()
    };
    let x = app.document.add_math(20., 20.);
    app.document.regions.iter_mut().find(|r| r.id == x).unwrap().content = CanvasContent::Math(worksheet_math::tree_from_mathml(r#"<m:define xmlns:m="http://schemas.mathsoft.com/math50"><m:id>x</m:id><m:matrix rows="3" cols="1"><m:real>1</m:real><m:real>2</m:real><m:real>3</m:real></m:matrix></m:define>"#).unwrap());
    let id = app.document.add_plot(20., 100.);
    let CanvasContent::Plot(plot) = &mut app
        .document
        .regions
        .iter_mut()
        .find(|r| r.id == id)
        .unwrap()
        .content
    else {
        panic!()
    };
    plot.source = PlotSource::axis_lists(
        vec!["x".into(), "2*x".into()],
        vec!["3*x".into(), "4*x".into(), "5*x".into()],
        vec![PlotTraceAppearance::default(); 3],
    )
    .unwrap();
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
    let original_plot = spec(&app, id).clone();
    let points = app.plot_results[&id].clone();
    let generation = app.background.submitted_generation();
    for _ in 0..4 {
        frame(
            &mut app,
            &ctx,
            vec![key(egui::Key::Tab, egui::Modifiers::NONE)],
        );
    }
    frame(
        &mut app,
        &ctx,
        vec![
            key(egui::Key::A, egui::Modifiers::COMMAND),
            egui::Event::Text("9*x".into()),
        ],
    );
    for _ in 0..4 {
        frame(
            &mut app,
            &ctx,
            vec![key(egui::Key::Tab, egui::Modifiers::NONE)],
        );
    }
    frame(
        &mut app,
        &ctx,
        vec![
            key(egui::Key::A, egui::Modifiers::COMMAND),
            egui::Event::Text("6*x".into()),
        ],
    );
    assert_eq!(app.document, original);
    assert_eq!(app.plot_results[&id], points);
    assert!(app.undo.is_empty() && app.last_executed_regions.is_empty());
    assert_eq!(app.background.submitted_generation(), generation);
    frame(&mut app, &ctx, vec![super::super::tests::enter()]);
    let committed = spec(&app, id).clone();
    let mut expected = original_plot.clone();
    expected.axis_lists_mut().unwrap().y[2].source = "9*x".into();
    expected.axis_lists_mut().unwrap().x[1].source = "6*x".into();
    assert_eq!(committed, expected);
    assert_eq!(app.undo.len(), 1);
    assert!(app.last_executed_regions.is_empty());
    super::super::tests::assert_pending_curves_unchanged(&app.plot_results[&id], &points);
    app.prepare_native_export().unwrap();
    app.document
        .save(&output.join("slot-edited-pending.mathcraft"))
        .unwrap();
    app.undo();
    assert_eq!(spec(&app, id), &original_plot);
    app.redo();
    assert_eq!(spec(&app, id), &committed);
    app.recalculate();
    for (ordinal, factor) in [3., 4. / 6., 9. / 6.].into_iter().enumerate() {
        assert!(
            app.plot_results[&id].as_ref().unwrap().traces[ordinal]
                .curve()
                .unwrap()
                .points
                .iter()
                .all(|p| (p[1] - factor * p[0]).abs() < 1e-10)
        );
    }
    app.prepare_native_export().unwrap();
    let path = output.join("slot-edited.mcdx");
    mathcraft_document::export::export_mcdx(&app.document, &path).unwrap();
    let mut cold = WorksheetApp::default();
    cold.open(&path);
    assert!(cold.last_executed_regions.is_empty());
    assert_eq!(spec(&cold, id).axis_lists().unwrap().x.len(), 2);
    assert_eq!(spec(&cold, id).axis_lists().unwrap().y.len(), 3);
    for (ordinal, factor) in [3., 4. / 6., 9. / 6.].into_iter().enumerate() {
        assert!(
            cold.plot_results[&id].as_ref().unwrap().traces[ordinal]
                .curve()
                .unwrap()
                .points
                .iter()
                .all(|p| (p[1] - factor * p[0]).abs() < 1e-10)
        );
    }
    let mut snapshot = cold.print_snapshot();
    let plan = snapshot.plan(PageSelection::All, 300).unwrap();
    let pages =
        printing::render_vector_pages(&plan, |ui, page, total| snapshot.paint(ui, page, total))
            .unwrap();
    printing::write_pdf(&output.join("slot-edited.pdf"), &pages).unwrap();
    println!(
        "MULTI_SLOT_EDIT_FLOW actual_keys=tab+text+tab+text+enter changed_slots=Y2_X1 source_while_typing=inert calculation_while_typing=0 history=1 identity=stable manual=previous_curves_pending undo_redo=exact cold_execution=0 native_pairs=2x3 scaling_fields=interleaved_typed"
    );
}
#[test]
fn multiple_axis_source_fields_real_keys_one_commit_save_native_reopen_and_print() {
    let output = std::env::temp_dir().join(format!(
        "mathcraft-slot-edits-{}-{}",
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
#[ignore = "Explicit non-primary axis actual-key/native/cache/vector-PDF probe"]
fn plot_slot_edit_save_native_export_reopen_probe() {
    run(&PathBuf::from(
        std::env::var_os("MATHCRAFT_PLOT_SLOT_EDIT_OUTPUT")
            .expect("MATHCRAFT_PLOT_SLOT_EDIT_OUTPUT"),
    ));
}
