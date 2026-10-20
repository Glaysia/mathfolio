use super::*;
use std::sync::Arc;
type Statements = Vec<(u64, f64, f64, String)>;
type Proofs = HashMap<u64, Arc<TypedMathml>>;
type ProvenStatements = (Statements, Proofs);
fn statements(raw: &[&str]) -> ProvenStatements {
    let mut proofs = HashMap::new();
    let regions = raw
        .iter()
        .enumerate()
        .map(|(i, xml)| {
            let wrapped = format!("<context xmlns:ml=\"{ML}\" xmlns=\"{WS}\">{xml}</context>");
            let tree = tree_from_mathml(&wrapped).unwrap();
            let proof = Arc::new(TypedMathml::new(&tree).unwrap());
            let source = proof.source().to_string();
            proofs.insert(i as u64 + 1, proof);
            (i as u64 + 1, -0., i as f64 * 40., source)
        })
        .collect();
    (regions, proofs)
}
fn verify(regions: &[(u64, f64, f64, String)], proofs: &HashMap<u64, Arc<TypedMathml>>) {
    for proof in proofs.values() {
        assert_eq!(
            proof.source(),
            serialize(proof.tree(), true, 0).unwrap(),
            "raw writer/legacy exact bytes"
        );
    }
    let mut old = WorksheetXmlCache::default();
    let mut new = WorksheetXmlCache::default();
    let expected = old.build_prepared_with_trees(regions).unwrap();
    let actual = new
        .build_prepared_with_typed_trees(regions, proofs)
        .unwrap();
    assert_eq!(actual.source, expected.source);
    assert_eq!(actual.decoded_trees, expected.decoded_trees);
    assert_eq!(old.bytes, new.bytes);
    assert_eq!(old.entries.len(), new.entries.len());
    for (id, a) in &old.entries {
        let b = &new.entries[id];
        assert_eq!(a.source, b.source);
        assert_eq!(a.dependencies, b.dependencies);
        assert_eq!(a.defined, b.defined);
        assert_eq!(a.labelled, b.labelled);
        assert_eq!(a.bytes, b.bytes);
    }
    // An independent operation-local trace route must match the original DOM,
    // including cache payload and projection canonicalization, for every case.
    let mut traces = MathmlTraces::new(proofs.len());
    let traced_proofs = proofs
        .iter()
        .map(|(id, proof)| (*id, traces.new_math(*id, proof.tree()).unwrap()))
        .collect();
    assert!(traces.bytes() <= 1024 * 1024);
    let mut traced = WorksheetXmlCache::default();
    let traced_actual = traced
        .build_prepared_with_traced_trees(regions, &traced_proofs, traces)
        .unwrap();
    assert_eq!(traced_actual.source, expected.source);
    assert_eq!(traced_actual.decoded_trees, expected.decoded_trees);
    assert_eq!(traced.bytes, old.bytes);
    for (id, a) in &old.entries {
        let b = &traced.entries[id];
        assert_eq!(
            (&a.source, &a.dependencies, &a.defined, &a.labelled, a.bytes),
            (&b.source, &b.dependencies, &b.defined, &b.labelled, b.bytes)
        );
    }
    let dom = roxmltree::Document::parse(&actual.source).unwrap();
    for (region, result) in actual.regions.iter().zip(actual.decoded_trees.iter()) {
        if let Some(range) = &region.math {
            let node = dom
                .descendants()
                .find(|n| n.range() == *range && n.has_tag_name((WS, "math")))
                .unwrap();
            assert_eq!(result, &Some(tree_from_mathml_node(node)));
        }
    }
    let expected = old.build_prepared_with_trees(regions).unwrap();
    let actual = new
        .build_prepared_with_typed_trees(regions, proofs)
        .unwrap();
    assert_eq!(actual.source, expected.source);
    assert_eq!(actual.decoded_trees, expected.decoded_trees);
    assert_eq!(old.bytes, new.bytes);
}
#[test]
fn typed_plain_labels_units_none_contextual_globals_and_xaml_are_exact() {
    let (mut source, proofs) = statements(&[
        "<ml:eval><ml:id>m</ml:id><ml:unitOverride><ml:id>m</ml:id></ml:unitOverride></ml:eval>",
        "<ml:define><ml:id labels='*'>m</ml:id><ml:real>-0</ml:real></ml:define>",
        "<ml:globalDefine><ml:id>cm</ml:id><ml:real>8</ml:real></ml:globalDefine>",
        "<ml:eval><ml:apply><ml:id label-is-contextual='true' labels='UNIT'>sin</ml:id><ml:id labels='CONSTANT'>π</ml:id></ml:apply><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>",
        "<ml:define><ml:id labels='*' xmlns:p='http://schemas.microsoft.com/winfx/2006/xaml/presentation' xmlns:t='clr-namespace:Ptc.Wpf;assembly=Ptc.Core'><p:Span>𝑥µ한글<t:Subscript>r😀</t:Subscript></p:Span></ml:id><ml:id labels='VARIABLE' label-is-contextual='false'>cm</ml:id></ml:define>",
        "<ml:define><ml:id>ORIGIN</ml:id><ml:real>1</ml:real></ml:define>",
        "<ml:define><ml:id>globalDefine &amp; &lt;µ&gt;</ml:id><ml:id labels='NONE'>cm</ml:id></ml:define>",
    ]);
    assert!(proofs.values().all(|p| p.eligible()));
    verify(&source, &proofs);
    source[1].2 = -100.;
    verify(&source, &proofs);
}
#[test]
fn typed_matrix_child_identity_and_index_unit_interrupt_are_exact() {
    let (source, proofs) = statements(&[
        "<ml:define><ml:id>m</ml:id><ml:real>3</ml:real></ml:define>",
        "<ml:define><ml:id>A</ml:id><ml:matrix rows='2' cols='2'><ml:id>m</ml:id><ml:id>s</ml:id><ml:id>kg</ml:id><ml:id>cm</ml:id></ml:matrix></ml:define>",
        "<ml:eval><ml:id>A</ml:id><ml:unitOverride><ml:apply><ml:indexer/><ml:id>m</ml:id><ml:id>cm</ml:id></ml:apply></ml:unitOverride></ml:eval>",
        "<ml:eval><ml:id>A</ml:id><ml:unitOverride><ml:apply><ml:matrow/><ml:id>kg</ml:id><ml:id>cm</ml:id></ml:apply></ml:unitOverride></ml:eval>",
    ]);
    verify(&source, &proofs);
}
#[test]
fn typed_function_program_and_proof_mismatch_use_ordinary_dom() {
    let (mut source, proofs) = statements(&[
        "<ml:define><ml:function><ml:id>f</ml:id><ml:boundVars><ml:id>m</ml:id></ml:boundVars></ml:function><ml:id>m</ml:id></ml:define>",
        "<ml:eval><ml:program><ml:localDefine><ml:id>m</ml:id><ml:real>2</ml:real></ml:localDefine><ml:id>m</ml:id></ml:program><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>",
        "<ml:define><ml:id>x</ml:id><ml:real>1</ml:real></ml:define>",
    ]);
    assert!(!proofs[&1].eligible());
    assert!(!proofs[&2].eligible());
    verify(&source, &proofs);
    source[2].3 = source[2].3.replace("<ml:real>1<", "<ml:real>5<");
    verify(&source, &proofs);
    source[2].3 = "<ml:eval bad='x'><ml:real>1</ml:real></ml:eval>".into();
    assert_eq!(
        WorksheetXmlCache::default()
            .build_prepared_with_typed_trees(&source, &proofs)
            .err(),
        WorksheetXmlCache::default()
            .build_prepared_with_trees(&source)
            .err()
    );
}

fn from_trees(trees: Vec<PortableTree>) -> ProvenStatements {
    let mut proofs = HashMap::new();
    let source = trees
        .into_iter()
        .enumerate()
        .map(|(i, tree)| {
            let proof = Arc::new(TypedMathml::new(&tree).unwrap());
            let xml = proof.source().to_owned();
            proofs.insert(i as u64 + 1, proof);
            (i as u64 + 1, 0., i as f64, xml)
        })
        .collect();
    (source, proofs)
}
#[test]
fn typed_all_plain_editor_variants_match_canonical_dom() {
    let id = || tree(NodeKind::Identifier("cm".into()), vec![]);
    let n = || tree(NodeKind::Number("-0.000e+0".into()), vec![]);
    let mut expressions = vec![
        id(),
        n(),
        tree(NodeKind::Imaginary("2".into()), vec![]),
        tree(
            NodeKind::ImaginarySymbol {
                value: "-3".into(),
                symbol: "j".into(),
            },
            vec![],
        ),
        tree(NodeKind::Placeholder, vec![]),
    ];
    for kind in [
        NodeKind::UnaryNeg,
        NodeKind::LogicalNot,
        NodeKind::Absolute,
        NodeKind::Factorial,
        NodeKind::Percent,
        NodeKind::Degree,
        NodeKind::Transpose,
        NodeKind::Conjugate,
        NodeKind::Vectorize,
        NodeKind::Determinant,
        NodeKind::Group,
    ] {
        expressions.push(tree(kind, vec![id()]));
    }
    for kind in [
        NodeKind::Root,
        NodeKind::Row,
        NodeKind::Column,
        NodeKind::Fraction,
        NodeKind::Power,
        NodeKind::Range { has_step: false },
        NodeKind::Index { indices: 1 },
    ] {
        expressions.push(tree(kind, vec![id(), n()]));
    }
    expressions.push(tree(
        NodeKind::Range { has_step: true },
        vec![n(), id(), n()],
    ));
    expressions.push(tree(NodeKind::Index { indices: 2 }, vec![id(), n(), id()]));
    for count in [0, 1, 2] {
        let mut children = vec![tree(NodeKind::Identifier("sin".into()), vec![])];
        children.extend((0..count).map(|_| id()));
        expressions.push(tree(
            NodeKind::Call {
                argument_count: count,
            },
            children,
        ));
    }
    for op in [
        BinaryOp::Add,
        BinaryOp::Subtract,
        BinaryOp::Multiply,
        BinaryOp::Scale,
        BinaryOp::InlineDivide,
        BinaryOp::CrossProduct,
        BinaryOp::Polar,
        BinaryOp::Equal,
        BinaryOp::NotEqual,
        BinaryOp::LessThan,
        BinaryOp::LessOrEqual,
        BinaryOp::GreaterThan,
        BinaryOp::GreaterOrEqual,
        BinaryOp::And,
        BinaryOp::Or,
        BinaryOp::Xor,
    ] {
        expressions.push(tree(NodeKind::Binary(op), vec![id(), n()]));
    }
    let trees = expressions
        .into_iter()
        .map(|rhs| {
            tree(
                NodeKind::Binary(BinaryOp::Evaluate),
                vec![rhs, tree(NodeKind::Placeholder, vec![])],
            )
        })
        .collect();
    let (source, proofs) = from_trees(trees);
    assert!(proofs.values().all(|p| p.eligible()));
    verify(&source, &proofs);
}
#[test]
fn typed_xml_normalization_and_depth_are_conservative_fallbacks() {
    for name in ["a\rb", "a\u{fffe}b", "a\u{ffff}b"] {
        let candidate = tree(
            NodeKind::Binary(BinaryOp::Define),
            vec![
                tree(NodeKind::Identifier(name.into()), vec![]),
                tree(NodeKind::Number("1".into()), vec![]),
            ],
        );
        if let Err(error) = TypedMathml::new(&candidate) {
            assert_eq!(Some(error), tree_to_mathml(&candidate).err());
            continue;
        }
        let (source, proofs) = from_trees(vec![candidate]);
        assert!(!proofs[&1].eligible());
        let mut old = WorksheetXmlCache::default();
        let mut new = WorksheetXmlCache::default();
        assert_eq!(
            old.build_prepared_with_trees(&source).err(),
            new.build_prepared_with_typed_trees(&source, &proofs).err()
        );
        assert_eq!(old.bytes, new.bytes);
    }
    for label in ["A\tB", "A\nB"] {
        let candidate = tree(
            NodeKind::Binary(BinaryOp::Define),
            vec![
                tree(
                    NodeKind::AnnotatedIdentifier {
                        name: "x".into(),
                        labels: label.into(),
                    },
                    vec![],
                ),
                tree(NodeKind::Number("1".into()), vec![]),
            ],
        );
        assert!(TypedMathml::new(&candidate).is_err());
        assert_eq!(
            TypedMathml::new(&candidate).err(),
            tree_to_mathml(&candidate).err()
        );
    }
    let mut rhs = tree(NodeKind::Identifier("m".into()), vec![]);
    for _ in 0..65 {
        rhs = tree(NodeKind::Group, vec![rhs]);
    }
    let (source, proofs) = from_trees(vec![tree(
        NodeKind::Binary(BinaryOp::Evaluate),
        vec![rhs, tree(NodeKind::Placeholder, vec![])],
    )]);
    assert!(!proofs[&1].eligible());
    verify(&source, &proofs);
}
pub(in crate::editor_bridge) fn captures() -> Vec<(String, Statements, Proofs)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../artifacts/rea/worksheet-preparation-inputs-20261019");
    let mut paths: Vec<_> = std::fs::read_dir(root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect();
    paths.sort();
    assert_eq!(paths.len(), 6);
    paths
        .into_iter()
        .map(|path| {
            let source: Vec<(u64, f64, f64, String)> =
                serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
            let proofs = source
                .iter()
                .filter_map(|(id, _, _, xml)| {
                    let wrapped =
                        format!("<context xmlns:ml=\"{ML}\" xmlns=\"{WS}\">{xml}</context>");
                    let proof = Arc::new(TypedMathml::new(&tree_from_mathml(&wrapped).ok()?).ok()?);
                    (proof.source() == xml).then_some((*id, proof))
                })
                .collect();
            (
                path.file_name().unwrap().to_string_lossy().into_owned(),
                source,
                proofs,
            )
        })
        .collect()
}
#[test]
#[ignore = "read-only actual Enter captures, not shipped fixtures"]
fn captured_typed_labels_sources_trees_and_payload_match_dom() {
    for (name, source, proofs) in captures() {
        verify(&source, &proofs);
        println!(
            "TYPED_ORACLE {name} regions={} exact_proofs={} eligible={}",
            source.len(),
            proofs.len(),
            proofs.values().filter(|p| p.eligible()).count()
        );
    }
}
#[test]
#[ignore = "CPU-only legacy/typed cold and warm builder profile; no phase recorder"]
fn captured_typed_builder_profile() {
    for (name, source, proofs) in captures() {
        for round in 0..10 {
            for typed in if round % 2 == 0 {
                [false, true]
            } else {
                [true, false]
            } {
                let mut cache = WorksheetXmlCache::default();
                let start = std::time::Instant::now();
                let cold = if typed {
                    cache.build_prepared_with_typed_trees(&source, &proofs)
                } else {
                    cache.build_prepared_with_trees(&source)
                }
                .unwrap();
                let cold_ns = start.elapsed().as_nanos();
                let start = std::time::Instant::now();
                let warm = if typed {
                    cache.build_prepared_with_typed_trees(&source, &proofs)
                } else {
                    cache.build_prepared_with_trees(&source)
                }
                .unwrap();
                let warm_ns = start.elapsed().as_nanos();
                assert_eq!(cold.source, warm.source);
                println!(
                    "TYPED_PROFILE {}",
                    serde_json::json!({"fixture":name,"round":round,"typed":typed,"cold_ns":cold_ns,"warm_ns":warm_ns,"retained_bytes":cache.bytes,"source_bytes":cold.source.len(),"proofs":proofs.len(),"eligible":proofs.values().filter(|p|p.eligible()).count()})
                );
            }
        }
    }
}

#[test]
fn typed_cache_context_changes_reorder_delete_and_new_ids_match_dom() {
    let (mut source, mut proofs) = statements(&[
        "<ml:define><ml:id>m</ml:id><ml:real>1</ml:real></ml:define>",
        "<ml:eval><ml:id>m</ml:id><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>",
        "<ml:eval><ml:apply><ml:id>sin</ml:id><ml:id>m</ml:id></ml:apply><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>",
    ]);
    let mut old = WorksheetXmlCache::default();
    let mut new = WorksheetXmlCache::default();
    for round in 0..7 {
        match round {
            1 => source[0].2 = 100.,
            2 => source[0].2 = -10.,
            3 => {
                source.remove(0);
                proofs.remove(&1);
            }
            4 => {
                let (mut added, added_proof) = statements(&[
                    "<ml:globalDefine><ml:id>m</ml:id><ml:real>4</ml:real></ml:globalDefine>",
                ]);
                added[0].0 = 9;
                added[0].2 = 400.;
                proofs.insert(9, added_proof[&1].clone());
                source.push(added.remove(0));
            }
            5 => source.reverse(),
            _ => (),
        }
        let a = old.build_prepared_with_trees(&source).unwrap();
        let b = new
            .build_prepared_with_typed_trees(&source, &proofs)
            .unwrap();
        assert_eq!(a.source, b.source);
        assert_eq!(a.decoded_trees, b.decoded_trees);
        assert_eq!(old.bytes, new.bytes);
        assert_eq!(old.entries.len(), new.entries.len());
        for (id, a) in &old.entries {
            let b = &new.entries[id];
            assert_eq!(a.source, b.source);
            assert_eq!(a.dependencies, b.dependencies);
            assert_eq!(a.defined, b.defined);
            assert_eq!(a.labelled, b.labelled);
            assert_eq!(a.bytes, b.bytes);
        }
    }
}
#[test]
fn typed_failure_order_and_partial_cache_side_effects_match_dom() {
    let (valid, proofs) =
        statements(&["<ml:define><ml:id>x</ml:id><ml:real>1</ml:real></ml:define>"]);
    let mut invalid = valid.clone();
    invalid.push((
        2,
        0.,
        30.,
        "<ml:eval><ml:id bad='1'>x</ml:id></ml:eval>".into(),
    ));
    let mut malformed = valid.clone();
    malformed.push((2, 0., 30., "<ml:eval>".into()));
    let cases = [
        invalid,
        malformed,
        vec![valid[0].clone(), valid[0].clone()],
        vec![valid[0].clone(), (2, f64::NAN, 30., valid[0].3.clone())],
    ];
    for source in cases {
        let mut old = WorksheetXmlCache::default();
        let mut new = WorksheetXmlCache::default();
        for _ in 0..2 {
            let a = old.build_prepared_with_trees(&source).err();
            let b = new.build_prepared_with_typed_trees(&source, &proofs).err();
            assert!(a.is_some());
            assert_eq!(a, b);
            assert_eq!(old.bytes, new.bytes);
            assert_eq!(old.entries.len(), new.entries.len());
        }
    }
}

#[test]
fn typed_output_budget_and_payload_eviction_match_dom() {
    let trees = (0..750)
        .map(|i| {
            tree(
                NodeKind::Binary(BinaryOp::Define),
                vec![
                    tree(
                        NodeKind::Identifier(format!("v{i}{}", "x".repeat(16000))),
                        vec![],
                    ),
                    tree(NodeKind::Number("1".into()), vec![]),
                ],
            )
        })
        .collect();
    let (source, proofs) = from_trees(trees);
    let mut old = WorksheetXmlCache::default();
    let mut new = WorksheetXmlCache::default();
    let a = old.build_prepared_with_trees(&source).err();
    let b = new.build_prepared_with_typed_trees(&source, &proofs).err();
    assert_eq!(a.as_deref(), Some("Worksheet exceeds XML budget"));
    assert_eq!(a, b);
    assert_eq!(old.bytes, new.bytes);
    assert!(new.bytes <= MAX_CACHED_BYTES);
    assert_eq!(old.entries.len(), new.entries.len());
    assert!(new.entries.len() < source.len(), "payload eviction reached");
    for (id, a) in &old.entries {
        let b = &new.entries[id];
        assert_eq!(a.source, b.source);
        assert_eq!(a.dependencies, b.dependencies);
        assert_eq!(a.defined, b.defined);
        assert_eq!(a.labelled, b.labelled);
        assert_eq!(a.bytes, b.bytes);
    }
}
