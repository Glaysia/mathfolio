use mathfolio_compat::editor_bridge::{tree_from_mathml, tree_to_mathml};
use mathfolio_editor::{Editor, NodeKind, PortableTree};

#[test]
fn native_structural_operators_roundtrip_exact_names_arity_and_operand_order() {
    for (tag, kind, arity) in [
        ("factorial", NodeKind::Factorial, 1),
        ("transpose", NodeKind::Transpose, 1),
        ("conjugate", NodeKind::Conjugate, 1),
        ("determinant", NodeKind::Determinant, 1),
        ("matrow", NodeKind::Row, 2),
        ("matcol", NodeKind::Column, 2),
        ("absval", NodeKind::Absolute, 1),
    ] {
        let source = format!(
            "<ml:apply xmlns:ml=\"http://schemas.mathsoft.com/math50\"><ml:{tag}/><ml:id labels=\"VARIABLE\" label-is-contextual=\"false\">A</ml:id>{}</ml:apply>",
            if arity == 2 {
                "<ml:real>2</ml:real>"
            } else {
                ""
            }
        );
        let tree = tree_from_mathml(&source).unwrap();
        assert_eq!(tree.kind, kind);
        assert_eq!(tree.children.len(), arity);
        let xml = tree_to_mathml(&tree).unwrap();
        assert!(xml.contains(&format!("<ml:{tag}/>")), "{xml}");
        assert_eq!(tree_from_mathml(&xml).unwrap(), tree);
        assert!(
            matches!(&tree.children[0].kind,NodeKind::NativeIdentifier{name,labels,contextual:Some(false),..} if name=="A"&&labels=="VARIABLE")
        );
        if arity == 2 {
            assert_eq!(tree.children[1].kind, NodeKind::Number("2".into()));
        }
    }
}

#[test]
fn native_operator_parser_rejects_unknown_attributes_children_namespaces_and_fake_norm() {
    for body in [
        "<ml:factorial changed=\"true\"/><ml:real>3</ml:real>",
        "<ml:transpose/><ml:real>2</ml:real><ml:real>3</ml:real>",
        "<ml:conjugate><ml:real>2</ml:real></ml:conjugate><ml:real>3</ml:real>",
        "<ml:determinant/>",
        "<ml:matrow/><ml:id>A</ml:id>",
        "<ml:matcol/><ml:id>A</ml:id><ml:sequence><ml:real>0</ml:real></ml:sequence>",
        "<ml:norm/><ml:id>A</ml:id>",
        "<transpose xmlns=\"urn:foreign\"/><ml:id>A</ml:id>",
    ] {
        let xml =
            format!("<ml:apply xmlns:ml=\"http://schemas.mathsoft.com/math50\">{body}</ml:apply>");
        assert!(tree_from_mathml(&xml).is_err(), "{xml}");
    }
}

#[test]
fn original_norm_and_absolute_determinant_commands_have_distinct_existing_native_identities() {
    let source = PortableTree {
        kind: NodeKind::Identifier("A".into()),
        children: vec![],
    };
    for (command, tag, kind) in [
        (
            Editor::insert_determinant as fn(&mut Editor) -> mathfolio_editor::NodeId,
            "absval",
            NodeKind::Absolute,
        ),
        (Editor::insert_norm, "determinant", NodeKind::Determinant),
    ] {
        let mut editor = Editor::from_portable(&source).unwrap();
        command(&mut editor);
        let tree = editor.to_portable().unwrap();
        assert_eq!(tree.kind, kind);
        let xml = tree_to_mathml(&tree).unwrap();
        assert!(xml.contains(&format!("<ml:{tag}/>")), "{xml}");
        assert_eq!(tree_from_mathml(&xml).unwrap(), tree);
    }
}

#[test]
fn a_user_function_with_the_same_name_remains_a_function_call() {
    for name in [
        "factorial",
        "transpose",
        "conjugate",
        "determinant",
        "matrow",
        "matcol",
        "norm",
    ] {
        let source = format!(
            "<ml:apply xmlns:ml=\"http://schemas.mathsoft.com/math50\"><ml:id labels=\"VARIABLE\">{name}</ml:id><ml:real>3</ml:real></ml:apply>"
        );
        let tree = tree_from_mathml(&source).unwrap();
        assert_eq!(tree.kind, NodeKind::Call { argument_count: 1 });
        assert_eq!(
            tree_from_mathml(&tree_to_mathml(&tree).unwrap()).unwrap(),
            tree
        );
    }
}
