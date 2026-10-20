use mathcraft_compat::editor_bridge::{tree_from_mathml, tree_to_mathml, worksheet_xml};
use mathcraft_editor::{BinaryOp, NodeKind, PortableTree};
const ML: &str = "http://schemas.mathsoft.com/math50";
fn parse(source: &str) -> Result<PortableTree, String> {
    tree_from_mathml(&format!("<context xmlns:ml=\"{ML}\">{source}</context>"))
}

#[test]
fn vectorize_and_cross_product_are_native_operators_with_exact_arity_and_roundtrip() {
    for (source, kind) in [
        (
            "<ml:apply><ml:vectorize/><ml:apply><ml:mult/><ml:id>A</ml:id><ml:id>B</ml:id></ml:apply></ml:apply>",
            NodeKind::Vectorize,
        ),
        (
            "<ml:apply><ml:crossProduct/><ml:id>A</ml:id><ml:id>B</ml:id></ml:apply>",
            NodeKind::Binary(BinaryOp::CrossProduct),
        ),
    ] {
        let tree = parse(source).unwrap();
        assert_eq!(tree.kind, kind);
        let native = tree_to_mathml(&tree).unwrap();
        assert_eq!(tree_from_mathml(&native).unwrap(), tree);
    }
    for source in [
        "<ml:apply vectorize=\"true\"><ml:mult/><ml:id>A</ml:id><ml:id>B</ml:id></ml:apply>",
        "<ml:apply><ml:vectorize/><ml:id>A</ml:id><ml:id>B</ml:id></ml:apply>",
        "<ml:apply><ml:vectorize mode=\"all\"/><ml:id>A</ml:id></ml:apply>",
        "<ml:apply><ml:crossProduct/><ml:id>A</ml:id></ml:apply>",
    ] {
        assert!(parse(source).is_err(), "{source}");
    }
}

#[test]
fn global_variable_function_and_indexed_definitions_roundtrip_with_rhs_evaluation() {
    for source in [
        "<ml:globalDefine><ml:id labels=\"VARIABLE\">x</ml:id><ml:real>3</ml:real></ml:globalDefine>",
        "<ml:globalDefine><ml:function><ml:id>f</ml:id><ml:boundVars><ml:id>x</ml:id></ml:boundVars></ml:function><ml:apply><ml:plus/><ml:id>x</ml:id><ml:real>1</ml:real></ml:apply></ml:globalDefine>",
        "<ml:globalDefine><ml:apply><ml:indexer/><ml:id>A</ml:id><ml:real>2</ml:real></ml:apply><ml:eval><ml:real>7</ml:real><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval></ml:globalDefine>",
    ] {
        let tree = parse(source).unwrap();
        assert_eq!(tree.kind, NodeKind::Binary(BinaryOp::GlobalDefine));
        let native = tree_to_mathml(&tree).unwrap();
        assert!(native.contains("<ml:globalDefine"));
        assert!(!native.contains("<ml:define"));
        assert_eq!(tree_from_mathml(&native).unwrap(), tree);
    }
}

#[test]
fn nested_global_definitions_and_non_rhs_evaluation_are_rejected() {
    for source in [
        "<ml:apply><ml:vectorize/><ml:globalDefine><ml:id>x</ml:id><ml:real>3</ml:real></ml:globalDefine></ml:apply>",
        "<ml:program><ml:globalDefine><ml:id>x</ml:id><ml:real>3</ml:real></ml:globalDefine></ml:program>",
        "<ml:globalDefine><ml:id>x</ml:id><ml:apply><ml:plus/><ml:eval><ml:real>3</ml:real><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval><ml:real>2</ml:real></ml:apply></ml:globalDefine>",
    ] {
        assert!(parse(source).is_err(), "{source}");
    }
    let global =
        parse("<ml:globalDefine><ml:id>x</ml:id><ml:real>3</ml:real></ml:globalDefine>").unwrap();
    assert!(
        tree_to_mathml(&PortableTree {
            kind: NodeKind::Vectorize,
            children: vec![global]
        })
        .is_err()
    );
}

#[test]
fn global_labels_are_available_before_their_region_without_relabeling_explicit_units() {
    let fragments = [
        "<ml:eval><ml:id>m</ml:id><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>",
        "<ml:globalDefine><ml:id>m</ml:id><ml:apply><ml:scale/><ml:real>2</ml:real><ml:id labels=\"UNIT\">m</ml:id></ml:apply></ml:globalDefine>",
    ];
    let rows: Vec<_> = fragments
        .iter()
        .enumerate()
        .map(|(i, s)| {
            (
                i as u64 + 1,
                0.,
                i as f64 * 40.,
                tree_to_mathml(&parse(s).unwrap()).unwrap(),
            )
        })
        .collect();
    let xml = worksheet_xml(&rows).unwrap();
    let rows = mathcraft_compat::evaluate_worksheet(&xml).unwrap();
    assert!(rows.iter().all(|r| r.error.is_none()), "{rows:?}\n{xml}");
    let Some(mathcraft_compat::Value::Scalar { quantity }) = rows[0].value else {
        panic!("{rows:?}")
    };
    assert_eq!(quantity.value.re, 2.);
    assert_eq!(
        quantity.dimension,
        mathcraft_core::unit("m").unwrap().dimension
    );
}
