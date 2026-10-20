use super::*;

fn fixture() -> AuthoredRichText {
    let format = AuthoredStyleSnapshot {
        key: "Heading 1".into(),
        font_family: "Explicit Family 한국".into(),
        size_dip: "17.2500".into(),
        weight: "Normal".into(),
        font_style: "Normal".into(),
        alignment: "Left".into(),
        foreground_rgba: [11, 22, 33, 44],
        background_rgba: [55, 66, 77, 88],
    };
    let seed = AuthoredTextSeed::new(AuthoredTextSeedSpec {
        catalog: vec![format.clone()],
        format,
        proofing_language: "ko-KR".into(),
        text_decorations: None,
    })
    .unwrap();
    AuthoredRichText::empty(seed)
        .unwrap()
        .replace_in_run(0, 0, 0, "가😀나다")
        .unwrap()
        .unwrap()
        .source
}

#[test]
fn captures_chosen_run_effective_four_properties_and_keeps_exact_other_authority() {
    let original = fixture();
    let changed = original
        .set_selection_format(
            TextSelection {
                paragraph: 0,
                anchor: 3,
                focus: 1,
            },
            &TextFormat {
                weight: Some(Weight::Bold),
                style: Some(FontStyle::Italic),
                size_dip: Some(DipSize::parse("29.1250").unwrap()),
                foreground_argb: Some([128, 1, 2, 3]),
                text_decorations: None,
            },
        )
        .unwrap()
        .unwrap()
        .source;
    let expected_base = original.seed.spec.format.clone();
    assert_eq!(changed.run_char_ranges().count(), 3);
    assert_eq!(changed.creation_format_for_run(0).unwrap(), expected_base);
    assert_eq!(changed.creation_format_for_run(2).unwrap(), expected_base);
    let format = changed.creation_format_for_run(1).unwrap();
    assert_eq!(format.key, "Heading 1");
    assert_eq!(format.font_family, "Explicit Family 한국");
    assert_eq!(format.background_rgba, [55, 66, 77, 88]);
    assert_eq!(format.alignment, "Left");
    assert_eq!(format.weight, "Bold");
    assert_eq!(format.font_style, "Italic");
    assert_eq!(format.size_dip, "29.1250");
    assert_eq!(format.foreground_rgba, [1, 2, 3, 128]);
    let wire = serde_json::to_vec(changed.wire()).unwrap();
    let seed = changed.creation_seed_for_run(1).unwrap();
    assert_eq!(seed.spec().format, format);
    assert_eq!(seed.spec().catalog, original.seed.spec.catalog);
    assert_eq!(seed.spec().proofing_language, "ko-KR");
    assert_eq!(seed.spec().catalog[0].size_dip, "17.2500");
    assert_eq!(serde_json::to_vec(changed.wire()).unwrap(), wire);
    let next = AuthoredRichText::empty(seed).unwrap();
    assert_eq!(next.plain_text(), "");
    assert!(next.xaml().contains("FontSize=\"29.1250\""));
    assert!(next.xaml().contains("Foreground=\"#80010203\""));
}

#[test]
fn unchanged_format_and_empty_runs_reuse_seed_and_cold_reopen_preserves_capture() {
    let original = fixture();
    let seed = original.creation_seed_for_run(0).unwrap();
    assert!(Arc::ptr_eq(&seed, original.seed()));
    let empty = AuthoredRichText::empty(seed).unwrap();
    assert_eq!(
        empty.creation_format_for_run(0).unwrap().size_dip,
        "17.2500"
    );
    assert!(Arc::ptr_eq(
        &empty.creation_seed_for_run(0).unwrap(),
        empty.seed()
    ));
    let serialized = serde_json::to_vec(original.wire()).unwrap();
    let cold = AuthoredRichText::from_json(&serialized, original.plain_text()).unwrap();
    assert_eq!(
        cold.creation_format_for_run(0).unwrap(),
        original.creation_format_for_run(0).unwrap()
    );
    assert_eq!(serde_json::to_vec(cold.wire()).unwrap(), serialized);
}

#[test]
fn retained_snapshot_is_source_version_bound_and_has_no_current_xml_or_catalog_copy() {
    let original = fixture();
    let before = original.creation_snapshot_for_run(0).unwrap();
    assert!(Arc::ptr_eq(&before.seed, original.seed()));
    let bold = original
        .set_selection_format(
            TextSelection {
                paragraph: 0,
                anchor: 0,
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
    let captured = bold.creation_snapshot_for_run(0).unwrap();
    assert!(Arc::ptr_eq(&captured.seed, original.seed()));
    assert_eq!(captured.format().weight, "Bold");
    assert_eq!(before.format().weight, "Normal");
    let normal_again = bold
        .set_selection_format(
            TextSelection {
                paragraph: 0,
                anchor: 0,
                focus: 4,
            },
            &TextFormat {
                weight: Some(Weight::Normal),
                ..Default::default()
            },
        )
        .unwrap()
        .unwrap()
        .source;
    assert_eq!(
        normal_again.creation_format_for_run(0).unwrap().weight,
        "Normal"
    );
    assert_eq!(captured.materialize().unwrap().spec().format.weight, "Bold");
    assert!(Arc::ptr_eq(&before.materialize().unwrap(), original.seed()));
    let xml = original.xaml().to_owned();
    assert!(original.creation_format_for_run(usize::MAX).is_err());
    assert!(original.creation_snapshot_for_run(1).is_err());
    assert!(original.creation_seed_for_run(1).is_err());
    assert_eq!(original.xaml(), xml);
}
