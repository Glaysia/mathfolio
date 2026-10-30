//! Proposed tests: not executed by the worker. No timing assertions.
use super::*;
use zip::write::SimpleFileOptions;

const CATALOG: &[u8] =
    include_bytes!("../../../../tests/reference/rich-text/lcc-standard-presentation.xml");
const ACTUAL: &str = include_str!("../../../../tests/reference/rich-text/sample-01.xaml");

fn identity() -> NativeTextIdentity {
    NativeTextIdentity {
        owner_region_id: "17".into(),
        item_idref: "FlowDocument0".into(),
        package_part: "mathcad/xaml/FlowDocument0.XamlPackage".into(),
        catalog_part: "mathcad/settings/presentation.xml".into(),
        owner_xml_sha256: hash(b"cross-run synthetic original wrapper"),
    }
}
fn package(xml: &str) -> Vec<u8> {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    zip.start_file(
        INNER,
        SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated),
    )
    .unwrap();
    zip.write_all(xml.as_bytes()).unwrap();
    zip.start_file("Resources/opaque.bin", SimpleFileOptions::default())
        .unwrap();
    zip.write_all(&[0, 255, 7, 128, 3]).unwrap();
    zip.finish().unwrap().into_inner()
}
fn native(xml: &str) -> ValidatedRichText {
    let a =
        OriginalTextAuthority::from_package(identity(), package(xml), CATALOG.to_vec()).unwrap();
    let plain = a.original_plain_text().to_owned();
    ValidatedRichText::from_original(a, &plain).unwrap()
}
fn xml() -> String {
    format!(
        "<Section xmlns=\"{XAML}\" xml:space=\"preserve\" xml:lang=\"en-US\" FlowDirection=\"LeftToRight\" FontFamily=\"Malgun Gothic\" FontWeight=\"Normal\" FontStyle=\"Normal\" FontSize=\"16\" Foreground=\"#FFFFFFFF\"><Paragraph><Run style=\"Normal\" xml:lang=\"ko-kr\" Background=\"#FF123456\" TextDecorations=\"None\">a&#x1F600;&amp;b</Run><Run style=\"Normal\" FontStyle=\"Italic\" Background=\"#FF654321\" TextDecorations=\"Underline\">가中</Run><Run style=\"Normal\" TextDecorations=\"None\"></Run><Run style=\"Normal\" FontWeight=\"Bold\" TextDecorations=\"None\">c한d&#101;f</Run></Paragraph></Section>"
    )
}
fn selection() -> TextSelection {
    TextSelection {
        paragraph: 0,
        anchor: 1,
        focus: 8,
    }
}
fn format(source: &ValidatedRichText) -> TextFormat {
    source
        .selection_format(TextSelection {
            paragraph: 0,
            anchor: 1,
            focus: 2,
        })
        .unwrap()
}
fn context(format: &TextFormat) -> CrossRunTypingContext<'_> {
    CrossRunTypingContext {
        insertion_owner: 0,
        expected_format: format,
        input_language: "KO-KR",
        input_flow_direction: TypingFlowDirection::LeftToRight,
        has_retained_springload: false,
    }
}
fn body_parts(xml: &str) -> (Vec<String>, Vec<String>) {
    let doc = Document::parse(xml).unwrap();
    let mut openings = Vec::new();
    let mut bodies = Vec::new();
    for node in doc.descendants().filter(|n| n.has_tag_name((XAML, "Run"))) {
        let raw = &xml[node.range()];
        openings.push(raw[..raw.find('>').unwrap() + 1].to_owned());
        bodies.push(node.children().filter_map(|n| n.text()).collect());
    }
    (openings, bodies)
}

#[test]
fn unicode_entity_replacement_keeps_every_original_wrapper_and_remaining_lexeme() {
    let original = native(&xml());
    assert_eq!(original.plain_text(), "a😀&b가中c한def");
    let before_wire = serde_json::to_string(original.wire()).unwrap();
    let patch = original
        .replace_cross_run_selection(selection(), "새<&😀", context(&format(&original)))
        .unwrap();
    assert_eq!(patch.source.plain_text(), "a새<&😀def");
    assert_eq!(
        patch.selection,
        TextSelection {
            paragraph: 0,
            anchor: 5,
            focus: 5
        }
    );
    assert_eq!(patch.insertion_owner, Some(0));
    assert_eq!(
        body_parts(original.xaml()).0,
        body_parts(patch.source.xaml()).0
    );
    assert_eq!(
        body_parts(patch.source.xaml()).1,
        ["a새<&😀", "", "", "def"]
    );
    assert!(patch.source.xaml().contains("d&#101;f</Run>"));
    assert!(patch.source.xaml().contains("새&lt;&amp;😀"));
    assert_eq!(serde_json::to_string(original.wire()).unwrap(), before_wire);
    assert!(patch.source.prove_format_only().is_err());
}

#[test]
fn inter_run_text_and_cdata_remain_outside_existing_source_admission() {
    for gap in ["\n \t", "<![CDATA[]]>"] {
        let raw = xml().replace("</Run><Run", &format!("</Run>{gap}<Run"));
        assert!(
            OriginalTextAuthority::from_package(identity(), package(&raw), CATALOG.to_vec())
                .is_err()
        );
    }
}

#[test]
fn reverse_selection_uses_normalized_start_owner_not_focus_direction() {
    let source = native(&xml());
    let mut reverse = selection();
    std::mem::swap(&mut reverse.anchor, &mut reverse.focus);
    let a = source
        .replace_cross_run_selection(selection(), "Z", context(&format(&source)))
        .unwrap();
    let b = source
        .replace_cross_run_selection(reverse, "Z", context(&format(&source)))
        .unwrap();
    assert_eq!(a.source.wire(), b.source.wire());
    assert_eq!(a.selection, b.selection);
    assert_eq!(b.insertion_owner, Some(0));
}

#[test]
fn same_plain_replacement_can_change_selected_format_and_is_not_noop() {
    let source = native(&xml());
    let patch = source
        .replace_cross_run_selection(selection(), "😀&b가中c한", context(&format(&source)))
        .unwrap();
    assert_eq!(patch.source.plain_text(), source.plain_text());
    assert_ne!(patch.source.wire(), source.wire());
    let effective = patch.source.selection_format(selection()).unwrap();
    assert_eq!(effective.style, Some(FontStyle::Normal));
    assert_eq!(effective.text_decorations, Some(TextDecorations::None));
    assert_eq!(body_parts(patch.source.xaml()).1[1], "");
}

#[test]
fn deletion_retains_empty_runs_but_does_not_invent_future_typing_affinity() {
    let source = native(&xml());
    let patch = source.delete_cross_run_selection(selection()).unwrap();
    assert_eq!(patch.source.plain_text(), "adef");
    assert_eq!(patch.selection.focus, 1);
    assert_eq!(patch.insertion_owner, None);
    assert_eq!(
        body_parts(source.xaml()).0,
        body_parts(patch.source.xaml()).0
    );
    assert_eq!(body_parts(patch.source.xaml()).1, ["a", "", "", "def"]);
}

#[test]
fn wrong_owner_retained_format_and_context_are_atomic_errors() {
    let source = native(&xml());
    let format = format(&source);
    let saved = serde_json::to_vec(source.wire()).unwrap();
    for change in 0..6 {
        let mut ctx = context(&format);
        let mut different = format.clone();
        match change {
            0 => ctx.insertion_owner = 3,
            1 => ctx.has_retained_springload = true,
            2 => ctx.input_language = "en-US",
            3 => ctx.input_flow_direction = TypingFlowDirection::RightToLeft,
            4 => {
                different.weight = Some(Weight::Bold);
                ctx.expected_format = &different;
            }
            _ => {
                different.text_decorations = None;
                ctx.expected_format = &different;
            }
        }
        assert!(
            source
                .replace_cross_run_selection(selection(), "Z", ctx)
                .is_err(),
            "case {change}"
        );
        assert_eq!(serde_json::to_vec(source.wire()).unwrap(), saved);
    }
}

#[test]
fn missing_native_language_flow_or_decoration_is_not_fabricated() {
    for text in [
        xml()
            .replace(" xml:lang=\"ko-kr\"", "")
            .replace(" xml:lang=\"en-US\"", ""),
        xml().replace(" FlowDirection=\"LeftToRight\"", ""),
        xml().replacen(" TextDecorations=\"None\"", "", 1),
    ] {
        let source = native(&text);
        let expected = format(&native(&xml()));
        assert!(
            source
                .replace_cross_run_selection(selection(), "Z", context(&expected))
                .is_err()
        );
        assert!(source.delete_cross_run_selection(selection()).is_ok());
    }
}

#[test]
fn collapsed_single_run_and_invalid_indices_stay_unsupported_for_cross_run_api() {
    let source = native(&xml());
    for (paragraph, anchor, focus) in [
        (0, 1, 1),
        (0, 1, 3),
        (0, 0, 12),
        (99, 1, 8),
        (0, 1, usize::MAX),
    ] {
        assert!(
            source
                .delete_cross_run_selection(TextSelection {
                    paragraph,
                    anchor,
                    focus
                })
                .is_err()
        );
    }
    assert!(
        source
            .replace_cross_run_selection(selection(), "", context(&format(&source)))
            .is_err()
    );
}

#[test]
fn controls_newline_and_escaped_expansion_budget_reject_without_source_mutation() {
    let source = native(&xml());
    let wire = source.wire().clone();
    for text in ["a\nb", "\r", "\t", "\0", "\u{fffe}", "\u{ffff}"] {
        assert!(
            source
                .replace_cross_run_selection(selection(), text, context(&format(&source)))
                .is_err()
        );
    }
    assert!(
        source
            .replace_cross_run_selection(
                selection(),
                &"&".repeat(MAX_SOURCE_BYTES / 3),
                context(&format(&source))
            )
            .is_err()
    );
    assert_eq!(source.wire(), &wire);
}

#[test]
fn native_own_json_package_resources_and_new_authority_round_trip_without_execution() {
    let source = native(&xml());
    let patch = source
        .replace_cross_run_selection(selection(), "한😀", context(&format(&source)))
        .unwrap();
    let encoded = serde_json::to_vec(patch.source.wire()).unwrap();
    let wire: RichTextWire = serde_json::from_slice(&encoded).unwrap();
    let own =
        ValidatedRichText::from_wire(wire, source.authority.clone(), patch.source.plain_text())
            .unwrap();
    assert_eq!(own.xaml(), patch.source.xaml());
    let exported = own.export_package().unwrap();
    let mut zip = ZipArchive::new(Cursor::new(exported.as_ref())).unwrap();
    let mut resource = Vec::new();
    zip.by_name("Resources/opaque.bin")
        .unwrap()
        .read_to_end(&mut resource)
        .unwrap();
    assert_eq!(resource, [0, 255, 7, 128, 3]);
    let fresh = OriginalTextAuthority::from_package(
        identity(),
        exported.as_ref().to_vec(),
        CATALOG.to_vec(),
    )
    .unwrap();
    let reopened = ValidatedRichText::from_original(fresh, own.plain_text()).unwrap();
    assert_eq!(reopened.xaml(), own.xaml());
    assert!(
        source
            .authority
            .original_xaml()
            .contains("a&#x1F600;&amp;b")
    );
}

#[test]
fn actual_native_sample_delete_does_not_drop_language_key_background_or_root_typography() {
    let source = native(ACTUAL);
    let patch = source
        .delete_cross_run_selection(TextSelection {
            paragraph: 0,
            anchor: 2,
            focus: 7,
        })
        .unwrap();
    assert_eq!(patch.source.plain_text(), "LC시스템 설계");
    assert_eq!(
        body_parts(source.xaml()).0,
        body_parts(patch.source.xaml()).0
    );
    let old_root = &source.xaml()[..source.xaml().find('>').unwrap() + 1];
    assert!(patch.source.xaml().starts_with(old_root));
    assert!(patch.source.xaml().contains("xml:lang=\"ko-kr\""));
    assert!(patch.source.xaml().contains("Background=\"#FF662D91\""));
}

#[test]
fn second_paragraph_coordinates_are_local_and_global_ranges_remain_correct() {
    let extra = "<Paragraph><Run style=\"Normal\" TextDecorations=\"None\">abcd</Run><Run style=\"Normal\" TextDecorations=\"None\">efgh</Run></Paragraph>";
    let source = native(&xml().replace("</Section>", &format!("{extra}</Section>")));
    let patch = source
        .delete_cross_run_selection(TextSelection {
            paragraph: 1,
            anchor: 2,
            focus: 6,
        })
        .unwrap();
    assert_eq!(patch.source.plain_text(), "a😀&b가中c한def\nabgh");
    assert_eq!(
        patch.selection,
        TextSelection {
            paragraph: 1,
            anchor: 2,
            focus: 2
        }
    );
    let ranges: Vec<_> = patch
        .source
        .run_char_ranges()
        .filter(|r| r.paragraph == 1)
        .collect();
    assert_eq!((ranges[0].global_start, ranges[0].global_end), (12, 14));
    assert_eq!((ranges[1].global_start, ranges[1].global_end), (14, 16));
}

#[test]
fn mapped_paragraph_metadata_proof_and_origin_bytes_survive_cross_run_deletion() {
    let original = native(&xml());
    let split = original
        .split_paragraph_at_caret(TextSelection {
            paragraph: 0,
            anchor: 2,
            focus: 2,
        })
        .unwrap();
    let mapped = split.patch.source;
    assert_eq!(mapped.plain_text(), "a😀\n&b가中c한def");
    let patch = mapped
        .delete_cross_run_selection(TextSelection {
            paragraph: 1,
            anchor: 1,
            focus: 6,
        })
        .unwrap();
    assert_eq!(patch.source.plain_text(), "a😀\n&def");
    assert_eq!(
        patch.source.wire().paragraph_origins,
        mapped.wire().paragraph_origins
    );
    assert!(patch.source.prove_preserved_metadata().is_ok());
    assert!(patch.source.prove_format_only().is_err());
    let json = serde_json::to_vec(patch.source.wire()).unwrap();
    let wire = serde_json::from_slice(&json).unwrap();
    assert_eq!(
        ValidatedRichText::from_wire(wire, original.authority.clone(), patch.source.plain_text())
            .unwrap()
            .xaml(),
        patch.source.xaml()
    );
}

#[test]
fn aggregate_provenance_budget_is_checked_before_allocating_candidate_xml() {
    let source = native(&xml());
    let expected = format(&source);
    // Independently chosen remaining envelope: original source fits exactly,
    // while a larger replacement would exceed XML+provenance, not XML alone.
    let extra = MAX_SOURCE_BYTES - source.xaml().len();
    assert!(
        prepare_cross_run(
            source.xaml(),
            &source.index,
            selection(),
            &"z".repeat(256),
            Some(context(&expected)),
            None,
            None,
            extra
        )
        .is_err()
    );
    let deleted = prepare_cross_run(
        source.xaml(),
        &source.index,
        selection(),
        "",
        None,
        None,
        None,
        extra,
    )
    .unwrap();
    assert!(deleted.xml.len() + extra <= MAX_SOURCE_BYTES);
    assert_eq!(source.plain_text(), "a😀&b가中c한def");
}

fn authored() -> AuthoredRichText {
    let style = AuthoredStyleSnapshot {
        key: "Body".into(),
        font_family: "Malgun Gothic".into(),
        size_dip: "16".into(),
        weight: "Normal".into(),
        font_style: "Normal".into(),
        alignment: "Left".into(),
        foreground_rgba: [255; 4],
        background_rgba: [0, 0, 0, 0],
    };
    let seed = AuthoredTextSeed::new(AuthoredTextSeedSpec {
        format: style.clone(),
        proofing_language: "ko-KR".into(),
        catalog: vec![style],
        text_decorations: None,
    })
    .unwrap();
    let source = AuthoredRichText::empty(seed)
        .unwrap()
        .replace_in_run(0, 0, 0, "abcdefghi")
        .unwrap()
        .unwrap()
        .source;
    source
        .set_selection_format(
            TextSelection {
                paragraph: 0,
                anchor: 3,
                focus: 6,
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

#[test]
fn authored_split_runs_keep_seed_authority_and_complete_effective_typing_format() {
    let source = authored();
    let expected = source
        .selection_format(TextSelection {
            paragraph: 0,
            anchor: 1,
            focus: 2,
        })
        .unwrap();
    let patch = source
        .replace_cross_run_selection(
            TextSelection {
                paragraph: 0,
                anchor: 1,
                focus: 8,
            },
            "한😀",
            context(&expected),
        )
        .unwrap();
    assert_eq!(patch.source.plain_text(), "a한😀i");
    assert_eq!(patch.selection.focus, 3);
    assert_eq!(patch.insertion_owner, Some(0));
    assert!(Arc::ptr_eq(source.seed(), patch.source.seed()));
    assert_eq!(
        body_parts(source.xaml()).0,
        body_parts(patch.source.xaml()).0
    );
    let wire = serde_json::to_vec(patch.source.wire()).unwrap();
    assert_eq!(
        AuthoredRichText::from_json(&wire, "a한😀i").unwrap().xaml(),
        patch.source.xaml()
    );
}

#[test]
fn authored_delete_and_typed_same_plain_have_distinct_policies_and_old_undo_sources_survive() {
    let original = authored();
    let selection = TextSelection {
        paragraph: 0,
        anchor: 1,
        focus: 8,
    };
    let deleted = original.delete_cross_run_selection(selection).unwrap();
    assert_eq!(deleted.source.plain_text(), "ai");
    assert_eq!(deleted.insertion_owner, None);
    let expected = original
        .selection_format(TextSelection {
            paragraph: 0,
            anchor: 1,
            focus: 2,
        })
        .unwrap();
    let same = original
        .replace_cross_run_selection(selection, "bcdefgh", context(&expected))
        .unwrap();
    assert_eq!(same.source.plain_text(), original.plain_text());
    assert_ne!(same.source.wire(), original.wire());
    assert_eq!(
        same.source.selection_format(selection).unwrap().weight,
        Some(Weight::Normal)
    );
    assert_eq!(
        original
            .selection_format(TextSelection {
                paragraph: 0,
                anchor: 3,
                focus: 6
            })
            .unwrap()
            .weight,
        Some(Weight::Bold)
    );
}

#[test]
fn boundary_and_full_paragraph_delete_preserve_literal_wrappers_entities_and_cold_authority() {
    let original = native(&xml());
    let wire = serde_json::to_vec(original.wire()).unwrap();
    for (anchor, focus, plain, first_body, last_body) in [
        (0, 8, "def", "", "d&#101;f"),
        (1, 11, "a", "a", ""),
        (0, 11, "", "", ""),
        (11, 0, "", "", ""),
    ] {
        let deleted = original
            .delete_cross_run_selection(TextSelection {
                paragraph: 0,
                anchor,
                focus,
            })
            .unwrap();
        let expected = xml()
            .replace("a&#x1F600;&amp;b", first_body)
            .replace("가中", "")
            .replace("c한d&#101;f", last_body);
        assert_eq!(deleted.source.xaml(), expected);
        assert_eq!(deleted.source.plain_text(), plain);
        assert_eq!(deleted.selection.anchor, anchor.min(focus));
        assert_eq!(deleted.selection.anchor, deleted.selection.focus);
        assert_eq!(deleted.insertion_owner, None);
        assert_eq!(
            body_parts(original.xaml()).0,
            body_parts(deleted.source.xaml()).0
        );
        assert_eq!(deleted.source.run_char_ranges().count(), 4);
        let reopened = ValidatedRichText::from_wire(
            deleted.source.wire().clone(),
            original.authority.clone(),
            plain,
        )
        .unwrap();
        assert_eq!(reopened.xaml(), expected);
        let output = reopened.export_package().unwrap();
        let mut zip = ZipArchive::new(Cursor::new(output.as_ref())).unwrap();
        let mut opaque = Vec::new();
        zip.by_name("Resources/opaque.bin")
            .unwrap()
            .read_to_end(&mut opaque)
            .unwrap();
        assert_eq!(opaque, [0, 255, 7, 128, 3]);
    }
    // Deletion endpoints never become a typing contract.
    assert!(
        original
            .replace_cross_run_selection(
                TextSelection {
                    paragraph: 0,
                    anchor: 0,
                    focus: 11
                },
                "X",
                context(&format(&original))
            )
            .is_err()
    );
    assert_eq!(serde_json::to_vec(original.wire()).unwrap(), wire);
}

#[test]
fn authored_full_deletion_keeps_seed_and_empty_run_metadata_and_rejects_separator_range() {
    let original = authored();
    let len = original.plain_text().chars().count();
    let deleted = original
        .delete_cross_run_selection(TextSelection {
            paragraph: 0,
            anchor: 0,
            focus: len,
        })
        .unwrap();
    assert_eq!(deleted.source.plain_text(), "");
    assert_eq!(deleted.insertion_owner, None);
    assert_eq!(
        body_parts(original.xaml()).0,
        body_parts(deleted.source.xaml()).0
    );
    assert!(
        body_parts(deleted.source.xaml())
            .1
            .iter()
            .all(String::is_empty)
    );
    assert!(Arc::ptr_eq(original.seed(), deleted.source.seed()));
    assert_eq!(
        AuthoredRichText::from_json(&serde_json::to_vec(deleted.source.wire()).unwrap(), "")
            .unwrap()
            .xaml(),
        deleted.source.xaml()
    );
    let multi = native(&xml().replace(
        "</Paragraph>",
        "</Paragraph><Paragraph><Run style=\"Normal\">tail</Run></Paragraph>",
    ));
    assert!(
        multi
            .delete_cross_run_selection(TextSelection {
                paragraph: 0,
                anchor: 0,
                focus: 12
            })
            .is_err()
    );
}
