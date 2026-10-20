use super::*;
use zip::write::SimpleFileOptions;

fn selected(start: usize, end: usize) -> TextSelection {
    TextSelection {
        paragraph: 0,
        anchor: start,
        focus: end,
    }
}
fn format(value: TextDecorations) -> TextFormat {
    TextFormat {
        text_decorations: Some(value),
        ..Default::default()
    }
}
fn package(xaml: &str) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    writer
        .start_file(INNER, SimpleFileOptions::default())
        .unwrap();
    writer.write_all(xaml.as_bytes()).unwrap();
    writer
        .start_file("Resources/opaque.bin", SimpleFileOptions::default())
        .unwrap();
    writer.write_all(&[0, 255, 3, 9]).unwrap();
    writer.finish().unwrap().into_inner()
}
fn original(xaml: &str) -> Result<ValidatedRichText> {
    let catalog = format!(
        "<presentation xmlns=\"{PRESENTATION}\"><textRendering><textStyles><textStyle style-key=\"Normal\" underline=\"true\"/></textStyles></textRendering></presentation>"
    );
    let authority = OriginalTextAuthority::from_package(
        NativeTextIdentity {
            owner_region_id: "7".into(),
            item_idref: "FlowDocument7".into(),
            package_part: "mathcad/xaml/FlowDocument7.XamlPackage".into(),
            catalog_part: "mathcad/settings/presentation.xml".into(),
            owner_xml_sha256: hash(b"exact synthetic original wrapper"),
        },
        package(xaml),
        catalog.into_bytes(),
    )?;
    let plain = authority.original_plain_text().to_owned();
    ValidatedRichText::from_original(authority, &plain)
}
fn source(run: &str) -> String {
    format!(
        "<Section xmlns=\"{XAML}\" FontWeight=\"Normal\" FontStyle=\"Normal\" FontSize=\"16\" Foreground=\"#FFFFFFFF\"><Paragraph>{run}</Paragraph></Section>"
    )
}

#[test]
fn imported_explicit_range_entity_split_clear_and_cold_rebind_preserve_authority() {
    let value = original(&source(
        "<Run style='Normal' TextDecorations='None'>A&amp;😀Z</Run>",
    ))
    .unwrap();
    let before = value.wire().clone();
    let patch = value
        .set_selection_format(selected(1, 3), &format(TextDecorations::Underline))
        .unwrap()
        .unwrap();
    assert_eq!(patch.source.plain_text(), "A&😀Z");
    assert_eq!(patch.source.run_char_ranges().count(), 3);
    assert_eq!(
        patch
            .source
            .selection_format(selected(1, 3))
            .unwrap()
            .text_decorations,
        Some(TextDecorations::Underline)
    );
    assert_eq!(
        patch
            .source
            .selection_format(selected(0, 4))
            .unwrap()
            .text_decorations,
        None
    );
    assert!(
        patch
            .source
            .selection_text_decorations_supported(selected(0, 4))
            .unwrap(),
        "mixed explicit None/Underline is supported"
    );
    assert!(patch.source.xaml().contains("&amp;😀"));
    let receipt = patch.source.prove_format_only().unwrap();
    assert_eq!(receipt.source().wire().origin(), before.origin());
    assert_eq!(value.wire(), &before);
    let xml = patch.source.xaml().to_owned();
    let reopened = ValidatedRichText::from_wire(
        patch.source.wire().clone(),
        value.authority.clone(),
        "A&😀Z",
    )
    .unwrap();
    assert_eq!(reopened.xaml(), xml);
    assert!(
        reopened
            .set_selection_format(selected(1, 3), &format(TextDecorations::Underline))
            .unwrap()
            .is_none()
    );
    let cleared = reopened
        .set_selection_format(selected(1, 3), &format(TextDecorations::None))
        .unwrap()
        .unwrap()
        .source;
    assert_eq!(
        cleared
            .selection_format(selected(0, 4))
            .unwrap()
            .text_decorations,
        Some(TextDecorations::None)
    );
    let bytes = cleared
        .prove_format_only()
        .unwrap()
        .export_package()
        .unwrap();
    let mut archive = ZipArchive::new(Cursor::new(bytes.as_ref())).unwrap();
    let mut resource = Vec::new();
    archive
        .by_name("Resources/opaque.bin")
        .unwrap()
        .read_to_end(&mut resource)
        .unwrap();
    assert_eq!(resource, [0, 255, 3, 9]);
}

#[test]
fn imported_missing_decoration_is_unknown_even_when_catalog_mentions_underline() {
    let value = original(&source("<Run style=\"Normal\">abc</Run>")).unwrap();
    assert_eq!(
        value
            .selection_format(selected(0, 3))
            .unwrap()
            .text_decorations,
        None
    );
    assert!(
        !value
            .selection_text_decorations_supported(selected(0, 3))
            .unwrap()
    );
    assert!(
        value
            .selection_text_decorations_supported(selected(1, 1))
            .is_err()
    );
    assert!(
        value
            .selection_text_decorations_supported(selected(0, 4))
            .is_err()
    );
    let before = value.wire().clone();
    // A toolbar assignment does not query/infer the old value. Both true
    // and false must materialize a property over unknown; neither is a no-op.
    for desired in [TextDecorations::Underline, TextDecorations::None] {
        let patch = value
            .set_selection_format(selected(0, 3), &format(desired))
            .unwrap()
            .expect("Explicit assignment over absent property");
        assert_eq!(value.wire(), &before);
        assert_eq!(patch.source.plain_text(), "abc");
        assert_eq!(
            patch
                .source
                .selection_format(selected(0, 3))
                .unwrap()
                .text_decorations,
            Some(desired)
        );
        assert!(
            patch
                .source
                .selection_text_decorations_supported(selected(0, 3))
                .unwrap()
        );
        assert_eq!(patch.source.wire().origin(), before.origin());
        let receipt = patch.source.prove_format_only().unwrap();
        let bytes = receipt.export_package().unwrap();
        let mut zip = ZipArchive::new(Cursor::new(bytes.as_ref())).unwrap();
        let mut resource = Vec::new();
        zip.by_name("Resources/opaque.bin")
            .unwrap()
            .read_to_end(&mut resource)
            .unwrap();
        assert_eq!(resource, [0, 255, 3, 9]);
        let cold = ValidatedRichText::from_wire(
            patch.source.wire().clone(),
            value.authority.clone(),
            "abc",
        )
        .unwrap();
        assert_eq!(cold.xaml(), patch.source.xaml());
        assert!(
            cold.set_selection_format(selected(0, 3), &format(desired))
                .unwrap()
                .is_none()
        );
    }
    assert_eq!(value.wire(), &before);
    assert_eq!(
        value
            .selection_format(selected(0, 3))
            .unwrap()
            .text_decorations,
        None,
        "Immutable original remains unknown, not inferred None"
    );
    let partly_known = original(&source(
        "<Run style=\"Normal\" TextDecorations=\"Underline\">a</Run><Run style=\"Normal\">b</Run>",
    ))
    .unwrap();
    assert!(
        partly_known
            .selection_text_decorations_supported(selected(0, 1))
            .unwrap()
    );
    assert!(
        !partly_known
            .selection_text_decorations_supported(selected(0, 2))
            .unwrap()
    );
}

#[test]
fn unsupported_collections_ancestor_properties_and_forged_metadata_are_not_flattened() {
    for value in [
        "OverLine",
        "Strikethrough",
        "Underline,Strikethrough",
        "Underline,Underline",
        "underline",
        "",
        "{Binding X}",
    ] {
        assert!(
            original(&source(&format!(
                "<Run style=\"Normal\" TextDecorations=\"{value}\">a</Run>"
            )))
            .is_err(),
            "{value}"
        );
    }
    let root = source("<Run style=\"Normal\">a</Run>")
        .replace("<Section ", "<Section TextDecorations=\"Underline\" ");
    assert!(original(&root).is_err());
    let value = original(&source(
        "<Run style=\"Normal\" TextDecorations=\"None\">abc</Run>",
    ))
    .unwrap();
    let patch = value
        .set_selection_format(selected(0, 3), &format(TextDecorations::Underline))
        .unwrap()
        .unwrap();
    let mut forged = patch.source.wire().clone();
    forged.current_xaml = forged
        .current_xaml
        .replace("style=\"Normal\"", "style=\"Foreign\"")
        .into();
    assert!(ValidatedRichText::from_wire(forged, value.authority.clone(), "abc").is_err());
    let mut removed = patch.source.wire().clone();
    removed.current_xaml = removed
        .current_xaml
        .replace(" TextDecorations=\"Underline\"", "")
        .into();
    let removed = ValidatedRichText::from_wire(removed, value.authority.clone(), "abc").unwrap();
    assert!(
        removed.prove_format_only().is_err(),
        "original explicit property removal must not obtain deferred proof"
    );
}

#[test]
fn explicit_decorations_over_absence_keep_other_unknown_properties_and_partitions_closed() {
    let value = original(&source("<Run style='Normal'>A&amp;😀Z</Run>")).unwrap();
    let patch = value
        .set_selection_format(selected(1, 3), &format(TextDecorations::Underline))
        .unwrap()
        .unwrap();
    assert_eq!(patch.source.run_char_ranges().count(), 3);
    assert!(patch.source.xaml().contains("&amp;😀"));
    assert_eq!(
        patch
            .source
            .selection_format(selected(0, 1))
            .unwrap()
            .text_decorations,
        None
    );
    assert_eq!(
        patch
            .source
            .selection_format(selected(3, 4))
            .unwrap()
            .text_decorations,
        None
    );
    assert!(
        !patch
            .source
            .selection_text_decorations_supported(selected(0, 4))
            .unwrap()
    );
    patch.source.prove_format_only().unwrap();
    let missing_weight =
        source("<Run style='Normal'>abc</Run>").replace(" FontWeight=\"Normal\"", "");
    let unknown = original(&missing_weight).unwrap();
    let mixed = TextFormat {
        weight: Some(Weight::Bold),
        text_decorations: Some(TextDecorations::Underline),
        ..Default::default()
    };
    assert!(
        unknown
            .set_selection_format(selected(0, 3), &mixed)
            .is_err()
    );
    assert_eq!(
        unknown
            .selection_format(selected(0, 3))
            .unwrap()
            .text_decorations,
        None
    );
}
