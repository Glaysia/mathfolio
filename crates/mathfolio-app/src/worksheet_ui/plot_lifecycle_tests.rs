//! CPU controller regressions: real calculation/commit/cache paths, no GUI.
use super::*;
use crate::plots::{PlotData, TraceOutcome};
use mathfolio_document::canvas::{PlotSource, PlotTraceAppearance};

fn define(app: &mut WorksheetApp, name: &str, rhs: &str, top: f64) -> u64 {
    let id = app.document.add_math(20., top);
    app.document.regions.iter_mut().find(|r| r.id == id).unwrap().content =
        CanvasContent::Math(worksheet_math::tree_from_mathml(&format!(
            "<m:define xmlns:m=\"http://schemas.mathsoft.com/math50\"><m:id>{name}</m:id>{rhs}</m:define>"
        )).unwrap());
    id
}

fn fixture(legacy: bool, failed_middle: bool) -> (WorksheetApp, u64, u64) {
    let mut app = WorksheetApp {
        automatic_calculation: false,
        ..Default::default()
    };
    app.document = CanvasDocument::default();
    define(
        &mut app,
        "x",
        "<m:matrix rows=\"3\" cols=\"1\"><m:real>1</m:real><m:real>2</m:real><m:real>3</m:real></m:matrix>",
        20.,
    );
    let gain = define(&mut app, "a", "<m:real>2</m:real>", 60.);
    let id = app.document.add_plot(20., 120.);
    let ys = ["a*x", if failed_middle { "1/(x-x)" } else { "2*x" }, "3*x"];
    let source = if legacy {
        PlotSource::legacy(
            "x",
            "a*x",
            ys.into_iter()
                .map(|y| PlotTraceSpec {
                    x_expression: "x".into(),
                    y_expression: y.into(),
                    ..Default::default()
                })
                .collect(),
        )
    } else {
        PlotSource::axis_lists(
            vec!["x".into()],
            ys.into_iter().map(String::from).collect(),
            vec![PlotTraceAppearance::default(); 3],
        )
        .unwrap()
    };
    let CanvasContent::Plot(spec) = &mut app
        .document
        .regions
        .iter_mut()
        .find(|r| r.id == id)
        .unwrap()
        .content
    else {
        panic!()
    };
    spec.source = source;
    app.recalculate();
    app.persist_calculation_cache();
    app.restore_saved_calculations();
    app.last_executed_regions.clear();
    app.undo.clear();
    app.redo.clear();
    assert!(!app.pending_regions.contains(&id));
    assert_eq!(data(&app, id).has_failures(), failed_middle);
    (app, id, gain)
}
fn data(app: &WorksheetApp, id: u64) -> &PlotData {
    app.plot_results[&id].as_ref().unwrap()
}
fn spec(app: &WorksheetApp, id: u64) -> &PlotSpec {
    let CanvasContent::Plot(spec) = &app
        .document
        .regions
        .iter()
        .find(|r| r.id == id)
        .unwrap()
        .content
    else {
        panic!()
    };
    spec
}
fn mark_native_origin(app: &mut WorksheetApp, id: u64) {
    let payload = mathfolio_document::native_plot_result::finite_unitless_xml(
        data(app, id).first_curve().unwrap().points.as_slice(),
    )
    .unwrap()
    .unwrap();
    app.saved_results.get_mut(&id).unwrap().numeric_source =
        Some(SavedNumericSource::NativeMathml(payload));
}
fn assert_invalidated(app: &WorksheetApp, id: u64, previous: &PlotData) {
    assert!(app.pending_regions.contains(&id));
    assert!(app.last_executed_regions.is_empty());
    assert_eq!(data(app, id).traces.len(), previous.traces.len());
    for (current, old) in data(app, id).traces.iter().zip(&previous.traces) {
        assert!(matches!(current, TraceOutcome::Pending { .. }));
        assert_eq!(current.curve(), old.curve());
        if let (Some(a), Some(b)) = (current.curve(), old.curve()) {
            assert_eq!(a.points.as_ptr(), b.points.as_ptr());
        }
    }
    assert!(app.saved_results[&id].numeric_source.is_none());
    assert_eq!(
        app.saved_results[&id].status,
        SavedCalculationStatus::Pending
    );
}
fn open_settings(app: &mut WorksheetApp, id: u64) {
    app.selected = Some(id);
    app.ribbon_command(&egui::Context::default(), RibbonAction::EditPlot);
    assert!(app.plot_draft.is_some());
}

#[test]
fn source_commit_invalidates_mixed_results_without_reviving_failed_curve_or_native_marker() {
    let (mut app, id, _) = fixture(false, true);
    mark_native_origin(&mut app, id);
    let previous = data(&app, id).clone();
    let ids = spec(&app, id)
        .axis_lists()
        .unwrap()
        .y
        .iter()
        .map(|e| e.id)
        .collect::<Vec<_>>();
    open_settings(&mut app, id);
    app.plot_draft
        .as_mut()
        .unwrap()
        .spec
        .axis_lists_mut()
        .unwrap()
        .y[1]
        .source = "4*x".into();
    assert!(app.commit_plot_draft());
    assert_eq!(app.undo.len(), 1);
    assert_eq!(
        spec(&app, id)
            .axis_lists()
            .unwrap()
            .y
            .iter()
            .map(|e| e.id)
            .collect::<Vec<_>>(),
        ids
    );
    assert_invalidated(&app, id, &previous);
    assert!(matches!(
        data(&app, id).traces[1],
        TraceOutcome::Pending { previous: None }
    ));
    app.persist_calculation_cache();
    let row = app
        .document
        .calculation_cache
        .as_ref()
        .unwrap()
        .rows
        .iter()
        .find(|row| row.region_id == id)
        .unwrap();
    let snapshot = mathfolio_document::plot_snapshot::SavedPlotSnapshot::parse_for_spec(
        row.display.as_deref().unwrap(),
        spec(&app, id),
    )
    .unwrap();
    assert_eq!(snapshot.version, 2);
    app.recalculate();
    assert!(!data(&app, id).has_pending() && !data(&app, id).has_failures());
    assert_eq!(
        data(&app, id).traces[1].curve().unwrap().points,
        vec![[1., 4.], [2., 8.], [3., 12.]]
    );
}

#[test]
fn upstream_context_edit_invalidates_every_trace_then_recalculates_against_new_binding() {
    let (mut app, id, gain) = fixture(false, true);
    mark_native_origin(&mut app, id);
    let previous = data(&app, id).clone();
    let source = spec(&app, id).clone();
    app.document.regions.iter_mut().find(|r|r.id==gain).unwrap().content=CanvasContent::Math(worksheet_math::tree_from_mathml(r#"<m:define xmlns:m="http://schemas.mathsoft.com/math50"><m:id>a</m:id><m:real>7</m:real></m:define>"#).unwrap());
    app.selected = Some(gain);
    app.calculation_changed(false);
    assert_eq!(spec(&app, id), &source);
    assert_invalidated(&app, id, &previous);
    app.recalculate();
    assert_eq!(
        data(&app, id).traces[0].curve().unwrap().points,
        vec![[1., 7.], [2., 14.], [3., 21.]]
    );
    assert!(matches!(data(&app, id).traces[1], TraceOutcome::Failed(_)));
    assert_eq!(
        data(&app, id).traces[2].curve().unwrap(),
        previous.traces[2].curve().unwrap()
    );
}

#[test]
fn same_count_axis_identity_reorder_drops_previous_curves_before_any_recalculation() {
    let (mut app, id, _) = fixture(false, false);
    let before = spec(&app, id).clone();
    let previous = data(&app, id).clone();
    open_settings(&mut app, id);
    app.plot_draft
        .as_mut()
        .unwrap()
        .spec
        .axis_lists_mut()
        .unwrap()
        .y
        .swap(0, 2);
    assert_eq!(
        app.plot_draft.as_ref().unwrap().spec.trace_count(),
        before.trace_count()
    );
    assert!(!same_plot_topology(
        &before,
        &app.plot_draft.as_ref().unwrap().spec
    ));
    assert!(
        !app.plot_draft.as_ref().unwrap().topology_changed,
        "identity check, not manual dirty flag, must detect this"
    );
    assert!(app.commit_plot_draft());
    assert!(
        data(&app, id)
            .traces
            .iter()
            .all(|slot| matches!(slot, TraceOutcome::Pending { previous: None }))
    );
    assert!(app.last_executed_regions.is_empty());
    assert_eq!(app.undo.len(), 1);
    app.recalculate();
    assert_eq!(data(&app, id).traces[0].curve(), previous.traces[2].curve());
    assert_eq!(data(&app, id).traces[2].curve(), previous.traces[0].curve());
}

#[test]
fn legacy_balanced_remove_add_dirty_flag_prevents_same_count_curve_misassociation() {
    let (mut app, id, _) = fixture(true, false);
    let before = spec(&app, id).clone();
    open_settings(&mut app, id);
    let draft = app.plot_draft.as_mut().unwrap();
    // These are the same operations and persistent dirty flag used by the
    // modal buttons. Balancing counts must not clear the topology event.
    draft.spec.remove_trace(0).unwrap();
    draft.topology_changed = true;
    draft
        .spec
        .add_trace(PlotTraceSpec {
            x_expression: "x".into(),
            y_expression: "9*x".into(),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(draft.spec.trace_count(), before.trace_count());
    assert!(
        same_plot_topology(&before, &draft.spec),
        "Legacy has no per-equation IDs"
    );
    assert!(app.commit_plot_draft());
    assert!(
        data(&app, id)
            .traces
            .iter()
            .all(|slot| matches!(slot, TraceOutcome::Pending { previous: None }))
    );
    assert!(app.last_executed_regions.is_empty());
    app.recalculate();
    assert_eq!(
        data(&app, id).traces[0].curve().unwrap().points,
        vec![[1., 2.], [2., 4.], [3., 6.]]
    );
    assert_eq!(
        data(&app, id).traces[2].curve().unwrap().points,
        vec![[1., 9.], [2., 18.], [3., 27.]]
    );
}

#[test]
fn cold_mixed_pending_failed_navigation_keeps_trace_diagnostic_and_submits_no_calculation() {
    let (mut original, id, _) = fixture(false, true);
    let message = data(&original, id).traces[1]
        .diagnostic()
        .unwrap()
        .to_owned();
    let first = original
        .plot_results
        .get_mut(&id)
        .unwrap()
        .as_mut()
        .unwrap()
        .traces
        .first_mut()
        .unwrap();
    let TraceOutcome::Ready(curve) = first.clone() else {
        panic!()
    };
    *first = TraceOutcome::Pending {
        previous: Some(curve),
    };
    original.pending_regions.insert(id); // Fixture models a partially restored native bundle, not invalidation.
    original.persist_calculation_cache();
    let mut app = WorksheetApp {
        document: original.document.clone(),
        automatic_calculation: false,
        ..Default::default()
    };
    app.restore_saved_calculations();
    assert!(data(&app, id).has_pending() && data(&app, id).has_failures());
    assert!(app.pending_regions.contains(&id) && app.region_has_error(id));
    assert_eq!(app.error_region_ids(), vec![id]);
    let source = app.document.clone();
    let target = app
        .document
        .regions
        .iter()
        .find(|region| region.id == id)
        .unwrap();
    let location = (target.left, target.top);
    let curves = data(&app, id).clone();
    let generation = app.background.submitted_generation();
    let ctx = egui::Context::default();
    for action in [RibbonAction::NextError, RibbonAction::PreviousError] {
        app.ribbon_command(&ctx, action);
        assert_eq!(app.selected, Some(id));
        assert_eq!(app.reveal, Some(location));
        assert_eq!(app.status, format!("곡선 2: {message}"));
        assert_eq!(app.background.submitted_generation(), generation);
        assert!(app.last_executed_regions.is_empty());
        assert_eq!(data(&app, id), &curves);
    }
    assert_eq!(app.document, source);
}
