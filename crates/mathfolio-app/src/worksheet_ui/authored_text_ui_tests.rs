//! Actual application input with a supplied, resolved authored format snapshot.
//! Factory/default resolution is tested separately; no original runtime/window.
#![allow(deprecated)]
use super::*;
use mathfolio_document::rich_text_source::{
    AuthoredRichText, AuthoredStyleSnapshot, AuthoredTextSeed, AuthoredTextSeedSpec,
};

fn source() -> AuthoredRichText {
    let style = AuthoredStyleSnapshot {
        key: "ExplicitOwnBody".into(),
        font_family: "Tahoma".into(),
        size_dip: "18.5".into(),
        weight: "Normal".into(),
        font_style: "Normal".into(),
        alignment: "Left".into(),
        foreground_rgba: [18, 52, 86, 255],
        background_rgba: [255, 255, 255, 0],
    };
    AuthoredRichText::empty(
        AuthoredTextSeed::new(AuthoredTextSeedSpec {
            format: style.clone(),
            catalog: vec![style],
            proofing_language: "ko-KR".into(),
            text_decorations: None,
        })
        .unwrap(),
    )
    .unwrap()
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
struct Fixture {
    app: WorksheetApp,
    ctx: egui::Context,
    id: u64,
    frame: u32,
}
impl Fixture {
    fn new() -> Self {
        let mut app = WorksheetApp {
            automatic_calculation: false,
            ..Default::default()
        };
        let value = source();
        let id = app.document.add_text(38.4, 38.4, "");
        app.document
            .regions
            .iter_mut()
            .find(|r| r.id == id)
            .unwrap()
            .authored_text = Some(Box::new(value.wire().clone()));
        app.selected = Some(id);
        app.focus_new = Some(id);
        app.insertion = None;
        app.refresh_rich_text_visual(id);
        let ctx = egui::Context::default();
        crate::configure(&ctx);
        let mut f = Self {
            app,
            ctx,
            id,
            frame: 0,
        };
        f.frame(vec![]);
        f.select(0, 0);
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
        panic!("Authored input queue did not drain");
    }
    fn region(&self) -> &CanvasRegion {
        self.app
            .document
            .regions
            .iter()
            .find(|r| r.id == self.id)
            .unwrap()
    }
    fn text(&self) -> &str {
        let CanvasContent::Text(text) = &self.region().content else {
            panic!()
        };
        text
    }
    fn xml(&self) -> String {
        self.region()
            .validated_authored_text()
            .unwrap()
            .unwrap()
            .xaml()
            .into()
    }
    fn select(&mut self, a: usize, b: usize) {
        let range =
            egui::text::CCursorRange::two(egui::text::CCursor::new(a), egui::text::CCursor::new(b));
        let id = egui::Id::new(("worksheet-text", self.id));
        let mut state = egui::text_edit::TextEditState::load(&self.ctx, id).unwrap_or_else(|| {
            panic!(
                "{}; {:?}",
                self.app.status,
                self.app
                    .rich_text
                    .authored_editors
                    .get(&self.id)
                    .and_then(|r| r.as_ref().err())
            )
        });
        state.cursor.set_char_range(Some(range));
        state.store(&self.ctx, id);
        self.app.text_cursors.insert(self.id, range);
        self.app.selected = Some(self.id);
        self.ctx.memory_mut(|m| m.request_focus(id));
    }
}

#[test]
fn authored_actual_ime_first_text_confirmation_and_blank_paragraphs() {
    let mut f = Fixture::new();
    let empty = f.xml();
    let history = f.app.undo.len();
    f.frame(vec![egui::Event::Ime(egui::ImeEvent::Preedit {
        text: "ㅎ".into(),
        active_range_chars: None,
    })]);
    assert_eq!(f.text(), "");
    assert_eq!(f.xml(), empty);
    assert_eq!(f.app.undo.len(), history);
    f.frame(vec![
        egui::Event::Ime(egui::ImeEvent::Commit("한😀".into())),
        key(egui::Key::Enter, egui::Modifiers::NONE),
    ]);
    assert_eq!(f.text(), "한😀", "{}", f.app.status);
    assert!(!f.xml().contains("ㅎ"));
    f.frame(vec![
        key(egui::Key::Enter, egui::Modifiers::NONE),
        key(egui::Key::Enter, egui::Modifiers::NONE),
        egui::Event::Text("끝".into()),
    ]);
    assert_eq!(f.text(), "한😀\n\n끝", "{}", f.app.status);
    assert_eq!(f.xml().matches("<Paragraph>").count(), 3);
    assert!(f.app.last_executed_regions.is_empty());
    assert!(!f.app.background.running);
}

#[test]
fn authored_actual_selection_toolbar_history_and_style_affinity() {
    let mut f = Fixture::new();
    f.frame(vec![egui::Event::Text("가나다😀라".into())]);
    f.select(3, 1);
    let generation = f.app.background.submitted_generation();
    let key = f
        .app
        .rich_text_context()
        .expect("Authored range must enable toolbar")
        .selection_key;
    f.app.ribbon_command(
        &f.ctx,
        RibbonAction::SetTextBold {
            value: true,
            selection_key: key,
        },
    );
    let xml = f.xml();
    assert!(
        roxmltree::Document::parse(&xml)
            .unwrap()
            .descendants()
            .any(|n| n.is_element()
                && n.tag_name().name() == "Run"
                && n.attribute("FontWeight") == Some("Bold")
                && n.text() == Some("나다")),
        "{xml}"
    );
    let bold = f.xml();
    f.app.undo();
    assert!(!f.xml().contains("FontWeight=\"Bold\""));
    f.app.redo();
    assert_eq!(f.xml(), bold);
    f.select(3, 1);
    f.frame(vec![
        egui::Event::Text("한".into()),
        egui::Event::Text("글".into()),
    ]);
    assert_eq!(f.text(), "가한글😀라", "{}", f.app.status);
    let xml = f.xml();
    assert!(
        roxmltree::Document::parse(&xml)
            .unwrap()
            .descendants()
            .any(|n| n.is_element()
                && n.tag_name().name() == "Run"
                && n.attribute("FontWeight") == Some("Bold")
                && n.text() == Some("한글")),
        "{xml}"
    );
    assert!(f.app.last_executed_regions.is_empty());
    // Text content may mark calculation context pending; it must not submit a
    // computation while automatic calculation is disabled.
    assert_eq!(f.app.background.submitted_generation(), generation);
}

#[test]
fn authored_own_cold_reopen_retains_source_and_idle_runtime_version() {
    let mut f = Fixture::new();
    f.frame(vec![egui::Event::Text("저장😀".into())]);
    let expected = f.region().authored_text.clone();
    let path = std::env::temp_dir().join(format!(
        "mathfolio-authored-own-{}-{}.mathfolio",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    f.app.document.save(&path).unwrap();
    let mut cold = WorksheetApp {
        automatic_calculation: false,
        ..Default::default()
    };
    cold.open(&path);
    std::fs::remove_file(&path).unwrap();
    assert!(cold.status.starts_with("Opened"), "{}", cold.status);
    let region = cold.document.regions.iter().find(|r| r.id == f.id).unwrap();
    assert_eq!(region.authored_text, expected);
    assert!(region.rich_text.is_none());
    assert!(cold.native_visuals.region(region).unwrap().is_ok());
    assert!(cold.last_executed_regions.is_empty());
    assert!(!cold.background.running);
    f.frame(vec![]);
    let pointer = f.app.rich_text.authored_editors[&f.id]
        .as_ref()
        .unwrap()
        .source
        .xaml()
        .as_ptr();
    let xml = f.xml();
    let history = f.app.undo.len();
    for _ in 0..10 {
        f.frame(vec![]);
    }
    assert_eq!(
        f.app.rich_text.authored_editors[&f.id]
            .as_ref()
            .unwrap()
            .source
            .xaml()
            .as_ptr(),
        pointer
    );
    assert_eq!(f.xml(), xml);
    assert_eq!(f.app.undo.len(), history);
}

#[test]
fn authored_clipboard_foreign_owner_and_forged_source_are_atomic() {
    let mut f = Fixture::new();
    f.frame(vec![egui::Event::Text("원본😀".into())]);
    f.app.duplicate_selected();
    let duplicate = f.app.selected.unwrap();
    assert_ne!(duplicate, f.id);
    assert_eq!(
        f.app
            .document
            .regions
            .iter()
            .find(|r| r.id == duplicate)
            .unwrap()
            .authored_text,
        f.region().authored_text
    );
    let payload =
        serde_json::json!({"content":f.region().content,"authored_text":f.region().authored_text});
    let mut other = WorksheetApp {
        automatic_calculation: false,
        ..Default::default()
    };
    assert!(other.paste_region(&format!("mathcraft-region:{payload}")));
    assert_eq!(other.document.regions.len(), 1);
    assert!(other.document.regions[0].authored_text.is_some());
    let mut forged = payload.clone();
    forged["authored_text"]["current_xaml"] = serde_json::json!("<FlowDocument/>");
    let before = other.document.clone();
    let history = other.undo.len();
    assert!(other.paste_region(&format!("mathcraft-region:{forged}")));
    assert_eq!(other.document, before);
    assert_eq!(other.undo.len(), history);
}

#[test]
fn authored_editor_budget_failure_preserves_version_and_affinity() {
    let mut editor = authored_text_input::AuthoredEditor::new(source()).unwrap();
    let cursor = egui::text::CCursorRange::one(egui::text::CCursor::new(0));
    let end = egui::text::CCursorRange::one(egui::text::CCursor::new(2));
    let wire = editor.source.wire().clone();
    assert!(editor.commit("한😀", cursor, end, 1).is_err());
    assert_eq!(editor.source.wire(), &wire);
    assert!(editor.affinity.owner(&editor.source).is_none());
    assert!(editor.split_paragraph(cursor, 1).is_err());
    assert_eq!(editor.source.wire(), &wire);
}

#[test]
fn actual_ctrl_t_creates_empty_authored_owner_and_first_text_in_one_command() {
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    let mut f = Fixture {
        app: WorksheetApp {
            automatic_calculation: false,
            ..Default::default()
        },
        ctx,
        id: 0,
        frame: 0,
    };
    f.frame(vec![key(egui::Key::T, egui::Modifiers::COMMAND)]);
    f.id = f
        .app
        .selected
        .expect("Ctrl+T must create an owner immediately");
    assert_eq!(f.app.document.regions.len(), 1);
    assert_eq!(f.app.undo.len(), 1);
    assert_eq!(f.text(), "");
    assert!(f.region().rich_text.is_none());
    assert!(f.region().authored_text.is_some());
    f.select(0, 0);
    f.frame(vec![egui::Event::Text("첫 한글😀".into())]);
    assert_eq!(f.text(), "첫 한글😀", "{}", f.app.status);
    f.app.document.validate().unwrap();
    assert!(f.app.last_executed_regions.is_empty());
}

#[test]
fn ctrl_t_captures_current_authored_format_with_own_key_and_language() {
    let mut f = Fixture::new();
    f.frame(vec![egui::Event::Text("AB".into())]);
    f.select(2, 0);
    let key = f.app.rich_text_context().unwrap().selection_key;
    f.app.ribbon_command(
        &f.ctx,
        RibbonAction::SetTextBold {
            value: true,
            selection_key: key,
        },
    );
    let previous = f.id;
    let old = f.xml();
    f.frame(vec![key_event_t()]);
    f.id = f.app.selected.unwrap();
    assert_ne!(f.id, previous);
    let source = f.region().validated_authored_text().unwrap().unwrap();
    assert_eq!(source.seed().spec().format.key, "ExplicitOwnBody");
    assert_eq!(source.seed().spec().format.size_dip, "18.5");
    assert_eq!(source.seed().spec().format.weight, "Bold");
    assert_eq!(source.seed().spec().proofing_language, "ko-KR");
    assert_eq!(source.plain_text(), "");
    f.app.undo();
    assert_eq!(f.app.document.regions.len(), 1);
    let restored = f
        .app
        .document
        .regions
        .iter()
        .find(|r| r.id == previous)
        .unwrap()
        .validated_authored_text()
        .unwrap()
        .unwrap();
    assert_eq!(restored.xaml(), old);
}
fn key_event_t() -> egui::Event {
    key(egui::Key::T, egui::Modifiers::COMMAND)
}

#[test]
fn failed_text_creation_does_not_publish_owner_history_or_selection() {
    let mut app = WorksheetApp {
        automatic_calculation: false,
        ..Default::default()
    };
    app.document.native_archive = Some(vec![0, 1, 2]);
    let before = app.document.clone();
    let selection = app.selected;
    let history = app.undo.len();
    app.begin_region(true);
    assert_eq!(app.document, before);
    assert_eq!(app.selected, selection);
    assert_eq!(app.undo.len(), history);
    assert!(app.status.starts_with("텍스트 삽입:"));
}

#[test]
#[ignore]
fn authored_text_artifact_probe() {
    use native_save_ui::SaveIntent;
    let output = std::env::var_os("MATHFOLIO_AUTHORED_TEXT_OUTPUT")
        .expect("Set fresh authored packet directory");
    let output = PathBuf::from(output);
    std::fs::create_dir(&output).expect("Packet directory must not exist");
    let mut f = Fixture::new();
    let mut observations = vec![];
    let mut capture = |f: &mut Fixture, name: &str| {
        let plain = f.text().to_owned();
        let xml = f.xml();
        let own = format!("authored-{name}.mathfolio");
        let native = format!("authored-{name}.mcdx");
        let pdf = format!("authored-{name}.pdf");
        assert!(
            f.app
                .save_to_destination(Some(output.join(&own)), SaveIntent::SaveAs),
            "{}",
            f.app.status
        );
        assert!(
            f.app
                .save_to_destination(Some(output.join(&native)), SaveIntent::SaveAs),
            "{}",
            f.app.status
        );
        assert_eq!(f.text(), plain);
        assert_eq!(f.xml(), xml);
        f.app
            .export_vector_pdf_for_test(&output.join(&pdf))
            .unwrap();
        let mut cold = WorksheetApp {
            automatic_calculation: false,
            ..Default::default()
        };
        cold.open(&output.join(&native));
        assert!(cold.status.starts_with("Opened"), "{}", cold.status);
        let region = cold
            .document
            .regions
            .iter()
            .find(|r| matches!(&r.content,CanvasContent::Text(text) if text==&plain))
            .unwrap();
        let owner = region.source_region.clone().unwrap();
        let package = NativePackage::open(&output.join(&native)).unwrap();
        let authority =
            mathfolio_document::export::read_native_text_authority(&package, &owner).unwrap();
        assert_eq!(authority.original_xaml(), xml);
        assert!(cold.native_visuals.region(region).unwrap().is_ok());
        assert!(cold.last_executed_regions.is_empty());
        assert!(!cold.background.running);
        observations.push(
            serde_json::json!({"name":name,"own":own,"native":native,"pdf":pdf,
            "owner_id":owner,"canvas_owner_id":f.id,"expected_plain":plain,"current_xml":xml,
            "executed_regions":[],"background_running":false}),
        );
    };
    capture(&mut f, "empty");
    f.frame(vec![egui::Event::Text("한글😀AB".into())]);
    capture(&mut f, "typed");
    f.select(2, 0);
    let selection_key = f.app.rich_text_context().unwrap().selection_key;
    f.app.ribbon_command(
        &f.ctx,
        RibbonAction::SetTextBold {
            value: true,
            selection_key,
        },
    );
    let selection_key = f.app.rich_text_context().unwrap().selection_key;
    f.app.ribbon_command(
        &f.ctx,
        RibbonAction::SetTextColor {
            rgba: [102, 45, 145, 255],
            selection_key,
        },
    );
    let selection_key = f.app.rich_text_context().unwrap().selection_key;
    f.app.ribbon_command(
        &f.ctx,
        RibbonAction::SetTextFontSize {
            points: 18.,
            selection_key,
        },
    );
    capture(&mut f, "styled");
    f.select(1, 1);
    f.frame(vec![key(egui::Key::Enter, egui::Modifiers::NONE)]);
    capture(&mut f, "paragraph");
    f.app.undo();
    capture(&mut f, "undo");
    f.app.redo();
    capture(&mut f, "redo");
    std::fs::write(
        output.join("observation.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
        "version":1,"original_runtime":false,"gui":false,"states":observations}))
        .unwrap(),
    )
    .unwrap();
    assert!(f.app.last_executed_regions.is_empty());
    assert!(!f.app.background.running);
    println!(
        "AUTHORED_TEXT states=6 own=6 native=6 pdf=6 original_runtime=false gui=false calculations=0"
    );
}

#[test]
fn fresh_ctrl_t_native_cold_reopens_editable_and_can_create_next_text() {
    let mut app = WorksheetApp {
        automatic_calculation: false,
        ..Default::default()
    };
    app.begin_region(true);
    let id = app.selected.unwrap();
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    let mut f = Fixture {
        app,
        ctx,
        id,
        frame: 0,
    };
    f.frame(vec![]);
    f.select(0, 0);
    f.frame(vec![egui::Event::Text("본문😀".into())]);
    let path = std::env::temp_dir().join(format!(
        "mathfolio-authored-fresh-{}-{}.mcdx",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    assert!(
        f.app
            .save_to_destination(Some(path.clone()), native_save_ui::SaveIntent::SaveAs),
        "{}",
        f.app.status
    );
    let mut cold = WorksheetApp {
        automatic_calculation: false,
        ..Default::default()
    };
    cold.open(&path);
    std::fs::remove_file(&path).unwrap();
    assert!(cold.status.starts_with("Opened"), "{}", cold.status);
    let id = cold
        .document
        .regions
        .iter()
        .find(|r| matches!(&r.content,CanvasContent::Text(text) if text=="본문😀"))
        .unwrap()
        .id;
    cold.selected = Some(id);
    cold.prepare_rich_text();
    assert!(cold.rich_text.editors[&id].is_ok());
    cold.begin_region(true);
    assert_eq!(cold.document.regions.len(), 2, "{}", cold.status);
    assert!(
        cold.document
            .regions
            .last()
            .unwrap()
            .authored_text
            .is_some()
    );
    assert!(cold.last_executed_regions.is_empty());
    assert!(!cold.background.running);
}

#[path = "underline_app_tests.rs"]
mod underline_app_tests;

#[path = "cross_run_app_tests.rs"]
mod cross_run_app_tests;

#[path = "selected_backspace_app_tests.rs"]
mod selected_backspace_app_tests;

#[path = "paragraph_alignment_app_tests.rs"]
mod paragraph_alignment_app_tests;

#[path = "select_all_app_tests.rs"]
mod select_all_app_tests;

#[path = "full_paragraph_backspace_app_tests.rs"]
mod full_paragraph_backspace_app_tests;
