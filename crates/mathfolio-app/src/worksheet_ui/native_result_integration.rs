//! End-to-end inert native results: calculator -> MCDX -> cold open -> print.
use super::*;
fn node(kind: NodeKind, children: Vec<PortableTree>) -> PortableTree {
    PortableTree { kind, children }
}
fn number(text: &str) -> PortableTree {
    node(NodeKind::Number(text.into()), vec![])
}
fn query(value: PortableTree) -> PortableTree {
    node(
        NodeKind::Binary(BinaryOp::Evaluate),
        vec![value, node(NodeKind::Placeholder, vec![])],
    )
}
fn run(output: &Path) {
    std::fs::create_dir_all(output).unwrap();
    let mut app = WorksheetApp::default();
    let complex=worksheet_math::tree_from_mathml(r#"<m:apply xmlns:m="http://schemas.mathsoft.com/math50"><m:plus/><m:real>2</m:real><m:imag>3</m:imag></m:apply>"#).unwrap();
    let matrix = node(
        NodeKind::Matrix {
            rows: 2,
            cols: 3,
            display: None,
        },
        ["1", "2", "3", "4", "5", "6"]
            .into_iter()
            .map(number)
            .collect(),
    );
    let converted = worksheet_math::tree_from_mathml(r#"<m:eval xmlns:m="http://schemas.mathsoft.com/math50"><m:apply><m:mult/><m:real>0.001</m:real><m:id labels="UNIT">s</m:id></m:apply><m:unitOverride><m:id labels="UNIT">ms</m:id></m:unitOverride></m:eval>"#).unwrap();
    let mut expected = HashMap::new();
    for (index, value) in [
        query(number("7.125")),
        query(complex),
        query(matrix),
        converted,
    ]
    .into_iter()
    .enumerate()
    {
        let id = app.document.add_math(20., 20. + index as f64 * 130.);
        app.document.regions.last_mut().unwrap().content = CanvasContent::Math(value);
        expected.insert(id, ());
    }
    app.recalculate();
    assert!(
        app.results.values().all(|row| row.error.is_none()),
        "{:?}",
        app.results
    );
    let displays = expected
        .keys()
        .map(|id| {
            (
                *id,
                crate::result_display::format(
                    app.results[id]
                        .display_value
                        .as_ref()
                        .or(app.results[id].value.as_ref())
                        .unwrap(),
                    &app.document
                        .regions
                        .iter()
                        .find(|region| region.id == *id)
                        .unwrap()
                        .format,
                ),
            )
        })
        .collect::<HashMap<_, _>>();
    assert_eq!(
        displays[&3]
            .cells
            .iter()
            .map(|cell| cell.plain())
            .collect::<Vec<_>>(),
        ["1", "2", "3", "4", "5", "6"]
    );
    assert!(
        matches!(&app.results[&2].value,Some(Value::Scalar{quantity}) if quantity.value.re==2. && quantity.value.im==3.)
    );
    assert_eq!(
        displays[&4]
            .cells
            .iter()
            .map(|cell| cell.plain())
            .collect::<Vec<_>>(),
        ["1"]
    );
    app.persist_calculation_cache();
    let native = output.join("native-results.mcdx");
    mathfolio_document::export::export_mcdx(&app.document, &native).unwrap();
    let package = mathfolio_document::native::NativePackage::open(&native).unwrap();
    assert_eq!(package.caches().unwrap().len(), 4);
    assert!(
        package
            .caches()
            .unwrap()
            .iter()
            .all(|row| row.calculation_status.as_deref() == Some("Synchronized"))
    );
    let mut opened = WorksheetApp::default();
    opened.open(&native);
    assert!(
        opened.results.is_empty()
            && opened.last_executed_regions.is_empty()
            && opened.pending_regions.is_empty(),
        "{} {:?}",
        opened.status,
        opened.saved_results
    );
    assert_eq!(opened.saved_results.len(), 4);
    for region in &opened.document.regions {
        let row = &opened.saved_results[&region.id];
        let display = numeric_cache::display(row, &region.format).unwrap();
        assert_eq!(display, displays[&region.id]);
    }
    let mut snapshot = opened.print_snapshot();
    let plan = snapshot.plan(PageSelection::All, 300).unwrap();
    let pages =
        printing::render_vector_pages(&plan, |ui, page, total| snapshot.paint(ui, page, total))
            .unwrap();
    printing::write_pdf(&output.join("native-results.pdf"), &pages).unwrap();
    assert!(opened.results.is_empty() && opened.last_executed_regions.is_empty());
    opened.persist_calculation_cache();
    opened
        .document
        .save(&output.join("native-results.mathfolio"))
        .unwrap();
    let copied = output.join("native-results-copy.mcdx");
    mathfolio_document::export::export_mcdx(&opened.document, &copied).unwrap();
    let copied = mathfolio_document::native::NativePackage::open(&copied).unwrap();
    assert_eq!(
        copied.original_part("mathcad/result.xml"),
        package.original_part("mathcad/result.xml")
    );
    println!(
        "NATIVE_RESULT_PERSISTENCE queries=4 scalar=1 complex=1 matrix=2x3 unit_override=1 restored=4 cold_execution=0 pending=0 print_execution=0"
    );
}
#[test]
fn scalar_complex_non_square_matrix_cold_native_open_and_vector_print_restore_without_evaluation() {
    let output = std::env::temp_dir().join(format!(
        "mathfolio-native-results-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    run(&output);
    for name in [
        "native-results.mcdx",
        "native-results-copy.mcdx",
        "native-results.mathfolio",
        "native-results.pdf",
    ] {
        std::fs::remove_file(output.join(name)).unwrap();
    }
    std::fs::remove_dir(output).unwrap();
}
#[test]
#[ignore = "Explicit native result persistence fixture; headless Rust only"]
fn native_result_save_reopen_print_probe() {
    run(&PathBuf::from(
        std::env::var_os("MATHFOLIO_NATIVE_RESULT_OUTPUT").expect("MATHFOLIO_NATIVE_RESULT_OUTPUT"),
    ));
}
