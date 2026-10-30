use super::*;

fn based(digits: &str, base: NumericBase) -> PortableTree {
    tree(NodeKind::based_number(digits, base).unwrap(), vec![])
}

#[test]
fn explicit_native_bases_roundtrip_lexemes_without_authoring_mode_or_calculation() {
    for (base, lexemes) in [
        (
            NumericBase::Binary,
            vec!["001101", "10.01", ".1", "2", "1E+2"],
        ),
        (NumericBase::Octal, vec!["032", "7.4", "8", "1E-2"]),
        (
            NumericBase::Hexadecimal,
            vec!["0a32", "2b3c", "1.Af", ".F", "0E"],
        ),
    ] {
        for digits in lexemes {
            let xml = format!(
                "<ml:real xmlns:ml=\"{ML}\" base=\"{}\">{digits}</ml:real>",
                base.radix()
            );
            let expected = based(digits, base);
            let parsed = tree_from_mathml(&xml).unwrap();
            assert_eq!(parsed, expected);
            assert_eq!(tree_to_mathml(&parsed).unwrap(), xml);
            let serialized = serde_json::to_string(&parsed).unwrap();
            assert_eq!(
                serde_json::from_str::<PortableTree>(&serialized).unwrap(),
                expected
            );
        }
    }
}

#[test]
fn both_writers_keep_based_source_through_matrix_eval_and_identifier_labelling() {
    let cells = vec![
        based("00101", NumericBase::Binary),
        based("32.4", NumericBase::Octal),
        based("0aF", NumericBase::Hexadecimal),
    ];
    let source = tree(
        NodeKind::Binary(BinaryOp::Evaluate),
        vec![
            tree(
                NodeKind::Matrix {
                    rows: 1,
                    cols: 3,
                    display: None,
                },
                cells,
            ),
            tree(NodeKind::Placeholder, vec![]),
        ],
    );
    let (optimized, eligible) = tree_to_mathml_proven(&source).unwrap();
    assert!(eligible);
    let fallback = serialize(&source, true, 0).unwrap();
    assert_eq!(optimized, fallback);
    assert_eq!(tree_from_mathml(&optimized).unwrap(), source);
    for (digits, radix) in [("00101", 2), ("32.4", 8), ("0aF", 16)] {
        assert!(optimized.contains(&format!("base=\"{radix}\">{digits}</ml:real>")));
    }
    // Labelling cannot turn based content into decimal text or a variable name.
    let labelled = label_fragment(&optimized, &mut HashMap::new()).unwrap();
    assert_eq!(tree_from_mathml(&labelled).unwrap(), source);
}

#[test]
fn native_negative_base_is_a_negation_and_decimal_wire_remains_old() {
    let input = format!("<ml:real xmlns:ml=\"{ML}\" base=\"16\">-0a32</ml:real>");
    let expected = tree(
        NodeKind::UnaryNeg,
        vec![based("0a32", NumericBase::Hexadecimal)],
    );
    let parsed = tree_from_mathml(&input).unwrap();
    assert_eq!(parsed, expected);
    let output = tree_to_mathml(&parsed).unwrap();
    assert!(output.contains("<ml:neg/><ml:real base=\"16\">0a32</ml:real>"));
    assert_eq!(tree_from_mathml(&output).unwrap(), expected);
    for text in ["12", "-0.0", "1.250E+2"] {
        let implicit = format!("<ml:real xmlns:ml=\"{ML}\">{text}</ml:real>");
        let explicit = format!("<ml:real xmlns:ml=\"{ML}\" base=\"10\">{text}</ml:real>");
        let decimal = tree(NodeKind::Number(text.into()), vec![]);
        assert_eq!(tree_from_mathml(&implicit).unwrap(), decimal);
        assert_eq!(tree_from_mathml(&explicit).unwrap(), decimal);
        // E119c omits the decimal default; source spelling stays byte exact.
        assert_eq!(tree_to_mathml(&decimal).unwrap(), implicit);
    }
}

#[test]
fn unsupported_radix_source_returns_error_for_lossless_opaque_fallback() {
    for inner in [
        "<ml:real base='3'>10</ml:real>",
        "<ml:real base='16'>FF</ml:real>",
        "<ml:real base='16'>0x10</ml:real>",
        "<ml:real base='2'>+1</ml:real>",
        "<ml:real base='2'></ml:real>",
        "<ml:real base='2'>1<!--split-->0</ml:real>",
        "<ml:real base='2' labels='UNIT'>10</ml:real>",
        "<ml:real x:base='2' xmlns:x='urn:other'>10</ml:real>",
        "<ml:imag base='16'>10</ml:imag>",
        // Legacy suffix-only and uppercase native spellings need separate provenance.
        "<ml:real>1101b</ml:real>",
        "<ml:real>1101B</ml:real>",
        "<ml:real base='10'>1101b</ml:real>",
    ] {
        let xml = format!("<ml:math xmlns:ml='{ML}'>{inner}</ml:math>");
        assert!(tree_from_mathml(&xml).is_err(), "{inner}");
    }
    // No new interpretation of existing Number/Imaginary text.
    assert!(tree_to_mathml(&tree(NodeKind::Number("1101b".into()), vec![])).is_err());
    let imag = tree(
        NodeKind::ImaginarySymbol {
            value: "12".into(),
            symbol: "j".into(),
        },
        vec![],
    );
    assert_eq!(
        tree_from_mathml(&tree_to_mathml(&imag).unwrap()).unwrap(),
        imag
    );
}

#[test]
fn forged_based_nodes_are_rejected_before_either_writer_emits_xml() {
    for literal in [
        "",
        "-1b",
        "1</ml:real>b",
        "0x20b",
        "1B",
        "1h",
        "1bb",
        "0".repeat(mathfolio_editor::MAX_ATOM_BYTES + 1).as_str(),
    ] {
        let forged = tree(
            NodeKind::BasedNumber {
                literal: literal.into(),
                base: NumericBase::Binary,
            },
            vec![],
        );
        assert!(tree_to_mathml(&forged).is_err());
        let eval = tree(
            NodeKind::Binary(BinaryOp::Evaluate),
            vec![forged, tree(NodeKind::Placeholder, vec![])],
        );
        assert!(tree_to_mathml(&eval).is_err());
    }
}
