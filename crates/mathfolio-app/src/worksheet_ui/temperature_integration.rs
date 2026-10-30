//! App-generated affine temperature source/display cache flow, not an original
//! Mathcad runtime oracle. Original cached affine unit metadata remains UNKNOWN;
//! the generated bare display coefficient uses our existing cache writer policy.
use super::*;
fn source(app: &WorksheetApp, id: u64) -> &PortableTree {
    match &app
        .document
        .regions
        .iter()
        .find(|r| r.id == id)
        .unwrap()
        .content
    {
        CanvasContent::Math(tree) => tree,
        _ => panic!("Expected temperature math source"),
    }
}
fn source_xml(app: &WorksheetApp, id: u64) -> String {
    worksheet_math::tree_to_mathml(source(app, id)).unwrap()
}
fn unit(tree: &PortableTree, expected: &str) {
    match &tree.kind {
        NodeKind::AnnotatedIdentifier { name, labels }
        | NodeKind::NativeIdentifier { name, labels, .. } => {
            assert_eq!(name, expected);
            assert_eq!(labels, "UNIT");
        }
        other => panic!("Expected explicit UNIT {expected}, found {other:?}"),
    }
    assert!(tree.children.is_empty());
}
fn source_contract(app: &WorksheetApp, id: u64, coefficient: &str, fahrenheit: bool) {
    let tree = source(app, id);
    assert_eq!(tree.kind, NodeKind::Binary(BinaryOp::Evaluate));
    assert_eq!(tree.children.len(), 2);
    let scale = &tree.children[0];
    assert_eq!(scale.kind, NodeKind::Binary(BinaryOp::Scale));
    assert_eq!(scale.children.len(), 2);
    assert_eq!(scale.children[0].kind, NodeKind::Number(coefficient.into()));
    unit(&scale.children[1], "°C");
    if fahrenheit {
        unit(&tree.children[1], "°F");
    } else {
        assert_eq!(tree.children[1].kind, NodeKind::Placeholder);
    }
    fn no_degree(t: &PortableTree) {
        assert_ne!(
            t.kind,
            NodeKind::Degree,
            "Unit action must not become the angle degree operator"
        );
        for child in &t.children {
            no_degree(child);
        }
    }
    no_degree(tree);
}
fn close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-10, "{actual} != {expected}");
}
fn canonical(app: &WorksheetApp, id: u64, expected: f64) {
    let row = app.results.get(&id).unwrap_or_else(|| {
        panic!(
            "Missing result {id}; status={} error={:?} pending={:?} visible={:?}",
            app.status, app.calculation_error, app.pending_regions, app.visible_regions
        )
    });
    let value = row
        .value
        .as_ref()
        .unwrap_or_else(|| panic!("Missing value; error={:?}", row.error));
    let mathfolio_compat::Value::Scalar { quantity: q } = value else {
        panic!("Expected scalar canonical value")
    };
    close(q.value.re, expected);
    close(q.value.im, 0.);
    assert_eq!(q.dimension, mathfolio_core::unit("K").unwrap().dimension);
}
fn live_display(app: &WorksheetApp, id: u64) -> f64 {
    let row = &app.results[&id];
    row.display_value
        .as_ref()
        .or(row.value.as_ref())
        .unwrap()
        .numeric_json()["real"]
        .as_f64()
        .unwrap()
}
fn displayed(app: &WorksheetApp, id: u64) -> String {
    let style = &app
        .document
        .regions
        .iter()
        .find(|r| r.id == id)
        .unwrap()
        .format;
    if let Some(row) = app.results.get(&id)
        && let Some(value) = row.display_value.as_ref().or(row.value.as_ref())
    {
        return crate::result_display::format(value, style).cells[0].plain();
    }
    numeric_cache::display(&app.saved_results[&id], style)
        .unwrap()
        .cells[0]
        .plain()
}
fn frame(app: &mut WorksheetApp, ctx: &egui::Context, events: Vec<egui::Event>) {
    let mut events = events;
    loop {
        let mut raw = egui::RawInput {
            events,
            screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1200., 900.))),
            ..Default::default()
        };
        app.prepare_plot_input(ctx, &mut raw);
        let output = ctx.run_ui(raw, |ui| app.page_ui(ui));
        output.drop_without_applying_deltas();
        if app.plot_input_queue.is_empty() {
            break;
        }
        events = vec![];
    }
}
fn enter() -> egui::Event {
    egui::Event::Key {
        key: egui::Key::Enter,
        physical_key: Some(egui::Key::Enter),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }
}
fn save(app: &mut WorksheetApp, path: &Path) {
    app.path = Some(path.to_owned());
    assert!(app.save(false), "{}", app.status);
}
fn native(app: &mut WorksheetApp, path: &Path) {
    app.prepare_native_export().unwrap();
    mathfolio_document::export::export_mcdx(&app.document, path).unwrap();
}
fn cold(path: &Path, id: u64, xml: &str, display: &str, pending: bool) -> WorksheetApp {
    let mut app = WorksheetApp::default();
    app.open(path);
    assert!(app.last_executed_regions.is_empty(), "{}", app.status);
    assert_eq!(app.document.regions.len(), 1);
    assert_eq!(
        source_xml(&app, id),
        xml,
        "Native/own source must remain source, not a Kelvin replacement literal"
    );
    assert_eq!(displayed(&app, id), display);
    assert_eq!(app.pending_regions.contains(&id), pending);
    app
}
/// Return a final inert native reopen for the parent's print/PDF integration.
pub(super) fn run(output: &Path) -> WorksheetApp {
    std::fs::create_dir_all(output).unwrap();
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    let mut app = WorksheetApp {
        automatic_calculation: false,
        insertion: Some((38.4, 38.4)),
        ..Default::default()
    };
    frame(&mut app, &ctx, vec![egui::Event::Text("20".into())]);
    let id = app.selected.unwrap();
    let before_unit = app.document.calculation_signature();
    app.ribbon_command(&ctx, crate::ribbon::unit_choice("°C"));
    let celsius_signature = app.document.calculation_signature();
    assert_ne!(before_unit, celsius_signature);
    assert!(app.last_executed_regions.is_empty());
    let scaled = source_xml(&app, id);
    app.ribbon_command(&ctx, RibbonAction::Undo);
    assert_eq!(app.document.calculation_signature(), before_unit);
    assert_eq!(source(&app, id).kind, NodeKind::Number("20".into()));
    app.ribbon_command(&ctx, RibbonAction::Redo);
    assert_eq!(app.document.calculation_signature(), celsius_signature);
    assert_eq!(source_xml(&app, id), scaled);
    frame(&mut app, &ctx, vec![]);
    app.ribbon_command(&ctx, RibbonAction::Evaluate);
    source_contract(&app, id, "20", false);
    // In manual mode, creating the eval operator is source authoring; obtain
    // the initial value through the explicit worksheet calculation command.
    frame(&mut app, &ctx, vec![]); // The new query enters the visible command range.
    app.ribbon_command(&ctx, RibbonAction::CalculateNow);
    canonical(&app, id, 293.15);
    let before_override = live_display(&app, id);
    app.last_executed_regions.clear();
    let generation = app.background.submitted_generation();
    let editor = app.editors.get_mut(&id).unwrap();
    let rhs = editor.arena().get(editor.root()).unwrap().children()[1];
    editor.place_caret(rhs, Direction::Left).unwrap();
    app.ribbon_command(&ctx, crate::ribbon::unit_choice("°F"));
    source_contract(&app, id, "20", true);
    frame(&mut app, &ctx, vec![enter()]);
    assert!(app.last_executed_regions.is_empty());
    assert_eq!(app.background.submitted_generation(), generation);
    assert!(app.pending_regions.contains(&id));
    close(live_display(&app, id), before_override);
    app.selected = Some(id);
    app.editing_region = Some(id);
    app.ribbon_command(&ctx, RibbonAction::CalculateNow);
    canonical(&app, id, 293.15);
    close(live_display(&app, id), 68.);
    assert_eq!(displayed(&app, id), "68");
    let first_source = source_xml(&app, id);
    let native_path = output.join("temperature.mcdx");
    native(&mut app, &native_path);
    save(&mut app, &output.join("temperature.mathfolio"));
    let _own = cold(
        &output.join("temperature.mathfolio"),
        id,
        &first_source,
        "68",
        false,
    );
    let mut reopened = cold(&native_path, id, &first_source, "68", false);
    source_contract(&reopened, id, "20", true);
    let copied = output.join("temperature-copy.mcdx");
    native(&mut reopened, &copied);
    let original_package = NativePackage::open(&native_path).unwrap();
    let copied_package = NativePackage::open(&copied).unwrap();
    assert_eq!(
        original_package.original_part("mathcad/result.xml"),
        copied_package.original_part("mathcad/result.xml")
    );
    let _copy = cold(&copied, id, &first_source, "68", false);
    // Edit through the real scalar Text route; Enter in manual mode preserves
    // the previous converted coefficient and source-bound pending cache.
    reopened.automatic_calculation = false;
    reopened.selected = Some(id);
    reopened.focus_new = Some(id);
    reopened.insertion = None;
    let edit_ctx = egui::Context::default();
    crate::configure(&edit_ctx);
    frame(&mut reopened, &edit_ctx, vec![]);
    let editor = reopened.editors.get_mut(&id).unwrap();
    let scale = editor.arena().get(editor.root()).unwrap().children()[0];
    let number = editor.arena().get(scale).unwrap().children()[0];
    editor.place_text_selection(number, 0, 2).unwrap();
    let previous_source_signature = reopened.document.calculation_signature();
    let undo = reopened.undo.len();
    reopened.last_executed_regions.clear();
    frame(
        &mut reopened,
        &edit_ctx,
        vec![egui::Event::Text("25".into())],
    );
    assert_eq!(reopened.undo.len(), undo + 1);
    assert!(reopened.uncommitted_math.contains(&id));
    assert!(reopened.last_executed_regions.is_empty());
    assert_eq!(displayed(&reopened, id), "68");
    frame(&mut reopened, &edit_ctx, vec![enter()]);
    assert!(!reopened.uncommitted_math.contains(&id));
    assert!(reopened.pending_regions.contains(&id));
    assert!(reopened.last_executed_regions.is_empty());
    source_contract(&reopened, id, "25", true);
    let edited_source = source_xml(&reopened, id);
    assert_ne!(
        reopened.document.calculation_signature(),
        previous_source_signature
    );
    assert_eq!(displayed(&reopened, id), "68");
    let own_pending = output.join("temperature-pending.mathfolio");
    save(&mut reopened, &own_pending);
    let mut pending = cold(&own_pending, id, &edited_source, "68", true);
    pending.selected = Some(id);
    pending.editing_region = Some(id);
    frame(&mut pending, &edit_ctx, vec![]); // Calculate Now uses the visible range.
    pending.ribbon_command(&edit_ctx, RibbonAction::CalculateNow);
    canonical(&pending, id, 298.15);
    close(live_display(&pending, id), 77.);
    assert_eq!(displayed(&pending, id), "77");
    native(&mut pending, &output.join("temperature-edited.mcdx"));
    let final_app = cold(
        &output.join("temperature-edited.mcdx"),
        id,
        &edited_source,
        "77",
        false,
    );
    source_contract(&final_app, id, "25", true);
    println!(
        "TEMPERATURE_NATIVE regions=1 saved_query_displays=1 saved_reopen_execution=0 native_files=3 own_files=2 unit_labels=true manual_previous=true celsius_kelvin=293.15 fahrenheit=68 edited_fahrenheit=77 original_cached_affine_metadata=unknown"
    );
    final_app
}
#[test]
fn unit_ribbon_temperature_source_manual_commit_native_cold_and_edit() {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let output = std::env::temp_dir().join(format!(
        "mathfolio-temperature-{}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&output).unwrap();
    let _app = run(&output);
    for entry in std::fs::read_dir(&output).unwrap() {
        std::fs::remove_file(entry.unwrap().path()).unwrap();
    }
    std::fs::remove_dir(&output).unwrap();
}
#[test]
#[ignore = "App-generated scalar affine-temperature native/own/cold/vector-PDF probe"]
fn temperature_save_native_export_reopen_probe() {
    let output = PathBuf::from(
        std::env::var_os("MATHFOLIO_TEMPERATURE_OUTPUT").expect("MATHFOLIO_TEMPERATURE_OUTPUT"),
    );
    let mut app = run(&output);
    let mut snapshot = app.print_snapshot();
    let plan = snapshot.plan(PageSelection::All, 300).unwrap();
    let pages = printing::render_vector_pages(&plan, |ui, p, t| snapshot.paint(ui, p, t)).unwrap();
    printing::write_pdf(&output.join("temperature.pdf"), &pages).unwrap();
}
