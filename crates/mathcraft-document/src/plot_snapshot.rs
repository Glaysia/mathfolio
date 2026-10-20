//! Bounded inert XY presentations. Ordinal slots never serve as expression input.
use crate::canvas::{PlotSource, PlotSpec, PlotTraceSpec, PlotTraceStyle};
use crate::plot_axis_presentation::PlotAxisPresentation;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SavedPlotCurve {
    pub points: Vec<[f64; 2]>,
    pub implicit_domain: bool,
    pub label: String,
    pub style: PlotTraceStyle,
    #[serde(deserialize_with = "required_option")]
    pub x_presentation: Option<Box<PlotAxisPresentation>>,
    #[serde(deserialize_with = "required_option")]
    pub y_presentation: Option<Box<PlotAxisPresentation>>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum SavedPlotTrace {
    Ready {
        data: SavedPlotCurve,
    },
    Failed {
        message: String,
    },
    Pending {
        #[serde(deserialize_with = "required_previous")]
        previous: Option<SavedPlotCurve>,
    },
    Incomplete {},
    Unavailable {
        message: String,
    },
}
fn required_option<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<Option<T>, D::Error> {
    Option::deserialize(d)
}
fn required_previous<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<SavedPlotCurve>, D::Error> {
    Option::deserialize(deserializer)
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyCurve {
    points: Vec<[f64; 2]>,
    implicit_domain: bool,
    label: String,
    style: PlotTraceStyle,
}
impl From<LegacyCurve> for SavedPlotCurve {
    fn from(v: LegacyCurve) -> Self {
        Self {
            points: v.points,
            implicit_domain: v.implicit_domain,
            label: v.label,
            style: v.style,
            x_presentation: None,
            y_presentation: None,
        }
    }
}
#[derive(Debug, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
enum LegacyTrace {
    Ready {
        data: LegacyCurve,
    },
    Failed {
        message: String,
    },
    Pending {
        #[serde(deserialize_with = "required_option")]
        previous: Option<LegacyCurve>,
    },
    Incomplete {},
    Unavailable {
        message: String,
    },
}
impl From<LegacyTrace> for SavedPlotTrace {
    fn from(v: LegacyTrace) -> Self {
        match v {
            LegacyTrace::Ready { data } => Self::Ready { data: data.into() },
            LegacyTrace::Failed { message } => Self::Failed { message },
            LegacyTrace::Pending { previous } => Self::Pending {
                previous: previous.map(Into::into),
            },
            LegacyTrace::Incomplete {} => Self::Incomplete {},
            LegacyTrace::Unavailable { message } => Self::Unavailable { message },
        }
    }
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Legacy {
    version: u8,
    traces: Vec<LegacyCurve>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MixedLegacy {
    version: u8,
    traces: Vec<LegacyTrace>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Mixed {
    version: u8,
    traces: Vec<SavedPlotTrace>,
}
#[derive(Deserialize)]
struct Version {
    version: u8,
}
#[derive(Clone, Debug, PartialEq)]
pub struct SavedPlotSnapshot {
    pub version: u8,
    pub traces: Vec<SavedPlotTrace>,
}
impl SavedPlotCurve {
    fn validate(&self) -> Result<(), String> {
        if self.points.len() > 2000 || self.points.iter().flatten().any(|value| !value.is_finite())
        {
            return Err("Saved plot curve exceeds the finite point budget".into());
        }
        for axis in [&self.x_presentation, &self.y_presentation]
            .into_iter()
            .flatten()
        {
            axis.validate()?;
        }
        PlotSpec {
            source: PlotSource::legacy(
                "x",
                "x",
                vec![PlotTraceSpec {
                    label: self.label.clone(),
                    style: self.style,
                    ..Default::default()
                }],
            ),
            ..Default::default()
        }
        .validate()
    }
}
impl SavedPlotSnapshot {
    pub fn parse(source: &str) -> Result<Self, String> {
        if source.len() > 2 * 1024 * 1024 {
            return Err("Saved plot presentation exceeds budget".into());
        }
        // Select the exact version grammar before decoding curves. A Value
        // intermediate would erase duplicate keys; untagged versions cannot
        // distinguish a v2/v3 all-Incomplete snapshot reliably.
        let version = serde_json::from_str::<Version>(source)
            .map_err(|e| e.to_string())?
            .version;
        let parsed = match version {
            1 => {
                let v: Legacy = serde_json::from_str(source).map_err(|e| e.to_string())?;
                Self {
                    version: v.version,
                    traces: v
                        .traces
                        .into_iter()
                        .map(|data| SavedPlotTrace::Ready { data: data.into() })
                        .collect(),
                }
            }
            2 => {
                let v: MixedLegacy = serde_json::from_str(source).map_err(|e| e.to_string())?;
                Self {
                    version: v.version,
                    traces: v.traces.into_iter().map(Into::into).collect(),
                }
            }
            3 => {
                let v: Mixed = serde_json::from_str(source).map_err(|e| e.to_string())?;
                Self {
                    version: v.version,
                    traces: v.traces,
                }
            }
            _ => return Err("Unsupported saved plot version".into()),
        };
        if !(1..=PlotSpec::MAX_TRACES).contains(&parsed.traces.len()) {
            return Err("Saved plot slot count exceeds limits".into());
        }
        for trace in &parsed.traces {
            match trace {
                SavedPlotTrace::Ready { data }
                | SavedPlotTrace::Pending {
                    previous: Some(data),
                } => data.validate()?,
                SavedPlotTrace::Failed { message } | SavedPlotTrace::Unavailable { message }
                    if message.len() > 16 * 1024 =>
                {
                    return Err("Saved plot diagnostic exceeds limits".into());
                }
                _ => {}
            }
        }
        Ok(parsed)
    }
    pub fn parse_for_spec(source: &str, spec: &PlotSpec) -> Result<Self, String> {
        spec.validate()?;
        let parsed = Self::parse(source)?;
        if parsed.traces.len() != spec.trace_count() {
            return Err("Saved plot slot/source count mismatch".into());
        }
        for (saved, source) in parsed.traces.iter().zip(spec.paired_sources()) {
            if let SavedPlotTrace::Ready { data } = saved {
                if data.label != source.label || data.style != *source.style {
                    return Err("Saved ready plot style/source mismatch".into());
                }
                for (presentation, scaling) in [
                    (&data.x_presentation, source.x.scaling),
                    (&data.y_presentation, source.y.scaling),
                ] {
                    if match presentation {
                        Some(axis) => !axis.matches_source(scaling),
                        None => scaling.is_some(),
                    } {
                        return Err("Saved ready axis presentation/scaling source mismatch".into());
                    }
                }
            }
        }
        Ok(parsed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn curve() -> serde_json::Value {
        serde_json::json!({"points":[[0.,-0.],[1.,2.]],"implicit_domain":false,"label":"","style":PlotTraceStyle::default()})
    }
    #[test]
    fn legacy_and_mixed_slots_remain_positional_and_pending_may_keep_old_style() {
        let old = serde_json::json!({"version":1,"traces":[curve()]}).to_string();
        assert_eq!(
            SavedPlotSnapshot::parse_for_spec(&old, &PlotSpec::default())
                .unwrap()
                .version,
            1
        );
        let mut spec = PlotSpec::default();
        spec.add_trace(PlotTraceSpec::default()).unwrap();
        let wire=serde_json::json!({"version":2,"traces":[{"state":"failed","message":"x"},{"state":"ready","data":curve()}]}).to_string();
        let parsed = SavedPlotSnapshot::parse_for_spec(&wire, &spec).unwrap();
        assert!(matches!(parsed.traces[0], SavedPlotTrace::Failed { .. }));
        assert!(matches!(parsed.traces[1], SavedPlotTrace::Ready { .. }));
        let mut changed = curve();
        changed["style"]["line_width"] = serde_json::json!(8.);
        for state in ["ready", "pending"] {
            let slot = if state == "ready" {
                serde_json::json!({"state":state,"data":changed})
            } else {
                serde_json::json!({"state":state,"previous":changed})
            };
            let wire = serde_json::json!({"version":2,"traces":[slot]}).to_string();
            assert_eq!(
                SavedPlotSnapshot::parse_for_spec(&wire, &PlotSpec::default()).is_ok(),
                state == "pending"
            );
        }
    }
    #[test]
    fn malformed_versions_extra_fields_duplicates_and_slot_budgets_are_rejected() {
        for wire in [
            r#"{"version":2,"traces":[{"state":"incomplete","points":[] }]}"#.into(),
            r#"{"version":2,"traces":[{"state":"failed","message":"a","message":"b"}]}"#.into(),
            r#"{"version":2,"version":2,"traces":[{"state":"incomplete"}]}"#.into(),
            r#"{"version":2,"traces":[{"state":"pending"}]}"#.into(),
            serde_json::json!({"version":4,"traces":[{"state":"incomplete"}]}).to_string(),
            serde_json::json!({"version":2,"traces":[]}).to_string(),
            serde_json::json!({"version":2,"traces":vec![serde_json::json!({"state":"incomplete"});17]}).to_string(),
            serde_json::json!({"version":2,"traces":[{"state":"failed","message":"x".repeat(16385)}]}).to_string(),
        ] { assert!(SavedPlotSnapshot::parse(&wire).is_err(),"accepted {wire}"); }
    }
}

#[cfg(test)]
mod unit_presentation_tests {
    use super::*;
    use crate::canvas::{
        CanvasContent, CanvasDocument, PlotAxis, PlotScaling, SavedCalculationCache,
        SavedCalculationRow, SavedCalculationStatus,
    };
    use crate::native_plot_units::{NativeUnitMonomial, NativeUnitReference};
    use serde_json::{Value, json};
    fn scaling(labels: &str) -> PlotScaling {
        PlotScaling::from_tree(mathcraft_editor::PortableTree {
            kind: mathcraft_editor::NodeKind::NativeIdentifier {
                name: "m".into(),
                labels: labels.into(),
                contextual: Some(false),
                subscript: None,
            },
            children: vec![],
        })
        .unwrap()
        .unwrap()
    }
    fn axis(source: Option<PlotScaling>) -> Value {
        serde_json::to_value(PlotAxisPresentation::Calculated {
            source_scaling: source,
            effective_factor: 0.001,
            effective_dimension: [1., 0., 0., 0., 0., 0., 0.],
            residual_units: None,
        })
        .unwrap()
    }
    fn curve() -> Value {
        json!({"points":[[-0.,1.],[2.,3.]],"implicit_domain":false,"label":"","style":PlotTraceStyle::default(),"x_presentation":null,"y_presentation":null})
    }
    fn wire(version: u8, data: Value) -> String {
        json!({"version":version,"traces":[{"state":"ready","data":data}]}).to_string()
    }
    #[test]
    fn old_versions_reject_even_null_new_keys_and_normalize_only_four_field_curves() {
        for version in [1, 2] {
            let mut old = curve();
            old.as_object_mut().unwrap().remove("x_presentation");
            old.as_object_mut().unwrap().remove("y_presentation");
            let encode = |v: Value| {
                if version == 1 {
                    json!({"version":1,"traces":[v]}).to_string()
                } else {
                    wire(2, v)
                }
            };
            let source = encode(old.clone());
            let parsed = SavedPlotSnapshot::parse_for_spec(&source, &PlotSpec::default()).unwrap();
            assert_eq!(parsed.version, version);
            if let SavedPlotTrace::Ready { data } = &parsed.traces[0] {
                assert!(data.x_presentation.is_none() && data.y_presentation.is_none());
            } else {
                panic!()
            }
            for key in ["x_presentation", "y_presentation"] {
                let mut v = old.clone();
                v[key] = Value::Null;
                assert!(SavedPlotSnapshot::parse(&encode(v)).is_err());
            }
        }
        assert!(
            SavedPlotSnapshot::parse(&json!({"version":3,"traces":[curve()]}).to_string()).is_err()
        );
        for version in [2, 3] {
            assert_eq!(
                SavedPlotSnapshot::parse(
                    &json!({"version":version,"traces":[{"state":"incomplete"}]}).to_string()
                )
                .unwrap()
                .version,
                version
            );
        }
    }
    #[test]
    fn v3_required_keys_duplicates_point_and_metadata_budgets_are_not_ignored() {
        let valid = wire(3, curve());
        assert!(SavedPlotSnapshot::parse(&valid).is_ok());
        for key in ["x_presentation", "y_presentation"] {
            let mut data = curve();
            data.as_object_mut().unwrap().remove(key);
            assert!(SavedPlotSnapshot::parse(&wire(3, data)).is_err());
        }
        for bad in [
            valid.replace(
                "\"x_presentation\":null",
                "\"x_presentation\":null,\"x_presentation\":null",
            ),
            valid.replace("\"version\":3", "\"version\":3,\"version\":3"),
            valid.replace(
                "\"state\":\"ready\"",
                "\"state\":\"ready\",\"state\":\"ready\"",
            ),
            valid.replace("\"label\":\"\"", "\"label\":\"\",\"extra\":true"),
        ] {
            assert!(SavedPlotSnapshot::parse(&bad).is_err(), "{bad}");
        }
        let mut data = curve();
        data["points"] = json!(vec![[0., 1.]; 2001]);
        assert!(SavedPlotSnapshot::parse(&wire(3, data)).is_err());
        let mut data = curve();
        data["points"] = json!([[0., null]]);
        assert!(SavedPlotSnapshot::parse(&wire(3, data)).is_err());
        let mut data = curve();
        data["x_presentation"] = axis(None);
        data["x_presentation"]["effective_factor"] = json!(0);
        assert!(SavedPlotSnapshot::parse(&wire(3, data)).is_err());
        let mut data = curve();
        data["x_presentation"] = json!({"origin":"native","source_scaling":null,"residual_units":{"system":null,"references":vec![json!({"unit":"m","power_numerator":null,"power_denominator":null});33]}});
        assert!(SavedPlotSnapshot::parse(&wire(3, data)).is_err());
        assert!(SavedPlotSnapshot::parse(&" ".repeat(2 * 1024 * 1024 + 1)).is_err());
    }
    #[test]
    fn ready_scaling_requires_exact_association_but_pending_keeps_historical_source_style() {
        let mut spec = PlotSpec::default();
        spec.set_axis_scaling(PlotAxis::X, 0, Some(scaling("UNIT")))
            .unwrap();
        for version in [1, 2] {
            let mut data = curve();
            data.as_object_mut().unwrap().remove("x_presentation");
            data.as_object_mut().unwrap().remove("y_presentation");
            let source = if version == 1 {
                json!({"version":1,"traces":[data]}).to_string()
            } else {
                wire(2, data)
            };
            assert!(SavedPlotSnapshot::parse_for_spec(&source, &spec).is_err());
        }
        assert!(SavedPlotSnapshot::parse_for_spec(&wire(3, curve()), &spec).is_err());
        let mut data = curve();
        data["x_presentation"] = axis(Some(scaling("UNIT")));
        assert!(SavedPlotSnapshot::parse_for_spec(&wire(3, data.clone()), &spec).is_ok());
        assert!(
            SavedPlotSnapshot::parse_for_spec(&wire(3, data.clone()), &PlotSpec::default())
                .is_err()
        );
        data["x_presentation"] = axis(Some(scaling("VARIABLE")));
        assert!(SavedPlotSnapshot::parse_for_spec(&wire(3, data.clone()), &spec).is_err());
        data["style"]["line_width"] = json!(8.);
        data["label"] = json!("previous source");
        let pending =
            json!({"version":3,"traces":[{"state":"pending","previous":data}]}).to_string();
        assert!(SavedPlotSnapshot::parse_for_spec(&pending, &spec).is_ok());
        let mut old = curve();
        old.as_object_mut().unwrap().remove("x_presentation");
        old.as_object_mut().unwrap().remove("y_presentation");
        assert!(
            SavedPlotSnapshot::parse_for_spec(
                &json!({"version":2,"traces":[{"state":"pending","previous":old}]}).to_string(),
                &spec
            )
            .is_ok()
        );
    }
    #[test]
    fn native_metadata_reopens_inert_without_binding_units_as_scaling_or_executing_source() {
        let mut data = curve();
        data["y_presentation"] = serde_json::to_value(PlotAxisPresentation::Native {
            source_scaling: None,
            residual_units: Some(NativeUnitMonomial {
                system: Some("saved-system".into()),
                references: vec![NativeUnitReference {
                    unit: "unknown-engine-unit𝛼".into(),
                    power_numerator: Some(-1),
                    power_denominator: Some(2),
                }],
            }),
        })
        .unwrap();
        let source = wire(3, data);
        let mut doc = CanvasDocument::default();
        let id = doc.add_plot(0., 0.);
        if let CanvasContent::Plot(spec) = &mut doc.regions[0].content {
            *spec.primary_y_expression_mut().unwrap() = "undefined_call(x)".into();
        }
        doc.calculation_cache = Some(SavedCalculationCache {
            source_signature: doc.calculation_signature(),
            rows: vec![SavedCalculationRow {
                region_id: id,
                status: SavedCalculationStatus::Synchronized,
                display: Some(source.clone()),
                error: None,
                warnings: vec![],
                symbolic_mathml: None,
                numeric_source: None,
            }],
        });
        doc.validate().unwrap();
        let reopened: CanvasDocument =
            serde_json::from_slice(&serde_json::to_vec(&doc).unwrap()).unwrap();
        reopened.validate().unwrap();
        let row = &reopened.saved_calculation_cache().unwrap().rows[0];
        assert_eq!(row.display.as_deref(), Some(source.as_str()));
        let CanvasContent::Plot(spec) = &reopened.regions[0].content else {
            panic!()
        };
        let result =
            SavedPlotSnapshot::parse_for_spec(row.display.as_deref().unwrap(), spec).unwrap();
        let SavedPlotTrace::Ready { data } = &result.traces[0] else {
            panic!()
        };
        assert_eq!(data.points[0][0].to_bits(), (-0_f64).to_bits());
        let native = data.y_presentation.as_ref().unwrap();
        assert!(native.source_scaling().is_none());
        assert_eq!(
            native.residual_units().unwrap().references[0].unit,
            "unknown-engine-unit𝛼"
        );
    }
}
