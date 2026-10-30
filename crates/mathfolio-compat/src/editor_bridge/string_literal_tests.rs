use super::*;
#[test]
fn source_string_preserves_empty_whitespace_quotes_entities_and_supplementary_text() {
    for text in [
        "",
        "  한글 + 1 = \"A\"  ",
        "<&>\r\n\t𝛼😀",
        "mathcraft-tree:{\"kind\":\"Number\"}",
    ] {
        let source = tree(NodeKind::StringLiteral(text.into()), vec![]);
        let xml = tree_to_mathml(&source).unwrap();
        assert_eq!(tree_from_mathml(&xml).unwrap(), source);
        assert_eq!(
            serialization_writer::serialize(&source, true, 0, None).unwrap(),
            serialize(&source, true, 0).unwrap()
        );
        let wrapped = tree(
            NodeKind::Binary(BinaryOp::Define),
            vec![tree(NodeKind::Identifier("s".into()), vec![]), source],
        );
        let labelled =
            label_fragment(&tree_to_mathml(&wrapped).unwrap(), &mut HashMap::new()).unwrap();
        assert!(labelled.contains("<ml:str"));
    }
}
#[test]
fn unknown_string_metadata_and_embedded_structures_are_rejected_without_text_projection() {
    for payload in [
        "<ml:str future='1'>a</ml:str>",
        "<ml:str xml:space='default'>a</ml:str>",
        "<ml:str>a<!--hidden-->b</ml:str>",
        "<ml:str>a<?x y?>b</ml:str>",
        "<ml:str>a<ml:id>x</ml:id>b</ml:str>",
    ] {
        assert!(tree_from_mathml(payload).is_err(), "{payload}");
    }
}
