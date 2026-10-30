//! Headless native Save integration using generated temporary files only.
#![allow(deprecated)]
use super::*;
use std::io::{Cursor, Write};
use zip::{ZipArchive, ZipWriter};

// A generated native fixture with an explicit known style catalog. The minimal
// export scaffold has none; it is not evidence for original new-Text defaults.
fn install_catalog(path: &Path) {
    const CATALOG: &[u8] =
        include_bytes!("../../../../tests/reference/rich-text/lcc-standard-presentation.xml");
    let mut archive = ZipArchive::new(Cursor::new(std::fs::read(path).unwrap())).unwrap();
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    writer
        .set_raw_comment(archive.comment().to_vec().into_boxed_slice())
        .unwrap();
    let mut replaced = 0;
    for index in 0..archive.len() {
        let entry = archive.by_index(index).unwrap();
        if entry.name().unwrap() == "mathcad/settings/presentation.xml" {
            replaced += 1;
            writer
                .start_file(entry.name().unwrap(), entry.options())
                .unwrap();
            writer.write_all(CATALOG).unwrap();
        } else {
            writer.raw_copy_file(entry).unwrap();
        }
    }
    assert_eq!(replaced, 1);
    std::fs::write(path, writer.finish().unwrap().into_inner()).unwrap();
}

struct Fixture {
    app: WorksheetApp,
    ctx: egui::Context,
    root: PathBuf,
    native: PathBuf,
    text: u64,
    frame: u32,
}
fn leaf(kind: NodeKind) -> PortableTree {
    PortableTree {
        kind,
        children: vec![],
    }
}
fn key(key: egui::Key, modifiers: egui::Modifiers) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers,
    }
}
impl Fixture {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "mathfolio-native-save-ui-{}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        let native = root.join("generated-native.mcdx");
        let mut producer = WorksheetApp {
            automatic_calculation: false,
            ..Default::default()
        };
        producer.document.add_text(38.4, 38.4, "앞😀뒤");
        let math = producer.document.add_math(38.4, 115.2);
        producer
            .document
            .regions
            .iter_mut()
            .find(|r| r.id == math)
            .unwrap()
            .content = CanvasContent::Math(PortableTree {
            kind: NodeKind::Binary(BinaryOp::Evaluate),
            children: vec![
                leaf(NodeKind::Number("7".into())),
                leaf(NodeKind::Placeholder),
            ],
        });
        producer.recalculate(); // Our explicit Rust calculation, never native execution.
        producer.prepare_native_export().unwrap();
        mathfolio_document::export::export_mcdx(&producer.document, &native).unwrap();
        install_catalog(&native);
        let mut app = WorksheetApp {
            automatic_calculation: false,
            ..Default::default()
        };
        app.open(&native);
        assert!(app.status.starts_with("Opened"), "{}", app.status);
        let text = app
            .document
            .regions
            .iter()
            .find(|r| matches!(r.content, CanvasContent::Text(_)))
            .unwrap()
            .id;
        app.selected = Some(text);
        app.focus_new = Some(text);
        app.insertion = None;
        let ctx = egui::Context::default();
        crate::configure(&ctx);
        let mut f = Self {
            app,
            ctx,
            root,
            native,
            text,
            frame: 0,
        };
        f.frame(vec![]);
        f.select(0, 1);
        f
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
            let output = self.ctx.run_ui(raw, |ui| self.app.app_ui_frame(ui));
            output.drop_without_applying_deltas();
            if self.app.plot_input_queue.is_empty() {
                return;
            }
            events = vec![];
        }
        panic!("Input queue did not drain");
    }
    fn select(&mut self, anchor: usize, focus: usize) {
        let widget = egui::Id::new(("worksheet-text", self.text));
        let range = egui::text::CCursorRange::two(
            egui::text::CCursor::new(anchor),
            egui::text::CCursor::new(focus),
        );
        let mut state =
            egui::text_edit::TextEditState::load(&self.ctx, widget).unwrap_or_else(|| {
                panic!(
                    "Missing TextEdit state; source admission: {:?}; status: {}",
                    self.app
                        .rich_text
                        .editors
                        .get(&self.text)
                        .and_then(|entry| entry.as_ref().err()),
                    self.app.status
                )
            });
        state.cursor.set_char_range(Some(range));
        state.store(&self.ctx, widget);
        self.app.text_cursors.insert(self.text, range);
        self.ctx.memory_mut(|memory| memory.request_focus(widget));
    }
    fn source(&self) -> &str {
        let CanvasContent::Text(text) = &self
            .app
            .document
            .regions
            .iter()
            .find(|r| r.id == self.text)
            .unwrap()
            .content
        else {
            panic!()
        };
        text
    }
    fn assert_cold(&self, path: &Path, expected: &str) {
        let mut cold = WorksheetApp {
            automatic_calculation: false,
            ..Default::default()
        };
        cold.open(path);
        assert!(cold.status.starts_with("Opened"), "{}", cold.status);
        assert!(
            cold.document
                .regions
                .iter()
                .any(|r| matches!(&r.content,CanvasContent::Text(text) if text==expected))
        );
        assert!(cold.last_executed_regions.is_empty());
        assert!(!cold.background.running);
        assert!(cold.results.is_empty());
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        // Only files created under this exclusive temporary fixture directory.
        if let Ok(entries) = std::fs::read_dir(&self.root) {
            for entry in entries.flatten() {
                let _ = std::fs::remove_file(entry.path());
            }
        }
        let _ = std::fs::remove_dir(&self.root);
    }
}

#[test]
fn native_open_actual_ctrl_s_twice_keeps_native_identity_old_authority_and_cold_pending_cache() {
    let mut f = Fixture::new();
    assert_eq!(f.app.path.as_deref(), Some(f.native.as_path()));
    assert!(f.app.native_save_target.is_some());
    let baseline = f.app.document.native_archive.clone();
    let generation = f.app.background.submitted_generation();
    f.frame(vec![egui::Event::Text("한".into())]);
    assert_eq!(f.source(), "한😀뒤");
    let history = f.app.undo.len();
    f.frame(vec![key(egui::Key::S, egui::Modifiers::COMMAND)]);
    assert!(f.app.status.starts_with("Saved"), "{}", f.app.status);
    assert!(!f.app.dirty);
    assert_eq!(f.app.undo.len(), history);
    let first_revision = f.app.native_save_target.as_ref().unwrap().revision;
    assert_eq!(first_revision, DiskRevision::read(&f.native).unwrap());
    f.assert_cold(&f.native, "한😀뒤");
    f.select(1, 1);
    f.frame(vec![egui::Event::Text("글".into())]);
    f.frame(vec![key(egui::Key::S, egui::Modifiers::COMMAND)]);
    assert!(f.app.status.starts_with("Saved"), "{}", f.app.status);
    assert_ne!(
        first_revision,
        f.app.native_save_target.as_ref().unwrap().revision
    );
    assert_eq!(f.app.document.native_archive, baseline);
    assert_eq!(f.app.background.submitted_generation(), generation);
    assert!(f.app.last_executed_regions.is_empty());
    f.assert_cold(&f.native, "한글😀뒤");
    let output = NativePackage::open(&f.native).unwrap();
    let caches = output.caches().unwrap();
    assert!(caches.iter().any(|row| {
        row.calculation_status.as_deref() == Some("Pending")
            && row
                .result_mathml
                .as_deref()
                .is_some_and(|xml| xml.contains(">7</"))
    }));
}

#[test]
fn save_as_new_and_existing_native_then_current_save_and_explicit_own_transition() {
    let mut f = Fixture::new();
    let original = std::fs::read(&f.native).unwrap();
    let new = f.root.join("native-save-as.mcdx");
    assert!(
        f.app
            .save_to_destination(Some(new.clone()), SaveIntent::SaveAs),
        "{}",
        f.app.status
    );
    assert_eq!(f.app.path.as_deref(), Some(new.as_path()));
    f.select(0, 1);
    f.frame(vec![egui::Event::Text("Z".into())]);
    assert!(f.app.save(false), "{}", f.app.status);
    assert_eq!(std::fs::read(&f.native).unwrap(), original);
    f.assert_cold(&new, "Z😀뒤");
    let overwrite = f.root.join("explicit-overwrite.mcdx");
    std::fs::write(&overwrite, &original).unwrap();
    assert!(
        f.app
            .save_to_destination(Some(overwrite.clone()), SaveIntent::SaveAs),
        "{}",
        f.app.status
    );
    f.assert_cold(&overwrite, "Z😀뒤");
    let native_target = f.app.native_save_target.clone();
    let bad_own = f.root.join("missing-parent/own.mathfolio");
    f.app.path = Some(bad_own.clone());
    assert!(!f.app.save(false));
    assert_eq!(f.app.path.as_deref(), Some(bad_own.as_path()));
    assert_eq!(f.app.native_save_target, native_target);
    assert!(!bad_own.exists());
    let own = f.root.join("own-after-native.mathfolio");
    // Preserve the pre-existing public/session path fixture convention.
    f.app.path = Some(own.clone());
    assert!(f.app.save(false), "{}", f.app.status);
    assert!(f.app.native_save_target.is_none());
    let own_bytes = std::fs::read(&own).unwrap();
    assert_eq!(own_bytes.first(), Some(&b'{'));
    let own_document = CanvasDocument::load(&own).unwrap();
    assert!(own_document.native_archive.is_some());
    f.app.open(&own);
    assert!(f.app.native_save_target.is_none());
    assert_eq!(f.app.path.as_deref(), Some(own.as_path()));
}

#[test]
fn different_native_path_without_save_as_unknown_suffix_and_conflict_are_atomic() {
    let mut f = Fixture::new();
    let target = f.app.native_save_target.clone();
    let old_path = f.app.path.clone();
    let date = f.app.saved_date.format(DateSavedFormat::YearMonthDay);
    let generation = f.app.background.submitted_generation();
    f.frame(vec![egui::Event::Text("Q".into())]);
    let source = serde_json::to_vec(&f.app.document).unwrap();
    let history = (f.app.undo.len(), f.app.redo.len());
    let other = f.root.join("not-chosen.mcdx");
    assert!(
        !f.app
            .save_to_destination(Some(other.clone()), SaveIntent::Current)
    );
    assert!(!other.exists());
    assert!(
        !f.app
            .save_to_destination(Some(f.root.join("bad.txt")), SaveIntent::SaveAs)
    );
    assert_eq!(serde_json::to_vec(&f.app.document).unwrap(), source);
    std::fs::write(&f.native, b"external change in our disposable fixture").unwrap();
    let external = std::fs::read(&f.native).unwrap();
    assert!(!f.app.save(false));
    assert_eq!(std::fs::read(&f.native).unwrap(), external);
    assert_eq!(f.app.native_save_target, target);
    assert_eq!(f.app.path, old_path);
    assert_eq!(f.app.saved_date.format(DateSavedFormat::YearMonthDay), date);
    assert_eq!((f.app.undo.len(), f.app.redo.len()), history);
    assert!(f.app.dirty);
    assert_eq!(f.app.background.submitted_generation(), generation);
}

#[test]
fn cancellation_preserves_ime_draft_source_history_redo_identity_and_bytes() {
    let mut f = Fixture::new();
    f.frame(vec![egui::Event::Ime(egui::ImeEvent::Enabled)]);
    f.frame(vec![egui::Event::Ime(egui::ImeEvent::Preedit {
        text: "한".into(),
        active_range_chars: Some(0..1),
    })]);
    assert!(f.app.text_composition.is_some());
    let source = serde_json::to_vec(&f.app.document).unwrap();
    let identity = (f.app.path.clone(), f.app.native_save_target.clone());
    let history = (f.app.undo.len(), f.app.redo.len(), f.app.dirty);
    let bytes = std::fs::read(&f.native).unwrap();
    assert!(!f.app.save_to_destination(None, SaveIntent::SaveAs));
    assert_eq!(serde_json::to_vec(&f.app.document).unwrap(), source);
    assert_eq!(
        (f.app.path.clone(), f.app.native_save_target.clone()),
        identity
    );
    assert_eq!((f.app.undo.len(), f.app.redo.len(), f.app.dirty), history);
    assert!(f.app.text_composition.is_some());
    assert_eq!(std::fs::read(&f.native).unwrap(), bytes);
    // Real Save publishes only committed source, not the ongoing preedit.
    assert!(f.app.save(false), "{}", f.app.status);
    f.assert_cold(&f.native, "앞😀뒤");
}

#[test]
fn readonly_and_prepare_failure_keep_identity_bytes_and_source_history() {
    let mut f = Fixture::new();
    f.frame(vec![egui::Event::Text("R".into())]);
    let identity = (f.app.path.clone(), f.app.native_save_target.clone());
    let history = (f.app.undo.len(), f.app.redo.len());
    let bytes = std::fs::read(&f.native).unwrap();
    let permissions = std::fs::metadata(&f.native).unwrap().permissions();
    let mut readonly = permissions.clone();
    readonly.set_readonly(true);
    std::fs::set_permissions(&f.native, readonly).unwrap();
    // Test the unchanged low-level writer refusal, not a real modal question.
    let accepted = f
        .app
        .save_to_destination(Some(f.native.clone()), SaveIntent::Current);
    std::fs::set_permissions(&f.native, permissions).unwrap();
    assert!(!accepted);
    assert_eq!(std::fs::read(&f.native).unwrap(), bytes);
    assert_eq!(
        (f.app.path.clone(), f.app.native_save_target.clone()),
        identity
    );
    assert_eq!((f.app.undo.len(), f.app.redo.len()), history);
    assert!(f.app.dirty);
    let width = f.app.document.width;
    f.app.document.width = f64::NAN;
    assert!(!f.app.save(false));
    f.app.document.width = width;
    assert_eq!(std::fs::read(&f.native).unwrap(), bytes);
    assert_eq!(
        (f.app.path.clone(), f.app.native_save_target.clone()),
        identity
    );
    assert!(f.app.dirty);
}

#[test]
fn failed_open_preserves_native_identity_new_resets_and_export_copy_still_refuses_existing() {
    let mut f = Fixture::new();
    let identity = (f.app.path.clone(), f.app.native_save_target.clone());
    f.app.open(&f.root.join("missing.mcdx"));
    assert_eq!(
        (f.app.path.clone(), f.app.native_save_target.clone()),
        identity
    );
    assert!(mathfolio_document::export::export_mcdx(&f.app.document, &f.native).is_err());
    f.app.apply(Pending::New);
    assert!(f.app.path.is_none() && f.app.native_save_target.is_none());
}

#[path = "native_save_artifact_probe.rs"]
mod artifact_probe;

#[path = "native_save_ui_tests/readonly_prompt_tests.rs"]
mod readonly_prompt_tests;
