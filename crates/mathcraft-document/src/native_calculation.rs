//! Read a bounded, explicitly typed package setting without executing source.
//! calculation10.xsd declares both authoring/index flags on calculationBehavior.
use crate::native::NativePackage;
const CONTENT: &str = "http://schemas.openxmlformats.org/package/2006/content-types";
const CALC: &str = "http://schemas.ptc.com/mathcad/settings/calculation10";
const TYPE: &str = "application/vnd.openxmlformats-officedocument.mathprocessingml.mathcad.settings.calculation+xml";
const MAX_BYTES: usize = 1024 * 1024;
const MAX_NODES: u32 = 16384;
fn xml_part(package: &NativePackage, path: &str) -> Result<String, String> {
    let bytes = package
        .original_part(path)
        .ok_or_else(|| format!("Missing package part: {path}"))?;
    // UTF-16 is decoded by the existing package reader; bound both representations.
    if bytes.len() > MAX_BYTES * 2 + 2 {
        return Err("Calculation settings XML exceeds byte budget".into());
    }
    let source = package.original_xml_part(path)?.ok_or("Missing XML part")?;
    if source.len() > MAX_BYTES {
        return Err("Calculation settings XML exceeds text budget".into());
    }
    Ok(source)
}
fn parse(source: &str) -> Result<roxmltree::Document<'_>, String> {
    roxmltree::Document::parse_with_options(
        source,
        roxmltree::ParsingOptions {
            allow_dtd: false,
            nodes_limit: MAX_NODES,
            ..Default::default()
        },
    )
    .map_err(|e| e.to_string())
}
/// Locate the unique calculation content-type Override, not a fixed filename.
/// No declared calculation part means the schema default false. A declared but
/// missing, ambiguous or malformed active part is an error, never that default.
pub fn strings_use_origin(package: &NativePackage) -> Result<bool, String> {
    boolean_setting(package, "strings-use-origin")
}

/// NUMERIC descriptor authoring mode; this never reinterprets an existing AST.
pub fn bin_oct_hex_mode(package: &NativePackage) -> Result<bool, String> {
    boolean_setting(package, "bin-oct-hex-mode")
}

pub(crate) fn active_part(package: &NativePackage) -> Result<Option<String>, String> {
    let types = xml_part(package, "[Content_Types].xml")?;
    let doc = parse(&types)?;
    let root = doc.root_element();
    if !root.has_tag_name((CONTENT, "Types")) {
        return Err("Invalid content types root".into());
    }
    let mut selected = None;
    for part in root.children().filter(|n| n.is_element()) {
        // A namespace mimic must not silently hide the setting or acquire false.
        if part.tag_name().name() == "Override"
            && part
                .attributes()
                .any(|a| a.name() == "ContentType" && a.value() == TYPE)
        {
            if !part.has_tag_name((CONTENT, "Override"))
                || part.attributes().any(|a| {
                    matches!(a.name(), "ContentType" | "PartName") && a.namespace().is_some()
                })
            {
                return Err("Unsupported namespaced calculation Override".into());
            }
            if selected.replace(part).is_some() {
                return Err("Ambiguous calculation settings parts".into());
            }
        }
    }
    let Some(part) = selected else {
        return Ok(None);
    };
    let path = part
        .attribute("PartName")
        .ok_or("Missing calculation settings part name")?;
    let path = path
        .strip_prefix('/')
        .ok_or("Calculation settings part name must be absolute")?;
    if path.is_empty()
        || path
            .split('/')
            .any(|p| p.is_empty() || p == "." || p == "..")
        || path.contains(['\\', '%', '?', '#'])
    {
        return Err("Unsupported calculation settings part name".into());
    }
    if root
        .children()
        .filter(|n| {
            n.has_tag_name((CONTENT, "Override"))
                && n.attribute("PartName") == part.attribute("PartName")
        })
        .count()
        != 1
    {
        return Err("Ambiguous calculation settings part declaration".into());
    }
    Ok(Some(path.to_owned()))
}

fn boolean_setting(package: &NativePackage, name: &str) -> Result<bool, String> {
    let Some(path) = active_part(package)? else {
        return Ok(false);
    };
    let xml = xml_part(package, &path)?;
    let settings = parse(&xml)?;
    let root = settings.root_element();
    if !root.has_tag_name((CALC, "calculation")) {
        return Err("Unsupported calculation settings namespace/root".into());
    }
    // The schema places this option on a direct calculationBehavior child,
    // never on calculation itself. Older root-attribute fixtures were wrong.
    if root.attributes().any(|a| a.name() == name) {
        return Err(format!("Misplaced root {name} setting"));
    }
    let mut behavior = None;
    for child in root.descendants().skip(1).filter(|n| n.is_element()) {
        if child.tag_name().name() == "calculationBehavior" {
            if child.parent() != Some(root) || !child.has_tag_name((CALC, "calculationBehavior")) {
                return Err("Unsupported nested/namespaced calculationBehavior".into());
            }
            if behavior.replace(child).is_some() {
                return Err("Duplicate calculationBehavior".into());
            }
        }
    }
    for child in root.descendants().skip(1).filter(|n| n.is_element()) {
        if child.tag_name().name() == name
            || (Some(child) != behavior && child.attributes().any(|a| a.name() == name))
        {
            return Err(format!("Misplaced {name} setting"));
        }
    }
    let Some(behavior) = behavior else {
        return Ok(false);
    };
    if behavior.children().any(|n| n.is_element())
        || behavior
            .attributes()
            .any(|a| a.name() == name && a.namespace().is_some())
    {
        return Err("Unsupported calculationBehavior setting content/namespace".into());
    }
    // xs:boolean whitespace facet is collapse (XML whitespace only).
    match behavior
        .attribute(name)
        .map(|s| s.trim_matches([' ', '\t', '\r', '\n']))
    {
        None | Some("false" | "0") => Ok(false),
        Some("true" | "1") => Ok(true),
        _ => Err(format!("Invalid {name} boolean")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    fn package(types: &str, parts: &[(&str, Vec<u8>)]) -> NativePackage {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let mut all = vec![("mathcad/worksheet.xml", b"<worksheet xmlns='http://schemas.mathsoft.com/worksheet50'><regions/></worksheet>".to_vec()), ("[Content_Types].xml", types.as_bytes().to_vec())];
        all.extend_from_slice(parts);
        for (name, data) in all {
            zip.start_file(name, zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(&data).unwrap();
        }
        NativePackage::from_bytes(&zip.finish().unwrap().into_inner()).unwrap()
    }
    fn types(body: &str) -> String {
        format!("<Types xmlns='{CONTENT}'>{body}</Types>")
    }
    fn declaration(path: &str) -> String {
        format!("<Override PartName='/{path}' ContentType='{TYPE}'/>")
    }
    fn settings(attr: &str) -> Vec<u8> {
        format!("<calculation xmlns='{CALC}'><calculationBehavior {attr}/></calculation>")
            .into_bytes()
    }
    #[test]
    fn radix_input_mode_is_a_separate_direct_child_boolean() {
        for (flag, expected) in [
            ("", false),
            ("bin-oct-hex-mode='true'", true),
            ("bin-oct-hex-mode='0'", false),
        ] {
            let attrs = format!("strings-use-origin='true' {flag}");
            let p = package(
                &types(&declaration("custom/settings.xml")),
                &[("custom/settings.xml", settings(&attrs))],
            );
            assert_eq!(bin_oct_hex_mode(&p).unwrap(), expected);
            assert!(strings_use_origin(&p).unwrap());
        }
        for xml in [
            format!("<calculation xmlns='{CALC}' bin-oct-hex-mode='true'/>"),
            format!(
                "<calculation xmlns='{CALC}'><calculationBehavior bin-oct-hex-mode='TRUE'/></calculation>"
            ),
            format!(
                "<calculation xmlns='{CALC}'><calculationBehavior xmlns:f='urn:fake' f:bin-oct-hex-mode='true'/></calculation>"
            ),
        ] {
            let p = package(
                &types(&declaration("custom/settings.xml")),
                &[("custom/settings.xml", xml.into_bytes())],
            );
            assert!(bin_oct_hex_mode(&p).is_err());
        }
    }
    #[test]
    fn schema_default_and_all_boolean_lexemes() {
        assert!(!strings_use_origin(&package(&types(""), &[])).unwrap());
        assert!(
            !strings_use_origin(&package(
                &types(&declaration("custom/calc.xml")),
                &[(
                    "custom/calc.xml",
                    format!("<calculation xmlns='{CALC}'/>").into_bytes()
                )]
            ))
            .unwrap()
        );
        for (attr, expected) in [
            ("", false),
            ("strings-use-origin='true'", true),
            ("strings-use-origin='1'", true),
            ("strings-use-origin='false'", false),
            ("strings-use-origin='0'", false),
            ("strings-use-origin=' &#x9;1&#xD; '", true),
        ] {
            let p = package(
                &types(&declaration("custom/calc.xml")),
                &[("custom/calc.xml", settings(attr))],
            );
            assert_eq!(strings_use_origin(&p).unwrap(), expected);
        }
    }
    #[test]
    fn relocated_utf16_active_part_wins_over_unselected_canonical_file() {
        let xml = format!(
            "<?xml version='1.0' encoding='utf-16'?><calculation xmlns='{CALC}'><calculationBehavior strings-use-origin='true'/></calculation>"
        );
        let utf16 = [0xff, 0xfe]
            .into_iter()
            .chain(xml.encode_utf16().flat_map(u16::to_le_bytes))
            .collect();
        let p = package(
            &types(&declaration("custom/settings.xml")),
            &[
                ("custom/settings.xml", utf16),
                (
                    "mathcad/calculation.xml",
                    settings("strings-use-origin='false'"),
                ),
            ],
        );
        assert!(strings_use_origin(&p).unwrap());
    }
    #[test]
    fn declared_missing_ambiguous_invalid_and_mimicked_settings_fail() {
        assert!(strings_use_origin(&package(&types(&declaration("missing.xml")), &[])).is_err());
        assert!(
            strings_use_origin(&package(
                &types(&(declaration("a.xml") + &declaration("b.xml"))),
                &[]
            ))
            .is_err()
        );
        for xml in [settings("strings-use-origin='TRUE'"), settings("strings-use-origin='&#xA0;true'"), settings("xmlns:x='urn:fake' x:strings-use-origin='true'"), format!("<calculation xmlns='{CALC}'><strings-use-origin>true</strings-use-origin></calculation>").into_bytes(), b"<calculation xmlns='urn:fake' strings-use-origin='true'/>".to_vec()] {
            assert!(strings_use_origin(&package(&types(&declaration("a.xml")), &[("a.xml", xml)])).is_err());
        }
        for body in [
            format!("<x:Override xmlns:x='urn:fake' PartName='/a.xml' ContentType='{TYPE}'/>"),
            format!("<Override xmlns:x='urn:fake' PartName='/a.xml' x:ContentType='{TYPE}'/>"),
            declaration("a.xml") + "<Override PartName='/a.xml' ContentType='other'/>",
        ] {
            assert!(
                strings_use_origin(&package(&types(&body), &[("a.xml", settings(""))])).is_err()
            );
        }
    }
    #[test]
    fn xml_node_and_byte_budgets_reject_before_defaulting() {
        assert!(parse("<!DOCTYPE calculation [<!ENTITY x 'true'>]><calculation/>").is_err());
        let crowded = format!(
            "<calculation xmlns='{CALC}'>{}</calculation>",
            "<unknown/>".repeat(MAX_NODES as usize)
        );
        let oversized = format!(
            "<calculation xmlns='{CALC}'><!--{}--></calculation>",
            "a".repeat(MAX_BYTES)
        );
        for xml in [crowded, oversized] {
            assert!(
                strings_use_origin(&package(
                    &types(&declaration("a.xml")),
                    &[("a.xml", xml.into_bytes())]
                ))
                .is_err()
            );
        }
    }
    #[test]
    fn root_attribute_old_fixture_and_duplicate_nested_mimic_are_negative() {
        for body in [
            format!("<calculation xmlns='{CALC}' strings-use-origin='true'/>"),
            format!(
                "<calculation xmlns='{CALC}'><calculationBehavior/><calculationBehavior strings-use-origin='true'/></calculation>"
            ),
            format!(
                "<calculation xmlns='{CALC}'><unknown><calculationBehavior strings-use-origin='true'/></unknown></calculation>"
            ),
            format!(
                "<calculation xmlns='{CALC}'><calculationBehavior xmlns='urn:fake' strings-use-origin='true'/></calculation>"
            ),
        ] {
            assert!(
                strings_use_origin(&package(
                    &types(&declaration("a.xml")),
                    &[("a.xml", body.into_bytes())]
                ))
                .is_err()
            );
        }
    }
}
