use super::*;

fn legacy_spec() -> AuthoredTextSeedSpec {
    let format = AuthoredStyleSnapshot {
        key: "Body".into(),
        font_family: "Noto Sans".into(),
        size_dip: "16".into(),
        weight: "Normal".into(),
        font_style: "Normal".into(),
        alignment: "Left".into(),
        foreground_rgba: [0, 0, 0, 255],
        background_rgba: [0, 0, 0, 0],
    };
    AuthoredTextSeedSpec {
        format: format.clone(),
        proofing_language: "ko-KR".into(),
        catalog: vec![format],
        text_decorations: None,
    }
}
fn typed() -> AuthoredRichText {
    AuthoredRichText::empty(AuthoredTextSeed::new(legacy_spec()).unwrap())
        .unwrap()
        .replace_in_run(0, 0, 0, "가&😀나")
        .unwrap()
        .unwrap()
        .source
}
fn range(start: usize, end: usize) -> TextSelection {
    TextSelection {
        paragraph: 0,
        anchor: start,
        focus: end,
    }
}
fn decoration(value: TextDecorations) -> TextFormat {
    TextFormat {
        text_decorations: Some(value),
        ..Default::default()
    }
}

#[test]
fn old_seed_wire_hash_scaffold_and_cold_own_remain_exact() {
    #[derive(Serialize)]
    struct LegacySeed<'a> {
        format: &'a AuthoredStyleSnapshot,
        proofing_language: &'a str,
        catalog: &'a [AuthoredStyleSnapshot],
    }
    #[derive(Serialize)]
    struct LegacyWire<'a> {
        version: u8,
        seed: &'a LegacySeed<'a>,
        seed_sha256: [u8; 32],
        current_xaml: &'a str,
    }
    let spec = legacy_spec();
    let old = LegacySeed {
        format: &spec.format,
        proofing_language: &spec.proofing_language,
        catalog: &spec.catalog,
    };
    let old_bytes = serde_json::to_vec(&old).unwrap();
    assert_eq!(serde_json::to_vec(&spec).unwrap(), old_bytes);
    let seed = AuthoredTextSeed::new(spec.clone()).unwrap();
    let empty = AuthoredRichText::empty(seed.clone()).unwrap();
    let expected = format!(
        "<FlowDocument xmlns=\"{XAML}\" xml:space=\"preserve\" xml:lang=\"ko-KR\" TextAlignment=\"Left\" FontFamily=\"Noto Sans\" Background=\"#00000000\"><Paragraph><Run style=\"Body\" FontWeight=\"Normal\" FontStyle=\"Normal\" FontSize=\"16\" Foreground=\"#FF000000\"></Run></Paragraph></FlowDocument>"
    );
    assert_eq!(empty.xaml(), expected);
    assert_eq!(
        seed.fingerprint(),
        <[u8; 32]>::from(Sha256::digest(&old_bytes))
    );
    let old_wire = serde_json::to_vec(&LegacyWire {
        version: 1,
        seed: &old,
        seed_sha256: seed.fingerprint(),
        current_xaml: &expected,
    })
    .unwrap();
    assert_eq!(serde_json::to_vec(empty.wire()).unwrap(), old_wire);
    let cold = AuthoredRichText::from_json(&old_wire, "").unwrap();
    assert_eq!(serde_json::to_vec(cold.wire()).unwrap(), old_wire);
    assert!(Arc::ptr_eq(
        &cold
            .creation_snapshot_for_run(0)
            .unwrap()
            .materialize()
            .unwrap(),
        cold.seed()
    ));
}

#[test]
fn fresh_authored_legacy_missing_is_resolved_only_after_scaffold_proof_and_range_edits_work() {
    let original = typed();
    let before = original.wire().clone();
    assert!(
        original
            .selection_text_decorations_supported(range(0, 4))
            .unwrap()
    );
    assert!(
        original
            .selection_text_decorations_supported(range(0, 0))
            .is_err()
    );
    assert_eq!(
        original
            .selection_format(range(0, 4))
            .unwrap()
            .text_decorations,
        Some(TextDecorations::None)
    );
    assert!(
        original
            .set_selection_format(range(0, 4), &decoration(TextDecorations::None))
            .unwrap()
            .is_none()
    );
    let patch = original
        .set_selection_format(range(1, 3), &decoration(TextDecorations::Underline))
        .unwrap()
        .unwrap();
    assert!(
        patch
            .source
            .selection_text_decorations_supported(range(0, 4))
            .unwrap()
    );
    assert_eq!(patch.source.plain_text(), "가&😀나");
    assert_eq!(patch.source.run_char_ranges().count(), 3);
    assert_eq!(
        patch
            .source
            .selection_format(range(1, 3))
            .unwrap()
            .text_decorations,
        Some(TextDecorations::Underline)
    );
    assert_eq!(
        patch
            .source
            .selection_format(range(0, 4))
            .unwrap()
            .text_decorations,
        None
    );
    assert_eq!(original.wire(), &before);
    let bytes = serde_json::to_vec(patch.source.wire()).unwrap();
    assert_eq!(bytes.len(), patch.source.budget().serialized_wire_bytes());
    let cold = AuthoredRichText::from_json(&bytes, "가&😀나").unwrap();
    assert_eq!(cold.xaml(), patch.source.xaml());
    let cleared = cold
        .set_selection_format(range(0, 4), &decoration(TextDecorations::None))
        .unwrap()
        .unwrap()
        .source;
    assert_eq!(
        cleared
            .selection_format(range(0, 4))
            .unwrap()
            .text_decorations,
        Some(TextDecorations::None)
    );
    assert!(
        cleared
            .set_selection_format(range(0, 4), &decoration(TextDecorations::None))
            .unwrap()
            .is_none()
    );
    assert!(
        original
            .set_selection_format(range(1, 1), &decoration(TextDecorations::Underline))
            .is_err()
    );
    assert_eq!(original.wire(), &before);
}

#[test]
fn selected_underlined_run_carries_current_decoration_to_a_new_empty_owner_without_seed_dedup() {
    let start = typed();
    let changed = start
        .set_selection_format(range(0, 4), &decoration(TextDecorations::Underline))
        .unwrap()
        .unwrap()
        .source;
    let snapshot = changed.creation_snapshot_for_run(0).unwrap();
    assert_eq!(snapshot.text_decorations(), TextDecorations::Underline);
    let seed = snapshot.materialize().unwrap();
    assert_eq!(
        seed.spec().text_decorations,
        Some(TextDecorations::Underline)
    );
    assert_eq!(
        seed.spec().catalog,
        legacy_spec().catalog,
        "current override must not fabricate native catalog style defaults"
    );
    let empty = AuthoredRichText::empty(seed).unwrap();
    assert!(empty.xaml().contains("TextDecorations=\"Underline\""));
    let inserted = empty
        .replace_in_run(0, 0, 0, "next")
        .unwrap()
        .unwrap()
        .source;
    assert_eq!(
        inserted
            .selection_format(range(0, 4))
            .unwrap()
            .text_decorations,
        Some(TextDecorations::Underline)
    );
    let split = inserted.split_paragraph_at_caret(range(2, 2)).unwrap();
    assert_eq!(split.source.plain_text(), "ne\nxt");
    assert_eq!(
        split
            .source
            .xaml()
            .matches("TextDecorations=\"Underline\"")
            .count(),
        2
    );
    let bytes = serde_json::to_vec(split.source.wire()).unwrap();
    assert_eq!(bytes.len(), split.source.budget().serialized_wire_bytes());
    assert_eq!(
        AuthoredRichText::from_json(&bytes, "ne\nxt")
            .unwrap()
            .xaml(),
        split.source.xaml()
    );
}

#[test]
fn optional_seed_is_strict_and_cold_forged_scaffold_cannot_obtain_generated_default() {
    let value = typed();
    let raw = serde_json::to_string(value.wire()).unwrap();
    let marker = "\"proofing_language\":\"ko-KR\"";
    for inserted in [
        "null",
        "\"OverLine\"",
        "\"Underline,Underline\"",
        "\"underline\"",
    ] {
        let bad = raw.replacen(
            marker,
            &format!("{marker},\"text_decorations\":{inserted}"),
            1,
        );
        assert!(
            AuthoredRichText::from_json(bad.as_bytes(), value.plain_text()).is_err(),
            "{inserted}"
        );
    }
    let duplicate = raw.replacen(
        marker,
        &format!("{marker},\"text_decorations\":\"None\",\"text_decorations\":\"None\""),
        1,
    );
    assert!(AuthoredRichText::from_json(duplicate.as_bytes(), value.plain_text()).is_err());
    let mut forged = value.wire().clone();
    forged.current_xaml = forged
        .current_xaml
        .replace("<Paragraph>", "<Paragraph FontStyle=\"Italic\">")
        .into();
    assert!(AuthoredRichText::from_wire(forged, value.plain_text()).is_err());
    let mut forged = value.wire().clone();
    forged.current_xaml = forged
        .current_xaml
        .replace(
            "Foreground=\"#FF000000\"",
            "Foreground=\"#FF000000\" TextDecorations=\"Underline,Strikethrough\"",
        )
        .into();
    assert!(AuthoredRichText::from_wire(forged, value.plain_text()).is_err());
    assert_eq!(
        value
            .selection_format(range(0, 4))
            .unwrap()
            .text_decorations,
        Some(TextDecorations::None)
    );
}
