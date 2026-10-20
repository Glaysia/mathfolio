//! Actual save/open lifecycle probes, kept apart from all-editor stress cases.
use super::*;

fn phases() -> usize {
    RECORDS.with_borrow(|records| records.as_ref().map_or(0, Vec::len))
}

fn key(key: egui::Key, pressed: bool) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }
}

fn frame(
    app: &mut WorksheetApp,
    context: &egui::Context,
    events: Vec<egui::Event>,
) -> serde_json::Value {
    let before = phases();
    let detailed = crate::frame_profile::Frame::start();
    let start = Instant::now();
    super::super::super::tests::frame(app, context, events);
    let cpu_us = start.elapsed().as_micros();
    let detailed = detailed.finish();
    let mut result = serde_json::json!({"cpu_us": cpu_us, "prepare_calls": phases()-before,
        "editors": app.editors.len(), "drafts": app.uncommitted_math.len(),
        "generation": app.background.submitted_generation(), "executed": app.last_executed_regions.len(),
        "calculation_error": app.calculation_error, "dirty": app.dirty});
    if let Some(detailed) = detailed {
        result["frame_phases"] = detailed;
    }
    result
}

fn target(app: &WorksheetApp) -> u64 {
    app.document
        .regions
        .iter()
        .find(|region| {
            matches!(&region.content, CanvasContent::Math(tree)
        if matches!(tree.kind,NodeKind::Binary(BinaryOp::Define)))
        })
        .expect("fixture contains a definition")
        .id
}

fn focus(app: &mut WorksheetApp, context: &egui::Context) -> serde_json::Value {
    let id = target(app);
    app.selected = Some(id);
    app.focus_new = Some(id);
    let measurement = frame(app, context, vec![]);
    let editor = app.editors.get_mut(&id).unwrap();
    let atom = editor.arena().nodes().find_map(|(id, node)| {
        if let NodeKind::Number(text) = node.kind() {
            Some((id, text.len()))
        } else {
            None
        }
    });
    if let Some((atom, length)) = atom {
        editor.place_text_caret(atom, length).unwrap();
    }
    measurement
}

fn scratch() -> PathBuf {
    let base = std::env::var_os("MATHCRAFT_OPEN_INPUT_OUTPUT")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../artifacts/rea/open-input-latency-20261019")
        });
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let output = base.join(format!("{}-{nonce}", std::process::id()));
    std::fs::create_dir_all(&output).unwrap();
    output
}

fn saved_fixture(output: &Path, name: &str, count: usize, matrix: usize) -> Vec<(String, PathBuf)> {
    let mut source = fixture(count, matrix);
    let without = output.join(format!("{name}-without-cache.mathcraft"));
    source.document.save(&without).unwrap();
    source.recalculate();
    assert!(source.calculation_error.is_none());
    source.persist_calculation_cache();
    let with = output.join(format!("{name}-with-cache.mathcraft"));
    source.document.save(&with).unwrap();
    vec![
        (format!("{name}-without-cache"), without),
        (format!("{name}-with-cache"), with),
    ]
}

fn run_case(name: &str, path: &Path, mode: &str) {
    let context = egui::Context::default();
    crate::configure(&context);
    let mut app = WorksheetApp::default();
    app.enable_background_calculation(context.clone());
    let detailed = crate::frame_profile::Frame::start();
    let start = Instant::now();
    app.open(path);
    let open_us = start.elapsed().as_micros();
    let open_phases = detailed.finish();
    assert!(app.status.starts_with("Opened"), "{}", app.status);
    assert!(app.editors.is_empty());
    assert!(app.last_executed_regions.is_empty() && !app.background.running);
    let generation = app.background.submitted_generation();
    let restored_rows = app.saved_results.len();
    let pending = app.pending_regions.clone();
    let empty = frame(&mut app, &context, vec![]);
    let focus = focus(&mut app, &context);
    assert!(app.last_executed_regions.is_empty() && !app.background.running);
    assert_eq!(app.background.submitted_generation(), generation);
    assert_eq!(app.saved_results.len(), restored_rows);
    assert_eq!(app.pending_regions, pending);
    if name.starts_with("4k") || name.starts_with("100-matrices") {
        assert!(
            app.editors.len() < app.document.regions.len(),
            "open must not create all editors"
        );
    }
    let events = match mode {
        "arrow-press" => vec![key(egui::Key::ArrowLeft, true)],
        "arrow-release" => vec![key(egui::Key::ArrowLeft, false)],
        "text" => vec![egui::Event::Text("1".into())],
        "paste" => vec![egui::Event::Paste("1".into())],
        "ime-preedit" => vec![egui::Event::Ime(egui::ImeEvent::Preedit {
            text: "1".into(),
            active_range_chars: Some(0..1),
        })],
        "ime-commit" => vec![egui::Event::Ime(egui::ImeEvent::Commit("1".into()))],
        _ => unreachable!(),
    };
    let stimulus = frame(&mut app, &context, events);
    assert_eq!(
        app.background.submitted_generation(),
        generation,
        "draft must not submit calculation"
    );
    assert!(app.last_executed_regions.is_empty() && !app.background.running);
    assert_eq!(app.saved_results.len(), restored_rows);
    let changed = !app.uncommitted_math.is_empty();
    assert_eq!(
        changed,
        matches!(mode, "text" | "paste" | "ime-commit"),
        "{name}/{mode}"
    );
    if mode == "text"
        && let Some(output) = std::env::var_os("MATHCRAFT_WORKSHEET_CAPTURE")
    {
        let output = PathBuf::from(output);
        std::fs::create_dir_all(&output).unwrap();
        WORKSHEET_CAPTURE.with_borrow_mut(|target| {
            *target = Some(output.join(format!("{name}.json")));
        });
    }
    let enter = frame(&mut app, &context, vec![key(egui::Key::Enter, true)]);
    WORKSHEET_CAPTURE.with_borrow_mut(|target| assert!(target.is_none()));
    let worker_start = Instant::now();
    let deadline = Instant::now() + std::time::Duration::from_secs(30);
    while app.background.running {
        app.poll_background_calculation();
        assert!(Instant::now() < deadline, "worker deadline");
        std::thread::yield_now();
    }
    let worker_us = worker_start.elapsed().as_micros();
    assert_eq!(
        app.background.submitted_generation(),
        generation + u64::from(changed)
    );
    if !changed {
        assert!(app.last_executed_regions.is_empty());
    }
    println!(
        "OPEN_INPUT {}",
        serde_json::json!({"name":name,"mode":mode,"regions":app.document.regions.len(),
        "open_us":open_us,"open_phases":open_phases,"open_editors":0,"self_cache":app.document.calculation_cache.is_some(),
        "restored_rows":restored_rows,"empty":empty,"focus":focus,"stimulus":stimulus,"enter":enter,
        "worker_remaining_us":worker_us,"final_executed":app.last_executed_regions.len(),
        "final_error":app.calculation_error,"prepare_calls_total":phases()})
    );
}

#[test]
#[ignore = "CPU-only actual saved/opened document first-input lifecycle"]
pub(super) fn actual_open_first_input_profile() {
    let output = scratch();
    let mut fixtures = saved_fixture(&output, "4k-scalars", 4000, 0);
    fixtures.extend(saved_fixture(&output, "100-matrices16", 100, 16));
    for name in ["1015_LCC_S", "1016_LCC_S"] {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../artifacts/rea/unit-selected-sources")
            .join(format!("{name}.mcdx"));
        if path.is_file() {
            fixtures.push((name.into(), path));
        }
    }
    for (name, path) in fixtures {
        for mode in [
            "arrow-press",
            "arrow-release",
            "text",
            "paste",
            "ime-preedit",
            "ime-commit",
        ] {
            RECORDS.with_borrow_mut(|records| *records = Some(Vec::new()));
            run_case(&name, &path, mode);
            RECORDS.with_borrow_mut(|records| *records = None);
        }
    }
}

fn saved_query_fixture() -> (PathBuf, u64) {
    let output = scratch();
    let mut app = fixture(3, 0);
    let mut editor = Editor::new();
    crate::formula::input_text(&mut editor, "v0+1=");
    let id = app.document.add_math(0., 150.);
    app.document.regions.last_mut().unwrap().content =
        CanvasContent::Math(editor.to_portable().unwrap());
    app.recalculate();
    assert!(app.calculation_error.is_none());
    app.persist_calculation_cache();
    let path = output.join("query.mathcraft");
    app.document.save(&path).unwrap();
    (path, id)
}

fn opened_query(path: &Path, automatic: bool) -> (WorksheetApp, egui::Context) {
    let context = egui::Context::default();
    crate::configure(&context);
    let mut app = WorksheetApp {
        automatic_calculation: automatic,
        ..Default::default()
    };
    app.enable_background_calculation(context.clone());
    app.open(path);
    assert!(app.status.starts_with("Opened"));
    assert!(app.editors.is_empty());
    frame(&mut app, &context, vec![]);
    focus(&mut app, &context);
    (app, context)
}

fn settle_opened(app: &mut WorksheetApp) {
    let deadline = Instant::now() + std::time::Duration::from_secs(10);
    while app.background.running {
        app.poll_background_calculation();
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
}

#[test]
fn actual_saved_open_manual_first_text_preserves_results_until_explicit_calculation() {
    let (path, query) = saved_query_fixture();
    let (mut app, context) = opened_query(&path, false);
    let generation = app.background.submitted_generation();
    let saved = app.saved_results.clone();
    assert!(app.last_executed_regions.is_empty() && !app.background.running);
    frame(&mut app, &context, vec![egui::Event::Text("1".into())]);
    assert_eq!(app.background.submitted_generation(), generation);
    assert_eq!(app.saved_results, saved);
    assert_eq!(app.uncommitted_math.len(), 1);
    assert!(app.calculation_error.is_none() && app.results.is_empty());
    frame(&mut app, &context, vec![key(egui::Key::Enter, true)]);
    settle_opened(&mut app);
    assert!(!app.automatic_calculation && app.last_executed_regions.is_empty());
    assert!(app.uncommitted_math.is_empty());
    app.calculate_command(CalculationCommand::Worksheet);
    settle_opened(&mut app);
    assert_eq!(
        app.results[&query].value.as_ref().unwrap().numeric_json()["real"],
        2.
    );
}

#[test]
fn actual_saved_open_ime_preedit_keeps_document_and_saved_error_state_unchanged() {
    let (path, _) = saved_query_fixture();
    let (mut app, context) = opened_query(&path, true);
    let generation = app.background.submitted_generation();
    let regions = app.document.regions.clone();
    let saved = app.saved_results.clone();
    frame(
        &mut app,
        &context,
        vec![egui::Event::Ime(egui::ImeEvent::Preedit {
            text: "한".into(),
            active_range_chars: Some(0..1),
        })],
    );
    assert_eq!(app.document.regions, regions);
    assert_eq!(app.saved_results, saved);
    assert_eq!(app.background.submitted_generation(), generation);
    assert!(app.uncommitted_math.is_empty() && app.last_executed_regions.is_empty());
    assert!(app.calculation_error.is_none() && !app.background.running);
}

#[test]
fn actual_saved_open_incomplete_first_edit_does_not_publish_fault_before_enter() {
    let (path, query) = saved_query_fixture();
    let (mut app, context) = opened_query(&path, true);
    let generation = app.background.submitted_generation();
    let saved = app.saved_results.clone();
    frame(&mut app, &context, vec![egui::Event::Text("+".into())]);
    assert_eq!(app.background.submitted_generation(), generation);
    assert_eq!(app.saved_results, saved);
    assert!(app.calculation_error.is_none() && app.results.is_empty());
    assert_eq!(app.uncommitted_math.len(), 1);
    frame(&mut app, &context, vec![key(egui::Key::Enter, true)]);
    settle_opened(&mut app);
    assert!(app.results[&query].error.is_some());
}

#[test]
fn actual_saved_open_offscreen_f5_uses_old_source_while_first_text_is_still_a_draft() {
    let (path, query) = saved_query_fixture();
    let (mut app, context) = opened_query(&path, false);
    let definition = target(&app);
    let generation = app.background.submitted_generation();
    frame(&mut app, &context, vec![egui::Event::Text("1".into())]);
    assert_eq!(app.background.submitted_generation(), generation);
    assert!(app.uncommitted_math.contains(&definition));
    app.visible_regions.clear();
    app.visible_calculation_end = Some(query);
    app.calculate_command(CalculationCommand::Now);
    settle_opened(&mut app);
    assert!(app.uncommitted_math.contains(&definition));
    assert!(app.calculation_error.is_none());
    assert_eq!(
        app.results[&query].value.as_ref().unwrap().numeric_json()["real"],
        1.
    );
    frame(&mut app, &context, vec![key(egui::Key::Enter, true)]);
    settle_opened(&mut app);
    app.calculate_command(CalculationCommand::Worksheet);
    settle_opened(&mut app);
    assert_eq!(
        app.results[&query].value.as_ref().unwrap().numeric_json()["real"],
        2.
    );
}

#[test]
#[ignore = "CPU-only actual save/open page-frame phase partition"]
fn actual_open_frame_phase_profile() {
    crate::frame_profile::record(actual_open_first_input_profile);
}

#[test]
#[ignore = "exact current-rendered intrinsic size versus old full typesetting, not timing"]
fn actual_open_enter_height_oracle() {
    let count = crate::frame_profile::with_height_oracle(actual_open_first_input_profile);
    assert!(count >= 12, "positive guarded reuse count {count}");
    println!("ENTER_HEIGHT_ORACLE {count}");
}

#[test]
#[ignore = "CPU-only actual page frame plus tessellation, no renderer/GPU"]
fn actual_open_frame_tessellation_profile() {
    crate::frame_profile::with_tessellation(actual_open_first_input_profile);
}

#[test]
fn intrinsic_enter_height_matches_legacy_for_font_density_fraction_and_fractional_position() {
    for font_points in [4., 11., 36.] {
        for zoom in [1., 2.] {
            for expression in ["v:1/2", "v:12^3", "v:123"] {
                let context = egui::Context::default();
                crate::configure(&context);
                context.set_pixels_per_point(2.);
                let mut app = fixture(1, 0);
                let id = app.document.regions[0].id;
                let mut editor = Editor::new();
                crate::formula::input_text(&mut editor, expression);
                app.document.regions[0].content =
                    CanvasContent::Math(editor.to_portable().unwrap());
                app.document.regions[0].format.font_points = font_points;
                app.document.regions[0].left = 63.3333;
                app.document.regions[0].top = 123.4567;
                app.editors.insert(id, editor);
                app.selected = Some(id);
                app.focus_new = Some(id);
                crate::typography::with_worksheet_zoom(&context, zoom, || {
                    frame(&mut app, &context, vec![]);
                    let count = crate::frame_profile::with_height_oracle(|| {
                        frame(&mut app, &context, vec![key(egui::Key::Enter, true)]);
                    });
                    assert_eq!(count, 1, "{expression} font={font_points} zoom={zoom}");
                });
            }
        }
    }
}

#[test]
fn mixed_text_enter_uses_changed_formula_fallback_and_undo_restores_source() {
    let (path, _) = saved_query_fixture();
    let (mut app, context) = opened_query(&path, false);
    let original = app.document.regions.clone();
    let count = crate::frame_profile::with_height_oracle(|| {
        frame(
            &mut app,
            &context,
            vec![egui::Event::Text("1".into()), key(egui::Key::Enter, true)],
        );
    });
    assert_eq!(count, 0, "changed/native source must remeasure");
    app.undo();
    assert_eq!(app.document.regions, original);
}
