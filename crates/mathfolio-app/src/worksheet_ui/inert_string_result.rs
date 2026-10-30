//! Strict inert string carrier admission, independent of display-size policy.
//! This proves neither source freshness nor rendering support and never executes.
use mathfolio_document::canvas::SavedNumericSource;
use serde::Deserialize;
const ML: &str = "http://schemas.mathsoft.com/math50";
const XML: &str = "http://www.w3.org/XML/1998/namespace";
// Encoded escaping must fit the existing per-SavedNumericSource transport cap;
// decoded values retain the engine's stricter 1 MiB/XML-text policy.
const MAX_SOURCE_BYTES: usize = 4 * 1024 * 1024;
#[derive(Debug)]
pub(super) struct ValidatedString {
    _decoded_bytes: usize,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StringWire {
    kind: StringKind,
    value: String,
}
#[derive(Deserialize)]
enum StringKind {
    #[serde(rename = "string")]
    String,
}
pub(super) fn validate_source(source: &SavedNumericSource) -> Option<ValidatedString> {
    match source {
        SavedNumericSource::NativeMathml(xml) => validate_native(xml),
        SavedNumericSource::EvaluatedValue(json) => {
            if json.len() > MAX_SOURCE_BYTES {
                return None;
            }
            let wire: StringWire = serde_json::from_str(json).ok()?;
            let StringKind::String = wire.kind;
            mathfolio_compat::strings::validate(&wire.value).ok()?;
            Some(ValidatedString {
                _decoded_bytes: wire.value.len(),
            })
        }
        SavedNumericSource::AreaPresentation(_) => None,
    }
}
fn whitespace(node: roxmltree::Node<'_, '_>) -> bool {
    node.is_text()
        && node
            .text()
            .is_some_and(|s| s.chars().all(|c| matches!(c, ' ' | '\t' | '\r' | '\n')))
}
pub(super) fn validate_native(xml: &str) -> Option<ValidatedString> {
    if xml.len() > MAX_SOURCE_BYTES {
        return None;
    }
    let doc = roxmltree::Document::parse_with_options(
        xml,
        roxmltree::ParsingOptions {
            allow_dtd: false,
            nodes_limit: 128,
            ..Default::default()
        },
    )
    .ok()?;
    let root = doc.root_element();
    if !root.has_tag_name((ML, "result")) || root.attributes().len() != 0 {
        return None;
    }
    if doc.root().children().any(|n| n != root && !whitespace(n)) {
        return None;
    }
    let mut children = root.children().filter(|n| !whitespace(*n));
    let string = children.next()?;
    if children.next().is_some() || !string.has_tag_name((ML, "str")) {
        return None;
    }
    if string
        .attributes()
        .any(|a| a.namespace() != Some(XML) || a.name() != "space" || a.value() != "preserve")
        || string.children().any(|n| !n.is_text())
    {
        return None;
    }
    let mut bytes = 0usize;
    for part in string.children() {
        let value = part.text()?;
        mathfolio_compat::strings::validate(value).ok()?;
        bytes = bytes.checked_add(value.len())?;
        if bytes > mathfolio_compat::strings::MAX_STRING_BYTES {
            return None;
        }
    }
    Some(ValidatedString {
        _decoded_bytes: bytes,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    fn native(body: &str) -> String {
        format!("<ml:result xmlns:ml='{ML}'>{body}</ml:result>")
    }
    #[test]
    fn inert_string_keeps_exact_utf8_cr_empty_and_nonrenderable_values() {
        for text in ["".into(), "한글𝛼<&\"\r\n".into(), "a".repeat(70000)] {
            let source = SavedNumericSource::EvaluatedValue(
                serde_json::json!({"kind":"string","value":text}).to_string(),
            );
            assert_eq!(validate_source(&source).unwrap()._decoded_bytes, text.len());
            let xml = mathfolio_document::native_result::numeric_result_xml(&source)
                .unwrap()
                .unwrap();
            assert_eq!(validate_native(&xml).unwrap()._decoded_bytes, text.len());
            assert_eq!(
                validate_source(&SavedNumericSource::NativeMathml(xml))
                    .unwrap()
                    ._decoded_bytes,
                text.len()
            );
        }
    }
    #[test]
    fn string_json_authority_is_strict_and_never_numeric_or_executable() {
        for json in [
            r#"{"kind":"string","value":"x","future":0}"#,
            r#"{"kind":"string","kind":"string","value":"x"}"#,
            r#"{"kind":"string","value":"x","value":"y"}"#,
            r#"{"kind":"string","value":9}"#,
            r#"{"kind":"scalar","quantity":{"value":[1,0],"dimension":[0,0,0,0,0,0,0]}}"#,
            r#"{"kind":"symbolic","mathml":"<apply/>"}"#,
            r#"{"kind":"string","value":"\u0000"}"#,
        ] {
            assert!(
                validate_source(&SavedNumericSource::EvaluatedValue(json.into())).is_none(),
                "{json}"
            );
        }
        assert!(validate_source(&SavedNumericSource::AreaPresentation("{}".into())).is_none());
    }
    #[test]
    fn native_string_requires_exact_wrapper_namespace_metadata_and_literal() {
        for body in [
            "<ml:str future='x'>a</ml:str>",
            "<ml:str xml:space='default'>a</ml:str>",
            "<ml:str><ml:id>x</ml:id></ml:str>",
            "<ml:str>a<!--x-->b</ml:str>",
            "<ml:str>a<?future x?>b</ml:str>",
            "<ml:str>a</ml:str><ml:str>b</ml:str>",
            "<ml:real>1</ml:real>",
            "x<ml:str>a</ml:str>",
            "<str xmlns='urn:fake'>a</str>",
        ] {
            assert!(validate_native(&native(body)).is_none(), "{body}");
        }
        assert!(
            validate_native(&native("<ml:str xml:space='preserve'> a&#13;b </ml:str>")).is_some()
        );
        assert!(validate_native(&native("\n <ml:str/>\t")).is_some());
        assert!(validate_native(&format!("<ml:str xmlns:ml='{ML}'>a</ml:str>")).is_none());
        assert!(
            validate_native(&native("<ml:str>a</ml:str>").replacen(
                "<ml:result ",
                "<ml:result future='x' ",
                1
            ))
            .is_none()
        );
        assert!(
            validate_native(&format!(
                "<!DOCTYPE x [<!ENTITY x 'a'>]>{}",
                native("<ml:str>&x;</ml:str>")
            ))
            .is_none()
        );
    }
    #[test]
    fn decoded_and_encoded_budgets_do_not_depend_on_renderability() {
        let max = mathfolio_compat::strings::MAX_STRING_BYTES;
        for (len, valid) in [(max, true), (max + 1, false)] {
            let text = "x".repeat(len);
            let json = serde_json::json!({"kind":"string","value":text}).to_string();
            assert_eq!(
                validate_source(&SavedNumericSource::EvaluatedValue(json)).is_some(),
                valid
            );
            assert_eq!(
                validate_native(&native(&format!("<ml:str>{text}</ml:str>"))).is_some(),
                valid
            );
        }
        assert!(
            validate_source(&SavedNumericSource::EvaluatedValue(
                " ".repeat(MAX_SOURCE_BYTES + 1)
            ))
            .is_none()
        );
        assert!(validate_native(&" ".repeat(MAX_SOURCE_BYTES + 1)).is_none());
        let crowded = native(&"<ml:str/>".repeat(129));
        assert!(validate_native(&crowded).is_none());
    }
}
