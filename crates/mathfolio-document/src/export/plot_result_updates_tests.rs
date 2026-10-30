//! Independent package-level selective curve result update oracles.
use super::*;
use crate::canvas::{
    PlotTraceSpec, SavedCalculationCache, SavedCalculationRow, SavedCalculationStatus,
};
use serde_json::{Value, json};

fn spec(canvas: &CanvasDocument) -> &crate::canvas::PlotSpec {
    let CanvasContent::Plot(spec) = &canvas.regions[0].content else {
        panic!("plot")
    };
    spec
}
fn points(seed: f64) -> Vec<[f64; 2]> {
    vec![[seed, seed + 1.], [seed + 2., seed + 3.]]
}
fn curve(canvas: &CanvasDocument, ordinal: usize, seed: f64) -> Value {
    let trace = &spec(canvas).resolved_traces().unwrap()[ordinal];
    json!({"points":points(seed),"implicit_domain":false,"label":trace.label,"style":trace.style})
}
fn ready(canvas: &CanvasDocument, ordinal: usize, seed: f64) -> Value {
    json!({"state":"ready","data":curve(canvas,ordinal,seed)})
}
fn install(canvas: &mut CanvasDocument, display: String, status: SavedCalculationStatus) {
    canvas.calculation_cache = Some(SavedCalculationCache {
        source_signature: canvas.calculation_signature(),
        rows: vec![SavedCalculationRow {
            region_id: canvas.regions[0].id,
            status,
            display: Some(display),
            error: None,
            warnings: vec![],
            symbolic_mathml: None,
            numeric_source: None,
        }],
    });
    assert!(canvas.saved_calculation_cache().is_some());
}
fn install_v2(canvas: &mut CanvasDocument, slots: Vec<Value>) {
    install(
        canvas,
        json!({"version":2,"traces":slots}).to_string(),
        SavedCalculationStatus::Pending,
    );
}
// Exercise ZIP persistence too: inspect the reopened package, never the writer's
// transient nodes or saved JSON alone.
fn export_reopen(canvas: &CanvasDocument, package: &mut NativePackage) {
    export_into(canvas, package).unwrap();
    let bytes = package.to_bytes().unwrap();
    *package = NativePackage::from_bytes(&bytes).unwrap();
}
fn fixture(count: usize) -> (CanvasDocument, NativePackage, Vec<i32>) {
    let mut canvas = CanvasDocument::default();
    canvas.add_plot(20., 20.);
    let CanvasContent::Plot(plot) = &mut canvas.regions[0].content else {
        unreachable!()
    };
    plot.promote_legacy_trace();
    for i in 1..count {
        plot.add_trace(PlotTraceSpec {
            x_expression: "x".into(),
            y_expression: format!("x+{i}"),
            ..Default::default()
        })
        .unwrap();
    }
    let initial: Vec<_> = (0..count)
        .map(|i| curve(&canvas, i, 10. * i as f64))
        .collect();
    install(
        &mut canvas,
        json!({"version":1,"traces":initial}).to_string(),
        SavedCalculationStatus::Synchronized,
    );
    let mut package = minimal_package(&canvas).unwrap();
    export_reopen(&canvas, &mut package);
    let region = &package.regions().unwrap()[0];
    let refs = crate::native_plot::trace_result_refs(&region.raw_xml)
        .unwrap()
        .into_iter()
        .map(Option::unwrap)
        .collect();
    canvas.regions[0].source_region = region.region_id.clone();
    let old_display = canvas.calculation_cache.as_ref().unwrap().rows[0]
        .display
        .clone()
        .unwrap();
    install(
        &mut canvas,
        old_display,
        SavedCalculationStatus::Synchronized,
    );
    (canvas, package, refs)
}
#[derive(Debug)]
struct Row {
    status: String,
    raw: String,
    points: Option<Vec<[f64; 2]>>,
    payload_count: usize,
}
fn row(package: &NativePackage, id: i32) -> Row {
    let source = package
        .original_xml_part("mathcad/result.xml")
        .unwrap()
        .unwrap();
    let doc = parse(&source).unwrap();
    let rows: Vec<_> = doc
        .root_element()
        .children()
        .filter(|n| {
            n.has_tag_name((RESULTS, "resultData"))
                && n.attribute("result-id")
                    .and_then(|id| crate::native::parse_result_id(id).ok())
                    == Some(id)
        })
        .collect();
    assert_eq!(rows.len(), 1, "ambiguous result {id}");
    let node = rows[0];
    let payloads: Vec<_> = node
        .children()
        .filter(|n| n.has_tag_name((ML, "Trace2dResult")))
        .collect();
    Row {
        status: node.attribute("calculation-status").unwrap().into(),
        raw: source[node.range()].into(),
        points: payloads.first().map(|n| {
            crate::native_plot_result::read_result(*n)
                .unwrap()
                .unwrap()
                .points
        }),
        payload_count: payloads.len(),
    }
}
fn assert_row(package: &NativePackage, id: i32, status: &str, expected: Option<Vec<[f64; 2]>>) {
    let actual = row(package, id);
    assert_eq!(actual.status, status, "id {id}");
    assert_eq!(
        actual.payload_count,
        usize::from(expected.is_some()),
        "old/duplicate payload id {id}: {}",
        actual.raw
    );
    assert_eq!(actual.points, expected, "id {id}");
}
fn assert_refs(package: &NativePackage, refs: &[i32]) {
    let regions = package.regions().unwrap();
    assert_eq!(
        crate::native_plot::trace_result_refs(&regions[0].raw_xml).unwrap(),
        refs.iter().copied().map(Some).collect::<Vec<_>>()
    );
}

#[test]
fn failed_first_or_middle_curve_clears_only_its_old_native_payload() {
    for (count, failed) in [(2, 0), (3, 1)] {
        let (mut canvas, mut package, refs) = fixture(count);
        let slots = (0..count)
            .map(|i| {
                if i == failed {
                    json!({"state":"failed","message":"undefined axis"})
                } else {
                    ready(&canvas, i, 100. + i as f64)
                }
            })
            .collect();
        install_v2(&mut canvas, slots);
        export_reopen(&canvas, &mut package);
        assert_refs(&package, &refs);
        assert_eq!(package.caches().unwrap().len(), count);
        for (i, id) in refs.into_iter().enumerate() {
            assert_row(
                &package,
                id,
                if i == failed {
                    "Pending"
                } else {
                    "Synchronized"
                },
                (i != failed).then(|| points(100. + i as f64)),
            );
        }
    }
}
#[test]
fn pending_previous_is_replaced_per_slot_and_none_or_incomplete_really_clears() {
    for clear_state in [
        json!({"state":"pending","previous":null}),
        json!({"state":"incomplete"}),
    ] {
        let (mut canvas, mut package, refs) = fixture(3);
        let mut previous = curve(&canvas, 0, 500.);
        previous["style"]["line_width"] = json!(8.); // historical styling is inert, not current-source proof.
        let slots = vec![
            json!({"state":"pending","previous":previous}),
            clear_state,
            ready(&canvas, 2, 700.),
        ];
        install_v2(&mut canvas, slots);
        export_reopen(&canvas, &mut package);
        assert_refs(&package, &refs);
        assert_row(&package, refs[0], "Pending", Some(points(500.)));
        assert_row(&package, refs[1], "Pending", None);
        assert_row(&package, refs[2], "Synchronized", Some(points(700.)));
    }
}
#[test]
fn unavailable_keeps_exact_native_row_while_ready_sibling_is_fresh() {
    let (mut canvas, mut package, refs) = fixture(2);
    let source = package
        .original_xml_part("mathcad/result.xml")
        .unwrap()
        .unwrap();
    let source = source.replacen(
        "calculation-status=\"Synchronized\"",
        "tag=\"original &amp; metadata\" calculation-status=\"Synchronized\"",
        1,
    );
    set_xml(&mut package, "mathcad/result.xml", source).unwrap();
    let old = row(&package, refs[0]).raw;
    let slots = vec![
        json!({"state":"unavailable","message":"unsupported native units"}),
        ready(&canvas, 1, 800.),
    ];
    install_v2(&mut canvas, slots);
    export_reopen(&canvas, &mut package);
    assert_eq!(row(&package, refs[0]).raw, old);
    assert_row(&package, refs[1], "Synchronized", Some(points(800.)));
}
#[test]
fn legacy_pending_keeps_old_payload_instead_of_using_new_snapshot_points() {
    let (mut canvas, mut package, refs) = fixture(2);
    let slots: Vec<_> = (0..2).map(|i| curve(&canvas, i, 900. + i as f64)).collect();
    install(
        &mut canvas,
        json!({"version":1,"traces":slots}).to_string(),
        SavedCalculationStatus::Pending,
    );
    export_reopen(&canvas, &mut package);
    for (i, id) in refs.into_iter().enumerate() {
        assert_row(&package, id, "Pending", Some(points(10. * i as f64)));
    }
}
#[test]
fn invalid_v2_and_missing_required_keys_never_partially_promote_ready_sibling() {
    for case in 0..7 {
        let (mut canvas, mut package, refs) = fixture(2);
        let mut slots = vec![ready(&canvas, 0, 1000.), ready(&canvas, 1, 1100.)];
        match case {
            0 => slots[0] = json!({"state":"pending"}),
            1 => {
                slots[0]["data"]["style"]["line_width"] = json!(8.);
            }
            2 => {
                slots[0]["data"].as_object_mut().unwrap().remove("points");
            }
            3 => {
                slots[0]["data"]["unrecognized"] = json!(true);
            }
            4 => {
                slots.pop();
            }
            5 => {
                slots[0] = json!({"state":"ready","data":{"points":[[0.]],"implicit_domain":false,"label":"curve-0","style":spec(&canvas).resolved_traces().unwrap()[0].style}})
            }
            _ => slots[0] = json!({"state":"failed","message":"x".repeat(16385)}),
        }
        install_v2(&mut canvas, slots);
        export_reopen(&canvas, &mut package);
        for (i, id) in refs.into_iter().enumerate() {
            assert_row(&package, id, "Pending", Some(points(10. * i as f64)));
        }
    }
}
#[test]
fn duplicate_result_rows_including_integer_aliases_are_not_overwritten() {
    for another_part in [false, true] {
        let (mut canvas, mut package, refs) = fixture(2);
        let old = row(&package, refs[0]).raw;
        let duplicate = old.replacen(
            &format!("result-id=\"{}\"", refs[0]),
            &format!("result-id=\" +000{} \"", refs[0]),
            1,
        );
        if another_part {
            set_xml(
                &mut package,
                "custom/duplicate-result.xml",
                format!(
                    "<resultsList xmlns=\"{RESULTS}\" xmlns:ml=\"{ML}\">{duplicate}</resultsList>"
                ),
            )
            .unwrap();
        } else {
            let source = package
                .original_xml_part("mathcad/result.xml")
                .unwrap()
                .unwrap()
                .replace("</resultsList>", &format!("{duplicate}</resultsList>"));
            set_xml(&mut package, "mathcad/result.xml", source).unwrap();
        }
        let slots = vec![ready(&canvas, 0, 1200.), ready(&canvas, 1, 1300.)];
        install_v2(&mut canvas, slots);
        export_reopen(&canvas, &mut package);
        let caches = package.caches().unwrap();
        let duplicates: Vec<_> = caches
            .iter()
            .filter(|r| {
                r.result_id
                    .as_deref()
                    .and_then(|id| crate::native::parse_result_id(id).ok())
                    == Some(refs[0])
            })
            .collect();
        assert_eq!(duplicates.len(), 2);
        for cached in duplicates {
            let payload = cached.result_mathml.as_ref().unwrap();
            let doc = parse(payload).unwrap();
            assert_eq!(
                crate::native_plot_result::read_result(doc.root_element())
                    .unwrap()
                    .unwrap()
                    .points,
                points(0.)
            );
        }
        assert_row(&package, refs[1], "Synchronized", Some(points(1300.)));
    }
}
#[test]
fn ambiguous_trace_references_block_all_curve_promotion_in_that_owner() {
    let (mut canvas, mut package, refs) = fixture(2);
    let source = package.worksheet_xml().unwrap().replace(
        &format!("resultRef=\"{}\"", refs[1]),
        &format!("resultRef=\" +000{} \"", refs[0]),
    );
    set_xml(&mut package, "mathcad/worksheet.xml", source).unwrap();
    let slots = vec![ready(&canvas, 0, 1400.), ready(&canvas, 1, 1500.)];
    install_v2(&mut canvas, slots);
    export_reopen(&canvas, &mut package);
    let caches = package.caches().unwrap();
    assert_eq!(caches.len(), 2);
    for (i, id) in refs.into_iter().enumerate() {
        assert_eq!(row(&package, id).points, Some(points(10. * i as f64)));
    }
}

#[test]
fn duplicate_json_keys_and_stale_source_signature_cannot_promote_new_coordinates() {
    for case in 0..4 {
        let (mut canvas, mut package, refs) = fixture(2);
        let slots = vec![ready(&canvas, 0, 1600.), ready(&canvas, 1, 1700.)];
        let wire = json!({"version":2,"traces":slots}).to_string();
        let wire = match case {
            0 => wire.replacen("\"version\":2", "\"version\":2,\"version\":2", 1),
            1 => wire.replacen(
                "\"state\":\"ready\"",
                "\"state\":\"ready\",\"state\":\"failed\"",
                1,
            ),
            _ => wire,
        };
        install(&mut canvas, wire, SavedCalculationStatus::Synchronized);
        if case == 2 {
            canvas
                .calculation_cache
                .as_mut()
                .unwrap()
                .source_signature
                .push('x');
        } else if case == 3 {
            canvas.calculation_cache.as_mut().unwrap().rows.clear();
        }
        export_reopen(&canvas, &mut package);
        for (i, id) in refs.into_iter().enumerate() {
            assert_row(
                &package,
                id,
                if case < 2 { "Pending" } else { "Synchronized" },
                Some(points(10. * i as f64)),
            );
        }
    }
}
