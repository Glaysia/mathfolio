use super::*;

fn spec() -> AuthoredTextSeedSpec {
    let style = AuthoredStyleSnapshot {
        key: "CustomBody".into(),
        font_family: "Noto Sans CJK KR".into(),
        size_dip: "17.25".into(),
        weight: "Normal".into(),
        font_style: "Italic".into(),
        alignment: "Left".into(),
        foreground_rgba: [12, 34, 56, 255],
        background_rgba: [240, 230, 220, 128],
    };
    AuthoredTextSeedSpec {
        format: style.clone(),
        proofing_language: "ko-KR".into(),
        catalog: vec![style],
        text_decorations: None,
    }
}
fn empty() -> AuthoredRichText {
    AuthoredRichText::empty(AuthoredTextSeed::new(spec()).unwrap()).unwrap()
}
fn text(value: &str) -> AuthoredRichText {
    empty()
        .replace_in_run(0, 0, 0, value)
        .unwrap()
        .unwrap()
        .source
}

#[test]
fn empty_has_explicit_current_snapshot_and_own_provenance_no_native_anchor() {
    let value = empty();
    assert_eq!(value.plain_text(), "");
    assert!(
        value
            .xaml()
            .contains("<Paragraph><Run style=\"CustomBody\"")
    );
    assert!(value.xaml().contains("></Run>"));
    assert!(value.xaml().contains("FontFamily=\"Noto Sans CJK KR\""));
    assert!(value.xaml().contains("xml:lang=\"ko-KR\""));
    assert!(value.xaml().contains("FontSize=\"17.25\""));
    let json = serde_json::to_string(value.wire()).unwrap();
    assert!(!json.contains("owner_region_id"));
    assert!(!json.contains("package_part"));
    let reopened = AuthoredRichText::from_json(json.as_bytes(), "").unwrap();
    assert_eq!(reopened.wire(), value.wire());
}
#[test]
fn committed_korean_emoji_entities_and_cold_reopen_preserve_source_style() {
    let start = empty();
    let patch = start.replace_in_run(0, 0, 0, "한😀<&\"'").unwrap().unwrap();
    assert_eq!(patch.selection.focus, 6);
    assert_eq!(patch.source.plain_text(), "한😀<&\"'");
    assert_eq!(start.plain_text(), "");
    assert!(patch.source.xaml().contains("&lt;&amp;&quot;&apos;"));
    assert!(patch.source.xaml().contains("FontStyle=\"Italic\""));
    let bytes = serde_json::to_vec(patch.source.wire()).unwrap();
    let reopened = AuthoredRichText::from_json(&bytes, patch.source.plain_text()).unwrap();
    assert_eq!(reopened.xaml(), patch.source.xaml());
    assert!(
        reopened
            .replace_in_run(0, 0, 6, "한😀<&\"'")
            .unwrap()
            .is_none()
    );
    assert!(reopened.replace_in_run(0, 0, 0, "a\nb").is_err());
}
#[test]
fn selected_style_splits_only_chosen_run_and_preserves_immutable_snapshot() {
    let value = text("가😀나다");
    let selection = TextSelection {
        paragraph: 0,
        anchor: 1,
        focus: 3,
    };
    let format = TextFormat {
        weight: Some(Weight::Bold),
        foreground_argb: Some([255, 1, 2, 3]),
        ..Default::default()
    };
    let patch = value
        .set_selection_format(selection, &format)
        .unwrap()
        .unwrap();
    assert_eq!(patch.source.plain_text(), value.plain_text());
    assert_eq!(patch.source.run_char_ranges().count(), 3);
    assert_eq!(patch.selection, selection);
    assert_eq!(
        patch.source.xaml().matches("style=\"CustomBody\"").count(),
        3
    );
    assert_eq!(
        patch.source.xaml().matches("FontWeight=\"Bold\"").count(),
        1
    );
    assert!(patch.source.xaml().contains("#FF010203"));
    assert_eq!(patch.source.seed().spec(), &spec());
    assert!(
        patch
            .source
            .set_selection_format(selection, &format)
            .unwrap()
            .is_none()
    );
    assert!(
        value
            .set_selection_format(
                TextSelection {
                    paragraph: 0,
                    anchor: 1,
                    focus: 1
                },
                &format
            )
            .is_err()
    );
}
#[test]
fn repeated_interior_paragraph_split_and_own_reopen_have_no_imported_map() {
    let value = text("가😀나다라");
    let first = value
        .split_paragraph_at_caret(TextSelection {
            paragraph: 0,
            anchor: 2,
            focus: 2,
        })
        .unwrap();
    assert_eq!(first.source.plain_text(), "가😀\n나다라");
    assert_eq!(
        first.selection,
        TextSelection {
            paragraph: 1,
            anchor: 0,
            focus: 0
        }
    );
    let next = first
        .source
        .split_paragraph_at_caret(TextSelection {
            paragraph: 1,
            anchor: 1,
            focus: 1,
        })
        .unwrap();
    assert_eq!(next.source.plain_text(), "가😀\n나\n다라");
    assert_eq!(next.source.run_char_ranges().count(), 3);
    let bytes = serde_json::to_vec(next.source.wire()).unwrap();
    assert!(!String::from_utf8_lossy(&bytes).contains("paragraph_origins"));
    assert_eq!(
        AuthoredRichText::from_json(&bytes, next.source.plain_text())
            .unwrap()
            .xaml(),
        next.source.xaml()
    );
    assert_eq!(
        value
            .split_paragraph_at_caret(TextSelection {
                paragraph: 0,
                anchor: 0,
                focus: 0
            })
            .unwrap()
            .source
            .plain_text(),
        "\n가😀나다라"
    );
}
#[test]
fn untrusted_seed_changes_require_regeneration_and_matching_checksum() {
    let value = text("hello");
    let mut wire = value.wire().clone();
    Arc::make_mut(&mut wire.seed).format.font_family = "Other Font".into();
    assert!(AuthoredRichText::from_wire(wire, "hello").is_err());
    let mut wire = value.wire().clone();
    wire.seed_sha256 = [0; 32];
    assert!(AuthoredRichText::from_wire(wire, "hello").is_err());
    let mut wire = value.wire().clone();
    wire.version = 2;
    assert!(AuthoredRichText::from_wire(wire, "hello").is_err());
    let mut wire = value.wire().clone();
    wire.current_xaml = wire
        .current_xaml
        .replace("Noto Sans CJK KR", "Other Font")
        .into();
    assert!(AuthoredRichText::from_wire(wire, "hello").is_err());
    assert!(AuthoredRichText::from_wire(value.wire().clone(), "different").is_err());
}
#[test]
fn strict_json_rejects_duplicate_unknown_and_null_catalog_fields() {
    let bytes = serde_json::to_string(empty().wire()).unwrap();
    for bad in [
        bytes.replacen("\"version\":1", "\"version\":1,\"version\":1", 1),
        bytes.replacen("\"version\":1", "\"version\":1,\"future\":true", 1),
        bytes.replacen(
            "\"proofing_language\":",
            "\"unknown_catalog\":1,\"proofing_language\":",
            1,
        ),
    ] {
        assert!(AuthoredRichText::from_json(bad.as_bytes(), "").is_err());
    }
    let mut v = serde_json::to_value(empty().wire()).unwrap();
    v["seed"]["catalog"] = serde_json::Value::Null;
    assert!(AuthoredRichText::from_json(&serde_json::to_vec(&v).unwrap(), "").is_err());
}
#[test]
fn absent_duplicate_unsupported_catalog_and_snapshot_are_not_defaulted() {
    let mut s = spec();
    s.catalog.clear();
    assert!(AuthoredTextSeed::new(s).is_err());
    let mut s = spec();
    s.catalog.push(s.catalog[0].clone());
    assert!(AuthoredTextSeed::new(s).is_err());
    let mut s = spec();
    s.format.key = "Missing".into();
    assert!(AuthoredTextSeed::new(s).is_err());
    let mut s = spec();
    s.format.font_family.clear();
    assert!(AuthoredTextSeed::new(s).is_err());
    let mut s = spec();
    s.format.weight = "SemiBold".into();
    assert!(AuthoredTextSeed::new(s).is_err());
    let mut s = spec();
    s.proofing_language = "{Binding}".into();
    assert!(AuthoredTextSeed::new(s).is_err());
}
#[test]
fn forged_source_metadata_and_nonrun_injection_do_not_become_authored() {
    let value = text("ab");
    for xml in [
        value
            .xaml()
            .replace("<Paragraph>", "<Paragraph FontWeight=\"Bold\">"),
        value
            .xaml()
            .replace("style=\"CustomBody\"", "style=\"Missing\""),
        value.xaml().replace("</Run>", "</Run><Span></Span>"),
        value
            .xaml()
            .replace("<Run ", "<Run Background=\"#FF000000\" "),
        value.xaml().replace("<Paragraph>", "<Paragraph/>"),
    ] {
        let mut wire = value.wire().clone();
        wire.current_xaml = xml.into();
        assert!(AuthoredRichText::from_wire(wire, "ab").is_err());
    }
}
#[test]
fn exact_wire_budget_counts_seed_catalog_and_json_escapes_per_owner() {
    let a = text("\"&😀");
    let serialized = serde_json::to_vec(a.wire()).unwrap();
    assert_eq!(a.budget().serialized_wire_bytes, serialized.len());
    assert_eq!(a.budget().current_xml_bytes, a.xaml().len());
    assert_eq!(
        a.budget().serialized_seed_bytes,
        serde_json::to_vec(a.seed().spec()).unwrap().len()
    );
    assert_eq!(a.clone().budget(), a.budget());
    let cost = a.budget().serialized_wire_bytes;
    assert_eq!(
        checked_authored_wire_bytes([&a, &a], cost * 2).unwrap(),
        cost * 2
    );
    assert!(checked_authored_wire_bytes([&a, &a], cost * 2 - 1).is_err());
    assert!(a.budget().serialized_wire_bytes > a.xaml().len() + a.seed().serialized_spec_bytes());
    // XML remains below cap while JSON escaping makes exact own wire exceed it.
    assert!(
        empty()
            .replace_in_run(0, 0, 0, &"\\".repeat(MAX_SOURCE_BYTES * 3 / 5))
            .is_err()
    );
    assert!(AuthoredRichText::from_json(&vec![b' '; MAX_SOURCE_BYTES + 1], "").is_err());
}

fn assert_cost(value: &AuthoredRichText) {
    assert_eq!(
        value.budget().serialized_wire_bytes(),
        serde_json::to_vec(value.wire()).unwrap().len()
    );
    assert_eq!(
        value.seed().serialized_spec_bytes(),
        serde_json::to_vec(value.seed().spec()).unwrap().len()
    );
    assert_eq!(
        value.seed().encoded_hash_bytes(),
        serde_json::to_vec(&value.seed().fingerprint())
            .unwrap()
            .len()
    );
}
#[test]
fn escaped_string_cost_matches_independent_serde_for_controls_unicode_and_slashes() {
    let controls: String = (0u32..32).map(|n| char::from_u32(n).unwrap()).collect();
    for value in [
        "",
        "한😀",
        "\"\\/",
        "\u{2028}\u{2029}",
        controls.as_str(),
        "<>&'",
    ] {
        assert_eq!(
            json_string_bytes(value).unwrap(),
            serde_json::to_vec(value).unwrap().len()
        );
    }
}
#[test]
fn cached_wire_cost_matches_serde_after_real_edits_format_and_splits_with_large_seed() {
    let mut s = spec();
    for n in 0..200 {
        let mut style = s.format.clone();
        style.key = format!("Heading {n}~한국");
        style.font_family = "가".repeat(80);
        s.catalog.push(style);
    }
    let seed = AuthoredTextSeed::new(s).unwrap();
    assert!(seed.serialized_spec_bytes() > 60_000);
    let a = AuthoredRichText::empty(seed).unwrap();
    assert_cost(&a);
    let b = a
        .replace_in_run(0, 0, 0, "한😀\\\"<&'")
        .unwrap()
        .unwrap()
        .source;
    assert_cost(&b);
    let c = b
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
    assert_cost(&c);
    let d = c
        .split_paragraph_at_caret(TextSelection {
            paragraph: 0,
            anchor: 0,
            focus: 0,
        })
        .unwrap()
        .source;
    assert_cost(&d);
    assert!(Arc::ptr_eq(&a.seed.spec, &b.wire.seed));
    assert!(Arc::ptr_eq(&b.wire.seed, &c.wire.seed));
    assert!(Arc::ptr_eq(&c.wire.seed, &d.wire.seed));
}
#[test]
fn arc_wire_keeps_old_owned_seed_json_shape_and_cold_admission_canonicalizes_pointer() {
    #[derive(Serialize)]
    struct OldWire<'a> {
        version: u8,
        seed: &'a AuthoredTextSeedSpec,
        seed_sha256: [u8; 32],
        current_xaml: &'a str,
    }
    let a = text("한😀\\");
    let old = OldWire {
        version: 1,
        seed: a.seed().spec(),
        seed_sha256: a.seed().fingerprint(),
        current_xaml: a.xaml(),
    };
    let bytes = serde_json::to_vec(a.wire()).unwrap();
    assert_eq!(bytes, serde_json::to_vec(&old).unwrap());
    let reopened = AuthoredRichText::from_json(&bytes, a.plain_text()).unwrap();
    assert!(Arc::ptr_eq(&reopened.wire.seed, &reopened.seed.spec));
    assert_cost(&reopened);
}
#[test]
fn original_heading_space_and_explicit_unicode_keys_roundtrip_verbatim() {
    let original = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/reference/rich-text/lcc-standard-presentation.xml"
    ));
    assert!(original.contains("style-key=\"Heading 1\""));
    for key in ["Heading 1", "직접 지정~본문"] {
        let mut s = spec();
        s.format.key = key.into();
        s.catalog[0].key = key.into();
        let a = AuthoredRichText::empty(AuthoredTextSeed::new(s).unwrap()).unwrap();
        let b = a
            .replace_in_run(0, 0, 0, "새 본문")
            .unwrap()
            .unwrap()
            .source;
        let bytes = serde_json::to_vec(b.wire()).unwrap();
        let reopened = AuthoredRichText::from_json(&bytes, "새 본문").unwrap();
        assert_eq!(reopened.seed().spec().format.key, key);
        assert!(reopened.xaml().contains(&format!("style=\"{key}\"")));
        assert_cost(&reopened);
    }
}
#[test]
fn empty_end_start_and_repeated_blank_paragraph_enter_preserve_typing_source_and_affinity() {
    let first = empty()
        .split_paragraph_at_caret(TextSelection {
            paragraph: 0,
            anchor: 0,
            focus: 0,
        })
        .unwrap();
    assert_eq!(first.source.plain_text(), "\n");
    assert_eq!(first.following_run, Some(1));
    let next = first
        .source
        .split_paragraph_at_caret(first.selection)
        .unwrap();
    assert_eq!(next.source.plain_text(), "\n\n");
    assert_eq!(next.following_run, Some(2));
    let typed = next
        .source
        .replace_in_run(next.following_run.unwrap(), 0, 0, "한😀")
        .unwrap()
        .unwrap()
        .source;
    let end = typed
        .split_paragraph_at_caret(TextSelection {
            paragraph: 2,
            anchor: 2,
            focus: 2,
        })
        .unwrap();
    assert_eq!(end.source.plain_text(), "\n\n한😀\n");
    assert_eq!(end.following_run, Some(3));
    let appended = end
        .source
        .replace_in_run(end.following_run.unwrap(), 0, 0, "뒤")
        .unwrap()
        .unwrap()
        .source;
    assert_eq!(appended.plain_text(), "\n\n한😀\n뒤");
    assert_eq!(appended.xaml().matches("FontStyle=\"Italic\"").count(), 4);
    let bytes = serde_json::to_vec(appended.wire()).unwrap();
    assert_eq!(
        AuthoredRichText::from_json(&bytes, appended.plain_text())
            .unwrap()
            .xaml(),
        appended.xaml()
    );
    assert_cost(&appended);
}
#[test]
fn known_catalog_other_run_key_and_active_xml_still_reject() {
    let mut s = spec();
    let mut second = s.format.clone();
    second.key = "Heading 1".into();
    s.catalog.push(second);
    let a = AuthoredRichText::empty(AuthoredTextSeed::new(s).unwrap()).unwrap();
    let mut wire = a.wire().clone();
    wire.current_xaml = wire
        .current_xaml
        .replace("style=\"CustomBody\"", "style=\"Heading 1\"")
        .into();
    assert!(AuthoredRichText::from_wire(wire, "").is_err());
    for injection in ["<!DOCTYPE x>", "<?x y?>", "<![CDATA[]]>"] {
        let mut wire = a.wire().clone();
        wire.current_xaml = wire
            .current_xaml
            .replace("<Paragraph>", &format!("<Paragraph>{injection}"))
            .into();
        assert!(AuthoredRichText::from_wire(wire, "").is_err());
    }
}

#[test]
fn authored_blank_enter_count_limit_is_atomic_and_does_not_change_imported_policy() {
    let a = empty();
    let mut wire = a.wire().clone();
    let body = format!("{P_OPEN}{}</Run>{P_END}", a.seed.run_open).repeat(1024);
    wire.current_xaml = format!("{}{body}{ROOT_END}", a.seed.root_open).into();
    let plain = "\n".repeat(1023);
    // This fixture deliberately changes private DTO fields. Exercise the real
    // cold wire path, which has no cached facts for the previous XML version.
    let bytes = serde_json::to_vec(&wire).unwrap();
    let full = AuthoredRichText::from_json(&bytes, &plain).unwrap();
    assert!(
        full.split_paragraph_at_caret(TextSelection {
            paragraph: 1023,
            anchor: 0,
            focus: 0
        })
        .is_err()
    );
    assert_eq!(full.plain_text(), plain);
    assert_eq!(full.run_char_ranges().count(), 1024);
    assert_cost(&full);
}
