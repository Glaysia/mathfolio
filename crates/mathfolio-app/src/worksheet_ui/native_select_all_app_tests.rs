//! Child of rich_text_integration_tests. Exact full-App/FIFO, not UI mirrors.
#![allow(deprecated)]
use super::*;
use rich_text_edit_map::GlobalSelection;
use std::io::Read;

fn fixture(explicit_none: bool) -> Fixture {
    let raw = include_str!("../../../../tests/reference/rich-text/sample-01.xaml");
    let doc = roxmltree::Document::parse(raw).unwrap();
    let paragraph = doc
        .descendants()
        .find(|n| n.is_element() && n.tag_name().name() == "Paragraph")
        .unwrap();
    let runs: Vec<_> = paragraph.children().filter(|n| n.is_element()).collect();
    let decorations = if explicit_none {
        " TextDecorations=\"None\""
    } else {
        ""
    };
    let body = format!(
        "<Run style=\"Normal\" FontStyle=\"Italic\" TextDecorations=\"Underline\"></Run><Run style=\"Normal\" FontWeight=\"Normal\"{decorations}>LCCL</Run><Run style=\"Normal\" FontWeight=\"Bold\" TextDecorations=\"Underline\">-S 시스템 설계</Run>"
    );
    let mut xml = raw.to_owned();
    xml.replace_range(
        runs.first().unwrap().range().start..runs.last().unwrap().range().end,
        &body,
    );
    let package = replaced_package_xaml(&xml);
    let mut f = Fixture::with_package(Some(&package));
    frame(&mut f, vec![]);
    f.select(2, 2);
    f
}
fn frame(f: &mut Fixture, events: Vec<egui::Event>) {
    let mut events = events;
    for _ in 0..128 {
        f.frame += 1;
        let mut raw = egui::RawInput {
            events,
            time: Some(f64::from(f.frame)),
            screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1200., 900.))),
            ..Default::default()
        };
        f.app.prepare_plot_input(&f.ctx, &mut raw);
        let out = f.ctx.run_ui(raw, |ui| f.app.app_ui_frame(ui));
        out.drop_without_applying_deltas();
        if f.app.plot_input_queue.is_empty() {
            return;
        }
        events = vec![];
    }
    panic!("Native full-App FIFO did not drain");
}
fn select_all(f: &mut Fixture) {
    frame(f, vec![key(egui::Key::A, egui::Modifiers::COMMAND)]);
}
fn active(f: &Fixture) -> bool {
    f.app.rich_text.editors[&f.id]
        .as_ref()
        .unwrap()
        .has_pending_select_all()
}
fn run_parts(xml: &str) -> Vec<(String, String, String)> {
    let doc = roxmltree::Document::parse(xml).unwrap();
    doc.descendants()
        .filter(|n| n.is_element() && n.tag_name().name() == "Run")
        .map(|n| {
            let raw = &xml[n.range()];
            let end = raw.find('>').unwrap() + 1;
            let close = raw.rfind("</").unwrap();
            (
                raw[..end].to_owned(),
                n.children().filter_map(|n| n.text()).collect(),
                raw[close..].to_owned(),
            )
        })
        .collect()
}
fn assert_owner(f: &Fixture, expected: &str) {
    let parts = run_parts(&f.xml());
    assert_eq!(parts.len(), 3);
    assert_eq!(parts[0].1, "");
    assert_eq!(parts[1].1, expected);
    assert_eq!(parts[2].1, "");
    assert!(parts[1].0.contains("FontWeight=\"Normal\""));
}

#[test]
fn native_actual_ctrl_a_unknown_and_mixed_underline_use_first_nonempty_raw_owner() {
    for known in [false, true] {
        let mut f = fixture(known);
        let before = f.xml();
        let raw = run_parts(&before);
        let history = f.app.undo.len();
        let generation = f.app.background.submitted_generation();
        select_all(&mut f);
        assert!(active(&f));
        frame(&mut f, vec![egui::Event::Text("한글😀<&".into())]);
        assert_eq!(f.text(), "한글😀<&", "{}", f.app.status);
        assert_owner(&f, "한글😀<&");
        for (old, new) in raw.iter().zip(run_parts(&f.xml())) {
            assert_eq!(old.0, new.0);
            assert_eq!(old.2, new.2);
        }
        assert_eq!(f.app.undo.len(), history + 1);
        assert_eq!(run_parts(&f.xml())[1].0.contains("TextDecorations"), known);
        frame(&mut f, vec![egui::Event::Text("Z".into())]);
        assert_owner(&f, "한글😀<&Z");
        assert_eq!(f.app.undo.len(), history + 2);
        assert_eq!(f.app.background.submitted_generation(), generation);
        assert!(f.app.last_executed_regions.is_empty() && !f.app.background.running);
    }
}

#[test]
fn native_same_plain_replacement_one_undo_own_native_cold_source_and_resources() {
    let mut f = fixture(false);
    let before = f.xml();
    let history = f.app.undo.len();
    let generation = f.app.background.submitted_generation();
    select_all(&mut f);
    frame(&mut f, vec![egui::Event::Text(ORIGINAL.into())]);
    assert_eq!(f.text(), ORIGINAL);
    assert_ne!(f.xml(), before);
    assert_owner(&f, ORIGINAL);
    assert_eq!(f.app.undo.len(), history + 1);
    let changed = f.xml();
    f.app.undo();
    assert_eq!(f.xml(), before);
    f.app.redo();
    assert_eq!(f.xml(), changed);
    let source = &f.app.rich_text.editors[&f.id].as_ref().unwrap().source;
    assert!(source.has_original_raw_body_only_delta());
    assert!(source.prove_format_only().is_err());
    let native_before = f.app.document.native_archive.clone().unwrap();
    let package = NativePackage::from_bytes(&native_before).unwrap();
    let own = f.save("native-select-all.mathfolio");
    let native = f.root.join("native-select-all.mcdx");
    mathfolio_document::export::export_mcdx(&f.app.document, &native).unwrap();
    assert_eq!(
        f.app.document.native_archive.as_deref(),
        Some(native_before.as_slice())
    );
    for path in [&own, &native] {
        let mut cold = WorksheetApp {
            automatic_calculation: false,
            ..Default::default()
        };
        cold.open(path);
        assert!(cold.status.starts_with("Opened"), "{}", cold.status);
        let id = cold
            .document
            .regions
            .iter()
            .find(|r| matches!(&r.content,CanvasContent::Text(t)if t==ORIGINAL))
            .unwrap()
            .id;
        cold.selected = Some(id);
        cold.prepare_rich_text();
        assert_eq!(
            cold.rich_text.editors[&id].as_ref().unwrap().source.xaml(),
            changed
        );
        assert!(cold.last_executed_regions.is_empty() && !cold.background.running);
    }
    let output = NativePackage::open(&native).unwrap();
    assert_eq!(
        output.original_part("mathcad/settings/presentation.xml"),
        package.original_part("mathcad/settings/presentation.xml")
    );
    let owner = f
        .app
        .document
        .regions
        .iter()
        .find(|r| r.id == f.id)
        .unwrap()
        .source_region
        .as_ref()
        .unwrap();
    let old = mathfolio_document::export::read_native_text_authority(&package, owner).unwrap();
    let new = mathfolio_document::export::read_native_text_authority(&output, owner).unwrap();
    let mut oldzip = ZipArchive::new(Cursor::new(old.original_package())).unwrap();
    let mut newzip = ZipArchive::new(Cursor::new(new.original_package())).unwrap();
    for i in 0..oldzip.len() {
        let mut a = oldzip.by_index(i).unwrap();
        let name = a.name().unwrap().into_owned();
        if name == "Xaml/Document.xaml" {
            continue;
        }
        let mut b = newzip.by_name(&name).unwrap();
        let mut av = vec![];
        let mut bv = vec![];
        a.read_to_end(&mut av).unwrap();
        b.read_to_end(&mut bv).unwrap();
        assert_eq!(av, bv, "{name}");
    }
    assert_eq!(f.app.background.submitted_generation(), generation);
    assert!(f.app.last_executed_regions.is_empty());
}

#[test]
fn native_prefix_ctrl_a_physical_key_text_fifo_has_exact_source_transactions() {
    for combined in [false, true] {
        let mut f = fixture(false);
        let history = f.app.undo.len();
        if combined {
            frame(
                &mut f,
                vec![
                    egui::Event::Text("P".into()),
                    key(egui::Key::A, egui::Modifiers::COMMAND),
                    key(egui::Key::X, egui::Modifiers::NONE),
                    egui::Event::Text("x".into()),
                ],
            );
        } else {
            frame(&mut f, vec![egui::Event::Text("P".into())]);
            select_all(&mut f);
            frame(
                &mut f,
                vec![
                    key(egui::Key::X, egui::Modifiers::NONE),
                    egui::Event::Text("x".into()),
                ],
            );
        }
        assert_eq!(f.text(), "x", "{}", f.app.status);
        assert_owner(&f, "x");
        assert_eq!(f.app.undo.len(), history + 2);
        assert!(f.app.plot_input_queue.is_empty() && f.app.last_executed_regions.is_empty());
    }
}

#[test]
fn native_pending_idle_reuses_profile_and_focus_or_noop_history_retires_capability() {
    for lose_focus in [true, false] {
        let mut f = fixture(false);
        select_all(&mut f);
        assert!(active(&f));
        let reads = f.app.rich_text.editors[&f.id]
            .as_ref()
            .unwrap()
            .select_all_profile_reads();
        assert_eq!(reads, 1);
        let pointer = f.app.rich_text.editors[&f.id]
            .as_ref()
            .unwrap()
            .source
            .xaml()
            .as_ptr();
        let before = f.xml();
        let history = f.app.undo.len();
        for _ in 0..10 {
            frame(&mut f, vec![]);
            assert!(active(&f));
            assert_eq!(
                f.app.rich_text.editors[&f.id]
                    .as_ref()
                    .unwrap()
                    .select_all_profile_reads(),
                reads
            );
            assert_eq!(
                f.app.rich_text.editors[&f.id]
                    .as_ref()
                    .unwrap()
                    .source
                    .xaml()
                    .as_ptr(),
                pointer
            );
        }
        if lose_focus {
            f.ctx
                .memory_mut(|m| m.surrender_focus(plain_text_input::widget_id(f.id)));
            frame(&mut f, vec![]);
        } else {
            f.app.undo.clear();
            f.app.redo.clear();
            frame(&mut f, vec![key(egui::Key::Z, egui::Modifiers::COMMAND)]);
        }
        assert!(!active(&f));
        f.select(0, ORIGINAL.chars().count());
        frame(&mut f, vec![egui::Event::Text("NO".into())]);
        assert_eq!(f.xml(), before);
        assert_eq!(f.text(), ORIGINAL);
        if lose_focus {
            assert_eq!(f.app.undo.len(), history);
        } else {
            assert!(f.app.undo.is_empty());
        }
    }
}

#[test]
fn native_paste_ime_and_noop_format_never_reuse_select_all_text_capability() {
    for operation in 0..3 {
        let mut f = fixture(false);
        select_all(&mut f);
        let before = f.xml();
        let history = f.app.undo.len();
        match operation {
            0 => frame(&mut f, vec![egui::Event::Paste("X".into())]),
            1 => {
                frame(
                    &mut f,
                    vec![egui::Event::Ime(egui::ImeEvent::Preedit {
                        text: "ㅎ".into(),
                        active_range_chars: None,
                    })],
                );
                frame(
                    &mut f,
                    vec![egui::Event::Ime(egui::ImeEvent::Commit("한".into()))],
                );
            }
            _ => {
                f.command(|selection_key| RibbonAction::SetTextColor {
                    rgba: [255, 255, 255, 255],
                    selection_key,
                });
                assert!(!active(&f));
                frame(&mut f, vec![egui::Event::Text("NO".into())]);
            }
        }
        assert_eq!(f.xml(), before, "operation={operation}: {}", f.app.status);
        assert_eq!(f.text(), ORIGINAL);
        assert_eq!(f.app.undo.len(), history);
        assert!(!active(&f));
    }
}

#[test]
fn native_replacement_budget_failure_keeps_source_runtime_and_history_atomic() {
    let mut f = fixture(false);
    select_all(&mut f);
    let before = f.xml();
    let history = f.app.undo.len();
    let old = f.app.text_cursors[&f.id];
    let selected = GlobalSelection {
        anchor: old.secondary.index.0,
        focus: old.primary.index.0,
    };
    let e = f
        .app
        .rich_text
        .editors
        .get_mut(&f.id)
        .unwrap()
        .as_mut()
        .unwrap();
    e.begin_select_all_delivery(
        f.id,
        selected,
        &[egui::Event::Text("X".into())],
        true,
        false,
    );
    assert!(e.has_select_all_replacement());
    assert!(
        e.commit_select_all(
            f.id,
            "X",
            selected,
            GlobalSelection {
                anchor: 1,
                focus: 1
            },
            0
        )
        .is_err()
    );
    assert_eq!(f.xml(), before);
    assert_eq!(f.app.undo.len(), history);
    assert_eq!(f.text(), ORIGINAL);
    assert!(!active(&f));
}

#[test]
fn native_same_bytes_cold_source_version_cannot_reuse_old_live_permission() {
    let mut f = fixture(false);
    select_all(&mut f);
    assert!(active(&f));
    let before = f.xml();
    let history = f.app.undo.len();
    let source = &f.app.rich_text.editors[&f.id].as_ref().unwrap().source;
    let encoded = serde_json::to_vec(source.wire()).unwrap();
    let wire = serde_json::from_slice(&encoded).unwrap();
    let package =
        NativePackage::from_bytes(f.app.document.native_archive.as_ref().unwrap()).unwrap();
    let owner = f
        .app
        .document
        .regions
        .iter()
        .find(|r| r.id == f.id)
        .unwrap()
        .source_region
        .as_ref()
        .unwrap();
    let authority =
        mathfolio_document::export::read_native_text_authority(&package, owner).unwrap();
    let cold = mathfolio_document::rich_text_source::ValidatedRichText::from_wire(
        wire, authority, ORIGINAL,
    )
    .unwrap();
    assert_eq!(cold.xaml(), source.xaml());
    assert_ne!(cold.xaml().as_ptr(), source.xaml().as_ptr());
    // Model an external fully admitted source replacement with identical bytes;
    // a retained runtime capability must still bind the former source version.
    f.app
        .rich_text
        .editors
        .get_mut(&f.id)
        .unwrap()
        .as_mut()
        .unwrap()
        .source = cold;
    frame(&mut f, vec![egui::Event::Text("NO".into())]);
    assert_eq!(f.xml(), before);
    assert_eq!(f.text(), ORIGINAL);
    assert_eq!(f.app.undo.len(), history);
    assert!(!active(&f));
}

fn partial_native_run_format_split() -> Fixture {
    let mut f = fixture(false);
    let before = f.xml();
    f.select(1, 3); // strict interior of the original first nonempty LCCL Run
    f.command(|selection_key| RibbonAction::SetTextBold {
        value: true,
        selection_key,
    });
    assert_ne!(f.xml(), before);
    assert!(run_parts(&f.xml()).len() > run_parts(&before).len());
    let source = &f.app.rich_text.editors[&f.id].as_ref().unwrap().source;
    assert!(!source.has_original_raw_body_only_delta());
    assert!(source.prove_format_only().is_ok());
    f
}
fn assert_native_cold_source(path: &Path, expected_text: &str, expected_xml: &str) {
    let mut cold = WorksheetApp {
        automatic_calculation: false,
        ..Default::default()
    };
    cold.open(path);
    assert!(cold.status.starts_with("Opened"), "{}", cold.status);
    let id = cold
        .document
        .regions
        .iter()
        .find(|r| matches!(&r.content, CanvasContent::Text(t) if t == expected_text))
        .unwrap()
        .id;
    cold.selected = Some(id);
    cold.prepare_rich_text();
    assert_eq!(
        cold.rich_text.editors[&id].as_ref().unwrap().source.xaml(),
        expected_xml
    );
    assert!(cold.last_executed_regions.is_empty() && !cold.background.running);
}

#[test]
fn native_prior_partial_format_split_same_original_plain_is_rejected_before_source_history_publish()
{
    let mut f = partial_native_run_format_split();
    let formatted = f.xml();
    let wire = f.wire().cloned();
    let history = f.app.undo.len();
    let generation = f.app.background.submitted_generation();
    select_all(&mut f);
    assert!(active(&f));
    let selection = f.app.text_cursors[&f.id];
    frame(&mut f, vec![egui::Event::Text(ORIGINAL.into())]);
    assert_eq!(
        f.xml(),
        formatted,
        "Same-plain unproved repartition must not be published; {}",
        f.app.status
    );
    assert_eq!(f.wire(), wire.as_ref());
    assert_eq!(f.text(), ORIGINAL);
    assert_eq!(f.app.undo.len(), history);
    assert_eq!(f.app.text_cursors[&f.id], selection);
    assert_eq!(f.app.background.submitted_generation(), generation);
    assert!(f.app.last_executed_regions.is_empty() && !active(&f));
    // Rejection must leave the legitimate previous format source exportable.
    let native = f.root.join("sameplain-refused-prior-format.mcdx");
    mathfolio_document::export::export_mcdx(&f.app.document, &native).unwrap();
    assert_native_cold_source(&native, ORIGINAL, &formatted);
}

#[test]
fn native_prior_partial_format_split_different_plain_still_saves_and_cold_reopens() {
    let mut f = partial_native_run_format_split();
    let formatted = f.xml();
    let history = f.app.undo.len();
    let generation = f.app.background.submitted_generation();
    select_all(&mut f);
    frame(&mut f, vec![egui::Event::Text("다른 입력😀<&".into())]);
    assert_eq!(f.text(), "다른 입력😀<&", "{}", f.app.status);
    assert_ne!(f.xml(), formatted);
    assert_eq!(f.app.undo.len(), history + 1);
    let changed = f.xml();
    let native = f.root.join("different-plain-prior-format.mcdx");
    mathfolio_document::export::export_mcdx(&f.app.document, &native).unwrap();
    assert_native_cold_source(&native, "다른 입력😀<&", &changed);
    let own = f.save("different-plain-prior-format.mathfolio");
    assert_native_cold_source(&own, "다른 입력😀<&", &changed);
    assert_eq!(f.app.background.submitted_generation(), generation);
    assert!(f.app.last_executed_regions.is_empty() && !f.app.background.running);
}
