//! Independent r193 pre-receipt validation and aggregate loops.
//! Do not replace these with calls to receipt-backed validate/source_bytes.
use super::*;
fn old_validate_tree(tree: &PortableTree) -> Result<(), String> {
    tree.validate()
        .map_err(|e| format!("Invalid plot expression: {e}"))?;
    let mut stack = vec![tree];
    let mut nodes = 0usize;
    let mut text = 0usize;
    while let Some(n) = stack.pop() {
        nodes += 1;
        if nodes > MAX_EXPRESSION_NODES {
            return Err("Plot expression node budget exceeded".into());
        }
        match &n.kind {
            NodeKind::Placeholder
            | NodeKind::UnaryNeg
            | NodeKind::LogicalNot
            | NodeKind::Root
            | NodeKind::Absolute
            | NodeKind::Factorial
            | NodeKind::Percent
            | NodeKind::Degree
            | NodeKind::Transpose
            | NodeKind::Conjugate
            | NodeKind::Vectorize
            | NodeKind::Determinant
            | NodeKind::Row
            | NodeKind::Column
            | NodeKind::Call { .. }
            | NodeKind::Matrix { .. }
            | NodeKind::Index { .. }
            | NodeKind::Range { .. }
            | NodeKind::Integral
            | NodeKind::Derivative { .. }
            | NodeKind::Summation
            | NodeKind::Product
            | NodeKind::Fraction
            | NodeKind::Power
            | NodeKind::Group => {}
            NodeKind::Binary(op)
                if !matches!(
                    op,
                    BinaryOp::Define | BinaryOp::GlobalDefine | BinaryOp::Evaluate
                ) => {}
            NodeKind::Number(s) | NodeKind::Imaginary(s) | NodeKind::Identifier(s) => {
                text = text.saturating_add(s.len())
            }
            NodeKind::ImaginarySymbol { value, symbol } => {
                text = text
                    .saturating_add(value.len())
                    .saturating_add(symbol.len())
            }
            NodeKind::AnnotatedIdentifier { name, labels } => {
                text = text.saturating_add(name.len()).saturating_add(labels.len())
            }
            NodeKind::NativeIdentifier {
                name,
                labels,
                subscript,
                ..
            } => {
                text = text
                    .saturating_add(name.len())
                    .saturating_add(labels.len())
                    .saturating_add(subscript.as_ref().map_or(0, String::len))
            }
            _ => {
                return Err(
                    "Plot expression requires a supported expression, not a statement".into(),
                );
            }
        }
        if text > MAX_EXPRESSION_BYTES {
            return Err("Plot expression text budget exceeded".into());
        }
        stack.extend(&n.children);
    }
    let xml = editor_bridge::tree_to_mathml(tree)?;
    if xml.len() > MAX_EXPRESSION_BYTES {
        return Err("Plot expression serialized budget exceeded".into());
    }
    // Do not canonicalize away source metadata in the authoritative tree.
    Ok(())
}
fn old_expression_budget<'a>(
    values: impl IntoIterator<Item = &'a PlotExpression>,
) -> Result<(), String> {
    let (mut nodes, mut bytes) = (0usize, 0usize);
    for value in values {
        match value {
            PlotExpression::LegacyText(s) if s.len() > 4096 => {
                return Err("Plot expression exceeds 4096 bytes".into());
            }
            PlotExpression::LegacyText(_) => (),
            PlotExpression::Typed(t) => old_validate_tree(&t.0.tree)?,
        };
        bytes = bytes.saturating_add(match value {
            PlotExpression::LegacyText(s) => s.len(),
            PlotExpression::Typed(_) => serde_json::to_vec(value).map_or(usize::MAX, |v| v.len()),
        });
        if let Some(tree) = value.as_tree() {
            let mut stack = vec![tree];
            while let Some(n) = stack.pop() {
                nodes += 1;
                if nodes > MAX_PLOT_EXPRESSION_NODES {
                    return Err("Plot expression aggregate node budget exceeded".into());
                }
                stack.extend(&n.children);
            }
        }
        if bytes > MAX_PLOT_EXPRESSION_BYTES {
            return Err("Plot expression aggregate byte budget exceeded".into());
        }
    }
    Ok(())
}

fn leaf(kind: NodeKind) -> PortableTree {
    PortableTree {
        kind,
        children: vec![],
    }
}
#[test]
fn receipt_preserves_exact_wire_and_old_validation_errors() {
    let mut corpus = vec![
        leaf(NodeKind::Placeholder),
        leaf(NodeKind::Number("-0.0".into())),
        leaf(NodeKind::NativeIdentifier {
            name: "𝛼&<\"x".into(),
            labels: "VARIABLE".into(),
            contextual: Some(false),
            subscript: Some("dc\n𝛽".into()),
        }),
    ];
    for n in [1, 8, 31, 32] {
        corpus.push(PortableTree {
            kind: NodeKind::Matrix {
                rows: n,
                cols: 32,
                display: None,
            },
            children: vec![leaf(NodeKind::Number("1".into())); n * 32],
        });
    }
    corpus.extend([
        leaf(NodeKind::Number("NaN".into())),
        leaf(NodeKind::Identifier("\u{0}".into())),
        leaf(NodeKind::Number("1".repeat(65537))),
        PortableTree {
            kind: NodeKind::Fraction,
            children: vec![],
        },
        PortableTree {
            kind: NodeKind::Binary(BinaryOp::Define),
            children: vec![leaf(NodeKind::Placeholder); 2],
        },
    ]);
    for tree in corpus {
        let old = old_validate_tree(&tree);
        let new = PlotExpression::from_tree(tree.clone());
        assert_eq!(old.as_ref().err(), new.as_ref().err());
        if let Ok(v) = new {
            let expected = format!("{{\"tree\":{}}}", serde_json::to_string(&tree).unwrap());
            assert_eq!(serde_json::to_string(&v).unwrap(), expected);
            assert_eq!(v.source_bytes(), expected.len());
            assert_eq!(v.validate(), old);
            let decoded: PlotExpression = serde_json::from_str(&expected).unwrap();
            assert_eq!(decoded, v);
            assert_eq!(decoded.source_bytes(), expected.len());
        }
    }
}
#[test]
fn aggregate_admission_same_error_order_and_mutable_legacy_guard() {
    let big = PlotExpression::from_tree(PortableTree {
        kind: NodeKind::Matrix {
            rows: 31,
            cols: 32,
            display: None,
        },
        children: vec![leaf(NodeKind::Number("1".into())); 992],
    })
    .unwrap();
    for n in 0..=16 {
        let values = vec![big.clone(); n];
        assert_eq!(expression_budget(&values), old_expression_budget(&values));
    }
    let escaped =
        PlotExpression::from_tree(leaf(NodeKind::Identifier("\"&<>𝛼".repeat(1000)))).unwrap();
    for n in 0..=16 {
        let values = vec![escaped.clone(); n];
        assert_eq!(expression_budget(&values), old_expression_budget(&values));
    }
    let mut mutable = PlotExpression::from("x");
    mutable
        .as_legacy_text_mut()
        .unwrap()
        .push_str(&"x".repeat(4096));
    for values in [
        vec![mutable.clone(), big.clone()],
        vec![big.clone(), mutable.clone()],
        vec![escaped, mutable, big],
    ] {
        assert_eq!(expression_budget(&values), old_expression_budget(&values));
    }
}

#[test]
fn untrusted_tree_wire_never_supplies_a_receipt_or_bypasses_grammar() {
    let value = PlotExpression::from_tree(leaf(NodeKind::Number("1".into()))).unwrap();
    let json = serde_json::to_string(&value).unwrap();
    for key in ["nodes", "source_bytes", "receipt", "future"] {
        let forged = json.replacen('{', &format!("{{\"{key}\":0,"), 1);
        assert!(serde_json::from_str::<PlotExpression>(&forged).is_err());
    }
    let duplicate = json.replacen('{', "{\"tree\":null,", 1);
    assert!(serde_json::from_str::<PlotExpression>(&duplicate).is_err());
    let statement = PortableTree {
        kind: NodeKind::Return,
        children: vec![leaf(NodeKind::Number("1".into()))],
    };
    assert_eq!(
        PlotExpression::from_tree(statement.clone()).unwrap_err(),
        old_validate_tree(&statement).unwrap_err()
    );
    assert!(
        serde_json::from_value::<PlotExpression>(serde_json::json!({"tree":statement})).is_err()
    );
    let mut deep = leaf(NodeKind::Placeholder);
    for _ in 0..200 {
        deep = PortableTree {
            kind: NodeKind::Group,
            children: vec![deep],
        };
    }
    assert_eq!(
        PlotExpression::from_tree(deep.clone()).as_ref().err(),
        old_validate_tree(&deep).as_ref().err()
    );
}

#[test]
fn original_expression_only_string_admission_is_an_explicit_extension_of_old_oracle() {
    // Keep old_validate_tree's original whitelist unchanged: the receipt work
    // preserved that boundary, and this independently evidenced feature fixes it.
    for text in ["", "123", " a + (b)=\" ", "\r\n\t😀&<>\\"] {
        let source = leaf(NodeKind::StringLiteral(text.into()));
        assert!(old_validate_tree(&source).is_err());
        let value = PlotExpression::from_tree(source.clone()).unwrap();
        let expected = format!("{{\"tree\":{}}}", serde_json::to_string(&source).unwrap());
        assert_eq!(serde_json::to_string(&value).unwrap(), expected);
        assert_eq!(value.source_bytes(), expected.len());
        assert!(value.validate().is_ok());
        let restored: PlotExpression = serde_json::from_str(&expected).unwrap();
        assert_eq!(restored, value);
        assert_eq!(restored.source_bytes(), expected.len());
        assert!(restored.is_complete());
        assert!(!restored.is_exact_placeholder());
        for key in ["nodes", "source_bytes", "receipt", "future"] {
            let forged = expected.replacen('{', &format!("{{\"{key}\":0,"), 1);
            assert!(serde_json::from_str::<PlotExpression>(&forged).is_err());
        }
    }
}
#[test]
fn node_budget_error_precedes_byte_error_at_same_expression() {
    let mut cell = leaf(NodeKind::Placeholder);
    for _ in 0..20 {
        cell = PortableTree {
            kind: NodeKind::Group,
            children: vec![cell],
        };
    }
    let source = PlotExpression::from_tree(PortableTree {
        kind: NodeKind::Matrix {
            rows: 1,
            cols: 31,
            display: None,
        },
        children: vec![cell; 31],
    })
    .unwrap();
    let values = vec![source; 16];
    let old = old_expression_budget(&values);
    assert_eq!(
        old,
        Err("Plot expression aggregate node budget exceeded".into())
    );
    assert_eq!(expression_budget(&values), old);
}
