use super::*;
#[test]
fn source_opening_query_matches_full_dom_on_validated_prefixes_and_unicode() {
    let keys = HashSet::from(["Normal".to_owned()]);
    for prelude in ["", "<?xml version=\"1.0\"?>", "\u{feff}"] {
        for (root, decl) in [
            ("Section", format!("xmlns=\"{XAML}\"")),
            ("p:Section", format!("xmlns:p=\"{XAML}\"")),
        ] {
            let p = if root.contains(':') { "p:" } else { "" };
            let xml = format!(
                "{prelude}<{root} {decl} FlowDirection='LeftToRight' TextAlignment='Left' FontFamily='A&gt;B'><{p}Paragraph TextAlignment=\"Right\"><{p}Run style=\"Normal\">한😀</{p}Run><{p}Run></{p}Run></{p}Paragraph></{root}>"
            );
            let index = parse_source(&xml, &keys).unwrap();
            let info = source_info(&xml, &index, false).unwrap();
            assert_eq!(info.alignment(), Some(ParagraphAlignment::Right));
            assert_eq!(info.scalar_len(), 2);
            assert_eq!(
                super::super::check_single(
                    &xml,
                    &index,
                    TextSelection {
                        paragraph: 0,
                        anchor: 2,
                        focus: 2
                    },
                    false
                )
                .unwrap(),
                info.alignment()
            );
        }
    }
}
#[test]
fn missing_source_alignment_is_not_inferred_and_generated_flow_is_separate() {
    let keys = HashSet::new();
    let xml =
        format!("<FlowDocument xmlns=\"{XAML}\"><Paragraph><Run></Run></Paragraph></FlowDocument>");
    let index = parse_source(&xml, &keys).unwrap();
    assert!(source_info(&xml, &index, false).is_err());
    let generated = source_info(&xml, &index, true).unwrap();
    assert_eq!(generated.alignment(), None);
    assert_eq!(generated.scalar_len(), 0);
    let xml = xml.replace("<Paragraph>", "<Paragraph FlowDirection=\"LeftToRight\">");
    let index = parse_source(&xml, &keys).unwrap();
    assert_eq!(source_info(&xml, &index, false).unwrap().alignment(), None);
}
#[test]
fn unsupported_flow_value_and_multiple_paragraphs_never_gain_capability() {
    let keys = HashSet::new();
    for (attrs, body) in [
        (
            "FlowDirection='RightToLeft'",
            "<Paragraph><Run>a</Run></Paragraph>",
        ),
        (
            "FlowDirection='LeftToRight' TextAlignment='Stretch'",
            "<Paragraph><Run>a</Run></Paragraph>",
        ),
        (
            "FlowDirection='LeftToRight'",
            "<Paragraph><Run>a</Run></Paragraph><Paragraph><Run>b</Run></Paragraph>",
        ),
    ] {
        let xml = format!("<Section xmlns=\"{XAML}\" {attrs}>{body}</Section>");
        let index = parse_source(&xml, &keys).unwrap();
        assert!(source_info(&xml, &index, false).is_err());
    }
}
