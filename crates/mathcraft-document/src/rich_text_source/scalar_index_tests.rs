//! Candidate tests. No timing threshold and no production parser instrumentation.
use super::*;

const ONE: &str = include_str!("../../../../tests/reference/rich-text/sample-01.xaml");
const TWO: &str = include_str!("../../../../tests/reference/rich-text/sample-02.xaml");
const CATALOG: &[u8] =
    include_bytes!("../../../../tests/reference/rich-text/lcc-standard-presentation.xml");
const PACKAGE: &[u8] =
    include_bytes!("../../../../tests/reference/rich-text/sample-01.XamlPackage");

fn keys() -> HashSet<String> {
    HashSet::from(["Normal".into(), "Normal~639058311443748180".into()])
}

/// Separate XML text oracle: roxmltree decodes entities, then Rust char_indices
/// enumerates the exact plain projection. It does not read cached boundaries,
/// scalar_start, paragraph_starts or any new cumulative production counter.
fn assert_plain_range_oracle(xml: &str, actual: &SourceIndex) {
    let document = Document::parse(xml).unwrap();
    let mut full_plain = String::new();
    let mut run_ordinal = 0;
    for (paragraph_ordinal, paragraph) in document
        .root_element()
        .children()
        .filter(Node::is_element)
        .enumerate()
    {
        if paragraph_ordinal != 0 {
            full_plain.push('\n');
        }
        let global_start = full_plain.char_indices().count();
        assert_eq!(actual.paragraph_starts[paragraph_ordinal], global_start);
        let mut paragraph_text = String::new();
        for run in paragraph.children().filter(Node::is_element) {
            let text: String = run.children().filter_map(|node| node.text()).collect();
            let prefix = paragraph_text.char_indices().count();
            let indexed = &actual.runs[run_ordinal];
            assert_eq!(indexed.paragraph, paragraph_ordinal);
            assert_eq!(indexed.scalar_start, prefix);
            assert_eq!(indexed.boundaries.len(), text.char_indices().count() + 1);
            paragraph_text.push_str(&text);
            run_ordinal += 1;
        }
        assert_eq!(actual.paragraphs[paragraph_ordinal], paragraph_text);
        full_plain.push_str(&paragraph_text);
    }
    assert_eq!(run_ordinal, actual.runs.len());
    assert_eq!(actual.plain, full_plain);
}

fn assert_all_metadata_equal(left: &SourceIndex, right: &SourceIndex) {
    assert_eq!(left.plain, right.plain);
    assert_eq!(left.paragraphs, right.paragraphs);
    assert_eq!(left.paragraph_starts, right.paragraph_starts);
    assert_eq!(left.paragraph_xml.len(), right.paragraph_xml.len());
    for (a, b) in left.paragraph_xml.iter().zip(&right.paragraph_xml) {
        assert_eq!(a.range, b.range);
        assert_eq!(a.opening_end, b.opening_end);
        assert_eq!(a.closing_start, b.closing_start);
    }
    assert_eq!(left.runs.len(), right.runs.len());
    for (a, b) in left.runs.iter().zip(&right.runs) {
        assert_eq!(a.range, b.range);
        assert_eq!(a.opening_end, b.opening_end);
        assert_eq!(a.closing_start, b.closing_start);
        assert_eq!(a.paragraph, b.paragraph);
        assert_eq!(a.scalar_start, b.scalar_start);
        assert_eq!(a.boundaries, b.boundaries);
        assert_eq!(a.effective, b.effective);
    }
}

fn xml(paragraphs: &str) -> String {
    format!(
        "<FlowDocument xmlns=\"{XAML}\" xml:space=\"preserve\" FontSize=\"16\" FontWeight=\"Normal\" FontStyle=\"Normal\" Foreground=\"#FF000000\">{paragraphs}</FlowDocument>"
    )
}

#[test]
fn scalar_offsets_are_neither_utf8_bytes_nor_utf16_units_and_empty_runs_do_not_advance() {
    let input = xml(concat!(
        "<Paragraph><Run style=\"Normal\">A한&#x1F600;&amp;</Run>",
        "<Run style=\"Normal\"></Run><Run style=\"Normal\">e&#769;</Run></Paragraph>",
        "<Paragraph><Run style=\"Normal\">&lt;😀</Run><Run style=\"Normal\">Z</Run></Paragraph>",
        "<Paragraph><Run style=\"Normal\"></Run></Paragraph>"
    ));
    let actual = parse_source(&input, &keys()).unwrap();
    assert_eq!(actual.plain, "A한😀&e\u{301}\n<😀Z\n");
    assert_eq!(actual.paragraph_starts, [0, 7, 11]);
    assert_eq!(
        actual
            .runs
            .iter()
            .map(|run| run.scalar_start)
            .collect::<Vec<_>>(),
        [0, 4, 4, 0, 2, 0]
    );
    assert_eq!("A한😀&".chars().count(), 4);
    assert_eq!("A한😀&".encode_utf16().count(), 5);
    assert_eq!("A한😀&".len(), 9);
    assert_eq!(actual.runs[0].boundaries, [0, 1, 4, 13, 18]);
    assert_eq!(actual.runs[1].boundaries, [0]);
    assert_eq!(actual.runs[2].boundaries, [0, 1, 7]);
    assert_plain_range_oracle(&input, &actual);
    assert_all_metadata_equal(&actual, &legacy_parse_source(&input, &keys()).unwrap());
}

#[test]
fn actual_native_xaml_and_generated_many_empty_entity_runs_match_independent_projection() {
    for fixture in [ONE, TWO] {
        let actual = parse_source(fixture, &keys()).unwrap();
        assert_plain_range_oracle(fixture, &actual);
        assert_all_metadata_equal(&actual, &legacy_parse_source(fixture, &keys()).unwrap());
    }
    let mut paragraphs = String::new();
    for p in 0..7 {
        paragraphs.push_str("<Paragraph>");
        for r in 0..17 {
            let body = match (p + r) % 4 {
                0 => "",
                1 => "&#x1F600;&amp;",
                2 => "가e&#769;",
                _ => "&lt;&quot;&apos;",
            };
            paragraphs.push_str(&format!("<Run style=\"Normal\">{body}</Run>"));
        }
        paragraphs.push_str("</Paragraph>");
    }
    let input = xml(&paragraphs);
    let actual = parse_source(&input, &keys()).unwrap();
    assert_plain_range_oracle(&input, &actual);
    assert_all_metadata_equal(&actual, &legacy_parse_source(&input, &keys()).unwrap());
}

#[test]
fn source_admission_errors_and_caps_remain_identical_to_pinned_parser() {
    for input in [
        xml("<Paragraph><Run style=\"Normal\">a\nb</Run></Paragraph>"),
        xml("<Paragraph><Run style=\"Missing\">a</Run></Paragraph>"),
        xml("<Paragraph><Run style=\"Normal\"/></Paragraph>"),
        xml("<Paragraph></Paragraph>"),
        xml("<Paragraph><Run style=\"Normal\">&#0;</Run></Paragraph>"),
        xml(&format!(
            "<Paragraph>{}</Paragraph>",
            "<Run style=\"Normal\"></Run>".repeat(MAX_RUNS + 1)
        )),
        "x".repeat(MAX_SOURCE_BYTES + 1),
    ] {
        assert_eq!(
            parse_source(&input, &keys()).unwrap_err(),
            legacy_parse_source(&input, &keys()).unwrap_err()
        );
    }
}

#[test]
fn native_wire_noop_and_entity_edit_own_reopen_keep_exact_source_authority() {
    let identity = NativeTextIdentity {
        owner_region_id: "17".into(),
        item_idref: "FlowDocument0".into(),
        package_part: "mathcad/xaml/FlowDocument0.XamlPackage".into(),
        catalog_part: "mathcad/settings/presentation.xml".into(),
        owner_xml_sha256: hash(b"scalar-index test original wrapper"),
    };
    let authority =
        OriginalTextAuthority::from_package(identity, PACKAGE.to_vec(), CATALOG.to_vec()).unwrap();
    let original_package = authority.original_package().to_vec();
    let source = ValidatedRichText::from_original(authority.clone(), "LCCL-S 시스템 설계").unwrap();
    let wire_before = serde_json::to_vec(source.wire()).unwrap();
    let reopened = ValidatedRichText::from_wire(
        source.wire().clone(),
        authority.clone(),
        source.plain_text(),
    )
    .unwrap();
    assert_eq!(serde_json::to_vec(reopened.wire()).unwrap(), wire_before);
    assert_eq!(reopened.xaml(), ONE);
    let ranges: Vec<_> = reopened.run_char_ranges().collect();
    let last = ranges.last().unwrap();
    let run = &reopened.index.runs[last.run_ordinal];
    let length = run.boundaries.len() - 1;
    let patch = reopened
        .replace_in_run(last.run_ordinal, length, length, "한😀&")
        .unwrap()
        .unwrap();
    assert!(patch.source.xaml().contains("한😀&amp;</Run>"));
    assert_eq!(patch.selection.focus, last.end_in_paragraph + 3);
    assert_eq!(source.authority.original_package(), original_package);
    let wire = patch.source.wire().clone();
    let restored =
        ValidatedRichText::from_wire(wire, authority, patch.source.plain_text()).unwrap();
    assert_eq!(restored.xaml(), patch.source.xaml());
    assert_eq!(
        serde_json::to_vec(restored.wire()).unwrap(),
        serde_json::to_vec(patch.source.wire()).unwrap()
    );
}

#[test]
#[ignore = "manual bounded CPU observation; no timing pass/fail threshold"]
fn profile_scalar_index_4096_runs_and_native_fixture() {
    let body = format!(
        "<Paragraph>{}</Paragraph>",
        "<Run style=\"Normal\">가😀&amp;e&#769;</Run>".repeat(MAX_RUNS)
    );
    let generated = xml(&body);
    for (name, input) in [
        ("generated4096", generated.as_str()),
        ("native-sample01", ONE),
    ] {
        let expected = legacy_parse_source(input, &keys()).unwrap();
        assert_all_metadata_equal(&parse_source(input, &keys()).unwrap(), &expected);
        for (label, parser) in [
            (
                "legacy",
                legacy_parse_source as fn(&str, &HashSet<String>) -> Result<SourceIndex>,
            ),
            ("candidate", parse_source),
        ] {
            let started = std::time::Instant::now();
            for _ in 0..8 {
                std::hint::black_box(parser(std::hint::black_box(input), &keys()).unwrap());
            }
            eprintln!(
                "SCALAR_INDEX_CPU workload={name} parser={label} iterations=8 elapsed_ns={}",
                started.elapsed().as_nanos()
            );
        }
    }
}

// The exact pinned pre-change parse_source body is appended below. It remains a
// test-only legacy oracle/profile reference; no alternate production parser.

fn legacy_parse_source(source: &str, keys: &HashSet<String>) -> Result<SourceIndex> {
    let doc = parse_xml(source)?;
    let root = doc.root_element();
    if root.tag_name().namespace() != Some(XAML)
        || !matches!(root.tag_name().name(), "Section" | "FlowDocument")
    {
        return Err("Unsupported XAML root".into());
    }
    validate_attributes(root, false)?;
    let mut runs = Vec::new();
    let mut paragraphs = Vec::new();
    let mut paragraph_xml = Vec::new();
    for paragraph in root.children() {
        if paragraph.is_text() && paragraph.text().unwrap_or("").is_empty() {
            continue;
        }
        if !paragraph.has_tag_name((XAML, "Paragraph")) {
            return Err("Only direct Paragraph is editable".into());
        }
        if paragraphs.len() >= 1024 {
            return Err("Paragraph budget exceeded".into());
        }
        validate_attributes(paragraph, false)?;
        let paragraph_index = paragraphs.len();
        let mut text = String::new();
        let mut count = 0;
        for node in paragraph.children() {
            if node.is_text() && node.text().unwrap_or("").is_empty() {
                continue;
            }
            if !node.has_tag_name((XAML, "Run")) {
                return Err("Only direct Run is editable".into());
            }
            validate_attributes(node, true)?;
            if let Some(style) = node.attribute("style")
                && !keys.contains(style)
            {
                return Err("Unknown Run style key".into());
            }
            if node.children().any(|child| !child.is_text()) {
                return Err("Nested Run content unsupported".into());
            }
            if runs.len() >= MAX_RUNS {
                return Err("Run budget exceeded".into());
            }
            let range = node.range();
            let opening_end = tag_end(source.as_bytes(), range.start)?;
            if source[..opening_end].ends_with("/>") {
                return Err("Self-closing Run unsupported".into());
            }
            let closing_start = range.start
                + source[range.clone()]
                    .rfind("</")
                    .ok_or("Missing Run closing tag")?;
            let body = &source[opening_end..closing_start];
            let (decoded, boundaries) = decode_body(body)?;
            if decoded.contains(['\r', '\n']) {
                return Err(
                    "Run literal line breaks are outside the paragraph mapping subset".into(),
                );
            }
            let parsed: String = node.children().filter_map(|child| child.text()).collect();
            if decoded != parsed {
                return Err("XML text normalization unsupported".into());
            }
            let scalar_start = text.chars().count();
            text.push_str(&decoded);
            let mut effective: [Option<String>; 5] = Default::default();
            for ancestor in [root, paragraph, node] {
                for (slot, name) in ["FontWeight", "FontStyle", "FontSize", "Foreground"]
                    .iter()
                    .enumerate()
                {
                    if let Some(value) = ancestor.attribute(*name) {
                        effective[slot] = Some(value.to_owned());
                    }
                }
            }
            effective[4] = node.attribute("TextDecorations").map(str::to_owned);
            runs.push(IndexedRun {
                range,
                opening_end,
                closing_start,
                paragraph: paragraph_index,
                scalar_start,
                boundaries,
                effective,
            });
            count += 1;
        }
        if count == 0 {
            return Err("Empty Paragraph source shape unsupported".into());
        }
        let range = paragraph.range();
        let opening_end = tag_end(source.as_bytes(), range.start)?;
        let closing_start = range.start
            + source[range.clone()]
                .rfind("</")
                .ok_or("Missing Paragraph closing tag")?;
        paragraph_xml.push(IndexedParagraph {
            range,
            opening_end,
            closing_start,
        });
        paragraphs.push(text);
    }
    if paragraphs.is_empty() {
        return Err("Empty rich source unsupported".into());
    }
    let plain = paragraphs.join("\n");
    let mut position = 0usize;
    let paragraph_starts = paragraphs
        .iter()
        .map(|text| {
            let start = position;
            position += text.chars().count() + 1;
            start
        })
        .collect();
    Ok(SourceIndex {
        paragraph_xml,
        runs,
        paragraphs,
        plain,
        paragraph_starts,
    })
}
