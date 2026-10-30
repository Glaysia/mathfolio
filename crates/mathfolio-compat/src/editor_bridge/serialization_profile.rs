//! Independent legacy serializer oracle and CPU-only profiles.
use super::*;
use std::hint::black_box;
use std::time::Instant;

fn node(kind: NodeKind, children: Vec<PortableTree>) -> PortableTree {
    PortableTree { kind, children }
}
#[test]
fn writer_preserves_leaf_entities_metadata_and_fallible_child_order() {
    let leaves = vec![
        node(NodeKind::Identifier("a&<'\"µ😀".into()), vec![]),
        node(
            NodeKind::AnnotatedIdentifier {
                name: "x&y".into(),
                labels: "CUSTOM<&'\"".into(),
            },
            vec![],
        ),
        node(
            NodeKind::NativeIdentifier {
                name: "xµ".into(),
                labels: "*".into(),
                contextual: Some(true),
                subscript: Some("r<&😀".into()),
            },
            vec![],
        ),
    ];
    for leaf in leaves {
        for root in [true, false] {
            assert_eq!(
                serialization_writer::serialize(&leaf, root, 0, None),
                serialize(&leaf, root, 0)
            );
        }
    }
    let bad_number = || node(NodeKind::Number("not-number".into()), vec![]);
    let bad_imaginary = || {
        node(
            NodeKind::ImaginarySymbol {
                value: "3".into(),
                symbol: "k".into(),
            },
            vec![],
        )
    };
    for candidate in [
        node(
            NodeKind::Call { argument_count: 1 },
            vec![bad_imaginary(), bad_number()],
        ),
        node(
            NodeKind::Index { indices: 1 },
            vec![bad_imaginary(), bad_number()],
        ),
        node(
            NodeKind::Matrix {
                rows: 2,
                cols: 2,
                display: None,
            },
            vec![
                node(NodeKind::Number("1".into()), vec![]),
                bad_imaginary(),
                bad_number(),
                node(NodeKind::Placeholder, vec![]),
            ],
        ),
        node(
            NodeKind::Binary(BinaryOp::Evaluate),
            vec![bad_imaginary(), bad_number()],
        ),
    ] {
        assert_eq!(
            serialization_writer::serialize(&candidate, true, 0, None).err(),
            serialize(&candidate, true, 0).err()
        );
    }
    let placeholder = node(NodeKind::Placeholder, vec![]);
    assert_eq!(
        serialization_writer::serialize(&placeholder, true, 129, None).err(),
        serialize(&placeholder, true, 129).err()
    );
}
fn captures() -> Vec<(String, Vec<PortableTree>)> {
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
            let trees = source
                .iter()
                .filter_map(|(_, _, _, xml)| {
                    let wrapped =
                        format!("<context xmlns:ml=\"{ML}\" xmlns=\"{WS}\">{xml}</context>");
                    let tree = tree_from_mathml(&wrapped).ok()?;
                    eligible_root(&tree).then_some(tree)
                })
                .collect();
            (
                path.file_name().unwrap().to_string_lossy().into_owned(),
                trees,
            )
        })
        .collect()
}
#[test]
#[ignore = "CPU-only same-executable legacy/writer function profiles; recorder disabled"]
fn fused_serialization_function_profile() {
    for (name, trees) in captures() {
        let mut definitions = HashMap::new();
        let mut plans = Vec::new();
        for tree in &trees {
            let mut capture = worksheet_cache::FragmentContext::default();
            plans.push(plan(tree, Context::default(), &definitions, &mut capture));
            if matches!(
                tree.kind,
                NodeKind::Binary(BinaryOp::Define | BinaryOp::GlobalDefine)
            ) {
                let (name, labels, contextual, subscript) = parts(&tree.children[0].kind).unwrap();
                definitions.insert(
                    identifier_identity(name, subscript),
                    declaration_label_parts(name, labels, contextual, subscript, true),
                );
            }
        }
        for round in 0..10 {
            for writer in if round % 2 == 0 {
                [false, true]
            } else {
                [true, false]
            } {
                let start = Instant::now();
                let raw_bytes: usize = trees
                    .iter()
                    .map(|tree| {
                        if writer {
                            serialization_writer::serialize(tree, true, 0, None)
                        } else {
                            serialize(tree, true, 0)
                        }
                        .unwrap()
                        .len()
                    })
                    .sum();
                black_box(raw_bytes);
                let raw_ns = start.elapsed().as_nanos();
                let start = Instant::now();
                let label_bytes: usize = trees
                    .iter()
                    .zip(&plans)
                    .map(|(tree, plan)| {
                        if writer {
                            serialization_writer::serialize(tree, true, 0, Some(plan))
                        } else {
                            serialize_with_plan(tree, true, 0, Some(plan))
                        }
                        .unwrap()
                        .len()
                    })
                    .sum();
                black_box(label_bytes);
                let label_ns = start.elapsed().as_nanos();
                let start = Instant::now();
                for tree in &trees {
                    black_box(tree).validate().unwrap();
                }
                let validation_ns = start.elapsed().as_nanos();
                let start = Instant::now();
                for tree in &trees {
                    black_box(tree.clone());
                }
                let clone_ns = start.elapsed().as_nanos();
                println!(
                    "SERIALIZATION_PROFILE {}",
                    serde_json::json!({"fixture":name,"round":round,"writer":writer,"raw_ns":raw_ns,"label_ns":label_ns,"validation_ns":validation_ns,"clone_ns":clone_ns,"raw_bytes":raw_bytes,"label_bytes":label_bytes,"trees":trees.len()})
                );
            }
        }
        for (tree, plan) in trees.iter().zip(&plans) {
            assert_eq!(
                serialization_writer::serialize(tree, true, 0, None),
                serialize(tree, true, 0)
            );
            assert_eq!(
                serialization_writer::serialize(tree, true, 0, Some(plan)),
                serialize_with_plan(tree, true, 0, Some(plan))
            );
        }
    }
}
