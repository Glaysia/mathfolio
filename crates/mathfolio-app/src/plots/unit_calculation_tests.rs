//! Physical axis conversion, source binding and old unitless behavior.
use super::*;
use mathfolio_document::canvas::{PlotAxis, PlotScaling};
fn scale(xml: &str) -> PlotScaling {
    PlotScaling::from_tree(editor_bridge::tree_from_mathml(xml).unwrap())
        .unwrap()
        .unwrap()
}
fn unit(name: &str) -> PlotScaling {
    scale(&format!("<ml:id labels='UNIT'>{name}</ml:id>"))
}
fn spec() -> PlotSpec {
    PlotSpec {
        source: PlotSource::single("x", "x^2"),
        ..Default::default()
    }
}
fn prior() -> Vec<(u64, f64, f64, String)> {
    vec![(1,0.,0.,"<ml:define><ml:id>x</ml:id><ml:matrix rows='3' cols='1'><ml:apply><ml:mult/><ml:real>1</ml:real><ml:id labels='UNIT'>m</ml:id></ml:apply><ml:apply><ml:mult/><ml:real>2</ml:real><ml:id labels='UNIT'>m</ml:id></ml:apply><ml:apply><ml:mult/><ml:real>3</ml:real><ml:id labels='UNIT'>m</ml:id></ml:apply></ml:matrix></ml:define>".into())]
}
fn run(spec: &PlotSpec, prior: &[(u64, f64, f64, String)]) -> PlotData {
    evaluate(spec, prior, 999, 0., 100.).unwrap()
}
fn curve(data: &PlotData) -> &PlotTraceData {
    match &data.traces[0] {
        TraceOutcome::Ready(c) => c,
        x => panic!("{x:?}"),
    }
}
fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1e-10,
        "{actual} versus {expected}"
    );
}
#[test]
fn physical_pointwise_substitution_retains_squared_dimension_and_explicit_linear_scales() {
    let mut spec = spec();
    spec.set_axis_scaling(PlotAxis::X, 0, Some(unit("in")))
        .unwrap();
    spec.set_axis_scaling(
        PlotAxis::Y,
        0,
        Some(scale(
            "<ml:apply><ml:pow/><ml:id labels='UNIT'>m</ml:id><ml:real>2</ml:real></ml:apply>",
        )),
    )
    .unwrap();
    let data = run(&spec, &prior());
    let c = curve(&data);
    for (i, point) in c.points.iter().enumerate() {
        let n = (i + 1) as f64;
        close(point[0], n / 0.0254);
        close(point[1], n * n);
    }
    let p = c.presentation.as_ref().unwrap();
    assert!(p.x.as_ref().unwrap().residual_units().is_none());
    assert!(p.y.as_ref().unwrap().residual_units().is_none());
    let saved = data.to_saved();
    assert!(saved.contains("\"version\":3"));
    assert_eq!(
        PlotData::from_saved_for_spec(&saved, &spec).unwrap().1,
        data
    );
    spec.set_axis_scaling(PlotAxis::X, 0, Some(unit("m")))
        .unwrap();
    assert!(PlotData::from_saved_for_spec(&saved, &spec).is_none());
    let mut pending = data.clone();
    pending.invalidate();
    assert_eq!(
        PlotData::from_saved_for_spec(&pending.to_saved(), &spec)
            .unwrap()
            .1,
        pending
    );
}
#[test]
fn auto_units_are_result_metadata_and_tenths_meter_is_source_scaling() {
    let mut spec = spec();
    let data = run(&spec, &prior());
    let c = curve(&data);
    assert_eq!(c.points, vec![[1., 1.], [2., 4.], [3., 9.]]);
    let p = c.presentation.as_ref().unwrap();
    let x = p.x.as_ref().unwrap();
    assert!(x.source_scaling().is_none());
    assert_eq!(x.residual_units().unwrap().references[0].unit, "meter");
    assert_eq!(
        p.y.as_ref().unwrap().residual_units().unwrap().references[0].power_numerator,
        Some(2)
    );
    assert!(spec.paired_sources().next().unwrap().x.scaling.is_none());
    let tenth = scale(
        "<ml:apply><ml:mult/><ml:real>0.1</ml:real><ml:id labels='UNIT'>m</ml:id></ml:apply>",
    );
    spec.set_axis_scaling(PlotAxis::X, 0, Some(tenth)).unwrap();
    let data = run(&spec, &prior());
    assert_eq!(curve(&data).points[1], [20., 4.]);
    assert!(
        curve(&data)
            .presentation
            .as_ref()
            .unwrap()
            .x
            .as_ref()
            .unwrap()
            .residual_units()
            .is_none()
    );
    spec.set_axis_scaling(PlotAxis::X, 0, Some(scale("<ml:real>0.1</ml:real>")))
        .unwrap();
    let data = run(&spec, &prior());
    let x = curve(&data)
        .presentation
        .as_ref()
        .unwrap()
        .x
        .as_ref()
        .unwrap();
    assert_eq!(curve(&data).points[1], [20., 4.]);
    assert_eq!(x.residual_units().unwrap().references[0].unit, "meter");
}
#[test]
fn forced_unit_scaling_and_sample_substitution_survive_variable_glyph_shadowing() {
    let mut prior = prior();
    prior.push((
        2,
        0.,
        20.,
        "<ml:define><ml:id labels='VARIABLE'>m</ml:id><ml:real>10</ml:real></ml:define>".into(),
    ));
    let mut spec = spec();
    spec.set_axis_scaling(PlotAxis::X, 0, Some(unit("m")))
        .unwrap();
    assert_eq!(curve(&run(&spec, &prior)).points[1], [2., 4.]);
    spec.set_axis_scaling(
        PlotAxis::X,
        0,
        Some(scale("<ml:id labels='VARIABLE'>m</ml:id>")),
    )
    .unwrap();
    let data = run(&spec, &prior);
    close(curve(&data).points[1][0], 0.2);
    assert!(
        curve(&data)
            .presentation
            .as_ref()
            .unwrap()
            .x
            .as_ref()
            .unwrap()
            .residual_units()
            .is_some()
    );
}
#[test]
fn incompatible_zero_negative_complex_or_incomplete_scaling_never_produces_ready_curve() {
    for xml in [
        "<ml:id labels='UNIT'>s</ml:id>",
        "<ml:real>0</ml:real>",
        "<ml:real>-1</ml:real>",
        "<ml:id>missingScale</ml:id>",
        "<ml:id>i</ml:id>",
    ] {
        let mut spec = spec();
        spec.set_axis_scaling(PlotAxis::X, 0, Some(scale(xml)))
            .unwrap();
        let data = run(&spec, &prior());
        assert!(
            matches!(data.traces[0], TraceOutcome::Failed(_)),
            "{xml}: {data:?}"
        );
    }
    let mut spec = spec();
    spec.set_axis_scaling(
        PlotAxis::X,
        0,
        Some(scale(
            "<ml:apply><ml:mult/><ml:placeholder/><ml:id labels='UNIT'>m</ml:id></ml:apply>",
        )),
    )
    .unwrap();
    assert!(matches!(
        run(&spec, &prior()).traces[0],
        TraceOutcome::Incomplete
    ));
}
#[test]
fn implicit_dimensionless_scaling_keeps_physical_domain_and_unitful_implicit_is_explicitly_unsupported()
 {
    let mut spec = spec();
    spec.set_axis_scaling(PlotAxis::X, 0, Some(scale("<ml:real>2</ml:real>")))
        .unwrap();
    let data = run(&spec, &[]);
    let c = curve(&data);
    assert_eq!(c.points[0], [-10., 400.]);
    assert_eq!(c.points.last().unwrap(), &[10., 400.]);
    spec.set_axis_scaling(PlotAxis::X, 0, Some(unit("m")))
        .unwrap();
    assert!(run(&spec, &[]).has_failures());
}
#[test]
fn old_unitless_snapshot_stays_v1_and_cannot_attach_to_new_scaled_source() {
    let mut spec = spec();
    let data = run(&spec, &[]);
    assert!(curve(&data).presentation.is_none());
    let saved = data.to_saved();
    assert!(saved.contains("\"version\":1"));
    assert_eq!(
        PlotData::from_saved_for_spec(&saved, &spec).unwrap().1,
        data
    );
    spec.set_axis_scaling(PlotAxis::X, 0, Some(scale("<ml:real>1</ml:real>")))
        .unwrap();
    assert!(PlotData::from_saved_for_spec(&saved, &spec).is_none());
}

#[test]
fn shared_physical_x_keeps_its_scale_and_a_bad_y_unit_fails_only_its_own_trace() {
    let mut spec = PlotSpec {
        source: PlotSource::axis_lists(
            vec!["x".into()],
            vec!["x^2".into(), "x^2".into()],
            vec![Default::default(); 2],
        )
        .unwrap(),
        ..Default::default()
    };
    spec.set_axis_scaling(PlotAxis::X, 0, Some(unit("in")))
        .unwrap();
    spec.set_axis_scaling(
        PlotAxis::Y,
        0,
        Some(scale(
            "<ml:apply><ml:pow/><ml:id labels='UNIT'>m</ml:id><ml:real>2</ml:real></ml:apply>",
        )),
    )
    .unwrap();
    spec.set_axis_scaling(PlotAxis::Y, 1, Some(unit("s")))
        .unwrap();
    let data = run(&spec, &prior());
    assert!(matches!(data.traces[0], TraceOutcome::Ready(_)));
    assert!(matches!(data.traces[1], TraceOutcome::Failed(_)));
    close(curve(&data).points[1][0], 2. / 0.0254);
    assert_eq!(curve(&data).points[1][1], 4.);
    assert_eq!(
        PlotData::from_saved_for_spec(&data.to_saved(), &spec)
            .unwrap()
            .1,
        data
    );
}
#[test]
fn physical_range_steps_and_scalar_broadcast_do_not_strip_dimension() {
    let range = tree(
        NodeKind::Range { has_step: true },
        vec![
            axis_units::quantity_tree(mathfolio_core::unit("m").unwrap()).unwrap(),
            axis_units::quantity_tree(
                Quantity::scalar(2.)
                    .multiply(mathfolio_core::unit("m").unwrap(), false)
                    .unwrap(),
            )
            .unwrap(),
            axis_units::quantity_tree(
                Quantity::scalar(3.)
                    .multiply(mathfolio_core::unit("m").unwrap(), false)
                    .unwrap(),
            )
            .unwrap(),
        ],
    );
    let definition = tree(
        NodeKind::Binary(BinaryOp::Define),
        vec![tree(NodeKind::Identifier("x".into()), vec![]), range],
    );
    let prior = vec![(
        1,
        0.,
        0.,
        editor_bridge::tree_to_mathml(&definition).unwrap(),
    )];
    let spec = PlotSpec {
        source: PlotSource::single("x", "2*m"),
        ..Default::default()
    };
    let data = run(&spec, &prior);
    let curve = curve(&data);
    assert_eq!(curve.points, vec![[1., 2.], [2., 2.], [3., 2.]]);
    assert!(
        curve
            .presentation
            .as_ref()
            .unwrap()
            .x
            .as_ref()
            .unwrap()
            .residual_units()
            .is_some()
    );
}
