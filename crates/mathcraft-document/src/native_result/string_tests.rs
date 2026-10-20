use super::*;
#[test]
fn string_results_are_raw_inert_str_values_without_source_whitespace_or_visual_quotes() {
    for text in ["", "  a & <b> \"x\"  ", "한글\r\n\t𝛼😀"] {
        let value = Value::String { value: text.into() };
        let source = SavedNumericSource::EvaluatedValue(serde_json::to_string(&value).unwrap());
        let xml = numeric_result_xml(&source).unwrap().unwrap();
        let doc = roxmltree::Document::parse(&xml).unwrap();
        let node = doc
            .root_element()
            .children()
            .find(|n| n.is_element())
            .unwrap();
        assert!(node.has_tag_name((ML, "str")));
        assert_eq!(node.attributes().len(), 0);
        assert_eq!(
            node.children().filter_map(|n| n.text()).collect::<String>(),
            text
        );
    }
    for text in [
        "{\"kind\":\"string\",\"value\":\"a\",\"future\":1}",
        "{\"kind\":\"string\",\"value\":3}",
        "{\"kind\":\"string\",\"value\":\"\\u0000\"}",
    ] {
        assert!(numeric_result_xml(&SavedNumericSource::EvaluatedValue(text.into())).is_err());
    }
}
