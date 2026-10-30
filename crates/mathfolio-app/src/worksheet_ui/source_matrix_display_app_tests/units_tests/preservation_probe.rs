//! Explicit file packet only; existing two normal unit regressions stay intact.
use super::*;
use sha2::{Digest, Sha256};

fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn file(path: &Path) -> serde_json::Value {
    let bytes = std::fs::read(path).unwrap();
    serde_json::json!({"name":path.file_name().unwrap().to_str().unwrap(),"bytes":bytes.len(),"sha256":sha(&bytes)})
}
fn native_query(package: &NativePackage, unit: bool) -> (String, String) {
    let source = package.worksheet_xml().unwrap();
    let document = roxmltree::Document::parse(&source).unwrap();
    let math = document
        .descendants()
        .find(|n| {
            n.has_tag_name((WS, "math")) && n.children().any(|c| c.has_tag_name((ML, "eval")))
        })
        .unwrap();
    let reference = math.attribute("resultRef").unwrap().to_owned();
    let rows = package.caches().unwrap();
    let rows: Vec<_> = rows
        .iter()
        .filter(|row| row.matches_result_ref(&reference))
        .collect();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].calculation_status.as_deref(), Some("Synchronized"));
    let xml = rows[0].result_mathml.as_ref().unwrap().clone();
    if unit {
        native_display_value(&xml);
    } else {
        assert_native_value(&xml);
    }
    (reference, xml)
}
fn numeric_values(value: &Value) -> Vec<f64> {
    let Value::Matrix { elements, .. } = value else {
        panic!("missing matrix")
    };
    elements.iter().map(|q| q.value.re).collect()
}
fn case(output: &Path, unit: bool) -> serde_json::Value {
    let scratch = Scratch::new();
    let generated = if unit {
        unit_fixture(&scratch.0, None)
    } else {
        fixture(&scratch.0, None)
    };
    let name = if unit { "source-unit" } else { "source-plain" };
    let input = output.join(format!("{name}-input.mcdx"));
    let own = output.join(format!("{name}.mathfolio"));
    let native = output.join(format!("{name}-output.mcdx"));
    std::fs::copy(&generated, &input).unwrap();
    let original = std::fs::read(&input).unwrap();
    let original_package = NativePackage::open(&input).unwrap();
    let source = source_matrix(&original_package);
    let unit_source_before = unit.then(|| unit_source(&original_package));
    let mut app = manual_open(&input);
    let definition = definition(&app);
    let owner = query(&app);
    let regions = app.document.regions.clone();
    let source_signature = app.document.calculation_signature();
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
    if unit {
        matrix_value(row.value.as_ref().unwrap(), 1., LENGTH);
        matrix_value(row.display_value.as_ref().unwrap(), 100., [0.; 7]);
    } else {
        assert_value(row.value.as_ref().unwrap());
    }
    let physical = numeric_values(row.value.as_ref().unwrap());
    let dimensions = if unit { LENGTH } else { [0.; 7] };
    let display = numeric_values(row.display_value.as_ref().or(row.value.as_ref()).unwrap());
    let executed = app.last_executed_regions.clone();
    assert!(executed.contains(&definition.to_string()) && executed.contains(&owner.to_string()));
    app.last_executed_regions.clear();
    let generation = app.background.submitted_generation();
    assert_eq!(app.document.regions, regions);
    assert_eq!(app.document.calculation_signature(), source_signature);
    assert_eq!(
        app.document.native_archive.as_deref(),
        Some(original.as_slice())
    );
    app.persist_calculation_cache();
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
        .unwrap()
        .clone();
    app.document.save(&own).unwrap();
    assert!(app.last_executed_regions.is_empty());
    assert_eq!(app.background.submitted_generation(), generation);
    let mut cold = manual_open(&own);
    if unit {
        assert_eq!(saved_cm(&cold), numeric);
    } else {
        saved_query(&cold, query(&cold));
        assert_eq!(
            cold.saved_results[&query(&cold)].numeric_source.as_ref(),
            Some(&numeric)
        );
    }
    assert_eq!(cold.document.regions, regions);
    assert_eq!(
        cold.document.native_archive.as_deref(),
        Some(original.as_slice())
    );
    let cold_generation = cold.background.submitted_generation();
    cold.prepare_native_export().unwrap();
    mathfolio_document::export::export_mcdx(&cold.document, &native).unwrap();
    assert_eq!(cold.background.submitted_generation(), cold_generation);
    assert!(cold.last_executed_regions.is_empty());
    let native_package = NativePackage::open(&native).unwrap();
    assert_eq!(source_matrix(&native_package), source);
    if let Some(before) = unit_source_before {
        assert_eq!(unit_source(&native_package), before);
    }
    let (reference, result_mathml) = native_query(&native_package, unit);
    let native_cold = manual_open(&native);
    if unit {
        saved_cm(&native_cold);
    } else {
        saved_query(&native_cold, query(&native_cold));
    }
    assert_eq!(std::fs::read(&input).unwrap(), original);
    let original_result = original_package
        .original_part("mathcad/result.xml")
        .unwrap();
    let result = native_package.original_part("mathcad/result.xml").unwrap();
    serde_json::json!({"name":name,"unit_source":unit,"scope":"actual Rust worksheet setup once; own/native cold and save/export no extra execution; source viewport editor/print unsupported","full_shape":[2,3],"physical_row_major":physical,"physical_dimension":dimensions,"display_row_major":display,"display_dimension":[0,0,0,0,0,0,0],"source_xml_column_major":[1,4,2,5,3,6],"source_display":{"offset":[0,1],"size":[1,2]},"visible_source_row_major":[2,3],"definition_owner":definition,"query_owner":owner,"source_signature":source_signature,"setup_calculation_requests":1,"setup_executed_region_ids":executed,"setup_generation":generation,"executions_after_setup":app.last_executed_regions,"cold_own_executed_region_ids":cold.last_executed_regions,"cold_native_executed_region_ids":native_cold.last_executed_regions,"cold_own_generation":cold_generation,"cold_native_generation":native_cold.background.submitted_generation(),"source_matrix_xml":source,"input_worksheet_sha256":sha(original_package.original_part(original_package.worksheet_part_name()).unwrap()),"output_worksheet_sha256":sha(native_package.original_part(native_package.worksheet_part_name()).unwrap()),"input_result_xml_sha256":sha(original_result),"output_result_xml_sha256":sha(result),"output_result_xml_bytes":result.len(),"result_reference":reference,"result_status":"Synchronized","result_mathml":result_mathml,"own_numeric_source":numeric,"own_numeric_source_sha256":sha(&serde_json::to_vec(&numeric).unwrap()),"own_retained_archive_sha256":sha(cold.document.native_archive.as_ref().unwrap()),"files":[file(&input),file(&own),file(&native)]})
}

#[test]
#[ignore = "Explicit raw source matrix display native/own preservation packet"]
fn source_matrix_display_native_own_preservation_probe() {
    let output = PathBuf::from(
        std::env::var_os("MATHFOLIO_SOURCE_MATRIX_DISPLAY_OUTPUT")
            .expect("MATHFOLIO_SOURCE_MATRIX_DISPLAY_OUTPUT"),
    );
    assert!(!output.exists(), "use a fresh output directory");
    std::fs::create_dir_all(&output).unwrap();
    let cases = [case(&output, false), case(&output, true)];
    let observation =
        serde_json::json!({"version":1,"original_runtime":false,"pdfs":0,"cases":cases});
    std::fs::write(
        output.join("observation.json"),
        serde_json::to_vec_pretty(&observation).unwrap(),
    )
    .unwrap();
    assert_eq!(std::fs::read_dir(&output).unwrap().count(), 7);
    println!(
        "SOURCE_MATRIX_DISPLAY_PACKET native=4 own=2 observations=1 full_shape=2x3 full_cells=6 unit_source=m explicit_display_unit=cm postsetup_execution=0"
    );
}
