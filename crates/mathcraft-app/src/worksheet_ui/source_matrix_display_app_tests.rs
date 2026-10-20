//! Actual App full calculation and typed source-display preservation. Source
//! viewport rendering/navigation, original runtime and schema parity not claimed.
use super::*;
use crate::result_display::{FormattedResult, MatrixWindow, PreparedNativeMatrix};
use std::io::{Cursor, Write};
use std::sync::atomic::{AtomicU64, Ordering};

const ML: &str = "http://schemas.mathsoft.com/math50";
const WS: &str = "http://schemas.mathsoft.com/worksheet50";
const DISPLAY: &str = "<ml:display offset=\"0,1\" size=\"1,2\"/>";
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        loop {
            let path = std::env::temp_dir().join(format!(
                "source-matrix-display-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match std::fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => panic!("{e}"),
            }
        }
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn tree(body: &str) -> PortableTree {
    worksheet_math::tree_from_mathml(&format!("<context xmlns:ml=\"{ML}\">{body}</context>"))
        .unwrap()
}
fn seed(path: &Path) {
    let mut document = CanvasDocument::default();
    for (top, body) in [
        (
            30.,
            "<ml:define><ml:id labels=\"VARIABLE\">M</ml:id><ml:matrix rows=\"2\" cols=\"3\"><ml:real>1</ml:real><ml:real>4</ml:real><ml:real>2</ml:real><ml:real>5</ml:real><ml:real>3</ml:real><ml:real>6</ml:real></ml:matrix></ml:define>",
        ),
        (
            180.,
            "<ml:eval><ml:id labels=\"VARIABLE\">M</ml:id><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>",
        ),
    ] {
        let owner = document.add_math(20., top);
        let region = document.regions.iter_mut().find(|r| r.id == owner).unwrap();
        region.content = CanvasContent::Math(tree(body));
        region.format = MathFormat {
            explicit: true,
            result_format: ResultFormat::Decimal,
            precision: 0,
            ..Default::default()
        };
    }
    mathcraft_document::export::export_mcdx(&document, path).unwrap();
}
fn fixture(root: &Path, hidden_bad: Option<&str>) -> PathBuf {
    let seed_path = root.join("plain-seed.mcdx");
    seed(&seed_path);
    let original = NativePackage::open(&seed_path).unwrap();
    let source = original.worksheet_xml().unwrap();
    let xml = roxmltree::Document::parse(&source).unwrap();
    let matrix = xml
        .descendants()
        .find(|n| n.has_tag_name((ML, "matrix")))
        .unwrap();
    assert_eq!(
        (matrix.attribute("rows"), matrix.attribute("cols")),
        (Some("2"), Some("3"))
    );
    let cells: Vec<_> = matrix
        .children()
        .filter(roxmltree::Node::is_element)
        .collect();
    assert_eq!(cells.len(), 6);
    assert_eq!(
        cells.iter().map(|n| n.text().unwrap()).collect::<Vec<_>>(),
        ["1", "4", "2", "5", "3", "6"]
    );
    // The hidden row1,col0 (column-major cell1) is outside row0,cols1..2.
    // Descending raw patches never reconstruct or relabel the source cells.
    let mut changed = source.clone();
    if let Some(bad) = hidden_bad {
        changed.replace_range(cells[1].range(), bad);
    }
    changed.insert_str(cells[0].range().start, DISPLAY);
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for part in original.parts() {
        zip.start_file(
            &part.name,
            zip::write::SimpleFileOptions::default().compression_method(part.compression),
        )
        .unwrap();
        zip.write_all(if part.name == original.worksheet_part_name() {
            changed.as_bytes()
        } else {
            part.bytes()
        })
        .unwrap();
    }
    let path = root.join("source-display.mcdx");
    NativePackage::from_bytes(&zip.finish().unwrap().into_inner())
        .unwrap()
        .write_new(&path)
        .unwrap();
    path
}
fn source_matrix(package: &NativePackage) -> String {
    let source = package.worksheet_xml().unwrap();
    let xml = roxmltree::Document::parse(&source).unwrap();
    let matrices: Vec<_> = xml
        .descendants()
        .filter(|n| n.has_tag_name((ML, "matrix")))
        .collect();
    assert_eq!(matrices.len(), 1);
    let matrix = matrices[0];
    let first = matrix.first_element_child().unwrap();
    assert!(first.has_tag_name((ML, "display")));
    assert_eq!(first.attribute("offset"), Some("0,1"));
    assert_eq!(first.attribute("size"), Some("1,2"));
    assert_eq!(
        matrix
            .children()
            .filter(roxmltree::Node::is_element)
            .count(),
        7
    );
    source[matrix.range()].to_owned()
}
fn definition(app: &WorksheetApp) -> u64 {
    let definitions: Vec<_> = app
        .document
        .regions
        .iter()
        .filter(|r| {
            match &r.content {
                CanvasContent::Math(t) => worksheet_math::tree_to_mathml(t)
                    .unwrap()
                    .contains("<ml:display "),
                // A malformed hidden expression still fails the bridge and
                // remains source-preserving Opaque; never synthesize a tree.
                CanvasContent::Opaque { kind, xml } => {
                    kind == "math" && xml.contains("<ml:display ")
                }
                _ => false,
            }
        })
        .collect();
    assert_eq!(
        definitions.len(),
        1,
        "exactly one complete display-bearing source definition"
    );
    definitions[0].id
}
fn query(app: &WorksheetApp) -> u64 {
    let queries: Vec<_> = app
        .document
        .regions
        .iter()
        .filter(|r| {
            matches!(&r.content,
        CanvasContent::Math(t) if matches!(t.kind,NodeKind::Binary(BinaryOp::Evaluate)))
        })
        .collect();
    assert_eq!(queries.len(), 1);
    queries[0].id
}
fn assert_value(value: &Value) {
    let Value::Matrix {
        rows,
        cols,
        elements,
    } = value
    else {
        panic!("not a full matrix: {value:?}")
    };
    assert_eq!((*rows, *cols, elements.len()), (2, 3, 6));
    for (q, number) in elements.iter().zip(1..=6) {
        assert_eq!(q.value.re.to_bits(), f64::from(number).to_bits());
        assert_eq!(q.value.im, 0.);
        assert!(q.is_scalar());
    }
}
fn assert_native_value(source: &str) {
    let prepared = PreparedNativeMatrix::parse(source).unwrap();
    assert_eq!(prepared.shape(), (2, 3));
    let view = prepared
        .project(
            MatrixWindow {
                rows: 2,
                cols: 3,
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
    assert_eq!(
        view.cells
            .iter()
            .map(|cell| cell.plain())
            .collect::<Vec<_>>(),
        ["1", "2", "3", "4", "5", "6"]
    );
}
fn saved_query(app: &WorksheetApp, owner: u64) {
    let row = &app.saved_results[&owner];
    assert_eq!(row.status, SavedCalculationStatus::Synchronized, "{row:?}");
    assert!(row.error.is_none());
    match row.numeric_source.as_ref().unwrap() {
        SavedNumericSource::EvaluatedValue(json) => {
            assert_value(&serde_json::from_str(json).unwrap())
        }
        SavedNumericSource::NativeMathml(xml) => assert_native_value(xml),
        other => panic!("lost full numeric source: {other:?}"),
    }
    let preview = FormattedResult::from_saved(row.display.as_deref().unwrap()).unwrap();
    assert_eq!((preview.rows, preview.cols, preview.cells.len()), (2, 3, 6));
    assert_eq!(
        preview.cells.iter().map(|c| c.plain()).collect::<Vec<_>>(),
        ["1", "2", "3", "4", "5", "6"]
    );
}
fn frame(app: &mut WorksheetApp, ctx: &egui::Context, tick: u64) {
    let mut raw = egui::RawInput {
        time: Some(tick as f64 / 60.),
        screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1200., 900.))),
        ..Default::default()
    };
    eframe::App::raw_input_hook(app, ctx, &mut raw);
    ctx.run_ui(raw, |ui| app.app_ui_frame(ui))
        .drop_without_applying_deltas();
}
fn manual_open(path: &Path) -> WorksheetApp {
    let mut app = WorksheetApp {
        automatic_calculation: false,
        ..Default::default()
    };
    app.open(path);
    assert_eq!(app.document.regions.len(), 2, "{}", app.status);
    let original = std::fs::read(path).unwrap();
    let regions = app.document.regions.clone();
    let signature = app.document.calculation_signature();
    let generation = app.background.submitted_generation();
    assert!(app.results.is_empty() && app.last_executed_regions.is_empty());
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    for tick in 0..2 {
        frame(&mut app, &ctx, tick);
    }
    assert!(app.results.is_empty() && app.last_executed_regions.is_empty());
    assert_eq!(app.background.submitted_generation(), generation);
    assert!(!app.background.running);
    assert_eq!(app.document.regions, regions);
    assert_eq!(app.document.calculation_signature(), signature);
    assert!(app.undo.is_empty() && app.redo.is_empty());
    assert_eq!(std::fs::read(path).unwrap(), original);
    app
}

#[test]
fn native_source_display_manual_open_then_explicit_app_calc_and_cold_roundtrip_keep_all_six_cells()
{
    let scratch = Scratch::new();
    let path = fixture(&scratch.0, None);
    let original = std::fs::read(&path).unwrap();
    let matrix = source_matrix(&NativePackage::open(&path).unwrap());
    let mut app = manual_open(&path);
    let def = definition(&app);
    let query_owner = query(&app);
    let content = app
        .document
        .regions
        .iter()
        .find(|r| r.id == def)
        .unwrap()
        .content
        .clone();
    let CanvasContent::Math(t) = &content else {
        panic!("strict source display must be typed")
    };
    let expected = tree(
        "<ml:define><ml:id labels=\"VARIABLE\">M</ml:id><ml:matrix rows=\"2\" cols=\"3\"><ml:display offset=\"0,1\" size=\"1,2\"/><ml:real>1</ml:real><ml:real>4</ml:real><ml:real>2</ml:real><ml:real>5</ml:real><ml:real>3</ml:real><ml:real>6</ml:real></ml:matrix></ml:define>",
    );
    assert_eq!(t, &expected);
    let NodeKind::Matrix {
        rows,
        cols,
        display: Some(display),
    } = t.children[1].kind
    else {
        panic!("lost source display")
    };
    assert_eq!((rows, cols), (2, 3));
    assert_eq!(display.offset(), Some([0, 1]));
    assert_eq!(display.size(), Some([1, 2]));
    assert_eq!(display.window([rows, cols]).unwrap(), ([0, 1], [1, 2]));
    assert_eq!(t.children[1].children.len(), 6);
    assert_eq!(
        app.document.native_archive.as_deref(),
        Some(original.as_slice())
    );
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    app.ribbon_command(&ctx, RibbonAction::Recalculate);
    assert!(app.results[&def].error.is_none(), "{:?}", app.results[&def]);
    let result = &app.results[&query_owner];
    assert!(result.error.is_none(), "{result:?}");
    assert_value(result.value.as_ref().unwrap());
    assert!(
        app.last_executed_regions.contains(&def.to_string())
            && app.last_executed_regions.contains(&query_owner.to_string())
    );
    assert_eq!(
        app.document
            .regions
            .iter()
            .find(|r| r.id == def)
            .unwrap()
            .content,
        content
    );
    assert_eq!(
        app.document.native_archive.as_deref(),
        Some(original.as_slice())
    );
    app.persist_calculation_cache();
    let own = scratch.0.join("calculated.mathcraft");
    app.document.save(&own).unwrap();
    let mut own_cold = manual_open(&own);
    let export_generation = own_cold.background.submitted_generation();
    saved_query(&own_cold, query(&own_cold));
    assert_eq!(
        own_cold
            .document
            .regions
            .iter()
            .find(|r| r.id == definition(&own_cold))
            .unwrap()
            .content,
        content
    );
    own_cold.prepare_native_export().unwrap();
    let copy = scratch.0.join("calculated-copy.mcdx");
    mathcraft_document::export::export_mcdx(&own_cold.document, &copy).unwrap();
    let native = NativePackage::open(&copy).unwrap();
    assert_eq!(source_matrix(&native), matrix);
    let source = native.worksheet_xml().unwrap();
    let dom = roxmltree::Document::parse(&source).unwrap();
    let refs: Vec<_> = dom
        .descendants()
        .filter(|n| {
            n.has_tag_name((WS, "math")) && n.children().any(|c| c.has_tag_name((ML, "eval")))
        })
        .map(|n| n.attribute("resultRef").unwrap())
        .collect();
    assert_eq!(refs.len(), 1);
    let caches = native.caches().unwrap();
    let rows: Vec<_> = caches
        .iter()
        .filter(|r| r.matches_result_ref(refs[0]))
        .collect();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].calculation_status.as_deref(), Some("Synchronized"));
    assert_native_value(rows[0].result_mathml.as_deref().unwrap());
    let cold = manual_open(&copy);
    saved_query(&cold, query(&cold));
    definition(&cold);
    assert_eq!(
        std::fs::read(&path).unwrap(),
        original,
        "original remains immutable"
    );
    assert!(own_cold.last_executed_regions.is_empty());
    assert_eq!(
        own_cold.background.submitted_generation(),
        export_generation
    );
}

#[test]
fn hidden_source_matrix_faults_are_not_skipped_to_make_a_plausible_visible_result() {
    for bad in [
        "<ml:id labels=\"VARIABLE\">hidden_outside_view</ml:id>",
        "<ml:apply><ml:plus/><ml:real>4</ml:real></ml:apply>",
        "<ml:apply><ml:plus/><ml:real>4</ml:real><ml:id labels=\"UNIT\">m</ml:id></ml:apply>",
    ] {
        let scratch = Scratch::new();
        let path = fixture(&scratch.0, Some(bad));
        let original = std::fs::read(&path).unwrap();
        let mut app = manual_open(&path);
        let def = definition(&app);
        let query_owner = query(&app);
        let regions = app.document.regions.clone();
        let ctx = egui::Context::default();
        crate::configure(&ctx);
        app.ribbon_command(&ctx, RibbonAction::Recalculate);
        assert!(
            app.results[&def].error.is_some(),
            "offscreen fault discarded: {:?}",
            app.results[&def]
        );
        assert!(
            app.results[&query_owner].error.is_some(),
            "undefined full M must not produce partial result"
        );
        assert!(app.results[&query_owner].value.is_none());
        assert!(
            app.results
                .values()
                .all(|row| !matches!(row.value, Some(Value::Matrix { .. }))),
            "no fabricated cropped matrix"
        );
        assert_eq!(app.document.regions, regions);
        assert_eq!(std::fs::read(&path).unwrap(), original);
    }
}

mod units_tests;

mod typed_edit_tests;

mod reshape_tests;
