//! Native source + inert multi-trace points through the real app import/export path.
use super::*;
use std::io::{Cursor, Read, Write};

fn scratch() -> PathBuf {
    let output = std::env::temp_dir().join(format!(
        "mathcraft-native-xy-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&output).unwrap();
    output
}
fn clean(output: &Path) {
    for item in std::fs::read_dir(output).unwrap() {
        std::fs::remove_file(item.unwrap().path()).unwrap();
    }
    std::fs::remove_dir(output).unwrap();
}
fn patch_results(path: &Path, patch: impl FnOnce(String) -> String) {
    let original = std::fs::read(path).unwrap();
    let mut input = zip::ZipArchive::new(Cursor::new(original)).unwrap();
    let mut output = zip::ZipWriter::new(Cursor::new(vec![]));
    let mut patch = Some(patch);
    for i in 0..input.len() {
        let mut member = input.by_index(i).unwrap();
        let mut bytes = vec![];
        member.read_to_end(&mut bytes).unwrap();
        if member.name().unwrap() == "mathcad/result.xml" {
            bytes = patch.take().unwrap()(String::from_utf8(bytes).unwrap()).into_bytes();
        }
        output
            .start_file(
                member.name().unwrap(),
                zip::write::SimpleFileOptions::default().compression_method(member.compression()),
            )
            .unwrap();
        output.write_all(&bytes).unwrap();
    }
    assert!(patch.is_none());
    std::fs::write(path, output.finish().unwrap().into_inner()).unwrap();
}

#[test]
fn unsupported_curve_is_visible_blocks_print_survives_own_save_and_recalculates() {
    let output = scratch();
    let native = output.join("unsupported.mcdx");
    mathcraft_document::export::export_mcdx(&fixture().document, &native).unwrap();
    patch_results(&native, |source| {
        source.replace("VectorsType=\"Columns\"", "VectorsType=\"Rows\"")
    });
    let mut opened = WorksheetApp::default();
    opened.open(&native);
    assert!(opened.status.contains("지원 필요"), "{}", opened.status);
    assert!(opened.plot_results.is_empty());
    assert!(opened.pending_regions.contains(&2) && opened.pending_regions.contains(&3));
    assert!(opened.last_executed_regions.is_empty());
    assert!(
        opened
            .print_snapshot()
            .plan(PageSelection::All, 300)
            .is_err()
    );
    opened.persist_calculation_cache();
    let own = output.join("unsupported.mathcraft");
    opened.document.save(&own).unwrap();
    let mut saved = WorksheetApp::default();
    saved.open(&own);
    assert!(saved.status.contains("지원 필요"), "{}", saved.status);
    assert!(
        saved
            .print_snapshot()
            .plan(PageSelection::All, 300)
            .is_err()
    );
    let copy = output.join("unsupported-copy.mcdx");
    mathcraft_document::export::export_mcdx(&saved.document, &copy).unwrap();
    let original = NativePackage::open(&native).unwrap();
    let copied = NativePackage::open(&copy).unwrap();
    assert_eq!(
        original.original_part("mathcad/result.xml"),
        copied.original_part("mathcad/result.xml")
    );
    saved.recalculate();
    assert!(saved.plot_results.values().all(Result::is_ok));
    assert!(saved.print_snapshot().plan(PageSelection::All, 300).is_ok());
    clean(&output);
}

#[test]
fn mixed_native_trace_status_survives_cold_open_and_no_edit_export_exactly() {
    let output = scratch();
    let native = output.join("mixed.mcdx");
    mathcraft_document::export::export_mcdx(&fixture().document, &native).unwrap();
    patch_results(&native, |source| {
        let dom = roxmltree::Document::parse(&source).unwrap();
        let row = dom
            .descendants()
            .find(|node| node.is_element() && node.attribute("result-id") == Some("3"))
            .unwrap();
        let status = row.attribute_node("calculation-status").unwrap().range();
        let mut changed = source.clone();
        changed.replace_range(status, "calculation-status=\"Pending\"");
        changed
    });
    let mut opened = WorksheetApp::default();
    opened.open(&native);
    assert_eq!(opened.pending_regions, HashSet::from([3]));
    assert_eq!(opened.plot_results.len(), 2);
    assert!(opened.last_executed_regions.is_empty());
    opened.persist_calculation_cache();
    let copy = output.join("mixed-copy.mcdx");
    mathcraft_document::export::export_mcdx(&opened.document, &copy).unwrap();
    let original = NativePackage::open(&native).unwrap();
    let copied = NativePackage::open(&copy).unwrap();
    assert_eq!(
        original.original_part("mathcad/result.xml"),
        copied.original_part("mathcad/result.xml")
    );
    clean(&output);
}

#[test]
fn source_only_pending_curve_opens_editable_without_invented_results_or_warning() {
    let output = scratch();
    let native = output.join("pending.mcdx");
    let mut document = CanvasDocument::default();
    let id = document.add_plot(20., 20.);
    mathcraft_document::export::export_mcdx(&document, &native).unwrap();
    let mut opened = WorksheetApp::default();
    opened.open(&native);
    assert!(matches!(
        opened.document.regions[0].content,
        CanvasContent::Plot(_)
    ));
    assert_eq!(opened.pending_regions, HashSet::from([id]));
    assert!(
        opened.plot_results[&id]
            .as_ref()
            .unwrap()
            .first_curve()
            .is_none()
            && opened.last_executed_regions.is_empty()
    );
    assert!(!opened.status.contains("지원 필요"), "{}", opened.status);
    opened.recalculate();
    assert!(opened.plot_results[&id].is_ok());
    clean(&output);
}
fn fixture() -> WorksheetApp {
    let mut app = WorksheetApp {
        automatic_calculation: false,
        ..Default::default()
    };
    let id = app.document.add_math(20., 20.);
    app.document.regions.iter_mut().find(|region|region.id==id).unwrap().content=CanvasContent::Math(worksheet_math::tree_from_mathml(r#"<m:define xmlns:m="http://schemas.mathsoft.com/math50"><m:id>gain0</m:id><m:real>2</m:real></m:define>"#).unwrap());
    app.document.add_plot(20., 100.);
    let id = app.document.add_plot(20., 440.);
    let CanvasContent::Plot(spec) = &mut app
        .document
        .regions
        .iter_mut()
        .find(|region| region.id == id)
        .unwrap()
        .content
    else {
        panic!()
    };
    spec.width = 520.;
    spec.height = 260.;
    spec.promote_legacy_trace();
    *spec.primary_y_expression_mut().unwrap() = "gain0*sin(x)".into();
    spec.add_trace(PlotTraceSpec {
        x_expression: "x".into(),
        y_expression: "gain0*cos(x)".into(),
        style: mathcraft_document::canvas::PlotTraceStyle {
            color: [210, 20, 30, 255],
            line: PlotLineStyle::Dashed,
            line_width: 2.5,
            ..Default::default()
        },
        ..Default::default()
    })
    .unwrap();
    app.recalculate();
    assert!(app.calculation_error.is_none());
    assert!(app.plot_results.values().all(Result::is_ok));
    app.persist_calculation_cache();
    app
}
fn run(output: &Path) {
    std::fs::create_dir_all(output).unwrap();
    let app = fixture();
    let expected = app.plot_results.clone();
    let native = output.join("native-xy.mcdx");
    mathcraft_document::export::export_mcdx(&app.document, &native).unwrap();
    let package = NativePackage::open(&native).unwrap();
    assert_eq!(
        package
            .caches()
            .unwrap()
            .iter()
            .filter(|row| row.result_kind.as_deref() == Some("Trace2dResult"))
            .count(),
        3
    );
    let mut opened = WorksheetApp::default();
    opened.open(&native);
    assert!(
        opened.last_executed_regions.is_empty()
            && opened.results.is_empty()
            && opened.pending_regions.is_empty(),
        "{} {:?}",
        opened.status,
        opened.pending_regions
    );
    assert_eq!(
        opened
            .document
            .regions
            .iter()
            .filter(|region| matches!(region.content, CanvasContent::Plot(_)))
            .count(),
        2
    );
    assert!(!opened.status.contains("지원 필요"), "{}", opened.status);
    for (id, prior) in &expected {
        let actual = opened.plot_results[id].as_ref().unwrap();
        let prior = prior.as_ref().unwrap();
        assert_eq!(actual.traces.len(), prior.traces.len());
        for (actual, prior) in actual.traces.iter().zip(&prior.traces) {
            let (actual, prior) = (actual.curve().unwrap(), prior.curve().unwrap());
            assert_eq!(actual.points, prior.points);
            assert_eq!(actual.style, prior.style);
            assert_eq!(actual.label, prior.label);
        }
        assert!(matches!(
            opened.saved_results[id].numeric_source,
            Some(SavedNumericSource::NativeMathml(_))
        ));
    }
    let mut snapshot = opened.print_snapshot();
    let plan = snapshot.plan(PageSelection::All, 300).unwrap();
    let pages =
        printing::render_vector_pages(&plan, |ui, page, total| snapshot.paint(ui, page, total))
            .unwrap();
    printing::write_pdf(&output.join("native-xy.pdf"), &pages).unwrap();
    assert!(opened.last_executed_regions.is_empty() && opened.results.is_empty());
    opened.persist_calculation_cache();
    opened
        .document
        .save(&output.join("native-xy.mathcraft"))
        .unwrap();
    let mut saved = WorksheetApp::default();
    saved.open(&output.join("native-xy.mathcraft"));
    assert!(saved.last_executed_regions.is_empty() && saved.pending_regions.is_empty());
    let copied = output.join("native-xy-copy.mcdx");
    mathcraft_document::export::export_mcdx(&saved.document, &copied).unwrap();
    let copied = NativePackage::open(&copied).unwrap();
    assert_eq!(
        copied.original_part("mathcad/result.xml"),
        package.original_part("mathcad/result.xml")
    );
    saved.automatic_calculation = false;
    saved.selected = Some(3);
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    saved.ribbon_command(&ctx, RibbonAction::EditPlot);
    let old = saved.document.clone();
    let oldpoints = saved.plot_results[&3].clone();
    saved
        .plot_draft
        .as_mut()
        .unwrap()
        .spec
        .set_axis_expression_text(mathcraft_document::canvas::PlotAxis::Y, 0, "gain0*x")
        .unwrap();
    assert_eq!(saved.document, old);
    assert_eq!(saved.plot_results[&3], oldpoints);
    assert!(saved.commit_plot_draft());
    assert!(saved.pending_regions.contains(&3));
    assert!(saved.last_executed_regions.is_empty());
    saved.recalculate();
    assert!(saved.plot_results.values().all(Result::is_ok));
    saved.persist_calculation_cache();
    assert!(
        saved
            .document
            .saved_calculation_cache()
            .unwrap()
            .rows
            .iter()
            .filter(|row| row.region_id == 3)
            .all(|row| row.numeric_source.is_none())
    );
    let edited = output.join("native-xy-edited.mcdx");
    mathcraft_document::export::export_mcdx(&saved.document, &edited).unwrap();
    let mut edited_app = WorksheetApp::default();
    edited_app.open(&edited);
    assert!(
        edited_app.pending_regions.is_empty() && edited_app.last_executed_regions.is_empty(),
        "{} {:?}",
        edited_app.status,
        edited_app.pending_regions
    );
    let trace = edited_app.plot_results[&3].as_ref().unwrap().traces[0]
        .curve()
        .unwrap();
    assert!(trace.points.iter().all(|point| point[1] == 2. * point[0]));
    edited_app.persist_calculation_cache();
    edited_app
        .document
        .save(&output.join("native-xy-edited.mathcraft"))
        .unwrap();
    println!(
        "NATIVE_XY_FLOW plots=2 traces=3 points=1500 source_open=editable restored=3 cold_execution=0 pending=0 copied_payload=exact edited_curve=x2 print_execution=0"
    );
}
#[test]
fn native_xy_single_and_multi_trace_cached_open_edit_export_and_vector_print() {
    let output = std::env::temp_dir().join(format!(
        "mathcraft-native-xy-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    run(&output);
    for name in [
        "native-xy.mcdx",
        "native-xy-copy.mcdx",
        "native-xy.mathcraft",
        "native-xy.pdf",
        "native-xy-edited.mcdx",
        "native-xy-edited.mathcraft",
    ] {
        std::fs::remove_file(output.join(name)).unwrap();
    }
    std::fs::remove_dir(output).unwrap();
}
#[test]
#[ignore = "Explicit native XY source/cache/edit/PDF fixture; headless Rust only"]
fn native_xy_source_cache_save_reopen_print_probe() {
    run(&PathBuf::from(
        std::env::var_os("MATHCRAFT_NATIVE_XY_OUTPUT").expect("MATHCRAFT_NATIVE_XY_OUTPUT"),
    ));
}
