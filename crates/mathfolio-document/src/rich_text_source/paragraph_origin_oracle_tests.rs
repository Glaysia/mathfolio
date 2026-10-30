//! Independent v2 reparse-fold oracle. Test-only; no v3 cached span/fold helper.
//! Register from paragraph_origin.rs with
//! #[cfg(test)] #[path="paragraph_origin_oracle_tests.rs"] mod oracle_tests;
use super::*;
use zip::write::SimpleFileOptions;

fn old_spans(xml: &str) -> Result<Vec<(Range<usize>, usize, usize)>> {
    let doc = parse_xml(xml)?;
    doc.root_element()
        .children()
        .filter(Node::is_element)
        .map(|p| {
            if !p.has_tag_name((XAML, "Paragraph")) {
                return Err("oracle paragraph shape".into());
            }
            let range = p.range();
            let open = tag_end(xml.as_bytes(), range.start)?;
            let close = range.start
                + xml[range.clone()]
                    .rfind("</")
                    .ok_or("oracle paragraph close")?;
            Ok((range, open, close))
        })
        .collect()
}

fn old_outside(xml: &str, index: &SourceIndex) -> String {
    let mut out = String::new();
    let mut cursor = 0;
    for run in &index.runs {
        out.push_str(&xml[cursor..run.range.start]);
        cursor = run.range.end;
    }
    out.push_str(&xml[cursor..]);
    out
}

/// Independent body-folding implementation retained from v2 semantics.
fn v2_oracle(source: &ValidatedRichText) -> Result<()> {
    let origins = source
        .wire
        .paragraph_origins
        .as_deref()
        .ok_or("oracle missing map")?;
    let old = old_spans(&source.authority.original_xaml)?;
    let current = old_spans(source.xaml())?;
    if old.is_empty()
        || old.len() > 1024
        || current.len() > 1024
        || current.len() <= old.len()
        || origins.len() != current.len()
        || origins[0] != 0
        || usize::from(*origins.last().unwrap()) != old.len() - 1
        || origins
            .windows(2)
            .any(|w| w[1] < w[0] || u32::from(w[1]) > u32::from(w[0]) + 1)
    {
        return Err("oracle invalid map".into());
    }
    let original = &source.authority.original_xaml;
    let mut counts = vec![0usize; old.len()];
    for origin in origins {
        *counts
            .get_mut(usize::from(*origin))
            .ok_or("oracle origin")? += 1;
    }
    if counts.contains(&0) {
        return Err("oracle uncovered".into());
    }
    if source.xaml()[..current[0].0.start] != original[..old[0].0.start]
        || source.xaml()[current.last().unwrap().0.end..] != original[old.last().unwrap().0.end..]
    {
        return Err("oracle root changed".into());
    }
    for (i, (range, open, close)) in current.iter().enumerate() {
        let origin = usize::from(origins[i]);
        let (old_range, old_open, old_close) = &old[origin];
        if source.xaml()[range.start..*open] != original[old_range.start..*old_open]
            || source.xaml()[*close..range.end] != original[*old_close..old_range.end]
            || (counts[origin] > 1 && source.index.paragraphs[i].is_empty())
        {
            return Err("oracle wrapper/empty changed".into());
        }
        if i == 0 {
            continue;
        }
        let gap = &source.xaml()[current[i - 1].0.end..range.start];
        if origins[i - 1] == origins[i] {
            if !gap.is_empty() {
                return Err("oracle clone gap".into());
            }
            let left = source
                .index
                .runs
                .iter()
                .rfind(|r| r.paragraph == i - 1)
                .ok_or("oracle left run")?;
            let right = source
                .index
                .runs
                .iter()
                .find(|r| r.paragraph == i)
                .ok_or("oracle right run")?;
            if non_format_opening(&source.xaml()[left.range.start..left.opening_end])?
                != non_format_opening(&source.xaml()[right.range.start..right.opening_end])?
                || source.xaml()[left.closing_start..left.range.end]
                    != source.xaml()[right.closing_start..right.range.end]
            {
                return Err("oracle boundary owner".into());
            }
        } else if gap != &original[old[origin - 1].0.end..old[origin].0.start] {
            return Err("oracle original gap".into());
        }
    }
    let mut folded = String::new();
    folded.push_str(&original[..old[0].0.start]);
    let mut cursor = 0;
    for (origin, count) in counts.into_iter().enumerate() {
        let (range, open, close) = &old[origin];
        folded.push_str(&original[range.start..*open]);
        for (_, open, close) in &current[cursor..cursor + count] {
            folded.push_str(&source.xaml()[*open..*close]);
        }
        folded.push_str(&original[*close..range.end]);
        cursor += count;
        if origin + 1 < old.len() {
            folded.push_str(&original[range.end..old[origin + 1].0.start]);
        }
    }
    folded.push_str(&original[old.last().unwrap().0.end..]);
    let reparsed = parse_source(&folded, &source.authority.keys)?;
    if old_outside(&folded, &reparsed) != old_outside(original, &source.authority.original_index)
        || metadata_groups(&folded, &reparsed)?
            != metadata_groups(original, &source.authority.original_index)?
    {
        return Err("oracle folded proof".into());
    }
    Ok(())
}

const CATALOG: &[u8] =
    include_bytes!("../../../../tests/reference/rich-text/lcc-standard-presentation.xml");
fn source(xml: &str) -> ValidatedRichText {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    zip.start_file(INNER, SimpleFileOptions::default()).unwrap();
    zip.write_all(xml.as_bytes()).unwrap();
    let identity = NativeTextIdentity {
        owner_region_id: "17".into(),
        item_idref: "FlowDocument0".into(),
        package_part: "mathcad/xaml/FlowDocument0.XamlPackage".into(),
        catalog_part: "mathcad/settings/presentation.xml".into(),
        owner_xml_sha256: [2; 32],
    };
    let authority = OriginalTextAuthority::from_package(
        identity,
        zip.finish().unwrap().into_inner(),
        CATALOG.to_vec(),
    )
    .unwrap();
    let plain = authority.original_plain_text().to_owned();
    ValidatedRichText::from_original(authority, &plain).unwrap()
}
fn compare(source: &ValidatedRichText) {
    assert_eq!(
        prove_mapped_metadata(source).is_ok(),
        v2_oracle(source).is_ok(),
        "{}",
        source.xaml()
    );
}
fn forged(source: &ValidatedRichText, xml: String) -> Option<ValidatedRichText> {
    let index = Arc::new(parse_source(&xml, &source.authority.keys).ok()?);
    let mut wire = source.wire.as_ref().clone();
    wire.current_xaml = xml.into();
    wire.visual_fingerprint = std::sync::OnceLock::new();
    // Deliberately bypass from_prepared's proof so both paths independently
    // inspect the same parse-admitted malicious source.
    Some(ValidatedRichText {
        wire: Arc::new(wire),
        authority: source.authority.clone(),
        index,
    })
}

#[test]
fn fast_proof_matches_independent_reparse_for_entities_qnames_and_repeated_origins() {
    for prefix in [false, true] {
        let (q, ns) = if prefix {
            ("p:", format!(" xmlns:p=\"{XAML}\""))
        } else {
            ("", String::new())
        };
        for body in ["A&amp;한😀BC", "ABCD"] {
            let xml = format!(
                "<Section xmlns=\"{XAML}\"{ns}><{q}Paragraph><{q}Run style=\"Normal\">{body}</{q}Run><{q}Run style=\"Normal\" Background=\"#FFFF0000\">EFGH</{q}Run></{q}Paragraph><{q}Paragraph><{q}Run style=\"Normal\">IJKL</{q}Run></{q}Paragraph></Section>"
            );
            let first = source(&xml)
                .split_paragraph_at_caret(TextSelection {
                    paragraph: 0,
                    anchor: 1,
                    focus: 1,
                })
                .unwrap()
                .patch
                .source;
            compare(&first);
            let second = first
                .split_paragraph_at_caret(TextSelection {
                    paragraph: 1,
                    anchor: 1,
                    focus: 1,
                })
                .unwrap()
                .patch
                .source;
            compare(&second);
            let third = second
                .split_paragraph_at_caret(TextSelection {
                    paragraph: 3,
                    anchor: 1,
                    focus: 1,
                })
                .unwrap()
                .patch
                .source;
            compare(&third);
        }
    }
}

#[test]
fn fast_proof_matches_reparse_for_forged_metadata_and_cdata_stays_outside_admission() {
    let xml = format!(
        "<Section xmlns=\"{XAML}\"><Paragraph><Run style=\"Normal\">ABCD</Run><Run style=\"Normal\">EFGH</Run></Paragraph></Section>"
    );
    let source = source(&xml)
        .split_paragraph_at_caret(TextSelection {
            paragraph: 0,
            anchor: 1,
            focus: 1,
        })
        .unwrap()
        .patch
        .source;
    compare(&source);
    // The existing parser rejects CDATA before either metadata proof. Do not
    // broaden that grammar merely to make a proposed differential case pass.
    for syntax in ["<![CDATA[]]>", "<![CDATA[]]><![CDATA[]]>"] {
        let xml = source
            .xaml()
            .replacen("</Run>", &format!("</Run>{syntax}"), 1);
        assert!(parse_source(&xml, &source.authority.keys).is_err());
        let mut wire = source.wire.as_ref().clone();
        wire.current_xaml = xml.into();
        assert!(
            ValidatedRichText::from_wire(wire, source.authority.clone(), source.plain_text())
                .is_err()
        );
    }
    for xml in [
        source
            .xaml()
            .replacen("<Paragraph>", "<Paragraph FontSize=\"20\">", 1),
        source
            .xaml()
            .replacen("style=\"Normal\"", "style=\"Normal\" xml:lang=\"en-US\"", 1),
    ] {
        let candidate = forged(&source, xml).expect("forged fixture remains parser-admitted");
        compare(&candidate);
        assert!(v2_oracle(&candidate).is_err());
    }
}
