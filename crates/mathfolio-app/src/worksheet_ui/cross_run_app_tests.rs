//! Actual app input; no installed runtime, window or graphics device.
use super::*;

const ORIGINAL: &str = "가A&😀BC한DE꼬";
const DELETED: &str = "가AE꼬";

fn formatted() -> Fixture {
    let mut f = Fixture::new();
    f.frame(vec![egui::Event::Text(ORIGINAL.into())]);
    f.select(4, 7);
    let selection_key = f.app.rich_text_context().unwrap().selection_key;
    f.app.ribbon_command(
        &f.ctx,
        RibbonAction::SetTextBold {
            value: true,
            selection_key,
        },
    );
    assert_eq!(f.xml().matches("<Run ").count(), 3);
    f
}

#[test]
fn selected_delete_and_backspace_cross_styles_preserve_metadata_history_and_generation() {
    for key_kind in [egui::Key::Delete, egui::Key::Backspace] {
        for (anchor, focus) in [(2, 8), (8, 2)] {
            let mut f = formatted();
            f.select(anchor, focus);
            let original = f.xml();
            let expected = original
                .replace("가A&amp;😀", "가A")
                .replace(">BC한</Run>", "></Run>")
                .replace(">DE꼬</Run>", ">E꼬</Run>");
            let history = f.app.undo.len();
            let generation = f.app.background.submitted_generation();
            f.frame(vec![key(key_kind, egui::Modifiers::NONE)]);
            assert_eq!(f.text(), DELETED, "{}", f.app.status);
            assert_eq!(f.xml(), expected);
            assert_eq!(f.app.undo.len(), history + 1);
            assert_eq!(
                f.app.rich_text.authored_editors[&f.id]
                    .as_ref()
                    .unwrap()
                    .affinity
                    .owner(&f.region().validated_authored_text().unwrap().unwrap()),
                (key_kind == egui::Key::Backspace).then_some(0)
            );
            assert_eq!(f.app.background.submitted_generation(), generation);
            assert!(f.app.last_executed_regions.is_empty());
            assert!(!f.app.background.running);
            f.app.undo();
            assert_eq!(f.xml(), original);
            assert_eq!(f.text(), ORIGINAL);
            f.app.redo();
            assert_eq!(f.xml(), expected);
            assert_eq!(f.text(), DELETED);
        }
    }
}

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "mathfolio-cross-run-app-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        for name in ["edited.mathfolio", "edited.mcdx"] {
            let _ = std::fs::remove_file(self.0.join(name));
        }
        let _ = std::fs::remove_dir(&self.0);
    }
}

#[test]
fn actual_selected_deletion_own_and_native_cold_reopen_keep_all_run_tags() {
    use native_save_ui::SaveIntent;
    let mut f = formatted();
    f.select(8, 2);
    f.frame(vec![key(egui::Key::Delete, egui::Modifiers::NONE)]);
    assert_eq!(f.text(), DELETED, "{}", f.app.status);
    let xml = f.xml();
    let scratch = Scratch::new();
    for name in ["edited.mathfolio", "edited.mcdx"] {
        let path = scratch.0.join(name);
        assert!(
            f.app
                .save_to_destination(Some(path.clone()), SaveIntent::SaveAs),
            "{}",
            f.app.status
        );
        let mut cold = WorksheetApp {
            automatic_calculation: false,
            ..Default::default()
        };
        cold.open(&path);
        assert!(cold.status.starts_with("Opened"), "{}", cold.status);
        let region = cold
            .document
            .regions
            .iter()
            .find(|region| matches!(&region.content, CanvasContent::Text(text) if text==DELETED))
            .unwrap();
        let id = region.id;
        if let Some(source) = region.validated_authored_text().unwrap() {
            assert_eq!(source.xaml(), xml);
        } else {
            let package = NativePackage::open(&path).unwrap();
            let authority = mathfolio_document::export::read_native_text_authority(
                &package,
                region.source_region.as_deref().unwrap(),
            )
            .unwrap();
            assert_eq!(authority.original_xaml(), xml);
        }
        cold.selected = Some(id);
        cold.prepare_rich_text();
        assert!(
            cold.rich_text.format_editor(id).is_some(),
            "{}",
            cold.status
        );
        assert!(cold.last_executed_regions.is_empty());
        assert!(!cold.background.running);
    }
}

#[test]
fn actual_select_all_delete_and_backspace_preserve_empty_runs_history_and_cold_source() {
    use native_save_ui::SaveIntent;
    for key_kind in [egui::Key::Delete, egui::Key::Backspace] {
        let mut f = formatted();
        f.select(2, 2);
        let original = f.xml();
        let expected = original
            .replace("가A&amp;😀", "")
            .replace(">BC한</Run>", "></Run>")
            .replace(">DE꼬</Run>", "></Run>");
        let generation = f.app.background.submitted_generation();
        let history = f.app.undo.len();
        f.frame(vec![key(egui::Key::A, egui::Modifiers::COMMAND)]);
        let selected = f.app.text_cursors[&f.id];
        assert_eq!(selected.primary.index.0.min(selected.secondary.index.0), 0);
        assert_eq!(selected.primary.index.0.max(selected.secondary.index.0), 10);
        assert_eq!(f.xml(), original);
        assert_eq!(f.app.undo.len(), history);
        f.frame(vec![key(key_kind, egui::Modifiers::NONE)]);
        assert_eq!(f.text(), "", "{}", f.app.status);
        assert_eq!(f.xml(), expected);
        assert_eq!(f.xml().matches("<Run ").count(), 3);
        assert_eq!(f.app.undo.len(), history + 1);
        let source = f.region().validated_authored_text().unwrap().unwrap();
        let editor = f.app.rich_text.authored_editors[&f.id].as_ref().unwrap();
        assert_eq!(
            editor.affinity.owner(&source),
            if key_kind == egui::Key::Backspace {
                Some(0)
            } else {
                None
            }
        );
        assert!(f.app.rich_text.format_editor(f.id).is_some());
        f.app.undo();
        assert_eq!(f.text(), ORIGINAL);
        assert_eq!(f.xml(), original);
        f.app.redo();
        assert_eq!(f.text(), "");
        assert_eq!(f.xml(), expected);

        let scratch = Scratch::new();
        for name in ["edited.mathfolio", "edited.mcdx"] {
            let path = scratch.0.join(name);
            assert!(
                f.app
                    .save_to_destination(Some(path.clone()), SaveIntent::SaveAs),
                "{}",
                f.app.status
            );
            let mut cold = WorksheetApp {
                automatic_calculation: false,
                ..Default::default()
            };
            cold.open(&path);
            assert!(cold.status.starts_with("Opened"), "{}", cold.status);
            let region = cold
                .document
                .regions
                .iter()
                .find(|r| matches!(&r.content, CanvasContent::Text(t) if t.is_empty()))
                .unwrap();
            let id = region.id;
            if let Some(source) = region.validated_authored_text().unwrap() {
                assert_eq!(source.xaml(), expected);
            } else {
                let package = NativePackage::open(&path).unwrap();
                let authority = mathfolio_document::export::read_native_text_authority(
                    &package,
                    region.source_region.as_deref().unwrap(),
                )
                .unwrap();
                assert_eq!(authority.original_xaml(), expected);
            }
            cold.selected = Some(id);
            cold.prepare_rich_text();
            assert!(
                cold.rich_text.format_editor(id).is_some(),
                "{}",
                cold.status
            );
            assert!(!cold.background.running && cold.last_executed_regions.is_empty());
        }
        // Empty visualization grants nothing. Only actual Backspace captured
        // before deletion permits the next ordinary Text in the old Start Run.
        // Delete retains the original no-owner/no-edit negative.
        f.select(0, 0);
        let history = f.app.undo.len();
        f.frame(vec![egui::Event::Text("X".into())]);
        if key_kind == egui::Key::Backspace {
            assert_eq!(f.text(), "X");
            let expected_typed = original
                .replace("가A&amp;😀", "X")
                .replace(">BC한</Run>", "></Run>")
                .replace(">DE꼬</Run>", "></Run>");
            assert_eq!(f.xml(), expected_typed);
            assert_eq!(f.app.undo.len(), history + 1);
        } else {
            assert_eq!(f.text(), "");
            assert_eq!(f.xml(), expected);
            assert_eq!(f.app.undo.len(), history);
        }
        assert_eq!(f.app.background.submitted_generation(), generation);
        assert!(!f.app.background.running && f.app.last_executed_regions.is_empty());
    }
}

#[test]
fn actual_run_edge_deletion_preserves_remaining_source_and_multi_paragraph_delete_is_atomic() {
    let mut f = formatted();
    f.select(0, 8);
    let original = f.xml();
    let expected = original
        .replace("가A&amp;😀", "")
        .replace(">BC한</Run>", "></Run>")
        .replace(">DE꼬</Run>", ">E꼬</Run>");
    let history = f.app.undo.len();
    f.frame(vec![key(egui::Key::Delete, egui::Modifiers::NONE)]);
    assert_eq!(f.text(), "E꼬", "{}", f.app.status);
    assert_eq!(f.xml(), expected);
    assert_eq!(f.app.undo.len(), history + 1);
    f.app.undo();
    assert_eq!(f.xml(), original);
    f.app.redo();
    assert_eq!(f.xml(), expected);

    let mut multi = Fixture::new();
    multi.frame(vec![egui::Event::Text("ABCD".into())]);
    multi.select(2, 2);
    multi.frame(vec![key(egui::Key::Enter, egui::Modifiers::NONE)]);
    assert_eq!(multi.text(), "AB\nCD");
    let original = multi.xml();
    multi.frame(vec![key(egui::Key::A, egui::Modifiers::COMMAND)]);
    let history = multi.app.undo.len();
    multi.frame(vec![key(egui::Key::Delete, egui::Modifiers::NONE)]);
    assert_eq!(multi.text(), "AB\nCD");
    assert_eq!(multi.xml(), original);
    assert_eq!(multi.app.undo.len(), history);
}
