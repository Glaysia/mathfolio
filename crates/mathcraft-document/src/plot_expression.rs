//! Typed first-math plot source; source labels are never inferred from spelling.
use mathcraft_compat::editor_bridge;
use mathcraft_editor::{BinaryOp, NodeKind, PortableTree};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub const MAX_EXPRESSION_NODES: usize = 1024;
pub const MAX_EXPRESSION_BYTES: usize = 64 * 1024;
pub const MAX_PLOT_EXPRESSION_NODES: usize = 8192;
pub const MAX_PLOT_EXPRESSION_BYTES: usize = 256 * 1024;
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlotExpressionTree(Box<ValidatedExpression>);
#[derive(Clone, Debug, Eq, PartialEq)]
// Only from_tree can construct this carrier, and no mutable tree is exposed.
// These receipts prove structural admission, never name resolution or evaluation.
struct ValidatedExpression {
    tree: PortableTree,
    nodes: usize,
    source_bytes: usize,
}
struct JsonSize(usize);
impl std::io::Write for JsonSize {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0 = self.0.saturating_add(bytes.len());
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlotExpression {
    LegacyText(String),
    Typed(PlotExpressionTree),
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PlotExpressionRef<'a> {
    LegacyText(&'a str),
    Typed(&'a PortableTree),
}
impl Default for PlotExpression {
    fn default() -> Self {
        Self::LegacyText(String::new())
    }
}
impl From<String> for PlotExpression {
    fn from(s: String) -> Self {
        Self::LegacyText(s)
    }
}
impl From<&str> for PlotExpression {
    fn from(s: &str) -> Self {
        Self::LegacyText(s.into())
    }
}
impl PlotExpression {
    pub fn from_tree(tree: PortableTree) -> Result<Self, String> {
        let nodes = validate_tree(&tree)?;
        let mut value = Self::Typed(PlotExpressionTree(Box::new(ValidatedExpression {
            tree,
            nodes,
            source_bytes: usize::MAX,
        })));
        // Count exactly the existing serde wire without retaining a JSON buffer.
        // Keep the old source_bytes failure sentinel and aggregate error ordering.
        let mut size = JsonSize(0);
        let source_bytes = serde_json::to_writer(&mut size, &value).map_or(usize::MAX, |_| size.0);
        if let Self::Typed(t) = &mut value {
            t.0.source_bytes = source_bytes;
        }
        Ok(value)
    }
    pub fn as_ref(&self) -> PlotExpressionRef<'_> {
        match self {
            Self::LegacyText(s) => PlotExpressionRef::LegacyText(s),
            Self::Typed(t) => PlotExpressionRef::Typed(&t.0.tree),
        }
    }
    pub fn as_legacy_text(&self) -> Option<&str> {
        self.as_ref().as_legacy_text()
    }
    pub fn as_legacy_text_mut(&mut self) -> Option<&mut String> {
        match self {
            Self::LegacyText(s) => Some(s),
            Self::Typed(_) => None,
        }
    }
    pub fn as_tree(&self) -> Option<&PortableTree> {
        self.as_ref().as_tree()
    }
    pub fn is_complete(&self) -> bool {
        self.as_ref().is_complete()
    }
    pub fn is_exact_placeholder(&self) -> bool {
        self.as_ref().is_exact_placeholder()
    }
    pub fn validate(&self) -> Result<(), String> {
        match self {
            Self::LegacyText(s) => {
                if s.len() > 4096 {
                    Err("Plot expression exceeds 4096 bytes".into())
                } else {
                    Ok(())
                }
            }
            // The private, immutable carrier has already passed validate_tree.
            Self::Typed(_) => Ok(()),
        }
    }
    pub(super) fn source_bytes(&self) -> usize {
        match self {
            Self::LegacyText(s) => s.len(),
            Self::Typed(t) => t.0.source_bytes,
        }
    }
}
impl<'a> PlotExpressionRef<'a> {
    pub fn as_legacy_text(self) -> Option<&'a str> {
        match self {
            Self::LegacyText(s) => Some(s),
            Self::Typed(_) => None,
        }
    }
    pub fn as_tree(self) -> Option<&'a PortableTree> {
        match self {
            Self::Typed(t) => Some(t),
            Self::LegacyText(_) => None,
        }
    }
    pub fn is_exact_placeholder(self) -> bool {
        match self {
            Self::LegacyText(s) => s.is_empty(),
            Self::Typed(t) => matches!(t.kind, NodeKind::Placeholder),
        }
    }
    pub fn is_complete(self) -> bool {
        match self {
            Self::LegacyText(s) => !s.trim().is_empty() && mathcraft_core::parse(s).is_ok(),
            Self::Typed(t) => {
                let mut stack = vec![t];
                while let Some(n) = stack.pop() {
                    if matches!(n.kind, NodeKind::Placeholder) {
                        return false;
                    }
                    stack.extend(&n.children);
                }
                true
            }
        }
    }
}
fn validate_tree(tree: &PortableTree) -> Result<usize, String> {
    tree.validate()
        .map_err(|e| format!("Invalid plot expression: {e}"))?;
    let mut stack = vec![tree];
    let mut nodes = 0usize;
    let mut text = 0usize;
    while let Some(n) = stack.pop() {
        nodes += 1;
        if nodes > MAX_EXPRESSION_NODES {
            return Err("Plot expression node budget exceeded".into());
        }
        match &n.kind {
            NodeKind::Placeholder
            | NodeKind::UnaryNeg
            | NodeKind::LogicalNot
            | NodeKind::Root
            | NodeKind::Absolute
            | NodeKind::Factorial
            | NodeKind::Percent
            | NodeKind::Degree
            | NodeKind::Transpose
            | NodeKind::Conjugate
            | NodeKind::Vectorize
            | NodeKind::Determinant
            | NodeKind::Row
            | NodeKind::Column
            | NodeKind::Call { .. }
            | NodeKind::Matrix { .. }
            | NodeKind::Index { .. }
            | NodeKind::Range { .. }
            | NodeKind::Integral
            | NodeKind::Derivative { .. }
            | NodeKind::Summation
            | NodeKind::Product
            | NodeKind::Fraction
            | NodeKind::Power
            | NodeKind::Group => {}
            NodeKind::Binary(op)
                if !matches!(
                    op,
                    BinaryOp::Define | BinaryOp::GlobalDefine | BinaryOp::Evaluate
                ) => {}
            // E017e/008d admits STRING in AllowExpressionsOnly editors; numeric
            // suitability belongs to trace calculation, not source admission.
            NodeKind::BasedNumber { literal, .. } => text = text.saturating_add(literal.len()),
            NodeKind::Number(s)
            | NodeKind::Imaginary(s)
            | NodeKind::Identifier(s)
            | NodeKind::StringLiteral(s) => text = text.saturating_add(s.len()),
            NodeKind::ImaginarySymbol { value, symbol } => {
                text = text
                    .saturating_add(value.len())
                    .saturating_add(symbol.len())
            }
            NodeKind::AnnotatedIdentifier { name, labels } => {
                text = text.saturating_add(name.len()).saturating_add(labels.len())
            }
            NodeKind::NativeIdentifier {
                name,
                labels,
                subscript,
                ..
            } => {
                text = text
                    .saturating_add(name.len())
                    .saturating_add(labels.len())
                    .saturating_add(subscript.as_ref().map_or(0, String::len))
            }
            _ => {
                return Err(
                    "Plot expression requires a supported expression, not a statement".into(),
                );
            }
        }
        if text > MAX_EXPRESSION_BYTES {
            return Err("Plot expression text budget exceeded".into());
        }
        stack.extend(&n.children);
    }
    let xml = editor_bridge::tree_to_mathml(tree)?;
    if xml.len() > MAX_EXPRESSION_BYTES {
        return Err("Plot expression serialized budget exceeded".into());
    }
    // Do not canonicalize away source metadata in the authoritative tree.
    Ok(nodes)
}
pub(super) fn expression_budget<'a>(
    values: impl IntoIterator<Item = &'a PlotExpression>,
) -> Result<(), String> {
    let (mut nodes, mut bytes) = (0usize, 0usize);
    for value in values {
        value.validate()?;
        bytes = bytes.saturating_add(value.source_bytes());
        if let PlotExpression::Typed(t) = value {
            nodes = nodes.saturating_add(t.0.nodes);
            if nodes > MAX_PLOT_EXPRESSION_NODES {
                return Err("Plot expression aggregate node budget exceeded".into());
            }
        }
        if bytes > MAX_PLOT_EXPRESSION_BYTES {
            return Err("Plot expression aggregate byte budget exceeded".into());
        }
    }
    Ok(())
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TypedWire {
    tree: PortableTree,
}
impl Serialize for PlotExpression {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::LegacyText(text) => text.serialize(s),
            Self::Typed(t) => {
                use serde::ser::SerializeStruct;
                let mut st = s.serialize_struct("PlotExpression", 1)?;
                st.serialize_field("tree", &t.0.tree)?;
                st.end()
            }
        }
    }
}
impl<'de> Deserialize<'de> for PlotExpression {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Wire {
            Text(String),
            Tree(TypedWire),
        }
        let value = match Wire::deserialize(d)? {
            Wire::Text(s) => Self::LegacyText(s),
            Wire::Tree(t) => Self::from_tree(t.tree).map_err(serde::de::Error::custom)?,
        };
        value.validate().map_err(serde::de::Error::custom)?;
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas::{
        CanvasContent, CanvasDocument, PlotAxis, PlotSource, PlotSpec, PlotTraceAppearance,
    };
    fn leaf(kind: NodeKind) -> PortableTree {
        PortableTree {
            kind,
            children: vec![],
        }
    }
    fn id() -> PortableTree {
        leaf(NodeKind::NativeIdentifier {
            name: "I".into(),
            labels: "VARIABLE".into(),
            contextual: Some(false),
            subscript: Some("dc𝛼".into()),
        })
    }
    #[test]
    fn legacy_wire_is_exact_and_typed_wire_has_one_strict_authority() {
        let text = "\"x +  2\"";
        let value: PlotExpression = serde_json::from_str(text).unwrap();
        assert_eq!(serde_json::to_string(&value).unwrap(), text);
        assert_eq!(value.as_legacy_text(), Some("x +  2"));
        let typed = PlotExpression::from_tree(id()).unwrap();
        let json = serde_json::to_string(&typed).unwrap();
        assert_eq!(
            serde_json::from_str::<PlotExpression>(&json).unwrap(),
            typed
        );
        assert!(typed.as_legacy_text().is_none());
        assert_eq!(typed.as_tree(), Some(&id()));
        for bad in [
            "null".to_string(),
            "{}".into(),
            json.replacen("{", "{\"future\":0,", 1),
            json.replacen("{", "{\"tree\":null,", 1),
        ] {
            assert!(
                serde_json::from_str::<PlotExpression>(&bad).is_err(),
                "{bad}"
            );
        }
    }
    #[test]
    fn typed_placeholder_is_not_absent_and_partial_tree_is_not_complete() {
        let blank = PlotExpression::from_tree(leaf(NodeKind::Placeholder)).unwrap();
        assert!(blank.as_tree().is_some());
        assert!(blank.is_exact_placeholder());
        assert!(!blank.is_complete());
        let partial = PlotExpression::from_tree(PortableTree {
            kind: NodeKind::Fraction,
            children: vec![id(), leaf(NodeKind::Placeholder)],
        })
        .unwrap();
        assert!(!partial.is_exact_placeholder());
        assert!(!partial.is_complete());
        assert!(PlotExpression::from_tree(id()).unwrap().is_complete());
    }
    #[test]
    fn forbidden_statements_and_aggregate_budget_are_explicit_atomic_guards() {
        let ph = leaf(NodeKind::Placeholder);
        let define = PortableTree {
            kind: NodeKind::Binary(BinaryOp::Define),
            children: vec![ph.clone(), ph.clone()],
        };
        assert!(PlotExpression::from_tree(define).is_err());
        let cell = leaf(NodeKind::Number("1".into()));
        let matrix = PortableTree {
            kind: NodeKind::Matrix {
                rows: 32,
                cols: 32,
                display: None,
            },
            children: vec![cell; 1024],
        };
        assert!(PlotExpression::from_tree(matrix).is_err());
        let mut spec = PlotSpec::typed_blank_xy();
        let before = spec.clone();
        assert!(
            spec.insert_typed_axis_equation(PlotAxis::Y, 9, ph, None)
                .is_err()
        );
        assert_eq!(spec, before);
        let big = PlotExpression::from_tree(PortableTree {
            kind: NodeKind::Matrix {
                rows: 31,
                cols: 32,
                display: None,
            },
            children: vec![leaf(NodeKind::Number("1".into())); 992],
        })
        .unwrap();
        let values = vec![big; 9];
        assert!(expression_budget(&values).is_err());
    }
    #[test]
    fn typed_edits_preserve_identity_scale_and_source_signature_without_legacy_projection() {
        let mut spec = PlotSpec {
            source: PlotSource::axis_lists(
                vec!["x".into()],
                vec!["x".into(), "x".into()],
                vec![PlotTraceAppearance::default(); 2],
            )
            .unwrap(),
            ..Default::default()
        };
        let ids = spec.axis_lists().unwrap().clone();
        let mut doc = CanvasDocument::default();
        doc.add_plot(0., 0.);
        doc.regions[0].content = CanvasContent::Plot(spec.clone());
        let old = doc.calculation_signature();
        assert!(
            spec.set_axis_expression(PlotAxis::X, 0, PlotExpression::from_tree(id()).unwrap())
                .unwrap()
        );
        assert_eq!(spec.axis_lists().unwrap().x[0].id, ids.x[0].id);
        assert_eq!(spec.axis_lists().unwrap().y, ids.y);
        assert!(spec.primary_x_expression_mut().is_none());
        assert!(spec.resolved_traces().is_err());
        assert_eq!(
            spec.paired_sources().nth(1).unwrap().x.expression.as_tree(),
            Some(&id())
        );
        doc.regions[0].content = CanvasContent::Plot(spec.clone());
        assert_ne!(old, doc.calculation_signature());
        spec.clear_axis_equation(PlotAxis::X, 0).unwrap();
        assert!(spec.primary_x_expression().as_tree().is_some());
        assert!(spec.primary_x_expression().is_exact_placeholder());
    }
}

#[cfg(test)]
#[path = "plot_expression_receipt_tests.rs"]
mod receipt_oracles;

#[cfg(test)]
#[path = "plot_expression_string_tests.rs"]
mod string_source_tests;
