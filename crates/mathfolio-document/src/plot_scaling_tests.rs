use super::*;
use crate::{
    canvas::{CanvasContent, CanvasDocument, PlotAxisEquation, PlotTraceAppearance},
    native_plot,
};
fn parse(xml: &str) -> PlotScaling {
    PlotScaling::from_tree(editor_bridge::tree_from_mathml(xml).unwrap())
        .unwrap()
        .unwrap()
}
fn unit(name: &str) -> PlotScaling {
    parse(&format!(r#"<ml:id labels="UNIT">{name}</ml:id>"#))
}
fn scale() -> PlotScaling {
    parse(r#"<ml:apply><ml:scale/><ml:real>0.1</ml:real><ml:id labels="UNIT">m</ml:id></ml:apply>"#)
}
fn spec() -> PlotSpec {
    PlotSpec {
        source: PlotSource::axis_lists(
            vec!["x".into()],
            vec!["x".into(), "2*x".into()],
            vec![super::super::PlotTraceAppearance::default(); 2],
        )
        .unwrap(),
        ..Default::default()
    }
}
#[test]
fn typed_source_keeps_explicit_contextual_labels_and_literal_subscripts_without_plain_parse() {
    let xmls = [
        r#"<ml:id labels="UNIT">m</ml:id>"#,
        r#"<ml:id labels="VARIABLE">m</ml:id>"#,
        r#"<ml:id labels="CONSTANT">m</ml:id>"#,
        r#"<ml:id labels="UNIT" label-is-contextual="true">m</ml:id>"#,
        r#"<ml:id labels="UNIT" label-is-contextual="false">m</ml:id>"#,
        r#"<ml:id labels="VARIABLE"><p:Span xmlns:p="http://schemas.microsoft.com/winfx/2006/xaml/presentation">V<ptc:Subscript xmlns:ptc="clr-namespace:Ptc.Wpf;assembly=Ptc.Core">dc𝛼</ptc:Subscript></p:Span></ml:id>"#,
    ];
    let values: Vec<_> = xmls.into_iter().map(parse).collect();
    for value in &values {
        let json = serde_json::to_string(value).unwrap();
        let restored: PlotScaling = serde_json::from_str(&json).unwrap();
        assert_eq!(&restored, value);
        let native = editor_bridge::tree_to_mathml(value.as_tree()).unwrap();
        assert_eq!(parse(&native), *value);
    }
    for (i, left) in values.iter().enumerate() {
        for right in values.iter().skip(i + 1) {
            assert_ne!(left, right);
        }
    }
    assert!(
        editor_bridge::tree_to_mathml(values.last().unwrap().as_tree())
            .unwrap()
            .contains("dc𝛼")
    );
}
#[test]
fn blank_default_explicit_one_and_incomplete_source_are_distinct_and_wire_is_strict() {
    let ph = PortableTree {
        kind: NodeKind::Placeholder,
        children: vec![],
    };
    assert!(PlotScaling::from_tree(ph.clone()).unwrap().is_none());
    assert!(serde_json::from_str::<PlotScaling>(&serde_json::to_string(&ph).unwrap()).is_err());
    let one = parse("<ml:real>1</ml:real>");
    assert!(one.is_complete());
    let incomplete = PlotScaling::from_tree(PortableTree {
        kind: NodeKind::Binary(BinaryOp::Scale),
        children: vec![one.as_tree().clone(), ph],
    })
    .unwrap()
    .unwrap();
    assert!(!incomplete.is_complete());
    for json in [
        r#"{"id":1,"source":"x","scaling":null}"#,
        r#"{"id":1,"source":"x","scaling":{"kind":"Number","children":[],"kind":"Placeholder"}}"#,
        r#"{"id":1,"source":"x","scaling":{"kind":{"Number":"1"},"children":[],"extra":true}}"#,
    ] {
        assert!(serde_json::from_str::<PlotAxisEquation>(json).is_err());
    }
    let json = serde_json::to_string(&PlotSpec::default()).unwrap();
    assert!(!json.contains("scaling"));
    for prefix in [
        r#""x_scaling":null,"#,
        r#""x_scaling":{"kind":{"Number":"1"},"children":[]},"x_scaling":{"kind":{"Number":"2"},"children":[]},"#,
    ] {
        assert!(serde_json::from_str::<PlotSpec>(&format!("{{{prefix}{}", &json[1..])).is_err());
    }
    let mut scaling_spec = spec();
    scaling_spec
        .set_axis_scaling(PlotAxis::Y, 0, Some(one))
        .unwrap();
    let json = serde_json::to_string(&scaling_spec).unwrap();
    assert!(json.contains("scaling"));
    assert_eq!(
        serde_json::from_str::<PlotSpec>(&json).unwrap(),
        scaling_spec
    );
}
#[test]
fn shared_borrowed_pair_keeps_single_x_authority_and_signatures_include_scaling() {
    let mut source = spec();
    let before = source.clone();
    source
        .set_axis_scaling(PlotAxis::X, 0, Some(unit("m")))
        .unwrap();
    source
        .set_axis_scaling(PlotAxis::Y, 0, Some(unit("in")))
        .unwrap();
    source
        .set_axis_scaling(PlotAxis::Y, 1, Some(scale()))
        .unwrap();
    let pairs: Vec<_> = source.paired_sources().collect();
    assert!(std::ptr::eq(
        pairs[0].x.scaling.unwrap(),
        pairs[1].x.scaling.unwrap()
    ));
    assert_ne!(pairs[0].y.scaling, pairs[1].y.scaling);
    assert_eq!(source.equation_sources(true).count(), 1);
    assert_eq!(source.equation_sources(false).count(), 2);
    let mut doc = CanvasDocument::default();
    let id = doc.add_plot(0., 0.);
    doc.regions[0].content = CanvasContent::Plot(before);
    let old = doc.calculation_signature();
    doc.regions[0].content = CanvasContent::Plot(source.clone());
    assert_ne!(doc.calculation_signature(), old);
    let old = doc.calculation_signature();
    let mut changed = source.clone();
    changed
        .set_axis_scaling(PlotAxis::Y, 1, Some(unit("m")))
        .unwrap();
    doc.regions.iter_mut().find(|r| r.id == id).unwrap().content = CanvasContent::Plot(changed);
    assert_ne!(doc.calculation_signature(), old);
    let resolved = source.resolved_traces().unwrap();
    assert_eq!(resolved[0].x_scaling, resolved[1].x_scaling);
    assert_ne!(resolved[0].y_scaling, resolved[1].y_scaling);
}
#[test]
fn legacy_fallback_promote_migrate_and_axis_remove_keep_scaling_with_source_identity() {
    let mut source = PlotSpec::default();
    source
        .set_axis_scaling(PlotAxis::X, 0, Some(unit("m")))
        .unwrap();
    source
        .set_axis_scaling(PlotAxis::Y, 0, Some(scale()))
        .unwrap();
    source.promote_legacy_trace();
    assert_eq!(source.legacy_traces().unwrap()[0].y_scaling, Some(scale()));
    source
        .add_trace(PlotTraceSpec {
            x_expression: "x".into(),
            y_expression: "sin(x)".into(),
            x_scaling: Some(unit("in")),
            y_scaling: Some(unit("m")),
            ..Default::default()
        })
        .unwrap();
    let json = serde_json::to_string(&source).unwrap();
    assert_eq!(serde_json::from_str::<PlotSpec>(&json).unwrap(), source);
    let pairs = source.resolved_traces().unwrap();
    source.promote_axis_lists().unwrap();
    let ids: Vec<_> = source
        .axis_lists()
        .unwrap()
        .x
        .iter()
        .chain(&source.axis_lists().unwrap().y)
        .map(|e| e.id)
        .collect();
    assert_eq!(source.resolved_traces().unwrap(), pairs);
    source
        .insert_axis_equation(PlotAxis::Y, 1, "", Some(PlotTraceAppearance::default()))
        .unwrap();
    assert!(source.axis_lists().unwrap().y[1].scaling.is_none());
    let removed = source.remove_axis_equation(PlotAxis::Y, 0).unwrap();
    let super::super::PlotAxisEquationChange::Removed { equation } = removed.change else {
        panic!()
    };
    assert_eq!(equation.id, ids[2]);
    assert_eq!(equation.scaling, Some(scale()));
    assert_eq!(source.axis_lists().unwrap().y[1].scaling, Some(unit("m")));
    let before = source.clone();
    assert!(
        !source
            .set_axis_scaling(PlotAxis::Y, 1, Some(unit("m")))
            .unwrap()
    );
    assert_eq!(source, before);
    assert!(source.set_axis_scaling(PlotAxis::Y, 99, None).is_err());
    assert_eq!(source, before);
}
fn balanced(depth: usize) -> PortableTree {
    if depth == 0 {
        PortableTree {
            kind: NodeKind::Number("1".into()),
            children: vec![],
        }
    } else {
        PortableTree {
            kind: NodeKind::Binary(BinaryOp::Add),
            children: vec![balanced(depth - 1), balanced(depth - 1)],
        }
    }
}
#[test]
fn statements_and_scaling_budgets_fail_before_atomic_source_install() {
    for op in [BinaryOp::Define, BinaryOp::GlobalDefine, BinaryOp::Evaluate] {
        assert!(
            PlotScaling::from_tree(PortableTree {
                kind: NodeKind::Binary(op),
                children: vec![balanced(0), balanced(0)]
            })
            .is_err()
        );
    }
    assert!(PlotScaling::from_tree(balanced(10)).is_err());
    let large = PlotScaling::from_tree(balanced(8)).unwrap().unwrap();
    let mut source = PlotSpec {
        source: PlotSource::axis_lists(
            vec!["x".into(); 16],
            vec!["x".into(); 16],
            vec![PlotTraceAppearance::default(); 16],
        )
        .unwrap(),
        ..Default::default()
    };
    for i in 0..16 {
        let before = source.clone();
        if source
            .set_axis_scaling(PlotAxis::X, i, Some(large.clone()))
            .is_err()
        {
            assert_eq!(source, before);
            assert!(i > 0);
            return;
        }
    }
    panic!("aggregate scaling budget did not reject expanded source");
}
// These three Math source fragments reproduce installed help examples, not a
// captured result payload or a complete supported help plot. 0170 retains the
// full help hash and original regions ID0ENFXJB / ID0E3HXJB / ID0ENJXJB.
const HELP_SCALINGS: [&str; 3] = [
    r#"<ml:id labels="UNIT" xml:space="preserve">m</ml:id>"#,
    r#"<ml:id labels="UNIT" xml:space="preserve">in</ml:id>"#,
    r#"<ml:apply><ml:scale/><ml:real>0.1</ml:real><ml:id labels="UNIT" xml:space="preserve">m</ml:id></ml:apply>"#,
];
fn region(payload: &str) -> String {
    format!(
        r#"<region xmlns="http://schemas.mathsoft.com/worksheet50" region-id="1" left="0" top="0">{payload}</region>"#
    )
}
#[test]
fn literal_help_math_source_roundtrips_in_separate_minimal_native_fixtures() {
    for fragment in HELP_SCALINGS {
        let value = parse(fragment);
        let mut source = PlotSpec::default();
        source
            .set_axis_scaling(PlotAxis::Y, 0, Some(value.clone()))
            .unwrap();
        let xml = native_plot::payload(&source).unwrap();
        let input = region(&xml);
        let parsed = native_plot::parse(&input).unwrap();
        assert_eq!(
            parsed.paired_sources().next().unwrap().y.scaling,
            Some(&value)
        );
        assert!(native_plot::matches_retained_source(&source, &parsed).unwrap());
        assert_eq!(xml.matches("<plotEquation>").count(), 2);
        let again = native_plot::parse(&region(&native_plot::payload(&parsed).unwrap())).unwrap();
        assert!(native_plot::same_source(&parsed, &again).unwrap());
    }
    let mut source = spec();
    source
        .set_axis_scaling(PlotAxis::X, 0, Some(unit("m")))
        .unwrap();
    source
        .set_axis_scaling(PlotAxis::Y, 0, Some(unit("in")))
        .unwrap();
    source
        .set_axis_scaling(PlotAxis::Y, 1, Some(scale()))
        .unwrap();
    let parsed = native_plot::parse(&region(&native_plot::payload(&source).unwrap())).unwrap();
    assert_eq!(parsed.axis_lists().unwrap().x.len(), 1);
    assert_eq!(parsed.axis_lists().unwrap().y.len(), 2);
    assert_eq!(
        parsed.equation_sources(false).nth(1).unwrap().scaling,
        Some(&scale())
    );
}
#[test]
fn scaling_native_guard_keeps_unknown_formatting_raw_and_does_not_erase_labels() {
    let mut source = PlotSpec::default();
    source
        .set_axis_scaling(PlotAxis::Y, 0, Some(unit("m")))
        .unwrap();
    let xml = region(&native_plot::payload(&source).unwrap());
    for changed in [
        xml.replace("</ml:id></math>", "</ml:id><formattingOverride/></math>"),
        xml.replace("labels=\"UNIT\"", "labels=\"UNIT\" future=\"lost\""),
        xml.replace(">m</ml:id>", ">m<!--lost-->x</ml:id>"),
    ] {
        assert!(native_plot::parse(&changed).is_err(), "{changed}");
    }
    let variable = xml.replace("labels=\"UNIT\"", "labels=\"VARIABLE\"");
    let variable = native_plot::parse(&variable).unwrap();
    let original = native_plot::parse(&xml).unwrap();
    assert!(!native_plot::same_source(&original, &variable).unwrap());
}
#[test]
#[ignore = "writes independently assembled source-only fixtures for external XSD validation"]
fn scaling_source_schema_fixtures() {
    let directory =
        std::env::var("MATHFOLIO_SCALING_FIXTURES").expect("MATHFOLIO_SCALING_FIXTURES");
    std::fs::create_dir_all(&directory).unwrap();
    for (i, fragment) in HELP_SCALINGS.into_iter().enumerate() {
        let mut source = PlotSpec::default();
        source
            .set_axis_scaling(PlotAxis::Y, 0, Some(parse(fragment)))
            .unwrap();
        let payload = native_plot::payload(&source).unwrap();
        let xml = format!(
            r#"<worksheet xmlns="http://schemas.mathsoft.com/worksheet50"><regions><region region-id="1" left="0" top="0">{payload}</region></regions></worksheet>"#
        );
        std::fs::write(
            std::path::Path::new(&directory).join(format!("help-scaling-{i}.xml")),
            xml,
        )
        .unwrap();
    }
}

#[test]
fn absent_scaling_keeps_frozen_legacy_trace_wire_exactly() {
    assert_eq!(
        serde_json::to_string(&PlotTraceSpec::default()).unwrap(),
        r#"{"x_expression":"x","y_expression":"sin(x)","label":"","style":{"color":[0,70,150,255],"line":"Solid","line_width":1.5,"marker":"None","marker_filled":false,"marker_size":3.0,"marker_frequency":1}}"#
    );
}
