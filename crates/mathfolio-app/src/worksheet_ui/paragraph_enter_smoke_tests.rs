//! Actual TextEdit + input queue + source + owner history.
use super::*;

fn paragraphs(xml: &str) -> usize {
    roxmltree::Document::parse(xml)
        .unwrap()
        .descendants()
        .filter(|n| n.is_element() && n.tag_name().name() == "Paragraph")
        .count()
}

#[test]
fn repeated_plain_enter_keeps_paragraph_source_following_typing_and_owner_history() {
    let mut f = Fixture::new();
    f.select(2, 2);
    let history = f.app.undo.len();
    let generation = f.app.background.submitted_generation();
    f.frame(vec![key(egui::Key::Enter, egui::Modifiers::NONE)]);
    assert_eq!(f.text(), "LC\nCL-S 시스템 설계", "{}", f.app.status);
    assert_eq!(paragraphs(&f.xml()), 2);
    assert_eq!(f.app.undo.len(), history + 1);
    f.frame(vec![egui::Event::Text("한😀".into())]);
    assert_eq!(f.text(), "LC\n한😀CL-S 시스템 설계", "{}", f.app.status);
    let typed = f.xml();
    f.select(6, 6);
    f.frame(vec![key(egui::Key::Enter, egui::Modifiers::NONE)]);
    let split = f.xml();
    assert_eq!(paragraphs(&split), 3);
    assert_eq!(f.text(), "LC\n한😀C\nL-S 시스템 설계", "{}", f.app.status);
    f.frame(vec![egui::Event::Text("Z".into())]);
    assert_eq!(f.text(), "LC\n한😀C\nZL-S 시스템 설계", "{}", f.app.status);
    assert_eq!(f.app.undo.len(), history + 4);
    f.app.undo();
    assert_eq!(f.xml(), split);
    f.app.undo();
    assert_eq!(f.xml(), typed);
    f.app.redo();
    assert_eq!(f.xml(), split);
    f.frame(vec![egui::Event::Text("Y".into())]);
    assert_eq!(f.text(), "LC\n한😀C\nYL-S 시스템 설계", "{}", f.app.status);
    assert_eq!(f.app.background.submitted_generation(), generation);
    assert!(f.app.last_executed_regions.is_empty());
}

#[test]
fn unsupported_enter_and_newline_commands_leave_source_cursor_and_history_atomic() {
    for (anchor, focus, event) in [
        (1, 3, key(egui::Key::Enter, egui::Modifiers::NONE)),
        (3, 1, key(egui::Key::Enter, egui::Modifiers::NONE)),
        (0, 0, key(egui::Key::Enter, egui::Modifiers::NONE)),
        (5, 5, key(egui::Key::Enter, egui::Modifiers::NONE)),
        (2, 2, key(egui::Key::Enter, egui::Modifiers::SHIFT)),
        (2, 2, egui::Event::Paste("A\nB".into())),
    ] {
        let mut f = Fixture::new();
        f.select(anchor, focus);
        let source = f.xml();
        let cursor = f.app.text_cursors[&f.id];
        let before = serde_json::to_vec(&f.app.document).unwrap();
        let history = (f.app.undo.len(), f.app.redo.len(), f.app.dirty);
        f.frame(vec![event]);
        assert_eq!(f.xml(), source);
        assert_eq!(serde_json::to_vec(&f.app.document).unwrap(), before);
        assert_eq!(f.app.text_cursors[&f.id], cursor);
        assert_eq!((f.app.undo.len(), f.app.redo.len(), f.app.dirty), history);
        assert!(f.app.last_executed_regions.is_empty());
    }
}

#[test]
fn cross_paragraph_backspace_cannot_flatten_the_source_structure() {
    let mut f = Fixture::new();
    f.select(2, 2);
    f.frame(vec![key(egui::Key::Enter, egui::Modifiers::NONE)]);
    let source = f.xml();
    let cursor = f.app.text_cursors[&f.id];
    let history = f.app.undo.len();
    f.frame(vec![key(egui::Key::Backspace, egui::Modifiers::NONE)]);
    assert_eq!(f.xml(), source);
    assert_eq!(f.app.text_cursors[&f.id], cursor);
    assert_eq!(f.app.undo.len(), history);
}
