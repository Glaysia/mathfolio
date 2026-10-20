//! Geometry oracles are independent of the app's layout/render loop.
use super::*;
use crate::canvas::{
    PlotGraphSize, PlotNativeGeometry, PlotSpec, SavedCalculationCache, SavedCalculationRow,
    SavedCalculationStatus,
};
use crate::native_plot::{self, matches_retained_source, same_source};
use serde_json::json;
fn explicit() -> PlotSpec {
    let geometry = PlotNativeGeometry {
        graph_size: Some(PlotGraphSize {
            width: 321.00000000001,
            height: 217.125,
        }),
        requested_width: Some(600.00000000001),
        requested_height: Some(400.125),
        actual_width: Some(598.75),
        actual_height: Some(-0.),
    };
    let [width, height] = geometry.allocation().unwrap();
    PlotSpec {
        width,
        height,
        native_geometry: Some(Box::new(geometry)),
        ..Default::default()
    }
}
fn region(spec: &PlotSpec) -> String {
    let dimensions = plot_source::geometry_attributes(spec)
        .into_iter()
        .filter_map(|(name, value)| value.map(|v| format!(" {name}=\"{v}\"")))
        .collect::<String>();
    format!(
        "<region xmlns=\"{WS}\" region-id=\"1\" left=\"0\" top=\"0\"{dimensions}>{}</region>",
        native_plot::payload(spec).unwrap()
    )
}
fn canvas(spec: PlotSpec) -> CanvasDocument {
    let mut doc = CanvasDocument::default();
    doc.add_plot(0., 0.);
    doc.regions[0].content = CanvasContent::Plot(spec);
    doc
}
fn parsed(package: &NativePackage) -> PlotSpec {
    native_plot::parse(&package.regions().unwrap()[0].raw_xml).unwrap()
}
fn package(doc: &CanvasDocument) -> NativePackage {
    let mut p = minimal_package(doc).unwrap();
    export_into(doc, &mut p).unwrap();
    p
}
fn inert_cache(doc: &mut CanvasDocument) {
    let CanvasContent::Plot(spec) = &doc.regions[0].content else {
        panic!()
    };
    let display=json!({"version":1,"traces":[{"points":[[0.,1.],[2.,3.]],"implicit_domain":false,"label":"","style":spec.resolved_traces().unwrap()[0].style}]}).to_string();
    doc.calculation_cache = Some(SavedCalculationCache {
        source_signature: doc.calculation_signature(),
        rows: vec![SavedCalculationRow {
            region_id: doc.regions[0].id,
            status: SavedCalculationStatus::Synchronized,
            display: Some(display),
            error: None,
            warnings: vec![],
            symbolic_mathml: None,
            numeric_source: None,
        }],
    });
}
#[test]
fn old_wire_default_and_persisted_signature_stay_exact_and_new_carrier_is_strict() {
    let old = r#"{"x_expression":"x","y_expression":"sin(x)","width":420.0,"height":260.0,"traces":[],"x_axis":{"label":"","range":null},"y_axis":{"label":"","range":null}}"#;
    assert_eq!(serde_json::to_string(&PlotSpec::default()).unwrap(), old);
    let old_doc = canvas(PlotSpec::default());
    let mut doc = old_doc.clone();
    inert_cache(&mut doc);
    let reopened: CanvasDocument =
        serde_json::from_slice(&serde_json::to_vec(&doc).unwrap()).unwrap();
    assert_eq!(
        reopened.calculation_signature(),
        old_doc.calculation_signature()
    );
    assert!(reopened.saved_calculation_cache().is_some());
    let wire = serde_json::to_string(&explicit()).unwrap();
    assert_eq!(serde_json::from_str::<PlotSpec>(&wire).unwrap(), explicit());
    for invalid in [
        old.replace("\"y_axis\":", "\"native_geometry\":null,\"y_axis\":"),
        wire.replace(
            "\"native_geometry\":",
            "\"native_geometry\":{},\"native_geometry\":",
        ),
        wire.replace("\"graph_size\":", "\"graph_size\":null,\"graph_size\":"),
        wire.replace("\"graph_size\":", "\"unknown\":0,\"graph_size\":"),
        wire.replace("\"width\":321.00000000001,", ""),
        wire.replace("321.00000000001", "-1"),
        wire.replace("598.75", "-1"),
    ] {
        assert!(
            serde_json::from_str::<PlotSpec>(&invalid).is_err(),
            "{invalid}"
        );
    }
}
#[test]
fn optional_graph_and_all_source_dimensions_roundtrip_without_inference() {
    for graph in [true, false] {
        for width in [true, false] {
            for height in [true, false] {
                let mut spec = explicit();
                let g = spec.native_geometry.as_mut().unwrap();
                if !graph {
                    g.graph_size = None;
                }
                if !width {
                    g.requested_width = None;
                }
                if !height {
                    g.requested_height = None;
                }
                [spec.width, spec.height] = g.allocation().unwrap();
                let source = region(&spec);
                let restored = native_plot::parse(&source).unwrap();
                assert!(same_source(&spec, &restored).unwrap());
                assert_eq!(
                    restored
                        .native_geometry
                        .as_ref()
                        .unwrap()
                        .actual_height
                        .unwrap()
                        .to_bits(),
                    (-0_f64).to_bits()
                );
                let payload = native_plot::payload(&restored).unwrap();
                assert_eq!(payload.contains("<graph-size"), graph);
                let copy: PlotSpec =
                    serde_json::from_slice(&serde_json::to_vec(&restored).unwrap()).unwrap();
                assert!(same_source(&restored, &copy).unwrap());
            }
        }
    }
    let blank = PlotSpec::blank_xy();
    assert!(blank.native_geometry.as_ref().unwrap().graph_size.is_none());
    assert!(!native_plot::payload(&blank).unwrap().contains("graph-size"));
}
#[test]
fn strict_geometry_comparison_binds_absence_f64_bits_and_current_projection() {
    let original = explicit();
    for case in 0..7 {
        let mut changed = original.clone();
        let g = changed.native_geometry.as_mut().unwrap();
        match case {
            0 => g.graph_size = None,
            1 => g.graph_size.as_mut().unwrap().width += 0.0000000001,
            2 => g.requested_width = Some(600.00000000002),
            3 => g.actual_width = None,
            4 => g.actual_height = Some(0.),
            5 => g.actual_width = Some(598.75000000001),
            _ => {
                g.requested_height = None;
                changed.height = 260.;
            }
        }
        assert!(!same_source(&original, &changed).unwrap(), "case{case}");
        assert_ne!(
            canvas(original.clone()).calculation_signature(),
            canvas(changed).calculation_signature()
        );
    }
    let mut bad = original.clone();
    bad.width += 1.;
    assert!(bad.validate().is_err());
}
#[test]
fn atomic_outer_resize_keeps_graph_and_unchanged_axis_precision_and_undo_snapshot() {
    let mut spec = explicit();
    let before = serde_json::to_vec(&spec).unwrap();
    assert!(!spec.set_outer_size(spec.width, spec.height).unwrap());
    assert_eq!(serde_json::to_vec(&spec).unwrap(), before);
    assert!(spec.set_outer_size(620., spec.height).unwrap());
    let g = spec.native_geometry.as_ref().unwrap();
    assert_eq!(g.requested_width, Some(620.));
    assert_eq!(g.requested_height, Some(400.125));
    assert_eq!(g.graph_size, explicit().native_geometry.unwrap().graph_size);
    assert!(g.actual_width.is_none() && g.actual_height.is_none());
    let after = serde_json::to_vec(&spec).unwrap();
    for (w, h) in [
        (0., 400.),
        (620., f32::NAN),
        (f32::INFINITY, 400.),
        (620., 2001.),
    ] {
        assert!(spec.set_outer_size(w, h).is_err());
        assert_eq!(serde_json::to_vec(&spec).unwrap(), after);
    }
    let restored: PlotSpec = serde_json::from_slice(&before).unwrap();
    assert!(same_source(&restored, &explicit()).unwrap());
    assert_eq!(
        serde_json::to_vec(&serde_json::from_slice::<PlotSpec>(&after).unwrap()).unwrap(),
        after
    );
    let mut legacy = PlotSpec::default();
    legacy.set_outer_size(500., 320.).unwrap();
    assert!(legacy.native_geometry.is_none());
    assert!(
        native_plot::payload(&legacy)
            .unwrap()
            .contains("height=\"270\"")
    );
}
#[test]
fn old_source_adapter_is_bounded_and_never_matches_absent_or_precision_lost_graph() {
    let old = PlotSpec::default();
    let previous = native_plot::parse(&format!(
        "<region xmlns=\"{WS}\" width=\"800\" height=\"900\">{}</region>",
        native_plot::payload(&old).unwrap()
    ))
    .unwrap();
    assert!(!same_source(&old, &previous).unwrap());
    assert!(matches_retained_source(&old, &previous).unwrap());
    let mut changed = old.clone();
    changed.width += 1.;
    assert!(!matches_retained_source(&changed, &previous).unwrap());
    *changed.primary_y_expression_mut().unwrap() = "cos(x)".into();
    assert!(!matches_retained_source(&changed, &previous).unwrap());
    for graph in [
        None,
        Some(PlotGraphSize {
            width: 420.,
            height: 210.00000000001,
        }),
        Some(PlotGraphSize {
            width: 420.,
            height: 49.,
        }),
    ] {
        let mut previous = previous.clone();
        previous.native_geometry.as_mut().unwrap().graph_size = graph;
        assert!(!matches_retained_source(&old, &previous).unwrap());
    }
    assert!(!matches_retained_source(&explicit(), &previous).unwrap());
}
#[test]
fn malformed_nonfinite_unsupported_source_is_rejected_without_clamping() {
    let source = region(&explicit());
    for invalid in [
        source.replace("321.00000000001", "0"),
        source.replace("321.00000000001", "-1"),
        source.replace("321.00000000001", "NaN"),
        source.replace("217.125", "INF"),
        source.replace("217.125", "-INF"),
        source.replace("600.00000000001", "119"),
        source.replace("400.125", "2001"),
        source.replace("598.75", "-1"),
        source.replace("<graph-size", "<graph-size unknown=\"keep\""),
        source.replace("<axes>", "<graph-size width=\"1\" height=\"1\"/><axes>"),
    ] {
        assert!(native_plot::parse(&invalid).is_err(), "{invalid}");
    }
}
#[test]
fn native_export_noedit_lexemes_results_and_utf16_stay_exact_then_resize_patches_wrapper() {
    let mut doc = canvas(explicit());
    inert_cache(&mut doc);
    let mut pkg = package(&doc);
    let result = pkg.original_part("mathcad/result.xml").unwrap().to_vec();
    let source = pkg
        .worksheet_xml()
        .unwrap()
        .replace("598.75", "5.9875e2")
        .replace("217.125", "2.17125e2");
    // Preserve the original UTF16 encoding and source numeric spelling on no edit.
    let source = format!(
        "<?xml version=\"1.0\" encoding=\"utf-16\"?>{}",
        source.trim_start_matches("\u{feff}")
    );
    let bytes: Vec<u8> = [0xff, 0xfe]
        .into_iter()
        .chain(source.encode_utf16().flat_map(u16::to_le_bytes))
        .collect();
    pkg.set_part("mathcad/worksheet.xml", bytes).unwrap();
    doc.regions[0].source_region = pkg.regions().unwrap()[0].region_id.clone();
    doc.regions[0].content = CanvasContent::Plot(parsed(&pkg));
    doc.calculation_cache = None;
    let before = pkg.original_part("mathcad/worksheet.xml").unwrap().to_vec();
    export_into(&doc, &mut pkg).unwrap();
    assert_eq!(pkg.original_part("mathcad/worksheet.xml").unwrap(), before);
    assert_eq!(pkg.original_part("mathcad/result.xml").unwrap(), result);
    let mut pkg = NativePackage::from_bytes(&pkg.to_bytes().unwrap()).unwrap();
    let CanvasContent::Plot(spec) = &mut doc.regions[0].content else {
        panic!()
    };
    spec.set_outer_size(640., 410.).unwrap();
    export_into(&doc, &mut pkg).unwrap();
    let restored = parsed(&pkg);
    let g = restored.native_geometry.as_ref().unwrap();
    assert_eq!(
        (g.requested_width, g.requested_height),
        (Some(640.), Some(410.))
    );
    assert!(g.actual_width.is_none() && g.actual_height.is_none());
    assert!(
        g.graph_size.as_ref().unwrap().same_source(
            explicit()
                .native_geometry
                .as_ref()
                .unwrap()
                .graph_size
                .as_ref()
                .unwrap()
        )
    );
    assert!(
        pkg.caches()
            .unwrap()
            .iter()
            .all(|r| r.calculation_status.as_deref() == Some("Pending"))
    );
}
#[test]
fn old_saved_model_reuses_native_source_and_raw_curve_until_real_source_change() {
    let mut doc = canvas(PlotSpec::default());
    inert_cache(&mut doc);
    let mut pkg = package(&doc);
    doc.regions[0].source_region = Some("1".into());
    doc.calculation_cache = None;
    let before = pkg.original_part("mathcad/result.xml").unwrap().to_vec();
    let worksheet = pkg.original_part("mathcad/worksheet.xml").unwrap().to_vec();
    export_into(&doc, &mut pkg).unwrap();
    assert_eq!(pkg.original_part("mathcad/result.xml").unwrap(), before);
    assert_eq!(
        pkg.original_part("mathcad/worksheet.xml").unwrap(),
        worksheet
    );
    let CanvasContent::Plot(spec) = &mut doc.regions[0].content else {
        panic!()
    };
    *spec.primary_y_expression_mut().unwrap() = "cos(x)".into();
    export_into(&doc, &mut pkg).unwrap();
    assert_ne!(
        native_plot::trace_result_refs(&pkg.regions().unwrap()[0].raw_xml).unwrap(),
        vec![Some(1)]
    );
}
#[test]
#[ignore = "Writes generated XY geometry packages for original offline XSD validation; no target runtime"]
fn native_xy_geometry_schema_fixture() {
    eprintln!(
        "geometry_bytes={} optional_geometry_bytes={} plot_bytes={} canvas_content_bytes={}",
        std::mem::size_of::<PlotNativeGeometry>(),
        std::mem::size_of::<Option<Box<PlotNativeGeometry>>>(),
        std::mem::size_of::<PlotSpec>(),
        std::mem::size_of::<CanvasContent>()
    );
    let output = std::path::PathBuf::from(
        std::env::var_os("MATHCRAFT_NATIVE_XY_GEOMETRY_OUTPUT").expect("output"),
    );
    std::fs::create_dir_all(&output).unwrap();
    let mut doc = canvas(explicit());
    doc.add_plot(0., 500.);
    doc.regions[1].content = CanvasContent::Plot(PlotSpec::blank_xy());
    let pkg = package(&doc);
    pkg.write_new(&output.join("geometry-source.mcdx")).unwrap();
    std::fs::write(output.join("worksheet.xml"), pkg.worksheet_xml().unwrap()).unwrap();
    std::fs::write(
        output.join("geometry-source.mathcraft"),
        serde_json::to_vec(&doc).unwrap(),
    )
    .unwrap();
}
