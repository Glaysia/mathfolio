//! Shared authored axes through real keys, native caches, history and vector print.
use super::*;
use mathfolio_document::canvas::{PlotSource, PlotTraceAppearance};

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
fn run(output: &Path) {
    std::fs::create_dir_all(output).unwrap();
    for shared_x in [true, false] {
        let prefix = if shared_x { "shared-x" } else { "shared-y" };
        let mut app = WorksheetApp {
            automatic_calculation: false,
            ..Default::default()
        };
        let definition = app.document.add_math(20., 20.);
        app.document.regions.iter_mut().find(|r|r.id==definition).unwrap().content=CanvasContent::Math(worksheet_math::tree_from_mathml(r#"<m:define xmlns:m="http://schemas.mathsoft.com/math50"><m:id>x</m:id><m:matrix rows="3" cols="1"><m:real>1</m:real><m:real>2</m:real><m:real>3</m:real></m:matrix></m:define>"#).unwrap());
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
        let (x, y) = if shared_x {
            (vec!["x"], vec!["2*x", "3*x"])
        } else {
            (vec!["x", "2*x"], vec!["3*x"])
        };
        plot.source = PlotSource::axis_lists(
            x.into_iter().map(String::from).collect(),
            y.into_iter().map(String::from).collect(),
            vec![PlotTraceAppearance::default(); 2],
        )
        .unwrap();
        app.recalculate();
        app.prepare_native_export().unwrap();
        let native = output.join(format!("{prefix}.mcdx"));
        mathfolio_document::export::export_mcdx(&app.document, &native).unwrap();
        let mut opened = WorksheetApp::default();
        opened.open(&native);
        opened.automatic_calculation = false;
        assert!(
            opened.pending_regions.is_empty() && opened.last_executed_regions.is_empty(),
            "{}",
            opened.status
        );
        assert!(opened.plot_results[&id].is_ok());
        let original = spec(&opened, id).clone();
        assert_eq!(
            original.axis_lists().unwrap().x.len(),
            if shared_x { 1 } else { 2 }
        );
        assert_eq!(
            original.axis_lists().unwrap().y.len(),
            if shared_x { 2 } else { 1 }
        );
        let before_points = opened.plot_results[&id].clone();
        opened.prepare_native_export().unwrap();
        let copy = output.join(format!("{prefix}-copy.mcdx"));
        mathfolio_document::export::export_mcdx(&opened.document, &copy).unwrap();
        assert_eq!(
            NativePackage::open(&native)
                .unwrap()
                .original_part("mathcad/result.xml"),
            NativePackage::open(&copy)
                .unwrap()
                .original_part("mathcad/result.xml")
        );
        let ctx = egui::Context::default();
        crate::configure(&ctx);
        opened.selected = Some(id);
        opened.focus_new = Some(id);
        opened.insertion = None;
        let frame = super::super::tests::frame;
        frame(&mut opened, &ctx, vec![]);
        frame(&mut opened, &ctx, vec![]);
        // Every authored Y expression/scaling pair precedes X.
        for _ in 0..if shared_x {
            original.axis_lists().unwrap().y.len() * 2
        } else {
            0
        } {
            frame(
                &mut opened,
                &ctx,
                vec![egui::Event::Key {
                    key: egui::Key::Tab,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
            );
        }
        let text = if shared_x { "2*x" } else { "4*x" };
        frame(
            &mut opened,
            &ctx,
            vec![
                egui::Event::Key {
                    key: egui::Key::A,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::COMMAND,
                },
                egui::Event::Text(text.into()),
            ],
        );
        assert!(
            opened
                .inline_plots
                .get(&id)
                .is_some_and(|draft| draft.has_changes()),
            "{prefix}: {}",
            opened.status
        );
        assert_eq!(spec(&opened, id), &original);
        assert_eq!(opened.plot_results[&id], before_points);
        assert!(opened.last_executed_regions.is_empty());
        frame(&mut opened, &ctx, vec![super::super::tests::enter()]);
        assert!(opened.pending_regions.contains(&id));
        assert_eq!(opened.undo.len(), 1);
        let committed = spec(&opened, id).clone();
        let mut expected = original.clone();
        let factor = if shared_x { "2" } else { "4" };
        let tree = worksheet_math::tree_from_mathml(&format!(
            "<ml:apply><ml:mult/><ml:real>{factor}</ml:real><ml:id>x</ml:id></ml:apply>"
        ))
        .unwrap();
        expected
            .set_axis_expression(
                if shared_x {
                    mathfolio_document::canvas::PlotAxis::X
                } else {
                    mathfolio_document::canvas::PlotAxis::Y
                },
                0,
                mathfolio_document::canvas::PlotExpression::from_tree(tree).unwrap(),
            )
            .unwrap();
        assert_eq!(committed, expected);
        opened.undo();
        assert_eq!(spec(&opened, id), &original);
        opened.redo();
        assert_eq!(spec(&opened, id), &committed);
        opened.prepare_native_export().unwrap();
        let own = output.join(format!("{prefix}.mathfolio"));
        opened.document.save(&own).unwrap();
        let mut saved = WorksheetApp::default();
        saved.open(&own);
        assert_eq!(spec(&saved, id), &committed);
        assert!(saved.pending_regions.contains(&id) && saved.last_executed_regions.is_empty());
        saved.recalculate();
        let data = saved.plot_results[&id].as_ref().unwrap();
        assert_eq!(data.traces.len(), 2);
        for (index, trace) in data.traces.iter().enumerate() {
            let trace = trace.curve().unwrap();
            assert_eq!(trace.points.len(), 3);
            let ratio = if shared_x {
                if index == 0 { 1. } else { 1.5 }
            } else if index == 0 {
                4.
            } else {
                2.
            };
            assert!(
                trace
                    .points
                    .iter()
                    .all(|p| (p[1] - ratio * p[0]).abs() < 1e-10)
            );
        }
        saved.prepare_native_export().unwrap();
        let edited = output.join(format!("{prefix}-edited.mcdx"));
        mathfolio_document::export::export_mcdx(&saved.document, &edited).unwrap();
        let mut reopened = WorksheetApp::default();
        reopened.open(&edited);
        assert!(reopened.pending_regions.is_empty() && reopened.last_executed_regions.is_empty());
        assert!(
            mathfolio_document::native_plot::same_source(spec(&reopened, id), &committed).unwrap()
        );
        assert_eq!(reopened.plot_results[&id], saved.plot_results[&id]);
        let mut snapshot = reopened.print_snapshot();
        let plan = snapshot.plan(PageSelection::All, 300).unwrap();
        let pages =
            printing::render_vector_pages(&plan, |ui, page, total| snapshot.paint(ui, page, total))
                .unwrap();
        printing::write_pdf(&output.join(format!("{prefix}.pdf")), &pages).unwrap();
        assert!(reopened.last_executed_regions.is_empty());
        println!(
            "SHARED_PLOT {prefix} authored_axes=preserved typing=inert enter=one_history manual=pending undo_redo=shared native_cold_execution=0 native_copy_cache=exact vector_print_execution=0"
        );
    }
}
#[test]
fn shared_xy_real_keys_history_native_cache_and_vector_print() {
    let output = std::env::temp_dir().join(format!(
        "mathfolio-shared-xy-{}-{}",
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
#[ignore = "Explicit shared axes native/key/history/PDF probe, headless Rust only"]
fn shared_xy_authoring_save_native_export_reopen_probe() {
    run(&PathBuf::from(
        std::env::var_os("MATHFOLIO_SHARED_PLOT_OUTPUT").expect("MATHFOLIO_SHARED_PLOT_OUTPUT"),
    ));
}

fn run_slot_model(output: &Path) {
    use mathfolio_document::canvas::{PlotAxis, PlotTraceAppearance};
    std::fs::create_dir_all(output).unwrap();
    let mut document = CanvasDocument::default();
    let id = document.add_plot(20., 20.);
    let CanvasContent::Plot(source) = &mut document.regions[0].content else {
        panic!()
    };
    let inserted = source
        .insert_axis_equation(PlotAxis::Y, 1, "", Some(PlotTraceAppearance::default()))
        .unwrap();
    assert_eq!(inserted.trace_count_delta(), 1);
    let blank = output.join("slot-placeholder.mcdx");
    mathfolio_document::export::export_mcdx(&document, &blank).unwrap();
    let mut opened = WorksheetApp::default();
    opened.open(&blank);
    assert!(opened.last_executed_regions.is_empty() && opened.pending_regions.contains(&id));
    assert!(
        opened.plot_results[&id]
            .as_ref()
            .unwrap()
            .first_curve()
            .is_none()
            && !opened.status.contains("지원 필요"),
        "{}",
        opened.status
    );
    let CanvasContent::Plot(source) = &mut opened.document.regions[0].content else {
        panic!()
    };
    assert!(
        source.axis_lists().unwrap().y[1]
            .source
            .is_exact_placeholder()
    );
    assert!(source.axis_lists().unwrap().y[1].source.as_tree().is_some());
    source
        .set_axis_expression_text(PlotAxis::Y, 1, "2*x")
        .unwrap();
    opened.recalculate();
    assert_eq!(opened.plot_results[&id].as_ref().unwrap().traces.len(), 2);
    opened.prepare_native_export().unwrap();
    let filled = output.join("slot-filled.mcdx");
    mathfolio_document::export::export_mcdx(&opened.document, &filled).unwrap();
    let mut loaded = WorksheetApp::default();
    loaded.open(&filled);
    assert!(loaded.last_executed_regions.is_empty() && loaded.pending_regions.is_empty());
    assert_eq!(loaded.plot_results[&id].as_ref().unwrap().traces.len(), 2);
    let CanvasContent::Plot(source) = &mut loaded.document.regions[0].content else {
        panic!()
    };
    let removed = source.remove_axis_equation(PlotAxis::Y, 0).unwrap();
    assert_eq!(removed.trace_count_delta(), -1);
    let remaining_id = source.axis_lists().unwrap().y[0].id;
    loaded.recalculate();
    assert_eq!(loaded.plot_results[&id].as_ref().unwrap().traces.len(), 1);
    assert!(
        loaded.plot_results[&id]
            .as_ref()
            .unwrap()
            .first_curve()
            .unwrap()
            .points
            .iter()
            .all(|p| (p[1] - 2. * p[0]).abs() < 1e-10)
    );
    loaded.prepare_native_export().unwrap();
    mathfolio_document::export::export_mcdx(&loaded.document, &output.join("slot-removed.mcdx"))
        .unwrap();
    let mut snapshot = loaded.print_snapshot();
    let plan = snapshot.plan(PageSelection::All, 300).unwrap();
    let pages =
        printing::render_vector_pages(&plan, |ui, page, total| snapshot.paint(ui, page, total))
            .unwrap();
    printing::write_pdf(&output.join("slot-removed.pdf"), &pages).unwrap();
    let CanvasContent::Plot(source) = &mut loaded.document.regions[0].content else {
        panic!()
    };
    let cleared = source.clear_axis_equation(PlotAxis::Y, 0).unwrap().unwrap();
    assert_eq!(cleared.equation_id, remaining_id);
    assert_eq!(cleared.trace_count_delta(), 0);
    assert!(
        source
            .clear_axis_equation(PlotAxis::Y, 0)
            .unwrap()
            .is_none()
    );
    assert!(source.remove_axis_equation(PlotAxis::Y, 0).is_err());
    // The changed source invalidates the previous synchronized presentation.
    mathfolio_document::export::export_mcdx(&loaded.document, &output.join("slot-cleared.mcdx"))
        .unwrap();
    let mut cleared = WorksheetApp::default();
    cleared.open(&output.join("slot-cleared.mcdx"));
    assert!(cleared.last_executed_regions.is_empty() && cleared.pending_regions.contains(&id));
    assert!(
        cleared.plot_results[&id]
            .as_ref()
            .unwrap()
            .first_curve()
            .is_none()
    );
    assert!(
        spec(&cleared, id)
            .primary_y_expression()
            .is_exact_placeholder()
    );
    assert!(
        spec(&cleared, id)
            .primary_y_expression()
            .as_tree()
            .is_some()
    );
    println!(
        "PLOT_SLOT_MODEL insert=placeholder native_pending=editable fill=two_curves remove=retained_two_x clear=identity_kept last_slot=retained native_cold_execution=0 ui_shortcut=not_yet_wired"
    );
}
#[test]
fn plot_slot_model_placeholder_native_roundtrip_and_remaining_curve() {
    let output = std::env::temp_dir().join(format!(
        "mathfolio-slot-model-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    run_slot_model(&output);
    for item in std::fs::read_dir(&output).unwrap() {
        std::fs::remove_file(item.unwrap().path()).unwrap();
    }
    std::fs::remove_dir(output).unwrap();
}
#[test]
#[ignore = "Explicit pure slot model/native/PDF probe; keyboard authoring is not implemented"]
fn plot_slot_model_native_export_reopen_probe() {
    run_slot_model(&PathBuf::from(
        std::env::var_os("MATHFOLIO_PLOT_SLOT_OUTPUT").expect("MATHFOLIO_PLOT_SLOT_OUTPUT"),
    ));
}
