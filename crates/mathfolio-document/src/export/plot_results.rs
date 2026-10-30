//! Bind source-signed curve snapshots to final native trace result identities.
use super::saved_results::ResultUpdate;
use super::*;
use crate::canvas::{PlotSpec, SavedCalculationStatus};
use crate::plot_snapshot::{SavedPlotCurve, SavedPlotSnapshot, SavedPlotTrace};

fn curve_xml(data: &SavedPlotCurve) -> Result<Option<String>, String> {
    let units = |axis: &Option<Box<crate::plot_axis_presentation::PlotAxisPresentation>>| {
        axis.as_ref().and_then(|a| a.residual_units()).cloned()
    };
    // User scaling is source, never an inferred native residual monomial.
    // With no metadata this delegates to the unchanged unitless writer.
    crate::native_plot_units::finite_with_units_xml(
        &data.points,
        &units(&data.x_presentation),
        &units(&data.y_presentation),
    )
}

fn actions(
    source: &str,
    spec: &PlotSpec,
    aggregate_pending: bool,
    disabled: bool,
) -> Result<Vec<Option<ResultUpdate>>, String> {
    let snapshot = SavedPlotSnapshot::parse_for_spec(source, spec)?;
    if snapshot.version == 1 && aggregate_pending {
        return Ok(snapshot
            .traces
            .iter()
            .map(|_| Some(ResultUpdate::PendingKeepPayload))
            .collect());
    }
    snapshot
        .traces
        .into_iter()
        .map(|slot| {
            Ok(match slot {
                SavedPlotTrace::Ready { data } => Some(match curve_xml(&data)? {
                    Some(xml) if disabled => ResultUpdate::PendingLiteral(xml),
                    Some(xml) => ResultUpdate::Synchronized(xml),
                    None => ResultUpdate::PendingClearPayload,
                }),
                SavedPlotTrace::Pending {
                    previous: Some(data),
                } => Some(match curve_xml(&data)? {
                    Some(xml) => ResultUpdate::PendingLiteral(xml),
                    None => ResultUpdate::PendingClearPayload,
                }),
                SavedPlotTrace::Pending { previous: None }
                | SavedPlotTrace::Failed { .. }
                | SavedPlotTrace::Incomplete { .. } => Some(ResultUpdate::PendingClearPayload),
                SavedPlotTrace::Unavailable { .. } => None,
            })
        })
        .collect()
}
pub(super) fn write(
    canvas: &CanvasDocument,
    package: &mut NativePackage,
    plots: &HashMap<u64, Vec<i32>>,
    references: &HashMap<i32, usize>,
) -> Result<usize, String> {
    let Some(cache) = canvas.saved_calculation_cache() else {
        return Ok(0);
    };
    let rows: HashMap<_, _> = cache.rows.iter().map(|row| (row.region_id, row)).collect();
    let regions: HashMap<_, _> = canvas
        .regions
        .iter()
        .map(|region| (region.id, region))
        .collect();
    let mut updates = HashMap::new();
    for (owner, refs) in plots {
        let Some(region) = regions.get(owner) else {
            continue;
        };
        let Some(row) = rows.get(owner) else { continue };
        if refs.iter().any(|id| references.get(id) != Some(&1)) {
            continue;
        }
        let CanvasContent::Plot(spec) = &region.content else {
            continue;
        };
        // Preserve each native trace's status and payload, including a mixed
        // Pending/Synchronized plot. Program/context invalidation is separate.
        if row.numeric_source.is_some()
            || (row.display.is_none() && row.error.is_none() && row.warnings.is_empty())
        {
            continue;
        }
        if row.error.is_some() || !row.warnings.is_empty() {
            for id in refs {
                updates.insert(*id, ResultUpdate::PendingClearPayload);
            }
            continue;
        }
        let values = row
            .display
            .as_deref()
            .ok_or_else(|| "Missing curve snapshot".to_string())
            .and_then(|source| {
                actions(
                    source,
                    spec,
                    row.status == SavedCalculationStatus::Pending,
                    region.calculation_disabled,
                )
            });
        match values {
            Ok(values) if values.len() == refs.len() => {
                for (id, value) in refs.iter().zip(values) {
                    if let Some(value) = value {
                        updates.insert(*id, value);
                    }
                }
            }
            _ => {
                for id in refs {
                    updates.insert(*id, ResultUpdate::PendingKeepPayload);
                }
            }
        }
    }
    saved_results::upsert_updates(package, updates)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas::{SavedCalculationCache, SavedCalculationRow, SavedNumericSource};

    fn fixture() -> CanvasDocument {
        let mut canvas = CanvasDocument::default();
        let id = canvas.add_plot(20., 20.);
        let CanvasContent::Plot(spec) = &canvas.regions[0].content else {
            panic!()
        };
        let display = serde_json::json!({"version":1,"traces":[{"points":[[0.,0.],[1.,1.]],"implicit_domain":true,"label":"","style":spec.resolved_traces().unwrap()[0].style}]}).to_string();
        canvas.calculation_cache = Some(SavedCalculationCache {
            source_signature: canvas.calculation_signature(),
            rows: vec![SavedCalculationRow {
                region_id: id,
                status: SavedCalculationStatus::Synchronized,
                display: Some(display),
                error: None,
                warnings: vec![],
                symbolic_mathml: None,
                numeric_source: None,
            }],
        });
        canvas
    }
    #[test]
    fn v3_export_uses_residual_units_never_authored_scaling_and_keeps_pending_states() {
        use crate::{
            canvas::{PlotAxis, PlotScaling},
            native_plot_units::{NativeUnitMonomial, NativeUnitReference},
            plot_axis_presentation::PlotAxisPresentation,
        };
        let mut spec = PlotSpec::default();
        let scaling = PlotScaling::from_tree(mathfolio_editor::PortableTree {
            kind: mathfolio_editor::NodeKind::AnnotatedIdentifier {
                name: "in".into(),
                labels: "UNIT".into(),
            },
            children: vec![],
        })
        .unwrap()
        .unwrap();
        spec.set_axis_scaling(PlotAxis::Y, 0, Some(scaling.clone()))
            .unwrap();
        let units = NativeUnitMonomial {
            system: Some("preserved".into()),
            references: vec![NativeUnitReference {
                unit: "meter".into(),
                power_numerator: None,
                power_denominator: Some(-2),
            }],
        };
        let data = SavedPlotCurve {
            points: vec![[0., 1.], [2., 3.]],
            implicit_domain: false,
            label: String::new(),
            style: spec.resolved_traces().unwrap()[0].style,
            x_presentation: Some(Box::new(PlotAxisPresentation::Native {
                source_scaling: None,
                residual_units: Some(units.clone()),
            })),
            y_presentation: Some(Box::new(PlotAxisPresentation::Calculated {
                source_scaling: Some(scaling),
                effective_factor: 0.0254,
                effective_dimension: [1., 0., 0., 0., 0., 0., 0.],
                residual_units: None,
            })),
        };
        for pending in [false, true] {
            let slot = if pending {
                SavedPlotTrace::Pending {
                    previous: Some(data.clone()),
                }
            } else {
                SavedPlotTrace::Ready { data: data.clone() }
            };
            let wire = serde_json::json!({"version":3,"traces":[slot]}).to_string();
            for disabled in [false, true] {
                let result = actions(&wire, &spec, false, disabled)
                    .unwrap()
                    .remove(0)
                    .unwrap();
                let xml = match result {
                    ResultUpdate::PendingLiteral(xml) if pending || disabled => xml,
                    ResultUpdate::Synchronized(xml) if !pending && !disabled => xml,
                    _ => panic!(),
                };
                let decoded = crate::native_plot_units::read_mathml_with_units(&xml)
                    .unwrap()
                    .unwrap();
                assert_eq!(decoded.range_units, Some(units.clone()));
                assert_eq!(decoded.result_units, None);
                assert!(!xml.contains("unit=\"in\""));
            }
        }
        let invalid=serde_json::json!({"version":3,"traces":[{"state":"ready","data":SavedPlotCurve{y_presentation:None,..data}}]}).to_string();
        assert!(actions(&invalid, &spec, false, false).is_err());
        for state in [
            SavedPlotTrace::Failed {
                message: "failure".into(),
            },
            SavedPlotTrace::Incomplete {},
        ] {
            let wire = serde_json::json!({"version":3,"traces":[state]}).to_string();
            assert!(matches!(
                actions(&wire, &spec, false, false).unwrap()[0],
                Some(ResultUpdate::PendingClearPayload)
            ));
        }
    }
    #[test]
    fn fresh_curve_promotes_but_pending_malformed_and_style_mismatch_do_not() {
        let canvas = fixture();
        let mut package = minimal_package(&canvas).unwrap();
        export_into(&canvas, &mut package).unwrap();
        let cache = &package.caches().unwrap()[0];
        assert_eq!(cache.calculation_status.as_deref(), Some("Synchronized"));
        assert_eq!(cache.result_kind.as_deref(), Some("Trace2dResult"));
        for variant in 0..4 {
            let mut canvas = fixture();
            let row = &mut canvas.calculation_cache.as_mut().unwrap().rows[0];
            match variant {
                0 => row.status = SavedCalculationStatus::Pending,
                1 => row.display = Some("broken JSON".into()),
                2 => {
                    let mut data: serde_json::Value =
                        serde_json::from_str(row.display.as_ref().unwrap()).unwrap();
                    data["traces"][0]["style"]["line_width"] = serde_json::json!(8.);
                    row.display = Some(data.to_string());
                }
                _ => row.error = Some("fresh calculation failed".into()),
            }
            let mut package = minimal_package(&canvas).unwrap();
            export_into(&canvas, &mut package).unwrap();
            let caches = package.caches().unwrap();
            assert_eq!(
                caches[0].calculation_status.as_deref(),
                Some("Pending"),
                "variant {variant}"
            );
            assert!(caches[0].result_mathml.is_none(), "variant {variant}");
        }
    }
    #[test]
    fn imported_pending_or_unknown_presentations_keep_original_curve_payload_exact() {
        for origin in [false, true] {
            let mut canvas = fixture();
            let mut package = minimal_package(&canvas).unwrap();
            export_into(&canvas, &mut package).unwrap();
            let old = package
                .original_part("mathcad/result.xml")
                .unwrap()
                .to_vec();
            canvas.regions[0].source_region = Some("1".into());
            let row = &mut canvas.calculation_cache.as_mut().unwrap().rows[0];
            row.status = SavedCalculationStatus::Pending;
            if origin {
                row.numeric_source = Some(SavedNumericSource::NativeMathml(
                    "inert-origin-marker".into(),
                ));
            } else {
                row.display = None;
            }
            export_into(&canvas, &mut package).unwrap();
            assert_eq!(
                package.original_part("mathcad/result.xml").unwrap(),
                old,
                "origin {origin}"
            );
        }
    }
    #[test]
    fn xml_integer_aliases_reserve_the_existing_result_space() {
        let mut canvas = fixture();
        let mut package = minimal_package(&canvas).unwrap();
        export_into(&canvas, &mut package).unwrap();
        let source = package
            .worksheet_xml()
            .unwrap()
            .replace("resultRef=\"1\"", "resultRef=\" +0001 \"");
        set_xml(&mut package, "mathcad/worksheet.xml", source).unwrap();
        let result = xml_text(package.original_part("mathcad/result.xml").unwrap())
            .unwrap()
            .replace("result-id=\"1\"", "result-id=\" +0001 \"");
        set_xml(&mut package, "mathcad/result.xml", result).unwrap();
        canvas.regions[0].source_region = Some("1".into());
        export_into(&canvas, &mut package).unwrap();
        assert_eq!(package.caches().unwrap().len(), 1);
        assert_eq!(result_references(&package).unwrap().get(&1), Some(&1));
        canvas.add_plot(20., 400.);
        export_into(&canvas, &mut package).unwrap();
        let refs = result_references(&package).unwrap();
        assert_eq!(refs.get(&1), Some(&1));
        assert_eq!(refs.get(&2), Some(&1));
    }
}
