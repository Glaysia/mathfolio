//! Actual headless native rich Text editing, source history and save/reopen.
//! No native runtime/GUI. Creation uses only public export + ZIP rebuild APIs.
#![allow(deprecated)]
use super::*;
use std::io::{Cursor, Write};
use zip::{ZipArchive, ZipWriter};

#[path = "paragraph_enter_smoke_tests.rs"]
mod paragraph_enter_smoke_tests;

// Change only these fixture paths when copying to the product tests directory.
const SOURCE_PACKAGE: &[u8] =
    include_bytes!("../../../../tests/reference/rich-text/sample-01.XamlPackage");
const CATALOG: &[u8] =
    include_bytes!("../../../../tests/reference/rich-text/lcc-standard-presentation.xml");
const ORIGINAL: &str = "LCCL-S 시스템 설계";

fn temporary() -> PathBuf {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let root = std::env::temp_dir().join(format!(
        "mathfolio-rich-text-ui-{}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    root
}

/// Generate a valid plain Text owner/relationships with our normal exporter,
/// then replace the target package + catalog without private set_part access.
pub(super) fn native_fixture(directory: &Path) -> PathBuf {
    native_fixture_with(directory, SOURCE_PACKAGE)
}
fn native_fixture_with(directory: &Path, source_package: &[u8]) -> PathBuf {
    let plain = directory.join("generated-plain.mcdx");
    let mut document = CanvasDocument::default();
    document.add_text(38.4, 38.4, ORIGINAL);
    mathfolio_document::export::export_mcdx(&document, &plain).unwrap();
    let mut archive = ZipArchive::new(Cursor::new(std::fs::read(&plain).unwrap())).unwrap();
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    writer
        .set_raw_comment(archive.comment().to_vec().into_boxed_slice())
        .unwrap();
    let mut packages = 0;
    let mut catalogs = 0;
    for index in 0..archive.len() {
        let entry = archive.by_index(index).unwrap();
        let name = entry.name().unwrap().into_owned();
        let data = if name.ends_with(".XamlPackage") {
            packages += 1;
            Some(source_package)
        } else if name == "mathcad/settings/presentation.xml" {
            catalogs += 1;
            Some(CATALOG)
        } else {
            None
        };
        if let Some(data) = data {
            writer.start_file(name, entry.options()).unwrap();
            writer.write_all(data).unwrap();
        } else {
            writer.raw_copy_file(entry).unwrap();
        }
    }
    assert_eq!(
        (packages, catalogs),
        (1, 1),
        "Fixture must replace exactly the generated Text payload/catalog"
    );
    let path = directory.join("original-rich-text.mcdx");
    std::fs::write(&path, writer.finish().unwrap().into_inner()).unwrap();
    let (parsed, _) = import_native(&path).unwrap();
    assert!(
        parsed
            .regions
            .iter()
            .any(|r| matches!(&r.content,CanvasContent::Text(text) if text==ORIGINAL))
    );
    path
}

struct Fixture {
    app: WorksheetApp,
    ctx: egui::Context,
    id: u64,
    root: PathBuf,
    frame: u32,
}
impl Fixture {
    fn new() -> Self {
        Self::with_package(None)
    }
    fn with_package(package: Option<&[u8]>) -> Self {
        let root = temporary();
        let native =
            package.map_or_else(|| native_fixture(&root), |p| native_fixture_with(&root, p));
        let mut app = WorksheetApp {
            automatic_calculation: false,
            ..Default::default()
        };
        app.open(&native);
        let id = app
            .document
            .regions
            .iter()
            .find(|r| matches!(&r.content, CanvasContent::Text(_)))
            .unwrap()
            .id;
        let ctx = egui::Context::default();
        crate::configure(&ctx);
        app.selected = Some(id);
        app.focus_new = Some(id);
        app.insertion = None;
        let mut result = Self {
            app,
            ctx,
            id,
            root,
            frame: 0,
        };
        result.frame(vec![]);
        if result
            .app
            .rich_text
            .editors
            .get(&id)
            .is_some_and(Result::is_ok)
        {
            result.select(1, 4);
        }
        result
    }
    fn frame(&mut self, events: Vec<egui::Event>) {
        let mut events = events;
        for _ in 0..128 {
            self.frame += 1;
            let mut raw = egui::RawInput {
                events,
                time: Some(f64::from(self.frame)),
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1200., 900.))),
                ..Default::default()
            };
            self.app.prepare_plot_input(&self.ctx, &mut raw);
            let output = self.ctx.run_ui(raw, |ui| {
                // Same command-before-field ordering as App::ui, with no
                // eframe window/dialog or original target execution.
                self.app.global_editing_shortcuts(ui.ctx());
                self.app.document_history_shortcuts(ui.ctx());
                self.app.page_ui(ui);
            });
            output.drop_without_applying_deltas();
            if self.app.plot_input_queue.is_empty() {
                return;
            }
            events = vec![];
        }
        panic!("Input queue did not drain");
    }
    fn select(&mut self, anchor: usize, focus: usize) {
        let range = egui::text::CCursorRange::two(
            egui::text::CCursor::new(anchor),
            egui::text::CCursor::new(focus),
        );
        let id = egui::Id::new(("worksheet-text", self.id));
        let mut state = egui::text_edit::TextEditState::load(&self.ctx, id)
            .expect("Actual editable TextEdit was painted");
        state.cursor.set_char_range(Some(range));
        state.store(&self.ctx, id);
        self.app.text_cursors.insert(self.id, range);
        self.ctx.memory_mut(|memory| memory.request_focus(id));
        self.app.selected = Some(self.id);
    }
    fn text(&self) -> &str {
        let CanvasContent::Text(text) = &self
            .app
            .document
            .regions
            .iter()
            .find(|r| r.id == self.id)
            .unwrap()
            .content
        else {
            panic!()
        };
        text
    }
    fn wire(&self) -> Option<&mathfolio_document::rich_text_source::RichTextWire> {
        self.app
            .document
            .regions
            .iter()
            .find(|r| r.id == self.id)
            .unwrap()
            .rich_text
            .as_deref()
    }
    fn xml(&self) -> String {
        self.app.rich_text.editors[&self.id]
            .as_ref()
            .unwrap()
            .source
            .xaml()
            .to_owned()
    }
    fn context(&mut self) -> ribbon::TextFormatContext {
        self.app
            .rich_text_context()
            .expect("Supported selected source must enable formatting")
    }
    fn command(&mut self, build: impl FnOnce(u64) -> RibbonAction) {
        let key = self.context().selection_key;
        self.app.ribbon_command(&self.ctx, build(key));
    }
    fn save(&mut self, name: &str) -> PathBuf {
        let path = self.root.join(name);
        self.app.path = Some(path.clone());
        assert!(self.app.save(false), "{}", self.app.status);
        path
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        // This exclusive fixture directory contains files only; no recursive
        // filesystem removal or cross-shell path construction.
        if let Ok(entries) = std::fs::read_dir(&self.root) {
            for entry in entries.flatten() {
                let _ = std::fs::remove_file(entry.path());
            }
        }
        let _ = std::fs::remove_dir(&self.root);
    }
}
fn key(key: egui::Key, modifiers: egui::Modifiers) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    }
}
fn preedit(text: &str) -> egui::Event {
    egui::Event::Ime(egui::ImeEvent::Preedit {
        text: text.into(),
        active_range_chars: (!text.is_empty()).then_some(0..text.chars().count()),
    })
}
fn attr_on_text(xml: &str, text: &str, name: &str) -> Option<String> {
    let doc = roxmltree::Document::parse(xml).unwrap();
    doc.descendants()
        .find(|node| {
            node.is_element() && node.tag_name().name() == "Run" && node.text() == Some(text)
        })
        .and_then(|node| node.attribute(name))
        .map(str::to_owned)
}

#[test]
fn selected_source_format_actions_and_real_shortcuts_preserve_keys_with_one_history_each() {
    let mut f = Fixture::new();
    let start = f.app.undo.len();
    let generation = f.app.background.submitted_generation();
    f.frame(vec![key(egui::Key::B, egui::Modifiers::COMMAND)]);
    assert_eq!(f.app.undo.len(), start + 1);
    assert_eq!(
        attr_on_text(&f.xml(), "CCL", "FontWeight").as_deref(),
        Some("Normal")
    );
    f.frame(vec![key(egui::Key::I, egui::Modifiers::COMMAND)]);
    assert_eq!(f.app.undo.len(), start + 2);
    assert_eq!(
        attr_on_text(&f.xml(), "CCL", "FontStyle").as_deref(),
        Some("Italic")
    );
    f.command(|selection_key| RibbonAction::SetTextFontSize {
        points: 18.,
        selection_key,
    });
    f.command(|selection_key| RibbonAction::SetTextColor {
        rgba: [12, 34, 56, 200],
        selection_key,
    });
    let expected = f.xml();
    assert_eq!(f.app.undo.len(), start + 4);
    assert_eq!(f.text(), ORIGINAL);
    assert_eq!(
        attr_on_text(&expected, "CCL", "FontSize").as_deref(),
        Some("24")
    );
    assert_eq!(
        attr_on_text(&expected, "CCL", "Foreground").as_deref(),
        Some("#C80C2238")
    );
    assert_eq!(
        attr_on_text(&expected, "CCL", "Background").as_deref(),
        Some("#FF662D91")
    );
    assert_eq!(
        attr_on_text(&expected, "CCL", "style").as_deref(),
        Some("Normal")
    );
    f.app.undo();
    f.app.prepare_rich_text();
    assert_ne!(f.xml(), expected);
    f.app.redo();
    f.app.prepare_rich_text();
    assert_eq!(f.xml(), expected);
    assert_eq!(f.app.background.submitted_generation(), generation);
    assert!(f.app.last_executed_regions.is_empty());
}

#[test]
fn formatted_run_reverse_selection_replacement_second_key_and_save_reopen_keep_source() {
    let mut f = Fixture::new();
    f.command(|selection_key| RibbonAction::SetTextItalic {
        value: true,
        selection_key,
    });
    f.select(4, 1);
    let before = f.app.undo.len();
    f.frame(vec![egui::Event::Text("한😀".into())]);
    assert_eq!(f.text(), "L한😀-S 시스템 설계"); // selected [1,4] = CCL
    f.frame(vec![egui::Event::Text("X".into())]);
    assert_eq!(f.app.undo.len(), before + 2);
    let expected = f.xml();
    assert_eq!(
        attr_on_text(&expected, "한😀X", "FontStyle").as_deref(),
        Some("Italic")
    );
    let own = f.save("rich-own.mathfolio");
    let mut own_cold = WorksheetApp::default();
    own_cold.open(&own);
    let own_region = own_cold
        .document
        .regions
        .iter()
        .find(|r| r.id == f.id)
        .unwrap();
    assert_eq!(own_region.rich_text.as_deref(), f.wire());
    assert!(own_cold.last_executed_regions.is_empty());
    let native = f.root.join("rich-current.mcdx");
    f.app.prepare_native_export().unwrap();
    mathfolio_document::export::export_mcdx(&f.app.document, &native).unwrap();
    let mut native_cold = WorksheetApp::default();
    native_cold.open(&native);
    let id = native_cold
        .document
        .regions
        .iter()
        .find(|r| matches!(r.content, CanvasContent::Text(_)))
        .unwrap()
        .id;
    native_cold.selected = Some(id);
    native_cold.prepare_rich_text();
    assert_eq!(
        native_cold.rich_text.editors[&id]
            .as_ref()
            .unwrap()
            .source
            .xaml(),
        expected
    );
    assert!(native_cold.last_executed_regions.is_empty());
    let package = NativePackage::open(&native).unwrap();
    assert_eq!(
        package.original_part("mathcad/settings/presentation.xml"),
        Some(CATALOG)
    );
}

#[test]
fn stale_selection_key_after_empty_selection_and_region_undo_cannot_format_new_context() {
    let mut f = Fixture::new();
    let stale = f.context().selection_key;
    f.select(4, 4);
    assert!(f.app.rich_text_context().is_none());
    f.select(1, 4);
    let new = f.context().selection_key;
    assert_ne!(stale, new);
    let before = f.xml();
    let undo = f.app.undo.len();
    f.app.ribbon_command(
        &f.ctx,
        RibbonAction::SetTextColor {
            rgba: [1, 2, 3, 255],
            selection_key: stale,
        },
    );
    assert_eq!(f.xml(), before);
    assert_eq!(f.app.undo.len(), undo);
    // First create a rich carrier, then capture a context whose source Arc will
    // be restored by Undo. This catches pointer-equality key resurrection.
    f.command(|selection_key| RibbonAction::SetTextBold {
        value: false,
        selection_key,
    });
    let old = f.context().selection_key;
    f.app.ribbon_command(
        &f.ctx,
        RibbonAction::SetTextColor {
            rgba: [1, 2, 3, 255],
            selection_key: old,
        },
    );
    f.app.undo();
    f.app.prepare_rich_text();
    assert_ne!(f.context().selection_key, old);
    let reverted = f.xml();
    let history = f.app.undo.len();
    f.app.ribbon_command(
        &f.ctx,
        RibbonAction::SetTextColor {
            rgba: [9, 9, 9, 255],
            selection_key: old,
        },
    );
    assert_eq!(f.xml(), reverted);
    assert_eq!(f.app.undo.len(), history);
}

#[test]
fn korean_ime_projection_save_commit_cancel_and_undo_never_publish_preedit() {
    let mut f = Fixture::new();
    f.command(|selection_key| RibbonAction::SetTextItalic {
        value: true,
        selection_key,
    });
    let committed = f.wire().unwrap().clone();
    let before = f.app.undo.len();
    let generation = f.app.background.submitted_generation();
    f.frame(vec![egui::Event::Ime(egui::ImeEvent::Enabled)]);
    f.frame(vec![preedit("ㅎ")]);
    f.frame(vec![preedit("하")]);
    assert_eq!(f.text(), ORIGINAL);
    assert_eq!(f.wire(), Some(&committed));
    assert_eq!(f.app.undo.len(), before);
    let path = f.root.join("preedit.mathfolio");
    f.app.path = Some(path.clone());
    let accepted = f.app.save(false);
    assert!(
        accepted,
        "IME Save must persist the committed source: {}",
        f.app.status
    );
    if accepted {
        let doc = CanvasDocument::load(&path).unwrap();
        assert_eq!(
            doc.regions
                .iter()
                .find(|r| r.id == f.id)
                .unwrap()
                .rich_text
                .as_deref(),
            Some(&committed)
        );
    }
    f.frame(vec![preedit("")]);
    assert_eq!(f.wire(), Some(&committed));
    assert_eq!(f.app.undo.len(), before);
    f.select(1, 4);
    f.frame(vec![
        egui::Event::Ime(egui::ImeEvent::Enabled),
        preedit("하"),
    ]);
    f.frame(vec![egui::Event::Ime(egui::ImeEvent::Commit("한".into()))]);
    assert_eq!(f.text(), "L한-S 시스템 설계");
    assert_eq!(f.app.undo.len(), before + 1);
    let after = f.wire().unwrap().clone();
    f.app.undo();
    assert_eq!(f.wire(), Some(&committed));
    f.app.redo();
    assert_eq!(f.wire(), Some(&after));
    assert!(f.app.last_executed_regions.is_empty());
    assert_eq!(f.app.background.submitted_generation(), generation);
}

#[test]
fn real_batched_navigation_and_format_shortcuts_respect_delivery_order() {
    let mut f = Fixture::new();
    let before = f.app.undo.len();
    f.frame(vec![
        key(egui::Key::B, egui::Modifiers::COMMAND),
        key(egui::Key::I, egui::Modifiers::COMMAND),
    ]);
    assert_eq!(
        attr_on_text(&f.xml(), "CCL", "FontWeight").as_deref(),
        Some("Normal")
    );
    assert_eq!(
        attr_on_text(&f.xml(), "CCL", "FontStyle").as_deref(),
        Some("Italic")
    );
    assert_eq!(f.app.undo.len(), before + 2);
    // Repeated chars expose stale before-cursor/minimal-diff attribution.
    // Bold root + explicit Normal first C. Arrow from1→2 leaves an unowned
    // internal boundary, so input must be rejected, not moved to position3.
    let mut repeated = Fixture::new();
    repeated.select(1, 2);
    repeated.command(|selection_key| RibbonAction::SetTextBold {
        value: false,
        selection_key,
    });
    repeated.select(1, 1);
    let source = repeated.xml();
    let history = repeated.app.undo.len();
    repeated.frame(vec![
        key(egui::Key::ArrowRight, egui::Modifiers::NONE),
        egui::Event::Text("C".into()),
    ]);
    assert_eq!(repeated.text(), ORIGINAL);
    assert_eq!(repeated.xml(), source);
    assert_eq!(repeated.app.undo.len(), history);
    assert!(f.app.last_executed_regions.is_empty());
}

#[test]
fn source_owned_run_affinity_survives_document_undo_before_next_character() {
    let mut f = Fixture::new();
    f.command(|selection_key| RibbonAction::SetTextItalic {
        value: true,
        selection_key,
    });
    f.select(4, 1);
    f.frame(vec![egui::Event::Text("한😀".into())]);
    let original_edit = f.xml();
    f.frame(vec![egui::Event::Text("X".into())]);
    f.app.undo();
    f.frame(vec![egui::Event::Text("Y".into())]);
    assert_eq!(f.text(), "L한😀Y-S 시스템 설계", "{}", f.app.status);
    assert_eq!(
        attr_on_text(&f.xml(), "한😀Y", "FontStyle").as_deref(),
        Some("Italic")
    );
    f.app.undo();
    assert_eq!(f.xml(), original_edit);
}

#[test]
fn allowed_font_point_range_is_supported_by_both_source_and_native_visual() {
    let mut f = Fixture::new();
    for points in [1., 400., 1638.] {
        f.command(|selection_key| RibbonAction::SetTextFontSize {
            points,
            selection_key,
        });
        let actual = attr_on_text(&f.xml(), "CCL", "FontSize")
            .unwrap()
            .parse::<f64>()
            .unwrap();
        assert!(
            (actual - points / 72. * 96.).abs() < 1e-10,
            "{}",
            f.app.status
        );
        let visual = f
            .app
            .native_visuals
            .region(&f.app.document.regions[0])
            .unwrap()
            .as_ref()
            .unwrap();
        let mathfolio_document::native_visual::Block::Paragraph(p) =
            &visual.document.as_ref().unwrap().blocks[0]
        else {
            panic!()
        };
        let run = p.runs.iter().find(|r| r.text == "CCL").unwrap();
        assert!((f64::from(run.style.font_size) - actual).abs() < 0.001);
    }
}

fn replaced_package_xaml(source: &str) -> Vec<u8> {
    let mut archive = ZipArchive::new(Cursor::new(SOURCE_PACKAGE)).unwrap();
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for i in 0..archive.len() {
        let entry = archive.by_index(i).unwrap();
        let name = entry.name().unwrap().into_owned();
        if name == "Xaml/Document.xaml" {
            writer.start_file(name, entry.options()).unwrap();
            writer.write_all(source.as_bytes()).unwrap();
        } else {
            writer.raw_copy_file(entry).unwrap();
        }
    }
    writer.finish().unwrap().into_inner()
}

#[test]
fn single_centered_paragraph_is_editable_while_background_keeps_source_readonly() {
    use mathfolio_document::native_visual::{Alignment, Block};
    let original = include_str!("../../../../tests/reference/rich-text/sample-01.xaml");
    for (attribute, centered, background) in [
        ("TextAlignment=\"Center\"", true, None),
        ("Background=\"#FFFF0000\"", false, Some([255, 0, 0, 255])),
    ] {
        let source = original.replacen("<Paragraph ", &format!("<Paragraph {attribute} "), 1);
        let package = replaced_package_xaml(&source);
        let mut f = Fixture::with_package(Some(&package));
        assert_eq!(
            f.app.rich_text.editors[&f.id].is_ok(),
            centered,
            "Single-Paragraph alignment is admitted; paragraph background is still readonly"
        );
        let package =
            NativePackage::from_bytes(f.app.document.native_archive.as_deref().unwrap()).unwrap();
        let owner = f.app.document.regions[0].source_region.as_deref().unwrap();
        let authority =
            mathfolio_document::export::read_native_text_authority(&package, owner).unwrap();
        let current = mathfolio_document::rich_text_source::ValidatedRichText::from_original(
            authority, ORIGINAL,
        )
        .unwrap();
        f.app.document.regions[0].rich_text = Some(Box::new(current.wire().clone()));
        f.app.refresh_rich_text_visual(f.id);
        let own = f.save("readonly-source.mathfolio");
        let mut cold = WorksheetApp::default();
        cold.open(&own);
        for app in [&f.app, &cold] {
            let visual = app
                .native_visuals
                .region(&app.document.regions[0])
                .unwrap()
                .as_ref()
                .unwrap();
            let Block::Paragraph(p) = &visual.document.as_ref().unwrap().blocks[0] else {
                panic!()
            };
            assert_eq!(p.style.alignment == Alignment::Center, centered);
            assert_eq!(p.style.background, background);
            assert_eq!(
                visual.document.as_ref().unwrap().imported_plain_text,
                ORIGINAL
            );
            assert!(app.last_executed_regions.is_empty());
        }
    }
}

fn copied_region(f: &mut Fixture) -> String {
    let output = f.ctx.run_ui(egui::RawInput::default(), |ui| {
        f.app.copy_region(ui.ctx(), false)
    });
    let copied = output.platform_output.commands.iter().find_map(|command| {
        if let egui::OutputCommand::CopyText(text) = command {
            Some(text.clone())
        } else {
            None
        }
    });
    output.drop_without_applying_deltas();
    copied.expect("Whole-region Copy must emit a clipboard envelope")
}

#[test]
fn native_original_region_copy_and_duplicate_preserve_rich_source_without_editing_original() {
    let mut f = Fixture::new();
    let source = f.xml();
    assert!(f.wire().is_none());
    let copy = copied_region(&mut f);
    assert!(f.wire().is_none());
    assert_eq!(f.app.undo.len(), 0);
    let original_id = f.id;
    assert!(f.app.paste_region(&copy), "{}", f.app.status);
    let pasted = f.app.selected.unwrap();
    assert_ne!(pasted, original_id);
    assert_eq!(f.app.document.regions.len(), 2);
    f.app.prepare_rich_text();
    assert_eq!(
        f.app.rich_text.editors[&pasted]
            .as_ref()
            .unwrap()
            .source
            .xaml(),
        source
    );
    f.app.selected = Some(original_id);
    f.app.duplicate_selected();
    assert_eq!(f.app.document.regions.len(), 3);
    let native = f.root.join("copied-rich.mcdx");
    f.app.prepare_native_export().unwrap();
    f.app.document.export_mcdx(&native).unwrap();
    let mut cold = WorksheetApp::default();
    cold.open(&native);
    assert_eq!(cold.document.regions.len(), 3);
    let ids: Vec<_> = cold.document.regions.iter().map(|r| r.id).collect();
    for id in ids {
        cold.selected = Some(id);
        cold.prepare_rich_text();
        assert_eq!(
            cold.rich_text.editors[&id].as_ref().unwrap().source.xaml(),
            source
        );
    }
    let mut foreign = WorksheetApp::default();
    let before = serde_json::to_vec(&foreign.document).unwrap();
    assert!(foreign.paste_region(&copy));
    assert_eq!(serde_json::to_vec(&foreign.document).unwrap(), before);
    assert!(foreign.undo.is_empty() && foreign.redo.is_empty());
}

#[test]
fn aggregate_budget_rejects_format_and_committed_key_before_source_runtime_history_or_id_change() {
    let mut f = Fixture::new();
    let available = 8 * 1024 * 1024 - f.app.document.source_text_bytes().unwrap();
    let mut left = available;
    while left > 0 {
        let size = left.min(1024 * 1024);
        f.app.document.add_text(4000., 4000., "x".repeat(size));
        left -= size;
    }
    assert_eq!(f.app.document.source_text_bytes().unwrap(), 8 * 1024 * 1024);
    let xml = f.xml();
    let ids = f.app.document.next_id;
    let history = f.app.undo.len();
    f.command(|selection_key| RibbonAction::SetTextItalic {
        value: true,
        selection_key,
    });
    assert_eq!(f.xml(), xml);
    assert!(f.wire().is_none());
    f.frame(vec![egui::Event::Text("한".into())]);
    assert_eq!(f.text(), ORIGINAL);
    assert_eq!(f.xml(), xml);
    assert_eq!(f.app.undo.len(), history);
    assert_eq!(f.app.document.next_id, ids);
    assert!(f.wire().is_none());
    f.app.duplicate_selected();
    assert_eq!(f.app.document.next_id, ids);
    assert_eq!(f.app.undo.len(), history);
}

fn artifact_flow(output: &Path) {
    assert!(!output.exists(), "Artifact output must be new");
    std::fs::create_dir_all(output).unwrap();
    let mut f = Fixture::new();
    for build in [
        RibbonAction::SetTextBold {
            value: false,
            selection_key: 0,
        },
        RibbonAction::SetTextItalic {
            value: true,
            selection_key: 0,
        },
        RibbonAction::SetTextFontSize {
            points: 18.,
            selection_key: 0,
        },
        RibbonAction::SetTextColor {
            rgba: [12, 34, 56, 200],
            selection_key: 0,
        },
    ] {
        f.command(|selection_key| match build {
            RibbonAction::SetTextBold { value, .. } => RibbonAction::SetTextBold {
                value,
                selection_key,
            },
            RibbonAction::SetTextItalic { value, .. } => RibbonAction::SetTextItalic {
                value,
                selection_key,
            },
            RibbonAction::SetTextFontSize { points, .. } => RibbonAction::SetTextFontSize {
                points,
                selection_key,
            },
            RibbonAction::SetTextColor { rgba, .. } => RibbonAction::SetTextColor {
                rgba,
                selection_key,
            },
            _ => unreachable!(),
        });
    }
    let formatted_xml = f.xml();
    f.save("rich-formatted.mathfolio");
    f.app.prepare_native_export().unwrap();
    f.app
        .document
        .export_mcdx(&f.root.join("rich-formatted.mcdx"))
        .unwrap();
    f.select(4, 1);
    f.frame(vec![egui::Event::Text("한😀".into())]);
    f.frame(vec![egui::Event::Text("X".into())]);
    let edited_xml = f.xml();
    assert_eq!(f.text(), "L한😀X-S 시스템 설계");
    f.save("rich-edited.mathfolio");
    f.app.prepare_native_export().unwrap();
    f.app
        .document
        .export_mcdx(&f.root.join("rich-edited.mcdx"))
        .unwrap();
    f.app.selected = Some(f.id);
    f.app.duplicate_selected();
    assert_eq!(f.app.document.regions.len(), 2);
    f.save("rich-copied.mathfolio");
    f.app.prepare_native_export().unwrap();
    f.app
        .document
        .export_mcdx(&f.root.join("rich-copied.mcdx"))
        .unwrap();
    let names = [
        "original-rich-text.mcdx",
        "rich-formatted.mathfolio",
        "rich-formatted.mcdx",
        "rich-edited.mathfolio",
        "rich-edited.mcdx",
        "rich-copied.mathfolio",
        "rich-copied.mcdx",
    ];
    let mut cold_rows = vec![];
    for name in names {
        let path = f.root.join(name);
        let mut cold = WorksheetApp::default();
        cold.open(&path);
        assert!(!cold.document.regions.is_empty(), "{}", cold.status);
        assert!(cold.last_executed_regions.is_empty());
        let copied = name.contains("copied");
        let expected = if name == "original-rich-text.mcdx" || name.contains("formatted") {
            ORIGINAL
        } else {
            "L한😀X-S 시스템 설계"
        };
        assert_eq!(cold.document.regions.len(), if copied { 2 } else { 1 });
        let ids = cold
            .document
            .regions
            .iter()
            .map(|r| r.id)
            .collect::<Vec<_>>();
        for id in ids {
            let region = cold.document.regions.iter().find(|r| r.id == id).unwrap();
            assert!(matches!(&region.content, CanvasContent::Text(plain) if plain==expected));
            assert!(
                cold.native_visuals
                    .region(region)
                    .is_some_and(Result::is_ok)
            );
            cold.selected = Some(id);
            cold.prepare_rich_text();
            let source = cold.rich_text.editors[&id].as_ref().unwrap().source.xaml();
            if name != "original-rich-text.mcdx" {
                assert_eq!(
                    source,
                    if name.contains("formatted") {
                        &formatted_xml
                    } else {
                        &edited_xml
                    }
                );
            }
        }
        std::fs::copy(&path, output.join(name)).unwrap();
        cold_rows.push(serde_json::json!({"file":name,"regions":cold.document.regions.len(),"executed_regions":cold.last_executed_regions}));
    }
    let report = serde_json::json!({"original_runtime":false,"gui":false,"native_files":4,"own_files":3,
        "cold_documents":cold_rows,"formatted_source":formatted_xml,"edited_source":edited_xml,
        "region_copy":true,"text_selection_clipboard_rich":false,"cross_run_text_replace":false});
    std::fs::write(
        output.join("observation.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
    println!(
        "RICH_TEXT_SOURCE_FLOW native_files=4 own_files=3 cold_documents=7 original_runtime=false gui=false"
    );
}

#[test]
#[ignore = "writes source/own/native rich Text artifacts to an explicit new output path"]
fn rich_text_source_save_native_export_reopen_probe() {
    let path = std::env::var_os("MATHFOLIO_RICH_TEXT_OUTPUT")
        .expect("Set a new artifact output directory");
    artifact_flow(Path::new(&path));
}

#[path = "rich_paragraph_integration_tests.rs"]
mod rich_paragraph_integration_tests;

#[path = "native_save_rich_tests.rs"]
mod native_save_rich_tests;

#[path = "native_save_rich_probe.rs"]
pub(super) mod native_save_rich_probe;

#[path = "underline_native_guard_tests.rs"]
mod underline_native_guard_tests;

#[path = "paragraph_alignment_native_tests.rs"]
mod paragraph_alignment_native_tests;

#[path = "native_select_all_app_tests.rs"]
mod native_select_all_app_tests;
