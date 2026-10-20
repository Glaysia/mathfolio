//! Authored typed source, actual exponent edits, physical units and cold caches.
use super::*;
use mathcraft_document::canvas::{PlotAxis, PlotExpression, PlotScaling};
fn tree(kind: NodeKind, children: Vec<PortableTree>) -> PortableTree {
    PortableTree { kind, children }
}
fn number(n: &str) -> PortableTree {
    tree(NodeKind::Number(n.into()), vec![])
}
fn id(name: &str, label: &str, suffix: Option<&str>) -> PortableTree {
    tree(
        NodeKind::NativeIdentifier {
            name: name.into(),
            labels: label.into(),
            contextual: Some(false),
            subscript: suffix.map(str::to_owned),
        },
        vec![],
    )
}
fn power(base: PortableTree, exponent: &str) -> PortableTree {
    tree(NodeKind::Power, vec![base, number(exponent)])
}
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
fn assert_curve(app: &WorksheetApp, id: u64, y: f64) {
    let data = app.plot_results[&id].as_ref().unwrap();
    assert!(!data.has_pending() && !data.has_failures(), "{data:?}");
    let c = data.first_curve().unwrap();
    assert_eq!(c.points.len(), 3);
    assert!((c.points[1][0] - 2. / 0.0254).abs() < 1e-10);
    assert_eq!(c.points[1][1], y);
}
fn native(app: &mut WorksheetApp, path: &Path) {
    app.prepare_native_export().unwrap();
    mathcraft_document::export::export_mcdx(&app.document, path).unwrap();
}
fn run(output: &Path) {
    std::fs::create_dir_all(output).unwrap();
    let mut app = WorksheetApp {
        automatic_calculation: false,
        ..Default::default()
    };
    let x = id("x", "VARIABLE", Some("dc𝛼"));
    let physical = |n| {
        tree(
            NodeKind::Binary(BinaryOp::Multiply),
            vec![number(n), id("m", "UNIT", None)],
        )
    };
    let definition = app.document.add_math(20., 20.);
    app.document
        .regions
        .iter_mut()
        .find(|r| r.id == definition)
        .unwrap()
        .content = CanvasContent::Math(tree(
        NodeKind::Binary(BinaryOp::Define),
        vec![
            x.clone(),
            tree(
                NodeKind::Matrix {
                    rows: 3,
                    cols: 1,
                    display: None,
                },
                vec![physical("1"), physical("2"), physical("3")],
            ),
        ],
    ));
    let plot = app.document.add_plot(20., 110.);
    let mut source = PlotSpec::typed_blank_xy();
    source
        .set_axis_expression(
            PlotAxis::X,
            0,
            PlotExpression::from_tree(x.clone()).unwrap(),
        )
        .unwrap();
    source
        .set_axis_expression(
            PlotAxis::Y,
            0,
            PlotExpression::from_tree(power(x.clone(), "2")).unwrap(),
        )
        .unwrap();
    source
        .set_axis_scaling(
            PlotAxis::X,
            0,
            PlotScaling::from_tree(id("in", "UNIT", None)).unwrap(),
        )
        .unwrap();
    source
        .set_axis_scaling(
            PlotAxis::Y,
            0,
            PlotScaling::from_tree(power(id("m", "UNIT", None), "2")).unwrap(),
        )
        .unwrap();
    app.document
        .regions
        .iter_mut()
        .find(|r| r.id == plot)
        .unwrap()
        .content = CanvasContent::Plot(source);
    app.recalculate();
    assert_curve(&app, plot, 4.);
    native(&mut app, &output.join("typed-source.mcdx"));
    app.document
        .save(&output.join("typed-source.mathcraft"))
        .unwrap();
    let mut cold = WorksheetApp::default();
    cold.open(&output.join("typed-source.mcdx"));
    assert_curve(&cold, plot, 4.);
    assert!(cold.last_executed_regions.is_empty());
    assert_eq!(spec(&cold, plot).primary_x_expression().as_tree(), Some(&x));
    native(&mut cold, &output.join("typed-source-copy.mcdx"));
    let a = NativePackage::open(&output.join("typed-source.mcdx")).unwrap();
    let b = NativePackage::open(&output.join("typed-source-copy.mcdx")).unwrap();
    for part in ["mathcad/worksheet.xml", "mathcad/result.xml"] {
        assert_eq!(a.original_part(part), b.original_part(part));
    }
    cold.automatic_calculation = false;
    cold.selected = Some(plot);
    cold.focus_new = Some(plot);
    cold.insertion = None;
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    let frame = super::super::tests::frame;
    frame(&mut cold, &ctx, vec![]);
    frame(&mut cold, &ctx, vec![]);
    let initial_widget = ctx.memory(|memory| memory.focused()).unwrap();
    let initial_widget_generation = cold.inline_plots[&plot].widget_generation();
    let original = cold.document.clone();
    let curves = cold.plot_results[&plot].clone();
    let generation = cold.background.submitted_generation();
    let edit = || {
        vec![
            key(egui::Key::End, egui::Modifiers::NONE),
            key(egui::Key::ArrowLeft, egui::Modifiers::SHIFT),
            egui::Event::Text("3".into()),
        ]
    };
    frame(&mut cold, &ctx, edit());
    frame(
        &mut cold,
        &ctx,
        vec![key(egui::Key::Tab, egui::Modifiers::NONE)],
    );
    frame(&mut cold, &ctx, edit());
    assert_eq!(cold.document, original);
    assert_eq!(cold.plot_results[&plot], curves);
    assert_eq!(cold.background.submitted_generation(), generation);
    assert!(cold.undo.is_empty() && cold.last_executed_regions.is_empty());
    frame(&mut cold, &ctx, vec![super::super::tests::enter()]);
    assert_eq!(cold.undo.len(), 1);
    assert!(cold.last_executed_regions.is_empty());
    let committed = spec(&cold, plot).clone();
    assert_eq!(
        committed.primary_y_expression().as_tree(),
        Some(&power(x.clone(), "3"))
    );
    assert!(cold.plot_results[&plot].as_ref().unwrap().has_pending());
    ctx.data_mut(|data| {
        data.insert_temp(
            initial_widget.with("ime-preedit"),
            "old composition".to_owned(),
        )
    });
    ctx.memory_mut(|memory| memory.request_focus(initial_widget));
    cold.undo();
    assert_eq!(
        cold.document.regions.iter().find(|r| r.id == plot).unwrap(),
        original.regions.iter().find(|r| r.id == plot).unwrap()
    );
    let undo_generation = cold.plot_widget_generations[&plot];
    assert!(undo_generation > initial_widget_generation);
    frame(&mut cold, &ctx, vec![]);
    let restored_generation = cold.inline_plots[&plot].widget_generation();
    assert!(restored_generation >= undo_generation);
    assert!(
        ctx.data(|data| data.get_temp::<String>(initial_widget.with("ime-preedit")))
            .is_none()
    );
    assert_ne!(ctx.memory(|memory| memory.focused()), Some(initial_widget));
    cold.redo();
    assert_eq!(spec(&cold, plot), &committed);
    assert!(cold.plot_widget_generations[&plot] > restored_generation);
    frame(&mut cold, &ctx, vec![]);
    native(&mut cold, &output.join("typed-source-pending.mcdx"));
    cold.document
        .save(&output.join("typed-source-pending.mathcraft"))
        .unwrap();
    let mut pending = WorksheetApp::default();
    pending.open(&output.join("typed-source-pending.mathcraft"));
    assert!(pending.last_executed_regions.is_empty());
    assert_eq!(spec(&pending, plot), &committed);
    assert!(pending.plot_results[&plot].as_ref().unwrap().has_pending());
    pending.recalculate();
    assert_curve(&pending, plot, 8.);
    native(&mut pending, &output.join("typed-source-edited.mcdx"));
    let mut reopened = WorksheetApp::default();
    reopened.open(&output.join("typed-source-edited.mcdx"));
    assert_curve(&reopened, plot, 8.);
    assert!(reopened.last_executed_regions.is_empty());
    assert_eq!(
        spec(&reopened, plot).primary_x_expression().as_tree(),
        Some(&x)
    );
    let mut snapshot = reopened.print_snapshot();
    let plan = snapshot.plan(PageSelection::All, 300).unwrap();
    let pages = printing::render_vector_pages(&plan, |ui, p, t| snapshot.paint(ui, p, t)).unwrap();
    printing::write_pdf(&output.join("typed-source.pdf"), &pages).unwrap();
    assert!(reopened.last_executed_regions.is_empty());
    println!(
        "TYPED_XY_FLOW first_math=direct_tree VARIABLE_context_false_literal_suffix=true actual_keys=end+shift_left+text+tab+edit+enter history=1 old_curve_pending=true physical_pow2_to_pow3=true own_native_cold_execution=0 vector_pdf=true"
    );
}
#[test]
fn typed_source_real_edit_native_cold_undo_physical_recalculation_and_vector_print() {
    let path = std::env::temp_dir().join(format!(
        "mathcraft-typed-source-{}-{}",
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
fn deleted_typed_owner_undo_retires_focus_and_keeps_widget_generation_floor() {
    let mut app = WorksheetApp {
        automatic_calculation: false,
        ..Default::default()
    };
    let plot = app.document.add_plot(20., 20.);
    app.document
        .regions
        .iter_mut()
        .find(|r| r.id == plot)
        .unwrap()
        .content = CanvasContent::Plot(PlotSpec::typed_blank_xy());
    app.selected = Some(plot);
    app.focus_new = Some(plot);
    app.insertion = None;
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    let frame = super::super::tests::frame;
    frame(&mut app, &ctx, vec![]);
    frame(&mut app, &ctx, vec![]);
    let widget = ctx.memory(|m| m.focused()).unwrap();
    let generation = app.inline_plots[&plot].widget_generation();
    ctx.data_mut(|d| d.insert_temp(widget.with("ime-preedit"), "stale composition".to_owned()));
    app.remove_selected();
    assert!(app.plot_widget_generations[&plot] > generation);
    assert!(!app.inline_plots.contains_key(&plot));
    app.undo();
    frame(&mut app, &ctx, vec![]);
    assert!(app.inline_plots[&plot].widget_generation() > generation);
    assert_ne!(ctx.memory(|m| m.focused()), Some(widget));
    assert!(
        ctx.data(|d| d.get_temp::<String>(widget.with("ime-preedit")))
            .is_none()
    );
    assert!(app.last_executed_regions.is_empty());
    assert!(
        spec(&app, plot)
            .primary_y_expression()
            .is_exact_placeholder()
    );
}
#[test]
#[ignore = "Explicit typed source/physical edit/native/cold/vector-PDF probe"]
fn typed_xy_source_save_native_export_reopen_probe() {
    run(&PathBuf::from(
        std::env::var_os("MATHCRAFT_TYPED_XY_OUTPUT").expect("MATHCRAFT_TYPED_XY_OUTPUT"),
    ));
}
