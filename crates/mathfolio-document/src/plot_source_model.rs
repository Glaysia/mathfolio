//! One authoritative plot source. Legacy JSON is a compatibility adapter;
//! axis-list identities represent independent authored slots, never deduped text.
use super::{
    PlotAxisSpec, PlotExpression, PlotExpressionRef, PlotSpec, PlotTraceSpec, PlotTraceStyle,
};
use serde::de::{self, MapAccess, Visitor};
use serde::ser::SerializeStruct;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{collections::HashSet, fmt};

#[derive(Clone, Debug, PartialEq)]
pub enum PlotSource {
    Legacy {
        x_expression: String,
        y_expression: String,
        traces: Vec<PlotTraceSpec>,
        x_scaling: Option<super::PlotScaling>,
        y_scaling: Option<super::PlotScaling>,
    },
    AxisLists(PlotAxisLists),
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlotAxisEquation {
    pub id: u64,
    pub source: PlotExpression,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "super::plot_scaling::present_scaling"
    )]
    pub scaling: Option<super::PlotScaling>,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlotTraceAppearance {
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub style: PlotTraceStyle,
}
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PlotAxisLists {
    pub next_equation_id: u64,
    pub x: Vec<PlotAxisEquation>,
    pub y: Vec<PlotAxisEquation>,
    pub appearances: Vec<PlotTraceAppearance>,
}
impl PlotAxisLists {
    pub fn validate(&self) -> Result<(), String> {
        if !(1..=PlotSpec::MAX_TRACES).contains(&self.x.len())
            || !(1..=PlotSpec::MAX_TRACES).contains(&self.y.len())
            || self.appearances.len() != self.x.len().max(self.y.len())
        {
            return Err(
                "Plot axis lists require 1–16 equations and one appearance per paired trace".into(),
            );
        }
        let mut ids = HashSet::with_capacity(self.x.len() + self.y.len());
        for equation in self.x.iter().chain(&self.y) {
            if equation.id == 0 || equation.id >= self.next_equation_id || !ids.insert(equation.id)
            {
                return Err(
                    "Plot equation identities must be positive, unique and below next_equation_id"
                        .into(),
                );
            }
            equation.source.validate()?;
        }
        super::plot_expression::expression_budget(self.x.iter().chain(&self.y).map(|e| &e.source))?;
        for appearance in &self.appearances {
            appearance_limit(&appearance.label, appearance.style)?;
        }
        super::plot_scaling::scaling_budget(
            self.x.iter().chain(&self.y).map(|e| e.scaling.as_ref()),
        )?;
        Ok(())
    }
    /// Reserve a fresh owner-local identity. Existing IDs never change/recycle.
    pub fn allocate_equation_id(&mut self) -> Result<u64, String> {
        self.validate()?;
        let next = self
            .next_equation_id
            .checked_add(1)
            .ok_or("Plot equation identity space exhausted")?;
        let id = self.next_equation_id;
        self.next_equation_id = next;
        Ok(id)
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AxisListsWire {
    next_equation_id: u64,
    x: Vec<PlotAxisEquation>,
    y: Vec<PlotAxisEquation>,
    appearances: Vec<PlotTraceAppearance>,
}
impl<'de> Deserialize<'de> for PlotAxisLists {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = AxisListsWire::deserialize(deserializer)?;
        let lists = Self {
            next_equation_id: raw.next_equation_id,
            x: raw.x,
            y: raw.y,
            appearances: raw.appearances,
        };
        lists.validate().map_err(de::Error::custom)?;
        Ok(lists)
    }
}
impl PlotSource {
    pub fn legacy(x: impl Into<String>, y: impl Into<String>, traces: Vec<PlotTraceSpec>) -> Self {
        Self::Legacy {
            x_expression: x.into(),
            y_expression: y.into(),
            traces,
            x_scaling: None,
            y_scaling: None,
        }
    }
    pub fn single(x: impl Into<String>, y: impl Into<String>) -> Self {
        Self::legacy(x, y, vec![])
    }
    pub fn axis_lists(
        x: Vec<String>,
        y: Vec<String>,
        appearances: Vec<PlotTraceAppearance>,
    ) -> Result<Self, String> {
        if !(1..=PlotSpec::MAX_TRACES).contains(&x.len())
            || !(1..=PlotSpec::MAX_TRACES).contains(&y.len())
        {
            return Err("Plot axis lists require 1–16 equations".into());
        }
        let nx = x.len();
        let lists = PlotAxisLists {
            next_equation_id: (nx + y.len() + 1) as u64,
            x: x.into_iter()
                .enumerate()
                .map(|(i, source)| PlotAxisEquation {
                    id: (i + 1) as u64,
                    source: source.into(),
                    scaling: None,
                })
                .collect(),
            y: y.into_iter()
                .enumerate()
                .map(|(i, source)| PlotAxisEquation {
                    id: (nx + i + 1) as u64,
                    source: source.into(),
                    scaling: None,
                })
                .collect(),
            appearances,
        };
        lists.validate()?;
        Ok(Self::AxisLists(lists))
    }
    /// Explicit migration keeps every legacy pair slot, even equal strings.
    pub fn into_axis_lists(self) -> Result<Self, String> {
        match self {
            Self::AxisLists(lists) => {
                lists.validate()?;
                Ok(Self::AxisLists(lists))
            }
            Self::Legacy {
                x_expression,
                y_expression,
                traces,
                x_scaling,
                y_scaling,
            } => {
                expression_limit(&x_expression)?;
                expression_limit(&y_expression)?;
                if traces.len() > PlotSpec::MAX_TRACES {
                    return Err(format!(
                        "A plot supports at most {} traces",
                        PlotSpec::MAX_TRACES
                    ));
                }
                if traces.is_empty() {
                    let mut result = Self::axis_lists(
                        vec![x_expression],
                        vec![y_expression],
                        vec![PlotTraceAppearance::default()],
                    )?;
                    if let Self::AxisLists(a) = &mut result {
                        a.x[0].scaling = x_scaling;
                        a.y[0].scaling = y_scaling;
                        a.validate()?;
                    }
                    Ok(result)
                } else {
                    let mut x = Vec::with_capacity(traces.len());
                    let mut y = Vec::with_capacity(traces.len());
                    let mut appearances = Vec::with_capacity(traces.len());
                    let mut scalings = Vec::with_capacity(traces.len());
                    for trace in traces {
                        scalings.push((trace.x_scaling, trace.y_scaling));
                        x.push(trace.x_expression);
                        y.push(trace.y_expression);
                        appearances.push(PlotTraceAppearance {
                            label: trace.label,
                            style: trace.style,
                        });
                    }
                    let mut result = Self::axis_lists(x, y, appearances)?;
                    if let Self::AxisLists(a) = &mut result {
                        for (i, (x, y)) in scalings.into_iter().enumerate() {
                            a.x[i].scaling = x;
                            a.y[i].scaling = y;
                        }
                        a.validate()?;
                    }
                    Ok(result)
                }
            }
        }
    }
}

impl Default for PlotSpec {
    fn default() -> Self {
        Self {
            source: PlotSource::single("x", "sin(x)"),
            native_geometry: None,
            width: 420.,
            height: 260.,
            x_axis: PlotAxisSpec::default(),
            y_axis: PlotAxisSpec::default(),
        }
    }
}
impl PlotSpec {
    pub const MAX_TRACES: usize = 16;
    pub fn axis_lists(&self) -> Option<&PlotAxisLists> {
        if let PlotSource::AxisLists(lists) = &self.source {
            Some(lists)
        } else {
            None
        }
    }
    pub fn axis_lists_mut(&mut self) -> Option<&mut PlotAxisLists> {
        if let PlotSource::AxisLists(lists) = &mut self.source {
            Some(lists)
        } else {
            None
        }
    }
    pub fn legacy_traces(&self) -> Option<&[PlotTraceSpec]> {
        if let PlotSource::Legacy { traces, .. } = &self.source {
            Some(traces)
        } else {
            None
        }
    }
    pub fn legacy_traces_mut(&mut self) -> Option<&mut Vec<PlotTraceSpec>> {
        if let PlotSource::Legacy { traces, .. } = &mut self.source {
            Some(traces)
        } else {
            None
        }
    }
    pub fn promote_axis_lists(&mut self) -> Result<bool, String> {
        if self.axis_lists().is_some() {
            self.validate()?;
            return Ok(false);
        }
        self.validate()?;
        self.source = self.source.clone().into_axis_lists()?;
        Ok(true)
    }
    /// Compatibility promotion never flattens an existing axis graph.
    pub fn promote_legacy_trace(&mut self) {
        if let PlotSource::Legacy {
            x_expression,
            y_expression,
            traces,
            x_scaling,
            y_scaling,
        } = &mut self.source
            && traces.is_empty()
        {
            traces.push(PlotTraceSpec {
                x_expression: x_expression.clone(),
                y_expression: y_expression.clone(),
                x_scaling: x_scaling.clone(),
                y_scaling: y_scaling.clone(),
                ..Default::default()
            });
        }
    }
    pub fn add_trace(&mut self, trace: PlotTraceSpec) -> Result<(), String> {
        if self.axis_lists().is_some() {
            return Err("Adding paired traces to axis-list source is unsupported".into());
        }
        let mut candidate = self.clone();
        candidate.promote_legacy_trace();
        let traces = candidate.legacy_traces_mut().unwrap();
        if traces.len() >= Self::MAX_TRACES {
            return Err(format!(
                "A plot supports at most {} traces",
                Self::MAX_TRACES
            ));
        }
        traces.push(trace);
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }
    pub fn remove_trace(&mut self, index: usize) -> Result<PlotTraceSpec, String> {
        if self.axis_lists().is_some() {
            return Err("Removing paired traces from axis-list source is unsupported".into());
        }
        let mut candidate = self.clone();
        candidate.promote_legacy_trace();
        let traces = candidate.legacy_traces_mut().unwrap();
        if traces.len() == 1 {
            return Err("A plot must retain at least one trace".into());
        }
        if index >= traces.len() {
            return Err("Plot trace index is out of bounds".into());
        }
        let removed = traces.remove(index);
        candidate.validate()?;
        *self = candidate;
        Ok(removed)
    }
    pub fn primary_x_expression(&self) -> PlotExpressionRef<'_> {
        match &self.source {
            PlotSource::Legacy {
                x_expression,
                traces,
                ..
            } => PlotExpressionRef::LegacyText(
                traces.first().map_or(x_expression, |t| &t.x_expression),
            ),
            PlotSource::AxisLists(lists) => lists
                .x
                .first()
                .expect("validated nonempty X list")
                .source
                .as_ref(),
        }
    }
    pub fn primary_y_expression(&self) -> PlotExpressionRef<'_> {
        match &self.source {
            PlotSource::Legacy {
                y_expression,
                traces,
                ..
            } => PlotExpressionRef::LegacyText(
                traces.first().map_or(y_expression, |t| &t.y_expression),
            ),
            PlotSource::AxisLists(lists) => lists
                .y
                .first()
                .expect("validated nonempty Y list")
                .source
                .as_ref(),
        }
    }
    /// AxisLists must retain its nonempty structural invariant while editing.
    pub fn primary_x_expression_mut(&mut self) -> Option<&mut String> {
        match &mut self.source {
            PlotSource::Legacy {
                x_expression,
                traces,
                ..
            } => Some(
                traces
                    .first_mut()
                    .map_or(x_expression, |t| &mut t.x_expression),
            ),
            PlotSource::AxisLists(lists) => lists
                .x
                .first_mut()
                .expect("validated nonempty X list")
                .source
                .as_legacy_text_mut(),
        }
    }
    pub fn primary_y_expression_mut(&mut self) -> Option<&mut String> {
        match &mut self.source {
            PlotSource::Legacy {
                y_expression,
                traces,
                ..
            } => Some(
                traces
                    .first_mut()
                    .map_or(y_expression, |t| &mut t.y_expression),
            ),
            PlotSource::AxisLists(lists) => lists
                .y
                .first_mut()
                .expect("validated nonempty Y list")
                .source
                .as_legacy_text_mut(),
        }
    }
    pub fn trace_count(&self) -> usize {
        match &self.source {
            PlotSource::Legacy { traces, .. } => traces.len().max(1),
            PlotSource::AxisLists(lists) => lists.x.len().max(lists.y.len()),
        }
    }
    pub fn resolved_traces(&self) -> Result<Vec<PlotTraceSpec>, String> {
        Ok(match &self.source {
            PlotSource::Legacy {
                x_expression,
                y_expression,
                traces,
                x_scaling,
                y_scaling,
            } => {
                if traces.is_empty() {
                    vec![PlotTraceSpec {
                        x_expression: x_expression.clone(),
                        y_expression: y_expression.clone(),
                        x_scaling: x_scaling.clone(),
                        y_scaling: y_scaling.clone(),
                        ..Default::default()
                    }]
                } else {
                    traces.clone()
                }
            }
            PlotSource::AxisLists(lists) => {
                if lists.x.is_empty() || lists.y.is_empty() {
                    return Err("Plot axes must be nonempty".into());
                }
                lists
                    .appearances
                    .iter()
                    .enumerate()
                    .map(|(i, a)| {
                        Ok(PlotTraceSpec {
                            x_expression: lists.x[i.min(lists.x.len() - 1)]
                                .source
                                .as_legacy_text()
                                .ok_or(
                                    "Typed plot expressions cannot be flattened to legacy traces",
                                )?
                                .into(),
                            y_expression: lists.y[i.min(lists.y.len() - 1)]
                                .source
                                .as_legacy_text()
                                .ok_or(
                                    "Typed plot expressions cannot be flattened to legacy traces",
                                )?
                                .into(),
                            x_scaling: lists.x[i.min(lists.x.len() - 1)].scaling.clone(),
                            y_scaling: lists.y[i.min(lists.y.len() - 1)].scaling.clone(),
                            label: a.label.clone(),
                            style: a.style,
                        })
                    })
                    .collect::<Result<Vec<_>, String>>()?
            }
        })
    }
    pub(super) fn source_text_bytes(&self) -> usize {
        let expression_bytes = match &self.source {
            PlotSource::Legacy {
                x_expression,
                y_expression,
                traces,
                ..
            } => traces.iter().fold(
                x_expression.len().saturating_add(y_expression.len()),
                |n, t| {
                    n.saturating_add(t.x_expression.len())
                        .saturating_add(t.y_expression.len())
                        .saturating_add(t.label.len())
                },
            ),
            PlotSource::AxisLists(lists) => lists
                .x
                .iter()
                .chain(&lists.y)
                .fold(0usize, |n, e| n.saturating_add(e.source.source_bytes()))
                .saturating_add(
                    lists
                        .appearances
                        .iter()
                        .fold(0usize, |n, a| n.saturating_add(a.label.len())),
                ),
        };
        self.all_scalings()
            .into_iter()
            .flatten()
            .fold(expression_bytes, |n, scale| {
                n.saturating_add(serde_json::to_vec(scale).map_or(usize::MAX, |xml| xml.len()))
            })
    }
    pub fn validate(&self) -> Result<(), String> {
        match &self.source {
            PlotSource::Legacy {
                x_expression,
                y_expression,
                traces,
                ..
            } => {
                expression_limit(x_expression)?;
                expression_limit(y_expression)?;
                if traces.len() > Self::MAX_TRACES {
                    return Err(format!(
                        "A plot supports at most {} traces",
                        Self::MAX_TRACES
                    ));
                }
                for trace in traces {
                    expression_limit(&trace.x_expression)?;
                    expression_limit(&trace.y_expression)?;
                    appearance_limit(&trace.label, trace.style)?;
                }
            }
            PlotSource::AxisLists(lists) => lists.validate()?,
        }
        super::plot_scaling::scaling_budget(self.all_scalings())?;
        for axis in [&self.x_axis, &self.y_axis] {
            if axis.label.len() > 1024 {
                return Err("Plot axis label exceeds 1024 bytes".into());
            }
            if let Some(range) = axis.range
                && (!range.min.is_finite() || !range.max.is_finite() || range.min >= range.max)
            {
                return Err("Plot axis range must be finite with min below max".into());
            }
        }
        if !self.width.is_finite()
            || !self.height.is_finite()
            || !(120. ..=2000.).contains(&self.width)
            || !(100. ..=2000.).contains(&self.height)
        {
            return Err("Plot size must be finite and within 120–2000 by 100–2000".into());
        }
        self.validate_native_legends()?;
        if let Some(geometry) = &self.native_geometry {
            let [width, height] = geometry.allocation()?;
            if width.to_bits() != self.width.to_bits() || height.to_bits() != self.height.to_bits()
            {
                return Err("Native plot allocation differs from its requested source size".into());
            }
        }
        Ok(())
    }
}
fn expression_limit(source: &str) -> Result<(), String> {
    if source.len() > 4096 {
        Err("Plot expression exceeds 4096 bytes".into())
    } else {
        Ok(())
    }
}
fn appearance_limit(label: &str, style: PlotTraceStyle) -> Result<(), String> {
    if label.len() > 256 {
        return Err("Plot trace label exceeds 256 bytes".into());
    }
    if !style.line_width.is_finite()
        || !(0.25..=9.).contains(&style.line_width)
        || !style.marker_size.is_finite()
        || !(0.25..=32.).contains(&style.marker_size)
        || style.marker_frequency == 0
        || (style.line == super::PlotLineStyle::None && style.marker == super::PlotMarker::None)
    {
        return Err("Plot trace style is outside supported limits".into());
    }
    Ok(())
}

impl Serialize for PlotSpec {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let legacy = matches!(self.source, PlotSource::Legacy { .. });
        let scaling_fields = match &self.source {
            PlotSource::Legacy {
                x_scaling,
                y_scaling,
                ..
            } => usize::from(x_scaling.is_some()) + usize::from(y_scaling.is_some()),
            _ => 0,
        };
        let mut state = serializer.serialize_struct(
            "PlotSpec",
            (if legacy { 7 } else { 5 })
                + usize::from(self.native_geometry.is_some())
                + scaling_fields,
        )?;
        match &self.source {
            PlotSource::Legacy {
                x_expression,
                y_expression,
                traces,
                x_scaling,
                y_scaling,
            } => {
                state.serialize_field("x_expression", x_expression)?;
                state.serialize_field("y_expression", y_expression)?;
                state.serialize_field("width", &self.width)?;
                state.serialize_field("height", &self.height)?;
                state.serialize_field("traces", traces)?;
                if let Some(value) = x_scaling {
                    state.serialize_field("x_scaling", value)?;
                }
                if let Some(value) = y_scaling {
                    state.serialize_field("y_scaling", value)?;
                }
            }
            PlotSource::AxisLists(lists) => {
                #[derive(Serialize)]
                struct Source<'a> {
                    kind: &'static str,
                    next_equation_id: u64,
                    x: &'a [PlotAxisEquation],
                    y: &'a [PlotAxisEquation],
                    appearances: &'a [PlotTraceAppearance],
                }
                state.serialize_field(
                    "source",
                    &Source {
                        kind: "axis_lists",
                        next_equation_id: lists.next_equation_id,
                        x: &lists.x,
                        y: &lists.y,
                        appearances: &lists.appearances,
                    },
                )?;
                state.serialize_field("width", &self.width)?;
                state.serialize_field("height", &self.height)?;
            }
        }
        state.serialize_field("x_axis", &self.x_axis)?;
        state.serialize_field("y_axis", &self.y_axis)?;
        if let Some(geometry) = &self.native_geometry {
            state.serialize_field("native_geometry", geometry)?;
        }
        state.end()
    }
}
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum SourceWire {
    AxisLists {
        next_equation_id: u64,
        x: Vec<PlotAxisEquation>,
        y: Vec<PlotAxisEquation>,
        appearances: Vec<PlotTraceAppearance>,
    },
}
impl<'de> Deserialize<'de> for PlotSpec {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct SpecVisitor;
        impl<'de> Visitor<'de> for SpecVisitor {
            type Value = PlotSpec;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a legacy plot or a tagged axis-list plot")
            }
            fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<PlotSpec, M::Error> {
                let (
                    mut x,
                    mut y,
                    mut width,
                    mut height,
                    mut traces,
                    mut x_axis,
                    mut y_axis,
                    mut source,
                    mut native_geometry,
                    mut x_scaling,
                    mut y_scaling,
                ) = (
                    None, None, None, None, None, None, None, None, None, None, None,
                );
                while let Some(key) = map.next_key::<String>()? {
                    macro_rules! field {
                        ($target:ident, $name:literal) => {{
                            if $target.is_some() {
                                return Err(de::Error::duplicate_field($name));
                            }
                            $target = Some(map.next_value()?);
                        }};
                    }
                    match key.as_str() {
                        "x_expression" => field!(x, "x_expression"),
                        "y_expression" => field!(y, "y_expression"),
                        "width" => field!(width, "width"),
                        "height" => field!(height, "height"),
                        "traces" => field!(traces, "traces"),
                        "x_axis" => field!(x_axis, "x_axis"),
                        "y_axis" => field!(y_axis, "y_axis"),
                        "source" => field!(source, "source"),
                        "native_geometry" => field!(native_geometry, "native_geometry"),
                        "x_scaling" => field!(x_scaling, "x_scaling"),
                        "y_scaling" => field!(y_scaling, "y_scaling"),
                        _ => {
                            return Err(de::Error::unknown_field(
                                &key,
                                &[
                                    "x_expression",
                                    "y_expression",
                                    "width",
                                    "height",
                                    "traces",
                                    "x_axis",
                                    "y_axis",
                                    "source",
                                    "native_geometry",
                                    "x_scaling",
                                    "y_scaling",
                                ],
                            ));
                        }
                    }
                }
                let source = match source {
                    Some(SourceWire::AxisLists {
                        next_equation_id,
                        x: axis_x,
                        y: axis_y,
                        appearances,
                    }) => {
                        if x.is_some()
                            || y.is_some()
                            || traces.is_some()
                            || x_scaling.is_some()
                            || y_scaling.is_some()
                        {
                            return Err(de::Error::custom(
                                "Mixed legacy and axis-list plot sources",
                            ));
                        }
                        let lists = PlotAxisLists {
                            next_equation_id,
                            x: axis_x,
                            y: axis_y,
                            appearances,
                        };
                        lists.validate().map_err(de::Error::custom)?;
                        PlotSource::AxisLists(lists)
                    }
                    None => PlotSource::Legacy {
                        x_expression: x.ok_or_else(|| de::Error::missing_field("x_expression"))?,
                        y_expression: y.ok_or_else(|| de::Error::missing_field("y_expression"))?,
                        traces: traces.unwrap_or_default(),
                        x_scaling,
                        y_scaling,
                    },
                };
                let spec = PlotSpec {
                    source,
                    native_geometry,
                    width: width.ok_or_else(|| de::Error::missing_field("width"))?,
                    height: height.ok_or_else(|| de::Error::missing_field("height"))?,
                    x_axis: x_axis.unwrap_or_default(),
                    y_axis: y_axis.unwrap_or_default(),
                };
                super::plot_scaling::scaling_budget(spec.all_scalings())
                    .map_err(de::Error::custom)?;
                Ok(spec)
            }
        }
        deserializer.deserialize_struct(
            "PlotSpec",
            &[
                "x_expression",
                "y_expression",
                "width",
                "height",
                "traces",
                "x_axis",
                "y_axis",
                "source",
                "native_geometry",
                "x_scaling",
                "y_scaling",
            ],
            SpecVisitor,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas::{
        CanvasContent, CanvasDocument, MathFormat, SavedCalculationCache, SavedCalculationRow,
        SavedCalculationStatus,
    };
    use sha2::{Digest, Sha256};

    // Independent frozen shape of the previous derived PlotSpec serializer.
    #[derive(Clone, Serialize)]
    struct OldTrace {
        x_expression: String,
        y_expression: String,
        label: String,
        style: PlotTraceStyle,
    }
    #[derive(Clone, Serialize)]
    struct OldPlot {
        x_expression: String,
        y_expression: String,
        width: f32,
        height: f32,
        traces: Vec<OldTrace>,
        x_axis: PlotAxisSpec,
        y_axis: PlotAxisSpec,
    }
    #[derive(Serialize)]
    enum OldContent {
        Plot(OldPlot),
    }
    #[derive(Serialize)]
    struct OldRegion {
        id: u64,
        left: f64,
        top: f64,
        content: OldContent,
        source_region: Option<String>,
        format: MathFormat,
    }
    fn lists(nx: usize, ny: usize) -> PlotSpec {
        PlotSpec {
            source: PlotSource::axis_lists(
                (0..nx).map(|i| format!("x{i}")).collect(),
                (0..ny).map(|i| format!("y{i}")).collect(),
                vec![PlotTraceAppearance::default(); nx.max(ny)],
            )
            .unwrap(),
            ..Default::default()
        }
    }
    #[test]
    fn old_derived_wire_and_persisted_cache_hash_are_byte_identical_including_inactive_fallbacks() {
        for traces in [
            vec![],
            vec![PlotTraceSpec {
                x_expression: "α\n\"x".into(),
                y_expression: "cos(x)".into(),
                label: "curve 🦀".into(),
                ..Default::default()
            }],
        ] {
            let old = OldPlot {
                x_expression: "unused literal x".into(),
                y_expression: "inactive y\t".into(),
                width: 420.,
                height: 260.,
                traces: traces
                    .into_iter()
                    .map(|trace| OldTrace {
                        x_expression: trace.x_expression,
                        y_expression: trace.y_expression,
                        label: trace.label,
                        style: trace.style,
                    })
                    .collect(),
                x_axis: PlotAxisSpec::default(),
                y_axis: PlotAxisSpec::default(),
            };
            let bytes = serde_json::to_vec(&old).unwrap();
            let spec: PlotSpec = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(serde_json::to_vec(&spec).unwrap(), bytes);
            let mut doc = CanvasDocument::default();
            let id = doc.add_plot(-0., 12.5);
            // Place exact native coordinates after the author's grid snap.
            doc.regions[0].left = -0.;
            doc.regions[0].top = 12.5;
            doc.regions[0].content = CanvasContent::Plot(spec);
            let prior_region = OldRegion {
                id,
                left: -0.,
                top: 12.5,
                content: OldContent::Plot(old),
                source_region: None,
                format: MathFormat::default(),
            };
            let prior_json =
                serde_json::to_vec(&(vec![prior_region], doc.width, doc.height, doc.grid)).unwrap();
            let mut old_hash = Sha256::new();
            old_hash.update(b"mathcraft-calculation-presentation-v1\0");
            old_hash.update(prior_json);
            let expected = format!("{:x}", old_hash.finalize());
            assert_eq!(doc.calculation_signature(), expected);
            assert_eq!(doc.active_calculation_signature(), expected);
            doc.calculation_cache = Some(SavedCalculationCache {
                source_signature: expected.clone(),
                rows: vec![SavedCalculationRow {
                    region_id: id,
                    status: SavedCalculationStatus::Synchronized,
                    display: Some("inert old points".into()),
                    error: None,
                    warnings: vec![],
                    symbolic_mathml: None,
                    numeric_source: None,
                }],
            });
            let reopened: CanvasDocument =
                serde_json::from_slice(&serde_json::to_vec(&doc).unwrap()).unwrap();
            assert_eq!(reopened.calculation_signature(), expected);
            assert_eq!(
                reopened.saved_calculation_cache().unwrap().rows[0]
                    .display
                    .as_deref(),
                Some("inert old points")
            );
        }
    }
    #[test]
    fn older_missing_defaults_and_duplicate_or_unknown_legacy_fields_keep_strict_wire() {
        let old = r#"{"x_expression":"x","y_expression":"sin(x)","width":420,"height":260}"#;
        let spec: PlotSpec = serde_json::from_str(old).unwrap();
        assert_eq!(spec, PlotSpec::default());
        let bytes = serde_json::to_string(&spec).unwrap();
        assert!(bytes.starts_with(r#"{"x_expression":"x","y_expression":"sin(x)","width":420.0,"height":260.0,"traces":[]"#));
        for invalid in [
            old.replace("\"width\":420", "\"width\":420,\"width\":500"),
            old.replace("\"width\":420", "\"future\":true,\"width\":420"),
            old.replace("\"x_expression\":\"x\"", "\"x_expression\":null"),
        ] {
            assert!(
                serde_json::from_str::<PlotSpec>(&invalid).is_err(),
                "{invalid}"
            );
        }
    }
    #[test]
    fn primary_shared_edits_update_every_paired_trace_without_replacing_equation_ids() {
        for (nx, ny) in [(1, 3), (3, 1), (2, 3), (3, 2)] {
            let mut spec = lists(nx, ny);
            let ids: Vec<_> = spec
                .axis_lists()
                .unwrap()
                .x
                .iter()
                .chain(&spec.axis_lists().unwrap().y)
                .map(|e| e.id)
                .collect();
            let next = spec.axis_lists().unwrap().next_equation_id;
            *spec.primary_x_expression_mut().unwrap() = "shared-x".into();
            *spec.primary_y_expression_mut().unwrap() = "shared-y".into();
            let traces = spec.resolved_traces().unwrap();
            assert_eq!(traces.len(), nx.max(ny));
            for (i, trace) in traces.iter().enumerate() {
                assert_eq!(
                    trace.x_expression,
                    if i == 0 || nx == 1 {
                        "shared-x".into()
                    } else {
                        format!("x{}", i.min(nx - 1))
                    }
                );
                assert_eq!(
                    trace.y_expression,
                    if i == 0 || ny == 1 {
                        "shared-y".into()
                    } else {
                        format!("y{}", i.min(ny - 1))
                    }
                );
            }
            assert_eq!(
                ids,
                spec.axis_lists()
                    .unwrap()
                    .x
                    .iter()
                    .chain(&spec.axis_lists().unwrap().y)
                    .map(|e| e.id)
                    .collect::<Vec<_>>()
            );
            assert_eq!(next, spec.axis_lists().unwrap().next_equation_id);
            let encoded = serde_json::to_vec(&spec).unwrap();
            assert_eq!(serde_json::from_slice::<PlotSpec>(&encoded).unwrap(), spec);
        }
    }
    #[test]
    fn explicit_legacy_migration_keeps_equal_strings_as_independent_slots_and_changes_source_hash()
    {
        let mut spec = PlotSpec {
            source: PlotSource::legacy(
                "ignored x",
                "ignored y",
                vec![PlotTraceSpec::default(), PlotTraceSpec::default()],
            ),
            ..Default::default()
        };
        let legacy = serde_json::to_vec(&spec).unwrap();
        assert!(spec.promote_axis_lists().unwrap());
        assert!(!spec.promote_axis_lists().unwrap());
        let graph = spec.axis_lists().unwrap();
        assert_eq!(graph.x.len(), 2);
        assert_eq!(graph.y.len(), 2);
        assert_eq!(graph.x[0].source, graph.x[1].source);
        assert_ne!(graph.x[0].id, graph.x[1].id);
        assert_ne!(graph.x[0].id, graph.y[0].id);
        *spec.primary_x_expression_mut().unwrap() = "edited".into();
        assert_eq!(spec.resolved_traces().unwrap()[1].x_expression, "x");
        assert_ne!(legacy, serde_json::to_vec(&spec).unwrap());
        let mut doc = CanvasDocument::default();
        doc.add_plot(0., 0.);
        let shared = lists(1, 2);
        let duplicated = PlotSpec {
            source: PlotSource::axis_lists(
                vec!["x0".into(), "x0".into()],
                vec!["y0".into(), "y1".into()],
                vec![PlotTraceAppearance::default(); 2],
            )
            .unwrap(),
            ..Default::default()
        };
        assert_eq!(
            shared.resolved_traces().unwrap(),
            duplicated.resolved_traces().unwrap()
        );
        doc.regions[0].content = CanvasContent::Plot(shared);
        let hash = doc.calculation_signature();
        doc.regions[0].content = CanvasContent::Plot(duplicated);
        assert_ne!(hash, doc.calculation_signature());
    }
    #[test]
    fn tagged_wire_rejects_mixed_authority_unknown_fields_duplicate_ids_and_invalid_allocator() {
        let original = serde_json::to_value(lists(1, 2)).unwrap();
        let mut invalids = Vec::new();
        for name in ["x_expression", "y_expression", "traces"] {
            let mut value = original.clone();
            value[name] = if name == "traces" {
                serde_json::json!([])
            } else {
                serde_json::json!("")
            };
            invalids.push(value);
        }
        for slot in ["x", "y"] {
            let mut value = original.clone();
            value["source"][slot][0]["id"] = serde_json::json!(0);
            invalids.push(value);
            let mut value = original.clone();
            value["source"][slot] = serde_json::json!([]);
            invalids.push(value);
        }
        let mut value = original.clone();
        value["source"]["y"][0]["id"] = value["source"]["x"][0]["id"].clone();
        invalids.push(value);
        let mut value = original.clone();
        value["source"]["next_equation_id"] = serde_json::json!(2);
        invalids.push(value);
        let mut value = original.clone();
        value["source"]["future"] = serde_json::json!(true);
        invalids.push(value);
        let mut value = original.clone();
        value["source"]["x"][0]["future"] = serde_json::json!(true);
        invalids.push(value);
        let mut value = original.clone();
        value["source"]["appearances"] = serde_json::json!([]);
        invalids.push(value);
        for value in invalids {
            assert!(serde_json::from_value::<PlotSpec>(value).is_err());
        }
        let raw = serde_json::to_string(&original).unwrap().replace(
            "\"next_equation_id\":4",
            "\"next_equation_id\":4,\"next_equation_id\":5",
        );
        assert!(serde_json::from_str::<PlotSpec>(&raw).is_err());
    }
    #[test]
    fn allocator_is_monotonic_and_axis_graph_cannot_be_flattened_by_legacy_edit_commands() {
        let mut spec = lists(2, 2);
        let original = spec.clone();
        spec.promote_legacy_trace();
        assert_eq!(spec, original);
        assert!(spec.add_trace(PlotTraceSpec::default()).is_err());
        assert!(spec.remove_trace(0).is_err());
        assert_eq!(spec, original);
        assert!(spec.legacy_traces_mut().is_none());
        let graph = spec.axis_lists_mut().unwrap();
        let first = graph.allocate_equation_id().unwrap();
        let second = graph.allocate_equation_id().unwrap();
        assert_eq!(second, first + 1);
        let original_ids: Vec<_> = graph.x.iter().chain(&graph.y).map(|e| e.id).collect();
        assert!(original_ids.iter().all(|id| *id < first));
        graph.next_equation_id = u64::MAX;
        assert!(graph.allocate_equation_id().is_err());
        assert_eq!(graph.next_equation_id, u64::MAX);
    }
    #[test]
    fn axis_structure_empty_edit_placeholders_and_text_budgets_are_checked_without_pair_duplication()
     {
        assert!(
            PlotSource::axis_lists(
                vec![],
                vec!["y".into()],
                vec![PlotTraceAppearance::default()]
            )
            .is_err()
        );
        assert!(
            PlotSource::axis_lists(
                vec!["x".into(); 17],
                vec!["y".into()],
                vec![PlotTraceAppearance::default(); 17]
            )
            .is_err()
        );
        let mut spec = PlotSpec {
            source: PlotSource::axis_lists(
                vec![String::new()],
                vec!["y".repeat(4096)],
                vec![PlotTraceAppearance::default()],
            )
            .unwrap(),
            ..Default::default()
        };
        spec.validate().unwrap();
        assert_eq!(spec.primary_x_expression().as_legacy_text().unwrap(), "");
        *spec.primary_x_expression_mut().unwrap() = "한".repeat(1366);
        assert!(spec.validate().unwrap_err().contains("4096"));
        let spec = lists(1, 16);
        let expected = spec
            .axis_lists()
            .unwrap()
            .x
            .iter()
            .chain(&spec.axis_lists().unwrap().y)
            .map(|e| e.source.source_bytes())
            .sum::<usize>();
        assert_eq!(spec.source_text_bytes(), expected);
        let legacy = PlotSpec {
            source: PlotSource::legacy("unused-x", "unused-y", vec![PlotTraceSpec::default()]),
            ..Default::default()
        };
        assert_eq!(
            legacy.source_text_bytes(),
            "unused-x".len() + "unused-y".len() + "x".len() + "sin(x)".len()
        );
    }
}
