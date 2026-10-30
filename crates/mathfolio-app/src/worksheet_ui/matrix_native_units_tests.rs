//! Generated app-fixture cache preservation, not original engine execution.
use super::*;
use std::io::{Cursor, Write};
use std::sync::atomic::{AtomicU64, Ordering};
const ML: &str = "http://schemas.mathsoft.com/math50";
const WS: &str = "http://schemas.mathsoft.com/worksheet50";
const U: &str = "http://schemas.mathsoft.com/units10";
static SEQ: AtomicU64 = AtomicU64::new(0);
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        loop {
            let p = std::env::temp_dir().join(format!(
                "matrix-units-{}-{}",
                std::process::id(),
                SEQ.fetch_add(1, Ordering::Relaxed)
            ));
            match std::fs::create_dir(&p) {
                Ok(()) => return Self(p),
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
fn unit(value: &str) -> String {
    format!(
        "<ml:unitedValue>{value}<u:unitMonomial><u:unitReference unit=\"meter\" power-numerator=\"1\" power-denominator=\"1\"/></u:unitMonomial></ml:unitedValue>"
    )
}
fn matrix(united: bool) -> String {
    let cells = [1, 3, 2, 4]
        .map(|n| {
            let s = format!("<ml:real>{n}</ml:real>");
            if united { unit(&s) } else { s }
        })
        .join("");
    format!("<ml:matrix rows=\"2\" cols=\"2\">{cells}</ml:matrix>")
}
fn fixture(path: &Path, area: bool, value: &str) -> Vec<u8> {
    let child = "<ws:region region-id=\"42\" left=\"10\" top=\"20\"><ws:math resultRef=\"900\"><ml:eval><ml:id labels=\"VARIABLE\">unavailable</ml:id><ml:unitOverride><ml:id labels=\"UNIT\">m</ml:id></ml:unitOverride></ml:eval></ws:math></ws:region>";
    let regions = if area {
        format!(
            "<ws:region region-id=\"41\" left=\"0\" top=\"20\"><ws:Area AreaHeight=\"150\" IsCollapsed=\"false\" IsBorderVisible=\"true\"><ws:regions>{child}</ws:regions></ws:Area></ws:region>"
        )
    } else {
        child.into()
    };
    let source = format!(
        "<ws:worksheet xmlns:ws=\"{WS}\" xmlns:ml=\"{ML}\"><ws:regions>{regions}</ws:regions></ws:worksheet>"
    );
    let result = format!(
        "<resultsList xmlns=\"http://schemas.mathsoft.com/result10\" xmlns:ml=\"{ML}\" xmlns:u=\"{U}\"><resultData result-id=\"900\" calculation-status=\"Synchronized\"><ml:result>{value}</ml:result></resultData></resultsList>"
    );
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in [
        ("mathcad/worksheet.xml", source.as_bytes()),
        ("mathcad/result.xml", result.as_bytes()),
        ("custom/untouched.bin", b"immutable".as_slice()),
    ] {
        zip.start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(bytes).unwrap();
    }
    std::fs::write(path, zip.finish().unwrap().into_inner()).unwrap();
    result.into_bytes()
}
fn row(app: &WorksheetApp, area: bool) -> &SavedCalculationRow {
    if area {
        app.area_saved
            .values()
            .next()
            .unwrap()
            .values()
            .next()
            .unwrap()
    } else {
        app.saved_results.values().next().unwrap()
    }
}
fn check(app: &WorksheetApp, area: bool, shape: (usize, usize), whole_matrix_source: bool) {
    assert!(app.last_executed_regions.is_empty());
    assert!(app.results.is_empty());
    let row = row(app, area);
    assert_eq!(row.status, SavedCalculationStatus::Synchronized);
    assert!(row.error.is_none());
    let display =
        crate::result_display::FormattedResult::from_saved(row.display.as_deref().unwrap())
            .unwrap();
    assert_eq!((display.rows, display.cols), shape);
    if shape == (2, 2) {
        for (cell, number) in display.cells.iter().zip(["1", "2", "3", "4"]) {
            assert!(cell.plain().starts_with(number), "{:?}", cell);
        }
    }
    let Some(SavedNumericSource::NativeMathml(xml)) = &row.numeric_source else {
        panic!("full inert source absent")
    };
    let doc = roxmltree::Document::parse(xml).unwrap();
    let matrix = doc.descendants().find(|n| n.has_tag_name((ML, "matrix")));
    if whole_matrix_source {
        let m = matrix.expect("matrix collapsed to first scalar");
        assert_eq!(m.attribute("rows"), Some("2"));
        assert_eq!(m.attribute("cols"), Some("2"));
        assert_eq!(m.children().filter(roxmltree::Node::is_element).count(), 4);
        assert!(xml.contains(">4<"));
    } else {
        assert!(matrix.is_none());
    }
}
fn exercise(area: bool, value: &str, shape: (usize, usize), whole_matrix_source: bool) {
    let scratch = Scratch::new();
    let original = scratch.0.join("original.mcdx");
    let bytes = fixture(&original, area, value);
    let mut app = WorksheetApp::default();
    app.open(&original);
    check(&app, area, shape, whole_matrix_source);
    assert_eq!(
        NativePackage::open(&original)
            .unwrap()
            .original_part("mathcad/result.xml")
            .unwrap(),
        bytes
    );
    // Reformat through inert saved source. No worksheet source execution.
    if !area {
        let id = app.document.regions[0].id;
        let mut format = app.document.regions[0].format;
        format.precision = 5;
        let display = app.display_result(id, format).unwrap();
        assert_eq!((display.rows, display.cols), shape);
    } else {
        let format = MathFormat {
            precision: 5,
            ..Default::default()
        };
        let display = numeric_cache::display(row(&app, true), &format).unwrap();
        assert_eq!((display.rows, display.cols), shape);
    }
    let own = scratch.0.join("retained.mathfolio");
    app.persist_calculation_cache();
    app.document.save(&own).unwrap();
    let mut cold = WorksheetApp::default();
    cold.open(&own);
    check(&cold, area, shape, whole_matrix_source);
    let copy = scratch.0.join("copy.mcdx");
    cold.prepare_native_export().unwrap();
    assert!(
        mathfolio_document::export::source_context_matches(
            &cold.document,
            &NativePackage::open(&original).unwrap()
        )
        .unwrap(),
        "source context area={area}"
    );
    mathfolio_document::export::export_mcdx(&cold.document, &copy).unwrap();
    let package = NativePackage::open(&copy).unwrap();
    assert_eq!(
        package.original_part("mathcad/result.xml").unwrap(),
        bytes,
        "area={area}, value={value}"
    );
    assert_eq!(
        package.original_part("custom/untouched.bin"),
        Some(b"immutable".as_slice())
    );
    let mut native = WorksheetApp::default();
    native.open(&copy);
    check(&native, area, shape, whole_matrix_source);
}
#[test]
fn top_explicit_output_unit_keeps_per_cell_united_matrix_on_cold_reformat_save() {
    exercise(false, &matrix(true), (2, 2), true);
}
#[test]
fn area_explicit_output_unit_keeps_per_cell_united_matrix_on_cold_reformat_save() {
    exercise(true, &matrix(true), (2, 2), true);
}
#[test]
fn scalar_and_top_united_matrix_explicit_output_unit_stay_compatible() {
    for area in [false, true] {
        exercise(area, &unit("<ml:real>12.3456</ml:real>"), (1, 1), false);
        exercise(area, &unit(&matrix(false)), (2, 2), true);
    }
}
