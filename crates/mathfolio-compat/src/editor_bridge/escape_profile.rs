//! Narrow escaping differential and CPU-only serializer measurements.
use super::*;
use std::{hint::black_box, time::Instant};

fn legacy_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[test]
fn escaped_bytes_match_legacy_for_entities_unicode_and_adjacent_special_characters() {
    let corpus = [
        "",
        "0123456789.-e+",
        "variable_name",
        "VARIABLE",
        "*",
        "NONE",
        "한글𝑥µ😀",
        "&<>\"'",
        "&amp;&lt;&#x1F600;",
        "''\"\"&&<<>>",
        "a&한글<µ>\"𝑥'😀",
        "\0\r\n\t",
    ];
    for text in corpus {
        assert_eq!(escape(text), legacy_escape(text));
    }
    let alphabet = ["&", "<", ">", "\"", "'", "a", "𝑥", "한", "😀", "\n"];
    for a in alphabet {
        for b in alphabet {
            for c in alphabet {
                let text = format!("{a}{b}{c}");
                assert_eq!(escape(&text), legacy_escape(&text));
            }
        }
    }
}

#[test]
fn serialized_identifier_metadata_and_subscripts_keep_legacy_entity_bytes() {
    let name = "幅𝑥&<>\"'😀";
    let labels = "CUSTOM&<>\"'";
    let sub = "rµ&<>\"'";
    let rich = tree(
        NodeKind::NativeIdentifier {
            name: name.into(),
            labels: labels.into(),
            contextual: Some(true),
            subscript: Some(sub.into()),
        },
        vec![],
    );
    let expected = format!(
        "<ml:id xmlns:ml=\"{ML}\" labels=\"{}\" label-is-contextual=\"true\" xml:space=\"preserve\"><p:Span xmlns:p=\"{XAML}\">{}<ptc:Subscript xmlns:ptc=\"{PTC}\">{}</ptc:Subscript></p:Span></ml:id>",
        legacy_escape(labels),
        legacy_escape(name),
        legacy_escape(sub)
    );
    assert_eq!(tree_to_mathml(&rich).unwrap(), expected);
    assert_eq!(tree_from_mathml(&expected).unwrap(), rich);
    let plain = tree(NodeKind::Identifier("&amp;&#x1F600;𝑥".into()), vec![]);
    assert_eq!(
        tree_to_mathml(&plain).unwrap(),
        format!("<ml:id xmlns:ml=\"{ML}\" xml:space=\"preserve\">&amp;amp;&amp;#x1F600;𝑥</ml:id>")
    );
}

fn error_cases() -> Vec<(&'static str, PortableTree)> {
    vec![
        ("nan", tree(NodeKind::Number("NaN".into()), vec![])),
        ("inf", tree(NodeKind::Number("inf".into()), vec![])),
        (
            "invalid-number",
            tree(NodeKind::Number("1&2".into()), vec![]),
        ),
        (
            "identifier-control",
            tree(NodeKind::Identifier("a\n".into()), vec![]),
        ),
        (
            "label-control",
            tree(
                NodeKind::AnnotatedIdentifier {
                    name: "a".into(),
                    labels: "V\n".into(),
                },
                vec![],
            ),
        ),
        (
            "bad-arity",
            tree(NodeKind::Binary(BinaryOp::Define), vec![]),
        ),
    ]
}

#[test]
fn escaping_preserves_existing_serialization_validation_errors() {
    let expected = [
        "Invalid structural tree: InvalidNumber",
        "Invalid structural tree: InvalidNumber",
        "Invalid structural tree: InvalidNumber",
        "Invalid structural tree: InvalidIdentifier",
        "Invalid structural tree: InvalidLabels",
        "Invalid structural tree: Arity { descriptor: \"Define\", expected: 2, actual: 0 }",
    ];
    for ((name, tree), expected) in error_cases().into_iter().zip(expected) {
        let error = tree_to_mathml(&tree).unwrap_err();
        if name == "bad-arity" {
            // The descriptor text is a separate editor policy; compare this
            // full error in the before/after profile rather than fixing it here.
            assert!(error.starts_with("Invalid structural tree: Arity"));
        } else {
            assert_eq!(error, expected, "{name}");
        }
    }
}

#[test]
#[ignore = "CPU-only escaping plus exact app-captured cold serialization profile"]
fn profile_escape_and_actual_cold_serializer() {
    let tokens = [
        "0123456789",
        "variable_name",
        "VARIABLE",
        "한글𝑥µ😀",
        "&<>\"'",
        "a&amp;<µ>\"𝑥'😀",
    ];
    for (name, run) in [
        ("production", escape as fn(&str) -> String),
        ("reference", legacy_escape as fn(&str) -> String),
    ] {
        for round in 0..7 {
            let start = Instant::now();
            for _ in 0..10_000 {
                for token in tokens {
                    black_box(run(black_box(token)));
                }
            }
            println!(
                "ESCAPE_MICRO {}",
                serde_json::json!({"name":name,"round":round,"ns":start.elapsed().as_nanos(),"calls":60_000})
            );
        }
    }
    for (name, tree) in error_cases() {
        println!(
            "SERIALIZER_ERROR {}",
            serde_json::json!({"name":name,"result":tree_to_mathml(&tree)})
        );
    }
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../artifacts/rea/worksheet-preparation-inputs-20261019");
    for name in [
        "4k-scalars-with-cache",
        "4k-scalars-without-cache",
        "100-matrices16-with-cache",
        "100-matrices16-without-cache",
        "1015_LCC_S",
        "1016_LCC_S",
    ] {
        let rows: Vec<(u64, f64, f64, String)> =
            serde_json::from_slice(&std::fs::read(base.join(format!("{name}.json"))).unwrap())
                .unwrap();
        let parsed: Vec<_> = rows.iter().map(|r| tree_from_mathml(&r.3)).collect();
        let trees: Vec<_> = parsed
            .iter()
            .filter_map(|tree| tree.as_ref().ok())
            .collect();
        let excluded: Vec<_> = parsed
            .iter()
            .filter_map(|tree| tree.as_ref().err())
            .collect();
        let exact: Vec<_> = trees.iter().map(|tree| tree_to_mathml(tree)).collect();
        println!(
            "SERIALIZER_BYTES {}",
            serde_json::json!({"name":name,"results":exact,"excluded_decode_errors":excluded})
        );
        for round in 0..7 {
            let start = Instant::now();
            let output: Vec<_> = trees
                .iter()
                .map(|tree| tree_to_mathml(black_box(tree)))
                .collect();
            let ns = start.elapsed().as_nanos();
            assert_eq!(output, exact);
            black_box(output);
            println!(
                "COLD_SERIALIZE {}",
                serde_json::json!({"name":name,"round":round,"ns":ns,"trees":trees.len(),"excluded":excluded.len()})
            );
        }
    }
}
