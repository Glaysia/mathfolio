//! Proposed print_ui child tests. Synthetic inert native cache, no engine setup.
//! Source/result computational consistency is deliberately not an oracle here.
use super::*;
use crate::result_display::{MatrixProjection, MatrixWindow, PreparedNativeMatrix};
use sha2::{Digest, Sha256};
use std::io::{Cursor, Write};
use std::sync::atomic::{AtomicU64, Ordering};

const ML: &str = "http://schemas.mathsoft.com/math50";
const WS: &str = "http://schemas.mathsoft.com/worksheet50";
const UNITS: &str = "http://schemas.mathsoft.com/units10";
const CUSTOM: &str = "custom/matrix-preserved.bin";
const CUSTOM_BYTES: &[u8] = b"matrix-view-packet\0unrelated bytes\xff";
static SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy)]
struct Case {
    name: &'static str,
    dimension: usize,
    unit: bool,
    local: bool,
}
const LARGE: Case = Case {
    name: "matrix-large",
    dimension: 100,
    unit: false,
    local: true,
};
const UNIT: Case = Case {
    name: "matrix-unit",
    dimension: 30,
    unit: true,
    local: false,
};
impl Case {
    fn window(self) -> MatrixWindow {
        if self.local {
            MatrixWindow {
                rows: 13,
                cols: 14,
                row_offset: 7,
                col_offset: 3,
            }
        } else {
            MatrixWindow {
                rows: 12,
                cols: 12,
                row_offset: 0,
                col_offset: 0,
            }
        }
    }
    fn number(self, row: usize, col: usize) -> usize {
        // Asymmetric in row/column, including the last requested cell.
        row * self.dimension + col + 1
    }
    fn plain(self, row: usize, col: usize) -> String {
        format!(
            "{}{}",
            self.number(row, col),
            if self.unit { " m" } else { "" }
        )
    }
}
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        loop {
            let path = std::env::temp_dir().join(format!(
                "matrix-view-packet-{}-{}",
                std::process::id(),
                SEQUENCE.fetch_add(1, Ordering::Relaxed)
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
fn before_closing(source: &str, node: roxmltree::Node<'_, '_>, addition: &str) -> String {
    let at = node.range().start
        + source[node.range()]
            .rfind("</")
            .expect("fixture closing tag");
    format!("{}{addition}{}", &source[..at], &source[at..])
}

/// Start with the real document exporter so required OPC parts/relationships
/// exist. Only test fixture parts are rebuilt through public ZIP/package APIs.
fn fixture(path: &Path, case: Case) -> NativePackage {
    let mut document = CanvasDocument {
        title: case.name.into(),
        ..Default::default()
    };
    // Native top is the equation baseline anchor. Leave room above the
    // baseline for the complete tall retained result; do not change its window.
    document.add_math(12., 350.);
    document.regions[0].format = MathFormat {
        explicit: true,
        result_format: ResultFormat::Decimal,
        precision: 0,
        ..Default::default()
    };
    let rhs = if case.unit {
        "<ml:id labels=\"UNIT\">m</ml:id>"
    } else {
        "<ml:placeholder/>"
    };
    document.regions[0].content = CanvasContent::Math(worksheet_math::tree_from_mathml(&format!(
        "<ml:eval xmlns:ml=\"{ML}\"><ml:id labels=\"VARIABLE\">M</ml:id><ml:unitOverride>{rhs}</ml:unitOverride></ml:eval>"
    )).unwrap());
    mathfolio_document::export::export_mcdx(&document, path).unwrap();
    let package = NativePackage::open(path).unwrap();
    let mut source = package.worksheet_xml().unwrap();
    let xml = roxmltree::Document::parse(&source).unwrap();
    let math = xml
        .descendants()
        .find(|n| n.has_tag_name((WS, "math")))
        .unwrap();
    let reference = math
        .attribute("resultRef")
        .expect("exported reference")
        .to_owned();
    if case.local {
        let format = math
            .children()
            .find(|n| n.has_tag_name((WS, "resultFormat")))
            .unwrap();
        source = before_closing(
            &source,
            format,
            "<matrix size=\"13,14\" offset=\"7,3\" show-indices=\"false\" expand-nested-arrays=\"false\"/>",
        );
    }
    let mut cells = String::new();
    for col in 0..case.dimension {
        for row in 0..case.dimension {
            let real = format!("<ml:real>{}</ml:real>", case.number(row, col));
            if case.unit {
                cells.push_str(&format!("<ml:unitedValue>{real}<u:unitMonomial><u:unitReference unit=\"meter\" power-numerator=\"1\" power-denominator=\"1\"/></u:unitMonomial></ml:unitedValue>"));
            } else {
                cells.push_str(&real);
            }
        }
    }
    let result = format!(
        "<resultsList xmlns=\"http://schemas.mathsoft.com/result10\" xmlns:ml=\"{ML}\" xmlns:u=\"{UNITS}\"><resultData result-id=\"{reference}\" calculation-status=\"Synchronized\" tag=\"retained-matrix-packet\"><ml:result><ml:matrix rows=\"{d}\" cols=\"{d}\">{cells}</ml:matrix></ml:result></resultData></resultsList>",
        d = case.dimension
    );
    assert!(result.len() < 1024 * 1024, "full inert carrier bound");
    let presentation = package
        .original_xml_part("mathcad/settings/presentation.xml")
        .unwrap()
        .unwrap();
    let tree = roxmltree::Document::parse(&presentation).unwrap();
    let replacement = format!(
        "<mathRendering><ws:results xmlns:ws=\"{WS}\"><ws:decimal precision=\"0\"/><ws:matrix size=\"12,12\" offset=\"0,0\" show-indices=\"false\" expand-nested-arrays=\"false\"/></ws:results></mathRendering>"
    );
    let mut renderings = tree.root_element().children().filter(|n| {
        n.has_tag_name((
            "http://schemas.ptc.com/mathcad/settings/presentation10",
            "mathRendering",
        ))
    });
    let presentation = if let Some(rendering) = renderings.next() {
        assert!(
            renderings.next().is_none(),
            "fresh exporter seed was ambiguous"
        );
        // Fixture-only replacement of the exact fresh-exporter default carrier.
        // Never normalize an arbitrary imported presentation or drop unknown data.
        let expected_seed = format!(
            "<mathRendering><ws:results xmlns:ws=\"{WS}\"><ws:matrix size=\"12,12\" offset=\"0,0\" show-indices=\"false\" expand-nested-arrays=\"false\"/></ws:results></mathRendering>"
        );
        assert_eq!(&presentation[rendering.range()], expected_seed);
        let mut updated = presentation.clone();
        updated.replace_range(rendering.range(), &replacement);
        updated
    } else {
        before_closing(&presentation, tree.root_element(), &replacement)
    };
    let types = package
        .original_xml_part("[Content_Types].xml")
        .unwrap()
        .unwrap();
    let tree = roxmltree::Document::parse(&types).unwrap();
    let types = before_closing(
        &types,
        tree.root_element(),
        &format!("<Override PartName=\"/{CUSTOM}\" ContentType=\"application/octet-stream\"/>"),
    );
    let mut replacements = HashMap::from([
        (
            package.worksheet_part_name().to_owned(),
            source.into_bytes(),
        ),
        ("mathcad/result.xml".into(), result.into_bytes()),
        (
            "mathcad/settings/presentation.xml".into(),
            presentation.into_bytes(),
        ),
        ("[Content_Types].xml".into(), types.into_bytes()),
        (CUSTOM.into(), CUSTOM_BYTES.to_vec()),
    ]);
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for part in package.parts() {
        let bytes = replacements
            .remove(&part.name)
            .unwrap_or_else(|| part.bytes().to_vec());
        zip.start_file(
            &part.name,
            zip::write::SimpleFileOptions::default().compression_method(part.compression),
        )
        .unwrap();
        zip.write_all(&bytes).unwrap();
    }
    for (name, bytes) in replacements {
        zip.start_file(
            name,
            zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated),
        )
        .unwrap();
        zip.write_all(&bytes).unwrap();
    }
    let package = NativePackage::from_bytes(&zip.finish().unwrap().into_inner()).unwrap();
    // write_new is intentionally create-new; the seed package occupied path.
    std::fs::remove_file(path).unwrap();
    package.write_new(path).unwrap();
    package
}

fn owner(app: &WorksheetApp) -> u64 {
    assert_eq!(app.document.regions.len(), 1);
    assert!(matches!(
        app.document.regions[0].content,
        CanvasContent::Math(_)
    ));
    app.document.regions[0].id
}
fn raw_source(app: &WorksheetApp) -> &str {
    let row = &app.saved_results[&owner(app)];
    assert_eq!(row.status, SavedCalculationStatus::Synchronized);
    assert!(row.error.is_none());
    let Some(SavedNumericSource::NativeMathml(source)) = &row.numeric_source else {
        panic!("lost complete native numeric authority: {row:?}");
    };
    source
}
fn inert(app: &WorksheetApp) {
    assert!(
        app.results.is_empty(),
        "must not manufacture evaluated Values"
    );
    assert!(
        app.pending_regions.is_empty(),
        "{} {:?}",
        app.status,
        app.saved_results
    );
    assert!(app.last_executed_regions.is_empty());
    assert!(!app.calculation_pending && !app.background.running);
    assert!(app.undo.is_empty() && app.redo.is_empty());
}
fn check_projection(view: &MatrixProjection, case: Case) {
    let w = case.window();
    assert_eq!(
        (view.full_rows, view.full_cols),
        (case.dimension, case.dimension)
    );
    assert_eq!(
        (view.rows, view.cols, view.row_offset, view.col_offset),
        (w.rows, w.cols, w.row_offset, w.col_offset)
    );
    assert_eq!(view.cells.len(), w.rows * w.cols);
    for row in 0..w.rows {
        for col in 0..w.cols {
            assert_eq!(
                view.cells[row * w.cols + col].plain(),
                case.plain(row + w.row_offset, col + w.col_offset),
                "visible row={row} col={col}"
            );
        }
    }
    assert_eq!(
        (view.hidden_above, view.hidden_left),
        (w.row_offset > 0, w.col_offset > 0)
    );
    assert!(view.hidden_below && view.hidden_right);
}
fn check_open(app: &mut WorksheetApp, case: Case, package: &NativePackage) -> MatrixProjection {
    inert(app);
    let id = owner(app);
    let caches = package.caches().unwrap();
    assert_eq!(caches.len(), 1);
    assert_eq!(
        caches[0].calculation_status.as_deref(),
        Some("Synchronized")
    );
    assert_eq!(caches[0].tag.as_deref(), Some("retained-matrix-packet"));
    assert_eq!(raw_source(app), caches[0].result_mathml.as_deref().unwrap());
    let prepared = PreparedNativeMatrix::parse(raw_source(app)).unwrap();
    assert_eq!(prepared.shape(), (case.dimension, case.dimension));
    assert!(raw_source(app).len() < 1024 * 1024);
    assert_eq!(
        app.matrix_view_sources
            .window(id, app.requested_matrix_view(id)),
        Some(case.window())
    );

    // Actual App body, not a mirrored formula wrapper. Dispose FullOutput before
    // asserting any work/readonly contract so egui's output invariant is honored.
    let before_document = serde_json::to_vec(&app.document).unwrap();
    let before_saved = app.saved_results.clone();
    let before_signature = app.document.calculation_signature();
    let before_generation = app.background.submitted_generation();
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    for tick in 0..4 {
        let mut raw = egui::RawInput {
            time: Some(tick as f64 / 60.),
            screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1800., 1300.))),
            ..Default::default()
        };
        eframe::App::raw_input_hook(app, &ctx, &mut raw);
        ctx.run_ui(raw, |ui| app.app_ui_frame(ui))
            .drop_without_applying_deltas();
        inert(app);
    }
    assert_eq!(serde_json::to_vec(&app.document).unwrap(), before_document);
    assert_eq!(app.saved_results, before_saved);
    assert_eq!(app.document.calculation_signature(), before_signature);
    assert_eq!(app.background.submitted_generation(), before_generation);
    let cache = &app.display_cache[&id].matrix;
    assert_eq!(cache.shape(), Some((case.dimension, case.dimension)));
    assert_eq!(
        cache.work_counts(),
        (1, 1),
        "idle must retain full admission/projection"
    );
    let view = cache
        .value()
        .expect("actual UI did not prepare a matrix view")
        .clone();
    check_projection(&view, case);
    let snapshot = app.print_snapshot();
    assert!(snapshot.matrix_errors.is_empty());
    assert_eq!(snapshot.matrix_displays.get(&id), Some(&view));
    assert!(snapshot.document.native_archive.is_none());
    assert!(snapshot.document.calculation_cache.is_none());
    assert_eq!(app.display_cache[&id].matrix.work_counts(), (1, 1));

    // Inert formatability is independent of the old <=12 serialized preview.
    // It is a derived projection only: no document format command is submitted.
    let changed_style = MathFormat {
        result_format: ResultFormat::Decimal,
        precision: 2,
        show_trailing_zeros: true,
        ..app.document.regions[0].format
    };
    let reformatted = prepared.project(case.window(), &changed_style).unwrap();
    assert_eq!(
        (reformatted.full_rows, reformatted.full_cols),
        (case.dimension, case.dimension)
    );
    assert_eq!(
        reformatted.cells[0].plain(),
        format!(
            "{}.00{}",
            case.number(case.window().row_offset, case.window().col_offset),
            if case.unit { " m" } else { "" }
        )
    );
    assert_eq!(
        snapshot.matrix_displays.get(&id),
        Some(&view),
        "print snapshot is immutable"
    );
    inert(app);
    view
}
fn check_copy(copy: &NativePackage, original: &NativePackage) {
    for part in [
        "mathcad/result.xml",
        "mathcad/settings/presentation.xml",
        CUSTOM,
    ] {
        assert_eq!(
            copy.original_part(part),
            original.original_part(part),
            "changed {part}"
        );
    }
    let old = original.regions().unwrap();
    let new = copy.regions().unwrap();
    assert_eq!(old.len(), 1);
    assert_eq!(new.len(), 1);
    assert_eq!(new[0].region_id, old[0].region_id);
    assert_eq!(
        new[0].math.as_ref().unwrap().result_ref,
        old[0].math.as_ref().unwrap().result_ref
    );
    let old_xml = original.worksheet_xml().unwrap();
    let new_xml = copy.worksheet_xml().unwrap();
    let old_tree = roxmltree::Document::parse(&old_xml).unwrap();
    let new_tree = roxmltree::Document::parse(&new_xml).unwrap();
    let old_math = old_tree
        .descendants()
        .find(|n| n.has_tag_name((WS, "math")))
        .unwrap();
    let new_math = new_tree
        .descendants()
        .find(|n| n.has_tag_name((WS, "math")))
        .unwrap();
    assert_eq!(
        mathfolio_document::matrix_result_format::read_local(new_math).unwrap(),
        mathfolio_document::matrix_result_format::read_local(old_math).unwrap()
    );
    assert_eq!(
        copy.caches().unwrap()[0].calculation_status.as_deref(),
        Some("Synchronized")
    );
}
fn exercise(case: Case, output: Option<&Path>) -> serde_json::Value {
    let scratch = Scratch::new();
    let root = output.unwrap_or(&scratch.0);
    let input = root.join(format!("{}-input.mcdx", case.name));
    let own = root.join(format!("{}.mathfolio", case.name));
    let copy = root.join(format!("{}-copy.mcdx", case.name));
    let original = fixture(&input, case);
    let mut native = WorksheetApp::default();
    native.open(&input);
    let native_view = check_open(&mut native, case, &original);
    let archive = native.document.native_archive.clone().unwrap();
    let native_signature = native.document.calculation_signature();
    native.persist_calculation_cache();
    assert_eq!(
        native
            .document
            .saved_calculation_cache()
            .unwrap()
            .source_signature,
        native_signature
    );
    native.document.save(&own).unwrap();
    inert(&native);

    let mut cold_own = WorksheetApp::default();
    cold_own.open(&own);
    assert_eq!(cold_own.document.calculation_signature(), native_signature);
    assert_eq!(
        cold_own.document.native_archive.as_deref(),
        Some(archive.as_slice())
    );
    assert_eq!(check_open(&mut cold_own, case, &original), native_view);
    cold_own.prepare_native_export().unwrap();
    mathfolio_document::export::export_mcdx(&cold_own.document, &copy).unwrap();
    assert_eq!(
        cold_own.document.native_archive.as_deref(),
        Some(archive.as_slice())
    );
    inert(&cold_own);
    let copied = NativePackage::open(&copy).unwrap();
    check_copy(&copied, &original);
    let mut cold_native = WorksheetApp::default();
    cold_native.open(&copy);
    assert_eq!(check_open(&mut cold_native, case, &copied), native_view);
    let mut names = vec![input, own, copy];
    if output.is_some() {
        let pdf = root.join(format!("{}.pdf", case.name));
        let generation = cold_native.background.submitted_generation();
        let before = raw_source(&cold_native).to_owned();
        cold_native.export_vector_pdf_for_test(&pdf).unwrap();
        assert_eq!(raw_source(&cold_native), before);
        assert_eq!(cold_native.background.submitted_generation(), generation);
        inert(&cold_native);
        names.push(pdf);
    }
    let w = case.window();
    let files = names.iter().map(|path| serde_json::json!({"name": path.file_name().unwrap().to_str().unwrap(), "bytes": std::fs::metadata(path).unwrap().len(), "sha256":sha(&std::fs::read(path).unwrap())})).collect::<Vec<_>>();
    serde_json::json!({
        "name":case.name,"fixture":"synthetic_inert_native_cache","engine_consistency_claimed":false,"setup_calculations":0,
        "full_rows":case.dimension,"full_cols":case.dimension,"full_cells":case.dimension*case.dimension,"per_cell_units":case.unit,
        "presentation_size":[12,12],"local_size":if case.local {Some([13,14])}else{None},"local_offset":if case.local {Some([7,3])}else{None},
        "window":{"rows":w.rows,"cols":w.cols,"row_offset":w.row_offset,"col_offset":w.col_offset},
        "first_cell":native_view.cells[0].plain(),"last_cell":native_view.cells.last().unwrap().plain(),
        "result_xml_bytes":original.original_part("mathcad/result.xml").unwrap().len(),"result_xml_sha256":sha(original.original_part("mathcad/result.xml").unwrap()),
        "original_worksheet_sha256":sha(original.original_part(original.worksheet_part_name()).unwrap()),
        "copy_worksheet_sha256":sha(copied.original_part(copied.worksheet_part_name()).unwrap()),
        "native_calculation_signature":native_signature,"own_calculation_signature":cold_own.document.calculation_signature(),"copy_calculation_signature":cold_native.document.calculation_signature(),
        "result_reference":original.regions().unwrap()[0].math.as_ref().unwrap().result_ref,
        "result_status":"Synchronized","whole_result_xml_equal":true,"native_custom_part_equal":true,"retained_archive_equal":true,
        "native_numeric_source_sha256":sha(raw_source(&cold_native).as_bytes()),"cold_open_executions":0,"idle_executions":0,"save_export_executions":0,"pdf_executions":0,
        "pending":0,"new_history":0,"generation_changes_after_each_open":0,"actual_app_frames_per_open":4,"open_stages":["native","own","native_copy"],
        "print_snapshot_equals_ui_projection":true,"origin_provenance":"unknown_not_fabricated","files":files
    })
}
#[test]
fn hundred_square_native_local_offset_full_payload_survives_own_and_native_cold() {
    exercise(LARGE, None);
}
#[test]
fn thirty_square_per_cell_units_explicit_output_unit_cold_remains_matrix_and_formatable() {
    exercise(UNIT, None);
}
#[test]
#[ignore = "Explicit generated native/own/vector-PDF matrix view preservation packet"]
fn matrix_result_view_native_own_pdf_preservation_probe() {
    let root = PathBuf::from(
        std::env::var_os("MATHFOLIO_MATRIX_VIEW_OUTPUT").expect("MATHFOLIO_MATRIX_VIEW_OUTPUT"),
    );
    assert!(!root.exists(), "packet must use a fresh output directory");
    std::fs::create_dir_all(&root).unwrap();
    let cases = [exercise(LARGE, Some(&root)), exercise(UNIT, Some(&root))];
    std::fs::write(root.join("observation.json"), serde_json::to_vec_pretty(&serde_json::json!({"version":1,"scope":"generated Mathfolio cache retention; not original execution, GPU, or original PDF parity","cases":cases})).unwrap()).unwrap();
    println!(
        "MATRIX_VIEW_PACKET cases=2 native=4 own=2 pdf=2 observation=1 cold_execution=0 pending=0 full_result_bytes_equal=1"
    );
}
