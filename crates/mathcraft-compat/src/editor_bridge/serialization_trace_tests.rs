use super::*;
use std::sync::Arc;
fn node(kind: NodeKind, children: Vec<PortableTree>) -> PortableTree {
    PortableTree { kind, children }
}
fn value(text: &str) -> PortableTree {
    node(NodeKind::Identifier(text.into()), vec![])
}
fn evaluate(rhs: PortableTree) -> PortableTree {
    node(
        NodeKind::Binary(BinaryOp::Evaluate),
        vec![rhs, node(NodeKind::Placeholder, vec![])],
    )
}
fn compare(
    sources: &[(u64, f64, f64, String)],
    proofs: &HashMap<u64, Arc<TypedMathml>>,
    traces: MathmlTraces,
) {
    let mut old = WorksheetXmlCache::default();
    let mut new = WorksheetXmlCache::default();
    let expected = old.build_prepared_with_typed_trees(sources, proofs);
    let actual = new.build_prepared_with_traced_trees(sources, proofs, traces);
    match (expected, actual) {
        (Ok(a), Ok(b)) => {
            assert_eq!(a.source, b.source);
            assert_eq!(a.decoded_trees, b.decoded_trees);
        }
        (Err(a), Err(b)) => assert_eq!(a, b),
        _ => panic!("trace acceptance changed"),
    }
    assert_eq!(old.bytes, new.bytes);
    assert_eq!(old.entries.len(), new.entries.len());
    for (id, a) in &old.entries {
        let b = &new.entries[id];
        assert_eq!(
            (&a.source, &a.dependencies, &a.defined, &a.labelled, a.bytes),
            (&b.source, &b.dependencies, &b.defined, &b.labelled, b.bytes)
        );
    }
}
#[test]
fn traces_require_the_exact_strong_proof_owner_and_complete_serialization() {
    let cells = [
        value("m"),
        value("π"),
        value("sin"),
        node(
            NodeKind::NativeIdentifier {
                name: "幅µ".into(),
                labels: "*".into(),
                contextual: Some(true),
                subscript: Some("r1".into()),
            },
            vec![],
        ),
    ];
    let matrix = node(
        NodeKind::Matrix {
            rows: 16,
            cols: 16,
            display: None,
        },
        (0..256).map(|i| cells[i % 4].clone()).collect(),
    );
    let tree = evaluate(matrix.clone());
    for replacement in [false, true] {
        let mut traces = MathmlTraces::new(1);
        let proof = traces.new_math(7, &tree).unwrap();
        assert_eq!(proof.source(), serialize(&tree, true, 0).unwrap());
        traces.sort();
        assert!(traces.get(7, &proof).is_some(), "positive dense trace path");
        let xml = proof.source().into();
        let proof = if replacement {
            Arc::new(TypedMathml::new(&tree).unwrap())
        } else {
            proof
        };
        let proofs = HashMap::from([(7, proof)]);
        compare(&[(7, -0., 1., xml)], &proofs, traces);
    }
    // Call and Index intentionally delegate entire subtrees to the independent
    // arguments-first serializer, invalidating the optional trace as a whole.
    for kind in [
        NodeKind::Call { argument_count: 1 },
        NodeKind::Index { indices: 1 },
    ] {
        let tree = evaluate(node(kind, vec![value("sin"), matrix.clone()]));
        let mut traces = MathmlTraces::new(1);
        let proof = traces.new_math(1, &tree).unwrap();
        traces.sort();
        assert!(traces.get(1, &proof).is_none());
        let source = vec![(1, 0., 0., proof.source().into())];
        compare(&source, &HashMap::from([(1, proof)]), traces);
    }
}
#[test]
fn trace_budget_exhaustion_discards_partial_metadata_and_keeps_full_writer_cache_effects() {
    let tree = evaluate(node(
        NodeKind::Matrix {
            rows: 64,
            cols: 64,
            display: None,
        },
        (0..4096).map(|i| value(&format!("v{i}"))).collect(),
    ));
    let mut traces = MathmlTraces::new(4096);
    let mut proofs = HashMap::new();
    let mut source = Vec::new();
    for id in 0..6 {
        let proof = traces.new_math(id, &tree).unwrap();
        assert_eq!(proof.source(), serialize(&tree, true, 0).unwrap());
        source.push((id, 0., id as f64 * 10., proof.source().into()));
        proofs.insert(id, proof);
        assert!(traces.bytes() <= 1024 * 1024);
    }
    traces.sort();
    assert!(
        proofs.iter().any(|(id, p)| traces.get(*id, p).is_none()),
        "budget exercised"
    );
    compare(&source, &proofs, traces);
    let mut zero = MathmlTraces::new(0);
    let proof = zero.new_math(1, &tree).unwrap();
    zero.sort();
    assert!(zero.get(1, &proof).is_none());
    compare(
        &[(1, 0., 0., proof.source().into())],
        &HashMap::from([(1, proof)]),
        zero,
    );
}
#[test]
fn traced_constructor_failures_and_output_error_order_match_the_old_paths() {
    let bad_number = node(NodeKind::Number("NaN".into()), vec![]);
    let bad_imag = node(
        NodeKind::ImaginarySymbol {
            value: "3".into(),
            symbol: "k".into(),
        },
        vec![],
    );
    for tree in [
        evaluate(node(
            NodeKind::Call { argument_count: 1 },
            vec![bad_imag.clone(), bad_number.clone()],
        )),
        evaluate(node(
            NodeKind::Index { indices: 1 },
            vec![bad_imag, bad_number.clone()],
        )),
        evaluate(node(
            NodeKind::Matrix {
                rows: 1,
                cols: 1,
                display: None,
            },
            vec![bad_number],
        )),
    ] {
        assert_eq!(
            MathmlTraces::new(1).new_math(1, &tree).err(),
            TypedMathml::new(&tree).err()
        );
    }
    let tree = evaluate(value("x"));
    let mut traces = MathmlTraces::new(1);
    let proof = traces.new_math(1, &tree).unwrap();
    let source = vec![
        (1, 0., 0., proof.source().into()),
        (2, 0., 10., "<ml:eval>".into()),
    ];
    compare(&source, &HashMap::from([(1, proof)]), traces);
    let mut traces = MathmlTraces::new(750);
    let mut source = Vec::new();
    let mut proofs = HashMap::new();
    for id in 0..750 {
        let tree = evaluate(value(&format!("v{id}{}", "x".repeat(16000))));
        let proof = traces.new_math(id, &tree).unwrap();
        source.push((id, 0., id as f64, proof.source().into()));
        proofs.insert(id, proof);
    }
    compare(&source, &proofs, traces);
}
#[test]
#[ignore = "CPU-only source-proof creation plus cold worksheet preparation, no recorder"]
fn traced_complete_preparation_profile() {
    for (name, source, old_proofs) in typed_tests::captures() {
        let reference = WorksheetXmlCache::default()
            .build_prepared_with_trees(&source)
            .unwrap();
        for round in 0..10 {
            for traced in if round % 2 == 0 {
                [false, true]
            } else {
                [true, false]
            } {
                let start = std::time::Instant::now();
                let mut traces = MathmlTraces::new(if traced { source.len() } else { 0 });
                let proofs = old_proofs
                    .iter()
                    .map(|(id, proof)| {
                        (
                            *id,
                            if traced {
                                traces.new_math(*id, proof.tree()).unwrap()
                            } else {
                                Arc::new(TypedMathml::new(proof.tree()).unwrap())
                            },
                        )
                    })
                    .collect::<HashMap<_, _>>();
                let proof_ns = start.elapsed().as_nanos();
                let trace_bytes = if traced { traces.bytes() } else { 0 };
                let mut cache = WorksheetXmlCache::default();
                let builder_start = std::time::Instant::now();
                let output = if traced {
                    cache.build_prepared_with_traced_trees(&source, &proofs, traces)
                } else {
                    cache.build_prepared_with_typed_trees(&source, &proofs)
                }
                .unwrap();
                let builder_ns = builder_start.elapsed().as_nanos();
                let total_ns = start.elapsed().as_nanos();
                assert_eq!(output.source, reference.source);
                assert_eq!(output.decoded_trees, reference.decoded_trees);
                println!(
                    "TRACED_PREPARATION {}",
                    serde_json::json!({"fixture":name,"round":round,"traced":traced,"proof_ns":proof_ns,"builder_ns":builder_ns,"total_ns":total_ns,"trace_bytes":trace_bytes,"retained_bytes":cache.bytes,"source_bytes":output.source.len()})
                );
            }
        }
    }
}

#[test]
fn unused_trace_pool_has_no_owned_index_or_persistent_payload() {
    let pool = MathmlTraces::new(10000);
    assert!(
        pool.bytes() < 1024,
        "no eager index allocation on exact source-cache hits"
    );
}

#[test]
fn dense_gate_255_256_cells_preserves_complete_output_and_cache_bytes() {
    for cells in [255, 256] {
        let tree = evaluate(node(
            NodeKind::Group,
            vec![node(
                NodeKind::Matrix {
                    rows: 1,
                    cols: cells,
                    display: None,
                },
                (0..cells).map(|i| value(&format!("v{i}"))).collect(),
            )],
        ));
        let mut traces = MathmlTraces::new(1);
        let proof = traces.new_math(1, &tree).unwrap();
        traces.sort();
        assert_eq!(traces.get(1, &proof).is_some(), cells >= 256);
        compare(
            &[(1, -0., 1., proof.source().into())],
            &HashMap::from([(1, proof)]),
            traces,
        );
    }
    let tree = evaluate(value("x"));
    let mut traces = MathmlTraces::new(4000);
    let proof = traces.new_math(1, &tree).unwrap();
    assert!(traces.bytes() < 1024);
    compare(
        &[(1, 0., 0., proof.source().into())],
        &HashMap::from([(1, proof)]),
        traces,
    );
}

#[path = "serialization_trace_tests/source_matrix_display_tests.rs"]
mod source_matrix_display_tests;
