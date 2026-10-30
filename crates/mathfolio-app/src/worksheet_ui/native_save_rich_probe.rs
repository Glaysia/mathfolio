//! Rich native Save observations; source fixture is public-export generated.
use super::*;
use crate::worksheet_ui::native_save_ui::SaveIntent;
use sha2::{Digest, Sha256};

fn copy_snapshot(output: &Path, source: &Path, name: &str) -> serde_json::Value {
    let bytes = std::fs::read(source).unwrap();
    assert!(!output.join(name).exists());
    std::fs::write(output.join(name), &bytes).unwrap();
    serde_json::json!({"name":name,"bytes":bytes.len(),"sha256":format!("{:x}",Sha256::digest(&bytes))})
}
fn observation(f: &Fixture, label: &str, generation: u64) -> serde_json::Value {
    assert!(f.app.last_executed_regions.is_empty());
    assert!(!f.app.background.running);
    assert_eq!(f.app.background.submitted_generation(), generation);
    let revision = f
        .app
        .path
        .as_deref()
        .filter(|p| {
            p.extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("mcdx"))
        })
        .map(|p| mathfolio_document::native_save::DiskRevision::read(p).unwrap());
    serde_json::json!({
        "label":label,"plain":f.text(),"xaml":f.xml(),"dirty":f.app.dirty,
        "path_name":f.app.path.as_ref().and_then(|p|p.file_name()).map(|p|p.to_string_lossy().into_owned()),
        "native_target_debug":f.app.native_save_target.as_ref().map(|t|format!("{t:?}")),
        "disk_revision_sha256":revision.map(|r|r.sha256().iter().map(|b|format!("{b:02x}")).collect::<String>()),
        "xaml_sha256":format!("{:x}",Sha256::digest(f.xml().as_bytes())),
        "loaded_archive_sha256":f.app.document.native_archive.as_deref().map(|b|format!("{:x}",Sha256::digest(b))),
        "undo":f.app.undo.len(),"redo":f.app.redo.len(),
        "submitted_generation":f.app.background.submitted_generation(),"executed_regions":0
    })
}
fn assert_cold(path: &Path, xml: &str, plain: &str) {
    let mut app = WorksheetApp {
        automatic_calculation: false,
        ..Default::default()
    };
    app.open(path);
    assert!(app.status.starts_with("Opened"), "{}", app.status);
    let region = app
        .document
        .regions
        .iter()
        .find(|r| matches!(&r.content,CanvasContent::Text(t) if t==plain))
        .unwrap();
    if path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("mcdx"))
    {
        let package = NativePackage::open(path).unwrap();
        let authority = mathfolio_document::export::read_native_text_authority(
            &package,
            region.source_region.as_deref().unwrap(),
        )
        .unwrap();
        assert_eq!(authority.original_xaml(), xml);
    } else {
        let id = region.id;
        app.selected = Some(id);
        app.prepare_rich_text();
        assert_eq!(
            app.rich_text.editors[&id].as_ref().unwrap().source.xaml(),
            xml
        );
    }
    assert!(app.last_executed_regions.is_empty());
    assert!(!app.background.running);
    assert!(app.results.is_empty() && app.plot_results.is_empty());
}

pub(in crate::worksheet_ui) fn capture(
    output: &Path,
) -> (Vec<serde_json::Value>, Vec<serde_json::Value>) {
    let mut f = Fixture::new();
    let original = f.app.path.clone().unwrap();
    let baseline = f.app.document.native_archive.clone();
    let generation = f.app.background.submitted_generation();
    let mut files = vec![];
    let mut observations = vec![observation(&f, "rich-open", generation)];
    f.command(|selection_key| RibbonAction::SetTextItalic {
        value: true,
        selection_key,
    });
    f.select(2, 2);
    f.frame(vec![key(egui::Key::Enter, egui::Modifiers::NONE)]);
    f.frame(vec![egui::Event::Text("한".into())]);
    let xml = f.xml();
    let plain = f.text().to_owned();
    let history = f.app.undo.len();
    assert!(f.app.save(false), "{}", f.app.status);
    assert_eq!(f.app.undo.len(), history);
    assert_cold(&original, &xml, &plain);
    files.push(copy_snapshot(
        output,
        &original,
        "native-save-rich-edited.mcdx",
    ));
    observations.push(observation(&f, "rich-format-content-paragraph", generation));
    f.app.undo();
    f.app.prepare_rich_text();
    let undo_xml = f.xml();
    let undo_plain = f.text().to_owned();
    assert_ne!(undo_plain, plain);
    assert!(f.app.save(false), "{}", f.app.status);
    assert_cold(&original, &undo_xml, &undo_plain);
    files.push(copy_snapshot(
        output,
        &original,
        "native-save-rich-undo.mcdx",
    ));
    observations.push(observation(&f, "rich-undo-save", generation));
    f.app.redo();
    f.app.prepare_rich_text();
    assert_eq!(f.xml(), xml);
    assert!(f.app.save(false), "{}", f.app.status);
    assert_cold(&original, &xml, &plain);
    files.push(copy_snapshot(
        output,
        &original,
        "native-save-rich-redo.mcdx",
    ));
    observations.push(observation(&f, "rich-redo-save", generation));
    assert_eq!(f.app.document.native_archive, baseline);
    let own = f.root.join("native-save-rich-own.mathfolio");
    assert!(
        f.app
            .save_to_destination(Some(own.clone()), SaveIntent::SaveAs),
        "{}",
        f.app.status
    );
    assert_cold(&own, &xml, &plain);
    files.push(copy_snapshot(
        output,
        &own,
        "native-save-rich-own.mathfolio",
    ));
    observations.push(observation(&f, "rich-own-roundtrip", generation));
    (files, observations)
}
