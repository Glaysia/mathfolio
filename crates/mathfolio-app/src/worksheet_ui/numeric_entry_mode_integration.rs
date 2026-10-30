//! Actual formula Text/Enter, committed values and inert native/own cache reopen.
//! Incremental whole-hex fusion parity remains outside this flow's evidence.
use super::*;
use mathfolio_editor::NumericBase;

fn leaf(kind: NodeKind) -> PortableTree {
    PortableTree {
        kind,
        children: vec![],
    }
}
fn eval(value: PortableTree) -> PortableTree {
    PortableTree {
        kind: NodeKind::Binary(BinaryOp::Evaluate),
        children: vec![value, leaf(NodeKind::Placeholder)],
    }
}
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
        _ => panic!("Expected an authored formula"),
    }
}
fn scalar(app: &WorksheetApp, id: u64) -> f64 {
    match &app.results[&id].value {
        Some(Value::Scalar { quantity }) => {
            assert_eq!(quantity.value.im, 0.);
            quantity.value.re
        }
        _ => panic!("Expected scalar for {id}: {:?}", app.results[&id].error),
    }
}
fn displayed(app: &WorksheetApp, id: u64) -> String {
    let format = &app
        .document
        .regions
        .iter()
        .find(|r| r.id == id)
        .unwrap()
        .format;
    if let Some(value) = app.results.get(&id).and_then(|r| r.value.as_ref()) {
        return crate::result_display::format(value, format).cells[0].plain();
    }
    numeric_cache::display(&app.saved_results[&id], format)
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
        // All assertions happen after dropping deltas; a failed oracle must
        // not trigger a second FullOutput drop panic and hide its cause.
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
fn native(app: &mut WorksheetApp, path: &Path) {
    app.prepare_native_export().unwrap();
    mathfolio_document::export::export_mcdx(&app.document, path).unwrap();
}
fn save(app: &mut WorksheetApp, path: &Path) {
    app.path = Some(path.to_owned());
    assert!(app.save(false), "{}", app.status);
}
fn cold(
    path: &Path,
    ids: &[u64],
    sources: &[PortableTree],
    displays: &[String],
    mode: bool,
) -> WorksheetApp {
    let mut app = WorksheetApp::default();
    app.open(path);
    assert!(app.last_executed_regions.is_empty(), "{}", app.status);
    assert_eq!(app.numeric_input_mode(), Some(mode));
    assert!(app.pending_regions.is_empty());
    assert_eq!(app.document.regions.len(), ids.len());
    for ((id, expected), display) in ids.iter().zip(sources).zip(displays) {
        assert_eq!(source(&app, *id), expected);
        assert_eq!(displayed(&app, *id), *display);
        assert_eq!(
            app.saved_results[id].status,
            SavedCalculationStatus::Synchronized
        );
    }
    app
}

/// Returns the final inert native reopen so the parent can add a vector print
/// probe without changing the source/input/cache test's calculation contract.
pub(super) fn run(output: &Path) -> WorksheetApp {
    std::fs::create_dir_all(output).unwrap();
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    let mut app = WorksheetApp {
        automatic_calculation: false,
        ..Default::default()
    };
    let cases = [
        ("1101b", "1101", NumericBase::Binary, 13.),
        ("32o", "32", NumericBase::Octal, 26.),
        ("0a32h", "0a32", NumericBase::Hexadecimal, 2610.),
    ];
    let mut ids = vec![];
    for index in 0..cases.len() {
        let id = app.document.add_math(38.4, 38.4 + index as f64 * 100.);
        app.document.regions.last_mut().unwrap().content =
            CanvasContent::Math(eval(leaf(NodeKind::Number("0".into()))));
        ids.push(id);
    }
    app.recalculate();
    app.last_executed_regions.clear();
    app.ribbon_command(&ctx, RibbonAction::SetBinOctHexMode(true));
    assert_eq!(app.numeric_input_mode(), Some(true));
    assert!(app.last_executed_regions.is_empty());
    let mut sources = vec![];
    for (id, (text, digits, base, expected)) in ids.iter().copied().zip(cases) {
        app.selected = Some(id);
        app.focus_new = Some(id);
        app.insertion = None;
        frame(&mut app, &ctx, vec![]);
        let editor = app.editors.get_mut(&id).unwrap();
        assert!(editor.bin_oct_hex_mode());
        let left = editor.arena().get(editor.root()).unwrap().children()[0];
        editor.place_text_selection(left, 0, 1).unwrap();
        app.last_executed_regions.clear();
        let generation = app.background.submitted_generation();
        frame(&mut app, &ctx, vec![egui::Event::Text(text.into())]);
        let expected_source = eval(leaf(NodeKind::based_number(digits, base).unwrap()));
        assert_eq!(
            app.editors[&id].to_portable().unwrap(),
            expected_source,
            "{text}"
        );
        assert!(app.uncommitted_math.contains(&id));
        assert!(app.last_executed_regions.is_empty());
        assert_eq!(app.background.submitted_generation(), generation);
        assert_eq!(scalar(&app, id), 0.);
        frame(&mut app, &ctx, vec![enter()]);
        assert!(!app.uncommitted_math.contains(&id));
        assert_eq!(source(&app, id), &expected_source);
        assert_eq!(
            scalar(&app, id),
            0.,
            "manual Enter keeps the previous result"
        );
        assert!(app.pending_regions.contains(&id));
        assert!(app.last_executed_regions.is_empty());
        app.ribbon_command(&ctx, RibbonAction::CalculateNow);
        assert_eq!(scalar(&app, id), expected, "{text}");
        sources.push(expected_source);
    }
    // Establish an independently checked fresh complete cache for all owners,
    // rather than deriving cold expected values from formatted cache payloads.
    app.recalculate();
    for (id, (_, _, _, expected)) in ids.iter().copied().zip(cases) {
        assert_eq!(scalar(&app, id), expected);
    }
    let displays: Vec<_> = ids.iter().map(|id| displayed(&app, *id)).collect();
    native(&mut app, &output.join("radix-input.mcdx"));
    save(&mut app, &output.join("radix-input.mathfolio"));
    let _own = cold(
        &output.join("radix-input.mathfolio"),
        &ids,
        &sources,
        &displays,
        true,
    );
    let mut reopened = cold(
        &output.join("radix-input.mcdx"),
        &ids,
        &sources,
        &displays,
        true,
    );
    native(&mut reopened, &output.join("radix-input-copy.mcdx"));
    let original = NativePackage::open(&output.join("radix-input.mcdx")).unwrap();
    let copied = NativePackage::open(&output.join("radix-input-copy.mcdx")).unwrap();
    assert_eq!(
        original.original_part("mathcad/result.xml"),
        copied.original_part("mathcad/result.xml")
    );
    assert!(mathfolio_document::native_calculation::bin_oct_hex_mode(&original).unwrap());
    // An authoring-mode toggle must never reinterpret an existing native atom,
    // mutate its calculation signature or execute source while restoring it.
    let signature = reopened.document.calculation_signature();
    let pending = reopened.pending_regions.clone();
    let undo = reopened.undo.len();
    reopened.ribbon_command(&ctx, RibbonAction::SetBinOctHexMode(false));
    assert_eq!(reopened.numeric_input_mode(), Some(false));
    assert_eq!(reopened.document.calculation_signature(), signature);
    assert_eq!(reopened.pending_regions, pending);
    assert!(reopened.last_executed_regions.is_empty());
    assert_eq!(reopened.undo.len(), undo + 1);
    for ((id, expected), display) in ids.iter().zip(&sources).zip(&displays) {
        assert_eq!(source(&reopened, *id), expected);
        assert_eq!(displayed(&reopened, *id), *display);
    }
    let inactive_ctx = egui::Context::default();
    crate::configure(&inactive_ctx);
    reopened.selected = Some(ids[0]);
    reopened.focus_new = Some(ids[0]);
    reopened.insertion = None;
    frame(&mut reopened, &inactive_ctx, vec![]);
    assert!(!reopened.editors[&ids[0]].bin_oct_hex_mode());
    assert_eq!(reopened.editors[&ids[0]].to_portable().unwrap(), sources[0]);
    assert!(reopened.last_executed_regions.is_empty());
    // Input mode has no effect on evaluation of explicit based source. Obtain
    // fresh trusted results under the new package settings before export;
    // this does not promote an opaque native cache after a context change.
    reopened.recalculate();
    for (id, (_, _, _, expected)) in ids.iter().copied().zip(cases) {
        assert_eq!(scalar(&reopened, id), expected);
    }
    native(&mut reopened, &output.join("radix-input-mode-off.mcdx"));
    save(
        &mut reopened,
        &output.join("radix-input-mode-off.mathfolio"),
    );
    let _own_off = cold(
        &output.join("radix-input-mode-off.mathfolio"),
        &ids,
        &sources,
        &displays,
        false,
    );
    let off = cold(
        &output.join("radix-input-mode-off.mcdx"),
        &ids,
        &sources,
        &displays,
        false,
    );
    let off_package = NativePackage::open(&output.join("radix-input-mode-off.mcdx")).unwrap();
    assert!(!mathfolio_document::native_calculation::bin_oct_hex_mode(&off_package).unwrap());
    println!(
        "RADIX_INPUT_FLOW regions=3 queries=3 real_text_key=true values=13,26,2610 native_own_cold_execution=0 mode_on_persisted=true mode_off_preserves_based=true result_copy_exact=true"
    );
    let mut off = off;
    let mut snapshot = off.print_snapshot();
    let plan = snapshot.plan(PageSelection::All, 300).unwrap();
    let pages = printing::render_vector_pages(&plan, |ui, p, t| snapshot.paint(ui, p, t)).unwrap();
    printing::write_pdf(&output.join("radix-input.pdf"), &pages).unwrap();
    assert!(off.last_executed_regions.is_empty());
    off
}

#[test]
fn actual_radix_input_enter_native_and_own_cold_open_keep_source_mode_and_results() {
    let path = std::env::temp_dir().join(format!(
        "mathfolio-radix-input-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&path).unwrap();
    let _app = run(&path);
    for entry in std::fs::read_dir(&path).unwrap() {
        std::fs::remove_file(entry.unwrap().path()).unwrap();
    }
    std::fs::remove_dir(path).unwrap();
}

#[test]
#[ignore = "Explicit radix authoring/native/own cold-open fixture; no GUI"]
fn radix_input_save_native_export_reopen_probe() {
    let path = PathBuf::from(
        std::env::var_os("MATHFOLIO_RADIX_INPUT_OUTPUT").expect("MATHFOLIO_RADIX_INPUT_OUTPUT"),
    );
    let _app = run(&path);
}
