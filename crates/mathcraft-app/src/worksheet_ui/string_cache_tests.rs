//! Actual live concat -> saved inert carrier -> zero-evaluation cold restore.
use super::*;
use sha2::{Digest, Sha256};
use std::io::{Cursor, Read, Write};
fn node(kind: NodeKind, children: Vec<PortableTree>) -> PortableTree {
    PortableTree { kind, children }
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn fixture() -> (WorksheetApp, u64) {
    let mut app = WorksheetApp {
        automatic_calculation: false,
        ..Default::default()
    };
    let mut children = vec![node(
        NodeKind::AnnotatedIdentifier {
            name: "concat".into(),
            labels: "FUNCTION".into(),
        },
        vec![],
    )];
    children.extend((0..5).map(|_| node(NodeKind::StringLiteral("a".repeat(14000)), vec![])));
    let tree = node(
        NodeKind::Binary(BinaryOp::Evaluate),
        vec![
            node(NodeKind::Call { argument_count: 5 }, children),
            node(NodeKind::Placeholder, vec![]),
        ],
    );
    tree.validate().unwrap();
    let id = app.document.add_math(38.4, 38.4);
    app.document.regions.last_mut().unwrap().content = CanvasContent::Math(tree);
    app.recalculate();
    let Some(Value::String { value }) = app.results[&id].value.as_ref() else {
        panic!("{:?}", app.results[&id])
    };
    assert_eq!(value.len(), 70000);
    (app, id)
}
fn export(app: &mut WorksheetApp, path: &Path) {
    app.prepare_native_export().unwrap();
    mathcraft_document::export::export_mcdx(&app.document, path).unwrap();
}
fn native_result(path: &Path) -> Vec<u8> {
    NativePackage::open(path)
        .unwrap()
        .original_part("mathcad/result.xml")
        .unwrap()
        .to_vec()
}
fn assert_literal(bytes: &[u8]) {
    let text = std::str::from_utf8(bytes).unwrap();
    let doc = roxmltree::Document::parse(text).unwrap();
    let strings: Vec<_> = doc
        .descendants()
        .filter(|n| n.has_tag_name(("http://schemas.mathsoft.com/math50", "str")))
        .collect();
    assert_eq!(strings.len(), 1);
    assert_eq!(strings[0].text().unwrap(), "a".repeat(70000));
}
fn save(app: &mut WorksheetApp, path: &Path) {
    app.path = Some(path.to_owned());
    assert!(app.save(false), "{}", app.status);
}
fn cold(path: &Path) -> WorksheetApp {
    let mut app = WorksheetApp::default();
    app.open(path);
    assert!(
        app.last_executed_regions.is_empty(),
        "cold restore evaluated source"
    );
    app
}
fn synchronized(app: &WorksheetApp, id: u64) {
    assert_eq!(
        app.saved_results[&id].status,
        SavedCalculationStatus::Synchronized
    );
    assert!(!app.pending_regions.contains(&id));
    let source = app.saved_results[&id]
        .numeric_source
        .as_ref()
        .expect("exact carrier retained");
    assert!(super::inert_string_result::validate_source(source).is_some());
}
fn area_fixture(source: &Path, destination: &Path) {
    let mut archive = zip::ZipArchive::new(Cursor::new(std::fs::read(source).unwrap())).unwrap();
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for i in 0..archive.len() {
        let mut part = archive.by_index(i).unwrap();
        let mut bytes = Vec::new();
        part.read_to_end(&mut bytes).unwrap();
        if part.name().unwrap() == "mathcad/worksheet.xml" {
            let xml = String::from_utf8(bytes).unwrap();
            let dom = roxmltree::Document::parse(&xml).unwrap();
            let regions = dom
                .descendants()
                .find(|n| n.has_tag_name(("http://schemas.mathsoft.com/worksheet50", "regions")))
                .unwrap();
            let children: Vec<_> = regions.children().filter(|n| n.is_element()).collect();
            assert_eq!(children.len(), 1);
            let child = children[0].range();
            let wrapped = format!(
                "<region region-id=\"9000\" left=\"20\" top=\"20\"><Area IsCollapsed=\"false\" IsBorderVisible=\"true\" AreaHeight=\"150\"><regions>{}</regions></Area></region>",
                &xml[child.clone()]
            );
            let mut changed = xml.clone();
            changed.replace_range(child, &wrapped);
            bytes = changed.into_bytes();
        }
        writer
            .start_file(
                part.name().unwrap(),
                zip::write::SimpleFileOptions::default().compression_method(part.compression()),
            )
            .unwrap();
        writer.write_all(&bytes).unwrap();
    }
    std::fs::write(destination, writer.finish().unwrap().into_inner()).unwrap();
}
fn area_owner(app: &WorksheetApp) -> u64 {
    app.document
        .regions
        .iter()
        .find(|r| matches!(&r.content,CanvasContent::Opaque{kind,..} if kind=="Area"))
        .unwrap()
        .id
}
fn area_row(app: &WorksheetApp) -> &SavedCalculationRow {
    let rows = &app.area_saved[&area_owner(app)];
    assert_eq!(rows.len(), 1);
    rows.values().next().unwrap()
}
fn area_synchronized(app: &WorksheetApp) {
    let owner = area_owner(app);
    let row = area_row(app);
    assert_eq!(row.status, SavedCalculationStatus::Synchronized);
    assert!(app.area_pending.iter().all(|(id, _)| *id != owner));
    assert!(
        super::inert_string_result::validate_source(row.numeric_source.as_ref().unwrap()).is_some()
    );
}
fn area_run(output: &Path, plain_source: &Path) {
    let original = output.join("area-large-string-original.mcdx");
    area_fixture(plain_source, &original);
    let before = native_result(&original);
    assert_literal(&before);
    let mut imported = cold(&original);
    area_synchronized(&imported);
    let numeric = area_row(&imported).numeric_source.clone();
    let copy = output.join("area-large-string-native-copy.mcdx");
    export(&mut imported, &copy);
    let native_after = native_result(&copy);
    assert!(
        before == native_after,
        "Area native copy changed exact native cache"
    );
    let own = output.join("area-large-string-retained.mathcraft");
    save(&mut imported, &own);
    let mut reopened = cold(&own);
    area_synchronized(&reopened);
    assert_eq!(area_row(&reopened).numeric_source, numeric);
    let own_copy = output.join("area-large-string-own-copy.mcdx");
    export(&mut reopened, &own_copy);
    let own_after = native_result(&own_copy);
    assert!(
        before == own_after,
        "Area native-own-native changed exact native cache"
    );
    assert_literal(&own_after);
    for (file, bytes) in [
        ("area-result-before.xml", &before),
        ("area-result-native-copy.xml", &native_after),
        ("area-result-own-copy.xml", &own_after),
    ] {
        std::fs::write(output.join(file), bytes).unwrap();
    }
    let mut pending = cold(&own);
    let owner = area_owner(&pending);
    let key = pending.area_saved[&owner].keys().next().unwrap().clone();
    pending.area_pending.insert((owner, key));
    let path = output.join("area-large-string-pending.mathcraft");
    save(&mut pending, &path);
    let pending_cold = cold(&path);
    assert_eq!(
        area_row(&pending_cold).status,
        SavedCalculationStatus::Pending
    );
    assert_eq!(area_row(&pending_cold).numeric_source, numeric);
    let mut disabled = cold(&own);
    let owner = area_owner(&disabled);
    let CanvasContent::Opaque { xml, .. } = &mut disabled
        .document
        .regions
        .iter_mut()
        .find(|r| r.id == owner)
        .unwrap()
        .content
    else {
        panic!()
    };
    let dom = roxmltree::Document::parse(xml).unwrap();
    let math = dom
        .descendants()
        .find(|n| n.has_tag_name(("http://schemas.mathsoft.com/worksheet50", "math")))
        .unwrap();
    assert!(math.attribute("disable-calc").is_none());
    let start = math.range().start;
    let end = xml[start..]
        .find(|c: char| c.is_ascii_whitespace() || c == '>')
        .unwrap();
    xml.insert_str(start + end, " disable-calc=\"true\"");
    let path = output.join("area-large-string-disabled.mathcraft");
    save(&mut disabled, &path);
    let disabled_cold = cold(&path);
    assert_eq!(
        area_row(&disabled_cold).status,
        SavedCalculationStatus::Pending
    );
    assert_eq!(area_row(&disabled_cold).numeric_source, numeric);
    let mut changed = cold(&own);
    let owner = area_owner(&changed);
    let CanvasContent::Opaque { xml, .. } = &mut changed
        .document
        .regions
        .iter_mut()
        .find(|r| r.id == owner)
        .unwrap()
        .content
    else {
        panic!()
    };
    let dom = roxmltree::Document::parse(xml).unwrap();
    let string = dom
        .descendants()
        .find(|n| n.has_tag_name(("http://schemas.mathsoft.com/math50", "str")))
        .unwrap();
    let text = string.first_child().unwrap().range();
    xml.replace_range(text.start..text.start + 1, "b");
    changed.document.calculation_cache = None;
    let path = output.join("area-large-string-source-changed.mathcraft");
    changed.document.save(&path).unwrap();
    let changed_cold = cold(&path);
    assert_eq!(
        area_row(&changed_cold).status,
        SavedCalculationStatus::Pending
    );
    std::fs::write(output.join("area-validation.json"),serde_json::to_vec_pretty(&serde_json::json!({"value_bytes":70000,"original_sha256":hash(&before),"native_copy_sha256":hash(&native_after),"own_copy_sha256":hash(&own_after),"cold_executed_regions":0,"pending_preserved":true,"disabled_child_preserved":true,"source_change_pending":true})).unwrap()).unwrap();
}
fn run(output: &Path) {
    std::fs::create_dir_all(output).unwrap();
    let (mut live, id) = fixture();
    // Engine result remains exact even when structured presentation exceeds its
    // independent64KiB UI budget; there is no truncation/rendering assertion.
    live.persist_calculation_cache();
    let direct_source = live.document.calculation_cache.as_ref().unwrap().rows[0]
        .numeric_source
        .clone();
    let direct_own = output.join("large-string-direct.mathcraft");
    live.document.save(&direct_own).unwrap();
    let own_direct = cold(&direct_own);
    synchronized(&own_direct, id);
    assert_eq!(own_direct.saved_results[&id].numeric_source, direct_source);
    let original = output.join("large-string-original.mcdx");
    export(&mut live, &original);
    let before = native_result(&original);
    assert_literal(&before);
    std::fs::write(output.join("result-before.xml"), &before).unwrap();
    let mut imported = cold(&original);
    synchronized(&imported, id);
    let imported_source = imported.saved_results[&id].numeric_source.clone();
    let copy = output.join("large-string-native-copy.mcdx");
    export(&mut imported, &copy);
    let native_after = native_result(&copy);
    assert_literal(&native_after);
    assert!(
        before == native_after,
        "native cold copy changed exact result part {} -> {}",
        hash(&before),
        hash(&native_after)
    );
    let own = output.join("large-string-retained.mathcraft");
    save(&mut imported, &own);
    let mut own_cold = cold(&own);
    synchronized(&own_cold, id);
    assert_eq!(own_cold.saved_results[&id].numeric_source, imported_source);
    let own_copy = output.join("large-string-own-copy.mcdx");
    export(&mut own_cold, &own_copy);
    let own_after = native_result(&own_copy);
    assert_literal(&own_after);
    assert!(
        before == own_after,
        "native-own-native changed exact result part {} -> {}",
        hash(&before),
        hash(&own_after)
    );
    std::fs::write(output.join("result-native-copy.xml"), &native_after).unwrap();
    std::fs::write(output.join("result-own-copy.xml"), &own_after).unwrap();
    // A valid historical carrier must never upgrade an actual Pending row.
    let mut pending = cold(&own);
    pending.pending_regions.insert(id);
    let path = output.join("large-string-pending.mathcraft");
    save(&mut pending, &path);
    let pending_cold = cold(&path);
    assert_eq!(
        pending_cold.saved_results[&id].status,
        SavedCalculationStatus::Pending
    );
    assert!(pending_cold.pending_regions.contains(&id));
    assert_eq!(
        pending_cold.saved_results[&id].numeric_source,
        imported_source
    );
    // Disabled status is also preserved, independent of carrier validity.
    let mut disabled = cold(&own);
    disabled.document.regions[0].calculation_disabled = true;
    let path = output.join("large-string-disabled.mathcraft");
    save(&mut disabled, &path);
    let disabled_cold = cold(&path);
    assert_eq!(
        disabled_cold.saved_results[&id].status,
        SavedCalculationStatus::Pending
    );
    assert!(disabled_cold.document.regions[0].calculation_disabled);
    assert_eq!(
        disabled_cold.saved_results[&id].numeric_source,
        imported_source
    );
    // Retained archive source proof remains authoritative on a newer own source.
    let mut changed = cold(&own);
    let CanvasContent::Math(tree) = &mut changed.document.regions[0].content else {
        panic!()
    };
    let NodeKind::StringLiteral(value) = &mut tree.children[0].children[1].kind else {
        panic!()
    };
    value.replace_range(0..1, "b");
    changed.document.calculation_cache = None;
    let path = output.join("large-string-source-changed.mathcraft");
    changed.document.save(&path).unwrap();
    let changed_cold = cold(&path);
    assert_eq!(
        changed_cold.saved_results[&id].status,
        SavedCalculationStatus::Pending
    );
    assert!(changed_cold.pending_regions.contains(&id));
    // Unknown metadata cannot obtain string admission from a permissive generic
    // Value deserializer. No valid display is provided to prove this gate.
    let mut malformed = cold(&own);
    let row = &mut malformed.document.calculation_cache.as_mut().unwrap().rows[0];
    row.status = SavedCalculationStatus::Synchronized;
    row.display = None;
    row.numeric_source = Some(SavedNumericSource::EvaluatedValue(
        r#"{"kind":"string","value":"a","future":0}"#.into(),
    ));
    let path = output.join("large-string-malformed.mathcraft");
    malformed.document.save(&path).unwrap();
    let malformed_cold = cold(&path);
    assert_eq!(
        malformed_cold.saved_results[&id].status,
        SavedCalculationStatus::Pending
    );
    assert!(malformed_cold.pending_regions.contains(&id));
    area_run(output, &original);
    std::fs::write(output.join("validation.json"),serde_json::to_vec_pretty(&serde_json::json!({"value_bytes":70000,"native_original_bytes":before.len(),"native_original_sha256":hash(&before),"native_copy_sha256":hash(&native_after),"own_copy_sha256":hash(&own_after),"cold_executed_regions":0,"pending_preserved":true,"disabled_preserved":true,"source_change_pending":true,"malformed_pending":true})).unwrap()).unwrap();
}
fn temporary() -> PathBuf {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    std::env::temp_dir().join(format!(
        "mathcraft-large-string-cache-{}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}
#[test]
fn large_string_exact_inert_source_survives_native_own_and_negative_cold_paths() {
    let path = temporary();
    run(&path);
    for file in std::fs::read_dir(&path).unwrap() {
        std::fs::remove_file(file.unwrap().path()).unwrap();
    }
    std::fs::remove_dir(path).unwrap();
}
#[test]
#[ignore = "Actual inert large String own/native artifacts; no GUI or source evaluation on restore"]
fn string_cache_preservation_native_export_reopen_probe() {
    let output = PathBuf::from(
        std::env::var_os("MATHCRAFT_STRING_CACHE_OUTPUT").expect("set output directory"),
    );
    run(&output);
}
