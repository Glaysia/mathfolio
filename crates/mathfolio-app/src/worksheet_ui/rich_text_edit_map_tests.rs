use super::*;
use mathfolio_document::rich_text_source::{
    NativeTextIdentity, OriginalTextAuthority, TextFormat, TextSelection, Weight,
};
use std::{
    io::{Cursor, Write},
    sync::Arc,
};
use zip::{ZipWriter, write::SimpleFileOptions};

const CATALOG: &[u8] =
    include_bytes!("../../../../tests/reference/rich-text/lcc-standard-presentation.xml");
fn selection(anchor: usize, focus: usize) -> GlobalSelection {
    GlobalSelection { anchor, focus }
}
fn fixture(body: &str) -> (ValidatedRichText, Arc<OriginalTextAuthority>) {
    let xaml = format!(
        "<Section xmlns=\"http://schemas.microsoft.com/winfx/2006/xaml/presentation\" FontWeight=\"Normal\" FontStyle=\"Normal\" FontSize=\"16\" Foreground=\"#FFFFFFFF\"><Paragraph><Run style=\"Normal\">{body}</Run></Paragraph></Section>"
    );
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    zip.start_file("Xaml/Document.xaml", SimpleFileOptions::default())
        .unwrap();
    zip.write_all(xaml.as_bytes()).unwrap();
    let identity = NativeTextIdentity {
        owner_region_id: "17".into(),
        item_idref: "t1".into(),
        package_part: "mathcad/xaml/t1.XamlPackage".into(),
        catalog_part: "mathcad/settings/presentation.xml".into(),
        owner_xml_sha256: [17; 32],
    };
    let authority = OriginalTextAuthority::from_package(
        identity,
        zip.finish().unwrap().into_inner(),
        CATALOG.to_vec(),
    )
    .unwrap();
    let plain = authority.original_plain_text().to_owned();
    (
        ValidatedRichText::from_original(authority.clone(), &plain).unwrap(),
        authority,
    )
}
fn formatted(
    body: &str,
    start: usize,
    end: usize,
) -> (ValidatedRichText, Arc<OriginalTextAuthority>) {
    let (source, authority) = fixture(body);
    let patch = source
        .set_selection_format(
            TextSelection {
                paragraph: 0,
                anchor: start,
                focus: end,
            },
            &TextFormat {
                weight: Some(Weight::Bold),
                ..Default::default()
            },
        )
        .unwrap()
        .unwrap();
    (patch.source, authority)
}

#[test]
fn reverse_whole_run_selection_then_next_key_at_edge_keeps_selected_style() {
    let (source, authority) = formatted("abcd", 1, 3);
    let mut affinity = RunAffinity::default();
    affinity
        .remember_selection(&source, selection(3, 1))
        .unwrap();
    let first = apply_committed(
        &source,
        selection(3, 1),
        selection(2, 2),
        "aXd",
        &mut affinity,
    )
    .unwrap()
    .unwrap();
    assert_eq!(first.run_ordinal, 1);
    assert_eq!(first.old_global_range, 1..3);
    assert!(
        first
            .patch
            .source
            .xaml()
            .contains("FontWeight=\"Bold\">X</Run>")
    );
    // Installing Box<wire.clone()> and rebuilding the runtime must not lose
    // the INNER Arc<str>-bound owner because the outer runtime pointer changes.
    let rebuilt =
        ValidatedRichText::from_wire(first.patch.source.wire().clone(), authority, "aXd").unwrap();
    assert_eq!(affinity.owner(&rebuilt), Some(1));
    let next = apply_committed(
        &rebuilt,
        selection(2, 2),
        selection(3, 3),
        "aXYd",
        &mut affinity,
    )
    .unwrap()
    .unwrap();
    assert_eq!(next.run_ordinal, 1);
    assert!(
        next.patch
            .source
            .xaml()
            .contains("FontWeight=\"Bold\">XY</Run>")
    );
}

#[test]
fn repeated_char_insertion_uses_actual_caret_not_longest_prefix_at_wrong_run() {
    let (source, _) = formatted("aa", 1, 2);
    let mut affinity = RunAffinity::default();
    affinity.remember_run(&source, 0).unwrap();
    let plan = plan_committed(&source, selection(1, 1), selection(2, 2), "aaa", &affinity)
        .unwrap()
        .unwrap();
    assert_eq!(plan.global_range, 1..1);
    assert_eq!(plan.run_ordinal, 0);
    assert_eq!(plan.inserted, "a");
    let edit = apply_committed(
        &source,
        selection(1, 1),
        selection(2, 2),
        "aaa",
        &mut affinity,
    )
    .unwrap()
    .unwrap();
    assert!(
        edit.patch
            .source
            .xaml()
            .contains("<Run style=\"Normal\">aa</Run>")
    );
}

#[test]
fn pointer_or_navigation_reset_makes_internal_boundary_explicitly_unsupported() {
    let (source, _) = formatted("abcd", 1, 3);
    let mut affinity = RunAffinity::default();
    affinity.remember_run(&source, 1).unwrap();
    affinity.clear();
    let before = source.wire().clone();
    assert!(
        apply_committed(
            &source,
            selection(3, 3),
            selection(4, 4),
            "abcXd",
            &mut affinity
        )
        .is_err()
    );
    assert_eq!(source.wire(), &before);
    assert_eq!(affinity.owner(&source), None);
}

#[test]
fn paragraph_end_and_start_choose_last_and_first_without_internal_wrap() {
    let (source, _) = formatted("abcd", 1, 3);
    let mut affinity = RunAffinity::default();
    let prefix = apply_committed(
        &source,
        selection(0, 0),
        selection(1, 1),
        "Xabcd",
        &mut affinity,
    )
    .unwrap()
    .unwrap();
    assert_eq!(prefix.run_ordinal, 0);
    affinity.clear();
    let suffix = apply_committed(
        &source,
        selection(4, 4),
        selection(5, 5),
        "abcdX",
        &mut affinity,
    )
    .unwrap()
    .unwrap();
    assert_eq!(suffix.run_ordinal, 2);
}

#[test]
fn deleting_whole_styled_run_then_typing_uses_retained_empty_run_owner() {
    let (source, _) = formatted("abcd", 1, 3);
    let mut affinity = RunAffinity::default();
    let removed = apply_committed(
        &source,
        selection(1, 3),
        selection(1, 1),
        "ad",
        &mut affinity,
    )
    .unwrap()
    .unwrap();
    assert_eq!(affinity.owner(&removed.patch.source), Some(1));
    assert!(
        removed
            .patch
            .source
            .xaml()
            .contains("FontWeight=\"Bold\"></Run>")
    );
    let typed = apply_committed(
        &removed.patch.source,
        selection(1, 1),
        selection(2, 2),
        "aZd",
        &mut affinity,
    )
    .unwrap()
    .unwrap();
    assert_eq!(typed.run_ordinal, 1);
    assert!(
        typed
            .patch
            .source
            .xaml()
            .contains("FontWeight=\"Bold\">Z</Run>")
    );
}

#[test]
fn korean_emoji_selection_uses_scalar_offsets_and_exact_old_plain_byte_range() {
    let (source, _) = formatted("가😀나라마", 1, 3);
    let mut affinity = RunAffinity::default();
    let plan = plan_committed(
        &source,
        selection(3, 1),
        selection(3, 3),
        "가한😀라마",
        &affinity,
    )
    .unwrap()
    .unwrap();
    assert_eq!(plan.global_range, 1..3);
    assert_eq!(plan.old_bytes, 3..10);
    assert_eq!(plan.local_range, 0..2);
    assert_eq!(plan.inserted, "한😀");
    let edit = apply_committed(
        &source,
        selection(3, 1),
        selection(3, 3),
        "가한😀라마",
        &mut affinity,
    )
    .unwrap()
    .unwrap();
    assert_eq!(edit.patch.source.plain_text(), "가한😀라마");
}

#[test]
fn cross_run_paragraph_newline_and_invalid_cursor_reject_without_affinity_change() {
    let (source, _) = formatted("abcd", 1, 3);
    let mut affinity = RunAffinity::default();
    affinity.remember_run(&source, 1).unwrap();
    let before = source.wire().clone();
    assert!(
        apply_committed(
            &source,
            selection(0, 3),
            selection(1, 1),
            "Xd",
            &mut affinity
        )
        .is_err()
    );
    assert!(
        apply_committed(
            &source,
            selection(1, 3),
            selection(2, 2),
            "a\nd",
            &mut affinity
        )
        .is_err()
    );
    assert!(
        apply_committed(
            &source,
            selection(100, 100),
            selection(1, 1),
            "aXd",
            &mut affinity
        )
        .is_err()
    );
    assert_eq!(source.wire(), &before);
    assert_eq!(affinity.owner(&source), Some(1));
    let (paragraphs, _) = fixture("ab</Run></Paragraph><Paragraph><Run style=\"Normal\">cd");
    let unchanged = paragraphs.wire().clone();
    assert!(
        apply_committed(
            &paragraphs,
            selection(1, 4),
            selection(1, 1),
            "ad",
            &mut RunAffinity::default()
        )
        .is_err()
    );
    assert_eq!(paragraphs.wire(), &unchanged);
}

#[test]
fn external_same_owner_source_change_invalidates_affinity_but_shallow_clone_does_not() {
    let (source, _) = formatted("abcd", 1, 3);
    let mut affinity = RunAffinity::default();
    affinity.remember_run(&source, 1).unwrap();
    assert_eq!(affinity.owner(&source.clone()), Some(1)); // resize/source-inert snapshot
    let external = source.replace_in_run(0, 0, 1, "A").unwrap().unwrap().source;
    assert_eq!(affinity.owner(&external), None);
    assert!(
        apply_committed(
            &external,
            selection(1, 1),
            selection(2, 2),
            "AXbcd",
            &mut affinity
        )
        .is_err()
    );
}

#[test]
fn ime_preview_save_and_cancel_do_not_call_source_mapper_commit_uses_origin_selection_once() {
    let (source, _) = formatted("abcd", 1, 3);
    let mut affinity = RunAffinity::default();
    affinity
        .remember_selection(&source, selection(1, 3))
        .unwrap();
    let checkpoint = affinity.clone();
    let saved_before = source.wire().clone();
    // Projection-only preedit buffers: do NOT apply_committed for either.
    let previews = ["aㅎd", "a하d"];
    assert_eq!(previews.len(), 2);
    assert_eq!(source.wire(), &saved_before);
    assert!(affinity.restore_if_bound(&source, &checkpoint)); // cancellation
    let commit = apply_committed(
        &source,
        selection(1, 3),
        selection(2, 2),
        "a한d",
        &mut affinity,
    )
    .unwrap()
    .unwrap();
    assert_eq!(commit.patch.source.plain_text(), "a한d");
    assert!(commit.patch.source.prove_format_only().is_err());
    // Save is passive: no extra source transform/history/affinity clearing.
    let saved_after = commit.patch.source.wire().clone();
    assert_eq!(saved_after, *commit.patch.source.wire());
    assert_eq!(affinity.owner(&commit.patch.source), Some(1));
    // Owner Undo restores source, selection AND runtime affinity checkpoint.
    assert!(affinity.restore_if_bound(&source, &checkpoint));
    assert_eq!(source.wire(), &saved_before);
    assert_eq!(affinity.owner(&source), Some(1));
}

#[test]
fn backspace_delete_and_noop_do_not_need_a_fake_selection_or_source_clone() {
    let (source, _) = fixture("abcd");
    let mut affinity = RunAffinity::default();
    let backward = apply_committed(
        &source,
        selection(2, 2),
        selection(1, 1),
        "acd",
        &mut affinity,
    )
    .unwrap()
    .unwrap();
    assert_eq!(backward.old_global_range, 1..2);
    let forward = apply_committed(
        &source,
        selection(1, 1),
        selection(1, 1),
        "acd",
        &mut affinity,
    )
    .unwrap()
    .unwrap();
    assert_eq!(forward.old_global_range, 1..2);
    let before = source.wire().clone();
    assert!(
        apply_committed(
            &source,
            selection(1, 1),
            selection(1, 1),
            "abcd",
            &mut affinity
        )
        .unwrap()
        .is_none()
    );
    assert_eq!(source.wire(), &before);
}

#[test]
fn cross_run_selected_deletion_keeps_unicode_raw_metadata_and_clears_affinity() {
    let (source, authority) = fixture(
        "가A&amp;😀</Run><Run style=\"Normal\" FontWeight=\"Bold\">BC한</Run><Run style=\"Normal\" FontStyle=\"Italic\">DE꼬",
    );
    assert_eq!(source.plain_text(), "가A&😀BC한DE꼬");
    let expected = source
        .xaml()
        .replace("가A&amp;😀", "가A")
        .replace(">BC한</Run>", "></Run>")
        .replace(">DE꼬</Run>", ">E꼬</Run>");
    for before in [selection(2, 8), selection(8, 2)] {
        let mut affinity = RunAffinity::default();
        affinity.remember_run(&source, 1).unwrap();
        let plan = plan_committed(&source, before, selection(2, 2), "가AE꼬", &affinity)
            .unwrap()
            .unwrap();
        assert_eq!(plan.run_ordinal, 0);
        let selected = plan.cross_run_deletion.unwrap();
        assert_eq!((selected.anchor, selected.focus), (2, 8));
        let edit = apply_committed(&source, before, selection(2, 2), "가AE꼬", &mut affinity)
            .unwrap()
            .unwrap();
        assert_eq!(edit.patch.source.xaml(), expected);
        assert_eq!(edit.patch.source.plain_text(), "가AE꼬");
        assert_eq!(affinity.owner(&edit.patch.source), None);
        let cold = ValidatedRichText::from_wire(
            edit.patch.source.wire().clone(),
            authority.clone(),
            "가AE꼬",
        )
        .unwrap();
        assert_eq!(cold.xaml(), expected);
        // No automatic left/right style choice at the new internal boundary.
        assert!(
            apply_committed(
                &cold,
                selection(2, 2),
                selection(3, 3),
                "가AXE꼬",
                &mut affinity,
            )
            .is_err()
        );
    }
    assert_eq!(source.plain_text(), "가A&😀BC한DE꼬");
}

#[test]
fn boundary_deletion_maps_exact_source_and_no_future_owner_while_typing_remains_strict() {
    let (source, _) = fixture("a&amp;😀</Run><Run style=\"Normal\" FontWeight=\"Bold\">BC");
    let original = source.wire().clone();
    for (before, new_text, expected_body) in [
        (
            selection(0, 4),
            "C",
            "<Run style=\"Normal\"></Run><Run style=\"Normal\" FontWeight=\"Bold\">C</Run>",
        ),
        (
            selection(0, 5),
            "",
            "<Run style=\"Normal\"></Run><Run style=\"Normal\" FontWeight=\"Bold\"></Run>",
        ),
        (
            selection(5, 0),
            "",
            "<Run style=\"Normal\"></Run><Run style=\"Normal\" FontWeight=\"Bold\"></Run>",
        ),
    ] {
        let mut affinity = RunAffinity::default();
        affinity.remember_run(&source, 0).unwrap();
        let result = apply_committed(&source, before, selection(0, 0), new_text, &mut affinity)
            .unwrap()
            .unwrap();
        assert!(result.patch.source.xaml().contains(expected_body));
        assert_eq!(result.patch.source.plain_text(), new_text);
        assert_eq!(affinity.owner(&result.patch.source), None);
        if new_text.is_empty() {
            // Multiple retained empty styles do not silently choose first/last.
            assert!(
                apply_committed(
                    &result.patch.source,
                    selection(0, 0),
                    selection(1, 1),
                    "X",
                    &mut affinity
                )
                .is_err()
            );
        }
    }
    let mut affinity = RunAffinity::default();
    for (before, after, text) in [
        (selection(0, 4), selection(1, 1), "XC"),
        (selection(0, 5), selection(1, 1), "X"),
    ] {
        assert!(apply_committed(&source, before, after, text, &mut affinity).is_err());
    }
    assert_eq!(source.wire(), &original);
}

#[test]
fn boundary_delete_crossing_paragraph_separator_stays_atomic() {
    let (source, _) = fixture(
        "a</Run><Run style=\"Normal\">b</Run></Paragraph><Paragraph><Run style=\"Normal\">c",
    );
    let wire = source.wire().clone();
    let mut affinity = RunAffinity::default();
    assert!(apply_committed(&source, selection(0, 4), selection(0, 0), "", &mut affinity).is_err());
    assert_eq!(source.wire(), &wire);
    assert_eq!(affinity.owner(&source), None);
}
