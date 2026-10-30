//! Proposed controller tests; no Cargo executed by artifact author.
use super::*;
use mathfolio_document::rich_text_source::{
    AuthoredStyleSnapshot, AuthoredTextSeed, AuthoredTextSeedSpec, Weight,
};
const OWNER: u64 = 71;
fn source() -> AuthoredRichText {
    let style = AuthoredStyleSnapshot {
        key: "OwnClosed".into(),
        font_family: "Tahoma".into(),
        size_dip: "18.5".into(),
        weight: "Normal".into(),
        font_style: "Normal".into(),
        alignment: "Left".into(),
        foreground_rgba: [1, 2, 3, 255],
        background_rgba: [255, 255, 255, 0],
    };
    let seed = AuthoredTextSeed::new(AuthoredTextSeedSpec {
        format: style.clone(),
        catalog: vec![style],
        proofing_language: "ko-KR".into(),
        text_decorations: None,
    })
    .unwrap();
    let s = AuthoredRichText::empty(seed)
        .unwrap()
        .replace_in_run(0, 0, 0, "가😀ABCD")
        .unwrap()
        .unwrap()
        .source;
    s.set_selection_format(
        TextSelection {
            paragraph: 0,
            anchor: 0,
            focus: 2,
        },
        &mathfolio_document::rich_text_source::TextFormat {
            weight: Some(Weight::Bold),
            ..Default::default()
        },
    )
    .unwrap()
    .unwrap()
    .source
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
fn ctrl_a() -> egui::Event {
    key(egui::Key::A, egui::Modifiers::COMMAND)
}
fn full(s: &AuthoredRichText) -> GlobalSelection {
    GlobalSelection {
        anchor: 0,
        focus: s.plain_text().chars().count(),
    }
}
fn begin(e: &mut AuthoredEditor) {
    e.begin_select_all_delivery(
        OWNER,
        GlobalSelection {
            anchor: 2,
            focus: 2,
        },
        &[ctrl_a()],
        true,
        false,
    );
    let selected = full(&e.source);
    e.select_all
        .observe_selection(OWNER, &e.source, Some(selected));
}
fn text(e: &mut AuthoredEditor, inserted: &str) {
    e.begin_select_all_delivery(
        OWNER,
        full(&e.source),
        &[egui::Event::Text(inserted.into())],
        true,
        false,
    );
}
#[test]
fn full_range_without_actual_selectall_never_grants_ready() {
    let mut e = AuthoredEditor::new(source()).unwrap();
    text(&mut e, "X");
    assert!(!e.select_all.ready());
    let before = e.source.wire().clone();
    let end = egui::text::CCursorRange::one(egui::text::CCursor::new(1));
    let r = full(&e.source);
    let selected = egui::text::CCursorRange::two(
        egui::text::CCursor::new(r.anchor),
        egui::text::CCursor::new(r.focus),
    );
    assert!(e.commit("X", selected, end, usize::MAX).is_err());
    assert_eq!(e.source.wire(), &before);
}
#[test]
fn selectall_must_be_focused_nonime_and_observed_complete() {
    for (focused, ime) in [(false, false), (true, true)] {
        let mut e = AuthoredEditor::new(source()).unwrap();
        e.begin_select_all_delivery(OWNER, full(&e.source), &[ctrl_a()], focused, ime);
        assert!(e.select_all.pending.is_none());
    }
    let mut e = AuthoredEditor::new(source()).unwrap();
    e.begin_select_all_delivery(OWNER, full(&e.source), &[ctrl_a()], true, false);
    e.select_all.observe_selection(
        OWNER,
        &e.source,
        Some(GlobalSelection {
            anchor: 0,
            focus: 1,
        }),
    );
    assert!(e.select_all.pending.is_none());
    e.begin_select_all_delivery(OWNER, full(&e.source), &[ctrl_a()], true, false);
    text(&mut e, "X"); // no observed widget selection
    assert!(!e.select_all.ready());
}
#[test]
fn admitted_separate_deliveries_capture_current_format_and_commit_once() {
    let mut e = AuthoredEditor::new(source()).unwrap();
    e.affinity.remember_run(&e.source, 1).unwrap();
    begin(&mut e);
    assert_eq!(e.affinity.owner(&e.source), None);
    text(&mut e, "한😀");
    assert!(e.select_all.ready());
    let before = full(&e.source);
    assert_eq!(
        e.select_all.preview_owner(&e.source, before, "한😀"),
        Some(0)
    );
    let wire = e
        .commit_select_all(
            OWNER,
            "한😀",
            before,
            GlobalSelection {
                anchor: 2,
                focus: 2,
            },
            usize::MAX,
        )
        .unwrap()
        .unwrap();
    assert_eq!(e.source.plain_text(), "한😀");
    assert_eq!(e.affinity.owner(&e.source), Some(0));
    assert_eq!(e.source.creation_format_for_run(0).unwrap().weight, "Bold");
    assert_eq!(e.source.seed().spec().format.weight, "Normal");
    assert_eq!(&wire, e.source.wire());
    assert!(e.select_all.pending.is_none());
}
#[test]
fn stale_owner_source_direction_or_unknown_delivery_retires_without_publishing() {
    for case in 0..5 {
        let mut e = AuthoredEditor::new(source()).unwrap();
        begin(&mut e);
        let before = e.source.wire().clone();
        let selected = full(&e.source);
        match case {
            0 => e.begin_select_all_delivery(
                OWNER + 1,
                selected,
                &[egui::Event::Text("X".into())],
                true,
                false,
            ),
            1 => {
                let raw = serde_json::to_vec(e.source.wire()).unwrap();
                e.source = AuthoredRichText::from_json(&raw, e.source.plain_text()).unwrap();
                text(&mut e, "X");
            }
            2 => e.begin_select_all_delivery(
                OWNER,
                GlobalSelection {
                    anchor: selected.focus,
                    focus: 0,
                },
                &[egui::Event::Text("X".into())],
                true,
                false,
            ),
            3 => e.begin_select_all_delivery(
                OWNER,
                selected,
                &[egui::Event::Paste("X".into())],
                true,
                false,
            ),
            _ => e.begin_select_all_delivery(
                OWNER,
                selected,
                &[egui::Event::Ime(egui::ImeEvent::Commit("X".into()))],
                true,
                false,
            ),
        };
        assert!(!e.select_all.ready());
        assert_eq!(e.source.wire(), &before);
    }
}
#[test]
fn unsplit_ctrl_a_plus_text_and_other_edits_never_forge_provenance() {
    for events in [
        vec![ctrl_a(), egui::Event::Text("X".into())],
        vec![ctrl_a(), ctrl_a()],
        vec![key(
            egui::Key::A,
            egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
        )],
        vec![egui::Event::Cut],
        vec![key(egui::Key::Delete, egui::Modifiers::NONE)],
        vec![key(egui::Key::ArrowLeft, egui::Modifiers::NONE)],
    ] {
        let mut e = AuthoredEditor::new(source()).unwrap();
        begin(&mut e);
        let s = full(&e.source);
        e.begin_select_all_delivery(OWNER, s, &events, true, false);
        assert!(e.select_all.pending.is_none());
    }
}
#[test]
fn wrong_profile_caret_buffer_or_headroom_is_atomic_and_consumes_only_token() {
    for case in 0..4 {
        let mut e = AuthoredEditor::new(source()).unwrap();
        begin(&mut e);
        text(&mut e, "X");
        let before = full(&e.source);
        let wire = e.source.wire().clone();
        if case == 0 {
            e.select_all.pending.as_mut().unwrap().profile.language = "en-US".into();
        }
        let after = GlobalSelection {
            anchor: if case == 1 { 0 } else { 1 },
            focus: if case == 1 { 0 } else { 1 },
        };
        assert!(
            e.commit_select_all(
                OWNER,
                if case == 2 { "Y" } else { "X" },
                before,
                after,
                if case == 3 { 0 } else { usize::MAX }
            )
            .is_err()
        );
        assert_eq!(e.source.wire(), &wire);
        assert_eq!(e.affinity.owner(&e.source), None);
        assert!(e.select_all.pending.is_none());
    }
}
#[test]
fn same_plain_replacement_is_dispatched_but_exact_single_run_noop_adds_no_wire() {
    let mut e = AuthoredEditor::new(source()).unwrap();
    let original = e.source.plain_text().to_owned();
    begin(&mut e);
    text(&mut e, &original);
    let f = full(&e.source);
    assert!(
        e.commit_select_all(
            OWNER,
            &original,
            f,
            GlobalSelection {
                anchor: f.focus,
                focus: f.focus
            },
            usize::MAX
        )
        .unwrap()
        .is_some()
    );
    let before = e.source.wire().clone();
    begin(&mut e);
    text(&mut e, &original);
    let f = full(&e.source);
    assert!(
        e.commit_select_all(
            OWNER,
            &original,
            f,
            GlobalSelection {
                anchor: f.focus,
                focus: f.focus
            },
            usize::MAX
        )
        .unwrap()
        .is_none()
    );
    assert_eq!(e.source.wire(), &before);
}
#[test]
fn benign_idle_release_preserves_but_navigation_and_explicit_format_retire() {
    let mut e = AuthoredEditor::new(source()).unwrap();
    begin(&mut e);
    let selected = full(&e.source);
    let mut release = ctrl_a();
    if let egui::Event::Key { pressed, .. } = &mut release {
        *pressed = false;
    }
    e.begin_select_all_delivery(OWNER, selected, &[release], true, false);
    e.begin_select_all_delivery(OWNER, selected, &[], true, false);
    e.begin_select_all_delivery(
        OWNER,
        selected,
        &[
            egui::Event::PointerMoved(egui::Pos2::new(800., 400.)),
            egui::Event::PointerGone,
        ],
        true,
        false,
    );
    assert!(e.select_all.pending.is_some());
    e.navigate();
    assert!(e.select_all.pending.is_none());
    begin(&mut e);
    e.retire_select_all();
    assert!(e.select_all.pending.is_none());
}

#[test]
fn nonediting_physical_character_keys_do_not_discard_following_ordinary_text() {
    let mut e = AuthoredEditor::new(source()).unwrap();
    begin(&mut e);
    let selected = full(&e.source);
    e.begin_select_all_delivery(
        OWNER,
        selected,
        &[key(egui::Key::X, egui::Modifiers::NONE)],
        true,
        false,
    );
    assert!(e.select_all.pending.is_some());
    text(&mut e, "x");
    assert!(e.select_all.ready());
}
