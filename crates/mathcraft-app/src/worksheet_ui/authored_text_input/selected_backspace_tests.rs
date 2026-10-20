//! Proposed pure/controller tests; not executed by the artifact author.
use super::*;
use mathcraft_document::rich_text_source::{
    AuthoredTextSeed, AuthoredTextSeedSpec, TextFormat, Weight,
};

const OWNER: u64 = 42;
const ORIGINAL: &str = "가A&😀BC한DE꼬";
const DELETED: &str = "가AE꼬";

fn source() -> AuthoredRichText {
    let style = AuthoredStyleSnapshot {
        key: "ClosedBody".into(),
        font_family: "Tahoma".into(),
        size_dip: "18.5".into(),
        weight: "Normal".into(),
        font_style: "Normal".into(),
        alignment: "Left".into(),
        foreground_rgba: [18, 52, 86, 255],
        background_rgba: [255, 255, 255, 0],
    };
    let seed = AuthoredTextSeed::new(AuthoredTextSeedSpec {
        format: style.clone(),
        catalog: vec![style],
        proofing_language: "ko-KR".into(),
        text_decorations: None,
    })
    .unwrap();
    let value = AuthoredRichText::empty(seed)
        .unwrap()
        .replace_in_run(0, 0, 0, ORIGINAL)
        .unwrap()
        .unwrap()
        .source;
    value
        .set_selection_format(
            TextSelection {
                paragraph: 0,
                anchor: 4,
                focus: 7,
            },
            &TextFormat {
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
fn backspace() -> egui::Event {
    key(egui::Key::Backspace, egui::Modifiers::NONE)
}
fn selected() -> GlobalSelection {
    GlobalSelection {
        anchor: 2,
        focus: 8,
    }
}
fn range(selection: GlobalSelection) -> egui::text::CCursorRange {
    egui::text::CCursorRange::two(
        egui::text::CCursor::new(selection.anchor),
        egui::text::CCursor::new(selection.focus),
    )
}
fn after() -> egui::text::CCursorRange {
    egui::text::CCursorRange::one(egui::text::CCursor::new(2))
}
fn proof(source: &AuthoredRichText, selected: GlobalSelection) -> SelectedBackspace {
    SelectedBackspace::capture(OWNER, source, selected, &[backspace()], true, false).unwrap()
}

#[test]
fn only_one_plain_backspace_plus_benign_key_releases_grants_capture() {
    let source = source();
    let mut release = key(egui::Key::Delete, egui::Modifiers::COMMAND);
    if let egui::Event::Key { pressed, .. } = &mut release {
        *pressed = false;
    }
    assert!(
        SelectedBackspace::capture(
            OWNER,
            &source,
            selected(),
            &[release.clone(), backspace(), release],
            true,
            false
        )
        .is_some()
    );
    assert!(
        SelectedBackspace::capture(OWNER, &source, selected(), &[backspace()], false, false)
            .is_none()
    );
    assert!(
        SelectedBackspace::capture(OWNER, &source, selected(), &[backspace()], true, true)
            .is_none()
    );
    for events in [
        vec![],
        vec![backspace(), backspace()],
        vec![backspace(), egui::Event::Text("X".into())],
        vec![backspace(), egui::Event::Paste("X".into())],
        vec![backspace(), egui::Event::Cut],
        vec![backspace(), key(egui::Key::Delete, egui::Modifiers::NONE)],
        vec![key(egui::Key::Backspace, egui::Modifiers::SHIFT)],
        vec![
            backspace(),
            egui::Event::Ime(egui::ImeEvent::Commit(String::new())),
        ],
        vec![backspace(), egui::Event::PointerMoved(egui::Pos2::ZERO)],
        vec![backspace(), egui::Event::WindowFocused(false)],
    ] {
        assert!(
            SelectedBackspace::capture(OWNER, &source, selected(), &events, true, false).is_none()
        );
    }
}

#[test]
fn capture_is_exact_source_owner_and_direction_bound_not_equal_plain_flag() {
    let source = source();
    let token = proof(&source, selected());
    assert!(token.validate_before(OWNER, &source, selected()).is_ok());
    assert!(
        token
            .validate_before(OWNER + 1, &source, selected())
            .is_err()
    );
    assert!(
        token
            .validate_before(
                OWNER,
                &source,
                GlobalSelection {
                    anchor: 8,
                    focus: 2
                }
            )
            .is_err()
    );
    let raw = serde_json::to_vec(source.wire()).unwrap();
    let cold = AuthoredRichText::from_json(&raw, source.plain_text()).unwrap();
    assert_eq!(cold.xaml(), source.xaml());
    assert!(token.validate_before(OWNER, &cold, selected()).is_err());
}

#[test]
fn unsupported_endpoint_or_collapsed_range_does_not_capture_or_change_source() {
    let source = source();
    let wire = source.wire().clone();
    for (anchor, focus) in [(2, 2), (0, 8), (2, 7), (1, 3), (0, 11), (2, usize::MAX)] {
        assert!(
            SelectedBackspace::capture(
                OWNER,
                &source,
                GlobalSelection { anchor, focus },
                &[backspace()],
                true,
                false
            )
            .is_none()
        );
    }
    assert_eq!(source.wire(), &wire);
}

#[test]
fn checked_selected_backspace_binds_restricted_owner_then_ordinary_text_promotes() {
    for selected in [
        selected(),
        GlobalSelection {
            anchor: 8,
            focus: 2,
        },
    ] {
        let mut editor = AuthoredEditor::new(source()).unwrap();
        let before_xml = editor.source.xaml().to_owned();
        let token = proof(&editor.source, selected);
        assert!(
            editor
                .commit_with_backspace(
                    OWNER,
                    DELETED,
                    range(selected),
                    after(),
                    usize::MAX,
                    Some(token)
                )
                .unwrap()
                .is_some()
        );
        assert_eq!(editor.affinity.owner(&editor.source), Some(0));
        assert!(editor.affinity.is_backspace_continuation());
        assert!(editor.source.xaml().contains(">가A</Run>"));
        assert_eq!(editor.source.xaml().matches("<Run ").count(), 3);
        assert!(before_xml.contains(">BC한</Run>"));
        assert!(
            editor
                .commit(
                    "가AXE꼬",
                    after(),
                    egui::text::CCursorRange::one(egui::text::CCursor::new(3)),
                    usize::MAX
                )
                .unwrap()
                .is_some()
        );
        assert_eq!(editor.affinity.owner(&editor.source), Some(0));
        assert!(!editor.affinity.is_backspace_continuation());
        assert_eq!(
            editor.source.creation_format_for_run(0).unwrap().weight,
            "Normal"
        );
    }
}

#[test]
fn generic_empty_diff_delete_cannot_grant_continuation() {
    let mut editor = AuthoredEditor::new(source()).unwrap();
    assert!(
        editor
            .commit(DELETED, range(selected()), after(), usize::MAX)
            .unwrap()
            .is_some()
    );
    assert_eq!(editor.affinity.owner(&editor.source), None);
    assert!(!editor.affinity.is_backspace_continuation());
    let wire = editor.source.wire().clone();
    assert!(
        editor
            .commit(
                "가AXE꼬",
                after(),
                egui::text::CCursorRange::one(egui::text::CCursor::new(3)),
                usize::MAX
            )
            .is_err()
    );
    assert_eq!(editor.source.wire(), &wire);
}

#[test]
fn budget_rejection_keeps_old_source_and_general_affinity_unpublished() {
    let mut editor = AuthoredEditor::new(source()).unwrap();
    editor.affinity.remember_run(&editor.source, 1).unwrap();
    let wire = editor.source.wire().clone();
    let token = proof(&editor.source, selected());
    assert!(
        editor
            .commit_with_backspace(OWNER, DELETED, range(selected()), after(), 1, Some(token))
            .is_err()
    );
    assert_eq!(editor.source.wire(), &wire);
    assert_eq!(editor.affinity.owner(&editor.source), Some(1));
    assert!(!editor.affinity.is_backspace_continuation());
}

#[test]
fn stale_token_or_wrong_actual_buffer_after_selection_rejects_atomically() {
    for stale in 0..3 {
        let mut editor = AuthoredEditor::new(source()).unwrap();
        let token = proof(&editor.source, selected());
        let original = editor.source.wire().clone();
        let (owner, text, cursor) = match stale {
            0 => (OWNER + 1, DELETED, after()),
            1 => (
                OWNER,
                "가AXE꼬",
                egui::text::CCursorRange::one(egui::text::CCursor::new(3)),
            ),
            _ => (
                OWNER,
                DELETED,
                egui::text::CCursorRange::one(egui::text::CCursor::new(1)),
            ),
        };
        assert!(
            editor
                .commit_with_backspace(
                    owner,
                    text,
                    range(selected()),
                    cursor,
                    usize::MAX,
                    Some(token)
                )
                .is_err()
        );
        assert_eq!(editor.source.wire(), &original);
        assert_eq!(editor.affinity.owner(&editor.source), None);
    }
}

#[test]
fn equal_plain_source_replacement_cannot_reuse_old_proof() {
    let original = source();
    let token = proof(&original, selected());
    let changed = original
        .set_selection_format(
            TextSelection {
                paragraph: 0,
                anchor: 0,
                focus: 2,
            },
            &TextFormat {
                weight: Some(Weight::Bold),
                ..Default::default()
            },
        )
        .unwrap()
        .unwrap()
        .source;
    let mut editor = AuthoredEditor::new(changed).unwrap();
    assert_eq!(editor.source.plain_text(), ORIGINAL);
    let wire = editor.source.wire().clone();
    assert!(
        editor
            .commit_with_backspace(
                OWNER,
                DELETED,
                range(selected()),
                after(),
                usize::MAX,
                Some(token)
            )
            .is_err()
    );
    assert_eq!(editor.source.wire(), &wire);
}

#[test]
fn post_source_format_mismatch_does_not_bind_snapshot_to_changed_run() {
    let original = source();
    let token = proof(&original, selected());
    let deleted = original
        .delete_cross_run_selection(TextSelection {
            paragraph: 0,
            anchor: 2,
            focus: 8,
        })
        .unwrap()
        .source;
    let changed = deleted
        .set_selection_format(
            TextSelection {
                paragraph: 0,
                anchor: 0,
                focus: 2,
            },
            &TextFormat {
                weight: Some(Weight::Bold),
                ..Default::default()
            },
        )
        .unwrap()
        .unwrap()
        .source;
    let mut next = AuthoredEditor::new(changed).unwrap();
    assert!(
        token
            .rebind_after_checked(
                &mut next,
                DELETED,
                GlobalSelection {
                    anchor: 2,
                    focus: 2
                }
            )
            .is_err()
    );
    assert_eq!(next.affinity.owner(&next.source), None);
}

#[test]
fn restricted_tag_retires_for_ime_paste_cut_and_other_edit_but_old_general_binding_is_unchanged() {
    let source = source();
    for events in [
        vec![egui::Event::Ime(egui::ImeEvent::Commit("한".into()))],
        vec![egui::Event::Paste("X".into())],
        vec![egui::Event::Cut],
        vec![key(egui::Key::Delete, egui::Modifiers::NONE)],
        vec![key(egui::Key::Tab, egui::Modifiers::NONE)],
    ] {
        let mut restricted = AuthoredEditor::new(source.clone()).unwrap();
        restricted
            .affinity
            .remember_backspace_run(&restricted.source, 0)
            .unwrap();
        retire_nonordinary_delivery(&mut restricted, &events, true, false);
        assert_eq!(restricted.affinity.owner(&restricted.source), None);
        let mut general = AuthoredEditor::new(source.clone()).unwrap();
        general.affinity.remember_run(&general.source, 0).unwrap();
        retire_nonordinary_delivery(&mut general, &events, true, false);
        assert_eq!(general.affinity.owner(&general.source), Some(0));
    }
}

#[test]
fn idle_save_releases_and_another_owners_ime_do_not_retire_restricted_binding() {
    let mut editor = AuthoredEditor::new(source()).unwrap();
    editor
        .affinity
        .remember_backspace_run(&editor.source, 0)
        .unwrap();
    for events in [
        vec![],
        vec![key(egui::Key::S, egui::Modifiers::COMMAND)],
        vec![egui::Event::Text("X".into())],
    ] {
        retire_nonordinary_delivery(&mut editor, &events, true, false);
        assert!(editor.affinity.is_backspace_continuation());
    }
    retire_nonordinary_delivery(
        &mut editor,
        &[egui::Event::Ime(egui::ImeEvent::Commit("다른".into()))],
        false,
        true,
    );
    assert!(editor.affinity.is_backspace_continuation());
}

#[test]
fn existing_runtime_snapshot_clones_keep_tag_only_for_exact_source() {
    let source = source();
    let mut affinity = RunAffinity::default();
    affinity.remember_backspace_run(&source, 0).unwrap();
    let snapshot = affinity.clone();
    affinity.clear();
    assert!(affinity.restore_if_bound(&source, &snapshot));
    assert!(affinity.is_backspace_continuation());
    let json = serde_json::to_vec(source.wire()).unwrap();
    let cold = AuthoredRichText::from_json(&json, ORIGINAL).unwrap();
    assert!(!affinity.restore_if_bound(&cold, &snapshot));
    assert_eq!(affinity.owner(&cold), None);
    assert!(!affinity.is_backspace_continuation());
    assert!(!String::from_utf8(json).unwrap().contains("backspace"));
}

#[test]
fn full_observed_selected_backspace_captures_first_nonempty_not_leading_empty_or_seed() {
    let original = source();
    // Independently remove the first Run's body, leaving its distinct Normal
    // metadata and the next Bold Run as the first nonempty source owner.
    let mut json = serde_json::to_value(original.wire()).unwrap();
    json["current_xaml"] =
        serde_json::Value::String(original.xaml().replace(">가A&amp;😀</Run>", "></Run>"));
    let wire: mathcraft_document::rich_text_source::AuthoredTextWire =
        serde_json::from_value(json).unwrap();
    let leading = AuthoredRichText::from_wire(wire, "BC한DE꼬").unwrap();
    for before in [
        GlobalSelection {
            anchor: 0,
            focus: 6,
        },
        GlobalSelection {
            anchor: 6,
            focus: 0,
        },
    ] {
        let mut editor = AuthoredEditor::new(leading.clone()).unwrap();
        let token = proof(&leading, before);
        assert!(token.full_paragraph);
        assert_eq!(token.first.run_ordinal, 1);
        assert_eq!(token.format.weight, "Bold");
        let zero = egui::text::CCursorRange::one(egui::text::CCursor::new(0));
        editor
            .commit_with_backspace(OWNER, "", range(before), zero, usize::MAX, Some(token))
            .unwrap()
            .unwrap();
        assert_eq!(editor.source.plain_text(), "");
        assert_eq!(editor.affinity.owner(&editor.source), Some(1));
        assert!(editor.affinity.is_backspace_continuation());
        assert_eq!(
            editor.source.xaml(),
            leading
                .xaml()
                .replace(">BC한</Run>", "></Run>")
                .replace(">DE꼬</Run>", "></Run>")
        );
        editor
            .commit(
                "X",
                zero,
                egui::text::CCursorRange::one(egui::text::CCursor::new(1)),
                usize::MAX,
            )
            .unwrap()
            .unwrap();
        assert_eq!(
            editor.source.creation_format_for_run(1).unwrap().weight,
            "Bold"
        );
        assert_eq!(editor.affinity.owner(&editor.source), Some(1));
        assert!(!editor.affinity.is_backspace_continuation());
        assert!(editor.source.xaml().contains(">X</Run>"));
    }
}

#[test]
fn full_wire_or_empty_diff_without_actual_bs_never_grants_and_cold_empty_remains_unknown() {
    let original = source();
    let before = GlobalSelection {
        anchor: 0,
        focus: 10,
    };
    for events in [
        vec![],
        vec![key(egui::Key::Delete, egui::Modifiers::NONE)],
        vec![egui::Event::Cut],
        vec![egui::Event::Ime(egui::ImeEvent::Commit(String::new()))],
    ] {
        assert!(
            SelectedBackspace::capture(OWNER, &original, before, &events, true, false).is_none()
        );
    }
    let mut editor = AuthoredEditor::new(original).unwrap();
    let zero = egui::text::CCursorRange::one(egui::text::CCursor::new(0));
    editor
        .commit("", range(before), zero, usize::MAX)
        .unwrap()
        .unwrap();
    assert_eq!(editor.affinity.owner(&editor.source), None);
    let cold = AuthoredRichText::from_json(&serde_json::to_vec(editor.source.wire()).unwrap(), "")
        .unwrap();
    assert!(
        SelectedBackspace::capture(
            OWNER,
            &cold,
            GlobalSelection {
                anchor: 0,
                focus: 0
            },
            &[backspace()],
            true,
            false
        )
        .is_none()
    );
    let mut cold = AuthoredEditor::new(cold).unwrap();
    assert!(
        cold.commit(
            "X",
            zero,
            egui::text::CCursorRange::one(egui::text::CCursor::new(1)),
            usize::MAX
        )
        .is_err()
    );
}

#[test]
fn full_bs_rebind_rejects_unrelated_paragraph_drift_and_stale_or_wrong_buffer_atomically() {
    let original = source();
    let before = GlobalSelection {
        anchor: 0,
        focus: 10,
    };
    let deleted = original
        .delete_cross_run_selection(mathcraft_document::rich_text_source::TextSelection {
            paragraph: 0,
            anchor: 0,
            focus: 10,
        })
        .unwrap()
        .source;
    let changed = deleted
        .set_paragraph_alignment(
            mathcraft_document::rich_text_source::TextSelection {
                paragraph: 0,
                anchor: 0,
                focus: 0,
            },
            mathcraft_document::rich_text_source::ParagraphAlignment::Center,
        )
        .unwrap()
        .unwrap()
        .source;
    assert!(Arc::ptr_eq(changed.seed(), original.seed()));
    let mut next = AuthoredEditor::new(changed).unwrap();
    let token = proof(&original, before);
    assert!(
        token
            .rebind_after_checked(
                &mut next,
                "",
                GlobalSelection {
                    anchor: 0,
                    focus: 0
                }
            )
            .is_err()
    );
    assert_eq!(next.affinity.owner(&next.source), None);
    let mut editor = AuthoredEditor::new(original.clone()).unwrap();
    let old = editor.source.wire().clone();
    let token = proof(&original, before);
    assert!(
        editor
            .commit_with_backspace(
                OWNER,
                "bad",
                range(before),
                egui::text::CCursorRange::one(egui::text::CCursor::new(0)),
                usize::MAX,
                Some(token)
            )
            .is_err()
    );
    assert_eq!(editor.source.wire(), &old);
}

#[test]
fn full_unique_run_capture_stays_on_old_generic_commit_path() {
    let old = source();
    let mut json = serde_json::to_value(old.wire()).unwrap();
    json["current_xaml"] = serde_json::Value::String(
        old.xaml()
            .replace(">BC한</Run>", "></Run>")
            .replace(">DE꼬</Run>", "></Run>"),
    );
    let wire: mathcraft_document::rich_text_source::AuthoredTextWire =
        serde_json::from_value(json).unwrap();
    let only = AuthoredRichText::from_wire(wire, "가A&😀").unwrap();
    assert!(
        SelectedBackspace::capture(
            OWNER,
            &only,
            GlobalSelection {
                anchor: 0,
                focus: 4
            },
            &[backspace()],
            true,
            false
        )
        .is_none()
    );
    let mut editor = AuthoredEditor::new(only).unwrap();
    editor
        .commit(
            "",
            range(GlobalSelection {
                anchor: 0,
                focus: 4,
            }),
            egui::text::CCursorRange::one(egui::text::CCursor::new(0)),
            usize::MAX,
        )
        .unwrap()
        .unwrap();
    assert_eq!(editor.source.plain_text(), "");
}
