//! Child of authored_text_ui_tests. Real full-App key/FIFO proposals.
//! The artifact author did not execute these tests. All FullOutput is dropped
//! before assertions; generated temp files only, never user original archives.
#![allow(deprecated)]
use super::*;
use mathfolio_document::rich_text_source::{TextFormat, TextSelection, Weight};

const ORIGINAL: &str = "가😀ABCD";
fn formatted() -> Fixture {
    let mut f = Fixture::new();
    f.frame(vec![egui::Event::Text(ORIGINAL.into())]);
    f.select(0, 2);
    let selection_key = f.app.rich_text_context().unwrap().selection_key;
    f.app.ribbon_command(
        &f.ctx,
        RibbonAction::SetTextBold {
            value: true,
            selection_key,
        },
    );
    f.select(4, 4);
    f
}
fn ctrl_a(f: &mut Fixture) {
    f.frame(vec![key(egui::Key::A, egui::Modifiers::COMMAND)]);
}
fn inserted_run(xml: &str, expected: &str) -> (usize, String) {
    let doc = roxmltree::Document::parse(xml).unwrap();
    let runs: Vec<_> = doc
        .descendants()
        .filter(|n| n.is_element() && n.tag_name().name() == "Run")
        .collect();
    let nonempty: Vec<_> = runs
        .iter()
        .enumerate()
        .filter(|(_, n)| n.children().filter_map(|c| c.text()).collect::<String>() == expected)
        .collect();
    assert_eq!(nonempty.len(), 1);
    let (id, node) = nonempty[0];
    (id, node.attribute("FontWeight").unwrap().into())
}
#[test]
fn real_ctrl_a_then_text_uses_current_normalized_start_format_and_one_history() {
    let mut f = formatted();
    let before = f.xml();
    let history = f.app.undo.len();
    let generation = f.app.background.submitted_generation();
    ctrl_a(&mut f);
    assert_eq!(f.text(), ORIGINAL);
    assert_eq!(f.xml(), before);
    assert_eq!(f.app.undo.len(), history);
    let selected = f.app.text_cursors[&f.id];
    assert_eq!(selected.primary.index.0.max(selected.secondary.index.0), 6);
    assert_eq!(selected.primary.index.0.min(selected.secondary.index.0), 0);
    f.frame(vec![egui::Event::Text("교체😀<&".into())]);
    assert_eq!(f.text(), "교체😀<&", "{}", f.app.status);
    assert_eq!(inserted_run(&f.xml(), "교체😀<&"), (0, "Bold".into()));
    assert_eq!(f.app.undo.len(), history + 1);
    assert_eq!(
        f.xml().matches("<Run ").count(),
        before.matches("<Run ").count()
    );
    f.frame(vec![egui::Event::Text("Z".into())]);
    assert_eq!(f.text(), "교체😀<&Z");
    assert_eq!(inserted_run(&f.xml(), "교체😀<&Z"), (0, "Bold".into()));
    assert_eq!(f.app.undo.len(), history + 2);
    assert!(f.app.last_executed_regions.is_empty());
    assert_eq!(f.app.background.submitted_generation(), generation);
}
#[test]
fn actual_fifo_keeps_ctrl_a_then_physical_key_and_text_separate_or_same_batch() {
    for same_batch in [false, true] {
        let mut f = formatted();
        let history = f.app.undo.len();
        let mut packet = vec![
            key(egui::Key::X, egui::Modifiers::NONE),
            egui::Event::Text("x".into()),
        ];
        if same_batch {
            packet.insert(0, key(egui::Key::A, egui::Modifiers::COMMAND));
        } else {
            ctrl_a(&mut f);
        }
        f.frame(packet);
        assert_eq!(f.text(), "x", "{}", f.app.status);
        assert_eq!(inserted_run(&f.xml(), "x"), (0, "Bold".into()));
        assert_eq!(f.app.undo.len(), history + 1);
    }
}
#[test]
fn leading_empty_run_keeps_its_bytes_but_does_not_own_replacement() {
    let mut f = Fixture::new();
    let s = source()
        .replace_in_run(0, 0, 0, "ABCD")
        .unwrap()
        .unwrap()
        .source;
    let s = s
        .set_selection_format(
            TextSelection {
                paragraph: 0,
                anchor: 2,
                focus: 4,
            },
            &TextFormat {
                weight: Some(Weight::Bold),
                ..Default::default()
            },
        )
        .unwrap()
        .unwrap()
        .source;
    let s = s.replace_in_run(0, 0, 2, "").unwrap().unwrap().source;
    let region = f
        .app
        .document
        .regions
        .iter_mut()
        .find(|r| r.id == f.id)
        .unwrap();
    region.content = CanvasContent::Text(s.plain_text().into());
    region.authored_text = Some(Box::new(s.wire().clone()));
    f.app.refresh_rich_text_visual(f.id);
    f.frame(vec![]);
    f.select(0, 0);
    ctrl_a(&mut f);
    f.frame(vec![egui::Event::Text("새".into())]);
    assert_eq!(f.text(), "새", "{}", f.app.status);
    assert_eq!(inserted_run(&f.xml(), "새"), (1, "Bold".into()));
    let xml = f.xml();
    let doc = roxmltree::Document::parse(&xml).unwrap();
    let first = doc
        .descendants()
        .find(|n| n.is_element() && n.tag_name().name() == "Run")
        .unwrap();
    assert_eq!(first.attribute("FontWeight"), Some("Normal"));
    assert_eq!(first.text().unwrap_or(""), "");
}
#[test]
fn same_plain_full_retyping_can_change_source_and_undo_redo_restores_exact_versions() {
    let mut f = formatted();
    let before = f.xml();
    let history = f.app.undo.len();
    ctrl_a(&mut f);
    f.frame(vec![egui::Event::Text(ORIGINAL.into())]);
    assert_eq!(f.text(), ORIGINAL);
    assert_ne!(f.xml(), before);
    assert_eq!(inserted_run(&f.xml(), ORIGINAL), (0, "Bold".into()));
    assert_eq!(f.app.undo.len(), history + 1);
    let typed = f.xml();
    f.app.undo();
    f.frame(vec![]);
    assert_eq!(f.xml(), before);
    f.app.redo();
    f.frame(vec![]);
    assert_eq!(f.xml(), typed);
    f.app.undo();
    f.frame(vec![]);
    // An old SelectAll token is not an Undo snapshot/serialized permission.
    f.select(0, 6);
    f.frame(vec![egui::Event::Text("NO".into())]);
    assert_eq!(f.xml(), before);
    assert_eq!(f.text(), ORIGINAL);
}
#[test]
fn unproven_full_range_and_noop_format_command_do_not_grant_typing() {
    let mut f = formatted();
    let before = f.xml();
    f.select(0, 6);
    f.frame(vec![egui::Event::Text("NO".into())]);
    assert_eq!(f.xml(), before);
    assert_eq!(f.text(), ORIGINAL);
    ctrl_a(&mut f);
    let selection_key = f.app.rich_text_context().unwrap().selection_key;
    f.app.ribbon_command(
        &f.ctx,
        RibbonAction::SetTextColor {
            rgba: [18, 52, 86, 255],
            selection_key,
        },
    ); // identical color still retires the command token
    assert_eq!(f.xml(), before);
    f.frame(vec![egui::Event::Text("NO".into())]);
    assert_eq!(f.xml(), before);
    assert_eq!(f.text(), ORIGINAL);
}
#[test]
fn actual_ime_and_paste_cannot_inherit_selectall_permission() {
    for ime in [false, true] {
        let mut f = formatted();
        ctrl_a(&mut f);
        let xml = f.xml();
        let history = f.app.undo.len();
        if ime {
            f.frame(vec![
                egui::Event::Ime(egui::ImeEvent::Enabled),
                egui::Event::Ime(egui::ImeEvent::Preedit {
                    text: "ㅎ".into(),
                    active_range_chars: None,
                }),
            ]);
            assert_eq!(f.text(), ORIGINAL);
            assert_eq!(f.xml(), xml);
            f.frame(vec![egui::Event::Ime(egui::ImeEvent::Commit("한".into()))]);
        } else {
            f.frame(vec![egui::Event::Paste("한".into())]);
        }
        assert_eq!(f.text(), ORIGINAL, "{}", f.app.status);
        assert_eq!(f.xml(), xml);
        assert_eq!(f.app.undo.len(), history);
    }
}
#[test]
fn generated_own_and_native_cold_open_preserve_replacement_and_execute_zero_math() {
    let mut f = formatted();
    ctrl_a(&mut f);
    f.frame(vec![egui::Event::Text("저장😀&".into())]);
    assert_eq!(f.text(), "저장😀&");
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let prefix = std::env::temp_dir().join(format!(
        "mathfolio-selectall-{}-{nonce}",
        std::process::id()
    ));
    let own = prefix.with_extension("mathfolio");
    let native = prefix.with_extension("mcdx");
    f.app.document.save(&own).unwrap();
    mathfolio_document::export::export_mcdx(&f.app.document, &native).unwrap();
    for path in [&own, &native] {
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
            .find(|r| matches!(&r.content,CanvasContent::Text(text) if text=="저장😀&"))
            .unwrap();
        assert!(cold.native_visuals.region(region).unwrap().is_ok());
        assert!(cold.last_executed_regions.is_empty());
        assert!(!cold.background.running);
        if path == &own {
            assert_eq!(region.authored_text, f.region().authored_text);
        } else {
            let package = NativePackage::open(path).unwrap();
            let source = mathfolio_document::export::read_native_text_authority(
                &package,
                region.source_region.as_ref().unwrap(),
            )
            .unwrap();
            assert_eq!(
                inserted_run(source.original_xaml(), "저장😀&"),
                (0, "Bold".into())
            );
        }
    }
    std::fs::remove_file(own).unwrap();
    std::fs::remove_file(native).unwrap();
}

#[test]
fn consumed_noop_history_command_cannot_leave_live_selectall_permission() {
    let mut f = formatted();
    ctrl_a(&mut f);
    let xml = f.xml();
    f.app.undo.clear();
    f.app.redo.clear();
    f.frame(vec![key(egui::Key::Z, egui::Modifiers::COMMAND)]);
    assert_eq!(f.xml(), xml);
    f.frame(vec![egui::Event::Text("NO".into())]);
    assert_eq!(f.xml(), xml);
    assert_eq!(f.text(), ORIGINAL);
}

#[test]
fn focus_loss_retires_permission_even_when_the_old_owner_is_not_typed() {
    let mut f = formatted();
    ctrl_a(&mut f);
    let xml = f.xml();
    let id = plain_text_input::widget_id(f.id);
    f.ctx.memory_mut(|m| m.surrender_focus(id));
    f.frame(vec![]);
    f.select(0, 6);
    f.frame(vec![egui::Event::Text("NO".into())]);
    assert_eq!(f.xml(), xml);
    assert_eq!(f.text(), ORIGINAL);
}
