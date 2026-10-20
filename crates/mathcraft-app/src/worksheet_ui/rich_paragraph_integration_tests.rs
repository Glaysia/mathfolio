//! ARTIFACT ONLY. Child of rich_text_integration_tests, not worksheet_ui itself.
//! Reuses its private Fixture/frame/select/save and native ZIP/source helpers.
#![allow(deprecated)]
use super::*;
use sha2::{Digest, Sha256};
use std::io::Read;

const XAML: &str = "http://schemas.microsoft.com/winfx/2006/xaml/presentation";
const SINGLE: &str = "LCCL-S 시스템 설계";
const DOUBLE: &str = "LCCL-S\n시스템 설계";

fn normal_source(paragraphs: &[&str]) -> String {
    let mut source = format!(
        "<Section xmlns=\"{XAML}\" xml:space=\"preserve\" FontSize=\"16\" FontWeight=\"Normal\" FontStyle=\"Normal\" Foreground=\"#FF112233\">"
    );
    for text in paragraphs {
        source.push_str(&format!("<Paragraph NumberSubstitution.CultureSource=\"User\"><Run style=\"Normal\" xml:lang=\"ko-kr\" Background=\"#FF31ADC2\" FontWeight=\"Normal\" FontStyle=\"Normal\">{text}</Run></Paragraph>"));
    }
    source.push_str("</Section>");
    source
}

fn fixture_source(source: &str, expected_plain: &str) -> Fixture {
    // First create the actual public exporter wrapper/relationships. The parent
    // helper deliberately checks ORIGINAL, so replace its nested part afterward
    // for a two-original-Paragraph fixture with a different plain projection.
    let root = temporary();
    let original = native_fixture_with(&root, SOURCE_PACKAGE);
    let nested = replaced_package_xaml(source);
    let mut archive = ZipArchive::new(Cursor::new(std::fs::read(original).unwrap())).unwrap();
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    writer
        .set_raw_comment(archive.comment().to_vec().into_boxed_slice())
        .unwrap();
    let mut replaced = 0;
    for index in 0..archive.len() {
        let entry = archive.by_index(index).unwrap();
        let name = entry.name().unwrap().into_owned();
        if name.ends_with(".XamlPackage") {
            replaced += 1;
            writer.start_file(name, entry.options()).unwrap();
            writer.write_all(&nested).unwrap();
        } else {
            writer.raw_copy_file(entry).unwrap();
        }
    }
    assert_eq!(replaced, 1);
    let path = root.join("paragraph-source.mcdx");
    std::fs::write(&path, writer.finish().unwrap().into_inner()).unwrap();
    fixture_open_at(&path, root, expected_plain)
}

fn fixture_open_at(path: &Path, root: PathBuf, expected_plain: &str) -> Fixture {
    let mut app = WorksheetApp {
        automatic_calculation: false,
        ..Default::default()
    };
    app.open(path);
    assert!(app.status.starts_with("Opened"), "{}", app.status);
    let id = app
        .document
        .regions
        .iter()
        .find(|r| matches!(r.content, CanvasContent::Text(_)))
        .unwrap()
        .id;
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    app.selected = Some(id);
    app.focus_new = Some(id);
    app.insertion = None;
    let mut f = Fixture {
        app,
        ctx,
        id,
        root,
        frame: 0,
    };
    f.frame(vec![]);
    assert_eq!(f.text(), expected_plain);
    assert!(f.app.last_executed_regions.is_empty());
    f
}

fn cold(path: &Path, expected: &str) -> Fixture {
    fixture_open_at(path, temporary(), expected)
}

fn paragraphs(xml: &str) -> Vec<String> {
    let doc = roxmltree::Document::parse(xml).unwrap();
    doc.root_element()
        .children()
        .filter(|node| node.has_tag_name((XAML, "Paragraph")))
        .map(|p| {
            p.children()
                .filter(|n| n.has_tag_name((XAML, "Run")))
                .flat_map(|run| {
                    run.children()
                        .filter(|n| n.is_text())
                        .map(|n| n.text().unwrap_or(""))
                })
                .collect()
        })
        .collect()
}

fn assert_source(xml: &str, plain: &str, count: usize) {
    let parts = paragraphs(xml);
    assert_eq!(parts.len(), count, "must be real Paragraph elements");
    assert_eq!(parts.join("\n"), plain);
    let doc = roxmltree::Document::parse(xml).unwrap();
    for run in doc.descendants().filter(|n| n.has_tag_name((XAML, "Run"))) {
        assert_eq!(run.attribute("style"), Some("Normal"));
        assert_eq!(run.attribute("FontWeight"), Some("Normal"));
        assert_eq!(run.attribute("FontStyle"), Some("Normal"));
        assert_eq!(run.attribute("Background"), Some("#FF31ADC2"));
        assert_eq!(
            run.attribute(("http://www.w3.org/XML/1998/namespace", "lang")),
            Some("ko-kr")
        );
        for text in run.children().filter(|n| n.is_text()) {
            assert!(
                !text.text().unwrap_or("").contains(['\n', '\r']),
                "Run literal newline is not a Paragraph"
            );
        }
    }
    for p in doc
        .root_element()
        .children()
        .filter(|p| p.has_tag_name((XAML, "Paragraph")))
    {
        assert_eq!(
            p.attribute("NumberSubstitution.CultureSource"),
            Some("User")
        );
    }
}

fn no_calculation(f: &Fixture, generation: u64) {
    assert!(f.app.last_executed_regions.is_empty());
    assert!(!f.app.background.running);
    assert_eq!(f.app.background.submitted_generation(), generation);
    assert!(f.app.results.is_empty() && f.app.plot_results.is_empty());
}

fn state(f: &Fixture, label: &str) -> serde_json::Value {
    serde_json::json!({"label":label,"plain":f.text(),"xaml":f.xml(),
        "paragraphs":paragraphs(&f.xml()),"undo":f.app.undo.len(),"redo":f.app.redo.len(),
        "generation":f.app.background.submitted_generation(),"executed":f.app.last_executed_regions.len()})
}

fn enter(f: &mut Fixture) {
    f.frame(vec![key(egui::Key::Enter, egui::Modifiers::NONE)]);
}

fn repeated_history(f: &mut Fixture) -> Vec<serde_json::Value> {
    let generation = f.app.background.submitted_generation();
    let history = f.app.undo.len();
    f.select(3, 3);
    enter(f);
    assert_eq!(f.text(), "LCC\nL-S 시스템 설계");
    assert_eq!(f.app.undo.len(), history + 1);
    f.frame(vec![egui::Event::Text("Q".into())]);
    assert_eq!(f.text(), "LCC\nQL-S 시스템 설계");
    assert_eq!(f.app.undo.len(), history + 2);
    let two = f.xml();
    f.select(6, 6);
    enter(f);
    assert_eq!(f.text(), "LCC\nQL\n-S 시스템 설계");
    let three = f.xml();
    assert_eq!(f.app.undo.len(), history + 3);
    f.frame(vec![egui::Event::Text("R".into())]);
    assert_eq!(f.text(), "LCC\nQL\nR-S 시스템 설계");
    assert_eq!(f.app.undo.len(), history + 4);
    let typed = f.xml();
    f.app.undo();
    f.app.prepare_rich_text();
    assert_eq!(f.xml(), three);
    f.app.undo();
    f.app.prepare_rich_text();
    assert_eq!(f.xml(), two);
    f.app.redo();
    f.app.prepare_rich_text();
    assert_eq!(f.xml(), three);
    f.app.redo();
    f.app.prepare_rich_text();
    assert_eq!(f.xml(), typed);
    // No manual reselection: restored actual caret + source-owned affinity must
    // allow the next key in the following Run, not the preceding Paragraph.
    f.frame(vec![]);
    f.frame(vec![egui::Event::Text("T".into())]);
    assert_eq!(f.text(), "LCC\nQL\nRT-S 시스템 설계");
    assert_eq!(f.app.undo.len(), history + 5);
    assert_source(&f.xml(), f.text(), 3);
    no_calculation(f, generation);
    vec![state(f, "repeated-split-undo-redo-typing")]
}

fn split_last_run(f: &mut Fixture) -> String {
    let caret = {
        let source = &f.app.rich_text.editors[&f.id].as_ref().unwrap().source;
        let run = source.run_char_ranges().last().unwrap();
        assert!(run.global_end > run.global_start + 1);
        run.global_start + 1
    };
    let expected: String = f
        .text()
        .chars()
        .take(caret)
        .chain(std::iter::once('\n'))
        .chain(f.text().chars().skip(caret))
        .collect();
    let history = f.app.undo.len();
    let generation = f.app.background.submitted_generation();
    f.select(caret, caret);
    enter(f);
    assert_eq!(f.text(), expected);
    assert_eq!(f.app.undo.len(), history + 1);
    no_calculation(f, generation);
    expected
}

fn export(f: &mut Fixture, name: &str) -> PathBuf {
    let path = f.root.join(name);
    f.app.prepare_native_export().unwrap();
    mathcraft_document::export::export_mcdx(&f.app.document, &path).unwrap();
    path
}

fn assert_native(path: &Path, xml: &str, plain: &str, count: usize) {
    let package = NativePackage::open(path).unwrap();
    let owner = package
        .regions()
        .unwrap()
        .into_iter()
        .find(|r| r.raw_xml.contains("item-idref"))
        .unwrap()
        .region_id
        .unwrap();
    let authority =
        mathcraft_document::export::read_native_text_authority(&package, &owner).unwrap();
    assert_eq!(authority.original_xaml(), xml);
    assert_eq!(authority.original_plain_text(), plain);
    assert_eq!(authority.original_catalog(), CATALOG);
    assert_source(authority.original_xaml(), plain, count);
    let mut original = ZipArchive::new(Cursor::new(SOURCE_PACKAGE)).unwrap();
    let mut current = ZipArchive::new(Cursor::new(authority.original_package())).unwrap();
    assert_eq!(original.len(), current.len());
    for i in 0..original.len() {
        let mut entry = original.by_index(i).unwrap();
        let name = entry.name().unwrap().into_owned();
        if name == "Xaml/Document.xaml" {
            continue;
        }
        let mut before = vec![];
        entry.read_to_end(&mut before).unwrap();
        let mut after = vec![];
        current
            .by_name(&name)
            .unwrap()
            .read_to_end(&mut after)
            .unwrap();
        assert_eq!(before, after, "preserved nested resource {name}");
    }
}

fn copy_file(output: Option<&Path>, source: &Path, name: &str, files: &mut Vec<serde_json::Value>) {
    let bytes = std::fs::read(source).unwrap();
    if let Some(output) = output {
        std::fs::write(output.join(name), &bytes).unwrap();
    }
    files.push(serde_json::json!({"name":name,"bytes":bytes.len(),"sha256":format!("{:x}",Sha256::digest(&bytes))}));
}

fn roundtrip(output: Option<&Path>) -> (Vec<serde_json::Value>, Vec<serde_json::Value>) {
    let mut files = vec![];
    let mut f = fixture_source(&normal_source(&[SINGLE]), SINGLE);
    // Native import intentionally has no own-format save path. Keep the
    // explicit fixture input rather than changing app path/save semantics.
    let original = f.root.join("paragraph-source.mcdx");
    copy_file(output, &original, "paragraph-original.mcdx", &mut files);
    let mut observations = repeated_history(&mut f);
    let edited = f.text().to_owned();
    let xml = f.xml();
    let own = f.save("paragraph-edited.mathcraft");
    copy_file(output, &own, "paragraph-edited.mathcraft", &mut files);
    let native = export(&mut f, "paragraph-edited.mcdx");
    assert_native(&native, &xml, &edited, 3);
    copy_file(output, &native, "paragraph-edited.mcdx", &mut files);
    let mut own_cold = cold(&own, &edited);
    assert_eq!(own_cold.xml(), xml);
    split_last_run(&mut own_cold);
    own_cold.frame(vec![egui::Event::Text("O".into())]);
    assert_eq!(own_cold.text(), "LCC\nQL\nR\nOT-S 시스템 설계");
    assert_eq!(own_cold.app.undo.len(), 2);
    assert_source(&own_cold.xml(), own_cold.text(), 4);
    observations.push(state(&own_cold, "own-cold-additional-split"));
    let own_more = own_cold.save("paragraph-own-resplit.mathcraft");
    copy_file(
        output,
        &own_more,
        "paragraph-own-resplit.mathcraft",
        &mut files,
    );
    let mut native_cold = cold(&native, &edited);
    assert_eq!(native_cold.xml(), xml);
    split_last_run(&mut native_cold);
    native_cold.frame(vec![egui::Event::Text("N".into())]);
    assert_eq!(native_cold.text(), "LCC\nQL\nR\nNT-S 시스템 설계");
    assert_eq!(native_cold.app.undo.len(), 2);
    assert_source(&native_cold.xml(), native_cold.text(), 4);
    observations.push(state(&native_cold, "native-cold-additional-split"));
    let native_own = native_cold.save("paragraph-native-resplit.mathcraft");
    copy_file(
        output,
        &native_own,
        "paragraph-native-resplit.mathcraft",
        &mut files,
    );
    let native_more = export(&mut native_cold, "paragraph-native-resplit.mcdx");
    assert_native(&native_more, &native_cold.xml(), native_cold.text(), 4);
    copy_file(
        output,
        &native_more,
        "paragraph-native-resplit.mcdx",
        &mut files,
    );
    let mut reimported = cold(&native_more, native_cold.text());
    assert_eq!(reimported.xml(), native_cold.xml());
    let final_own = reimported.save("paragraph-reimported.mathcraft");
    copy_file(
        output,
        &final_own,
        "paragraph-reimported.mathcraft",
        &mut files,
    );
    observations.push(state(&reimported, "native-reimported"));
    let mut two = fixture_source(&normal_source(&["LCCL-S", "시스템 설계"]), DOUBLE);
    let source = two.root.join("paragraph-source.mcdx");
    copy_file(output, &source, "paragraph-two-original.mcdx", &mut files);
    let generation = two.app.background.submitted_generation();
    let history = two.app.undo.len();
    two.select(2, 2);
    enter(&mut two);
    assert_eq!(two.text(), "LC\nCL-S\n시스템 설계");
    split_last_run(&mut two);
    two.frame(vec![egui::Event::Text("W".into())]);
    assert_eq!(two.text(), "LC\nCL-S\n시\nW스템 설계");
    assert_eq!(two.app.undo.len(), history + 3);
    no_calculation(&two, generation);
    assert_source(&two.xml(), two.text(), 4);
    let two_own = two.save("paragraph-two-edited.mathcraft");
    copy_file(
        output,
        &two_own,
        "paragraph-two-edited.mathcraft",
        &mut files,
    );
    let two_native = export(&mut two, "paragraph-two-edited.mcdx");
    assert_native(&two_native, &two.xml(), two.text(), 4);
    copy_file(output, &two_native, "paragraph-two-edited.mcdx", &mut files);
    observations.push(state(&two, "two-original-paragraphs"));
    assert_eq!(files.len(), 10);
    (files, observations)
}

#[test]
fn repeated_real_enter_and_following_run_typing_survive_history_own_native_and_original_two_paragraphs()
 {
    roundtrip(None);
}

fn ime_and_ordering() -> Vec<serde_json::Value> {
    let mut records = vec![];
    for batch_commit in [true, false] {
        let mut f = fixture_source(&normal_source(&[SINGLE]), SINGLE);
        let generation = f.app.background.submitted_generation();
        let history = f.app.undo.len();
        f.select(3, 3);
        if batch_commit {
            f.frame(vec![
                egui::Event::Ime(egui::ImeEvent::Enabled),
                preedit("한"),
                egui::Event::Ime(egui::ImeEvent::Commit("한".into())),
                key(egui::Key::Enter, egui::Modifiers::NONE),
            ]);
        } else {
            f.frame(vec![
                egui::Event::Ime(egui::ImeEvent::Enabled),
                preedit("한"),
            ]);
            assert_eq!(f.text(), SINGLE);
            assert_eq!(f.app.undo.len(), history);
            enter(&mut f);
            assert_eq!(f.text(), SINGLE);
            assert_eq!(f.app.undo.len(), history);
            f.frame(vec![egui::Event::Ime(egui::ImeEvent::Commit("한".into()))]);
        }
        assert_eq!(f.text(), "LCC한L-S 시스템 설계");
        assert_eq!(f.app.undo.len(), history + 1);
        assert_source(&f.xml(), f.text(), 1);
        records.push(state(
            &f,
            if batch_commit {
                "ime-commit-enter-same-native-batch"
            } else {
                "preedit-bare-enter-then-commit"
            },
        ));
        enter(&mut f);
        assert_eq!(f.text(), "LCC한\nL-S 시스템 설계");
        assert_eq!(f.app.undo.len(), history + 2);
        f.frame(vec![egui::Event::Text("Z".into())]);
        assert_eq!(f.text(), "LCC한\nZL-S 시스템 설계");
        assert_eq!(f.app.undo.len(), history + 3);
        assert_source(&f.xml(), f.text(), 2);
        no_calculation(&f, generation);
        records.push(state(&f, "later-separate-enter-then-typing"));
    }
    let mut f = fixture_source(&normal_source(&[SINGLE]), SINGLE);
    let generation = f.app.background.submitted_generation();
    let history = f.app.undo.len();
    f.select(3, 3);
    f.frame(vec![
        egui::Event::Text("X".into()),
        key(egui::Key::Enter, egui::Modifiers::NONE),
        egui::Event::Text("Y".into()),
    ]);
    assert_eq!(f.text(), "LCCX\nYL-S 시스템 설계");
    assert_eq!(f.app.undo.len(), history + 3);
    assert_source(&f.xml(), f.text(), 2);
    no_calculation(&f, generation);
    records.push(state(&f, "text-prefix-enter-text-suffix-raw-batch"));
    records
}

#[test]
fn ime_confirmation_and_raw_text_enter_text_keep_distinct_native_batch_ownership() {
    ime_and_ordering();
}

struct Atomic {
    document: Vec<u8>,
    xml: Option<String>,
    cursor: Option<egui::text::CCursorRange>,
    undo: usize,
    redo: usize,
    generation: u64,
}
impl Atomic {
    fn new(f: &Fixture) -> Self {
        Self {
            document: serde_json::to_vec(&f.app.document).unwrap(),
            xml: f
                .app
                .rich_text
                .editors
                .get(&f.id)
                .and_then(|e| e.as_ref().ok())
                .map(|e| e.source.xaml().to_owned()),
            cursor: f.app.text_cursors.get(&f.id).copied(),
            undo: f.app.undo.len(),
            redo: f.app.redo.len(),
            generation: f.app.background.submitted_generation(),
        }
    }
    fn check(&self, f: &Fixture) {
        assert_eq!(serde_json::to_vec(&f.app.document).unwrap(), self.document);
        assert_eq!(
            f.app
                .rich_text
                .editors
                .get(&f.id)
                .and_then(|e| e.as_ref().ok())
                .map(|e| e.source.xaml().to_owned()),
            self.xml
        );
        assert_eq!(f.app.text_cursors.get(&f.id).copied(), self.cursor);
        assert_eq!(f.app.undo.len(), self.undo);
        assert_eq!(f.app.redo.len(), self.redo);
        no_calculation(f, self.generation);
    }
}

#[test]
fn rejected_selection_edges_run_boundary_shift_enter_paste_and_cross_paragraph_delete_are_atomic() {
    for (anchor, focus) in [(1, 4), (0, 0), (13, 13)] {
        let mut f = fixture_source(&normal_source(&[SINGLE]), SINGLE);
        f.select(anchor, focus);
        let old = Atomic::new(&f);
        enter(&mut f);
        old.check(&f);
    }
    let source = normal_source(&[SINGLE]).replace("LCCL-S", "LCC</Run><Run style=\"Normal\" xml:lang=\"ko-kr\" Background=\"#FF31ADC2\" FontWeight=\"Normal\" FontStyle=\"Normal\">L-S");
    let mut boundary = fixture_source(&source, SINGLE);
    boundary.select(3, 3);
    let old = Atomic::new(&boundary);
    enter(&mut boundary);
    old.check(&boundary);
    for event in [
        key(egui::Key::Enter, egui::Modifiers::SHIFT),
        egui::Event::Paste("x\ny".into()),
    ] {
        let mut f = fixture_source(&normal_source(&[SINGLE]), SINGLE);
        f.select(3, 3);
        let old = Atomic::new(&f);
        f.frame(vec![event]);
        old.check(&f);
    }
    let mut f = fixture_source(&normal_source(&[SINGLE]), SINGLE);
    f.select(3, 3);
    enter(&mut f);
    f.select(2, 5);
    let old = Atomic::new(&f);
    f.frame(vec![key(egui::Key::Backspace, egui::Modifiers::NONE)]);
    old.check(&f);
    f.select(3, 3);
    let old = Atomic::new(&f);
    f.frame(vec![key(egui::Key::Delete, egui::Modifiers::NONE)]);
    old.check(&f);
}

#[test]
fn global_eight_mib_failure_preserves_editor_and_next_enter_recovers_when_budget_is_available() {
    let mut f = fixture_source(&normal_source(&[SINGLE]), SINGLE);
    let mut remaining = 8 * 1024 * 1024 - f.app.document.source_text_bytes().unwrap();
    let mut last = 0;
    while remaining != 0 {
        let bytes = remaining.min(1024 * 1024);
        last = f.app.document.add_text(
            0.,
            100_000. + f.app.document.regions.len() as f64 * 50.,
            "x".repeat(bytes),
        );
        remaining -= bytes;
    }
    assert_eq!(f.app.document.source_text_bytes().unwrap(), 8 * 1024 * 1024);
    f.select(3, 3);
    let old = Atomic::new(&f);
    enter(&mut f);
    old.check(&f);
    let CanvasContent::Text(padding) = &mut f
        .app
        .document
        .regions
        .iter_mut()
        .find(|r| r.id == last)
        .unwrap()
        .content
    else {
        panic!()
    };
    assert!(padding.len() > 4096);
    padding.truncate(padding.len() - 4096);
    let history = f.app.undo.len();
    enter(&mut f);
    assert_eq!(f.text(), "LCC\nL-S 시스템 설계");
    assert_eq!(f.app.undo.len(), history + 1);
    assert_source(&f.xml(), f.text(), 2);
}

#[test]
fn readonly_line_height_geometry_keeps_centered_source_on_enter() {
    let source = normal_source(&[SINGLE]).replace(
        "<Paragraph ",
        "<Paragraph TextAlignment=\"Center\" LineHeight=\"40\" ",
    );
    let mut f = fixture_source(&source, SINGLE);
    assert!(f.app.rich_text.editors[&f.id].is_err());
    let old = Atomic::new(&f);
    enter(&mut f);
    old.check(&f);
    let region = f
        .app
        .document
        .regions
        .iter()
        .find(|r| r.id == f.id)
        .unwrap();
    let visual = f
        .app
        .native_visuals
        .region(region)
        .unwrap()
        .as_ref()
        .unwrap()
        .document
        .as_ref()
        .unwrap();
    let mathcraft_document::native_visual::Block::Paragraph(p) = &visual.blocks[0] else {
        panic!()
    };
    assert_eq!(
        p.style.alignment,
        mathcraft_document::native_visual::Alignment::Center
    );
    assert_eq!(visual.imported_plain_text, SINGLE);
    assert_eq!(p.line_height, Some(40.));
}

#[test]
#[ignore = "writes five native/five own paragraph fixtures to a new explicit directory"]
fn rich_paragraph_source_own_native_reopen_probe() {
    let output = PathBuf::from(
        std::env::var_os("MATHCRAFT_PARAGRAPH_OUTPUT").expect("Set a new output directory"),
    );
    assert!(output.is_absolute());
    assert!(!output.exists(), "never overwrite evidence");
    std::fs::create_dir_all(&output).unwrap();
    let (files, mut observations) = roundtrip(Some(&output));
    observations.extend(ime_and_ordering());
    assert_eq!(
        files
            .iter()
            .filter(|file| file["name"].as_str().unwrap().ends_with(".mcdx"))
            .count(),
        5
    );
    assert_eq!(
        files
            .iter()
            .filter(|file| file["name"].as_str().unwrap().ends_with(".mathcraft"))
            .count(),
        5
    );
    let report = serde_json::json!({"original_runtime":false,"gui":false,"calculation_executions":0,
        "native_files":5,"own_files":5,"actual_paragraph_xml":true,"run_literal_newlines":false,
        "styles_catalog_resources_preserved":true,"history_undo_redo_following_run_typing":true,
        "files":files,"observations":observations});
    std::fs::write(
        output.join("observation.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
    println!(
        "RICH_PARAGRAPH native_files=5 own_files=5 original_runtime=false gui=false calculations=0"
    );
}
