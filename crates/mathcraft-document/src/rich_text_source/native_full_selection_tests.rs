use super::*;
use zip::write::SimpleFileOptions;
const CATALOG: &[u8] =
    include_bytes!("../../../../tests/reference/rich-text/lcc-standard-presentation.xml");
fn original(body: &str) -> ValidatedRichText {
    let xml = format!(
        "<Section xmlns='{XAML}' xml:space='preserve' xml:lang='ko-KR' FlowDirection='LeftToRight' FontFamily='Tahoma' FontWeight='Normal' FontStyle='Normal' FontSize='16' Foreground='#FF123456'><Paragraph>{body}</Paragraph></Section>"
    );
    let mut z = ZipWriter::new(Cursor::new(Vec::new()));
    for (path, bytes) in [
        (INNER, xml.as_bytes()),
        ("Resources/opaque.bin", &[0, 255, 9][..]),
    ] {
        z.start_file(path, SimpleFileOptions::default()).unwrap();
        z.write_all(bytes).unwrap();
    }
    let authority = OriginalTextAuthority::from_package(
        NativeTextIdentity {
            owner_region_id: "17".into(),
            item_idref: "Text0".into(),
            package_part: "mathcad/xaml/Text0.XamlPackage".into(),
            catalog_part: "mathcad/settings/presentation.xml".into(),
            owner_xml_sha256: hash(b"native select-all original wrapper"),
        },
        z.finish().unwrap().into_inner(),
        CATALOG.to_vec(),
    )
    .unwrap();
    let plain = authority.original_plain_text().to_owned();
    ValidatedRichText::from_original(authority, &plain).unwrap()
}
fn profile() -> NativeFullSelectionTypingContext<'static> {
    NativeFullSelectionTypingContext {
        input_language: "KO-kr",
        input_flow_direction: TypingFlowDirection::LeftToRight,
        has_retained_springload: false,
    }
}
fn all(s: &ValidatedRichText) -> TextSelection {
    TextSelection {
        paragraph: 0,
        anchor: 0,
        focus: s.plain_text().chars().count(),
    }
}

#[test]
fn first_nonempty_native_owner_keeps_unknown_decoration_and_every_wrapper_exact() {
    let raw = "<Run style='Normal' FontStyle='Italic'></Run><Run style='Normal' Background='#FF662D91'>a&#x1F600;&amp;b</Run><Run style='Normal' FontWeight='Bold'>tail</Run>";
    let old = original(raw);
    let before = serde_json::to_vec(old.wire()).unwrap();
    assert!(
        old.selection_format(TextSelection {
            paragraph: 0,
            anchor: 0,
            focus: 1
        })
        .unwrap()
        .text_decorations
        .is_none()
    );
    let incomplete = old
        .selection_format(TextSelection {
            paragraph: 0,
            anchor: 1,
            focus: 2,
        })
        .unwrap();
    let old_error = old
        .replace_cross_run_selection(
            TextSelection {
                paragraph: 0,
                anchor: 1,
                focus: 6,
            },
            "X",
            CrossRunTypingContext {
                insertion_owner: 1,
                expected_format: &incomplete,
                input_language: "ko-KR",
                input_flow_direction: TypingFlowDirection::LeftToRight,
                has_retained_springload: false,
            },
        )
        .unwrap_err();
    assert!(old_error.contains("Missing/different effective insertion format"));
    let patch = old
        .replace_select_all_text(all(&old), "한<&😀", profile())
        .unwrap()
        .unwrap();
    let expected = old
        .xaml()
        .replace("a&#x1F600;&amp;b", "한&lt;&amp;😀")
        .replace(">tail</Run>", "></Run>");
    assert_eq!(patch.source.xaml(), expected);
    assert_eq!(patch.source.plain_text(), "한<&😀");
    assert_eq!(patch.insertion_owner, Some(1));
    assert_eq!(
        patch.selection,
        TextSelection {
            paragraph: 0,
            anchor: 4,
            focus: 4
        }
    );
    assert!(!patch.source.xaml().contains("TextDecorations"));
    assert!(patch.source.has_original_raw_body_only_delta());
    assert!(patch.source.prove_format_only().is_err());
    let wire: RichTextWire =
        serde_json::from_slice(&serde_json::to_vec(patch.source.wire()).unwrap()).unwrap();
    let cold = ValidatedRichText::from_wire(wire, old.authority.clone(), "한<&😀").unwrap();
    assert_eq!(cold.xaml(), expected);
    assert!(cold.has_original_raw_body_only_delta());
    let package = cold.export_package().unwrap();
    let mut zip = ZipArchive::new(Cursor::new(package.as_ref())).unwrap();
    let mut opaque = vec![];
    zip.by_name("Resources/opaque.bin")
        .unwrap()
        .read_to_end(&mut opaque)
        .unwrap();
    assert_eq!(opaque, [0, 255, 9]);
    assert_eq!(serde_json::to_vec(old.wire()).unwrap(), before);
}

#[test]
fn same_plain_replacement_redistributes_body_and_exact_xml_noop_is_separate() {
    let old = original(
        "<Run style='Normal' Background='#FF111111'>ab</Run><Run style='Normal' Background='#FF222222'>cd</Run>",
    );
    let patch = old
        .replace_select_all_text(all(&old), "abcd", profile())
        .unwrap()
        .unwrap();
    assert_eq!(patch.source.plain_text(), old.plain_text());
    assert_eq!(
        patch.source.xaml(),
        old.xaml()
            .replace(">ab</Run>", ">abcd</Run>")
            .replace(">cd</Run>", "></Run>")
    );
    assert!(patch.source.has_original_raw_body_only_delta());
    assert!(patch.source.prove_format_only().is_err());
    assert!(
        patch
            .source
            .replace_select_all_text(all(&patch.source), "abcd", profile())
            .unwrap()
            .is_none()
    );
}

#[test]
fn context_range_control_empty_and_budget_failures_preserve_native_source() {
    let old = original("<Run style='Normal'>ab</Run><Run style='Normal'>cd</Run>");
    let wire = serde_json::to_vec(old.wire()).unwrap();
    for context in [
        NativeFullSelectionTypingContext {
            input_language: "en-US",
            ..profile()
        },
        NativeFullSelectionTypingContext {
            input_flow_direction: TypingFlowDirection::RightToLeft,
            ..profile()
        },
        NativeFullSelectionTypingContext {
            has_retained_springload: true,
            ..profile()
        },
    ] {
        assert!(
            old.replace_select_all_text(all(&old), "X", context)
                .is_err()
        );
    }
    for text in ["", "a\nb", "a\tb", "\0"] {
        assert!(
            old.replace_select_all_text(all(&old), text, profile())
                .is_err()
        );
    }
    assert!(
        old.replace_select_all_text(
            TextSelection {
                paragraph: 0,
                anchor: 1,
                focus: 3
            },
            "X",
            profile()
        )
        .is_err()
    );
    assert!(
        old.replace_select_all_text(all(&old), &"&".repeat(MAX_SOURCE_BYTES / 4), profile())
            .is_err()
    );
    let empty = original("<Run style='Normal'></Run>");
    assert!(
        empty
            .replace_select_all_text(all(&empty), "X", profile())
            .is_err()
    );
    assert_eq!(serde_json::to_vec(old.wire()).unwrap(), wire);
}

#[test]
fn positive_witness_rejects_format_property_removal_and_changed_block_or_run_identity() {
    let old =
        original("<Run style='Normal' FontWeight='Bold'>ab</Run><Run style='Normal'>cd</Run>");
    assert!(!old.has_original_raw_body_only_delta());
    let mut wire = old.wire().clone();
    wire.current_xaml = old.xaml().replace(" FontWeight='Bold'", "").into();
    wire.visual_fingerprint = Default::default();
    let removed =
        ValidatedRichText::from_wire(wire, old.authority.clone(), old.plain_text()).unwrap();
    assert!(!removed.has_original_raw_body_only_delta());
    assert!(removed.prove_format_only().is_err());
    let formatted = old
        .set_selection_format(
            TextSelection {
                paragraph: 0,
                anchor: 0,
                focus: 2,
            },
            &TextFormat {
                style: Some(FontStyle::Italic),
                ..Default::default()
            },
        )
        .unwrap()
        .unwrap()
        .source;
    assert!(!formatted.has_original_raw_body_only_delta());
    assert!(formatted.prove_format_only().is_ok());
    let replaced = formatted
        .replace_select_all_text(all(&formatted), old.plain_text(), profile())
        .unwrap()
        .unwrap()
        .source;
    assert!(!replaced.has_original_raw_body_only_delta());
    assert!(replaced.prove_format_only().is_err());
}
