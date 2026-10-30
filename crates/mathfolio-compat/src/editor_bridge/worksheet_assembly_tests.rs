//! Frozen r125 assembly oracle, independent of the direct output/range writer.
use super::*;
fn legacy_worksheet_xml_with(
    regions: &[(u64, f64, f64, String)],
    mut label: impl FnMut(usize, u64, &str, &mut HashMap<String, String>) -> Result<String, String>,
    mut metadata: Option<&mut Vec<GeneratedRegion>>,
) -> Result<String, String> {
    let order = {
        #[cfg(test)]
        let _timing = worksheet_cache::profile::scope(worksheet_cache::profile::Phase::Order);
        worksheet_region_order(regions)?
    };
    let mut definitions = HashMap::new();
    // Globals participate before all normal regions. This only chooses a
    // missing label; the compatibility evaluator owns validity/conflict rules.
    {
        #[cfg(test)]
        let _timing = worksheet_cache::profile::scope(worksheet_cache::profile::Phase::Globals);
        for (_, _, _, xml) in regions {
            // XML local names cannot be written using character/entity escapes.
            // A top-level MathML globalDefine therefore requires these exact
            // bytes. False positives still take the full parser path; all other
            // fragments are validated by label_fragment below, as before.
            if !xml.contains("globalDefine") {
                continue;
            }
            let context = format!("<context xmlns:ml=\"{ML}\" xmlns=\"{WS}\">{xml}</context>");
            let document = roxmltree::Document::parse(&context).map_err(|e| e.to_string())?;
            for global in document
                .root_element()
                .children()
                .filter(|n| math(*n, "globalDefine"))
            {
                if let Some(target) = global.children().find(Node::is_element) {
                    let id = definition_identifier(target);
                    if let Some(id) = id {
                        let (name, _, _, subscript) = identifier_parts(id)?;
                        definitions.insert(
                            identifier_identity(&name, subscript.as_deref()),
                            declaration_label(id)?,
                        );
                    }
                }
            }
        }
    }
    let mut labelled = vec![String::new(); regions.len()];
    for index in order {
        labelled[index] = label(index, regions[index].0, &regions[index].3, &mut definitions)?;
    }
    #[cfg(test)]
    let _timing = worksheet_cache::profile::scope(worksheet_cache::profile::Phase::Assemble);
    let mut output = format!("<worksheet xmlns=\"{WS}\" xmlns:ml=\"{ML}\"><regions>");
    for ((id, left, top, _), math) in regions.iter().zip(labelled) {
        let start = output.len();
        let header = format!("<region region-id=\"{id}\" left=\"{left}\" top=\"{top}\">");
        output.push_str(&header);
        let math_start = output.len();
        if math.starts_with("<spec-table ") {
            output.push_str(&math);
        } else {
            use std::fmt::Write;
            write!(output, "<math resultRef=\"{id}\">").unwrap();
            output.push_str(&math);
            output.push_str("</math>");
        }
        if let Some(metadata) = metadata.as_mut() {
            // The generated header has only numeric values, so its quote
            // delimiters are unambiguous and independent of UTF-8 payloads.
            let left_start = header.find(" left=\"").unwrap() + 7;
            let left_end = left_start + header[left_start..].find('"').unwrap();
            let top_start = header.find(" top=\"").unwrap() + 6;
            let top_end = top_start + header[top_start..].find('"').unwrap();
            metadata.push(GeneratedRegion {
                id: *id,
                left: start + left_start..start + left_end,
                top: start + top_start..start + top_end,
                math: (!math.starts_with("<spec-table ")).then_some(math_start..output.len()),
            });
        }
        output.push_str("</region>");
        if output.len() > MAX_XML {
            return Err("Worksheet exceeds XML budget".into());
        }
    }
    output.push_str("</regions></worksheet>");
    Ok(output)
}

fn compare(regions: &[(u64, f64, f64, String)]) {
    let mut ranges = Vec::new();
    let old = legacy_worksheet_xml_with(
        regions,
        |_, _, xml, definitions| label_fragment(xml, definitions),
        Some(&mut ranges),
    );
    let mut cache = WorksheetXmlCache::default();
    let new = cache.build_prepared(regions);
    match (old, new) {
        (Ok(old), Ok(new)) => {
            assert_eq!(old, new.source);
            assert_eq!(ranges.len(), new.regions.len());
            for (a, b) in ranges.iter().zip(&new.regions) {
                assert_eq!(
                    (a.id, &a.left, &a.top, &a.math),
                    (b.id, &b.left, &b.top, &b.math)
                );
            }
        }
        (Err(a), Err(b)) => assert_eq!(a, b),
        _ => panic!("assembly acceptance differs"),
    }
}
#[test]
fn direct_assembly_preserves_frozen_r125_source_ranges_coordinates_and_errors() {
    let mut source=vec![(0,-0.,0.,"<ml:eval><ml:id>幅µ</ml:id><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>".into()),(u64::MAX,1e100,-1e-100,"<ml:globalDefine><ml:id>幅µ</ml:id><ml:real>3</ml:real></ml:globalDefine>".into()),(2,1.2345678901234567,3.,"<spec-table disable-calc='true'><math><ml:define><ml:id>table</ml:id><ml:real>1</ml:real></ml:define></math></spec-table>".into())];
    compare(&[]);
    compare(&source);
    source.reverse();
    compare(&source);
    source[0].1 = f64::NAN;
    compare(&source);
    source[0].1 = 0.;
    source.push(source[0].clone());
    compare(&source);
    for xml in [
        "<ml:eval>",
        "<ml:eval><ml:id>globalDefine</ml:id></ml:eval>",
        "<ml:globalDefine><ml:id bad='1'>x</ml:id><ml:real>3</ml:real></ml:globalDefine>",
    ] {
        compare(&[(1, 0., 0., xml.into())]);
    }
    let large = format!(
        "<ml:eval><ml:id>{}</ml:id><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>",
        "x".repeat(MAX_XML / 2)
    );
    compare(&[(1, 0., 0., large.clone()), (2, 0., 10., large)]);
}
#[test]
#[ignore = "Read-only six actual Enter captures versus independent r125 assembly"]
fn direct_assembly_captured_source_and_ranges_match_frozen_r125() {
    for (_, source, _) in worksheet_cache::typed_tests::captures() {
        compare(&source);
    }
}
