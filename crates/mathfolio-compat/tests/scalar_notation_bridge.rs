use mathfolio_compat::editor_bridge::{tree_from_mathml, tree_to_mathml};
use mathfolio_editor::{BinaryOp, NodeKind};

fn parse(source: &str) -> Result<mathfolio_editor::PortableTree, String> {
    tree_from_mathml(&format!(
        "<context xmlns:ml=\"http://schemas.mathsoft.com/math50\">{source}</context>"
    ))
}

#[test]
fn original_percent_degree_and_polar_opcodes_roundtrip_without_result_format_state() {
    for (source, kind, opcode) in [
        (
            "<ml:apply><ml:percent/><ml:id labels=\"VARIABLE\" label-is-contextual=\"false\">A</ml:id></ml:apply>",
            NodeKind::Percent,
            "percent",
        ),
        (
            "<ml:apply><ml:degree_op/><ml:real>90</ml:real></ml:apply>",
            NodeKind::Degree,
            "degree_op",
        ),
        (
            "<ml:apply><ml:polar/><ml:real>2</ml:real><ml:apply><ml:degree_op/><ml:real>90</ml:real></ml:apply></ml:apply>",
            NodeKind::Binary(BinaryOp::Polar),
            "polar",
        ),
    ] {
        let tree = parse(source).unwrap();
        assert_eq!(tree.kind, kind);
        let native = tree_to_mathml(&tree).unwrap();
        assert!(native.contains(&format!("<ml:{opcode}/>")));
        assert!(!native.contains("resultFormat"));
        assert_eq!(tree_from_mathml(&native).unwrap(), tree);
    }
}

#[test]
fn operator_arity_attributes_and_invented_dms_operator_are_rejected() {
    for source in [
        "<ml:apply><ml:percent/></ml:apply>",
        "<ml:apply><ml:percent/><ml:real>2</ml:real><ml:real>3</ml:real></ml:apply>",
        "<ml:apply><ml:degree_op/><ml:real>2</ml:real><ml:real>3</ml:real></ml:apply>",
        "<ml:apply><ml:polar/><ml:real>2</ml:real></ml:apply>",
        "<ml:apply><ml:polar mode=\"degrees\"/><ml:real>2</ml:real><ml:real>3</ml:real></ml:apply>",
        "<ml:apply><ml:degree/><ml:real>2</ml:real></ml:apply>",
        "<ml:apply><ml:dms/><ml:real>2</ml:real></ml:apply>",
    ] {
        assert!(parse(source).is_err(), "{source}");
    }
}

#[test]
fn notation_syntax_does_not_relabel_percent_or_degree_as_unit_names() {
    let expression=parse("<ml:apply><ml:plus/><ml:apply><ml:percent/><ml:id labels=\"VARIABLE\"><p:Span xmlns:p=\"http://schemas.microsoft.com/winfx/2006/xaml/presentation\">m<ptc:Subscript xmlns:ptc=\"clr-namespace:Ptc.Wpf;assembly=Ptc.Core\">a</ptc:Subscript></p:Span></ml:id></ml:apply><ml:apply><ml:degree_op/><ml:id labels=\"UNIT\">m</ml:id></ml:apply></ml:apply>").unwrap();
    let native = tree_to_mathml(&expression).unwrap();
    let roundtrip = tree_from_mathml(&native).unwrap();
    assert_eq!(expression, roundtrip);
    assert!(native.contains("labels=\"VARIABLE\""));
    assert!(native.contains("labels=\"UNIT\""));
    assert!(native.contains(">a</ptc:Subscript>"));
}
