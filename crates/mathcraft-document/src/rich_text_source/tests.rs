use super::*;
use serde::{Deserialize, Serialize};
use zip::write::SimpleFileOptions;

const ONE: &str = include_str!("../../../../tests/reference/rich-text/sample-01.xaml");
const TWO: &str = include_str!("../../../../tests/reference/rich-text/sample-02.xaml");
const CATALOG: &[u8] =
    include_bytes!("../../../../tests/reference/rich-text/lcc-standard-presentation.xml");
const PACKAGE_ONE: &[u8] =
    include_bytes!("../../../../tests/reference/rich-text/sample-01.XamlPackage");

fn identity() -> NativeTextIdentity {
    NativeTextIdentity {
        owner_region_id: "17".into(),
        item_idref: "FlowDocument0".into(),
        package_part: "mathcad/xaml/FlowDocument0.XamlPackage".into(),
        catalog_part: "mathcad/settings/presentation.xml".into(),
        owner_xml_sha256: hash(b"synthetic original Text wrapper"),
    }
}
fn package(source: &str) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    writer
        .set_raw_comment(b"immutable-original-comment".to_vec().into_boxed_slice())
        .unwrap();
    writer
        .start_file(
            INNER,
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated),
        )
        .unwrap();
    writer.write_all(source.as_bytes()).unwrap();
    writer
        .start_file(
            "Resources/uninterpreted.bin",
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored),
        )
        .unwrap();
    writer.write_all(&[0, 255, 128, 3, 7]).unwrap();
    writer.finish().unwrap().into_inner()
}
fn authority(source: &str) -> Arc<OriginalTextAuthority> {
    OriginalTextAuthority::from_package(identity(), package(source), CATALOG.to_vec()).unwrap()
}
fn validated(source: &str) -> ValidatedRichText {
    let a = authority(source);
    let plain = parse_source(source, &a.keys).unwrap().plain;
    ValidatedRichText::from_original(a, &plain).unwrap()
}
fn selection(start: usize, end: usize) -> TextSelection {
    TextSelection {
        paragraph: 0,
        anchor: start,
        focus: end,
    }
}
fn format_bold(weight: Weight) -> TextFormat {
    TextFormat {
        weight: Some(weight),
        ..Default::default()
    }
}
fn minimal(body: &str) -> String {
    format!(
        "<Section xmlns=\"{XAML}\" FontWeight=\"Normal\" FontStyle=\"Normal\" FontSize=\"16\" Foreground=\"#FFFFFFFF\"><Paragraph><Run style=\"Normal\">{body}</Run></Paragraph></Section>"
    )
}

#[test]
fn actual_fixture_identity_plain_and_catalog_remain_separate() {
    let a = OriginalTextAuthority::from_package(identity(), PACKAGE_ONE.to_vec(), CATALOG.to_vec())
        .unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(PACKAGE_ONE)),
        "e174bf7a70e8bc397682c436878c866c7aa7c4796c8e3e83989f2081eba3be2a"
    );
    assert_eq!(
        format!("{:x}", Sha256::digest(ONE.as_bytes())),
        "f024b20ce857ef024cffa4e5e03dd21ebac83c354f3c9eaca2eacc1246e4b653"
    );
    assert!(a.keys.contains("Normal"));
    assert!(a.keys.contains("Normal~639058311443748180"));
    let source = ValidatedRichText::from_original(a.clone(), "LCCL-S 시스템 설계").unwrap();
    assert_eq!(source.xaml(), ONE);
    assert_eq!(a.original_package(), PACKAGE_ONE);
    assert_eq!(a.original_catalog(), CATALOG);
    // Catalog Normal is not substituted for source inherited Bold/16/white.
    assert!(
        source
            .set_selection_format(
                selection(1, 4),
                &TextFormat {
                    weight: Some(Weight::Bold),
                    size_dip: Some(DipSize::parse("16.0").unwrap()),
                    foreground_argb: Some([255, 255, 255, 255]),
                    ..Default::default()
                }
            )
            .unwrap()
            .is_none()
    );
}

#[test]
fn selected_run_split_preserves_exact_surrounding_bytes_keys_and_attributes() {
    let source = validated(ONE);
    let run = source.index.runs[0].clone();
    let patch = source
        .set_selection_format(selection(1, 4), &format_bold(Weight::Normal))
        .unwrap()
        .unwrap();
    assert!(patch.source.xaml().starts_with(&ONE[..run.range.start]));
    assert!(patch.source.xaml().ends_with(&ONE[run.range.end..]));
    assert_eq!(patch.source.plain_text(), source.plain_text());
    assert_eq!(patch.selection, selection(1, 4));
    let doc = Document::parse(patch.source.xaml()).unwrap();
    let runs: Vec<_> = doc
        .descendants()
        .filter(|n| n.has_tag_name((XAML, "Run")))
        .collect();
    assert_eq!(
        runs.iter().map(|n| n.text().unwrap()).collect::<Vec<_>>(),
        ["L", "CCL", "-", "S 시스템 설계"]
    );
    assert_eq!(
        runs.iter()
            .map(|n| n.attribute("FontWeight"))
            .collect::<Vec<_>>(),
        [None, Some("Normal"), None, None]
    );
    assert!(runs.iter().all(|n| n.attribute("style") == Some("Normal")
        && n.attribute("Background") == Some("#FF662D91")));
    assert_eq!(source.xaml(), ONE); // immutable candidate, original unchanged
}

#[test]
fn korean_selection_color_style_and_size_preserve_source_precision() {
    let source = validated(TWO);
    let patch = source
        .set_selection_format(
            selection(3, 6),
            &TextFormat {
                style: Some(FontStyle::Italic),
                size_dip: Some(DipSize::parse("17.3333333333333").unwrap()),
                foreground_argb: Some([255, 16, 32, 48]),
                ..Default::default()
            },
        )
        .unwrap()
        .unwrap();
    let doc = Document::parse(patch.source.xaml()).unwrap();
    let runs: Vec<_> = doc
        .descendants()
        .filter(|n| n.has_tag_name((XAML, "Run")))
        .collect();
    assert_eq!(runs[1].text(), Some("커플링"));
    assert_eq!(runs[1].attribute("FontSize"), Some("17.3333333333333"));
    assert_eq!(runs[1].attribute("FontStyle"), Some("Italic"));
    assert_eq!(runs[1].attribute("Foreground"), Some("#FF102030"));
    assert_eq!(patch.source.plain_text(), "설계 커플링 계수");
}

#[test]
fn cross_run_range_preserves_language_and_reverse_selection_direction() {
    let source = validated(ONE);
    let selected = selection(8, 4);
    let patch = source
        .set_selection_format(
            selected,
            &TextFormat {
                style: Some(FontStyle::Italic),
                ..Default::default()
            },
        )
        .unwrap()
        .unwrap();
    assert_eq!(patch.selection, selected);
    let doc = Document::parse(patch.source.xaml()).unwrap();
    let runs: Vec<_> = doc
        .descendants()
        .filter(|n| n.has_tag_name((XAML, "Run")))
        .collect();
    assert_eq!(
        runs.iter().map(|n| n.text().unwrap()).collect::<Vec<_>>(),
        ["LCCL", "-", "S 시", "스템 설계"]
    );
    assert_eq!(runs[1].attribute((XML, "lang")), Some("ko-kr"));
    assert_eq!(runs[2].attribute((XML, "lang")), None);
}

#[test]
fn exact_raw_entities_supplementary_scalar_and_original_quotes_survive_split() {
    let source = minimal("가&#x1F600;&amp;나").replace("style=\"Normal\"", "style='Normal'");
    let source = validated(&source);
    let patch = source
        .set_selection_format(selection(1, 3), &format_bold(Weight::Bold))
        .unwrap()
        .unwrap();
    assert_eq!(patch.source.plain_text(), "가😀&나");
    assert!(patch.source.xaml().contains(">&#x1F600;&amp;</Run>"));
    assert_eq!(patch.source.xaml().matches("style='Normal'").count(), 3);
}

#[test]
fn custom_key_is_not_folded_to_display_name_and_catalog_stays_exact() {
    let source = TWO.replace("style=\"Normal\"", "style=\"Normal~639058311443748180\"");
    let source = validated(&source);
    let patch = source
        .set_selection_format(selection(3, 6), &format_bold(Weight::Bold))
        .unwrap()
        .unwrap();
    assert_eq!(
        patch
            .source
            .xaml()
            .matches("style=\"Normal~639058311443748180\"")
            .count(),
        3
    );
    assert_eq!(patch.source.authority.original_catalog(), CATALOG);
}

#[test]
fn same_effective_value_is_noop_even_when_decimal_lexeme_differs() {
    let source = validated(ONE);
    assert!(
        source
            .set_selection_format(selection(1, 4), &format_bold(Weight::Bold))
            .unwrap()
            .is_none()
    );
    let patch = source
        .set_selection_format(selection(1, 4), &format_bold(Weight::Normal))
        .unwrap()
        .unwrap();
    assert!(
        patch
            .source
            .set_selection_format(selection(1, 4), &format_bold(Weight::Normal))
            .unwrap()
            .is_none()
    );
    assert_eq!(canonical_decimal("00016.000").unwrap(), "16");
    assert_eq!(
        canonical_decimal("13.3333333333333").unwrap(),
        "13.3333333333333"
    );
}

#[test]
fn missing_effective_source_value_does_not_use_catalog_as_initial_default() {
    let source = minimal("x").replace(" FontWeight=\"Normal\"", "");
    let source = validated(&source);
    assert!(
        source
            .set_selection_format(selection(0, 1), &format_bold(Weight::Bold))
            .is_err()
    );
    assert_eq!(source.plain_text(), "x");
}

#[test]
fn namespace_prefix_and_attribute_entity_are_preserved_without_new_namespace_guess() {
    let source = minimal("a&amp;b")
        .replace("<Section xmlns=", "<x:Section xmlns:x=")
        .replace("</Section>", "</x:Section>")
        .replace("<Paragraph>", "<x:Paragraph>")
        .replace("</Paragraph>", "</x:Paragraph>")
        .replace("<Run ", "<x:Run ")
        .replace("</Run>", "</x:Run>");
    let source = validated(&source);
    let patch = source
        .set_selection_format(selection(1, 2), &format_bold(Weight::Bold))
        .unwrap()
        .unwrap();
    assert!(patch.source.xaml().contains("<x:Run"));
    assert!(patch.source.xaml().contains(">&amp;</x:Run>"));
    assert_eq!(patch.source.plain_text(), "a&b");
}

#[test]
fn all_unsupported_source_shapes_are_rejected_not_flattened() {
    let base = minimal("text");
    for source in [
        base.replace("text", "<Span>text</Span>"),
        base.replace("text", "<Hyperlink>text</Hyperlink>"),
        base.replace("text", "te<!--x-->xt"),
        base.replace("text", "te<?x y?>xt"),
        base.replace("text", "te\rxt"),
        base.replace("<Run ", "<Run Name=\"Unique\" "),
        base.replace("<Run ", "<Run Text=\"text\" "),
        base.replace("<Run ", "<Run FontWeight=\"SemiBold\" "),
        base.replace("<Paragraph>", "<Paragraph> "),
        base.replace("style=\"Normal\"", "style=\"Unknown\""),
        base.replace("<Run ", "<Run UnknownProperty=\"v\" "),
        base.replace("text", "<LineBreak/>"),
        base.replace("<Paragraph>", "<Paragraph><Table/>"),
        base.replace(
            "<Run style=\"Normal\">text</Run>",
            "<Run style=\"Normal\"/>",
        ),
        base.replace("text", "<![CDATA[text]]>"),
        format!("<!DOCTYPE Section [<!ENTITY z 'x'>]>{base}"),
        base.replace("FontSize=\"16\"", "FontSize=\"{{Binding Size}}\""),
    ] {
        let candidate =
            OriginalTextAuthority::from_package(identity(), package(&source), CATALOG.to_vec());
        assert!(candidate.is_err(), "accepted unsupported source: {source}");
    }
}

#[test]
fn catalog_origin_and_duplicate_key_guards_fail_closed() {
    for catalog in [
        String::from_utf8(CATALOG.to_vec()).unwrap().replace(
            "style-key=\"Normal~639058311443748180\"",
            "style-key=\"Normal\"",
        ),
        String::from_utf8(CATALOG.to_vec())
            .unwrap()
            .replace("based-on-key=\"\"", "based-on-key=\"Missing\""),
        String::from_utf8(CATALOG.to_vec())
            .unwrap()
            .replace(PRESENTATION, "urn:wrong"),
    ] {
        assert!(
            OriginalTextAuthority::from_package(identity(), package(ONE), catalog.into_bytes())
                .is_err()
        );
    }
    let source = validated(ONE);
    let mut wire = source.wire().clone();
    wire.origin.identity.owner_region_id = "different-owner".into();
    assert!(
        ValidatedRichText::from_wire(wire, source.authority.clone(), source.plain_text()).is_err()
    );
    let mut wire = source.wire().clone();
    wire.origin.catalog_sha256[0] ^= 1;
    assert!(
        ValidatedRichText::from_wire(wire, source.authority.clone(), source.plain_text()).is_err()
    );
}

#[test]
fn wire_is_inert_strict_and_plain_projection_mismatch_cannot_authorize_edit() {
    let source = validated(ONE);
    let json = serde_json::to_string(source.wire()).unwrap();
    let roundtrip: RichTextWire = serde_json::from_str(&json).unwrap();
    assert!(
        ValidatedRichText::from_wire(roundtrip.clone(), source.authority.clone(), "wrong").is_err()
    );
    assert_eq!(
        ValidatedRichText::from_wire(roundtrip, source.authority.clone(), source.plain_text())
            .unwrap()
            .xaml(),
        ONE
    );
    let duplicated = json.replacen("\"version\":1", "\"version\":1,\"version\":1", 1);
    assert!(serde_json::from_str::<RichTextWire>(&duplicated).is_err());
    let unknown = json.replacen("\"version\":1", "\"extra\":0,\"version\":1", 1);
    assert!(serde_json::from_str::<RichTextWire>(&unknown).is_err());
    let mut wire = source.wire().clone();
    wire.version = 2;
    assert!(
        ValidatedRichText::from_wire(wire, source.authority.clone(), source.plain_text()).is_err()
    );
}

#[test]
fn optional_carrier_absent_preserves_existing_plain_text_json_exactly() {
    #[derive(Debug, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Region {
        id: u64,
        content: Content,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rich_text: Option<Box<RichTextWire>>,
    }
    #[derive(Debug, Serialize, Deserialize)]
    enum Content {
        Text(String),
    }
    let old = r#"{"id":3,"content":{"Text":"hello"}}"#;
    let region: Region = serde_json::from_str(old).unwrap();
    assert_eq!(serde_json::to_string(&region).unwrap(), old);
    let source = validated(ONE);
    let region = Region {
        id: 3,
        content: Content::Text(source.plain_text().into()),
        rich_text: Some(Box::new(source.wire().clone())),
    };
    let reopened: Region = serde_json::from_str(&serde_json::to_string(&region).unwrap()).unwrap();
    let Content::Text(plain) = &reopened.content;
    ValidatedRichText::from_wire(
        *reopened.rich_text.unwrap(),
        source.authority.clone(),
        plain,
    )
    .unwrap();
}

#[test]
fn no_op_export_returns_original_arc_and_changed_export_keeps_resource_payloads() {
    let source = validated(ONE);
    let original = source.export_package().unwrap();
    assert!(Arc::ptr_eq(&original, &source.authority.package));
    let patch = source
        .set_selection_format(selection(1, 4), &format_bold(Weight::Normal))
        .unwrap()
        .unwrap();
    let output = patch.source.export_package().unwrap();
    let mut before = ZipArchive::new(Cursor::new(original.as_ref())).unwrap();
    let mut after = ZipArchive::new(Cursor::new(output.as_ref())).unwrap();
    assert_eq!(before.len(), after.len());
    assert_eq!(before.comment(), after.comment());
    for ordinal in 0..before.len() {
        let mut a = before.by_index(ordinal).unwrap();
        let mut b = after.by_index(ordinal).unwrap();
        assert_eq!(a.name_raw(), b.name_raw());
        assert_eq!(a.compression(), b.compression());
        assert_eq!(a.last_modified(), b.last_modified());
        assert_eq!(a.unix_mode(), b.unix_mode());
        let mut aa = Vec::new();
        let mut bb = Vec::new();
        a.read_to_end(&mut aa).unwrap();
        b.read_to_end(&mut bb).unwrap();
        if a.name().unwrap().as_ref() == INNER {
            assert_eq!(bb, patch.source.xaml().as_bytes());
        } else {
            assert_eq!(aa, bb);
        }
    }
    assert_eq!(source.authority.original_catalog(), CATALOG);
}

#[test]
fn size_range_selection_and_source_budget_fail_atomically() {
    for value in [
        "0",
        "-1",
        "NaN",
        "inf",
        "1e2",
        ".5",
        "4096.1",
        "9999999999999999999999999999",
        "",
    ] {
        assert!(DipSize::parse(value).is_err(), "{value}");
    }
    assert!(DipSize::parse("0004096.000").is_ok());
    let source = validated(ONE);
    let before = serde_json::to_string(source.wire()).unwrap();
    for range in [
        selection(0, 0),
        selection(0, 10000),
        TextSelection {
            paragraph: 9,
            anchor: 0,
            focus: 1,
        },
    ] {
        assert!(
            source
                .set_selection_format(range, &format_bold(Weight::Normal))
                .is_err()
        );
    }
    let mut wire = source.wire().clone();
    wire.current_xaml = minimal(&"x".repeat(MAX_SOURCE_BYTES)).into();
    assert!(ValidatedRichText::from_wire(wire, source.authority.clone(), "x").is_err());
    assert_eq!(serde_json::to_string(source.wire()).unwrap(), before);
}

#[test]
fn run_limit_and_unsafe_zip_member_or_relationship_are_rejected() {
    let run = "<Run style=\"Normal\">x</Run>";
    let source = minimal("x").replace(run, &run.repeat(MAX_RUNS + 1));
    assert!(
        OriginalTextAuthority::from_package(identity(), package(&source), CATALOG.to_vec())
            .is_err()
    );
    let mut unsafe_id = identity();
    unsafe_id.package_part = "../outside".into();
    assert!(
        OriginalTextAuthority::from_package(unsafe_id, package(ONE), CATALOG.to_vec()).is_err()
    );
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    writer
        .start_file("../escape", SimpleFileOptions::default())
        .unwrap();
    writer.write_all(b"x").unwrap();
    assert!(
        OriginalTextAuthority::from_package(
            identity(),
            writer.finish().unwrap().into_inner(),
            CATALOG.to_vec()
        )
        .is_err()
    );
}

#[test]
fn exact_original_to_current_format_only_receipt_accepts_splits_and_noop() {
    let source = validated(ONE);
    let original = source.prove_format_only().unwrap();
    assert!(Arc::ptr_eq(
        &original.export_package().unwrap(),
        &source.authority.package
    ));
    let patch = source
        .set_selection_format(
            selection(1, 8),
            &TextFormat {
                weight: Some(Weight::Normal),
                style: Some(FontStyle::Italic),
                foreground_argb: Some([255, 16, 32, 48]),
                ..Default::default()
            },
        )
        .unwrap()
        .unwrap();
    let receipt = patch.source.prove_format_only().unwrap();
    assert_eq!(receipt.source().plain_text(), source.plain_text());
    assert_eq!(receipt.source().xaml(), patch.source.xaml());
    assert_ne!(
        receipt.export_package().unwrap().as_ref(),
        source.authority.original_package()
    );
}

#[test]
fn forged_wire_shape_and_same_plain_do_not_prove_format_only_delta() {
    let source = validated(ONE);
    for forged in [
        ONE.replace("FontFamily=\"Malgun Gothic\"", "FontFamily=\"Tahoma\""),
        ONE.replace("Background=\"#FF662D91\"", "Background=\"#FF102030\""),
        ONE.replace("style=\"Normal\"", "style=\"Normal~639058311443748180\""),
        ONE.replace("xml:lang=\"ko-kr\"", "xml:lang=\"en-us\""),
        ONE.replace(
            "NumberSubstitution.CultureSource=\"User\"",
            "NumberSubstitution.CultureSource=\"Text\"",
        ),
        ONE.replace("LCCL-", "LCCL&#45;"),
    ] {
        assert_ne!(forged, ONE, "test replacement must actually mutate source");
        let mut wire = source.wire().clone();
        wire.current_xaml = forged.into();
        if let Ok(candidate) =
            ValidatedRichText::from_wire(wire, source.authority.clone(), source.plain_text())
        {
            assert!(
                candidate.prove_format_only().is_err(),
                "forged same-plain source must not obtain receipt"
            );
        }
    }
}

#[test]
fn original_explicit_property_removal_and_run_merge_cannot_get_receipt() {
    let original = minimal("ab").replace(
        "<Run style=\"Normal\">",
        "<Run style=\"Normal\" FontWeight=\"Bold\">",
    );
    let source = validated(&original);
    let mut wire = source.wire().clone();
    wire.current_xaml = original.replace(" FontWeight=\"Bold\"", "").into();
    let candidate = ValidatedRichText::from_wire(wire, source.authority.clone(), "ab").unwrap();
    assert!(candidate.prove_format_only().is_err());
    let original = minimal("a").replace("a</Run>", "a</Run><Run style=\"Normal\">b</Run>");
    let source = validated(&original);
    let mut wire = source.wire().clone();
    wire.current_xaml = minimal("ab").into();
    let merged = ValidatedRichText::from_wire(wire, source.authority.clone(), "ab").unwrap();
    assert!(merged.prove_format_only().is_err());
}

#[test]
fn content_edit_wire_is_admitted_with_matching_plain_but_not_format_receipt() {
    let source = validated(ONE);
    let mut wire = source.wire().clone();
    wire.current_xaml = ONE.replace("LCCL-", "changed-").into();
    let text = source.plain_text().replace("LCCL-", "changed-");
    let candidate = ValidatedRichText::from_wire(wire, source.authority.clone(), &text).unwrap();
    assert!(candidate.prove_format_only().is_err());
    assert_eq!(source.xaml(), ONE);
}

#[test]
fn run_split_at_run_budget_is_atomic_and_does_not_install_partial_source() {
    let run = "<Run style=\"Normal\">abc</Run>";
    let original = minimal("abc").replace(run, &run.repeat(MAX_RUNS));
    let source = validated(&original);
    let before = source.wire().clone();
    assert!(
        source
            .set_selection_format(selection(1, 2), &format_bold(Weight::Bold))
            .is_err()
    );
    assert_eq!(source.wire(), &before);
    assert_eq!(source.xaml(), original);
}

#[test]
fn lowercase_argb_same_value_is_noop_without_source_lexical_normalization() {
    let original = ONE.replace("#FFFFFFFF", "#ffffffff");
    let source = validated(&original);
    assert!(
        source
            .set_selection_format(
                selection(1, 4),
                &TextFormat {
                    foreground_argb: Some([255, 255, 255, 255]),
                    ..Default::default()
                }
            )
            .unwrap()
            .is_none()
    );
    assert_eq!(source.xaml(), original);
}

#[test]
fn committed_single_run_text_keeps_raw_owner_attributes_and_outside_bytes() {
    let source = validated(TWO);
    let run = source.index.runs[0].clone();
    let patch = source
        .replace_in_run(0, 3, 6, "한😀&<>\"")
        .unwrap()
        .unwrap();
    assert_eq!(patch.source.plain_text(), "설계 한😀&<>\" 계수");
    assert!(patch.source.xaml().starts_with(&TWO[..run.opening_end]));
    assert!(patch.source.xaml().ends_with(&TWO[run.closing_start..]));
    assert!(patch.source.xaml().contains("한😀&amp;&lt;&gt;\""));
    assert_eq!(patch.selection, selection(9, 9));
    assert!(patch.source.prove_format_only().is_err());
    patch.source.prove_preserved_metadata().unwrap();
    assert!(source.replace_in_run(0, 3, 6, "커플링").unwrap().is_none());
    assert_eq!(source.xaml(), TWO);
}

#[test]
fn whole_run_delete_keeps_empty_run_and_rejects_newline_or_out_of_run_atomically() {
    let source = validated(TWO);
    let count = source.plain_text().chars().count();
    let deleted = source.replace_in_run(0, 0, count, "").unwrap().unwrap();
    assert_eq!(deleted.source.plain_text(), "");
    assert_eq!(deleted.source.index.runs.len(), 1);
    assert!(deleted.source.xaml().contains("></Run>"));
    assert_eq!(deleted.selection, selection(0, 0));
    assert!(deleted.source.prove_format_only().is_err());
    let before = source.wire().clone();
    for text in ["\r", "\n", "\t", "\0", "\u{FFFE}"] {
        assert!(source.replace_in_run(0, 0, 1, text).is_err());
    }
    assert!(source.replace_in_run(0, 0, count + 1, "x").is_err());
    assert!(source.replace_in_run(1, 0, 0, "x").is_err());
    assert!(source.replace_in_run(0, 3, 1, "x").is_err());
    assert_eq!(source.wire(), &before);
}

#[test]
fn format_then_committed_text_preserves_keys_and_does_not_get_inert_cache_receipt() {
    let source = validated(TWO);
    let formatted = source
        .set_selection_format(selection(3, 6), &format_bold(Weight::Bold))
        .unwrap()
        .unwrap()
        .source;
    formatted.prove_format_only().unwrap();
    let edited = formatted
        .replace_in_run(1, 0, 3, "글")
        .unwrap()
        .unwrap()
        .source;
    assert_eq!(edited.plain_text(), "설계 글 계수");
    assert_eq!(edited.xaml().matches("style=\"Normal\"").count(), 3);
    assert!(edited.xaml().contains("FontWeight=\"Bold\">글</Run>"));
    assert!(edited.prove_format_only().is_err());
    let restored = ValidatedRichText::from_wire(
        source.wire().clone(),
        source.authority.clone(),
        source.plain_text(),
    )
    .unwrap();
    restored.prove_format_only().unwrap();
}

#[test]
fn paragraph_global_scalar_spans_do_not_confuse_surrogates_with_newline_or_bytes() {
    let original = minimal("가😀").replace(
        "</Paragraph>",
        "</Paragraph><Paragraph><Run style=\"Normal\">abc</Run></Paragraph>",
    );
    let source = validated(&original);
    assert_eq!(source.plain_text(), "가😀\nabc");
    assert_eq!(
        source.run_char_ranges().collect::<Vec<_>>(),
        vec![
            RunCharRange {
                run_ordinal: 0,
                paragraph: 0,
                start_in_paragraph: 0,
                end_in_paragraph: 2,
                global_start: 0,
                global_end: 2
            },
            RunCharRange {
                run_ordinal: 1,
                paragraph: 1,
                start_in_paragraph: 0,
                end_in_paragraph: 3,
                global_start: 3,
                global_end: 6
            },
        ]
    );
    let edited = source.replace_in_run(1, 1, 2, "한😀").unwrap().unwrap();
    assert_eq!(edited.source.plain_text(), "가😀\na한😀c");
    assert_eq!(
        edited.selection,
        TextSelection {
            paragraph: 1,
            anchor: 3,
            focus: 3
        }
    );
    for body in ["a\nb", "a&#10;b", "a&#xD;b"] {
        assert!(
            OriginalTextAuthority::from_package(
                identity(),
                package(&minimal(body)),
                CATALOG.to_vec()
            )
            .is_err()
        );
    }
}

#[test]
fn selection_property_reader_reports_uniform_or_mixed_without_catalog_or_toggle_guess() {
    let original = validated(ONE);
    let initial = original.selection_format(selection(1, 4)).unwrap();
    assert_eq!(initial.weight, Some(Weight::Bold));
    assert_eq!(initial.size_dip.unwrap().as_str(), "16");
    assert_eq!(initial.foreground_argb, Some([255, 255, 255, 255]));
    let source = original
        .set_selection_format(selection(1, 4), &format_bold(Weight::Normal))
        .unwrap()
        .unwrap()
        .source;
    assert_eq!(
        source.selection_format(selection(1, 4)).unwrap().weight,
        Some(Weight::Normal)
    );
    assert_eq!(
        source.selection_format(selection(0, 5)).unwrap().weight,
        None
    );
    assert_eq!(
        source.selection_format(selection(0, 5)).unwrap().style,
        Some(FontStyle::Normal)
    );
    assert!(source.selection_format(selection(0, 0)).is_err());
}

#[test]
fn wire_clone_shares_source_allocation_while_json_remains_an_exact_string() {
    let source = validated(ONE);
    let copied = source.wire().clone();
    assert!(Arc::ptr_eq(&copied.current_xaml, &source.wire.current_xaml));
    assert!(Arc::ptr_eq(
        &copied.current_xaml,
        &source.authority.original_xaml
    ));
    let value = serde_json::to_value(&copied).unwrap();
    assert_eq!(
        value.get("current_xaml"),
        Some(&serde_json::Value::String(ONE.into()))
    );
    let restored: RichTextWire = serde_json::from_value(value).unwrap();
    assert_eq!(restored, copied);
    let patch = source.replace_in_run(0, 0, 1, "한").unwrap().unwrap();
    assert!(!Arc::ptr_eq(
        &patch.source.wire.current_xaml,
        &source.wire.current_xaml
    ));
    assert_eq!(source.wire.current_xaml.as_ref(), ONE);
    assert_eq!(
        std::mem::size_of::<Option<Box<RichTextWire>>>(),
        std::mem::size_of::<usize>()
    );
}
