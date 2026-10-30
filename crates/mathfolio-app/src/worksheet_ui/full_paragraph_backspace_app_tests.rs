//! Proposed actual full-App tests. Unrun artifact; no original runtime/window.
use super::*;

// Test-owned fresh directory. No sibling fixture visibility or clock-resolution
// uniqueness assumption; create_dir is exclusive and collisions are skipped.
struct FullBackspaceScratch(std::path::PathBuf);
impl FullBackspaceScratch {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        for _ in 0..128 {
            let sequence = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "mathfolio-full-backspace-test-{}-{sequence}",
                std::process::id()
            ));
            match std::fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("Create full Backspace test scratch: {error}"),
            }
        }
        panic!("Exhausted fresh full Backspace test scratch candidates");
    }
}
impl Drop for FullBackspaceScratch {
    fn drop(&mut self) {
        // This exact directory was exclusively created above by this helper.
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn setup(leading_empty: bool, mixed: bool) -> Fixture {
    let mut f = Fixture::new();
    f.frame(vec![egui::Event::Text(
        if leading_empty { "XABCD" } else { "ABCD" }.into(),
    )]);
    let offset = usize::from(leading_empty);
    f.select(offset, offset + 2);
    let selection_key = f.app.rich_text_context().unwrap().selection_key;
    f.app.ribbon_command(
        &f.ctx,
        RibbonAction::SetTextBold {
            value: true,
            selection_key,
        },
    );
    if !mixed {
        f.select(offset, offset + 2);
        let selection_key = f.app.rich_text_context().unwrap().selection_key;
        f.app.ribbon_command(
            &f.ctx,
            RibbonAction::SetTextBold {
                value: false,
                selection_key,
            },
        );
    }
    if leading_empty {
        f.select(0, 1);
        f.frame(vec![key(egui::Key::Delete, egui::Modifiers::NONE)]);
    }
    assert_eq!(f.text(), "ABCD");
    f
}
fn owner(f: &Fixture) -> Option<usize> {
    let editor = f.app.rich_text.authored_editors[&f.id].as_ref().unwrap();
    editor.affinity.owner(&editor.source)
}
fn restricted(f: &Fixture) -> bool {
    f.app.rich_text.authored_editors[&f.id]
        .as_ref()
        .unwrap()
        .affinity
        .is_backspace_continuation()
}
fn run_bodies(xml: &str) -> Vec<String> {
    let d = roxmltree::Document::parse(xml).unwrap();
    d.descendants()
        .filter(|n| n.tag_name().name() == "Run")
        .map(|n| n.children().filter_map(|x| x.text()).collect())
        .collect()
}
fn expected_empty(original: &str) -> String {
    original
        .replace(">X</Run>", "></Run>")
        .replace(">AB</Run>", "></Run>")
        .replace(">CD</Run>", "></Run>")
}
fn expected_x(original: &str) -> String {
    original
        .replace(">X</Run>", "></Run>")
        .replace(">AB</Run>", ">X</Run>")
        .replace(">CD</Run>", "></Run>")
}

#[test]
fn full_actual_ctrl_a_bs_then_physical_text_separate_and_fifo_keep_start_format_and_history() {
    for leading in [false, true] {
        for mixed in [false, true] {
            for batch in [false, true] {
                let mut f = setup(leading, mixed);
                let original = f.xml();
                let history = f.app.undo.len();
                let generation = f.app.background.submitted_generation();
                let signature = f.app.document.calculation_signature();
                let events = vec![
                    key(egui::Key::A, egui::Modifiers::COMMAND),
                    key(egui::Key::Backspace, egui::Modifiers::NONE),
                    key(egui::Key::X, egui::Modifiers::NONE),
                    egui::Event::Text("X".into()),
                ];
                if batch {
                    f.frame(events);
                } else {
                    f.frame(vec![events[0].clone()]);
                    f.frame(vec![events[1].clone()]);
                    assert_eq!(f.text(), "");
                    assert_eq!(f.xml(), expected_empty(&original));
                    assert_eq!(owner(&f), Some(usize::from(leading)));
                    assert!(restricted(&f));
                    f.frame(events[2..].to_vec());
                }
                assert_eq!(f.text(), "X", "{}", f.app.status);
                assert_eq!(f.xml(), expected_x(&original));
                assert_eq!(owner(&f), Some(usize::from(leading)));
                assert!(!restricted(&f));
                let bodies = run_bodies(&f.xml());
                assert_eq!(bodies[usize::from(leading)], "X");
                let actual = f
                    .region()
                    .validated_authored_text()
                    .unwrap()
                    .unwrap()
                    .creation_format_for_run(usize::from(leading))
                    .unwrap();
                assert_eq!(actual.weight, if mixed { "Bold" } else { "Normal" });
                assert_eq!(f.app.undo.len(), history + 2);
                f.app.undo();
                f.frame(vec![]);
                assert_eq!(f.xml(), expected_empty(&original));
                assert!(restricted(&f));
                f.app.undo();
                f.frame(vec![]);
                assert_eq!(f.xml(), original);
                f.app.redo();
                f.frame(vec![]);
                assert_eq!(f.xml(), expected_empty(&original));
                assert!(restricted(&f));
                f.app.redo();
                f.frame(vec![]);
                assert_eq!(f.xml(), expected_x(&original));
                assert_eq!(f.app.background.submitted_generation(), generation);
                assert_eq!(f.app.document.calculation_signature(), signature);
                assert!(f.app.last_executed_regions.is_empty() && !f.app.background.running);
            }
        }
    }
}

#[test]
fn full_delete_has_no_bs_grant_and_paste_ime_navigation_retire_existing_bs_grant() {
    for retire in 0..4 {
        let mut f = setup(true, true);
        f.frame(vec![key(egui::Key::A, egui::Modifiers::COMMAND)]);
        f.frame(vec![key(
            if retire == 0 {
                egui::Key::Delete
            } else {
                egui::Key::Backspace
            },
            egui::Modifiers::NONE,
        )]);
        assert_eq!(restricted(&f), retire != 0);
        let old = f.xml();
        let history = f.app.undo.len();
        match retire {
            1 => f.frame(vec![egui::Event::Paste("X".into())]),
            2 => {
                f.frame(vec![egui::Event::Ime(egui::ImeEvent::Preedit {
                    text: "ㅎ".into(),
                    active_range_chars: None,
                })]);
                f.frame(vec![egui::Event::Ime(egui::ImeEvent::Commit("한".into()))]);
            }
            3 => f.frame(vec![key(egui::Key::ArrowLeft, egui::Modifiers::NONE)]),
            _ => {}
        }
        assert!(!restricted(&f));
        f.frame(vec![egui::Event::Text("X".into())]);
        assert_eq!(f.xml(), old);
        assert_eq!(f.text(), "");
        assert_eq!(f.app.undo.len(), history);
    }
}

#[test]
fn full_bs_empty_own_native_cold_keeps_source_but_never_runtime_grant() {
    let mut f = setup(true, true);
    let original = f.xml();
    f.frame(vec![
        key(egui::Key::A, egui::Modifiers::COMMAND),
        key(egui::Key::Backspace, egui::Modifiers::NONE),
    ]);
    assert!(restricted(&f));
    let expected = expected_empty(&original);
    let scratch = FullBackspaceScratch::new();
    for name in ["full-bs-empty.mathfolio", "full-bs-empty.mcdx"] {
        let path = scratch.0.join(name);
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
        assert!(cold.status.starts_with("Opened"), "{}", cold.status);
        let id = cold
            .document
            .regions
            .iter()
            .find(|r| matches!(&r.content,CanvasContent::Text(t) if t.is_empty()))
            .unwrap()
            .id;
        cold.selected = Some(id);
        cold.focus_new = Some(id);
        cold.insertion = None;
        let ctx = egui::Context::default();
        crate::configure(&ctx);
        let mut cold = Fixture {
            app: cold,
            ctx,
            id,
            frame: 0,
        };
        cold.frame(vec![]);
        cold.select(0, 0);
        let n = cold.app.undo.len();
        let actual = if let Some(e) = cold
            .app
            .rich_text
            .authored_editors
            .get(&id)
            .and_then(|e| e.as_ref().ok())
        {
            assert!(!e.affinity.is_backspace_continuation());
            e.source.xaml()
        } else {
            cold.app.rich_text.editors[&id]
                .as_ref()
                .unwrap()
                .source
                .xaml()
        };
        assert_eq!(actual, expected);
        cold.frame(vec![egui::Event::Text("X".into())]);
        assert_eq!(cold.text(), "");
        assert_eq!(cold.app.undo.len(), n);
        assert!(cold.app.last_executed_regions.is_empty() && !cold.app.background.running);
    }
    // In-session Save preserves the existing BS-only lifetime; cold did not.
    assert!(restricted(&f));
    f.frame(vec![
        key(egui::Key::X, egui::Modifiers::NONE),
        egui::Event::Text("X".into()),
    ]);
    assert_eq!(f.xml(), expected_x(&original));
}
