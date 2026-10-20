//! Independent source/body/format assertions; proposed, not executed here.
use super::*;
use crate::rich_text_source::{
    AuthoredStyleSnapshot, AuthoredTextSeed, AuthoredTextSeedSpec, TypingFlowDirection, Weight,
};

fn source() -> AuthoredRichText {
    let style = AuthoredStyleSnapshot {
        key: "ClosedBody".into(),
        font_family: "Tahoma".into(),
        size_dip: "18.5".into(),
        weight: "Normal".into(),
        font_style: "Normal".into(),
        alignment: "Left".into(),
        foreground_rgba: [18, 52, 86, 255],
        background_rgba: [255, 255, 255, 0],
    };
    let seed = AuthoredTextSeed::new(AuthoredTextSeedSpec {
        format: style.clone(),
        catalog: vec![style],
        proofing_language: "ko-KR".into(),
        text_decorations: None,
    })
    .unwrap();
    let s = AuthoredRichText::empty(seed)
        .unwrap()
        .replace_in_run(0, 0, 0, "가&😀AB꼬")
        .unwrap()
        .unwrap()
        .source;
    s.set_selection_format(
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
    .source
}
fn full(s: &AuthoredRichText) -> TextSelection {
    TextSelection {
        paragraph: 0,
        anchor: 0,
        focus: s.plain_text().chars().count(),
    }
}
fn owner(s: &AuthoredRichText) -> usize {
    s.run_char_ranges()
        .find(|r| r.global_start < r.global_end)
        .unwrap()
        .run_ordinal
}
fn format(s: &AuthoredRichText) -> TextFormat {
    let r = s
        .run_char_ranges()
        .find(|r| r.run_ordinal == owner(s))
        .unwrap();
    s.selection_format(TextSelection {
        paragraph: 0,
        anchor: r.start_in_paragraph,
        focus: r.start_in_paragraph + 1,
    })
    .unwrap()
}
fn apply(
    s: &AuthoredRichText,
    text: &str,
    selection: TextSelection,
) -> CrossRunPatch<AuthoredRichText> {
    let f = format(s);
    s.replace_after_select_all(
        selection,
        text,
        CrossRunTypingContext {
            insertion_owner: owner(s),
            expected_format: &f,
            input_language: "ko-KR",
            input_flow_direction: TypingFlowDirection::LeftToRight,
            has_retained_springload: false,
        },
    )
    .unwrap()
}
// Independent DOM/raw span oracle: no source index/production metadata proof.
fn skeleton(xml: &str) -> String {
    let doc = roxmltree::Document::parse(xml).unwrap();
    let mut out = String::new();
    let mut copied = 0;
    for run in doc
        .descendants()
        .filter(|n| n.is_element() && n.tag_name().name() == "Run")
    {
        let range = run.range();
        let raw = &xml[range.clone()];
        let open = range.start + raw.find('>').unwrap() + 1;
        let close = range.start + raw.rfind("</").unwrap();
        out.push_str(&xml[copied..open]);
        copied = close;
    }
    out.push_str(&xml[copied..]);
    out
}
fn bodies(xml: &str) -> Vec<(Option<String>, String)> {
    let d = roxmltree::Document::parse(xml).unwrap();
    d.descendants()
        .filter(|n| n.is_element() && n.tag_name().name() == "Run")
        .map(|n| {
            (
                n.attribute("FontWeight").map(str::to_owned),
                n.children().filter_map(|n| n.text()).collect(),
            )
        })
        .collect()
}
#[test]
fn full_unicode_replacement_preserves_all_raw_wrappers_and_current_first_format() {
    let s = source();
    let before = skeleton(s.xaml());
    assert_eq!(s.seed().spec().format.weight, "Normal");
    let p = apply(&s, "새<&😀", full(&s));
    assert_eq!(p.source.plain_text(), "새<&😀");
    assert_eq!(p.insertion_owner, Some(0));
    assert_eq!(
        p.selection,
        TextSelection {
            paragraph: 0,
            anchor: 4,
            focus: 4
        }
    );
    assert_eq!(skeleton(p.source.xaml()), before);
    assert_eq!(
        bodies(p.source.xaml()),
        vec![
            (Some("Bold".into()), "새<&😀".into()),
            (Some("Normal".into()), "".into())
        ]
    );
}
#[test]
fn leading_empty_run_is_skipped_not_seed_or_ordinal_zero() {
    let s = AuthoredRichText::empty(source().seed().clone())
        .unwrap()
        .replace_in_run(0, 0, 0, "ABCD")
        .unwrap()
        .unwrap()
        .source;
    let s = s
        .set_selection_format(
            TextSelection {
                paragraph: 0,
                anchor: 2,
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
    let s = s.replace_in_run(0, 0, 2, "").unwrap().unwrap().source;
    assert_eq!(owner(&s), 1);
    let p = apply(&s, "Z", full(&s));
    assert_eq!(p.insertion_owner, Some(1));
    assert_eq!(skeleton(p.source.xaml()), skeleton(s.xaml()));
    let b = bodies(p.source.xaml());
    assert_eq!(b[0], (Some("Normal".into()), "".into()));
    assert_eq!(b[1], (Some("Bold".into()), "Z".into()));
}
#[test]
fn reverse_full_selection_has_identical_normalized_owner() {
    let s = source();
    let f = full(&s);
    let forward = apply(&s, "한", f);
    let reverse = apply(
        &s,
        "한",
        TextSelection {
            anchor: f.focus,
            focus: 0,
            ..f
        },
    );
    assert_eq!(forward.source.wire(), reverse.source.wire());
    assert_eq!(forward.insertion_owner, reverse.insertion_owner);
}
#[test]
fn same_plain_replacement_is_not_automatically_a_source_noop() {
    let s = source();
    let p = apply(&s, s.plain_text(), full(&s));
    assert_eq!(p.source.plain_text(), s.plain_text());
    assert_ne!(p.source.xaml(), s.xaml());
    assert_eq!(bodies(p.source.xaml())[0].1, s.plain_text());
    assert_eq!(skeleton(p.source.xaml()), skeleton(s.xaml()));
}
#[test]
fn exact_single_run_same_value_keeps_json_bytes() {
    let s = AuthoredRichText::empty(source().seed().clone())
        .unwrap()
        .replace_in_run(0, 0, 0, "같음😀")
        .unwrap()
        .unwrap()
        .source;
    let before = serde_json::to_vec(s.wire()).unwrap();
    let p = apply(&s, s.plain_text(), full(&s));
    assert_eq!(serde_json::to_vec(p.source.wire()).unwrap(), before);
}
#[test]
fn wrong_owner_format_profile_or_retained_state_rejects_without_source_change() {
    let s = source();
    let f = format(&s);
    let original = serde_json::to_vec(s.wire()).unwrap();
    for case in 0..5 {
        let mut partial = f.clone();
        if case == 1 {
            partial.weight = None;
        }
        let context = CrossRunTypingContext {
            insertion_owner: if case == 0 { 1 } else { 0 },
            expected_format: &partial,
            input_language: if case == 2 { "en-US" } else { "ko-KR" },
            input_flow_direction: if case == 3 {
                TypingFlowDirection::RightToLeft
            } else {
                TypingFlowDirection::LeftToRight
            },
            has_retained_springload: case == 4,
        };
        assert!(s.replace_after_select_all(full(&s), "X", context).is_err());
        assert_eq!(serde_json::to_vec(s.wire()).unwrap(), original);
    }
}
#[test]
fn incomplete_range_empty_source_multiple_paragraphs_and_controls_reject() {
    let s = source();
    let f = format(&s);
    for selection in [
        TextSelection {
            paragraph: 0,
            anchor: 1,
            focus: full(&s).focus,
        },
        TextSelection {
            paragraph: 0,
            anchor: 0,
            focus: 1,
        },
        TextSelection {
            paragraph: 1,
            anchor: 0,
            focus: full(&s).focus,
        },
    ] {
        assert!(
            s.replace_after_select_all(
                selection,
                "X",
                CrossRunTypingContext {
                    insertion_owner: 0,
                    expected_format: &f,
                    input_language: "ko-KR",
                    input_flow_direction: TypingFlowDirection::LeftToRight,
                    has_retained_springload: false,
                }
            )
            .is_err()
        );
    }
    for text in ["", "\n", "\r", "\t", "\u{0}", "\u{FFFF}"] {
        assert!(
            s.replace_after_select_all(
                full(&s),
                text,
                CrossRunTypingContext {
                    insertion_owner: 0,
                    expected_format: &f,
                    input_language: "ko-KR",
                    input_flow_direction: TypingFlowDirection::LeftToRight,
                    has_retained_springload: false,
                }
            )
            .is_err()
        );
    }
    let empty = AuthoredRichText::empty(s.seed().clone()).unwrap();
    let split = s
        .split_paragraph_at_caret(TextSelection {
            paragraph: 0,
            anchor: 1,
            focus: 1,
        })
        .unwrap()
        .source;
    for value in [&empty, &split] {
        assert!(
            value
                .replace_after_select_all(
                    full(value),
                    "X",
                    CrossRunTypingContext {
                        insertion_owner: 0,
                        expected_format: &f,
                        input_language: "ko-KR",
                        input_flow_direction: TypingFlowDirection::LeftToRight,
                        has_retained_springload: false,
                    }
                )
                .is_err()
        );
    }
}
#[test]
fn escaped_and_own_wire_budget_rejection_is_atomic() {
    let s = source();
    let f = format(&s);
    let before = serde_json::to_vec(s.wire()).unwrap();
    let large = "&".repeat(MAX_SOURCE_BYTES / 5 + 1);
    assert!(
        s.replace_after_select_all(
            full(&s),
            &large,
            CrossRunTypingContext {
                insertion_owner: 0,
                expected_format: &f,
                input_language: "ko-KR",
                input_flow_direction: TypingFlowDirection::LeftToRight,
                has_retained_springload: false,
            }
        )
        .is_err()
    );
    assert_eq!(serde_json::to_vec(s.wire()).unwrap(), before);
}
#[test]
fn own_cold_roundtrip_retains_source_without_encoding_command_provenance() {
    let s = source();
    let p = apply(&s, "한😀&", full(&s));
    let bytes = serde_json::to_vec(p.source.wire()).unwrap();
    let cold = AuthoredRichText::from_json(&bytes, "한😀&").unwrap();
    assert_eq!(cold.xaml(), p.source.xaml());
    assert!(!String::from_utf8(bytes).unwrap().contains("SelectAll"));
}
