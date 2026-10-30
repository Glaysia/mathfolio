//! Dense source metadata exercises operation-local identifier spans, not only
//! the small bare-matrix/reference-writer route. No calculator or timing test.
use super::*;
use mathfolio_editor::SourceMatrixDisplay;

const ROWS: usize = 16;
const COLS: usize = 17;

fn dense(display: SourceMatrixDisplay) -> PortableTree {
    evaluate(node(
        NodeKind::Matrix {
            rows: ROWS,
            cols: COLS,
            display: Some(display),
        },
        (0..ROWS)
            .flat_map(|row| {
                (0..COLS).map(move |col| {
                    node(
                        NodeKind::Binary(BinaryOp::Add),
                        vec![
                            node(NodeKind::Number((row * 1000 + col + 1).to_string()), vec![]),
                            node(
                                NodeKind::Binary(BinaryOp::Scale),
                                vec![
                                    node(
                                        NodeKind::Binary(BinaryOp::Add),
                                        vec![
                                            node(
                                                NodeKind::NativeIdentifier {
                                                    name: format!("v{row}_{col}"),
                                                    labels: "VARIABLE".into(),
                                                    contextual: Some(false),
                                                    subscript: Some(format!("r{row}𝛼")),
                                                },
                                                vec![],
                                            ),
                                            // A preceding VARIABLE m declaration changes this
                                            // ordinary identifier, while explicit UNIT m survives.
                                            value("m"),
                                        ],
                                    ),
                                    node(
                                        NodeKind::NativeIdentifier {
                                            name: "m".into(),
                                            labels: "UNIT".into(),
                                            contextual: Some(false),
                                            subscript: None,
                                        },
                                        vec![],
                                    ),
                                ],
                            ),
                        ],
                    )
                })
            })
            .collect(),
    ))
}

/// Check every XML column-major position against independent asymmetric full
/// coordinates, including all hidden cells; do not infer success from a count.
fn assert_full_labelled(source: &str, offset: &str, size: &str) {
    let xml = roxmltree::Document::parse(source).unwrap();
    let matrix = xml
        .descendants()
        .find(|n| n.has_tag_name((ML, "matrix")))
        .unwrap();
    assert_eq!(matrix.attribute("rows"), Some("16"));
    assert_eq!(matrix.attribute("cols"), Some("17"));
    let direct: Vec<_> = matrix.children().filter(Node::is_element).collect();
    assert_eq!(direct.len(), 1 + ROWS * COLS);
    assert!(direct[0].has_tag_name((ML, "display")));
    assert_eq!(direct[0].attribute("offset"), Some(offset));
    assert_eq!(direct[0].attribute("size"), Some(size));
    assert_eq!(
        xml.descendants()
            .filter(|n| n.has_tag_name((ML, "display")))
            .count(),
        1
    );
    for col in 0..COLS {
        for row in 0..ROWS {
            let cell = direct[1 + col * ROWS + row];
            let number = cell
                .descendants()
                .find(|n| n.has_tag_name((ML, "real")))
                .unwrap();
            assert_eq!(number.text().unwrap(), (row * 1000 + col + 1).to_string());
            let identifiers: Vec<_> = cell
                .descendants()
                .filter(|n| n.has_tag_name((ML, "id")))
                .collect();
            assert_eq!(identifiers.len(), 3);
            assert_eq!(
                crate::editor_bridge::identifier_parts(identifiers[0]).unwrap(),
                (
                    format!("v{row}_{col}"),
                    Some("VARIABLE".into()),
                    Some(false),
                    Some(format!("r{row}𝛼")),
                )
            );
            assert_eq!(
                crate::editor_bridge::identifier_parts(identifiers[1]).unwrap(),
                ("m".into(), Some("VARIABLE".into()), None, None,)
            );
            assert_eq!(
                crate::editor_bridge::identifier_parts(identifiers[2]).unwrap(),
                ("m".into(), Some("UNIT".into()), Some(false), None,)
            );
        }
    }
}

#[test]
fn dense_source_display_trace_warm_replay_replaces_prefix_without_losing_hidden_cells_or_labels() {
    let definition = node(
        NodeKind::Binary(BinaryOp::Define),
        vec![value("m"), node(NodeKind::Number("1".into()), vec![])],
    );
    let before = dense(SourceMatrixDisplay::from_parts(Some([0, 1]), Some([1, 2])).unwrap());
    let after = dense(SourceMatrixDisplay::from_parts(Some([12, 13]), Some([4, 4])).unwrap());
    assert_eq!(before.children[0].children, after.children[0].children);
    let mut traces = MathmlTraces::new(2);
    let declaration = traces.new_math(1, &definition).unwrap();
    let original_proof = traces.new_math(7, &before).unwrap();
    assert_eq!(
        original_proof.source(),
        serialize(&before, true, 0).unwrap()
    );
    traces.sort();
    assert!(
        traces.get(7, &original_proof).is_some(),
        "272 full cells must use the dense trace path despite a1x2 source window"
    );
    assert!(traces.bytes() <= 1024 * 1024);
    let mut proofs = HashMap::from([(1, declaration.clone()), (7, original_proof.clone())]);
    let mut regions = vec![
        (1, 0., 0., declaration.source().into()),
        (7, 0., 40., original_proof.source().into()),
    ];
    let mut retained = WorksheetXmlCache::default();
    let first = retained
        .build_prepared_with_traced_trees(&regions, &proofs, traces)
        .unwrap();
    let reference = WorksheetXmlCache::default()
        .build_prepared_with_trees(&regions)
        .unwrap();
    assert!(
        first.source == reference.source,
        "dense traced label source differs from independent DOM label source"
    );
    assert!(
        first.decoded_trees == reference.decoded_trees,
        "dense full typed projection differs from DOM"
    );
    assert_full_labelled(&first.source, "0,1", "1,2");
    assert_eq!(retained.hits, 0);
    let hits = retained.hits;
    let warm = retained
        .build_prepared_with_typed_trees(&regions, &proofs)
        .unwrap();
    assert!(warm.source == first.source);
    assert!(warm.decoded_trees.iter().all(Option::is_none));
    assert_eq!(
        retained.hits,
        hits + 2,
        "exact warm source/context must reuse both fragments"
    );

    let mut next_traces = MathmlTraces::new(2);
    let updated_proof = next_traces.new_math(7, &after).unwrap();
    assert_eq!(updated_proof.source(), serialize(&after, true, 0).unwrap());
    assert!(
        updated_proof
            .source()
            .contains("<ml:display offset=\"12,13\" size=\"4,4\"/>")
    );
    assert!(
        !updated_proof
            .source()
            .contains("<ml:display offset=\"0,1\" size=\"1,2\"/>")
    );
    next_traces.sort();
    assert!(next_traces.get(7, &updated_proof).is_some());
    assert!(
        next_traces.get(7, &original_proof).is_none(),
        "old strong owner cannot supply new byte spans"
    );
    regions[1].3 = updated_proof.source().into();
    proofs.insert(7, updated_proof);
    let hits = retained.hits;
    let changed = retained
        .build_prepared_with_traced_trees(&regions, &proofs, next_traces)
        .unwrap();
    assert_eq!(
        retained.hits,
        hits + 1,
        "unchanged declaration hits; changed source-display matrix must miss"
    );
    let reference = WorksheetXmlCache::default()
        .build_prepared_with_trees(&regions)
        .unwrap();
    assert!(
        changed.source == reference.source,
        "new display prefix/span replay differs from full DOM label pass"
    );
    assert!(changed.decoded_trees[0].is_none());
    assert!(changed.decoded_trees[1] == reference.decoded_trees[1]);
    assert_full_labelled(&changed.source, "12,13", "4,4");
    assert_eq!(retained.entries[&7].source, regions[1].3);
    assert!(
        retained.entries[&7]
            .labelled
            .contains("<ml:display offset=\"12,13\" size=\"4,4\"/>")
    );
    let hits = retained.hits;
    let warm_after = retained
        .build_prepared_with_typed_trees(&regions, &proofs)
        .unwrap();
    assert_eq!(retained.hits, hits + 2);
    assert!(warm_after.source == changed.source);
    assert!(warm_after.decoded_trees.iter().all(Option::is_none));
    assert_full_labelled(&warm_after.source, "12,13", "4,4");
}
