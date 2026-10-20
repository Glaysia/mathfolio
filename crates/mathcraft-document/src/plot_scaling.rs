//! Typed user scaling source (0170/0174), not automatic result-unit decoration.
use super::{PlotAxis, PlotExpressionRef, PlotSource, PlotSpec, PlotTraceSpec, PlotTraceStyle};
use mathcraft_compat::editor_bridge;
use mathcraft_editor::{BinaryOp, NodeKind, PortableTree};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub const MAX_SCALING_NODES: usize = 1024;
pub const MAX_PLOT_SCALING_NODES: usize = 8192;
pub const MAX_PLOT_SCALING_BYTES: usize = 128 * 1024;
/// A validated expression tree. No string parser may infer its identifier labels.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlotScaling(Box<PortableTree>);
impl PlotScaling {
    pub fn from_tree(tree: PortableTree) -> Result<Option<Self>, String> {
        tree.validate()
            .map_err(|e| format!("Invalid plot scaling: {e}"))?;
        if matches!(tree.kind, NodeKind::Placeholder) {
            return Ok(None);
        }
        let mut stack = vec![&tree];
        let mut nodes = 0;
        let mut text_bytes = 0usize;
        while let Some(node) = stack.pop() {
            nodes += 1;
            text_bytes = text_bytes.saturating_add(match &node.kind {
                NodeKind::Identifier(text) | NodeKind::Number(text) | NodeKind::Imaginary(text) => {
                    text.len()
                }
                NodeKind::BasedNumber { literal, .. } => literal.len(),
                NodeKind::ImaginarySymbol { value, .. } => value.len(),
                NodeKind::AnnotatedIdentifier { name, .. }
                | NodeKind::NativeIdentifier { name, .. } => name.len(),
                _ => 0,
            });
            if let NodeKind::AnnotatedIdentifier { labels, .. }
            | NodeKind::NativeIdentifier { labels, .. } = &node.kind
            {
                text_bytes = text_bytes.saturating_add(labels.len());
            }
            if let NodeKind::NativeIdentifier {
                subscript: Some(suffix),
                ..
            } = &node.kind
            {
                text_bytes = text_bytes.saturating_add(suffix.len());
            }
            if text_bytes > MAX_PLOT_SCALING_BYTES {
                return Err("Plot scaling exceeds text budget".into());
            }
            if nodes > MAX_SCALING_NODES {
                return Err("Plot scaling exceeds node budget".into());
            }
            if matches!(
                node.kind,
                NodeKind::Binary(BinaryOp::Define | BinaryOp::GlobalDefine | BinaryOp::Evaluate)
                    | NodeKind::Function { .. }
                    | NodeKind::Program { .. }
                    | NodeKind::LocalDefine
                    | NodeKind::If { .. }
                    | NodeKind::For
                    | NodeKind::While
                    | NodeKind::Return
                    | NodeKind::Break
                    | NodeKind::Continue
                    | NodeKind::Try
                    | NodeKind::SymbolicEvaluation { .. }
                    | NodeKind::SymbolicCommand { .. }
            ) {
                return Err("Plot scaling requires a supported expression, not a statement".into());
            }
            stack.extend(node.children.iter());
        }
        let xml = editor_bridge::tree_to_mathml(&tree)?;
        if xml.len() > MAX_PLOT_SCALING_BYTES {
            return Err("Plot scaling exceeds byte budget".into());
        }
        // Canonicalize representation-only differences (e.g. a Native id with
        // no contextual/suffix attributes) through the same native writer.
        let canonical = editor_bridge::tree_from_mathml(&xml)?;
        Ok(Some(Self(Box::new(canonical))))
    }
    pub fn as_tree(&self) -> &PortableTree {
        &self.0
    }
    pub fn into_tree(self) -> PortableTree {
        *self.0
    }
    pub fn is_complete(&self) -> bool {
        let mut stack = vec![self.as_tree()];
        while let Some(node) = stack.pop() {
            if matches!(node.kind, NodeKind::Placeholder) {
                return false;
            }
            stack.extend(&node.children);
        }
        true
    }
}
impl Serialize for PlotScaling {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(s)
    }
}
impl<'de> Deserialize<'de> for PlotScaling {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Self::from_tree(PortableTree::deserialize(d)?)
            .map_err(serde::de::Error::custom)?
            .ok_or_else(|| {
                serde::de::Error::custom(
                    "Blank scaling must be absent, not an explicit placeholder carrier",
                )
            })
    }
}
pub(super) fn present_scaling<'de, D: Deserializer<'de>>(
    d: D,
) -> Result<Option<PlotScaling>, D::Error> {
    PlotScaling::deserialize(d).map(Some)
}
pub(super) fn scaling_budget<'a>(
    values: impl IntoIterator<Item = Option<&'a PlotScaling>>,
) -> Result<usize, String> {
    let (mut count, mut bytes) = (0usize, 0usize);
    for scaling in values.into_iter().flatten() {
        let mut stack = vec![scaling.as_tree()];
        while let Some(node) = stack.pop() {
            count += 1;
            if count > MAX_PLOT_SCALING_NODES {
                return Err("Plot scaling total node budget exceeded".into());
            }
            stack.extend(&node.children);
        }
        bytes = bytes.saturating_add(
            serde_json::to_vec(scaling)
                .map_err(|e| e.to_string())?
                .len(),
        );
        if bytes > MAX_PLOT_SCALING_BYTES {
            return Err("Plot scaling total byte budget exceeded".into());
        }
    }
    Ok(bytes)
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlotEquationRef<'a> {
    pub expression: PlotExpressionRef<'a>,
    pub scaling: Option<&'a PlotScaling>,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlotPairRef<'a> {
    pub x: PlotEquationRef<'a>,
    pub y: PlotEquationRef<'a>,
    pub label: &'a str,
    pub style: &'a PlotTraceStyle,
}
fn equation(spec: &PlotSpec, x: bool, i: usize) -> Option<PlotEquationRef<'_>> {
    match &spec.source {
        PlotSource::AxisLists(a) => (if x { &a.x } else { &a.y })
            .get(i)
            .map(|e| PlotEquationRef {
                expression: e.source.as_ref(),
                scaling: e.scaling.as_ref(),
            }),
        PlotSource::Legacy {
            x_expression,
            y_expression,
            x_scaling,
            y_scaling,
            traces,
        } => {
            if traces.is_empty() {
                (i == 0).then_some(PlotEquationRef {
                    expression: PlotExpressionRef::LegacyText(if x {
                        x_expression
                    } else {
                        y_expression
                    }),
                    scaling: if x {
                        x_scaling.as_ref()
                    } else {
                        y_scaling.as_ref()
                    },
                })
            } else {
                traces.get(i).map(|t| PlotEquationRef {
                    expression: PlotExpressionRef::LegacyText(if x {
                        &t.x_expression
                    } else {
                        &t.y_expression
                    }),
                    scaling: if x {
                        t.x_scaling.as_ref()
                    } else {
                        t.y_scaling.as_ref()
                    },
                })
            }
        }
    }
}
impl PlotSpec {
    pub(super) fn all_scalings(&self) -> Vec<Option<&PlotScaling>> {
        match &self.source {
            PlotSource::AxisLists(a) => {
                a.x.iter().chain(&a.y).map(|e| e.scaling.as_ref()).collect()
            }
            PlotSource::Legacy {
                x_scaling,
                y_scaling,
                traces,
                ..
            } => [x_scaling.as_ref(), y_scaling.as_ref()]
                .into_iter()
                .chain(
                    traces
                        .iter()
                        .flat_map(|t| [t.x_scaling.as_ref(), t.y_scaling.as_ref()]),
                )
                .collect(),
        }
    }

    pub fn equation_sources(&self, x: bool) -> impl Iterator<Item = PlotEquationRef<'_>> {
        let n = self.axis_lists().map_or(
            self.trace_count(),
            |a| if x { a.x.len() } else { a.y.len() },
        );
        (0..n).filter_map(move |i| equation(self, x, i))
    }
    pub fn paired_sources(&self) -> impl Iterator<Item = PlotPairRef<'_>> {
        (0..self.trace_count()).filter_map(move |i| {
            let (x, y, label, style) = match &self.source {
                PlotSource::AxisLists(a) => {
                    let nx = a.x.len().checked_sub(1)?;
                    let ny = a.y.len().checked_sub(1)?;
                    let appearance = a.appearances.get(i)?;
                    (
                        i.min(nx),
                        i.min(ny),
                        appearance.label.as_str(),
                        &appearance.style,
                    )
                }
                PlotSource::Legacy { traces, .. } => {
                    if traces.is_empty() {
                        static DEFAULT: std::sync::LazyLock<PlotTraceSpec> =
                            std::sync::LazyLock::new(PlotTraceSpec::default);
                        (0, 0, "", &DEFAULT.style)
                    } else {
                        let t = traces.get(i)?;
                        (i, i, t.label.as_str(), &t.style)
                    }
                }
            };
            Some(PlotPairRef {
                x: equation(self, true, x)?,
                y: equation(self, false, y)?,
                label,
                style,
            })
        })
    }
    pub fn set_axis_scaling(
        &mut self,
        axis: PlotAxis,
        index: usize,
        scaling: Option<PlotScaling>,
    ) -> Result<bool, String> {
        self.validate()?;
        let mut next = self.clone();
        let x = axis == PlotAxis::X;
        let target = match &mut next.source {
            PlotSource::AxisLists(a) => (if x { &mut a.x } else { &mut a.y })
                .get_mut(index)
                .map(|e| &mut e.scaling),
            PlotSource::Legacy {
                x_scaling,
                y_scaling,
                traces,
                ..
            } => {
                if traces.is_empty() {
                    (index == 0).then_some(if x { x_scaling } else { y_scaling })
                } else {
                    traces.get_mut(index).map(|t| {
                        if x {
                            &mut t.x_scaling
                        } else {
                            &mut t.y_scaling
                        }
                    })
                }
            }
        }
        .ok_or("Plot scaling slot out of bounds")?;
        if *target == scaling {
            return Ok(false);
        }
        *target = scaling;
        next.validate()?;
        *self = next;
        Ok(true)
    }
}

#[cfg(test)]
#[path = "plot_scaling_tests.rs"]
mod tests;
