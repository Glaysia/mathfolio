//! Independent graph/outer source values through app cache, history and print.
use super::*;
use mathfolio_document::canvas::{PlotGraphSize, PlotNativeGeometry};

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
fn spec_mut(app: &mut WorksheetApp, id: u64) -> &mut PlotSpec {
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
    spec
}
fn native(app: &mut WorksheetApp, path: &Path) {
    app.prepare_native_export().unwrap();
    mathfolio_document::export::export_mcdx(&app.document, path).unwrap();
}
fn assert_geometry(actual: &PlotSpec, expected: &PlotSpec) {
    assert!(
        actual
            .native_geometry
            .as_ref()
            .unwrap()
            .same_source(expected.native_geometry.as_ref().unwrap())
    );
    assert_eq!(
        [actual.width, actual.height],
        [expected.width, expected.height]
    );
}
fn run(output: &Path) {
    std::fs::create_dir_all(output).unwrap();
    let mut app = WorksheetApp {
        automatic_calculation: false,
        ..Default::default()
    };
    let id = app.document.add_plot(20., 20.);
    let plot = spec_mut(&mut app, id);
    *plot.primary_y_expression_mut().unwrap() = "2*x".into();
    plot.native_geometry = Some(Box::new(PlotNativeGeometry {
        graph_size: Some(PlotGraphSize {
            width: 321.00000000001,
            height: 217.125,
        }),
        requested_width: Some(600.00000000001),
        requested_height: Some(400.00000000002),
        actual_width: Some(598.75),
        actual_height: Some(399.5),
    }));
    plot.width = 600.;
    plot.height = 400.;
    let original = plot.clone();
    app.recalculate();
    native(&mut app, &output.join("geometry-source.mcdx"));
    app.document
        .save(&output.join("geometry-source.mathfolio"))
        .unwrap();
    for extension in ["mcdx", "mathfolio"] {
        let mut cold = WorksheetApp::default();
        cold.open(&output.join(format!("geometry-source.{extension}")));
        assert_geometry(spec(&cold, id), &original);
        assert!(cold.last_executed_regions.is_empty());
        assert!(
            cold.plot_results[&id]
                .as_ref()
                .unwrap()
                .first_curve()
                .unwrap()
                .points
                .iter()
                .all(|p| p[1] == 2. * p[0])
        );
        assert!(!cold.status.contains("지원 필요"), "{}", cold.status);
    }
    let mut cold = WorksheetApp::default();
    cold.open(&output.join("geometry-source.mcdx"));
    cold.automatic_calculation = false;
    native(&mut cold, &output.join("geometry-source-copy.mcdx"));
    let before = NativePackage::open(&output.join("geometry-source.mcdx")).unwrap();
    let copied = NativePackage::open(&output.join("geometry-source-copy.mcdx")).unwrap();
    for part in ["mathcad/worksheet.xml", "mathcad/result.xml"] {
        assert_eq!(before.original_part(part), copied.original_part(part));
    }
    let mut snapshot = cold.print_snapshot();
    let plan = snapshot.plan(PageSelection::All, 300).unwrap();
    let pages =
        printing::render_vector_pages(&plan, |ui, page, total| snapshot.paint(ui, page, total))
            .unwrap();
    printing::write_pdf(&output.join("geometry-source.pdf"), &pages).unwrap();
    assert_geometry(spec(&cold, id), &original);
    assert!(cold.last_executed_regions.is_empty());
    // This is the same controller transaction used after the first real drag
    // delta. The separate pointer regression covers capture before mutation.
    let before = spec(&cold, id).clone();
    spec_mut(&mut cold, id).set_outer_size(680., 440.).unwrap();
    cold.begin_plot_resize(id, before.clone());
    cold.calculation_changed(false);
    let resized = spec(&cold, id).clone();
    let geometry = resized.native_geometry.as_ref().unwrap();
    assert_eq!(geometry.requested_width, Some(680.));
    assert_eq!(geometry.requested_height, Some(440.));
    assert_eq!(geometry.actual_width, None);
    assert_eq!(geometry.actual_height, None);
    assert!(
        geometry.graph_size.as_ref().unwrap().same_source(
            original
                .native_geometry
                .as_ref()
                .unwrap()
                .graph_size
                .as_ref()
                .unwrap()
        )
    );
    assert!(cold.last_executed_regions.is_empty());
    assert_eq!(cold.undo.len(), 1);
    assert!(cold.plot_results[&id].as_ref().unwrap().has_pending());
    native(&mut cold, &output.join("geometry-resized-pending.mcdx"));
    let mut pending = WorksheetApp::default();
    pending.open(&output.join("geometry-resized-pending.mcdx"));
    assert_geometry(spec(&pending, id), &resized);
    assert!(pending.last_executed_regions.is_empty());
    assert!(pending.plot_results[&id].as_ref().unwrap().has_pending());
    assert!(
        pending.plot_results[&id]
            .as_ref()
            .unwrap()
            .first_curve()
            .unwrap()
            .points
            .iter()
            .all(|p| p[1] == 2. * p[0])
    );
    cold.undo();
    assert_geometry(spec(&cold, id), &before);
    cold.redo();
    assert_geometry(spec(&cold, id), &resized);
    println!(
        "PLOT_GEOMETRY_FLOW source=independent_f64 requested=600x400 graph=321x217.125 actual=598.75x399.5 native_copy=exact open_and_print_execution=0 resize=outer_only actual_cleared=true pending_curve=retained undo_redo=all_source_fields"
    );
}
#[test]
fn independent_plot_geometry_cache_resize_history_native_and_print() {
    let output = std::env::temp_dir().join(format!(
        "mathfolio-plot-geometry-{}-{}",
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
#[ignore = "Explicit independent native plot geometry/cache/history/vector-PDF probe"]
fn plot_geometry_save_native_export_reopen_probe() {
    run(&PathBuf::from(
        std::env::var_os("MATHFOLIO_PLOT_GEOMETRY_OUTPUT").expect("MATHFOLIO_PLOT_GEOMETRY_OUTPUT"),
    ));
}

#[test]
fn legacy_own_model_restores_retained_curve_without_geometry_migration_but_rejects_newer_source() {
    let output = std::env::temp_dir().join(format!(
        "mathfolio-old-plot-source-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&output).unwrap();
    let mut app = WorksheetApp {
        automatic_calculation: false,
        ..Default::default()
    };
    let id = app.document.add_plot(20., 20.);
    app.recalculate();
    let initial = output.join("initial.mcdx");
    native(&mut app, &initial);
    let before = spec(&app, id).clone();
    assert!(before.native_geometry.is_none());
    app.document.native_archive = Some(std::fs::read(&initial).unwrap());
    app.document.regions[0].source_region = Some("1".into());
    app.document.calculation_cache = None;
    let own = output.join("legacy.mathfolio");
    app.document.save(&own).unwrap();
    let mut cold = WorksheetApp::default();
    cold.open(&own);
    assert_eq!(spec(&cold, id), &before);
    assert!(cold.last_executed_regions.is_empty() && cold.pending_regions.is_empty());
    assert!(
        cold.plot_results[&id]
            .as_ref()
            .unwrap()
            .first_curve()
            .is_some()
    );
    assert!(!cold.status.contains("지원 필요"));
    let copy = output.join("copy.mcdx");
    native(&mut cold, &copy);
    let original_package = NativePackage::open(&initial).unwrap();
    let copied_package = NativePackage::open(&copy).unwrap();
    for part in ["mathcad/worksheet.xml", "mathcad/result.xml"] {
        assert_eq!(
            original_package.original_part(part),
            copied_package.original_part(part)
        );
    }
    *spec_mut(&mut app, id).primary_y_expression_mut().unwrap() = "3*x".into();
    let edited = output.join("edited-no-cache.mathfolio");
    app.document.save(&edited).unwrap();
    let mut cold = WorksheetApp::default();
    cold.open(&edited);
    assert!(cold.last_executed_regions.is_empty());
    assert!(cold.pending_regions.contains(&id));
    assert!(
        cold.plot_results[&id]
            .as_ref()
            .unwrap()
            .first_curve()
            .is_none()
    );
    assert!(
        !cold.region_has_error(id) && !cold.status.contains("지원 필요"),
        "{}",
        cold.status
    );
    for item in std::fs::read_dir(&output).unwrap() {
        std::fs::remove_file(item.unwrap().path()).unwrap();
    }
    std::fs::remove_dir(output).unwrap();
}
