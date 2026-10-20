//! Full saved physical XY lifecycle through the controller and vector printing.
use super::*;
use mathcraft_document::canvas::{PlotAxis, PlotScaling, PlotSource};
fn plot(app: &WorksheetApp, id: u64) -> &PlotSpec {
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
fn native(app: &mut WorksheetApp, path: &Path) {
    app.prepare_native_export().unwrap();
    mathcraft_document::export::export_mcdx(&app.document, path).unwrap();
}
fn assert_points(app: &WorksheetApp, id: u64) {
    let data = app.plot_results[&id].as_ref().unwrap();
    assert!(!data.has_failures() && !data.has_pending());
    let curve = data.first_curve().unwrap();
    assert_eq!(curve.points.len(), 3);
    assert!((curve.points[1][0] - 2. / 0.0254).abs() < 1e-10);
    assert_eq!(curve.points[1][1], 4.);
    assert!(
        curve
            .presentation
            .as_ref()
            .unwrap()
            .x
            .as_ref()
            .unwrap()
            .residual_units()
            .is_none()
    );
}
fn run(output: &Path) {
    std::fs::create_dir_all(output).unwrap();
    let mut app = WorksheetApp {
        automatic_calculation: false,
        ..Default::default()
    };
    let x = app.document.add_math(20., 20.);
    app.document.regions.iter_mut().find(|r|r.id==x).unwrap().content=CanvasContent::Math(worksheet_math::tree_from_mathml("<ml:define><ml:id>x</ml:id><ml:matrix rows='3' cols='1'><ml:apply><ml:mult/><ml:real>1</ml:real><ml:id labels='UNIT'>m</ml:id></ml:apply><ml:apply><ml:mult/><ml:real>2</ml:real><ml:id labels='UNIT'>m</ml:id></ml:apply><ml:apply><ml:mult/><ml:real>3</ml:real><ml:id labels='UNIT'>m</ml:id></ml:apply></ml:matrix></ml:define>").unwrap());
    let id = app.document.add_plot(20., 110.);
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
    p.source = PlotSource::single("x", "x^2");
    app.recalculate();
    native(&mut app, &output.join("physical-auto.mcdx"));
    let mut auto = WorksheetApp::default();
    auto.open(&output.join("physical-auto.mcdx"));
    let auto_curve = auto.plot_results[&id]
        .as_ref()
        .unwrap()
        .first_curve()
        .unwrap();
    assert_eq!(auto_curve.points[1], [2., 4.]);
    assert_eq!(
        auto_curve
            .presentation
            .as_ref()
            .unwrap()
            .x
            .as_ref()
            .unwrap()
            .residual_units()
            .unwrap()
            .references[0]
            .unit,
        "meter"
    );
    assert!(
        plot(&auto, id)
            .paired_sources()
            .next()
            .unwrap()
            .x
            .scaling
            .is_none()
    );
    assert!(auto.last_executed_regions.is_empty());
    native(&mut auto, &output.join("physical-auto-copy.mcdx"));
    let mut snapshot = auto.print_snapshot();
    let plan = snapshot.plan(PageSelection::All, 300).unwrap();
    let pages = printing::render_vector_pages(&plan, |ui, p, t| snapshot.paint(ui, p, t)).unwrap();
    printing::write_pdf(&output.join("physical-auto.pdf"), &pages).unwrap();
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
    let scale =
        |xml| PlotScaling::from_tree(worksheet_math::tree_from_mathml(xml).unwrap()).unwrap();
    p.set_axis_scaling(PlotAxis::X, 0, scale("<ml:id labels='UNIT'>in</ml:id>"))
        .unwrap();
    p.set_axis_scaling(
        PlotAxis::Y,
        0,
        scale("<ml:apply><ml:pow/><ml:id labels='UNIT'>m</ml:id><ml:real>2</ml:real></ml:apply>"),
    )
    .unwrap();
    let source = p.clone();
    app.recalculate();
    assert_points(&app, id);
    app.persist_calculation_cache();
    let own = output.join("physical-xy.mathcraft");
    app.document.save(&own).unwrap();
    native(&mut app, &output.join("physical-xy.mcdx"));
    let mut cold = WorksheetApp::default();
    cold.open(&own);
    assert_points(&cold, id);
    assert_eq!(plot(&cold, id), &source);
    assert!(cold.last_executed_regions.is_empty());
    let mut cold = WorksheetApp::default();
    cold.open(&output.join("physical-xy.mcdx"));
    assert_points(&cold, id);
    assert!(cold.last_executed_regions.is_empty());
    native(&mut cold, &output.join("physical-xy-copy.mcdx"));
    let original = NativePackage::open(&output.join("physical-xy.mcdx")).unwrap();
    let copied = NativePackage::open(&output.join("physical-xy-copy.mcdx")).unwrap();
    for name in ["mathcad/worksheet.xml", "mathcad/result.xml"] {
        assert_eq!(original.original_part(name), copied.original_part(name));
    }
    let mut snapshot = cold.print_snapshot();
    let plan = snapshot.plan(PageSelection::All, 300).unwrap();
    let pages = printing::render_vector_pages(&plan, |ui, p, t| snapshot.paint(ui, p, t)).unwrap();
    printing::write_pdf(&output.join("physical-xy.pdf"), &pages).unwrap();
    assert!(cold.last_executed_regions.is_empty());
    // A source unit change retains the former coordinate/units association only
    // as Pending until an explicit controller calculation recomputes it.
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    cold.automatic_calculation = false;
    cold.selected = Some(id);
    cold.focus_new = Some(id);
    cold.insertion = None;
    let frame = super::super::tests::frame;
    frame(&mut cold, &ctx, vec![]);
    frame(&mut cold, &ctx, vec![]);
    let original_document = cold.document.clone();
    let original_points = cold.plot_results[&id].clone();
    let generation = cold.background.submitted_generation();
    let key = |key, modifiers| egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    };
    for _ in 0..3 {
        frame(
            &mut cold,
            &ctx,
            vec![key(egui::Key::Tab, egui::Modifiers::NONE)],
        );
    }
    frame(
        &mut cold,
        &ctx,
        vec![
            key(egui::Key::A, egui::Modifiers::COMMAND),
            egui::Event::Text("m".into()),
        ],
    );
    assert_eq!(cold.document, original_document);
    assert_eq!(cold.plot_results[&id], original_points);
    assert_eq!(cold.background.submitted_generation(), generation);
    assert!(cold.undo.is_empty());
    assert!(cold.inline_plots[&id].has_changes());
    frame(&mut cold, &ctx, vec![super::super::tests::enter()]);
    assert_eq!(cold.undo.len(), 1);
    assert!(cold.last_executed_regions.is_empty());
    let scaling = plot(&cold, id)
        .paired_sources()
        .next()
        .unwrap()
        .x
        .scaling
        .unwrap();
    let xml = worksheet_math::tree_to_mathml(scaling.as_tree()).unwrap();
    assert!(xml.contains(">m<") && xml.contains("UNIT"));
    assert!(cold.plot_results[&id].as_ref().unwrap().has_pending());
    native(&mut cold, &output.join("physical-xy-pending.mcdx"));
    let mut pending = WorksheetApp::default();
    pending.open(&output.join("physical-xy-pending.mcdx"));
    assert!(pending.last_executed_regions.is_empty());
    assert!(pending.plot_results[&id].as_ref().unwrap().has_pending());
    pending.recalculate();
    assert_eq!(
        pending.plot_results[&id]
            .as_ref()
            .unwrap()
            .first_curve()
            .unwrap()
            .points[1],
        [2., 4.]
    );
    println!(
        "PHYSICAL_XY_FLOW physical_point_substitution=true X=in Y=m^2 own_v3=true native_units_residual_separate=true native_copy=exact open_print_execution=0 typed_scale_edit=one_commit changed_scale=pending_then_recalculate"
    );
}
#[test]
fn physical_xy_save_native_reopen_and_vector_print() {
    let path = std::env::temp_dir().join(format!(
        "mathcraft-physical-xy-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    run(&path);
    for f in std::fs::read_dir(&path).unwrap() {
        std::fs::remove_file(f.unwrap().path()).unwrap();
    }
    std::fs::remove_dir(path).unwrap();
}
#[test]
#[ignore = "Explicit physical XY units/scaling/save/reopen/vector-PDF fixture"]
fn physical_xy_units_save_native_export_reopen_probe() {
    run(&PathBuf::from(
        std::env::var_os("MATHCRAFT_PHYSICAL_XY_OUTPUT").expect("MATHCRAFT_PHYSICAL_XY_OUTPUT"),
    ));
}
