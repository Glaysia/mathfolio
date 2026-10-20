//! Conditional editing/import/export integration; independent numerical fixtures.
use mathcraft_compat::{
    Value,
    editor_bridge::{WorksheetXmlCache, tree_from_mathml, tree_to_mathml, worksheet_xml},
    evaluate_worksheet,
};
use mathcraft_editor::{BinaryOp, Direction, Editor, NodeKind, PortableTree};

const ML: &str = "http://schemas.mathsoft.com/math50";
fn node(kind: NodeKind, children: Vec<PortableTree>) -> PortableTree {
    PortableTree { kind, children }
}
fn number(value: i32) -> PortableTree {
    node(NodeKind::Number(value.to_string()), vec![])
}
fn name(value: &str) -> PortableTree {
    node(NodeKind::Identifier(value.into()), vec![])
}
fn program(children: Vec<PortableTree>) -> PortableTree {
    node(
        NodeKind::Program {
            statement_count: children.len(),
        },
        children,
    )
}
fn binary(op: BinaryOp, left: PortableTree, right: PortableTree) -> PortableTree {
    node(NodeKind::Binary(op), vec![left, right])
}
fn assign(name_: &str, value: PortableTree) -> PortableTree {
    node(NodeKind::LocalDefine, vec![name(name_), value])
}
fn conditional(
    pairs: Vec<(PortableTree, PortableTree)>,
    otherwise: Option<PortableTree>,
    alternate: bool,
) -> PortableTree {
    let kind = NodeKind::If {
        branch_count: pairs.len(),
        has_else: otherwise.is_some(),
        alternate,
    };
    let mut children: Vec<_> = pairs
        .into_iter()
        .flat_map(|(test, body)| [test, body])
        .collect();
    children.extend(otherwise);
    node(kind, children)
}
fn evaluation(value: PortableTree) -> PortableTree {
    binary(
        BinaryOp::Evaluate,
        value,
        node(NodeKind::Placeholder, vec![]),
    )
}
fn scalar(value: PortableTree, expected: f64) {
    let value = evaluation(value);
    let xml = tree_to_mathml(&value).unwrap();
    assert_eq!(tree_from_mathml(&xml).unwrap(), value);
    let worksheet = worksheet_xml(&[(1, 0., 0., xml)]).unwrap();
    let row = evaluate_worksheet(&worksheet).unwrap().remove(0);
    assert!(row.error.is_none(), "{row:?}");
    let Some(Value::Scalar { quantity }) = row.value else {
        panic!("{row:?}")
    };
    assert_eq!(quantity.value.re, expected);
    assert_eq!(quantity.value.im, 0.);
}

#[test]
fn flat_chains_roundtrip_all_tests_bodies_and_single_alternate_mode() {
    for count in [1, 2, 9, 64] {
        for alternate in [false, true] {
            // A one-pair node has no native alternate token to retain.
            if count == 1 && alternate {
                continue;
            }
            for has_else in [false, true] {
                let value = conditional(
                    (0..count)
                        .map(|index| (number(index), program(vec![number(index + 10)])))
                        .collect(),
                    has_else.then(|| program(vec![number(99)])),
                    alternate,
                );
                let xml = tree_to_mathml(&value).unwrap();
                assert_eq!(tree_from_mathml(&xml).unwrap(), value);
                let document = roxmltree::Document::parse(&xml).unwrap();
                let branch = if alternate { "alsoif" } else { "elseif" };
                assert_eq!(
                    document
                        .descendants()
                        .filter(|n| n.has_tag_name((ML, branch)))
                        .count(),
                    (count - 1) as usize
                );
                assert_eq!(
                    document
                        .descendants()
                        .filter(|n| n.has_tag_name((ML, "else")))
                        .count(),
                    usize::from(has_else)
                );
            }
        }
    }
}

#[test]
fn editor_inserted_branch_flows_through_native_bridge_and_calculation() {
    let initial = conditional(vec![(number(0), program(vec![number(9)]))], None, false);
    for alternate in [false, true] {
        let mut editor = Editor::from_portable(&initial).unwrap();
        editor.place_caret(editor.root(), Direction::Right).unwrap();
        assert!(if alternate {
            editor.insert_also_if()
        } else {
            editor.insert_else_if()
        });
        let mut edited = editor.to_portable().unwrap();
        assert_eq!(edited.children.len(), 4);
        edited.children[2] = number(1);
        edited.children[3] = program(vec![number(37)]);
        scalar(program(vec![edited]), 37.);
        assert!(editor.undo());
        assert_eq!(editor.to_portable().unwrap(), initial);
    }
}

#[test]
fn elseif_short_circuits_but_alsoif_executes_every_matching_body_and_else_only_when_none_match() {
    let error = binary(BinaryOp::InlineDivide, number(1), number(0));
    scalar(
        program(vec![conditional(
            vec![
                (number(1), program(vec![number(17)])),
                (error.clone(), program(vec![number(99)])),
            ],
            Some(program(vec![error.clone()])),
            false,
        )]),
        17.,
    );
    let increment = |value| assign("total", binary(BinaryOp::Add, name("total"), number(value)));
    scalar(
        program(vec![
            assign("total", number(0)),
            conditional(
                vec![
                    (number(1), program(vec![increment(1)])),
                    (number(0), program(vec![error.clone()])),
                    (number(1), program(vec![increment(10)])),
                    (number(1), program(vec![increment(100)])),
                ],
                Some(program(vec![error.clone()])),
                true,
            ),
            name("total"),
        ]),
        111.,
    );
    for alternate in [false, true] {
        scalar(
            program(vec![conditional(
                vec![
                    (number(0), program(vec![error.clone()])),
                    (number(0), program(vec![error.clone()])),
                ],
                Some(program(vec![number(43)])),
                alternate,
            )]),
            43.,
        );
    }
}

#[test]
fn nested_return_and_break_propagate_out_of_later_branches() {
    let returned = conditional(
        vec![
            (number(0), program(vec![number(10)])),
            (
                number(1),
                program(vec![node(NodeKind::Return, vec![number(73)])]),
            ),
            (number(1), program(vec![number(999)])),
        ],
        None,
        true,
    );
    scalar(program(vec![returned, number(9999)]), 73.);
    let branches = conditional(
        vec![
            (number(0), program(vec![number(0)])),
            (number(1), program(vec![node(NodeKind::Break, vec![])])),
        ],
        None,
        false,
    );
    scalar(
        program(vec![
            assign("total", number(7)),
            node(
                NodeKind::For,
                vec![
                    name("i"),
                    node(
                        NodeKind::Range { has_step: false },
                        vec![number(1), number(3)],
                    ),
                    program(vec![branches, assign("total", number(999))]),
                ],
            ),
            name("total"),
        ]),
        7.,
    );
}

#[test]
fn branch_metadata_and_contextual_local_labels_survive_cache_reuse_and_value_edits() {
    let mut value = conditional(
        vec![
            (number(0), program(vec![number(9)])),
            (number(1), program(vec![assign("m", number(12)), name("m")])),
        ],
        Some(program(vec![number(20)])),
        false,
    );
    let mut cache = WorksheetXmlCache::default();
    for replacement in [12, 42, 42] {
        value.children[3] = program(vec![assign("m", number(replacement)), name("m")]);
        let source = vec![(
            1,
            0.,
            0.,
            tree_to_mathml(&evaluation(program(vec![value.clone()]))).unwrap(),
        )];
        let cached = cache.build_prepared(&source).unwrap().into_parts().0;
        assert_eq!(cached, worksheet_xml(&source).unwrap());
        let rows = evaluate_worksheet(&cached).unwrap();
        assert!(rows[0].error.is_none(), "{:?}", rows[0]);
        assert_eq!(
            rows[0].value.as_ref().unwrap().numeric_json()["real"].as_f64(),
            Some(f64::from(replacement))
        );
        let tree = tree_from_mathml(&source[0].3).unwrap();
        assert_eq!(tree_to_mathml(&tree).unwrap(), source[0].3);
    }
}

#[test]
fn invalid_branch_order_arity_attributes_namespace_and_payload_are_preserved_as_unsupported() {
    let first = "<ml:test><ml:real>0</ml:real></ml:test><ml:then><ml:program><ml:real>1</ml:real></ml:program></ml:then>";
    let pair = "<ml:test><ml:real>1</ml:real></ml:test><ml:then><ml:program><ml:real>2</ml:real></ml:program></ml:then>";
    let otherwise = "<ml:else><ml:program><ml:real>3</ml:real></ml:program></ml:else>";
    for tail in [
        format!("{otherwise}<ml:elseif>{pair}</ml:elseif>"),
        format!("{otherwise}{otherwise}"),
        format!("<ml:elseif extra='1'>{pair}</ml:elseif>"),
        format!("<ml:elseif>{pair}<ml:real>9</ml:real></ml:elseif>"),
        "<ml:elseif/>".into(),
        format!("<elseif xmlns='other'>{pair}</elseif>"),
        format!("<ml:elseif>unexpected{pair}</ml:elseif>"),
    ] {
        let xml = format!("<ml:if xmlns:ml='{ML}'>{first}{tail}</ml:if>");
        assert!(tree_from_mathml(&xml).is_err(), "{xml}");
    }
}

#[test]
fn native_reader_normalizes_mixed_modes_and_scalar_bodies_without_flattening_nested_programs() {
    let pair = "<ml:test><ml:real>1</ml:real></ml:test><ml:then><ml:real>2</ml:real></ml:then>";
    let source = format!(
        "<ml:if xmlns:ml='{ML}'>{pair}<ml:elseif>{pair}</ml:elseif><ml:alsoif>{pair}</ml:alsoif><ml:else><ml:real>9</ml:real></ml:else></ml:if>"
    );
    let parsed = tree_from_mathml(&source).unwrap();
    assert_eq!(
        parsed.kind,
        NodeKind::If {
            branch_count: 3,
            has_else: true,
            alternate: true
        }
    );
    assert!(
        parsed
            .children
            .iter()
            .enumerate()
            .filter(|(index, _)| index % 2 == 1 || *index == 6)
            .all(|(_, body)| matches!(body.kind, NodeKind::Program { statement_count: 1 }))
    );
    let canonical = tree_to_mathml(&parsed).unwrap();
    assert!(!canonical.contains("elseif"));
    assert_eq!(canonical.matches("<ml:alsoif>").count(), 2);
    assert_eq!(tree_from_mathml(&canonical).unwrap(), parsed);
    // Raw numeric/program execution keeps its explicit mixed-mode boundary.
    let mixed = canonical
        .replacen("<ml:alsoif>", "<ml:elseif>", 1)
        .replacen("</ml:alsoif>", "</ml:elseif>", 1);
    let raw = format!("<program xmlns='{ML}'>{mixed}</program>");
    assert!(mathcraft_compat::program::from_mathml(&raw).is_err());
    let nested = format!(
        "<ml:if xmlns:ml='{ML}'><ml:test><ml:real>1</ml:real></ml:test><ml:then><ml:program><ml:program><ml:real>7</ml:real></ml:program></ml:program></ml:then></ml:if>"
    );
    let parsed = tree_from_mathml(&nested).unwrap();
    assert!(matches!(
        parsed.children[1].kind,
        NodeKind::Program { statement_count: 1 }
    ));
    assert!(matches!(
        parsed.children[1].children[0].kind,
        NodeKind::Program { statement_count: 1 }
    ));
    assert!(
        tree_to_mathml(&parsed)
            .unwrap()
            .contains("<ml:program><ml:program>")
    );
}
