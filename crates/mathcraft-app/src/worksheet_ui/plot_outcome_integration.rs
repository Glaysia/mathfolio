//! Per-trace computation/cache lifetime through app commands and native packages.
use super::*;
use crate::plots::TraceOutcome;
use mathcraft_document::canvas::{PlotSource, PlotTraceAppearance};
fn fixture() -> (WorksheetApp, u64) {
    let mut app = WorksheetApp {
        automatic_calculation: false,
        ..Default::default()
    };
    let x = app.document.add_math(20., 20.);
    app.document.regions.iter_mut().find(|r|r.id==x).unwrap().content=CanvasContent::Math(worksheet_math::tree_from_mathml(r#"<m:define xmlns:m="http://schemas.mathsoft.com/math50"><m:id>x</m:id><m:matrix rows="3" cols="1"><m:real>1</m:real><m:real>2</m:real><m:real>3</m:real></m:matrix></m:define>"#).unwrap());
    let id = app.document.add_plot(20., 100.);
    let CanvasContent::Plot(spec) = &mut app
        .document
        .regions
        .iter_mut()
        .find(|r| r.id == id)
        .unwrap()
        .content
    else {
        panic!()
    };
    spec.source = PlotSource::axis_lists(
        vec!["x".into()],
        vec!["2*x".into(), "4*x".into(), "3*x".into()],
        vec![PlotTraceAppearance::default(); 3],
    )
    .unwrap();
    app.recalculate();
    app.persist_calculation_cache();
    (app, id)
}
fn edit(app: &mut WorksheetApp, id: u64, source: &str) {
    let ctx = egui::Context::default();
    app.selected = Some(id);
    app.ribbon_command(&ctx, RibbonAction::EditPlot);
    app.plot_draft
        .as_mut()
        .unwrap()
        .spec
        .axis_lists_mut()
        .unwrap()
        .y[1]
        .source = source.into();
    assert!(app.commit_plot_draft(), "{}", app.status);
}
fn data(app: &WorksheetApp, id: u64) -> &crate::plots::PlotData {
    app.plot_results[&id].as_ref().unwrap()
}
fn write_native(app: &mut WorksheetApp, path: &Path) {
    app.prepare_native_export().unwrap();
    mathcraft_document::export::export_mcdx(&app.document, path).unwrap();
}
fn pdf(app: &mut WorksheetApp, path: &Path) {
    let mut snapshot = app.print_snapshot();
    let plan = snapshot.plan(PageSelection::All, 300).unwrap();
    let pages =
        printing::render_vector_pages(&plan, |ui, page, total| snapshot.paint(ui, page, total))
            .unwrap();
    printing::write_pdf(path, &pages).unwrap();
}
fn run(output: &Path) {
    std::fs::create_dir_all(output).unwrap();
    let (mut app, id) = fixture();
    let original = data(&app, id).clone();
    edit(&mut app, id, "1/(x-x)");
    super::super::tests::assert_pending_curves_unchanged(
        &app.plot_results[&id],
        &Ok(original.clone()),
    );
    let pending = output.join("trace-before-calculation.mcdx");
    write_native(&mut app, &pending);
    let mut cold = WorksheetApp::default();
    cold.open(&pending);
    assert!(cold.last_executed_regions.is_empty() && data(&cold, id).has_pending());
    assert!(
        data(&cold, id)
            .traces
            .iter()
            .all(|trace| matches!(trace, TraceOutcome::Pending { previous: Some(_) }))
    );
    app.recalculate();
    assert!(matches!(data(&app, id).traces[0], TraceOutcome::Ready(_)));
    assert!(matches!(data(&app, id).traces[1], TraceOutcome::Failed(_)));
    assert!(matches!(data(&app, id).traces[2], TraceOutcome::Ready(_)));
    assert!(data(&app, id).traces[1].curve().is_none());
    for index in [0, 2] {
        assert_eq!(
            data(&app, id).traces[index].curve(),
            original.traces[index].curve()
        );
    }
    assert!(app.region_has_error(id) && app.error_region_ids().contains(&id));
    app.persist_calculation_cache();
    let own = output.join("trace-failed.mathcraft");
    app.document.save(&own).unwrap();
    let mut reopened = WorksheetApp::default();
    reopened.open(&own);
    assert!(reopened.last_executed_regions.is_empty() && data(&reopened, id).has_failures());
    assert_eq!(data(&reopened, id), data(&app, id));
    pdf(&mut reopened, &output.join("trace-failed.pdf"));
    let native = output.join("trace-failed.mcdx");
    write_native(&mut reopened, &native);
    let mut native_opened = WorksheetApp::default();
    native_opened.open(&native);
    assert!(native_opened.last_executed_regions.is_empty());
    assert!(matches!(
        data(&native_opened, id).traces[0],
        TraceOutcome::Ready(_)
    ));
    assert!(matches!(
        data(&native_opened, id).traces[1],
        TraceOutcome::Pending { previous: None }
    ));
    assert!(matches!(
        data(&native_opened, id).traces[2],
        TraceOutcome::Ready(_)
    ));
    let copy = output.join("trace-failed-copy.mcdx");
    write_native(&mut native_opened, &copy);
    assert_eq!(
        NativePackage::open(&native)
            .unwrap()
            .original_part("mathcad/result.xml"),
        NativePackage::open(&copy)
            .unwrap()
            .original_part("mathcad/result.xml")
    );
    native_opened.persist_calculation_cache();
    let mixed = output.join("trace-native-mixed.mathcraft");
    native_opened.document.save(&mixed).unwrap();
    let mut mixed_opened = WorksheetApp::default();
    mixed_opened.open(&mixed);
    assert!(mixed_opened.last_executed_regions.is_empty());
    assert!(matches!(
        data(&mixed_opened, id).traces[0],
        TraceOutcome::Ready(_)
    ));
    assert!(matches!(
        data(&mixed_opened, id).traces[1],
        TraceOutcome::Pending { previous: None }
    ));
    assert!(matches!(
        data(&mixed_opened, id).traces[2],
        TraceOutcome::Ready(_)
    ));
    reopened.automatic_calculation = false;
    edit(&mut reopened, id, "4*x");
    assert!(data(&reopened, id).has_pending() && !data(&reopened, id).has_failures());
    assert!(matches!(
        data(&reopened, id).traces[0],
        TraceOutcome::Pending { previous: Some(_) }
    ));
    assert!(matches!(
        data(&reopened, id).traces[1],
        TraceOutcome::Pending { previous: None }
    ));
    let pending = output.join("trace-fixed-pending.mcdx");
    write_native(&mut reopened, &pending);
    let mut pending_opened = WorksheetApp::default();
    pending_opened.open(&pending);
    assert!(pending_opened.last_executed_regions.is_empty());
    assert!(matches!(
        data(&pending_opened, id).traces[0],
        TraceOutcome::Pending { previous: Some(_) }
    ));
    assert!(matches!(
        data(&pending_opened, id).traces[1],
        TraceOutcome::Pending { previous: None }
    ));
    assert!(matches!(
        data(&pending_opened, id).traces[2],
        TraceOutcome::Pending { previous: Some(_) }
    ));
    reopened.recalculate();
    assert!(
        data(&reopened, id)
            .traces
            .iter()
            .all(|trace| matches!(trace, TraceOutcome::Ready(_)))
    );
    assert!(
        data(&reopened, id).traces[1]
            .curve()
            .unwrap()
            .points
            .iter()
            .all(|point| point[1] == 4. * point[0])
    );
    write_native(&mut reopened, &output.join("trace-recovered.mcdx"));
    pdf(&mut reopened, &output.join("trace-recovered.pdf"));
    println!(
        "TRACE_OUTCOME_FLOW failed_middle=own_curve_removed ready_siblings=preserved native_failed=empty_pending_no_fake_error native_copy=exact v2_pending_siblings=ready manual_commit=explicit_per_slot curve_recovery=4x cold_execution=0"
    );
}
#[test]
fn per_trace_failure_pending_native_and_own_cache_lifetime() {
    let output = std::env::temp_dir().join(format!(
        "mathcraft-trace-outcome-{}-{}",
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
#[ignore = "Explicit per-trace pending/failure native/own/PDF headless probe"]
fn per_trace_plot_result_save_native_export_reopen_probe() {
    run(&PathBuf::from(
        std::env::var_os("MATHCRAFT_TRACE_OUTCOME_OUTPUT").expect("MATHCRAFT_TRACE_OUTCOME_OUTPUT"),
    ));
}

fn run_blank(output: &Path) {
    use mathcraft_document::canvas::PlotLegendPosition;
    std::fs::create_dir_all(output).unwrap();
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    let mut app = WorksheetApp {
        automatic_calculation: false,
        ..Default::default()
    };
    app.ribbon_command(&ctx, RibbonAction::InsertPlot);
    let id = app.selected.unwrap();
    let source = |app: &WorksheetApp| {
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
        spec.clone()
    };
    let blank = source(&app);
    assert!(blank.primary_x_expression().is_exact_placeholder());
    assert!(blank.primary_x_expression().as_tree().is_some());
    assert!(blank.primary_y_expression().is_exact_placeholder());
    assert!(blank.primary_y_expression().as_tree().is_some());
    assert_eq!(blank.trace_count(), 1);
    assert_eq!(
        blank.x_axis.legend_position,
        Some(PlotLegendPosition::Bottom)
    );
    assert_eq!(
        blank.y_axis.legend_position,
        Some(PlotLegendPosition::Right)
    );
    app.recalculate();
    assert!(matches!(data(&app, id).traces[0], TraceOutcome::Incomplete));
    assert!(!app.region_has_error(id));
    write_native(&mut app, &output.join("blank-xy.mcdx"));
    app.document
        .save(&output.join("blank-xy.mathcraft"))
        .unwrap();
    for path in [
        output.join("blank-xy.mcdx"),
        output.join("blank-xy.mathcraft"),
    ] {
        let mut reopened = WorksheetApp::default();
        reopened.open(&path);
        let reopened_source = source(&reopened);
        assert_eq!(
            reopened_source.paired_sources().collect::<Vec<_>>(),
            blank.paired_sources().collect::<Vec<_>>()
        );
        assert_eq!(reopened_source.x_axis, blank.x_axis);
        assert_eq!(reopened_source.y_axis, blank.y_axis);
        assert_eq!(
            [reopened_source.width, reopened_source.height],
            [blank.width, blank.height]
        );
        if path
            .extension()
            .is_some_and(|extension| extension == "mathcraft")
        {
            assert_eq!(reopened_source, blank);
        }
        assert!(reopened.last_executed_regions.is_empty());
        assert!(data(&reopened, id).first_curve().is_none());
        assert!(!reopened.region_has_error(id));
    }
    // Exercise the actual text widget and focus path, without a GUI or bitmap.
    app.restore_saved_calculations();
    app.focus_new = Some(id);
    app.insertion = None;
    let frame = super::super::tests::frame;
    frame(&mut app, &ctx, vec![]);
    frame(&mut app, &ctx, vec![]);
    let before = app.document.clone();
    frame(&mut app, &ctx, vec![egui::Event::Text("2*x".into())]);
    assert_eq!(app.document, before);
    for _ in 0..2 {
        frame(
            &mut app,
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
    frame(&mut app, &ctx, vec![egui::Event::Text("x".into())]);
    assert_eq!(app.document, before);
    assert!(app.last_executed_regions.is_empty());
    frame(&mut app, &ctx, vec![super::super::tests::enter()]);
    let authored = source(&app);
    let expected_x = worksheet_math::tree_from_mathml("<ml:id>x</ml:id>").unwrap();
    let expected_y = worksheet_math::tree_from_mathml(
        "<ml:apply><ml:mult/><ml:real>2</ml:real><ml:id>x</ml:id></ml:apply>",
    )
    .unwrap();
    assert_eq!(authored.primary_x_expression().as_tree(), Some(&expected_x));
    assert_eq!(authored.primary_y_expression().as_tree(), Some(&expected_y));
    assert_eq!(
        authored.axis_lists().unwrap().x[0].id,
        blank.axis_lists().unwrap().x[0].id
    );
    assert_eq!(
        authored.axis_lists().unwrap().y[0].id,
        blank.axis_lists().unwrap().y[0].id
    );
    assert!(app.pending_regions.contains(&id) && app.last_executed_regions.is_empty());
    assert_eq!(app.undo.len(), 2); // Insert + one aggregate axis edit.
    app.recalculate();
    assert!(
        data(&app, id)
            .first_curve()
            .unwrap()
            .points
            .iter()
            .all(|p| p[1] == 2. * p[0])
    );
    write_native(&mut app, &output.join("blank-xy-authored.mcdx"));
    pdf(&mut app, &output.join("blank-xy-authored.pdf"));
    let mut reopened = WorksheetApp::default();
    reopened.open(&output.join("blank-xy-authored.mcdx"));
    let reopened_source = source(&reopened);
    // Native first math retains the typed multiplication and slot identity.
    assert_eq!(
        reopened_source.primary_x_expression().as_tree(),
        Some(&expected_x)
    );
    assert_eq!(
        reopened_source.primary_y_expression().as_tree(),
        Some(&expected_y)
    );
    assert_eq!(
        reopened_source.paired_sources().next().unwrap().style,
        authored.paired_sources().next().unwrap().style
    );
    assert_eq!(reopened_source.x_axis, authored.x_axis);
    assert_eq!(reopened_source.y_axis, authored.y_axis);
    assert!(reopened.last_executed_regions.is_empty());
    assert!(
        data(&reopened, id)
            .first_curve()
            .unwrap()
            .points
            .iter()
            .all(|p| p[1] == 2. * p[0])
    );
    println!(
        "BLANK_XY_FLOW source=two_placeholders trace_count=1 legend_source=bottom_right typing_execution=0 enter=one_axis_edit curve=2x native_reopen_execution=0 blank_curve=none"
    );
}
#[test]
fn blank_plot_real_keys_commit_save_reopen_without_example_curve() {
    let output = std::env::temp_dir().join(format!(
        "mathcraft-blank-xy-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    run_blank(&output);
    for item in std::fs::read_dir(&output).unwrap() {
        std::fs::remove_file(item.unwrap().path()).unwrap();
    }
    std::fs::remove_dir(output).unwrap();
}
#[test]
#[ignore = "Explicit blank source and actual axis key/native/PDF headless probe"]
fn blank_xy_authoring_save_native_export_reopen_probe() {
    run_blank(&PathBuf::from(
        std::env::var_os("MATHCRAFT_BLANK_XY_OUTPUT").expect("MATHCRAFT_BLANK_XY_OUTPUT"),
    ));
}
