//! Native Save child of worksheet_ui::rich_text_integration_tests.
//! Reuses its generated/public-export+ZIP-rebuild fixture; no user archive writes.
use super::*;
use crate::worksheet_ui::native_save_ui::SaveIntent;

fn assert_saved_source(path: &Path, xml: &str, plain: &str) {
    let mut cold = WorksheetApp {
        automatic_calculation: false,
        ..Default::default()
    };
    cold.open(path);
    assert!(cold.status.starts_with("Opened"), "{}", cold.status);
    let region = cold
        .document
        .regions
        .iter()
        .find(|region| matches!(&region.content,CanvasContent::Text(text) if text==plain))
        .expect("Exact paragraph/Run plain source");
    let package = NativePackage::open(path).unwrap();
    let authority = mathcraft_document::export::read_native_text_authority(
        &package,
        region.source_region.as_deref().unwrap(),
    )
    .unwrap();
    assert_eq!(authority.original_xaml(), xml);
    assert!(cold.last_executed_regions.is_empty());
    assert!(cold.results.is_empty() && cold.plot_results.is_empty());
    assert!(!cold.background.running);
}

#[test]
fn rich_native_same_path_repeated_save_format_content_paragraph_undo_keeps_loaded_anchor() {
    let mut f = Fixture::new();
    let path = f
        .app
        .path
        .clone()
        .expect("Native open now retains file identity");
    let baseline = f.app.document.native_archive.clone();
    let generation = f.app.background.submitted_generation();
    f.command(|selection_key| RibbonAction::SetTextItalic {
        value: true,
        selection_key,
    });
    f.select(2, 2); // Interior of selected/formatted CCL Run, not a Run edge.
    f.frame(vec![key(egui::Key::Enter, egui::Modifiers::NONE)]);
    assert!(f.text().contains('\n'));
    f.frame(vec![egui::Event::Text("한".into())]);
    let xml = f.xml();
    let plain = f.text().to_owned();
    let history = f.app.undo.len();
    assert!(f.app.save(false), "{}", f.app.status);
    assert_eq!(f.app.undo.len(), history);
    assert_saved_source(&path, &xml, &plain);
    assert_eq!(f.app.document.native_archive, baseline);
    f.app.undo();
    f.app.prepare_rich_text();
    let undo_xml = f.xml();
    let undo_plain = f.text().to_owned();
    assert_ne!(undo_plain, plain);
    assert!(f.app.save(false), "{}", f.app.status);
    assert_saved_source(&path, &undo_xml, &undo_plain);
    f.app.redo();
    f.app.prepare_rich_text();
    assert_eq!(f.xml(), xml);
    assert!(f.app.save(false), "{}", f.app.status);
    assert_saved_source(&path, &xml, &plain);
    assert_eq!(f.app.document.native_archive, baseline);
    assert_eq!(f.app.background.submitted_generation(), generation);
    assert!(f.app.last_executed_regions.is_empty());
}

#[test]
fn rich_native_save_as_then_own_open_and_native_save_as_does_not_forge_new_authority() {
    let mut f = Fixture::new();
    let original = f.app.path.clone().unwrap();
    let original_bytes = std::fs::read(&original).unwrap();
    f.command(|selection_key| RibbonAction::SetTextFontSize {
        points: 18.,
        selection_key,
    });
    let first_xml = f.xml();
    let native_copy = f.root.join("native-save-as.mcdx");
    assert!(
        f.app
            .save_to_destination(Some(native_copy.clone()), SaveIntent::SaveAs),
        "{}",
        f.app.status
    );
    assert_saved_source(&native_copy, &first_xml, f.text());
    assert_eq!(std::fs::read(&original).unwrap(), original_bytes);
    let own = f.root.join("own-after-native.mathcraft");
    f.app.path = Some(own.clone());
    assert!(f.app.save(false), "{}", f.app.status);
    assert!(f.app.native_save_target.is_none());
    f.app.open(&own);
    assert!(f.app.native_save_target.is_none());
    f.app.selected = Some(f.id);
    f.app.focus_new = Some(f.id);
    f.frame(vec![]);
    f.select(1, 4);
    f.command(|selection_key| RibbonAction::SetTextColor {
        rgba: [10, 20, 30, 255],
        selection_key,
    });
    let current_xml = f.xml();
    let return_native = f.root.join("own-back-to-native.mcdx");
    assert!(
        f.app
            .save_to_destination(Some(return_native.clone()), SaveIntent::SaveAs),
        "{}",
        f.app.status
    );
    assert_saved_source(&return_native, &current_xml, f.text());
    assert_eq!(std::fs::read(&original).unwrap(), original_bytes);
}
