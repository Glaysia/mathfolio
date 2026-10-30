use super::*;
use zip::write::SimpleFileOptions;

const CATALOG: &[u8] =
    include_bytes!("../../../../tests/reference/rich-text/lcc-standard-presentation.xml");
fn package(xml: &str) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, body) in [
        (INNER, xml.as_bytes()),
        ("Resources/keep.bin", &[0, 255, 9, 13, 10][..]),
    ] {
        writer
            .start_file(name, SimpleFileOptions::default())
            .unwrap();
        writer.write_all(body).unwrap();
    }
    writer.finish().unwrap().into_inner()
}
fn native(xml: &str) -> ValidatedRichText {
    let authority = OriginalTextAuthority::from_package(
        NativeTextIdentity {
            owner_region_id: "17".into(),
            item_idref: "FlowDocument0".into(),
            package_part: "mathcad/xaml/FlowDocument0.XamlPackage".into(),
            catalog_part: "mathcad/settings/presentation.xml".into(),
            owner_xml_sha256: hash(b"fixed alignment fixture Text wrapper"),
        },
        package(xml),
        CATALOG.to_vec(),
    )
    .unwrap();
    let plain = parse_source(xml, &authority.keys).unwrap().plain;
    ValidatedRichText::from_original(authority, &plain).unwrap()
}
fn xml(paragraph: &str, body: &str) -> String {
    format!(
        "<Section xmlns=\"{XAML}\" xml:lang=\"ko-KR\" FlowDirection=\"LeftToRight\" TextAlignment=\"Left\" FontFamily=\"Tahoma\" FontSize=\"16\" Foreground=\"#FF123456\" FontWeight=\"Normal\" FontStyle=\"Normal\"><Paragraph{paragraph}><Run style=\"Normal\" Background=\"#FF662D91\">{body}</Run></Paragraph></Section>"
    )
}
fn selection(anchor: usize, focus: usize) -> TextSelection {
    TextSelection {
        paragraph: 0,
        anchor,
        focus,
    }
}
fn authored() -> AuthoredRichText {
    let style = AuthoredStyleSnapshot {
        key: "Body".into(),
        font_family: "Tahoma".into(),
        size_dip: "16".into(),
        weight: "Normal".into(),
        font_style: "Normal".into(),
        alignment: "Left".into(),
        foreground_rgba: [18, 52, 86, 255],
        background_rgba: [255, 255, 255, 255],
    };
    let seed = AuthoredTextSeed::new(AuthoredTextSeedSpec {
        format: style.clone(),
        proofing_language: "ko-KR".into(),
        catalog: vec![style],
        text_decorations: None,
    })
    .unwrap();
    AuthoredRichText::empty(seed).unwrap()
}

#[test]
fn native_collapsed_alignment_only_changes_owning_paragraph_and_cold_source() {
    let raw = xml(" xml:lang='ko-KR' \t", "한글😀&amp;AB");
    let old = native(&raw);
    let original_package = old.authority.original_package().to_vec();
    for alignment in [
        ParagraphAlignment::Center,
        ParagraphAlignment::Right,
        ParagraphAlignment::Justify,
    ] {
        let patch = old
            .set_paragraph_alignment(selection(2, 2), alignment)
            .unwrap()
            .unwrap();
        let expected = raw.replacen(
            "<Paragraph xml:lang='ko-KR' \t>",
            &format!(
                "<Paragraph xml:lang='ko-KR' TextAlignment=\"{}\" \t>",
                alignment.as_str()
            ),
            1,
        );
        assert_eq!(patch.source.xaml(), expected); // independent literal splice
        assert_eq!(patch.source.plain_text(), "한글😀&AB");
        assert_eq!(patch.selection, selection(2, 2));
        assert_eq!(old.xaml(), raw);
        assert_eq!(old.authority.original_package(), original_package);
        let json = serde_json::to_vec(patch.source.wire()).unwrap();
        let wire: RichTextWire = serde_json::from_slice(&json).unwrap();
        let reopened =
            ValidatedRichText::from_wire(wire, old.authority.clone(), old.plain_text()).unwrap();
        assert_eq!(reopened.xaml(), expected);
        assert!(reopened.prove_format_only().is_ok());
        assert!(
            reopened
                .set_paragraph_alignment(selection(1, 4), alignment)
                .unwrap()
                .is_none()
        );
        // Actual source package retains non-XAML members and immutable original.
        let output = reopened.export_package().unwrap();
        let mut zip = ZipArchive::new(Cursor::new(output.as_ref())).unwrap();
        let mut kept = Vec::new();
        zip.by_name("Resources/keep.bin")
            .unwrap()
            .read_to_end(&mut kept)
            .unwrap();
        assert_eq!(kept, [0, 255, 9, 13, 10]);
        assert_eq!(old.authority.original_package(), original_package);
    }
    assert!(
        old.set_paragraph_alignment(selection(0, 0), ParagraphAlignment::Left)
            .unwrap()
            .is_none()
    );
}

#[test]
fn authored_local_alignment_keeps_old_seed_wire_and_snapshot_undo_compatible() {
    let old = authored();
    let old_json = serde_json::to_vec(old.wire()).unwrap();
    assert!(!old.xaml().contains("<Paragraph TextAlignment"));
    let changed = old
        .set_paragraph_alignment(selection(0, 0), ParagraphAlignment::Right)
        .unwrap()
        .unwrap()
        .source;
    assert_eq!(
        changed.xaml(),
        old.xaml()
            .replacen("<Paragraph>", "<Paragraph TextAlignment=\"Right\">", 1)
    );
    assert_eq!(changed.seed().fingerprint(), old.seed().fingerprint());
    let changed_json = serde_json::to_value(changed.wire()).unwrap();
    let old_value = serde_json::to_value(old.wire()).unwrap();
    assert_eq!(changed_json["seed_sha256"], old_value["seed_sha256"]);
    assert_eq!(changed_json["seed"], old_value["seed"]);
    assert_eq!(
        changed.budget().serialized_wire_bytes(),
        serde_json::to_vec(changed.wire()).unwrap().len()
    );
    assert!(changed.budget().serialized_wire_bytes() > old.budget().serialized_wire_bytes());
    let typed = changed
        .replace_in_run(0, 0, 0, "한😀AB")
        .unwrap()
        .unwrap()
        .source;
    let reopened =
        AuthoredRichText::from_json(&serde_json::to_vec(typed.wire()).unwrap(), "한😀AB").unwrap();
    assert_eq!(reopened.xaml(), typed.xaml());
    assert!(
        reopened
            .xaml()
            .contains("<Paragraph TextAlignment=\"Right\">")
    );
    // History can store ordinary exact source DTO snapshots, not renderer state.
    let undo = AuthoredRichText::from_json(&old_json, "").unwrap();
    assert_eq!(serde_json::to_vec(undo.wire()).unwrap(), old_json);
    assert_eq!(serde_json::to_vec(old.wire()).unwrap(), old_json);
    assert!(
        reopened
            .set_paragraph_alignment(selection(1, 3), ParagraphAlignment::Right)
            .unwrap()
            .is_none()
    );
}

#[test]
fn block_mask_rejects_root_run_metadata_removal_unknown_and_rtl_forgery() {
    let old = native(&xml(" TextAlignment='Center'", "ab"));
    let patch = old
        .set_paragraph_alignment(selection(0, 2), ParagraphAlignment::Right)
        .unwrap()
        .unwrap();
    let good = patch.source.xaml();
    for bad in [
        good.replace(" TextAlignment='Right'", ""),
        good.replace("TextAlignment='Right'", "TextAlignment='Stretch'"),
        good.replacen("TextAlignment=\"Left\"", "TextAlignment=\"Right\"", 1),
        good.replace("<Paragraph", "<Paragraph FlowDirection=\"RightToLeft\""),
        good.replace(
            "style=\"Normal\"",
            "style=\"Normal\" TextAlignment=\"Right\"",
        ),
        good.replace("FontFamily=\"Tahoma\"", "FontFamily=\"Arial\""),
        good.replace("xml:lang=\"ko-KR\"", "xml:lang=\"en-US\""),
    ] {
        let mut wire = patch.source.wire().clone();
        wire.current_xaml = bad.into();
        wire.visual_fingerprint = Default::default();
        assert!(ValidatedRichText::from_wire(wire, old.authority.clone(), "ab").is_err());
    }
    let rtl = native(&xml(" FlowDirection=\"RightToLeft\"", "ab"));
    assert!(
        rtl.set_paragraph_alignment(selection(1, 1), ParagraphAlignment::Left)
            .is_err()
    );
    let unknown_flow = native(&xml("", "ab").replace(" FlowDirection=\"LeftToRight\"", ""));
    assert!(
        unknown_flow
            .set_paragraph_alignment(selection(1, 1), ParagraphAlignment::Right)
            .is_err()
    );
}

#[test]
fn commands_reject_multiple_paragraphs_bounds_and_budget_atomically() {
    let raw = xml("", "ab");
    let multi = native(&raw.replace(
        "</Paragraph>",
        "</Paragraph><Paragraph><Run style=\"Normal\">second</Run></Paragraph>",
    ));
    assert!(
        multi
            .set_paragraph_alignment(selection(0, 1), ParagraphAlignment::Center)
            .is_err()
    );
    assert_eq!(multi.plain_text(), "ab\nsecond");
    let old = native(&raw);
    for invalid in [
        TextSelection {
            paragraph: 1,
            anchor: 0,
            focus: 0,
        },
        selection(3, 3),
    ] {
        assert!(
            old.set_paragraph_alignment(invalid, ParagraphAlignment::Right)
                .is_err()
        );
    }
    let empty = authored();
    let before = serde_json::to_vec(empty.wire()).unwrap();
    // The existing exact whole-wire budget, not only XML length, remains active.
    let mut json: serde_json::Value = serde_json::from_slice(&before).unwrap();
    let source = empty.xaml().replace(
        "></Run>",
        &format!(">{}</Run>", "a".repeat(MAX_SOURCE_BYTES)),
    );
    json["current_xaml"] = source.into();
    assert!(
        AuthoredRichText::from_json(
            &serde_json::to_vec(&json).unwrap(),
            &"a".repeat(MAX_SOURCE_BYTES)
        )
        .is_err()
    );
    assert_eq!(serde_json::to_vec(empty.wire()).unwrap(), before);
}

#[test]
fn native_xml_and_authored_exact_wire_budget_reject_alignment_without_mutation() {
    let shell = xml("", "");
    let raw = xml("", &"a".repeat(MAX_SOURCE_BYTES - shell.len() - 4));
    let old = native(&raw);
    assert_eq!(old.xaml().len(), MAX_SOURCE_BYTES - 4);
    assert!(
        old.set_paragraph_alignment(selection(0, 0), ParagraphAlignment::Center)
            .is_err()
    );
    assert_eq!(old.xaml(), raw);
    let empty = authored();
    let count = MAX_SOURCE_BYTES - empty.budget().serialized_wire_bytes() - 4;
    let value = empty
        .replace_in_run(0, 0, 0, &"a".repeat(count))
        .unwrap()
        .unwrap()
        .source;
    assert_eq!(value.budget().serialized_wire_bytes(), MAX_SOURCE_BYTES - 4);
    let old_wire = serde_json::to_vec(value.wire()).unwrap();
    assert!(
        value
            .set_paragraph_alignment(selection(count, count), ParagraphAlignment::Right)
            .is_err()
    );
    assert_eq!(serde_json::to_vec(value.wire()).unwrap(), old_wire);
}

#[test]
fn same_plain_alignment_receipt_never_accepts_content_or_unrelated_block_changes() {
    let old = native(&xml("", "a&amp;b"));
    let aligned = old
        .set_paragraph_alignment(selection(1, 1), ParagraphAlignment::Justify)
        .unwrap()
        .unwrap()
        .source;
    assert!(aligned.prove_format_only().is_ok());
    let content = aligned
        .replace_in_run(0, 1, 2, "X")
        .unwrap()
        .unwrap()
        .source;
    assert!(content.prove_preserved_metadata().is_ok());
    assert!(content.prove_format_only().is_err());
    let old_json = serde_json::to_vec(old.wire()).unwrap();
    let restored = ValidatedRichText::from_wire(
        serde_json::from_slice(&old_json).unwrap(),
        old.authority.clone(),
        old.plain_text(),
    )
    .unwrap();
    assert_eq!(restored.xaml(), old.xaml());
    assert_eq!(serde_json::to_vec(restored.wire()).unwrap(), old_json);
}

#[test]
fn authored_alignment_permission_cannot_mask_seed_root_language_or_run_key_changes() {
    let old = authored();
    let aligned = old
        .set_paragraph_alignment(selection(0, 0), ParagraphAlignment::Center)
        .unwrap()
        .unwrap()
        .source;
    let before = serde_json::to_vec(aligned.wire()).unwrap();
    let good = aligned.xaml();
    for bad in [
        good.replacen(
            "<FlowDocument ",
            "<FlowDocument FlowDirection=\"RightToLeft\" ",
            1,
        ),
        good.replace("xml:lang=\"ko-KR\"", "xml:lang=\"en-US\""),
        good.replace(
            "<Paragraph TextAlignment=\"Center\">",
            "<Paragraph xml:lang=\"en-US\" TextAlignment=\"Center\">",
        ),
        good.replace("style=\"Body\"", "style=\"Other\""),
        good.replace("FontFamily=\"Tahoma\"", "FontFamily=\"Arial\""),
    ] {
        assert_ne!(bad, good, "fixture must actually change protected source");
        let mut forged = serde_json::to_value(aligned.wire()).unwrap();
        forged["current_xaml"] = bad.into();
        assert!(AuthoredRichText::from_json(&serde_json::to_vec(&forged).unwrap(), "").is_err());
    }
    assert_eq!(serde_json::to_vec(aligned.wire()).unwrap(), before);
    assert_eq!(aligned.seed().fingerprint(), old.seed().fingerprint());
}

#[test]
fn mapped_paragraph_clones_cannot_gain_alignment_by_changing_one_or_all_wrappers() {
    let old = native(&xml("", "한😀AB"));
    let split = old
        .split_paragraph_at_caret(selection(2, 2))
        .unwrap()
        .patch
        .source;
    assert_eq!(split.plain_text(), "한😀\nAB");
    let before = serde_json::to_vec(split.wire()).unwrap();
    let reopened = ValidatedRichText::from_wire(
        serde_json::from_slice(&before).unwrap(),
        old.authority.clone(),
        split.plain_text(),
    )
    .unwrap();
    assert_eq!(reopened.xaml(), split.xaml());
    for bad in [
        split
            .xaml()
            .replacen("<Paragraph>", "<Paragraph TextAlignment=\"Right\">", 1),
        split
            .xaml()
            .replace("<Paragraph>", "<Paragraph TextAlignment=\"Right\">"),
    ] {
        assert_ne!(bad, split.xaml());
        let mut wire = split.wire().clone();
        wire.current_xaml = bad.into();
        wire.visual_fingerprint = Default::default();
        assert!(
            ValidatedRichText::from_wire(wire, old.authority.clone(), split.plain_text()).is_err()
        );
    }
    assert_eq!(serde_json::to_vec(split.wire()).unwrap(), before);
}

#[test]
fn authored_nonleft_to_left_restores_exact_seed_scaffold_wire_and_existing_enter() {
    let before = authored()
        .replace_in_run(0, 0, 0, "한😀AB")
        .unwrap()
        .unwrap()
        .source;
    let wire = serde_json::to_vec(before.wire()).unwrap();
    for desired in [
        ParagraphAlignment::Center,
        ParagraphAlignment::Right,
        ParagraphAlignment::Justify,
    ] {
        let aligned = before
            .set_paragraph_alignment(selection(2, 2), desired)
            .unwrap()
            .unwrap()
            .source;
        let restored = aligned
            .set_paragraph_alignment(selection(2, 2), ParagraphAlignment::Left)
            .unwrap()
            .unwrap();
        assert_eq!(restored.selection, selection(2, 2));
        assert_eq!(restored.source.xaml(), before.xaml());
        assert_eq!(serde_json::to_vec(restored.source.wire()).unwrap(), wire);
        assert_eq!(
            restored.source.seed().fingerprint(),
            before.seed().fingerprint()
        );
        assert_eq!(restored.source.plain_text(), "한😀AB");
        let cold = AuthoredRichText::from_json(
            &serde_json::to_vec(restored.source.wire()).unwrap(),
            "한😀AB",
        )
        .unwrap();
        let split = cold.split_paragraph_at_caret(selection(2, 2)).unwrap();
        assert_eq!(split.source.plain_text(), "한😀\nAB");
        assert_eq!(split.source.xaml().matches("<Paragraph>").count(), 2);
        assert!(!split.source.xaml().contains("<Paragraph TextAlignment"));
        assert_eq!(
            split.source.seed().fingerprint(),
            before.seed().fingerprint()
        );
    }
    assert!(
        before
            .set_paragraph_alignment(selection(1, 1), ParagraphAlignment::Left)
            .unwrap()
            .is_none()
    );
    assert_eq!(serde_json::to_vec(before.wire()).unwrap(), wire);
}

#[test]
fn stored_canonical_authored_explicit_left_normalizes_once_without_changing_run_or_seed() {
    let original = authored()
        .replace_in_run(0, 0, 0, "ab😀")
        .unwrap()
        .unwrap()
        .source;
    let mut json = serde_json::to_value(original.wire()).unwrap();
    json["current_xaml"] = original
        .xaml()
        .replace("<Paragraph>", "<Paragraph TextAlignment=\"Left\">")
        .into();
    let old = AuthoredRichText::from_json(&serde_json::to_vec(&json).unwrap(), "ab😀").unwrap();
    let result = old
        .set_paragraph_alignment(selection(1, 1), ParagraphAlignment::Left)
        .unwrap()
        .unwrap();
    assert_eq!(result.source.xaml(), original.xaml());
    assert_eq!(
        serde_json::to_vec(result.source.wire()).unwrap(),
        serde_json::to_vec(original.wire()).unwrap()
    );
    assert_eq!(result.selection, selection(1, 1));
    assert!(
        result
            .source
            .set_paragraph_alignment(selection(1, 1), ParagraphAlignment::Left)
            .unwrap()
            .is_none()
    );
    assert_eq!(old.xaml(), json["current_xaml"].as_str().unwrap());
}

#[test]
fn original_native_explicit_left_is_never_removed_by_authored_normalization_policy() {
    let original = native(&xml(" TextAlignment='Left'", "ab"));
    assert!(
        original
            .set_paragraph_alignment(selection(1, 1), ParagraphAlignment::Left)
            .unwrap()
            .is_none()
    );
    let right = original
        .set_paragraph_alignment(selection(1, 1), ParagraphAlignment::Right)
        .unwrap()
        .unwrap()
        .source;
    let left = right
        .set_paragraph_alignment(selection(1, 1), ParagraphAlignment::Left)
        .unwrap()
        .unwrap()
        .source;
    assert_eq!(left.xaml(), original.xaml());
    assert!(left.xaml().contains("<Paragraph TextAlignment='Left'>"));
    let mut forged = left.wire().clone();
    forged.current_xaml = left.xaml().replace(" TextAlignment='Left'", "").into();
    forged.visual_fingerprint = Default::default();
    assert!(ValidatedRichText::from_wire(forged, original.authority.clone(), "ab").is_err());
}
