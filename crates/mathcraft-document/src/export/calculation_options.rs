//! Source-preserving write of the positively recovered NUMERIC entry flag.
use super::*;
const CALC: &str = "http://schemas.ptc.com/mathcad/settings/calculation10";
const TYPE: &str = "application/vnd.openxmlformats-officedocument.mathprocessingml.mathcad.settings.calculation+xml";
const CANONICAL: &str = "mathcad/settings/calculation.xml";

pub(super) fn write(enabled: Option<bool>, package: &mut NativePackage) -> Result<(), String> {
    let Some(enabled) = enabled else {
        return Ok(());
    };
    let current = crate::native_calculation::bin_oct_hex_mode(package)?;
    if current == enabled {
        return Ok(());
    }
    let selected = crate::native_calculation::active_part(package)?;
    let path = selected.as_deref().unwrap_or(CANONICAL);
    if selected.is_none() && package.original_part(path).is_some() {
        return Err("Cannot replace an undeclared calculation settings part".into());
    }
    let new_part_types = if selected.is_none() {
        let types = package
            .original_xml_part("[Content_Types].xml")?
            .ok_or("Missing content types")?;
        let dom = parse(&types)?;
        let reserved = format!("/{path}");
        // A declaration can reserve the name even when its ZIP part is absent.
        // Do not create a second Override or replace another property's part.
        // Namespace mimics are not permission to reuse an ambiguous name.
        if dom.root_element().children().any(|node| {
            node.is_element()
                && node.tag_name().name() == "Override"
                && node.attributes().any(|attribute| {
                    attribute.name() == "PartName" && attribute.value() == reserved
                })
        }) {
            return Err("Cannot replace a reserved calculation settings part name".into());
        }
        Some(types)
    } else {
        None
    };
    let source = if selected.is_some() {
        package
            .original_xml_part(path)?
            .ok_or("Missing calculation settings")?
    } else {
        format!("<calculation xmlns=\"{CALC}\"/>")
    };
    let dom = parse(&source)?;
    let root = dom.root_element();
    let flag = if enabled { "true" } else { "false" };
    let behavior = root
        .children()
        .find(|n| n.has_tag_name((CALC, "calculationBehavior")));
    let changed = if let Some(behavior) = behavior {
        apply(
            source.clone(),
            attrs(
                behavior,
                &source,
                &[("bin-oct-hex-mode", Some(flag.into()))],
            )?,
        )?
    } else {
        let child = format!("<calculationBehavior xmlns=\"{CALC}\" bin-oct-hex-mode=\"{flag}\"/>");
        // calculation10 orders builtInVariables, calculationBehavior, then units.
        if let Some(units) = root.children().find(|n| n.has_tag_name((CALC, "units"))) {
            apply(
                source.clone(),
                vec![(units.range().start..units.range().start, child)],
            )?
        } else {
            insert_child(&source, &child)?
        }
    };
    set_xml(package, path, changed)?;
    if let Some(types) = new_part_types {
        let added =
            format!("<Override xmlns=\"{CONTENT}\" PartName=\"/{path}\" ContentType=\"{TYPE}\"/>");
        set_xml(
            package,
            "[Content_Types].xml",
            insert_child(&types, &added)?,
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn package() -> NativePackage {
        minimal_package(&CanvasDocument::default()).unwrap()
    }
    fn without_calculation_declaration(
        original: &NativePackage,
        keep_part: bool,
        replacement_declaration: &str,
    ) -> NativePackage {
        use std::io::Write;
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        for part in original.parts() {
            if part.name == CANONICAL && !keep_part {
                continue;
            }
            zip.start_file(&part.name, zip::write::SimpleFileOptions::default())
                .unwrap();
            if part.name == "[Content_Types].xml" {
                let types = original.original_xml_part(&part.name).unwrap().unwrap();
                let doc = parse(&types).unwrap();
                let declaration = doc
                    .root_element()
                    .children()
                    .find(|n| {
                        n.has_tag_name((CONTENT, "Override"))
                            && n.attribute("ContentType") == Some(TYPE)
                    })
                    .unwrap();
                let changed = apply(
                    types.clone(),
                    vec![(declaration.range(), replacement_declaration.into())],
                )
                .unwrap();
                zip.write_all(changed.as_bytes()).unwrap();
            } else {
                zip.write_all(part.bytes()).unwrap();
            }
        }
        NativePackage::from_bytes(&zip.finish().unwrap().into_inner()).unwrap()
    }
    fn parts_snapshot(package: &NativePackage) -> Vec<(String, Vec<u8>)> {
        package
            .parts()
            .iter()
            .map(|part| (part.name.clone(), part.bytes().to_vec()))
            .collect()
    }
    #[test]
    fn missing_zip_part_does_not_release_a_canonical_content_type_reservation() {
        for declaration in [
            format!("<Override xmlns='{CONTENT}' PartName='/{CANONICAL}' ContentType='other'/>"),
            format!(
                "<f:Override xmlns:f='urn:foreign' PartName='/{CANONICAL}' ContentType='other'/>"
            ),
        ] {
            let mut native = without_calculation_declaration(&package(), false, &declaration);
            assert!(native.original_part(CANONICAL).is_none());
            assert!(!crate::native_calculation::bin_oct_hex_mode(&native).unwrap());
            let before = parts_snapshot(&native);
            assert_eq!(
                write(Some(true), &mut native).unwrap_err(),
                "Cannot replace a reserved calculation settings part name"
            );
            assert_eq!(parts_snapshot(&native), before);
        }
    }
    #[test]
    fn undeclared_existing_calculation_part_is_rejected_without_mutation() {
        let mut native = without_calculation_declaration(&package(), true, "");
        assert!(native.original_part(CANONICAL).is_some());
        let before = parts_snapshot(&native);
        assert_eq!(
            write(Some(true), &mut native).unwrap_err(),
            "Cannot replace an undeclared calculation settings part"
        );
        assert_eq!(parts_snapshot(&native), before);
    }
    #[test]
    fn mode_override_retains_existing_program_signature_and_legacy_wire() {
        let mut doc = CanvasDocument::default();
        let json = serde_json::to_string(&doc).unwrap();
        assert!(!json.contains("bin_oct_hex_mode"));
        let signature = doc.calculation_signature();
        let mut native = package();
        let result = native.original_part("mathcad/result.xml").unwrap().to_vec();
        for enabled in [true, false, true] {
            doc.bin_oct_hex_mode = Some(enabled);
            assert_eq!(doc.calculation_signature(), signature);
            let own: CanvasDocument =
                serde_json::from_str(&serde_json::to_string(&doc).unwrap()).unwrap();
            assert_eq!(own.bin_oct_hex_mode, Some(enabled));
            export_into(&own, &mut native).unwrap();
            assert_eq!(
                crate::native_calculation::bin_oct_hex_mode(&native).unwrap(),
                enabled
            );
            assert_eq!(native.original_part("mathcad/result.xml").unwrap(), result);
        }
    }
    #[test]
    fn relocation_utf16_other_options_and_schema_order_survive_a_mode_patch() {
        let mut native = package();
        let types = native
            .original_xml_part("[Content_Types].xml")
            .unwrap()
            .unwrap()
            .replace("/mathcad/settings/calculation.xml", "/custom/calc.xml");
        set_xml(&mut native, "[Content_Types].xml", types).unwrap();
        let source = format!(
            "<?xml version='1.0' encoding='utf-16'?><c:calculation xmlns:c='{CALC}' cache-worksheet='true'><c:builtInVariables array-origin='1'/><c:units><c:currentUnitSystem name='SI'/></c:units></c:calculation>"
        );
        let bytes = [0xff, 0xfe]
            .into_iter()
            .chain(source.encode_utf16().flat_map(u16::to_le_bytes))
            .collect();
        native.set_part("custom/calc.xml", bytes).unwrap();
        let unselected = native.original_part(CANONICAL).unwrap().to_vec();
        write(Some(true), &mut native).unwrap();
        assert!(
            native
                .original_part("custom/calc.xml")
                .unwrap()
                .starts_with(&[0xff, 0xfe])
        );
        assert_eq!(native.original_part(CANONICAL).unwrap(), unselected);
        let changed = native
            .original_xml_part("custom/calc.xml")
            .unwrap()
            .unwrap();
        let dom = parse(&changed).unwrap();
        let root = dom.root_element();
        assert_eq!(root.attribute("cache-worksheet"), Some("true"));
        let elements: Vec<_> = root
            .children()
            .filter(Node::is_element)
            .map(|n| n.tag_name().name())
            .collect();
        assert_eq!(
            elements,
            ["builtInVariables", "calculationBehavior", "units"]
        );
        assert!(crate::native_calculation::bin_oct_hex_mode(&native).unwrap());
        assert!(!crate::native_calculation::strings_use_origin(&native).unwrap());
    }
}
