//! Actual package -> controller -> inline/worker string indexing settings.
use super::*;
use std::io::{Cursor, Read, Write};
fn atom(kind: NodeKind) -> PortableTree {
    PortableTree {
        kind,
        children: vec![],
    }
}
fn op(kind: NodeKind, children: Vec<PortableTree>) -> PortableTree {
    PortableTree { kind, children }
}
fn call(name: &str, args: Vec<PortableTree>) -> PortableTree {
    let count = args.len();
    let mut children = vec![atom(NodeKind::AnnotatedIdentifier {
        name: name.into(),
        labels: "FUNCTION".into(),
    })];
    children.extend(args);
    op(
        NodeKind::Call {
            argument_count: count,
        },
        children,
    )
}
fn num(n: &str) -> PortableTree {
    atom(NodeKind::Number(n.into()))
}
fn string(s: &str) -> PortableTree {
    atom(NodeKind::StringLiteral(s.into()))
}
fn add(app: &mut WorksheetApp, tree: PortableTree) -> u64 {
    let id = app
        .document
        .add_math(38.4, app.document.regions.len() as f64 * 70. + 38.4);
    app.document
        .regions
        .iter_mut()
        .find(|r| r.id == id)
        .unwrap()
        .content = CanvasContent::Math(tree);
    id
}
fn eval(tree: PortableTree) -> PortableTree {
    op(
        NodeKind::Binary(BinaryOp::Evaluate),
        vec![tree, atom(NodeKind::Placeholder)],
    )
}
fn scratch() -> PathBuf {
    // SystemTime can have coarser resolution than concurrent test startup.
    // The process-local sequence makes ownership distinct even at equal ticks.
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let p = std::env::temp_dir().join(format!(
        "mathfolio-string-settings-{}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&p).unwrap();
    p
}
fn patch_setting(path: &Path, flag: &str) {
    let mut input = zip::ZipArchive::new(Cursor::new(std::fs::read(path).unwrap())).unwrap();
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let mut patched = false;
    for i in 0..input.len() {
        let mut part = input.by_index(i).unwrap();
        let mut bytes = Vec::new();
        part.read_to_end(&mut bytes).unwrap();
        if part.name().unwrap() == "mathcad/settings/calculation.xml" {
            bytes = format!("<calculation xmlns='http://schemas.ptc.com/mathcad/settings/calculation10'><calculationBehavior strings-use-origin='{flag}'/></calculation>").into_bytes();
            patched = true;
        }
        output
            .start_file(
                part.name().unwrap(),
                zip::write::SimpleFileOptions::default().compression_method(part.compression()),
            )
            .unwrap();
        output.write_all(&bytes).unwrap();
    }
    assert!(patched);
    std::fs::write(path, output.finish().unwrap().into_inner()).unwrap();
}
fn fixture(path: &Path) -> [u64; 4] {
    let mut app = WorksheetApp::default();
    add(
        &mut app,
        op(
            NodeKind::Binary(BinaryOp::Define),
            vec![
                atom(NodeKind::AnnotatedIdentifier {
                    name: "ORIGIN".into(),
                    labels: "SYSTEM".into(),
                }),
                num("1"),
            ],
        ),
    );
    let substring = add(
        &mut app,
        eval(call("substr", vec![string("abc"), num("1"), num("1")])),
    );
    let search = add(
        &mut app,
        eval(call("search", vec![string("abc"), string("b"), num("1")])),
    );
    let length = add(&mut app, eval(call("strlen", vec![string("abc")])));
    let numeric = add(&mut app, eval(num("9")));
    app.recalculate();
    assert_values(&app, [substring, search, length, numeric], false);
    app.prepare_native_export().unwrap();
    mathfolio_document::export::export_mcdx(&app.document, path).unwrap();
    [substring, search, length, numeric]
}
fn assert_values(app: &WorksheetApp, ids: [u64; 4], origin: bool) {
    let value = |id| {
        app.results.get(&id).and_then(|row| row.value.as_ref()).unwrap_or_else(|| panic!("Missing live value id={id}, ids={:?}, status={}, error={:?}, running={}, rows={:?}", app.results.keys().collect::<Vec<_>>(), app.status, app.calculation_error, app.background.running, app.results))
    };
    assert_eq!(
        value(ids[0]).as_string().unwrap(),
        if origin { "a" } else { "b" }
    );
    assert_eq!(
        value(ids[1]).numeric_json()["real"],
        if origin { 2. } else { 1. }
    );
    assert_eq!(value(ids[2]).numeric_json()["real"], 3.);
    assert_eq!(value(ids[3]).numeric_json()["real"], 9.);
}
fn settle(app: &mut WorksheetApp) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while app.background.running {
        app.poll_background_calculation();
        assert!(
            std::time::Instant::now() < deadline,
            "worker did not settle"
        );
        std::thread::yield_now();
    }
}
fn cleanup(path: &Path) {
    for p in std::fs::read_dir(path).unwrap() {
        std::fs::remove_file(p.unwrap().path()).unwrap();
    }
    std::fs::remove_dir(path).unwrap();
}
#[test]
fn generated_native_string_origin_true_survives_cold_own_and_background_paths() {
    let out = scratch();
    run_true(&out);
    cleanup(&out);
}
fn run_true(out: &Path) {
    std::fs::create_dir_all(out).unwrap();
    let path = out.join("string-origin-settings.mcdx");
    let ids = fixture(&path);
    patch_setting(&path, "true");
    let mut app = WorksheetApp::default();
    app.open(&path);
    assert!(app.last_executed_regions.is_empty());
    app.recalculate();
    assert_values(&app, ids, true);
    app.persist_calculation_cache();
    let copy = out.join("string-origin-settings-copy.mcdx");
    app.prepare_native_export().unwrap();
    mathfolio_document::export::export_mcdx(&app.document, &copy).unwrap();
    verify_native_true(&copy, ids);
    let own = out.join("string-origin-settings.mathfolio");
    app.document.save(&own).unwrap();
    let mut cold = WorksheetApp::default();
    cold.open(&own);
    assert!(cold.last_executed_regions.is_empty());
    cold.recalculate();
    assert_values(&cold, ids, true);
    let own_native = out.join("string-origin-settings-own.mcdx");
    cold.prepare_native_export().unwrap();
    mathfolio_document::export::export_mcdx(&cold.document, &own_native).unwrap();
    verify_native_true(&own_native, ids);
    verify_native_true(&path, ids);
    let mut worker = WorksheetApp::default();
    worker.open(&path);
    assert!(worker.last_executed_regions.is_empty());
    worker.enable_background_calculation(egui::Context::default());
    worker.recalculate();
    settle(&mut worker);
    assert_values(&worker, ids, true);
    println!(
        "STRING_ORIGIN_SETTINGS_NATIVE positive_mcdx=3 settings_child=true cold_execution=0 own_reopen=true background=true"
    );
}
fn verify_native_true(path: &Path, ids: [u64; 4]) {
    let package = NativePackage::open(path).unwrap();
    assert!(mathfolio_document::native_calculation::strings_use_origin(&package).unwrap());
    let mut app = WorksheetApp::default();
    app.open(path);
    assert!(app.last_executed_regions.is_empty());
    app.recalculate();
    assert_values(&app, ids, true);
}
#[test]
#[ignore = "Write schema-valid actual string-origin settings/native/own/cold/worker probe"]
fn string_origin_settings_native_export_reopen_probe() {
    run_true(&PathBuf::from(
        std::env::var_os("MATHFOLIO_STRING_SETTINGS_OUTPUT")
            .expect("MATHFOLIO_STRING_SETTINGS_OUTPUT"),
    ));
}
#[test]
fn default_false_and_unavailable_native_option_keep_nonindex_calculations() {
    let out = scratch();
    let path = out.join("default.mcdx");
    let ids = fixture(&path);
    let mut app = WorksheetApp::default();
    app.open(&path);
    assert!(app.last_executed_regions.is_empty());
    app.recalculate();
    assert_values(&app, ids, false);
    patch_setting(&path, "invalid");
    for background in [false, true] {
        let mut app = WorksheetApp::default();
        app.open(&path);
        assert!(app.last_executed_regions.is_empty());
        if background {
            app.enable_background_calculation(egui::Context::default());
        }
        app.recalculate();
        settle(&mut app);
        for id in &ids[..2] {
            let error = app.results[id].error.as_ref().unwrap();
            assert!(error.unsupported);
            assert!(error.detail.contains("String origin setting unavailable"));
        }
        assert_eq!(
            app.results[&ids[2]].value.as_ref().unwrap().numeric_json()["real"],
            3.
        );
        assert_eq!(
            app.results[&ids[3]].value.as_ref().unwrap().numeric_json()["real"],
            9.
        );
    }
    cleanup(&out);
}
