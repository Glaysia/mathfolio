//! Real Rust calculation setup, then inert view/save/print/cold workflow.
//! Native original print hierarchy/geometry parity is not claimed.
use super::*;
use crate::result_display::{MatrixProjection, PreparedNativeMatrix};
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicU64, Ordering};

const ML: &str = "http://schemas.mathsoft.com/math50";
const WS: &str = "http://schemas.mathsoft.com/worksheet50";
static SERIAL: AtomicU64 = AtomicU64::new(0);
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        loop {
            let path = std::env::temp_dir().join(format!(
                "matrix-live-indices-{}-{}",
                std::process::id(),
                SERIAL.fetch_add(1, Ordering::Relaxed)
            ));
            match std::fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("{error}"),
            }
        }
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn tree(body: &str) -> PortableTree {
    worksheet_math::tree_from_mathml(&format!("<context xmlns:ml=\"{ML}\">{body}</context>"))
        .unwrap()
}
fn add(app: &mut WorksheetApp, top: f64, body: &str) -> u64 {
    let id = app.document.add_math(20., top);
    app.document.regions.last_mut().unwrap().content = CanvasContent::Math(tree(body));
    id
}
fn query(app: &WorksheetApp) -> u64 {
    let mut rows = app.document.regions.iter().filter(|region| matches!(&region.content, CanvasContent::Math(tree) if matches!(tree.kind,NodeKind::Binary(BinaryOp::Evaluate))));
    let id = rows.next().unwrap().id;
    assert!(rows.next().is_none());
    id
}
fn frame(app: &mut WorksheetApp, ctx: &egui::Context, tick: u64) {
    let mut raw = egui::RawInput {
        time: Some(tick as f64 / 60.),
        screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(2400., 2200.))),
        ..Default::default()
    };
    eframe::App::raw_input_hook(app, ctx, &mut raw);
    ctx.run_ui(raw, |ui| app.app_ui_frame(ui))
        .drop_without_applying_deltas();
}
fn source_focus(app: &WorksheetApp, ctx: &egui::Context, owner: u64) -> serde_json::Value {
    let focused = ctx.memory(|memory| memory.focused());
    let query = super::super::matrix_result_view_app_tests::observed_formula(owner)
        .expect("actual query widget was not observed");
    let mut sources = Vec::new();
    for region in &app.document.regions {
        if !matches!(region.content, CanvasContent::Math(_)) {
            continue;
        }
        let response = super::super::matrix_result_view_app_tests::observed_formula(region.id)
            .expect("actual source widget was not observed");
        let preedit = ctx
            .data(|data| data.get_temp::<String>(response.id.with("ime-preedit")))
            .unwrap_or_default();
        assert!(
            !response.has_focus() && focused != Some(response.id),
            "source {} focused: global={focused:?}, actual_id={:?}, actual_has_focus={}",
            region.id,
            response.id,
            response.has_focus()
        );
        assert!(preedit.is_empty(), "source {} has IME preedit", region.id);
        sources.push(serde_json::json!({"owner":region.id,"response_id":format!("{:?}",response.id),"has_focus":response.has_focus(),"ime_preedit_chars":preedit.chars().count()}));
    }
    assert!(app.editing_region.is_none() && app.focus_new.is_none());
    assert!(app.text_composition.is_none());
    serde_json::json!({"global_focused_id":focused.map(|id|format!("{id:?}")),"query_response_id":format!("{:?}",query.id),"query_has_focus":query.has_focus(),"sources":sources})
}
fn rows(app: &WorksheetApp) -> Vec<(u64, serde_json::Value)> {
    let mut rows: Vec<_> = app
        .results
        .iter()
        .map(|(id, row)| (*id, serde_json::to_value(row).unwrap()))
        .collect();
    rows.sort_by_key(|row| row.0);
    rows
}
struct Inert {
    regions: Vec<CanvasRegion>,
    cache: Option<SavedCalculationCache>,
    saved: HashMap<u64, SavedCalculationRow>,
    rows: Vec<(u64, serde_json::Value)>,
    signature: String,
    active: String,
    generation: u64,
}
impl Inert {
    fn new(app: &WorksheetApp) -> Self {
        Self {
            regions: app.document.regions.clone(),
            cache: app.document.calculation_cache.clone(),
            saved: app.saved_results.clone(),
            rows: rows(app),
            signature: app.document.calculation_signature(),
            active: app.document.active_calculation_signature(),
            generation: app.background.submitted_generation(),
        }
    }
    fn check(&self, app: &WorksheetApp) {
        assert_eq!(app.document.regions, self.regions);
        assert_eq!(app.document.calculation_cache, self.cache);
        assert_eq!(app.saved_results, self.saved);
        assert_eq!(rows(app), self.rows);
        assert_eq!(app.document.calculation_signature(), self.signature);
        assert_eq!(app.document.active_calculation_signature(), self.active);
        assert_eq!(app.background.submitted_generation(), self.generation);
        assert!(app.last_executed_regions.is_empty());
        assert!(app.pending_regions.is_empty());
        assert!(!app.calculation_pending && !app.background.running);
    }
}
fn identity_projection(projection: &MatrixProjection) {
    assert_eq!((projection.full_rows, projection.full_cols), (20, 20));
    assert_eq!(
        (
            projection.rows,
            projection.cols,
            projection.row_offset,
            projection.col_offset
        ),
        (12, 12, 0, 0)
    );
    assert_eq!(projection.cells.len(), 144);
    for row in 0..12 {
        for col in 0..12 {
            assert_eq!(
                projection.cells[row * 12 + col].plain(),
                if row == col { "1" } else { "0" }
            );
        }
    }
    assert_eq!(
        projection
            .cells
            .iter()
            .filter(|cell| cell.plain() == "1")
            .count(),
        12
    );
    assert_eq!(
        projection
            .cells
            .iter()
            .filter(|cell| cell.plain() == "0")
            .count(),
        132
    );
}
fn snapshot(app: &mut WorksheetApp, owner: u64, show: bool) -> (PrintSnapshot, f64) {
    let mut snapshot = app.print_snapshot();
    assert!(snapshot.matrix_errors.is_empty());
    let projection = &snapshot.matrix_displays[&owner];
    identity_projection(projection);
    let expected = formula::MatrixIndexLabels::for_projection(projection, 1000, show).unwrap();
    assert_eq!(snapshot.matrix_indices.get(&owner), Some(&expected));
    assert_eq!(app.display_cache[&owner].matrix.value(), Some(projection));
    let plan = snapshot.plan(PageSelection::All, 300).unwrap();
    assert_eq!(plan.page_count(), 1);
    let source = snapshot
        .document
        .regions
        .iter()
        .find(|r| r.id == owner)
        .unwrap();
    assert!(
        source.source_region.is_none(),
        "this live source fixture uses own top-edge geometry"
    );
    let alignment = f64::from(snapshot.alignment_offsets[&owner]);
    assert_eq!(alignment, 0.);
    let y = f64::from(
        snapshot
            .geometry
            .to_paper(source.left, source.top + alignment)
            .y,
    ) * 0.75;
    assert!(
        y >= 300.,
        "query must be fully below the independent PDF crop"
    );
    assert!(
        y + f64::from(snapshot.heights[&owner]) * 0.75
            <= f64::from(snapshot.geometry.height - snapshot.geometry.margins[3]) * 0.75
    );
    (snapshot, y)
}
fn native(path: &Path) -> (NativePackage, String, Vec<u8>) {
    let package = NativePackage::open(path).unwrap();
    let source = package.worksheet_xml().unwrap();
    let xml = roxmltree::Document::parse(&source).unwrap();
    let mut candidates = xml.descendants().filter(|n| {
        n.has_tag_name((WS, "math")) && n.children().any(|c| c.has_tag_name((ML, "eval")))
    });
    let reference = candidates
        .next()
        .unwrap()
        .attribute("resultRef")
        .unwrap()
        .to_owned();
    assert!(candidates.next().is_none());
    let caches = package.caches().unwrap();
    let row = caches
        .iter()
        .find(|row| row.matches_result_ref(&reference))
        .unwrap();
    assert_eq!(row.calculation_status.as_deref(), Some("Synchronized"));
    let full = PreparedNativeMatrix::parse(row.result_mathml.as_deref().unwrap()).unwrap();
    assert_eq!(full.shape(), (20, 20));
    let value = full
        .project(
            crate::result_display::MatrixWindow {
                rows: 20,
                cols: 20,
                row_offset: 0,
                col_offset: 0,
            },
            &MathFormat {
                result_format: ResultFormat::Decimal,
                precision: 0,
                ..Default::default()
            },
        )
        .unwrap();
    for row in 0..20 {
        for col in 0..20 {
            assert_eq!(
                value.cells[row * 20 + col].plain(),
                if row == col { "1" } else { "0" }
            );
        }
    }
    let raw = package
        .original_part("mathcad/result.xml")
        .unwrap()
        .to_vec();
    assert!(!String::from_utf8_lossy(&raw).contains("matrix_origin"));
    (package, reference, raw)
}
fn cold(path: &Path, show: bool) {
    let file_before = std::fs::read(path).unwrap();
    let mut app = WorksheetApp::default();
    app.open(path);
    let owner = query(&app);
    app.selected = Some(owner);
    assert!(app.results.is_empty());
    assert!(app.pending_regions.is_empty(), "{}", app.status);
    assert_eq!(app.result_matrix_indices(owner), None);
    let effective = app
        .matrix_view_sources
        .effective(owner, app.requested_matrix_view(owner))
        .unwrap();
    assert_eq!(effective.show_indices, Some(show));
    assert_eq!(
        app.saved_results[&owner].status,
        SavedCalculationStatus::Synchronized
    );
    let before = Inert::new(&app);
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    for tick in 0..4 {
        frame(&mut app, &ctx, tick);
    }
    assert!(app.matrix_indices_context().is_none());
    assert_eq!(app.result_matrix_indices(owner), None);
    identity_projection(app.display_cache[&owner].matrix.value().unwrap());
    let snapshot = app.print_snapshot();
    assert!(
        !snapshot.matrix_indices.contains_key(&owner),
        "cold source SYSTEM ORIGIN is not execution provenance"
    );
    identity_projection(&snapshot.matrix_displays[&owner]);
    before.check(&app);
    assert!(app.undo.is_empty() && app.redo.is_empty());
    assert_eq!(std::fs::read(path).unwrap(), file_before);
}
fn exercise(output: Option<&Path>) {
    let _observing = super::super::matrix_result_view_app_tests::Observe::start();
    let scratch = Scratch::new();
    let root = output.unwrap_or(&scratch.0);
    let mut app = WorksheetApp {
        automatic_calculation: false,
        ..Default::default()
    };
    app.document.title = "Live matrix indices".into();
    add(
        &mut app,
        20.,
        "<ml:define><ml:id labels=\"SYSTEM\">ORIGIN</ml:id><ml:real>1000</ml:real></ml:define>",
    );
    add(
        &mut app,
        80.,
        "<ml:define><ml:id labels=\"VARIABLE\">M</ml:id><ml:apply><ml:id labels=\"FUNCTION\">identity</ml:id><ml:real>20</ml:real></ml:apply></ml:define>",
    );
    let owner = add(
        &mut app,
        350.,
        "<ml:eval><ml:id labels=\"VARIABLE\">M</ml:id><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>",
    );
    app.document.regions.last_mut().unwrap().format = MathFormat {
        explicit: true,
        result_format: ResultFormat::Decimal,
        precision: 0,
        ..Default::default()
    };
    // One actual Rust worksheet calculation, excluded from zero-execution scope.
    app.recalculate();
    assert!(
        app.results.values().all(|row| row.error.is_none()),
        "{:?}",
        app.results
    );
    assert_eq!(app.results[&owner].matrix_origin, Some(1000));
    let setup_executed = app.last_executed_regions.clone();
    assert!(!setup_executed.is_empty());
    let Some(Value::Matrix {
        rows: full_rows,
        cols: full_cols,
        elements,
    }) = &app.results[&owner].value
    else {
        panic!("identity query was not an executed matrix")
    };
    assert_eq!((*full_rows, *full_cols, elements.len()), (20, 20, 400));
    for row in 0..20 {
        for col in 0..20 {
            let q = &elements[row * 20 + col];
            let expected: f64 = if row == col { 1. } else { 0. };
            assert_eq!(q.value.re.to_bits(), expected.to_bits());
            assert_eq!(q.value.im, 0.);
            assert!(q.is_scalar());
        }
    }
    app.persist_calculation_cache();
    app.selected = Some(owner);
    app.editing_region = None;
    app.focus_new = None;
    app.insertion = None;
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    for tick in 0..4 {
        frame(&mut app, &ctx, tick);
    }
    // The selected read-only worksheet intentionally focuses its background.
    // Distinguish that legitimate navigation focus from every source widget.
    let _setup_focus = source_focus(&app, &ctx, owner);
    app.last_executed_regions.clear();
    app.undo.clear();
    app.redo.clear();
    app.dirty = false;
    let before = Inert::new(&app);
    let mut states = Vec::new();
    let mut source_results = None;
    let mut source_reference = None;
    for (index, (name, show)) in [("shown", true), ("hidden", false)].into_iter().enumerate() {
        app.ribbon_command(&ctx, RibbonAction::SetMatrixIndices { owner, show });
        assert_eq!(app.undo.len(), index + 1);
        assert!(app.redo.is_empty());
        assert_eq!(app.result_matrix_indices(owner), Some((1000, show)));
        for tick in 4 + index * 3..7 + index * 3 {
            frame(&mut app, &ctx, tick as u64);
        }
        let focus = source_focus(&app, &ctx, owner);
        before.check(&app);
        let (frozen, query_y) = snapshot(&mut app, owner, show);
        let own = root.join(format!("indices-{name}.mathcraft"));
        let native_path = root.join(format!("indices-{name}.mcdx"));
        app.persist_calculation_cache();
        app.document.save(&own).unwrap();
        app.prepare_native_export().unwrap();
        mathcraft_document::export::export_mcdx(&app.document, &native_path).unwrap();
        let (package, reference, result) = native(&native_path);
        if let Some(previous) = &source_results {
            assert_eq!(
                &result, previous,
                "view-only flag changed full native result XML"
            );
        } else {
            source_results = Some(result.clone());
        }
        if let Some(previous) = &source_reference {
            assert_eq!(&reference, previous);
        } else {
            source_reference = Some(reference.clone());
        }
        assert!(
            app.document.native_archive.is_none(),
            "export does not rebase original authority"
        );
        let mut files = vec![own.clone(), native_path.clone()];
        if output.is_some() {
            let pdf = root.join(format!("indices-{name}.pdf"));
            app.export_vector_pdf_for_test(&pdf).unwrap();
            files.push(pdf);
        }
        // Held snapshot stays bound to this state across save/export and the
        // next source-independent flag change; no transient focus is copied.
        let p = &frozen.matrix_displays[&owner];
        identity_projection(p);
        assert_eq!(
            frozen.matrix_indices[&owner],
            formula::MatrixIndexLabels::for_projection(p, 1000, show).unwrap()
        );
        before.check(&app);
        cold(&own, show);
        cold(&native_path, show);
        let numeric = app
            .document
            .calculation_cache
            .as_ref()
            .unwrap()
            .rows
            .iter()
            .find(|row| row.region_id == owner)
            .unwrap()
            .numeric_source
            .as_ref()
            .unwrap();
        let files:Vec<_>=files.iter().map(|file|serde_json::json!({"name":file.file_name().unwrap().to_str().unwrap(),"bytes":std::fs::metadata(file).unwrap().len(),"sha256":sha(&std::fs::read(file).unwrap())})).collect();
        states.push(serde_json::json!({"name":name,"show_indices":show,"live_origin":1000,"full_shape":[20,20],"window":{"rows":12,"cols":12,"row_offset":0,"col_offset":0},"visible_ones":12,"visible_zeros":132,"expected_visible_labels":if show {Some((1000..1012).collect::<Vec<i32>>())}else{None},"label_occurrences_each":if show {2}else{0},"hidden_endpoint":if show {Some(1019)}else{None},"query_model_top_dips":app.document.regions.iter().find(|r|r.id==owner).unwrap().top,"query_pdf_top_pt":query_y,"pdf_crop_y_min_pt":300.,"source_signature":app.document.calculation_signature(),"result_reference":reference,"result_status":"Synchronized","result_xml_bytes":result.len(),"result_xml_sha256":sha(&result),"numeric_source_sha256":sha(&serde_json::to_vec(numeric).unwrap()),"native_worksheet_sha256":sha(package.original_part(package.worksheet_part_name()).unwrap()),"view_history":index+1,"executions_after_setup":0,"worker_generation":app.background.submitted_generation(),"pending":0,"cold_origin":null,"cold_own_execution":0,"cold_native_execution":0,"cold_snapshot_indices":false,"print_snapshot_persistent_flag_only":true,"source_focus":focus,"files":files}));
    }
    assert_eq!(app.undo.len(), 2);
    assert!(app.redo.is_empty());
    before.check(&app);
    if output.is_some() {
        std::fs::write(root.join("observation.json"),serde_json::to_vec_pretty(&serde_json::json!({"version":1,"scope":"actual Rust identity setup, then view-only print/save/cold; original print hierarchy unknown","setup_executed_region_ids":setup_executed,"setup_calculation_requests":1,"query_owner":owner,"original_runtime":false,"states":states})).unwrap()).unwrap();
    }
}
#[test]
fn live_identity_indices_snapshot_save_and_cold_sources_remain_inert() {
    exercise(None);
}
#[test]
#[ignore = "Explicit live-origin Show/Hide readonly vector PDF packet"]
fn matrix_live_indices_print_preservation_probe() {
    let root = PathBuf::from(
        std::env::var_os("MATHCRAFT_MATRIX_LIVE_INDICES_OUTPUT")
            .expect("MATHCRAFT_MATRIX_LIVE_INDICES_OUTPUT"),
    );
    assert!(!root.exists(), "use a fresh output directory");
    std::fs::create_dir_all(&root).unwrap();
    exercise(Some(&root));
    println!(
        "MATRIX_LIVE_INDICES_PACKET live_origin=1000 visible=12x12 native=2 own=2 pdf=2 observations=1 postsetup_execution=0 cold_origin=unknown"
    );
}
