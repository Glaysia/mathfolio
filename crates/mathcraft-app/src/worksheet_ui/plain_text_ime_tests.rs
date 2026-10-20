//! Headless tests for the plain CanvasContent::Text path.
//! Actual pre-fix failures and observations are retained under artifacts/rea.
//! No original Mathcad runtime/GUI. The expected preedit policy below is an
//! independent integration contract, not recovered native text-save behavior.
#![allow(deprecated)] // Exercise legacy platform Enabled/Disabled delivery too.
use super::*;

const BEFORE: &str = "앞😀뒤";
const COMMITTED: &str = "한😀뒤";

struct Fixture {
    app: WorksheetApp,
    context: egui::Context,
    id: u64,
    frame_index: u32,
}
impl Fixture {
    fn new() -> Self {
        let context = egui::Context::default();
        crate::configure(&context);
        let mut app = WorksheetApp {
            automatic_calculation: false,
            ..Default::default()
        };
        let id = app.document.add_text(38.4, 38.4, BEFORE);
        app.selected = Some(id);
        app.focus_new = Some(id);
        app.insertion = None;
        let mut fixture = Self {
            app,
            context,
            id,
            frame_index: 0,
        };
        fixture.frame(vec![]);
        // Exact Unicode-scalar selection in our current egui TextEdit. This
        // does not claim native WPF TextPointer offsets have the same units.
        let widget = egui::Id::new(("worksheet-text", id));
        let range =
            egui::text::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(1));
        let mut state = egui::text_edit::TextEditState::load(&fixture.context, widget)
            .expect("The real TextEdit must have been painted");
        state.cursor.set_char_range(Some(range));
        state.store(&fixture.context, widget);
        fixture.app.text_cursors.insert(id, range);
        fixture
            .context
            .memory_mut(|memory| memory.request_focus(widget));
        assert_eq!(fixture.text(), BEFORE);
        assert!(fixture.focused());
        fixture
    }
    fn text(&self) -> &str {
        text(&self.app.document, self.id)
    }
    fn focused(&self) -> bool {
        let widget = egui::Id::new(("worksheet-text", self.id));
        self.context
            .memory(|memory| memory.focused() == Some(widget))
    }
    fn frame(&mut self, events: Vec<egui::Event>) {
        let mut events = events;
        loop {
            self.frame_index += 1;
            let mut raw = egui::RawInput {
                events,
                time: Some(f64::from(self.frame_index)),
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1200., 900.))),
                ..Default::default()
            };
            // Same pre-InputState path as native eframe delivery; no GUI/waits.
            self.app.prepare_plot_input(&self.context, &mut raw);
            let output = self.context.run_ui(raw, |ui| self.app.page_ui(ui));
            output.drop_without_applying_deltas();
            if self.app.plot_input_queue.is_empty() {
                break;
            }
            events = vec![];
        }
    }
    fn ime(&mut self, event: egui::ImeEvent) {
        self.frame(vec![egui::Event::Ime(event)]);
    }
    fn begin(&mut self) {
        self.ime(egui::ImeEvent::Enabled);
    }
    fn state(&self, label: &str) -> serde_json::Value {
        serde_json::json!({
            "step":label,
            "source":self.text(),
            "undo_len":self.app.undo.len(),
            "redo_len":self.app.redo.len(),
            "dirty":self.app.dirty,
            "selected":self.app.selected,
            "editing_region":self.app.editing_region,
            "focused":self.focused(),
            "cursor":self.app.text_cursors.get(&self.id).map(|range|format!("{range:?}")),
            "last_executed_regions":self.app.last_executed_regions,
            "submitted_generation":self.app.background.submitted_generation(),
        })
    }
    fn save_state(&mut self, directory: &Path, name: &str) -> serde_json::Value {
        let path = directory.join(name);
        self.app.path = Some(path.clone());
        let accepted = self.app.save(false); // Real app Save, no file chooser.
        let saved = if path.exists() {
            let document = CanvasDocument::load(&path).expect("Saved own document must parse");
            Some(text(&document, self.id).to_owned())
        } else {
            None
        };
        serde_json::json!({
            "file":name,"save_accepted":accepted,"disk_source":saved,
            "memory_source":self.text(),"undo_len":self.app.undo.len(),
            "status":self.app.status,
        })
    }
}
fn text(document: &CanvasDocument, id: u64) -> &str {
    let region = document
        .regions
        .iter()
        .find(|region| region.id == id)
        .unwrap();
    let CanvasContent::Text(value) = &region.content else {
        panic!("Expected actual plain text")
    };
    value
}
fn preedit(value: &str) -> egui::ImeEvent {
    egui::ImeEvent::Preedit {
        text: value.into(),
        active_range_chars: (!value.is_empty()).then_some(0..value.chars().count()),
    }
}
fn temporary_output() -> PathBuf {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let path = std::env::temp_dir().join(format!(
        "mathcraft-plain-text-ime-{}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&path).unwrap();
    path
}

#[test]
#[ignore = "Observation proposal: actual CanvasText IME/source/Save/Undo, not an original runtime"]
fn plain_text_ime_observation_probe() {
    let output = PathBuf::from(
        std::env::var_os("MATHCRAFT_PLAIN_TEXT_IME_OUTPUT")
            .expect("MATHCRAFT_PLAIN_TEXT_IME_OUTPUT"),
    );
    std::fs::create_dir_all(&output).unwrap();
    let mut reports = vec![];

    let mut fixture = Fixture::new();
    let initial = fixture.state("selected-before");
    fixture.begin();
    let mut steps = vec![initial, fixture.state("enabled")];
    for value in ["ㅎ", "하"] {
        let event = egui::Event::Ime(preedit(value));
        steps.push(serde_json::json!({
            "step":"intent","event":format!("{event:?}"),
            "may_edit":input_intent::may_edit(&event),
        }));
        fixture.frame(vec![event]);
        steps.push(fixture.state("preedit"));
    }
    let save = fixture.save_state(&output, "during-preedit.mathcraft");
    fixture.ime(egui::ImeEvent::Commit("한".into()));
    steps.push(fixture.state("commit"));
    let committed_save = fixture.save_state(&output, "after-commit.mathcraft");
    fixture
        .app
        .ribbon_command(&fixture.context, RibbonAction::Undo);
    steps.push(fixture.state("worksheet-undo"));
    fixture
        .app
        .ribbon_command(&fixture.context, RibbonAction::Redo);
    steps.push(fixture.state("worksheet-redo"));
    reports.push(serde_json::json!({
        "scenario":"selected-prefix-commit",
        "steps":steps,"preedit_save":save,"commit_save":committed_save,
    }));

    for (name, cancel) in [
        ("empty-preedit", preedit("")),
        ("empty-commit", egui::ImeEvent::Commit(String::new())),
        ("disabled", egui::ImeEvent::Disabled),
    ] {
        let mut fixture = Fixture::new();
        fixture.begin();
        fixture.ime(preedit("하"));
        let before = fixture.state("pre-cancel");
        fixture.ime(cancel);
        let after = fixture.state("post-cancel");
        let save = fixture.save_state(&output, &format!("cancel-{name}.mathcraft"));
        fixture
            .app
            .ribbon_command(&fixture.context, RibbonAction::Undo);
        reports.push(serde_json::json!({
            "scenario":name,"before":before,"after":after,"save":save,
            "after_undo":fixture.state("cancel-undo"),
        }));
    }
    let report = serde_json::json!({
        "executed_original_runtime":false,"used_gui":false,
        "policy":"Observation only; compare independently proposed committed-source invariants",
        "initial":BEFORE,"expected_after_commit":COMMITTED,"scenarios":reports,
    });
    std::fs::write(
        output.join("observation.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
    println!("PLAIN_TEXT_IME_OBSERVATION scenarios=4 original_runtime=false gui=false");
}

/// Proposed contract: view preedit must not become committed source or a saved
/// own-document payload. Save may refuse while composition is active, or save
/// the previous committed source; the test does not prescribe either UI choice.
#[test]
fn plain_text_preedit_keeps_committed_source_history_and_saved_payload() {
    let directory = temporary_output();
    let mut fixture = Fixture::new();
    let undo = fixture.app.undo.len();
    fixture.begin();
    fixture.ime(preedit("하"));
    let source = fixture.text().to_owned();
    let after_undo = fixture.app.undo.len();
    let save = fixture.save_state(&directory, "preedit.mathcraft");
    if directory.join("preedit.mathcraft").exists() {
        std::fs::remove_file(directory.join("preedit.mathcraft")).unwrap();
    }
    std::fs::remove_dir(&directory).unwrap();
    assert_eq!(
        source, BEFORE,
        "Preedit is a view composition, not committed source"
    );
    assert_eq!(
        after_undo, undo,
        "Preedit alone must not create invisible Undo steps"
    );
    if save["save_accepted"] == true {
        assert_eq!(save["disk_source"], BEFORE);
    } else {
        assert!(
            save["disk_source"].is_null(),
            "Refused Save must not write transient source"
        );
    }
    assert!(fixture.app.last_executed_regions.is_empty());
}

#[test]
fn plain_text_commit_is_one_owner_edit_and_undo_restores_precomposition_selection_source() {
    let mut fixture = Fixture::new();
    let undo = fixture.app.undo.len();
    let before_cursor = fixture.app.text_cursors[&fixture.id];
    fixture.begin();
    fixture.ime(preedit("ㅎ"));
    fixture.ime(preedit("하"));
    fixture.ime(egui::ImeEvent::Commit("한".into()));
    let committed = fixture.text().to_owned();
    let committed_undo = fixture.app.undo.len();
    let committed_cursor = fixture.app.text_cursors[&fixture.id];
    fixture
        .app
        .ribbon_command(&fixture.context, RibbonAction::Undo);
    let reverted = fixture.text().to_owned();
    let reverted_cursor = fixture.app.text_cursors[&fixture.id];
    fixture
        .app
        .ribbon_command(&fixture.context, RibbonAction::Redo);
    let redone = fixture.text().to_owned();
    assert_eq!(committed, COMMITTED);
    assert_eq!(committed_undo, undo + 1);
    assert_eq!(
        reverted, BEFORE,
        "Undo must not resurrect a transient Korean preedit"
    );
    assert_eq!(redone, COMMITTED);
    assert_eq!(reverted_cursor, before_cursor);
    assert_eq!(fixture.app.text_cursors[&fixture.id], committed_cursor);
    assert!(fixture.app.last_executed_regions.is_empty());
}

#[test]
fn plain_text_cancel_restores_selected_committed_text_without_a_hidden_delete() {
    for cancellation in [
        preedit(""),
        egui::ImeEvent::Commit(String::new()),
        egui::ImeEvent::Disabled,
    ] {
        let mut fixture = Fixture::new();
        let undo = fixture.app.undo.len();
        fixture.begin();
        fixture.ime(preedit("하"));
        fixture.ime(cancellation);
        let source = fixture.text().to_owned();
        let after_undo = fixture.app.undo.len();
        assert_eq!(
            source, BEFORE,
            "Independent cancellation policy must preserve selected source"
        );
        assert_eq!(after_undo, undo);
        assert!(fixture.app.last_executed_regions.is_empty());
    }
}

#[test]
fn a_batched_commit_and_next_preedit_publishes_only_completed_syllables() {
    let mut fixture = Fixture::new();
    fixture.begin();
    fixture.frame(vec![
        egui::Event::Ime(preedit("ㅎ")),
        egui::Event::Ime(egui::ImeEvent::Commit("한".into())),
        egui::Event::Ime(preedit("ㄱ")),
    ]);
    assert_eq!(fixture.text(), COMMITTED);
    assert_eq!(fixture.app.undo.len(), 1);
    assert!(fixture.app.text_composition.is_some());
    fixture.ime(preedit("그"));
    assert_eq!(fixture.text(), COMMITTED);
    fixture.ime(egui::ImeEvent::Commit("글".into()));
    assert_eq!(fixture.text(), "한글😀뒤");
    assert_eq!(fixture.app.undo.len(), 2);
    fixture
        .app
        .ribbon_command(&fixture.context, RibbonAction::Undo);
    assert_eq!(fixture.text(), COMMITTED);
    fixture
        .app
        .ribbon_command(&fixture.context, RibbonAction::Undo);
    assert_eq!(fixture.text(), BEFORE);
}

#[test]
fn saving_during_preedit_does_not_reset_the_composition_cursor_or_commit_it() {
    let directory = temporary_output();
    let mut fixture = Fixture::new();
    fixture.begin();
    fixture.ime(preedit("ㅎ"));
    assert_eq!(
        fixture.save_state(&directory, "before.mathcraft")["disk_source"],
        BEFORE
    );
    fixture.ime(preedit("하"));
    fixture.ime(egui::ImeEvent::Commit("한".into()));
    assert_eq!(fixture.text(), COMMITTED);
    assert_eq!(fixture.app.undo.len(), 1);
    assert_eq!(
        fixture.save_state(&directory, "after.mathcraft")["disk_source"],
        COMMITTED
    );
    for name in ["before.mathcraft", "after.mathcraft"] {
        std::fs::remove_file(directory.join(name)).unwrap();
    }
    std::fs::remove_dir(directory).unwrap();
}

#[test]
fn focus_loss_and_ordinary_paste_cancel_preedit_without_losing_selected_source() {
    let mut fixture = Fixture::new();
    fixture.begin();
    fixture.ime(preedit("하"));
    fixture
        .context
        .memory_mut(|memory| memory.surrender_focus(egui::Id::new(("worksheet-text", fixture.id))));
    fixture.frame(vec![]);
    assert_eq!(fixture.text(), BEFORE);
    assert!(fixture.app.text_composition.is_none());
    assert!(fixture.app.undo.is_empty() && !fixture.app.dirty);

    let mut fixture = Fixture::new();
    fixture.begin();
    fixture.ime(preedit("하"));
    fixture.frame(vec![egui::Event::Paste("가나다".into())]);
    assert_eq!(fixture.text(), "가나다😀뒤");
    assert!(fixture.app.text_composition.is_none());
    fixture
        .app
        .ribbon_command(&fixture.context, RibbonAction::Undo);
    assert_eq!(fixture.text(), BEFORE);
    // A following frame must not revive a discarded widget composition.
    fixture.frame(vec![]);
    fixture.ime(egui::ImeEvent::Commit("나".into()));
    assert_eq!(fixture.text(), "나😀뒤");
}

#[test]
fn text_enter_and_shift_enter_insert_newlines_without_calculation_or_new_regions() {
    for modifiers in [egui::Modifiers::NONE, egui::Modifiers::SHIFT] {
        let mut fixture = Fixture::new();
        fixture.frame(vec![egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }]);
        assert_eq!(fixture.text(), "\n😀뒤");
        assert_eq!(fixture.app.document.regions.len(), 1);
        assert!(fixture.app.last_executed_regions.is_empty());
        assert!(fixture.app.uncommitted_math.is_empty());
        fixture
            .app
            .ribbon_command(&fixture.context, RibbonAction::Undo);
        assert_eq!(fixture.text(), BEFORE);
    }
}

#[test]
fn same_frame_cancel_and_owner_undo_cannot_revive_preedit_or_clear_redo() {
    for cancellation in [
        preedit(""),
        egui::ImeEvent::Commit(String::new()),
        egui::ImeEvent::Disabled,
    ] {
        let mut fixture = Fixture::new();
        fixture.frame(vec![egui::Event::Text("값".into())]);
        fixture
            .app
            .ribbon_command(&fixture.context, RibbonAction::Undo);
        fixture.frame(vec![]);
        assert_eq!(fixture.app.redo.len(), 1);
        fixture.frame(vec![
            egui::Event::Ime(preedit("하")),
            egui::Event::Ime(cancellation),
        ]);
        assert_eq!(fixture.text(), BEFORE);
        assert_eq!((fixture.app.undo.len(), fixture.app.redo.len()), (0, 1));
        fixture.ime(preedit("ㅎ"));
        fixture
            .app
            .ribbon_command(&fixture.context, RibbonAction::Undo);
        fixture.frame(vec![]);
        assert_eq!(fixture.text(), BEFORE);
        assert!(fixture.app.text_composition.is_none());
        fixture
            .app
            .ribbon_command(&fixture.context, RibbonAction::Redo);
        assert_eq!(fixture.text(), "값😀뒤");
    }
}

#[test]
fn redundant_cancel_signals_end_composition_and_commit_then_cancel_preserves_the_prefix() {
    let mut fixture = Fixture::new();
    fixture.frame(vec![
        egui::Event::Ime(preedit("ㅎ")),
        egui::Event::Ime(preedit("")),
        egui::Event::Ime(egui::ImeEvent::Commit(String::new())),
    ]);
    assert_eq!(fixture.text(), BEFORE);
    assert!(fixture.app.text_composition.is_none());
    assert!(fixture.app.undo.is_empty() && !fixture.app.dirty);
    fixture.frame(vec![
        egui::Event::Ime(preedit("ㅎ")),
        egui::Event::Ime(egui::ImeEvent::Commit("한".into())),
        egui::Event::Ime(preedit("그")),
        egui::Event::Ime(egui::ImeEvent::Disabled),
    ]);
    assert_eq!(fixture.text(), COMMITTED);
    assert!(fixture.app.text_composition.is_none());
    assert!(fixture.app.text_cursors[&fixture.id].is_empty());
    fixture.frame(vec![egui::Event::Text("글".into())]);
    assert_eq!(fixture.text(), "한글😀뒤");
}

#[test]
fn reopening_a_different_document_cannot_reuse_a_same_id_composition() {
    let directory = temporary_output();
    let path = directory.join("other.mathcraft");
    let mut document = CanvasDocument::default();
    let id = document.add_text(38.4, 38.4, "다른 문서");
    document.save(&path).unwrap();
    let mut fixture = Fixture::new();
    assert_eq!(fixture.id, id);
    fixture.ime(preedit("하"));
    assert!(fixture.app.text_composition.is_some());
    fixture.app.open(&path);
    assert!(fixture.app.text_composition.is_none());
    fixture.app.selected = Some(id);
    fixture.app.focus_new = Some(id);
    fixture.frame(vec![]);
    assert_eq!(fixture.text(), "다른 문서");
    assert!(fixture.app.undo.is_empty());
    fixture.ime(preedit(""));
    assert_eq!(fixture.text(), "다른 문서");
    assert!(fixture.app.undo.is_empty());
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(directory).unwrap();
}

#[test]
fn ordinary_text_before_preedit_and_after_cancel_survives_the_ordered_input_delivery() {
    let mut fixture = Fixture::new();
    fixture.frame(vec![
        egui::Event::Text("값".into()),
        egui::Event::Ime(preedit("하")),
    ]);
    assert_eq!(fixture.text(), "값😀뒤");
    fixture.ime(preedit(""));
    assert_eq!(fixture.text(), "값😀뒤");
    assert_eq!(fixture.app.undo.len(), 1);
    fixture
        .app
        .ribbon_command(&fixture.context, RibbonAction::Undo);
    assert_eq!(fixture.text(), BEFORE);

    let mut fixture = Fixture::new();
    fixture.ime(preedit("하"));
    fixture.frame(vec![
        egui::Event::Ime(egui::ImeEvent::Commit(String::new())),
        egui::Event::Text("값".into()),
    ]);
    assert_eq!(fixture.text(), "값😀뒤");
    assert_eq!(fixture.app.undo.len(), 1);
    fixture
        .app
        .ribbon_command(&fixture.context, RibbonAction::Undo);
    assert_eq!(fixture.text(), BEFORE);
}

#[test]
fn selection_before_preedit_and_arrow_after_commit_never_publish_the_composing_text() {
    let mut fixture = Fixture::new();
    fixture.frame(vec![
        egui::Event::Key {
            key: egui::Key::End,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::SHIFT,
        },
        egui::Event::Ime(preedit("하")),
    ]);
    fixture.ime(preedit(""));
    assert_eq!(fixture.text(), BEFORE);
    assert!(fixture.app.undo.is_empty());

    let mut fixture = Fixture::new();
    fixture.frame(vec![
        egui::Event::Ime(egui::ImeEvent::Commit("한".into())),
        egui::Event::Ime(preedit("그")),
        egui::Event::Key {
            key: egui::Key::ArrowLeft,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        },
    ]);
    assert_eq!(fixture.text(), COMMITTED);
    assert!(fixture.app.text_composition.is_none());
    fixture.frame(vec![egui::Event::Text("x".into())]);
    assert_eq!(fixture.text(), "x한😀뒤");
}

#[test]
fn surrounding_deletion_is_committed_while_selected_source_and_preedit_remain_distinct() {
    let mut fixture = Fixture::new();
    fixture.ime(preedit("하"));
    fixture.ime(egui::ImeEvent::DeleteSurrounding {
        before_chars: 0,
        after_chars: 1,
    });
    assert_eq!(fixture.text(), "앞뒤");
    assert_eq!(fixture.app.undo.len(), 1);
    fixture.ime(preedit(""));
    assert_eq!(fixture.text(), "앞뒤");
    fixture
        .app
        .ribbon_command(&fixture.context, RibbonAction::Undo);
    assert_eq!(fixture.text(), BEFORE);

    let mut fixture = Fixture::new();
    fixture.frame(vec![
        egui::Event::Ime(preedit("하")),
        egui::Event::Ime(egui::ImeEvent::DeleteSurrounding {
            before_chars: 0,
            after_chars: 1,
        }),
        egui::Event::Ime(egui::ImeEvent::Commit("한".into())),
    ]);
    assert_eq!(fixture.text(), "한뒤");
    assert_eq!(fixture.app.undo.len(), 2);
    fixture
        .app
        .ribbon_command(&fixture.context, RibbonAction::Undo);
    assert_eq!(fixture.text(), "앞뒤");
    fixture
        .app
        .ribbon_command(&fixture.context, RibbonAction::Undo);
    assert_eq!(fixture.text(), BEFORE);
}

#[test]
fn window_focus_loss_discards_only_the_unconfirmed_composition() {
    let mut fixture = Fixture::new();
    fixture.ime(preedit("하"));
    fixture.frame(vec![egui::Event::WindowFocused(false)]);
    assert_eq!(fixture.text(), BEFORE);
    assert!(fixture.app.text_composition.is_none());
    assert!(fixture.app.undo.is_empty() && !fixture.app.dirty);
}
