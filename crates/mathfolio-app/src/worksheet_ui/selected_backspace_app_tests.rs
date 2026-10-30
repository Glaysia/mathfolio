//! Child of authored_text_ui_tests: actual full app frames, no target runtime.
//! Proposed tests only; the artifact author did not run Cargo.
#![allow(deprecated)]
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
fn restricted(f: &Fixture) -> bool {
    f.app.rich_text.authored_editors[&f.id]
        .as_ref()
        .unwrap()
        .affinity
        .is_backspace_continuation()
}
fn owner(f: &Fixture) -> Option<usize> {
    let e = f.app.rich_text.authored_editors[&f.id].as_ref().unwrap();
    e.affinity.owner(&e.source)
}
fn is_normal_inserted_x(f: &Fixture) -> bool {
    let xml = f.xml();
    let doc = roxmltree::Document::parse(&xml).unwrap();
    doc.descendants().any(|node| {
        node.is_element()
            && node.tag_name().name() == "Run"
            && node.attribute("FontWeight") == Some("Normal")
            && node.text() == Some("가AX")
    })
}

#[test]
fn actual_selected_backspace_then_ordinary_text_keeps_start_format_and_one_history_each() {
    for (anchor, focus) in [(2, 8), (8, 2)] {
        let mut f = formatted();
        f.select(anchor, focus);
        let source_before = f.xml();
        let history = f.app.undo.len();
        let generation = f.app.background.submitted_generation();
        f.frame(vec![key(egui::Key::Backspace, egui::Modifiers::NONE)]);
        assert_eq!(f.text(), DELETED, "{}", f.app.status);
        assert_eq!(f.app.undo.len(), history + 1);
        assert_eq!(owner(&f), Some(0));
        assert!(restricted(&f));
        f.frame(vec![egui::Event::Text("X".into())]);
        assert_eq!(f.text(), "가AXE꼬", "{}", f.app.status);
        assert_eq!(f.app.undo.len(), history + 2);
        assert!(is_normal_inserted_x(&f));
        assert_eq!(
            f.xml().matches("<Run ").count(),
            source_before.matches("<Run ").count()
        );
        assert!(!restricted(&f));
        assert!(f.app.last_executed_regions.is_empty());
        assert_eq!(f.app.background.submitted_generation(), generation);
        assert!(!f.app.background.running);
    }
}

#[test]
fn actual_delete_does_not_gain_backspace_owner_and_boundary_text_remains_atomic() {
    let mut f = formatted();
    f.select(2, 8);
    f.frame(vec![key(egui::Key::Delete, egui::Modifiers::NONE)]);
    assert_eq!(f.text(), DELETED);
    assert_eq!(owner(&f), None);
    assert!(!restricted(&f));
    let xml = f.xml();
    let history = f.app.undo.len();
    f.frame(vec![egui::Event::Text("X".into())]);
    assert_eq!(f.text(), DELETED);
    assert_eq!(f.xml(), xml);
    assert_eq!(f.app.undo.len(), history);
}

#[test]
fn observed_fifo_split_backspace_text_batch_can_publish_two_ordered_commands() {
    let mut f = formatted();
    f.select(8, 2);
    let history = f.app.undo.len();
    f.frame(vec![
        key(egui::Key::Backspace, egui::Modifiers::NONE),
        egui::Event::Text("X".into()),
    ]);
    assert_eq!(f.text(), "가AXE꼬", "{}", f.app.status);
    assert_eq!(f.app.undo.len(), history + 2);
    assert!(is_normal_inserted_x(&f));
    assert!(!restricted(&f));
}

fn unsplit_full_frame(f: &mut Fixture, events: Vec<egui::Event>) {
    f.frame += 1;
    let raw = egui::RawInput {
        events,
        time: Some(f64::from(f.frame)),
        screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1200., 900.))),
        ..Default::default()
    };
    // Deliberately bypass FIFO only to exercise one admitted widget delivery.
    // Full app UI still runs; no inference about ordinary platform batching.
    let output = f.ctx.run_ui(raw, |ui| f.app.app_ui_frame(ui));
    output.drop_without_applying_deltas();
}

#[test]
fn one_unsplit_mixed_delivery_cannot_forge_selected_backspace_continuation() {
    let mut f = formatted();
    f.select(2, 8);
    let xml = f.xml();
    let history = f.app.undo.len();
    unsplit_full_frame(
        &mut f,
        vec![
            key(egui::Key::Backspace, egui::Modifiers::NONE),
            egui::Event::Text("X".into()),
        ],
    );
    assert_eq!(f.text(), ORIGINAL, "{}", f.app.status);
    assert_eq!(f.xml(), xml);
    assert_eq!(f.app.undo.len(), history);
    assert!(!restricted(&f));
}

#[test]
fn actual_paste_cut_and_ime_retire_only_restricted_grant_and_do_not_inherit_it() {
    for operation in 0..3 {
        let mut f = formatted();
        f.select(2, 8);
        f.frame(vec![key(egui::Key::Backspace, egui::Modifiers::NONE)]);
        assert!(restricted(&f));
        let xml = f.xml();
        let history = f.app.undo.len();
        match operation {
            0 => f.frame(vec![egui::Event::Paste("X".into())]),
            1 => f.frame(vec![egui::Event::Cut]),
            _ => {
                f.frame(vec![egui::Event::Ime(egui::ImeEvent::Preedit {
                    text: "ㅎ".into(),
                    active_range_chars: None,
                })]);
                assert_eq!(f.xml(), xml);
                assert!(!restricted(&f));
                f.frame(vec![egui::Event::Ime(egui::ImeEvent::Commit("한".into()))]);
            }
        }
        assert_eq!(f.text(), DELETED, "operation={operation}: {}", f.app.status);
        assert_eq!(f.xml(), xml);
        assert_eq!(f.app.undo.len(), history);
        assert!(!restricted(&f));
        assert_eq!(owner(&f), None);
    }
}

#[test]
fn runtime_history_restores_restricted_source_bound_tag_then_fresh_typing_branches() {
    let mut f = formatted();
    f.select(8, 2);
    let before = f.xml();
    f.frame(vec![key(egui::Key::Backspace, egui::Modifiers::NONE)]);
    let deleted = f.xml();
    f.frame(vec![egui::Event::Text("X".into())]);
    let typed = f.xml();
    f.app.undo();
    f.frame(vec![]);
    assert_eq!(f.xml(), deleted);
    assert!(restricted(&f));
    assert_eq!(owner(&f), Some(0));
    f.app.redo();
    f.frame(vec![]);
    assert_eq!(f.xml(), typed);
    assert!(!restricted(&f));
    f.app.undo();
    f.frame(vec![]);
    f.frame(vec![egui::Event::Text("Y".into())]);
    assert_eq!(f.text(), "가AYE꼬", "{}", f.app.status);
    assert!(!restricted(&f));
    f.app.undo();
    f.app.undo();
    f.frame(vec![]);
    assert_eq!(f.xml(), before);
    assert_eq!(f.text(), ORIGINAL);
}

#[test]
fn actual_navigation_retires_grant_even_if_caret_returns_to_same_plain_boundary() {
    let mut f = formatted();
    f.select(2, 8);
    f.frame(vec![key(egui::Key::Backspace, egui::Modifiers::NONE)]);
    assert!(restricted(&f));
    f.frame(vec![
        key(egui::Key::ArrowLeft, egui::Modifiers::NONE),
        key(egui::Key::ArrowRight, egui::Modifiers::NONE),
    ]);
    assert!(!restricted(&f));
    let xml = f.xml();
    let history = f.app.undo.len();
    f.frame(vec![egui::Event::Text("X".into())]);
    assert_eq!(f.xml(), xml);
    assert_eq!(f.text(), DELETED);
    assert_eq!(f.app.undo.len(), history);
}
