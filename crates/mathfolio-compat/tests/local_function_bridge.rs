use mathfolio_compat::editor_bridge::{tree_from_mathml, tree_to_mathml, worksheet_xml};
use mathfolio_editor::{NodeKind, PortableTree};

const ML: &str = "http://schemas.mathsoft.com/math50";
fn parse(xml: &str) -> Result<PortableTree, String> {
    tree_from_mathml(&format!("<context xmlns:ml='{ML}'>{xml}</context>"))
}

#[test]
fn local_function_declaration_roundtrips_native_function_and_identifier_metadata() {
    let source = "<ml:program><ml:localDefine><ml:function><ml:id labels='VARIABLE' label-is-contextual='false'><Span xmlns='http://schemas.microsoft.com/winfx/2006/xaml/presentation'>f<Subscript xmlns='clr-namespace:Ptc.Wpf;assembly=Ptc.Core'>local</Subscript></Span></ml:id><ml:boundVars><ml:id labels='UNIT' label-is-contextual='true'>m</ml:id><ml:id labels='VARIABLE'>n</ml:id></ml:boundVars></ml:function><ml:apply><ml:plus/><ml:id>m</ml:id><ml:id>n</ml:id></ml:apply></ml:localDefine><ml:real>7</ml:real></ml:program>";
    let tree = parse(source).unwrap();
    assert_eq!(
        tree.children[0].children[0].kind,
        NodeKind::Function { parameter_count: 2 }
    );
    let native = tree_to_mathml(&tree).unwrap();
    assert!(native.contains("<ml:localDefine><ml:function>"));
    assert!(native.contains("<ml:boundVars>"));
    assert_eq!(tree_from_mathml(&native).unwrap(), tree);
    assert!(tree_to_mathml(&tree.children[0]).is_err());
}

#[test]
fn local_function_writer_rejects_incomplete_formals_without_confusing_calls_or_loop_targets() {
    let source = "<ml:program><ml:localDefine><ml:function><ml:id>f</ml:id><ml:boundVars><ml:id>x</ml:id></ml:boundVars></ml:function><ml:id>x</ml:id></ml:localDefine></ml:program>";
    let mut draft = parse(source).unwrap();
    draft.children[0].children[0].children[1] = PortableTree {
        kind: NodeKind::Placeholder,
        children: vec![],
    };
    assert!(draft.validate().is_ok());
    assert!(tree_to_mathml(&draft).is_err());
    for target in [
        "<ml:apply><ml:id>f</ml:id><ml:id>x</ml:id></ml:apply>",
        "<ml:function><ml:real>2</ml:real><ml:boundVars><ml:id>x</ml:id></ml:boundVars></ml:function>",
        "<ml:function><ml:id>f</ml:id><ml:boundVars><ml:real>2</ml:real></ml:boundVars></ml:function>",
        "<ml:function><ml:id>f</ml:id><ml:boundVars><ml:placeholder/></ml:boundVars></ml:function>",
    ] {
        assert!(parse(&format!("<ml:program><ml:localDefine>{target}<ml:real>3</ml:real></ml:localDefine></ml:program>")).is_err(), "{target}");
    }
}

#[test]
fn local_function_formals_shadow_only_their_assignment_body_and_keep_outer_units() {
    let source = "<ml:define><ml:id>answer</ml:id><ml:apply><ml:plus/><ml:program><ml:localDefine><ml:function><ml:id>f</ml:id><ml:boundVars><ml:id labels='UNIT' label-is-contextual='true'>m</ml:id></ml:boundVars></ml:function><ml:id labels='UNIT' label-is-contextual='true'>m</ml:id></ml:localDefine><ml:localDefine><ml:function><ml:id>h</ml:id><ml:boundVars><ml:id>s</ml:id></ml:boundVars></ml:function><ml:apply><ml:plus/><ml:id>m</ml:id><ml:id>s</ml:id></ml:apply></ml:localDefine><ml:id>m</ml:id></ml:program><ml:id>m</ml:id></ml:apply></ml:define>";
    let xml = worksheet_xml(&[(1, 0., 0., source.into())]).unwrap();
    let doc = roxmltree::Document::parse(&xml).unwrap();
    let labels: Vec<_> = doc
        .descendants()
        .filter(|n| n.has_tag_name((ML, "id")) && n.text() == Some("m"))
        .map(|n| n.attribute("labels").unwrap())
        .collect();
    assert_eq!(labels, ["VARIABLE", "VARIABLE", "UNIT", "UNIT", "UNIT"]);
    let s_labels: Vec<_> = doc
        .descendants()
        .filter(|n| n.has_tag_name((ML, "id")) && n.text() == Some("s"))
        .map(|n| n.attribute("labels").unwrap())
        .collect();
    assert_eq!(s_labels, ["VARIABLE", "VARIABLE"]);
}

#[test]
fn ordinary_worksheet_function_and_calculus_binder_scopes_still_work() {
    let source = "<ml:define><ml:function><ml:id>outer</ml:id><ml:boundVars><ml:id>m</ml:id></ml:boundVars></ml:function><ml:program><ml:localDefine><ml:function><ml:id>inner</ml:id><ml:boundVars><ml:id>s</ml:id></ml:boundVars></ml:function><ml:apply><ml:plus/><ml:id>m</ml:id><ml:id>s</ml:id></ml:apply></ml:localDefine><ml:id>m</ml:id></ml:program></ml:define>";
    let tree = parse(source).unwrap();
    assert_eq!(
        tree_from_mathml(&tree_to_mathml(&tree).unwrap()).unwrap(),
        tree
    );
    let xml = worksheet_xml(&[(1, 0., 0., source.into())]).unwrap();
    let doc = roxmltree::Document::parse(&xml).unwrap();
    assert!(
        doc.descendants()
            .filter(|n| n.has_tag_name((ML, "id")) && n.text() == Some("m"))
            .all(|n| n.attribute("labels") == Some("VARIABLE"))
    );
    let calculus = "<ml:eval><ml:apply><ml:plus/><ml:apply><ml:summation/><ml:lambda><ml:boundVars><ml:id>s</ml:id></ml:boundVars><ml:id>s</ml:id></ml:lambda><ml:lowerBound><ml:real>1</ml:real></ml:lowerBound><ml:upperBound><ml:real>2</ml:real></ml:upperBound></ml:apply><ml:id>s</ml:id></ml:apply><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>";
    let xml = worksheet_xml(&[(2, 0., 0., calculus.into())]).unwrap();
    let doc = roxmltree::Document::parse(&xml).unwrap();
    let labels: Vec<_> = doc
        .descendants()
        .filter(|n| n.has_tag_name((ML, "id")) && n.text() == Some("s"))
        .map(|n| n.attribute("labels").unwrap())
        .collect();
    assert_eq!(labels, ["VARIABLE", "VARIABLE", "UNIT"]);
}
