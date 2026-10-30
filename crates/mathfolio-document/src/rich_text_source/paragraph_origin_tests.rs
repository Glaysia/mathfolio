//! Origin map source/serialization/budget regressions.
use super::*;
use zip::write::SimpleFileOptions;

const CATALOG: &[u8] =
    include_bytes!("../../../../tests/reference/rich-text/lcc-standard-presentation.xml");
const ONE: &str = r##"<Section xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation" xml:space="preserve" FontSize="16" FontStyle="Normal"><Paragraph NumberSubstitution.CultureSource="User"><Run style="Normal" xml:lang="ko-kr" Background="#FF31ADC2">A&amp;한😀BC</Run></Paragraph></Section>"##;
fn identity() -> NativeTextIdentity {
    NativeTextIdentity {
        owner_region_id: "17".into(),
        item_idref: "FlowDocument0".into(),
        package_part: "mathcad/xaml/FlowDocument0.XamlPackage".into(),
        catalog_part: "mathcad/settings/presentation.xml".into(),
        owner_xml_sha256: hash(b"candidate original Text owner"),
    }
}
fn package(xml: &str) -> Vec<u8> {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    zip.start_file(INNER, SimpleFileOptions::default()).unwrap();
    zip.write_all(xml.as_bytes()).unwrap();
    zip.start_file("Resources/inert.bin", SimpleFileOptions::default())
        .unwrap();
    zip.write_all(&[0, 255, 7]).unwrap();
    zip.finish().unwrap().into_inner()
}
fn validated(xml: &str) -> ValidatedRichText {
    let authority =
        OriginalTextAuthority::from_package(identity(), package(xml), CATALOG.to_vec()).unwrap();
    let plain = authority.original_plain_text().to_owned();
    ValidatedRichText::from_original(authority, &plain).unwrap()
}
fn caret(paragraph: usize, at: usize) -> TextSelection {
    TextSelection {
        paragraph,
        anchor: at,
        focus: at,
    }
}
fn split() -> ValidatedRichText {
    validated(ONE)
        .split_paragraph_at_caret(caret(0, 3))
        .unwrap()
        .patch
        .source
}
fn two_original() -> String {
    ONE.replace("한😀", "한</Run></Paragraph><Paragraph NumberSubstitution.CultureSource=\"User\"><Run style=\"Normal\" xml:lang=\"ko-kr\" Background=\"#FF31ADC2\">😀")
}

#[test]
fn absent_map_old_json_and_fingerprint_are_exact() {
    let source = validated(ONE);
    let wire = source.wire();
    let expected = format!(
        "{{\"version\":1,\"current_xaml\":{},\"origin\":{}}}",
        serde_json::to_string(source.xaml()).unwrap(),
        serde_json::to_string(wire.origin()).unwrap()
    );
    assert_eq!(serde_json::to_string(wire).unwrap(), expected);
    assert_eq!(
        wire.visual_source_fingerprint(),
        hash(
            &serde_json::to_vec(&(wire.version, &wire.origin, wire.current_xaml.as_ref())).unwrap()
        )
    );
    assert_eq!(wire.source_bytes(), source.xaml().len());
}

#[test]
fn repeated_entity_safe_splits_keep_global_run_order_and_origin_groups() {
    let first = split();
    assert_eq!(first.plain_text(), "A&한\n😀BC");
    let next = first.split_paragraph_at_caret(caret(1, 1)).unwrap();
    assert_eq!(next.patch.source.plain_text(), "A&한\n😀\nBC");
    assert_eq!(
        next.patch.source.wire.paragraph_origins.as_deref(),
        Some(&[0, 0, 0][..])
    );
    assert_eq!(next.following_run, 2);
    assert_eq!(next.patch.selection, caret(2, 0));
    let runs: Vec<_> = next.patch.source.run_char_ranges().collect();
    assert_eq!((runs[0].global_start, runs[0].global_end), (0, 3));
    assert_eq!((runs[1].global_start, runs[1].global_end), (4, 5));
    assert_eq!((runs[2].global_start, runs[2].global_end), (6, 8));
    assert!(next.patch.source.prove_format_only().is_err());
    assert_eq!(validated(ONE).xaml(), ONE);
}

#[test]
fn original_multiple_paragraphs_are_covered_without_reordering() {
    let original = validated(&two_original());
    let first = original
        .split_paragraph_at_caret(caret(0, 1))
        .unwrap()
        .patch
        .source;
    assert_eq!(
        first.wire.paragraph_origins.as_deref(),
        Some(&[0, 0, 1][..])
    );
    let second = first
        .split_paragraph_at_caret(caret(2, 1))
        .unwrap()
        .patch
        .source;
    assert_eq!(
        second.wire.paragraph_origins.as_deref(),
        Some(&[0, 0, 1, 1][..])
    );
    assert_eq!(second.plain_text(), "A\n&한\n😀\nBC");
}

#[test]
fn native_reimport_resets_authority_and_can_split_again() {
    let source = split();
    let raw = source.export_package().unwrap();
    let authority =
        OriginalTextAuthority::from_package(identity(), raw.to_vec(), CATALOG.to_vec()).unwrap();
    let reopened = ValidatedRichText::from_original(authority, source.plain_text()).unwrap();
    assert!(reopened.wire.paragraph_origins.is_none());
    assert_eq!(reopened.xaml(), source.xaml());
    let later = reopened
        .split_paragraph_at_caret(caret(1, 1))
        .unwrap()
        .patch
        .source;
    assert_eq!(
        later.wire.paragraph_origins.as_deref(),
        Some(&[0, 1, 1][..])
    );
    assert_eq!(later.plain_text(), "A&한\n😀\nBC");
}

#[test]
fn own_json_revalidation_and_extra_map_budget_are_exact() {
    let source = split();
    let json = serde_json::to_string(source.wire()).unwrap();
    let wire: RichTextWire = serde_json::from_str(&json).unwrap();
    let reopened =
        ValidatedRichText::from_wire(wire, source.authority.clone(), source.plain_text()).unwrap();
    assert_eq!(reopened.wire(), source.wire());
    let mut absent = source.wire().clone();
    absent.paragraph_origins = None;
    absent.visual_fingerprint = std::sync::OnceLock::new();
    assert_eq!(
        source.wire().source_bytes() - absent.source_bytes(),
        json.len() - serde_json::to_string(&absent).unwrap().len()
    );
    assert_ne!(
        source.wire().visual_source_fingerprint(),
        absent.visual_source_fingerprint()
    );
    assert!(
        ValidatedRichText::from_wire(absent, source.authority.clone(), source.plain_text())
            .is_err()
    );
}

#[test]
fn malicious_maps_are_bounded_strict_and_never_authority() {
    let source = split();
    let value = serde_json::to_value(source.wire()).unwrap();
    for bad in [
        serde_json::json!(null),
        serde_json::json!([]),
        serde_json::json!([-1, 0]),
        serde_json::json!([0, 70000]),
        serde_json::json!([0.0, 0]),
    ] {
        let mut v = value.clone();
        v["paragraph_origins"] = bad;
        assert!(serde_json::from_value::<RichTextWire>(v).is_err());
    }
    let mut oversized = value.clone();
    oversized["paragraph_origins"] = serde_json::json!(vec![0; 1025]);
    assert!(serde_json::from_value::<RichTextWire>(oversized).is_err());
    for bad in [
        vec![0],
        vec![1, 1],
        vec![0, 1],
        vec![0, 0, 0],
        vec![0, 65535],
    ] {
        let mut wire = source.wire().clone();
        wire.paragraph_origins = Some(bad.into());
        assert!(
            ValidatedRichText::from_wire(wire, source.authority.clone(), source.plain_text())
                .is_err()
        );
    }
    let json = serde_json::to_string(source.wire()).unwrap();
    let duplicate = format!("{},\"paragraph_origins\":[0,0]}}", &json[..json.len() - 1]);
    assert!(serde_json::from_str::<RichTextWire>(&duplicate).is_err());
    assert!(checked_map(&[0, 2, 2, 2], 3, 4).is_err());
    assert!(checked_map(&[0, 1, 0, 2], 3, 4).is_err());
}

#[test]
fn clone_wrappers_root_metadata_and_run_owner_may_not_be_forged() {
    let source = split();
    for forged in [
        source.xaml().replacen(
            "NumberSubstitution.CultureSource=\"User\"",
            "NumberSubstitution.CultureSource=\"Text\"",
            1,
        ),
        source
            .xaml()
            .replacen("Background=\"#FF31ADC2\"", "Background=\"#FFFFFFFF\"", 1),
        source.xaml().replace("FontSize=\"16\"", "FontSize=\"18\""),
    ] {
        let mut wire = source.wire().clone();
        wire.current_xaml = forged.into();
        wire.visual_fingerprint = std::sync::OnceLock::new();
        assert!(
            ValidatedRichText::from_wire(wire, source.authority.clone(), source.plain_text())
                .is_err()
        );
    }
}

#[test]
fn typing_and_selected_style_keep_map_but_never_format_only_receipt() {
    let source = split();
    let map = source.wire.paragraph_origins.as_ref().unwrap().clone();
    let edited = source
        .replace_in_run(1, 0, 0, "글")
        .unwrap()
        .unwrap()
        .source;
    assert!(Arc::ptr_eq(
        &map,
        edited.wire.paragraph_origins.as_ref().unwrap()
    ));
    let styled = edited
        .set_selection_format(
            TextSelection {
                paragraph: 1,
                anchor: 0,
                focus: 1,
            },
            &TextFormat {
                style: Some(FontStyle::Italic),
                ..Default::default()
            },
        )
        .unwrap()
        .unwrap()
        .source;
    assert_eq!(styled.plain_text(), "A&한\n글😀BC");
    assert!(styled.prove_format_only().is_err());
}

#[test]
fn unsupported_selection_edges_empty_sides_and_run_budget_are_atomic() {
    let original = validated(ONE);
    let before = original.wire().clone();
    for c in [
        caret(0, 0),
        caret(0, 6),
        TextSelection {
            paragraph: 0,
            anchor: 1,
            focus: 3,
        },
    ] {
        assert!(original.split_paragraph_at_caret(c).is_err());
        assert_eq!(original.wire(), &before);
    }
    let source = split();
    assert!(source.replace_in_run(1, 0, 3, "").is_err());
    let xml = format!(
        "<Section xmlns=\"{XAML}\"><Paragraph>{}</Paragraph></Section>",
        "<Run style=\"Normal\">ab</Run>".repeat(MAX_RUNS)
    );
    let source = validated(&xml);
    let before = source.wire().clone();
    assert!(source.split_paragraph_at_caret(caret(0, 1)).is_err());
    assert_eq!(source.wire(), &before);
}

#[test]
fn identity_or_missing_coverage_map_cannot_use_equal_xml_shortcut() {
    let original = validated(ONE);
    let mut wire = original.wire().clone();
    wire.paragraph_origins = Some(vec![0].into());
    assert!(
        ValidatedRichText::from_wire(wire, original.authority.clone(), original.plain_text())
            .is_err()
    );
    let source = split();
    let (xml, plain) = (source.xaml().to_owned(), source.plain_text().to_owned());
    assert!(!xml.contains("LineBreak"));
    assert!(plain.contains('\n'));
}

#[test]
fn near_xml_cap_provenance_overhead_rejects_command_and_untrusted_wire() {
    let opening = format!("<Section xmlns=\"{XAML}\"><Paragraph><Run style=\"Normal\">");
    let closing = "</Run></Paragraph></Section>";
    let insertion = "</Run></Paragraph><Paragraph><Run style=\"Normal\">";
    let count = MAX_SOURCE_BYTES - opening.len() - closing.len() - insertion.len();
    let body = "a".repeat(count);
    let xml = format!("{opening}{body}{closing}");
    let original = validated(&xml);
    let before = original.wire().clone();
    assert!(original.split_paragraph_at_caret(caret(0, 1)).is_err());
    assert_eq!(original.wire(), &before);
    let mut current = xml.clone();
    current.insert_str(opening.len() + 1, insertion);
    assert_eq!(current.len(), MAX_SOURCE_BYTES);
    let mut wire = original.wire().clone();
    wire.current_xaml = current.into();
    wire.paragraph_origins = Some(vec![0, 0].into());
    wire.visual_fingerprint = std::sync::OnceLock::new();
    assert!(wire.source_bytes() > MAX_SOURCE_BYTES);
    let plain = format!("a\n{}", "a".repeat(count - 1));
    assert!(ValidatedRichText::from_wire(wire, original.authority.clone(), &plain).is_err());
}
