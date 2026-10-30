//! Optional immutable proof and bounded typed label adapter. Raw XML remains
//! authoritative for forms whose parser normalization is not proven here.
use super::*;
#[path = "serialization_trace.rs"]
mod trace;
pub(super) use trace::{IdentifierSpans, TraceCursor};
pub use trace::{MathmlTrace, MathmlTraces};

pub struct TypedMathml {
    source: String,
    tree: PortableTree,
    eligible: bool,
}
impl TypedMathml {
    pub fn new(tree: &PortableTree) -> Result<Self, String> {
        let (source, eligible) = tree_to_mathml_proven(tree)?;
        Ok(Self {
            source,
            tree: tree.clone(),
            eligible,
        })
    }
    pub fn source(&self) -> &str {
        &self.source
    }
    pub fn tree(&self) -> &PortableTree {
        &self.tree
    }
    pub(super) fn eligible(&self) -> bool {
        self.eligible
    }
}
#[derive(Default)]
pub(super) struct Plan {
    pub(super) replacement: Option<NodeKind>,
    pub(super) children: HashMap<usize, Plan>,
}
fn identifier(kind: &NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::Identifier(_)
            | NodeKind::AnnotatedIdentifier { .. }
            | NodeKind::NativeIdentifier { .. }
    )
}
fn xml_text(text: &str) -> bool {
    !text.contains('\r')
        && text
            .chars()
            .all(|c| matches!(c, '\t' | '\n') || c >= ' ' && c != '\u{fffe}' && c != '\u{ffff}')
}
pub(super) fn eligible_root(tree: &PortableTree) -> bool {
    matches!(
        tree.kind,
        NodeKind::Binary(BinaryOp::Define | BinaryOp::GlobalDefine | BinaryOp::Evaluate)
    ) && eligible(tree, 0)
}
fn eligible(node: &PortableTree, depth: usize) -> bool {
    if depth >= 64 {
        return false;
    }
    let allowed = match &node.kind {
        NodeKind::Identifier(name) => xml_text(name),
        NodeKind::StringLiteral(value) => xml_text(value),
        NodeKind::AnnotatedIdentifier { name, labels } => {
            xml_text(name) && xml_text(labels) && !labels.contains(['\t', '\n'])
        }
        NodeKind::NativeIdentifier {
            name,
            labels,
            subscript,
            ..
        } => {
            xml_text(name)
                && xml_text(labels)
                && !labels.contains(['\t', '\n'])
                && subscript.as_deref().is_none_or(xml_text)
        }
        NodeKind::Binary(BinaryOp::Define | BinaryOp::GlobalDefine) => {
            depth == 0 && identifier(&node.children[0].kind)
        }
        NodeKind::Call { .. } => identifier(&node.children[0].kind),
        NodeKind::Placeholder
        | NodeKind::Number(_)
        | NodeKind::BasedNumber { .. }
        | NodeKind::Imaginary(_)
        | NodeKind::ImaginarySymbol { .. }
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
        | NodeKind::Matrix { .. }
        | NodeKind::Index { .. }
        | NodeKind::Range { .. }
        | NodeKind::Binary(_)
        | NodeKind::Fraction
        | NodeKind::Power
        | NodeKind::Group => true,
        _ => false,
    };
    allowed && node.children.iter().all(|child| eligible(child, depth + 1))
}
type BorrowedIdentifierParts<'a> = (&'a str, Option<&'a str>, Option<bool>, Option<&'a str>);
fn parts(kind: &NodeKind) -> Option<BorrowedIdentifierParts<'_>> {
    match kind {
        NodeKind::Identifier(name) => Some((name, None, None, None)),
        NodeKind::AnnotatedIdentifier { name, labels } => Some((name, Some(labels), None, None)),
        NodeKind::NativeIdentifier {
            name,
            labels,
            contextual,
            subscript,
        } => Some((name, Some(labels), *contextual, subscript.as_deref())),
        _ => None,
    }
}
fn canonical(kind: &NodeKind) -> NodeKind {
    match kind {
        NodeKind::NativeIdentifier {
            name,
            labels,
            contextual: None,
            subscript: None,
        } => NodeKind::AnnotatedIdentifier {
            name: name.clone(),
            labels: labels.clone(),
        },
        _ => kind.clone(),
    }
}
#[derive(Clone, Copy, Default)]
struct Context {
    forced: Option<&'static str>,
    declaration: bool,
    call_target: bool,
}
fn plan(
    node: &PortableTree,
    context: Context,
    definitions: &HashMap<String, String>,
    capture: &mut worksheet_cache::FragmentContext,
) -> Plan {
    let mut output = Plan::default();
    if let Some((name, labels, contextual, subscript)) = parts(&node.kind) {
        if let Some(label) = resolve_identifier_label(
            name,
            labels,
            contextual,
            subscript,
            LabelFacts {
                declaration: context.declaration,
                direct_declaration: context.declaration,
                structural: context.forced,
                call_target: context.call_target,
                ..Default::default()
            },
            definitions,
            Some(capture),
        ) {
            output.replacement = Some(NodeKind::NativeIdentifier {
                name: name.into(),
                labels: label,
                contextual: None,
                subscript: subscript.map(str::to_string),
            });
        }
        return output;
    }
    for (i, child) in node.children.iter().enumerate() {
        let mut next = Context {
            forced: context.forced,
            ..Default::default()
        };
        match node.kind {
            NodeKind::Binary(BinaryOp::Define | BinaryOp::GlobalDefine) if i == 0 => {
                next.declaration = true;
                next.forced = Some("VARIABLE");
            }
            NodeKind::Binary(BinaryOp::Evaluate) if i == 1 => next.forced = Some("UNIT"),
            NodeKind::Index { .. } | NodeKind::Row | NodeKind::Column if i >= 1 => {
                next.forced = None
            }
            NodeKind::Call { .. } if i == 0 => next.call_target = true,
            _ => (),
        }
        let child = plan(child, next, definitions, capture);
        if child.replacement.is_some() || !child.children.is_empty() {
            output.children.insert(i, child);
        }
    }
    output
}
fn projected(node: &PortableTree, plan: Option<&Plan>) -> PortableTree {
    PortableTree {
        kind: canonical(
            plan.and_then(|p| p.replacement.as_ref())
                .unwrap_or(&node.kind),
        ),
        children: node
            .children
            .iter()
            .enumerate()
            .map(|(i, child)| projected(child, plan.and_then(|p| p.children.get(&i))))
            .collect(),
    }
}
pub(super) fn label(
    proof: &TypedMathml,
    definitions: &mut HashMap<String, String>,
    decode: bool,
    trace: Option<&MathmlTrace>,
) -> Result<(String, worksheet_cache::FragmentContext), String> {
    let mut capture = worksheet_cache::FragmentContext::default();
    capture.cacheable = true;
    capture.decode_tree = decode;
    #[cfg(test)]
    let plan_timing = worksheet_cache::profile::scope(worksheet_cache::profile::Phase::TypedPlan);
    let plan = plan(&proof.tree, Context::default(), definitions, &mut capture);
    #[cfg(test)]
    drop(plan_timing);
    #[cfg(test)]
    let serialize_timing =
        worksheet_cache::profile::scope(worksheet_cache::profile::Phase::TypedSerialize);
    let labelled = if let Some(trace) = trace {
        label_traced(proof, &plan, trace)?
    } else {
        serialization_writer::serialize(&proof.tree, true, 0, Some(&plan))?
    };
    #[cfg(test)]
    drop(serialize_timing);
    #[cfg(test)]
    let definition_timing =
        worksheet_cache::profile::scope(worksheet_cache::profile::Phase::TypedDefinition);
    if matches!(
        proof.tree.kind,
        NodeKind::Binary(BinaryOp::Define | BinaryOp::GlobalDefine)
    ) {
        let (name, labels, contextual, subscript) =
            parts(&proof.tree.children[0].kind).expect("eligible direct definition");
        let identity = identifier_identity(name, subscript);
        let label = declaration_label_parts(name, labels, contextual, subscript, true);
        capture.defined = Some((identity.clone(), label.clone()));
        definitions.insert(identity, label);
    }
    #[cfg(test)]
    drop(definition_timing);
    if decode {
        #[cfg(test)]
        let project_timing =
            worksheet_cache::profile::scope(worksheet_cache::profile::Phase::TypedProject);
        let tree = projected(&proof.tree, Some(&plan));
        #[cfg(test)]
        drop(project_timing);
        #[cfg(test)]
        let _validate_timing =
            worksheet_cache::profile::scope(worksheet_cache::profile::Phase::TypedValidate);
        capture.decoded_tree = Some(
            tree.validate()
                .map_err(|e| format!("Invalid structural tree: {e:?}"))
                .map(|_| tree),
        );
    }
    Ok((labelled, capture))
}

#[cfg(test)]
#[path = "serialization_profile.rs"]
mod serialization_profile;

fn label_traced(proof: &TypedMathml, plan: &Plan, trace: &MathmlTrace) -> Result<String, String> {
    let mut output = String::with_capacity(proof.source.len());
    let mut start = 0;
    for span in &trace.identifiers.spans {
        let mut current = Some(plan);
        let mut node = &proof.tree;
        for index in &span.path {
            current = current.and_then(|p| p.children.get(index));
            node = &node.children[*index];
        }
        if let Some(replacement) = current.filter(|p| p.replacement.is_some()) {
            output.push_str(&proof.source[start..span.range.start]);
            serialization_writer::append_identifier(
                &mut output,
                node,
                replacement,
                span.path.len(),
            )?;
            start = span.range.end;
        }
    }
    output.push_str(&proof.source[start..]);
    Ok(output.into_boxed_str().into_string())
}
