//! Ordered trace presentation state. This is not a native engine result enum.
//! Saved data is inert: no expression or cached MathML is evaluated here.
use super::{MAX_SAVED_PLOT_BYTES, PlotData, PlotSpec, PlotTraceData, valid_saved_traces};
use serde_json::Value;

const MAX_DIAGNOSTIC_BYTES: usize = 16 * 1024;

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(target_arch = "wasm32", derive(serde::Serialize, serde::Deserialize))]
pub(crate) enum TraceOutcome {
    Ready(PlotTraceData),
    Failed(String),
    Pending { previous: Option<PlotTraceData> },
    Incomplete,
    Unavailable(String),
}

impl TraceOutcome {
    pub(crate) fn curve(&self) -> Option<&PlotTraceData> {
        match self {
            Self::Ready(curve) => Some(curve),
            Self::Pending { previous } => previous.as_ref(),
            _ => None,
        }
    }
    pub(crate) fn is_pending(&self) -> bool {
        matches!(self, Self::Pending { .. })
    }
    pub(crate) fn diagnostic(&self) -> Option<&str> {
        match self {
            Self::Failed(message) | Self::Unavailable(message) => Some(message),
            _ => None,
        }
    }
}

impl PlotData {
    #[cfg_attr(not(test), allow(dead_code))] // Optional compatibility/probe accessor.
    pub(crate) fn first_curve(&self) -> Option<&PlotTraceData> {
        self.traces.iter().find_map(TraceOutcome::curve)
    }
    pub(crate) fn has_implicit_domain(&self) -> bool {
        self.traces
            .iter()
            .filter_map(TraceOutcome::curve)
            .any(|curve| curve.implicit_domain)
    }
    pub(crate) fn has_failures(&self) -> bool {
        self.traces
            .iter()
            .any(|trace| matches!(trace, TraceOutcome::Failed(_)))
    }
    pub(crate) fn has_pending(&self) -> bool {
        self.traces.iter().any(TraceOutcome::is_pending)
    }
    pub(crate) fn has_unavailable(&self) -> bool {
        self.traces
            .iter()
            .any(|trace| matches!(trace, TraceOutcome::Unavailable(_)))
    }
    /// Same-topology invalidation. A caller changing slot identity/order must
    /// rebuild slots rather than attaching a previous curve to another source.
    pub(crate) fn invalidate(&mut self) {
        for trace in &mut self.traces {
            *trace = match std::mem::replace(trace, TraceOutcome::Pending { previous: None }) {
                TraceOutcome::Ready(curve) => TraceOutcome::Pending {
                    previous: Some(curve),
                },
                pending @ TraceOutcome::Pending { .. } => pending,
                _ => TraceOutcome::Pending { previous: None },
            };
        }
    }
    pub(crate) fn to_saved(&self) -> String {
        if self.traces.is_empty()
            || self.traces.len() > PlotSpec::MAX_TRACES
            || self.traces.iter().any(|trace| {
                trace
                    .curve()
                    .is_some_and(|curve| !valid_saved_traces(std::slice::from_ref(curve)))
                    || trace
                        .diagnostic()
                        .is_some_and(|message| message.len() > MAX_DIAGNOSTIC_BYTES)
            })
        {
            return String::new();
        }
        let all_ready = self
            .traces
            .iter()
            .all(|trace| matches!(trace, TraceOutcome::Ready(_)));
        let version = if self
            .traces
            .iter()
            .filter_map(TraceOutcome::curve)
            .any(|c| c.presentation.is_some())
        {
            3
        } else if all_ready {
            1
        } else {
            2
        };
        let curve_json = |curve: &PlotTraceData| curve_json(curve, version == 3);
        let traces:Vec<_>=self.traces.iter().map(|trace|if version == 1 {
            curve_json(trace.curve().unwrap())
        }else {match trace {
            TraceOutcome::Ready(curve)=>serde_json::json!({"state":"ready","data":curve_json(curve)}),
            TraceOutcome::Failed(message)=>serde_json::json!({"state":"failed","message":message}),
            TraceOutcome::Pending{previous}=>serde_json::json!({"state":"pending","previous":previous.as_ref().map(curve_json)}),
            TraceOutcome::Incomplete=>serde_json::json!({"state":"incomplete"}),
            TraceOutcome::Unavailable(message)=>serde_json::json!({"state":"unavailable","message":message}),
        }}).collect();
        let saved = serde_json::json!({"version":version,"traces":traces}).to_string();
        if saved.len() > MAX_SAVED_PLOT_BYTES {
            String::new()
        } else {
            saved
        }
    }
    #[cfg_attr(not(test), allow(dead_code))] // Compatibility wrapper for callers not needing wire version.
    pub(crate) fn from_saved(source: &str) -> Option<Self> {
        Self::from_saved_versioned(source).map(|(_, data)| data)
    }
    pub(crate) fn from_saved_versioned(source: &str) -> Option<(u8, Self)> {
        Self::from_snapshot(
            mathfolio_document::plot_snapshot::SavedPlotSnapshot::parse(source).ok()?,
        )
    }
    pub(crate) fn from_saved_for_spec(source: &str, spec: &PlotSpec) -> Option<(u8, Self)> {
        Self::from_snapshot(
            mathfolio_document::plot_snapshot::SavedPlotSnapshot::parse_for_spec(source, spec)
                .ok()?,
        )
    }
    fn from_snapshot(
        snapshot: mathfolio_document::plot_snapshot::SavedPlotSnapshot,
    ) -> Option<(u8, Self)> {
        use mathfolio_document::plot_snapshot::SavedPlotTrace;
        let traces = snapshot
            .traces
            .into_iter()
            .map(|trace| match trace {
                SavedPlotTrace::Ready { data } => TraceOutcome::Ready(from_curve(data)),
                SavedPlotTrace::Failed { message } => TraceOutcome::Failed(message),
                SavedPlotTrace::Pending { previous } => TraceOutcome::Pending {
                    previous: previous.map(from_curve),
                },
                SavedPlotTrace::Incomplete {} => TraceOutcome::Incomplete,
                SavedPlotTrace::Unavailable { message } => TraceOutcome::Unavailable(message),
            })
            .collect();
        Some((snapshot.version, Self { traces }))
    }
}

fn curve_json(curve: &PlotTraceData, v3: bool) -> Value {
    let mut wire = serde_json::json!({"points":curve.points.as_slice(),"implicit_domain":curve.implicit_domain,"label":curve.label,"style":curve.style});
    if v3 {
        wire["x_presentation"] =
            serde_json::to_value(curve.presentation.as_ref().and_then(|p| p.x.as_ref())).unwrap();
        wire["y_presentation"] =
            serde_json::to_value(curve.presentation.as_ref().and_then(|p| p.y.as_ref())).unwrap();
    }
    wire
}
fn from_curve(curve: mathfolio_document::plot_snapshot::SavedPlotCurve) -> PlotTraceData {
    PlotTraceData {
        presentation: super::TracePresentation::pair(curve.x_presentation, curve.y_presentation),
        points: curve.points.into(),
        implicit_domain: curve.implicit_domain,
        label: curve.label,
        style: curve.style,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn curve() -> PlotTraceData {
        PlotTraceData {
            presentation: None,
            points: vec![[1., 2.], [2., 3.]].into(),
            implicit_domain: false,
            label: "original".into(),
            style: Default::default(),
        }
    }
    #[test]
    fn mixed_wire_preserves_ordinals_and_pending_curve_without_fabricating_failed_points() {
        let ready = curve();
        let data = PlotData {
            traces: vec![
                TraceOutcome::Ready(ready.clone()),
                TraceOutcome::Failed("y軸: missing".into()),
                TraceOutcome::Pending {
                    previous: Some(ready),
                },
                TraceOutcome::Incomplete,
                TraceOutcome::Unavailable("original payload unsupported".into()),
            ],
        };
        let saved = data.to_saved();
        assert!(saved.contains("\"version\":2"));
        let restored = PlotData::from_saved(&saved).unwrap();
        assert_eq!(restored, data);
        assert!(restored.traces[1].curve().is_none());
        assert!(restored.traces[3].curve().is_none());
        assert!(restored.has_failures() && restored.has_pending() && restored.has_unavailable());
        assert_eq!(
            restored.traces[2].curve().unwrap().points,
            vec![[1., 2.], [2., 3.]]
        );
    }
    #[test]
    fn invalidation_moves_only_ready_curve_to_pending_and_is_idempotent() {
        let curve = curve();
        let pointer = curve.points.as_ptr();
        let mut data = PlotData {
            traces: vec![
                TraceOutcome::Ready(curve),
                TraceOutcome::Failed("bad".into()),
                TraceOutcome::Incomplete,
                TraceOutcome::Unavailable("unknown".into()),
            ],
        };
        data.invalidate();
        assert_eq!(data.traces[0].curve().unwrap().points.as_ptr(), pointer);
        assert!(data.traces.iter().all(TraceOutcome::is_pending));
        assert!(!data.has_failures() && !data.has_unavailable());
        assert!(data.traces[1..].iter().all(|slot| slot.curve().is_none()));
        let before = data.clone();
        data.invalidate();
        assert_eq!(data, before);
    }
    #[test]
    fn malformed_v2_states_and_oversized_diagnostics_are_rejected() {
        for slot in [
            serde_json::json!({"state":"incomplete","points":[]}),
            serde_json::json!({"state":"pending"}),
            serde_json::json!({"state":"failed","message":7}),
            serde_json::json!({"state":"unknown"}),
            serde_json::json!({"state":"unavailable","message":"x".repeat(MAX_DIAGNOSTIC_BYTES+1)}),
        ] {
            assert!(
                PlotData::from_saved(&serde_json::json!({"version":2,"traces":[slot]}).to_string())
                    .is_none()
            );
        }
        assert!(PlotData::from_saved(r#"{"version":2,"traces":[{"state":"failed","message":"first","message":"second"}]}"#).is_none());
    }
    #[test]
    fn legacy_empty_inert_curve_and_wire_version_survive_pending_migration() {
        let mut old = curve();
        old.points = Vec::new().into();
        let mut data = PlotData {
            traces: vec![TraceOutcome::Ready(old)],
        };
        let legacy = data.to_saved();
        let (version, restored) = PlotData::from_saved_versioned(&legacy).unwrap();
        assert_eq!(version, 1);
        assert_eq!(restored, data);
        assert!(restored.first_curve().unwrap().points.is_empty());
        data.invalidate();
        let (version, restored) = PlotData::from_saved_versioned(&data.to_saved()).unwrap();
        assert_eq!(version, 2);
        assert_eq!(restored, data);
        assert!(restored.has_pending());
        assert!(restored.first_curve().unwrap().points.is_empty());
    }
}
