//! Source-display metadata never clips unit-bearing calculation inputs.
//! Explicit cm conversion is existing Scope::display policy, not unit inference.
use super::*;

const LENGTH: [f64; 7] = [1., 0., 0., 0., 0., 0., 0.];

fn metres(number: &str) -> String {
    format!(
        "<ml:apply><ml:mult/><ml:real>{number}</ml:real><ml:id labels=\"UNIT\">m</ml:id></ml:apply>"
    )
}
fn unit_fixture(root: &Path, hidden_bad: Option<&str>) -> PathBuf {
    let plain = fixture(root, None);
    let package = NativePackage::open(&plain).unwrap();
    let source = package.worksheet_xml().unwrap();
    let xml = roxmltree::Document::parse(&source).unwrap();
    let matrix = xml
        .descendants()
        .find(|n| n.has_tag_name((ML, "matrix")))
        .unwrap();
    let cells: Vec<_> = matrix
        .children()
        .filter(|n| n.is_element() && !n.has_tag_name((ML, "display")))
        .collect();
    assert_eq!(cells.len(), 6);
    let mut patches: Vec<_> = cells
        .iter()
        .enumerate()
        .map(|(index, cell)| {
            let body = if index == 1 {
                hidden_bad
                    .map(str::to_owned)
                    .unwrap_or_else(|| metres(cell.text().unwrap()))
            } else {
                metres(cell.text().unwrap())
            };
            (cell.range(), body)
        })
        .collect();
    let output = xml
        .descendants()
        .find(|n| n.has_tag_name((ML, "unitOverride")))
        .unwrap();
    assert!(
        output
            .first_element_child()
            .unwrap()
            .has_tag_name((ML, "placeholder"))
    );
    patches.push((
        output.range(),
        "<ml:unitOverride><ml:id labels=\"UNIT\">cm</ml:id></ml:unitOverride>".into(),
    ));
    patches.sort_by_key(|patch| std::cmp::Reverse(patch.0.start));
    let mut changed = source.clone();
    for (range, body) in patches {
        changed.replace_range(range, &body);
    }
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for part in package.parts() {
        writer
            .start_file(
                &part.name,
                zip::write::SimpleFileOptions::default().compression_method(part.compression),
            )
            .unwrap();
        writer
            .write_all(if part.name == package.worksheet_part_name() {
                changed.as_bytes()
            } else {
                part.bytes()
            })
            .unwrap();
    }
    let path = root.join("source-display-metres.mcdx");
    NativePackage::from_bytes(&writer.finish().unwrap().into_inner())
        .unwrap()
        .write_new(&path)
        .unwrap();
    path
}
fn matrix_value(value: &Value, factor: f64, dimensions: [f64; 7]) {
    let Value::Matrix {
        rows,
        cols,
        elements,
    } = value
    else {
        panic!("not complete matrix: {value:?}")
    };
    assert_eq!((*rows, *cols, elements.len()), (2, 3, 6));
    for (quantity, number) in elements.iter().zip(1..=6) {
        assert!(
            (quantity.value.re - f64::from(number) * factor).abs() < 1e-9,
            "{quantity:?}"
        );
        assert_eq!(quantity.value.im, 0.);
        assert_eq!(quantity.dimension, dimensions);
    }
}
fn unit_source(package: &NativePackage) -> (String, String) {
    let raw = source_matrix(package);
    let source = package.worksheet_xml().unwrap();
    let document = roxmltree::Document::parse(&source).unwrap();
    let matrix = document
        .descendants()
        .find(|n| n.has_tag_name((ML, "matrix")))
        .unwrap();
    let cells: Vec<_> = matrix
        .children()
        .filter(|n| n.is_element() && !n.has_tag_name((ML, "display")))
        .collect();
    assert_eq!(cells.len(), 6);
    for (cell, number) in cells.iter().zip([1, 4, 2, 5, 3, 6]) {
        let args: Vec<_> = cell
            .children()
            .filter(roxmltree::Node::is_element)
            .collect();
        assert!(
            cell.has_tag_name((ML, "apply"))
                && args.len() == 3
                && args[0].has_tag_name((ML, "mult"))
        );
        assert!(args[1].has_tag_name((ML, "real")));
        assert_eq!(args[1].text(), Some(number.to_string().as_str()));
        assert!(args[2].has_tag_name((ML, "id")));
        assert_eq!(args[2].attribute("labels"), Some("UNIT"));
        assert_eq!(args[2].text(), Some("m"));
    }
    let output = document
        .descendants()
        .find(|n| n.has_tag_name((ML, "unitOverride")))
        .unwrap();
    let id = output.first_element_child().unwrap();
    assert!(id.has_tag_name((ML, "id")));
    assert_eq!(id.attribute("labels"), Some("UNIT"));
    assert_eq!(id.text(), Some("cm"));
    (raw, source[output.range()].to_owned())
}
fn native_display_value(xml: &str) {
    let dom = roxmltree::Document::parse(xml).unwrap();
    let matrices: Vec<_> = dom
        .descendants()
        .filter(|n| n.has_tag_name((ML, "matrix")))
        .collect();
    assert_eq!(matrices.len(), 1);
    let matrix = matrices[0];
    assert_eq!(
        (matrix.attribute("rows"), matrix.attribute("cols")),
        (Some("2"), Some("3"))
    );
    let cells: Vec<_> = matrix
        .children()
        .filter(roxmltree::Node::is_element)
        .collect();
    assert_eq!(cells.len(), 6);
    for (cell, number) in cells.iter().zip([100., 400., 200., 500., 300., 600.]) {
        assert!(
            cell.has_tag_name((ML, "real"))
                && cell.attributes().len() == 0
                && cell.first_element_child().is_none()
        );
        assert!((cell.text().unwrap().parse::<f64>().unwrap() - number).abs() < 1e-9);
    }
    let prepared = PreparedNativeMatrix::parse(xml).unwrap();
    assert_eq!(prepared.shape(), (2, 3));
    let p = prepared
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
        p.cells.iter().map(|c| c.plain()).collect::<Vec<_>>(),
        ["100", "200", "300", "400", "500", "600"]
    );
}
fn saved_cm(app: &WorksheetApp) -> SavedNumericSource {
    let row = &app.saved_results[&query(app)];
    assert_eq!(row.status, SavedCalculationStatus::Synchronized, "{row:?}");
    assert!(row.error.is_none());
    let source = row.numeric_source.as_ref().unwrap();
    match source {
        SavedNumericSource::EvaluatedValue(json) => {
            matrix_value(&serde_json::from_str(json).unwrap(), 100., [0.; 7])
        }
        SavedNumericSource::NativeMathml(xml) => native_display_value(xml),
        other => panic!("missing full converted numeric payload: {other:?}"),
    }
    let display = FormattedResult::from_saved(row.display.as_deref().unwrap()).unwrap();
    assert_eq!((display.rows, display.cols, display.cells.len()), (2, 3, 6));
    assert_eq!(
        display.cells.iter().map(|c| c.plain()).collect::<Vec<_>>(),
        ["100", "200", "300", "400", "500", "600"]
    );
    source.clone()
}
fn query_cache(package: &NativePackage) -> String {
    let source = package.worksheet_xml().unwrap();
    let doc = roxmltree::Document::parse(&source).unwrap();
    let math = doc
        .descendants()
        .find(|n| {
            n.has_tag_name((WS, "math")) && n.children().any(|c| c.has_tag_name((ML, "eval")))
        })
        .unwrap();
    let reference = math.attribute("resultRef").unwrap();
    let rows = package.caches().unwrap();
    let rows: Vec<_> = rows
        .iter()
        .filter(|row| row.matches_result_ref(reference))
        .collect();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].calculation_status.as_deref(), Some("Synchronized"));
    let xml = rows[0].result_mathml.as_ref().unwrap();
    native_display_value(xml);
    xml.clone()
}

#[test]
fn raw_source_display_metres_to_explicit_cm_keeps_full_si_value_and_full_cold_display_payload() {
    let scratch = Scratch::new();
    let path = unit_fixture(&scratch.0, None);
    let original = std::fs::read(&path).unwrap();
    let expected_source = unit_source(&NativePackage::open(&path).unwrap());
    let mut app = manual_open(&path);
    let owner = query(&app);
    let definition = definition(&app);
    let regions = app.document.regions.clone();
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    app.ribbon_command(&ctx, RibbonAction::Recalculate);
    assert!(
        app.results[&definition].error.is_none(),
        "{:?}",
        app.results[&definition]
    );
    let row = &app.results[&owner];
    assert!(row.error.is_none(), "{row:?}");
    matrix_value(row.value.as_ref().unwrap(), 1., LENGTH);
    matrix_value(row.display_value.as_ref().unwrap(), 100., [0.; 7]);
    let live_display_json = serde_json::to_string(row.display_value.as_ref().unwrap()).unwrap();
    assert_eq!(app.document.regions, regions);
    assert_eq!(
        app.document.native_archive.as_deref(),
        Some(original.as_slice())
    );
    app.persist_calculation_cache();
    let own = scratch.0.join("metres-cm.mathcraft");
    app.document.save(&own).unwrap();
    let mut own_cold = manual_open(&own);
    assert_eq!(
        saved_cm(&own_cold),
        SavedNumericSource::EvaluatedValue(live_display_json)
    );
    let generation = own_cold.background.submitted_generation();
    own_cold.prepare_native_export().unwrap();
    let copy = scratch.0.join("metres-cm-copy.mcdx");
    mathcraft_document::export::export_mcdx(&own_cold.document, &copy).unwrap();
    assert_eq!(own_cold.background.submitted_generation(), generation);
    assert!(own_cold.last_executed_regions.is_empty());
    let exported = NativePackage::open(&copy).unwrap();
    assert_eq!(unit_source(&exported), expected_source);
    query_cache(&exported);
    let raw_result = exported
        .original_part("mathcad/result.xml")
        .unwrap()
        .to_vec();
    let mut native_cold = manual_open(&copy);
    let native_numeric = saved_cm(&native_cold);
    native_cold.persist_calculation_cache();
    let own_again = scratch.0.join("native-cold.mathcraft");
    native_cold.document.save(&own_again).unwrap();
    let mut final_cold = manual_open(&own_again);
    assert_eq!(saved_cm(&final_cold), native_numeric);
    let generation = final_cold.background.submitted_generation();
    final_cold.prepare_native_export().unwrap();
    let final_copy = scratch.0.join("native-cold-copy.mcdx");
    mathcraft_document::export::export_mcdx(&final_cold.document, &final_copy).unwrap();
    let final_package = NativePackage::open(&final_copy).unwrap();
    assert_eq!(unit_source(&final_package), expected_source);
    query_cache(&final_package);
    assert_eq!(
        final_package.original_part("mathcad/result.xml").unwrap(),
        raw_result
    );
    assert_eq!(final_cold.background.submitted_generation(), generation);
    assert!(final_cold.last_executed_regions.is_empty());
    assert_eq!(std::fs::read(&path).unwrap(), original);
}

#[test]
fn hidden_incompatible_or_malformed_unit_cell_cannot_be_skipped_by_source_display() {
    for (bad, definition_error) in [
        (
            "<ml:apply><ml:mult/><ml:real>4</ml:real><ml:id labels=\"UNIT\">s</ml:id></ml:apply>",
            false,
        ),
        (
            "<ml:apply><ml:mult/><ml:real>4</ml:real><ml:unknown-unit/></ml:apply>",
            true,
        ),
    ] {
        let scratch = Scratch::new();
        let path = unit_fixture(&scratch.0, Some(bad));
        let original = std::fs::read(&path).unwrap();
        let mut app = manual_open(&path);
        let def = definition(&app);
        let owner = query(&app);
        let regions = app.document.regions.clone();
        let ctx = egui::Context::default();
        crate::configure(&ctx);
        app.ribbon_command(&ctx, RibbonAction::Recalculate);
        assert_eq!(
            app.results[&def].error.is_some(),
            definition_error,
            "{:?}",
            app.results[&def]
        );
        let row = &app.results[&owner];
        assert!(row.error.is_some(), "hidden unit fault discarded: {row:?}");
        if !definition_error {
            assert_eq!(row.error.as_ref().unwrap().code, "bad%_dimensions");
        }
        assert!(row.value.is_none() && row.display_value.is_none());
        app.persist_calculation_cache();
        let saved = app
            .document
            .calculation_cache
            .as_ref()
            .unwrap()
            .rows
            .iter()
            .find(|row| row.region_id == owner)
            .unwrap();
        assert!(saved.error.is_some() && saved.numeric_source.is_none() && saved.display.is_none());
        assert_eq!(app.document.regions, regions);
        assert_eq!(std::fs::read(&path).unwrap(), original);
    }
}

mod preservation_probe;

mod print_probe;
