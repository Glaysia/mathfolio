//! A bounded structural bridge, not a Mathcad file converter.
//!
//! Explicit identifier labels and contextual display metadata survive editing.
//! `worksheet_xml` applies proven structural label constraints, then a bounded,
//! independent lookup policy using visible definitions and the prototype catalog.
//! Cached contextual labels do not become explicit user choices. This does not
//! implement Prime's complete contextual/wildcard label resolution.
//! Unsupported attributes or structures return an error so callers retain raw XML.

use mathcraft_editor::{BinaryOp, NodeKind, PortableTree};
use roxmltree::Node;
use std::collections::{HashMap, HashSet};

const ML: &str = "http://schemas.mathsoft.com/math50";
const WS: &str = "http://schemas.mathsoft.com/worksheet50";
const XML: &str = "http://www.w3.org/XML/1998/namespace";
const MAX_XML: usize = 8 * 1024 * 1024;
const XAML: &str = "http://schemas.microsoft.com/winfx/2006/xaml/presentation";
const PTC: &str = "clr-namespace:Ptc.Wpf;assembly=Ptc.Core";

/// Decode only the shipped identifier representation whose metadata the editor
/// can retain. This reads XAML as inert XML, never as executable markup.
type IdentifierParts = (String, Option<String>, Option<bool>, Option<String>);
pub(crate) fn identifier_parts(node: Node<'_, '_>) -> Result<IdentifierParts, String> {
    attributes(
        node,
        &[
            ("labels", None),
            ("space", Some(XML)),
            ("label-is-contextual", None),
        ],
    )?;
    if node
        .attribute((XML, "space"))
        .is_some_and(|x| x != "preserve")
    {
        return Err("Unsupported identifier whitespace policy".into());
    }
    let contextual = node
        .attribute("label-is-contextual")
        .map(|x| match x {
            "true" | "1" => Ok(true),
            "false" | "0" => Ok(false),
            _ => Err("Invalid contextual flag".to_string()),
        })
        .transpose()?;
    let labels = node.attribute("labels").map(str::to_string);
    if node.children().all(|n| n.is_text()) {
        return Ok((node.text().unwrap_or("").into(), labels, contextual, None));
    }
    let content = elements(node)?;
    if content.len() != 1 || !content[0].has_tag_name((XAML, "Span")) {
        return Err("Unsupported structured identifier".into());
    }
    let span = content[0];
    attributes(span, &[])?;
    let mut name = String::new();
    let mut subscript = None;
    for child in span.children() {
        if child.is_text() && subscript.is_none() {
            name.push_str(child.text().unwrap_or(""));
        } else if child.has_tag_name((PTC, "Subscript")) && subscript.is_none() {
            attributes(child, &[])?;
            if child.children().any(|n| !n.is_text()) {
                return Err("Unsupported subscript markup".into());
            }
            subscript = Some(child.text().unwrap_or("").to_string());
        } else {
            return Err("Unsupported identifier span content".into());
        }
    }
    if subscript.is_none() {
        return Err("Unsupported span without literal subscript".into());
    }
    Ok((name, labels, contextual, subscript))
}

pub(crate) fn identifier_identity(name: &str, subscript: Option<&str>) -> String {
    match subscript {
        Some(sub) => format!("{name}\u{1f}{sub}"),
        None => name.into(),
    }
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn elements<'a, 'input>(node: Node<'a, 'input>) -> Result<Vec<Node<'a, 'input>>, String> {
    if node.children().any(|n| {
        n.is_comment() || n.is_pi() || (n.is_text() && !n.text().unwrap_or("").trim().is_empty())
    }) {
        return Err("Unsupported text/comment inside structural expression".into());
    }
    Ok(node.children().filter(Node::is_element).collect())
}

fn attributes(node: Node<'_, '_>, allowed: &[(&str, Option<&str>)]) -> Result<(), String> {
    for attr in node.attributes() {
        if !allowed.contains(&(attr.name(), attr.namespace())) {
            return Err(format!(
                "Unsupported {} attribute {}",
                node.tag_name().name(),
                attr.name()
            ));
        }
    }
    Ok(())
}

fn math(node: Node<'_, '_>, name: &str) -> bool {
    node.has_tag_name((ML, name))
}

fn tree(kind: NodeKind, children: Vec<PortableTree>) -> PortableTree {
    PortableTree { kind, children }
}

fn parse(node: Node<'_, '_>, depth: usize) -> Result<PortableTree, String> {
    if depth > 128 || node.tag_name().namespace() != Some(ML) {
        return Err("Unsupported expression namespace/depth".into());
    }
    let result = match node.tag_name().name() {
        "id" => {
            let (name, labels, contextual, subscript) = identifier_parts(node)?;
            let kind = match (labels, contextual, subscript) {
                (Some(labels), None, None) => NodeKind::AnnotatedIdentifier { name, labels },
                (None, None, None) => NodeKind::Identifier(name),
                (Some(labels), contextual, subscript) => NodeKind::NativeIdentifier {
                    name,
                    labels,
                    contextual,
                    subscript,
                },
                _ => return Err("Unlabelled native identifier requires raw preservation".into()),
            };
            tree(kind, vec![])
        }
        "real" | "imag" => {
            attributes(
                node,
                if math(node, "imag") {
                    &[("symbol", None)]
                } else {
                    &[]
                },
            )?;
            if node.children().any(|n| !n.is_text()) {
                return Err("Structured numeric literal requires raw preservation".into());
            }
            let text = node.text().unwrap_or("");
            let value: f64 = text.parse().map_err(|_| "Invalid real literal")?;
            if !value.is_finite() {
                return Err("Nonfinite literal is outside this editor subset".into());
            }
            tree(
                if math(node, "imag") {
                    match node.attribute("symbol") {
                        Some(symbol @ ("i" | "j")) => NodeKind::ImaginarySymbol {
                            value: text.into(),
                            symbol: symbol.into(),
                        },
                        Some(_) => return Err("Unknown imaginary symbol".into()),
                        None => NodeKind::Imaginary(text.into()),
                    }
                } else {
                    NodeKind::Number(text.into())
                },
                vec![],
            )
        }
        "placeholder" => {
            attributes(node, &[])?;
            if !elements(node)?.is_empty() {
                return Err("Placeholder contains unknown data".into());
            }
            tree(NodeKind::Placeholder, vec![])
        }
        "parens" => {
            attributes(node, &[])?;
            let nodes = elements(node)?;
            if nodes.len() != 1 {
                return Err("Unsupported grouping arity".into());
            }
            tree(NodeKind::Group, vec![parse(nodes[0], depth + 1)?])
        }
        "define" => {
            if depth != 0 {
                return Err("Nested definition requires raw preservation".into());
            }
            attributes(node, &[])?;
            let nodes = elements(node)?;
            if nodes.len() != 2 || !(math(nodes[0], "id") || math(nodes[0], "function")) {
                return Err("Unsupported definition target".into());
            }
            tree(
                NodeKind::Binary(BinaryOp::Define),
                nodes
                    .iter()
                    .map(|n| parse(*n, depth + 1))
                    .collect::<Result<_, _>>()?,
            )
        }
        "eval" => {
            if depth != 0
                && !(depth == 1 && node.parent_element().is_some_and(|p| math(p, "define")))
            {
                return Err("Nested evaluation requires raw preservation".into());
            }
            attributes(node, &[])?;
            let nodes = elements(node)?;
            if nodes.len() != 2 || !math(nodes[1], "unitOverride") {
                return Err("Unsupported evaluation structure".into());
            }
            attributes(nodes[1], &[])?;
            let units = elements(nodes[1])?;
            if units.len() != 1 {
                return Err("Unsupported unit override structure".into());
            }
            tree(
                NodeKind::Binary(BinaryOp::Evaluate),
                vec![parse(nodes[0], depth + 1)?, parse(units[0], depth + 1)?],
            )
        }
        "apply" => {
            attributes(node, &[])?;
            let nodes = elements(node)?;
            if nodes.is_empty() || nodes[0].tag_name().namespace() != Some(ML) {
                return Err("Unsupported operator/arity".into());
            }
            let op = nodes[0];
            if math(op, "id") {
                let arguments = if nodes.len() == 2 && math(nodes[1], "sequence") {
                    attributes(nodes[1], &[])?;
                    elements(nodes[1])?
                } else {
                    nodes[1..].to_vec()
                };
                let mut children = vec![parse(op, depth + 1)?];
                for arg in &arguments {
                    children.push(parse(*arg, depth + 1)?);
                }
                return Ok(tree(
                    NodeKind::Call {
                        argument_count: arguments.len(),
                    },
                    children,
                ));
            }
            if !elements(op)?.is_empty() {
                return Err("Operator carries unknown content".into());
            }
            let kind = match op.tag_name().name() {
                "plus" => NodeKind::Binary(BinaryOp::Add),
                "minus" => NodeKind::Binary(BinaryOp::Subtract),
                "mult" => NodeKind::Binary(BinaryOp::Multiply),
                "scale" => NodeKind::Binary(BinaryOp::Scale),
                "pow" => NodeKind::Power,
                "neg" => NodeKind::UnaryNeg,
                "absval" => NodeKind::Absolute,
                "nthRoot" => NodeKind::Root,
                "indexer" => {
                    let indices = if nodes.len() == 3 && math(nodes[2], "sequence") {
                        attributes(nodes[2], &[])?;
                        elements(nodes[2])?
                    } else {
                        nodes.get(2..).unwrap_or(&[]).to_vec()
                    };
                    if !(1..=2).contains(&indices.len()) || nodes.len() < 3 {
                        return Err("Unsupported index arity".into());
                    }
                    let mut children = vec![parse(nodes[1], depth + 1)?];
                    for i in &indices {
                        children.push(parse(*i, depth + 1)?);
                    }
                    attributes(op, &[])?;
                    return Ok(tree(
                        NodeKind::Index {
                            indices: indices.len(),
                        },
                        children,
                    ));
                }
                "div" => {
                    attributes(op, &[("inline", None)])?;
                    match op.attribute("inline") {
                        None | Some("false" | "0") => NodeKind::Fraction,
                        Some("true" | "1") => NodeKind::Binary(BinaryOp::InlineDivide),
                        _ => return Err("Unknown division presentation".into()),
                    }
                }
                _ => return Err("Unsupported operator requires raw preservation".into()),
            };
            if !math(op, "div") {
                attributes(op, &[])?;
            }
            let arity = if matches!(kind, NodeKind::UnaryNeg | NodeKind::Absolute) {
                1
            } else {
                2
            };
            if nodes.len() != arity + 1 {
                return Err("Unsupported operator arity".into());
            }
            tree(
                kind,
                nodes[1..]
                    .iter()
                    .map(|n| parse(*n, depth + 1))
                    .collect::<Result<_, _>>()?,
            )
        }
        "function" => {
            attributes(node, &[])?;
            let nodes = elements(node)?;
            if nodes.len() != 2 || !math(nodes[0], "id") || !math(nodes[1], "boundVars") {
                return Err("Unsupported function declaration".into());
            }
            attributes(nodes[1], &[])?;
            let params = elements(nodes[1])?;
            if params.iter().any(|p| !math(*p, "id")) {
                return Err("Unsupported function parameter".into());
            }
            let mut children = vec![parse(nodes[0], depth + 1)?];
            for p in &params {
                children.push(parse(*p, depth + 1)?);
            }
            tree(
                NodeKind::Function {
                    parameter_count: params.len(),
                },
                children,
            )
        }
        "matrix" => {
            attributes(node, &[("rows", None), ("cols", None)])?;
            let rows: usize = node
                .attribute("rows")
                .unwrap_or("")
                .parse()
                .map_err(|_| "Invalid matrix rows")?;
            let cols: usize = node
                .attribute("cols")
                .unwrap_or("")
                .parse()
                .map_err(|_| "Invalid matrix cols")?;
            let nodes = elements(node)?;
            if rows == 0
                || cols == 0
                || rows.checked_mul(cols) != Some(nodes.len())
                || nodes.len() > 16384
            {
                return Err("Invalid matrix size".into());
            }
            let mut row_major = Vec::with_capacity(nodes.len());
            for row in 0..rows {
                for col in 0..cols {
                    row_major.push(parse(nodes[col * rows + row], depth + 1)?);
                }
            }
            tree(NodeKind::Matrix { rows, cols }, row_major)
        }
        "range" => {
            attributes(node, &[])?;
            let parts = elements(node)?;
            if parts.len() != 2 {
                return Err("Unsupported range arity".into());
            }
            let (has_step, mut nodes) = if math(parts[0], "sequence") {
                attributes(parts[0], &[])?;
                let pair = elements(parts[0])?;
                if pair.len() != 2 {
                    return Err("Unsupported range start/second".into());
                }
                (true, pair)
            } else {
                (false, vec![parts[0]])
            };
            nodes.push(parts[1]);
            tree(
                NodeKind::Range { has_step },
                nodes
                    .iter()
                    .map(|n| parse(*n, depth + 1))
                    .collect::<Result<_, _>>()?,
            )
        }
        name => return Err(format!("Unsupported MathML {name}; retain raw XML")),
    };
    Ok(result)
}

/// Read a supported MathML expression, definition, or evaluation. A worksheet
/// `math` wrapper or a namespace-only outer wrapper is accepted as context.
pub fn tree_from_mathml(xml: &str) -> Result<PortableTree, String> {
    if xml.len() > MAX_XML {
        return Err("MathML exceeds the editor import budget".into());
    }
    let wrapped =
        format!("<context xmlns:ml=\"{ML}\" xmlns:ws=\"{WS}\" xmlns=\"{WS}\">{xml}</context>");
    let document = roxmltree::Document::parse(&wrapped).map_err(|e| e.to_string())?;
    let mut roots = elements(document.root_element())?;
    if roots.len() != 1 {
        return Err("Expected one MathML fragment".into());
    }
    let mut root = roots.remove(0);
    if root.tag_name().namespace() != Some(ML) {
        // Wrapper attributes other than known math's resultRef cannot be
        // silently discarded; a namespace-only context has no attributes.
        if root.has_tag_name((WS, "math")) {
            attributes(root, &[("resultRef", None)])?;
        } else {
            attributes(root, &[])?;
        }
        let mut children = elements(root)?;
        if root.has_tag_name((WS, "math")) {
            // The owning document retains the original wrapper and these inert
            // presentation siblings. This bridge only edits its expression.
            children.retain(|n| {
                !n.has_tag_name((WS, "resultFormat")) && !n.has_tag_name((WS, "formattingOverride"))
            });
        }
        if children.len() != 1 {
            return Err("Expected one expression in namespace wrapper".into());
        }
        root = children[0];
    }
    let tree = parse(root, 0)?;
    tree.validate()
        .map_err(|e| format!("Invalid structural tree: {e:?}"))?;
    Ok(tree)
}

fn serialize(node: &PortableTree, root: bool, depth: usize) -> Result<String, String> {
    if depth > 128 {
        return Err("Math tree exceeds serialization depth".into());
    }
    let ns = if root {
        format!(" xmlns:ml=\"{ML}\"")
    } else {
        String::new()
    };
    let child = |i: usize| serialize(&node.children[i], false, depth + 1);
    let text = match &node.kind {
        NodeKind::Placeholder => format!("<ml:placeholder{ns}/>"),
        NodeKind::Identifier(name) => {
            format!("<ml:id{ns} xml:space=\"preserve\">{}</ml:id>", escape(name))
        }
        NodeKind::AnnotatedIdentifier { name, labels } => format!(
            "<ml:id{ns} labels=\"{}\" xml:space=\"preserve\">{}</ml:id>",
            escape(labels),
            escape(name)
        ),
        NodeKind::NativeIdentifier {
            name,
            labels,
            contextual,
            subscript,
        } => {
            let contextual = contextual
                .map(|x| format!(" label-is-contextual=\"{x}\""))
                .unwrap_or_default();
            let body = match subscript {
                Some(sub) => format!(
                    "<p:Span xmlns:p=\"{XAML}\">{}<ptc:Subscript xmlns:ptc=\"{PTC}\">{}</ptc:Subscript></p:Span>",
                    escape(name),
                    escape(sub)
                ),
                None => escape(name),
            };
            format!(
                "<ml:id{ns} labels=\"{}\"{contextual} xml:space=\"preserve\">{body}</ml:id>",
                escape(labels)
            )
        }
        NodeKind::Number(number) => {
            let value: f64 = number.parse().map_err(|_| "Invalid number literal")?;
            if !value.is_finite() {
                return Err("Nonfinite number literal".into());
            }
            format!("<ml:real{ns}>{}</ml:real>", escape(number))
        }
        NodeKind::Imaginary(number) | NodeKind::ImaginarySymbol { value: number, .. } => {
            if !number.parse::<f64>().is_ok_and(f64::is_finite) {
                return Err("Invalid imaginary literal".into());
            }
            let symbol = if let NodeKind::ImaginarySymbol { symbol, .. } = &node.kind {
                if symbol != "i" && symbol != "j" {
                    return Err("Unsupported imaginary symbol".into());
                }
                format!(" symbol=\"{}\"", escape(symbol))
            } else {
                String::new()
            };
            format!("<ml:imag{ns}{symbol}>{}</ml:imag>", escape(number))
        }
        NodeKind::Group => format!("<ml:parens{ns}>{}</ml:parens>", child(0)?),
        NodeKind::UnaryNeg => format!("<ml:apply{ns}><ml:neg/>{}</ml:apply>", child(0)?),
        NodeKind::Absolute => format!("<ml:apply{ns}><ml:absval/>{}</ml:apply>", child(0)?),
        NodeKind::Root => format!(
            "<ml:apply{ns}><ml:nthRoot/>{}{}</ml:apply>",
            child(0)?,
            child(1)?
        ),
        NodeKind::Call { argument_count } => {
            let args = (1..=*argument_count)
                .map(child)
                .collect::<Result<Vec<_>, _>>()?
                .join("");
            let args = if *argument_count == 1 {
                args
            } else {
                format!("<ml:sequence>{args}</ml:sequence>")
            };
            format!("<ml:apply{ns}>{}{args}</ml:apply>", child(0)?)
        }
        NodeKind::Function { parameter_count } => {
            let params = (1..=*parameter_count)
                .map(child)
                .collect::<Result<Vec<_>, _>>()?
                .join("");
            format!(
                "<ml:function{ns}>{}<ml:boundVars>{params}</ml:boundVars></ml:function>",
                child(0)?
            )
        }
        NodeKind::Matrix { rows, cols } => {
            let mut content = String::new();
            for col in 0..*cols {
                for row in 0..*rows {
                    content.push_str(&child(row * cols + col)?);
                }
            }
            format!("<ml:matrix{ns} rows=\"{rows}\" cols=\"{cols}\">{content}</ml:matrix>")
        }
        NodeKind::Range { has_step } => {
            if *has_step {
                format!(
                    "<ml:range{ns}><ml:sequence>{}{}</ml:sequence>{}</ml:range>",
                    child(0)?,
                    child(1)?,
                    child(2)?
                )
            } else {
                format!("<ml:range{ns}>{}{}</ml:range>", child(0)?, child(1)?)
            }
        }
        NodeKind::Index { indices } => {
            let index = (1..=*indices)
                .map(child)
                .collect::<Result<Vec<_>, _>>()?
                .join("");
            let index = if *indices == 1 {
                index
            } else {
                format!("<ml:sequence>{index}</ml:sequence>")
            };
            format!("<ml:apply{ns}><ml:indexer/>{}{index}</ml:apply>", child(0)?)
        }
        NodeKind::Binary(BinaryOp::Define) => {
            if depth != 0
                || !matches!(
                    node.children[0].kind,
                    NodeKind::Identifier(_)
                        | NodeKind::AnnotatedIdentifier { .. }
                        | NodeKind::NativeIdentifier { .. }
                        | NodeKind::Function { .. }
                )
            {
                return Err("Only top-level identifier definitions are supported".into());
            }
            format!("<ml:define{ns}>{}{}</ml:define>", child(0)?, child(1)?)
        }
        NodeKind::Binary(BinaryOp::Evaluate) => {
            if depth > 1 {
                return Err("Nested evaluation needs raw preservation".into());
            }
            format!(
                "<ml:eval{ns}>{}<ml:unitOverride>{}</ml:unitOverride></ml:eval>",
                child(0)?,
                child(1)?
            )
        }
        kind => {
            let op = match kind {
                NodeKind::Binary(BinaryOp::Add) => "plus",
                NodeKind::Binary(BinaryOp::Subtract) => "minus",
                NodeKind::Binary(BinaryOp::Multiply) => "mult",
                NodeKind::Binary(BinaryOp::Scale) => "scale",
                NodeKind::Binary(BinaryOp::InlineDivide) => "div inline=\"true\"",
                NodeKind::Fraction => "div",
                NodeKind::Power => "pow",
                _ => return Err("Unsupported structural operator".into()),
            };
            format!(
                "<ml:apply{ns}><ml:{op}/>{}{}</ml:apply>",
                child(0)?,
                child(1)?
            )
        }
    };
    Ok(text)
}

/// Bare expressions stay bare. A visible '=' (Evaluate node) is necessary to
/// produce an eval statement; this function never inserts one automatically.
pub fn tree_to_mathml(tree: &PortableTree) -> Result<String, String> {
    tree.validate()
        .map_err(|e| format!("Invalid structural tree: {e:?}"))?;
    fn context(node: &PortableTree, allow_eval: bool, root: bool) -> Result<(), String> {
        if matches!(node.kind, NodeKind::Binary(BinaryOp::Evaluate)) && !allow_eval {
            return Err("Evaluation is only supported at the region root or definition RHS".into());
        }
        if matches!(node.kind, NodeKind::Binary(BinaryOp::Define)) && !root {
            return Err("Nested definitions require raw preservation".into());
        }
        for (i, child) in node.children.iter().enumerate() {
            context(
                child,
                matches!(node.kind, NodeKind::Binary(BinaryOp::Define)) && i == 1,
                false,
            )?;
        }
        Ok(())
    }
    context(tree, true, true)?;
    serialize(tree, true, 0)
}

fn inferred_label(name: &str, definitions: &HashMap<String, String>) -> String {
    if let Some(label) = definitions.get(name) {
        return label.clone();
    }
    if system_identifier(name) {
        return "SYSTEM".into();
    }
    if matches!(name, "π" | "e" | "γ" | "%") {
        return "CONSTANT".into();
    }
    if mathcraft_core::unit(name).is_some() {
        return "UNIT".into();
    }
    if mathcraft_core::units::constant_lookup(name).is_some() {
        return "CONSTANT".into();
    }
    if crate::scalars::is_builtin(name)
        || crate::arrays::is_builtin(name)
        || crate::statistics::is_builtin(name)
        || crate::numerics::is_builtin(name)
    {
        return "FUNCTION".into();
    }
    if name == "root" {
        return "KEYWORD".into();
    }
    "VARIABLE".into()
}

fn system_identifier(name: &str) -> bool {
    matches!(
        name,
        "ORIGIN" | "TOL" | "CTOL" | "PRNCOLWIDTH" | "PRNPRECISION" | "CWD" | "Seed"
    ) || name
        .strip_prefix("in")
        .is_some_and(|n| n.len() == 1 && n.as_bytes()[0].is_ascii_digit())
}

fn declaration_label(id: Node<'_, '_>) -> Result<String, String> {
    let (name, labels, contextual, subscript) = identifier_parts(id)?;
    Ok(
        if let Some(label) = labels.filter(|_| contextual != Some(true)) {
            label
        } else if subscript.is_none()
            && system_identifier(&name)
            && id
                .parent_element()
                .is_some_and(|parent| math(parent, "define") || math(parent, "globalDefine"))
        {
            "SYSTEM".into()
        } else {
            "VARIABLE".into()
        },
    )
}

// Descriptor-table constraints apply to an entire child subtree. Index operands
// interrupt this inheritance: Prime's ForceUnlabeled... keeps them unresolved.
// Here they continue through bounded lookup, rather than inheriting UNIT.
fn structural_label(id: Node<'_, '_>) -> Option<&'static str> {
    let mut child = id;
    while let Some(parent) = child.parent_element() {
        if math(parent, "apply") {
            let children: Vec<_> = parent.children().filter(Node::is_element).collect();
            if children
                .first()
                .is_some_and(|op| math(*op, "indexer") || math(*op, "row") || math(*op, "col"))
                && children
                    .iter()
                    .position(|n| *n == child)
                    .is_some_and(|i| i >= 2)
            {
                return None;
            }
        }
        if math(parent, "unitOverride") {
            return Some("UNIT");
        }
        if math(parent, "boundVars") {
            return Some("VARIABLE");
        }
        if (math(parent, "define") || math(parent, "globalDefine"))
            && parent.children().find(Node::is_element) == Some(child)
        {
            return Some("VARIABLE");
        }
        child = parent;
    }
    None
}

fn label_fragment(xml: &str, definitions: &mut HashMap<String, String>) -> Result<String, String> {
    let wrapped = format!("<context xmlns:ml=\"{ML}\" xmlns=\"{WS}\">{xml}</context>");
    let document = roxmltree::Document::parse(&wrapped).map_err(|e| e.to_string())?;
    let roots = elements(document.root_element())?;
    if roots.len() != 1
        || !(math(roots[0], "define") || math(roots[0], "eval") || math(roots[0], "globalDefine"))
    {
        return Err("Calculation accepts only explicit define/eval/globalDefine regions".into());
    }
    let root = roots[0];
    let definition = if math(root, "define") || math(root, "globalDefine") {
        let children: Vec<_> = root.children().filter(Node::is_element).collect();
        children.first().copied().and_then(|n| {
            if math(n, "id") {
                Some(n)
            } else if math(n, "function") {
                n.children().find(|x| math(*x, "id"))
            } else {
                None
            }
        })
    } else {
        None
    };
    let mut locals = definitions.clone();
    for bound in root.descendants().filter(|n| math(*n, "boundVars")) {
        for parameter in bound.children().filter(|n| math(*n, "id")) {
            let (name, labels, contextual, sub) = identifier_parts(parameter)?;
            locals.insert(
                identifier_identity(&name, sub.as_deref()),
                if contextual == Some(true) {
                    None
                } else {
                    labels
                }
                .unwrap_or_else(|| "VARIABLE".into()),
            );
        }
    }
    let mut patches = Vec::new();
    for id in root.descendants().filter(|n| math(*n, "id")) {
        let (name, labels, contextual, subscript) = identifier_parts(id)?;
        if labels.is_some() && contextual != Some(true) {
            continue;
        }
        let identity = identifier_identity(&name, subscript.as_deref());
        let label = if Some(id) == definition {
            declaration_label(id)?
        } else if let Some(label) = structural_label(id) {
            label.into()
        } else if id.parent().is_some_and(|parent| {
            math(parent, "apply") && parent.children().find(Node::is_element) == Some(id)
        }) && !locals.contains_key(&identity)
            && (crate::scalars::is_builtin(&identity)
                || crate::arrays::is_builtin(&identity)
                || crate::statistics::is_builtin(&identity)
                || crate::numerics::is_builtin(&identity))
        {
            "FUNCTION".into()
        } else {
            inferred_label(&identity, &locals)
        };
        let node = tree(
            NodeKind::NativeIdentifier {
                name,
                labels: label,
                contextual: None,
                subscript,
            },
            vec![],
        );
        patches.push((id.range(), serialize(&node, false, 0)?));
    }
    if let Some(id) = definition {
        let (name, _, _, subscript) = identifier_parts(id)?;
        definitions.insert(
            identifier_identity(&name, subscript.as_deref()),
            declaration_label(id)?,
        );
    }
    let mut output = wrapped;
    for (range, text) in patches.into_iter().rev() {
        output.replace_range(range, &text);
    }
    let start = output.find('>').ok_or("Invalid context")? + 1;
    let end = output.rfind("</context>").ok_or("Invalid context")?;
    Ok(output[start..end].into())
}

/// Build calculation-only worksheet XML from explicitly positioned statements.
/// Regions retain their supplied IDs and order. The limited automatic label
/// policy runs in geometric order; it never changes explicit imported labels.
/// Contextual labels are cached display feedback, so only this calculation copy
/// resolves them again. The stored expression and its metadata stay unchanged.
pub fn worksheet_xml(regions: &[(u64, f64, f64, String)]) -> Result<String, String> {
    if regions.len() > 10_000 {
        return Err("Region budget exceeded".into());
    }
    let mut seen = HashSet::new();
    let mut order: Vec<_> = (0..regions.len()).collect();
    for (id, left, top, xml) in regions {
        if !seen.insert(*id) {
            return Err("Duplicate region ID".into());
        }
        if !left.is_finite() || !top.is_finite() || xml.len() > MAX_XML {
            return Err("Invalid region coordinate/XML budget".into());
        }
    }
    order.sort_by(|a, b| {
        regions[*a]
            .2
            .total_cmp(&regions[*b].2)
            .then(regions[*a].1.total_cmp(&regions[*b].1))
    });
    let mut definitions = HashMap::new();
    // Globals participate before all normal regions. This only chooses a
    // missing label; the compatibility evaluator owns validity/conflict rules.
    for (_, _, _, xml) in regions {
        let context = format!("<context xmlns:ml=\"{ML}\" xmlns=\"{WS}\">{xml}</context>");
        let document = roxmltree::Document::parse(&context).map_err(|e| e.to_string())?;
        for global in document
            .root_element()
            .children()
            .filter(|n| math(*n, "globalDefine"))
        {
            if let Some(target) = global.children().find(Node::is_element) {
                let id = if math(target, "id") {
                    Some(target)
                } else if math(target, "function") {
                    target.children().find(|n| math(*n, "id"))
                } else {
                    None
                };
                if let Some(id) = id {
                    let (name, _, _, subscript) = identifier_parts(id)?;
                    definitions.insert(
                        identifier_identity(&name, subscript.as_deref()),
                        declaration_label(id)?,
                    );
                }
            }
        }
    }
    let mut labelled = vec![String::new(); regions.len()];
    for index in order {
        labelled[index] = label_fragment(&regions[index].3, &mut definitions)?;
    }
    let mut output = format!("<worksheet xmlns=\"{WS}\" xmlns:ml=\"{ML}\"><regions>");
    for ((id, left, top, _), math) in regions.iter().zip(labelled) {
        output.push_str(&format!("<region region-id=\"{id}\" left=\"{left}\" top=\"{top}\"><math resultRef=\"{id}\">{math}</math></region>"));
        if output.len() > MAX_XML {
            return Err("Worksheet exceeds XML budget".into());
        }
    }
    output.push_str("</regions></worksheet>");
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(name: &str) -> PortableTree {
        tree(NodeKind::Identifier(name.into()), vec![])
    }
    fn number(n: &str) -> PortableTree {
        tree(NodeKind::Number(n.into()), vec![])
    }
    fn binary(op: BinaryOp, a: PortableTree, b: PortableTree) -> PortableTree {
        tree(NodeKind::Binary(op), vec![a, b])
    }

    #[test]
    fn explicit_label_and_unit_override_roundtrip() {
        let source = format!(
            "<ml:eval xmlns:ml=\"{ML}\"><ml:id labels=\"VARIABLE\" xml:space=\"preserve\">m</ml:id><ml:unitOverride><ml:id labels=\"UNIT\">cm</ml:id></ml:unitOverride></ml:eval>"
        );
        let value = tree_from_mathml(&source).unwrap();
        let output = tree_to_mathml(&value).unwrap();
        assert_eq!(tree_from_mathml(&output).unwrap(), value);
        assert!(output.contains("labels=\"VARIABLE\""));
        assert!(output.contains("labels=\"UNIT\""));
        assert!(output.contains("unitOverride"));
    }

    #[test]
    fn presentation_group_fraction_inline_and_power_roundtrip() {
        let value = binary(
            BinaryOp::Evaluate,
            tree(
                NodeKind::Group,
                vec![tree(
                    NodeKind::Power,
                    vec![
                        tree(NodeKind::Fraction, vec![number("1"), number("2")]),
                        number("3"),
                    ],
                )],
            ),
            tree(NodeKind::Placeholder, vec![]),
        );
        assert_eq!(
            tree_from_mathml(&tree_to_mathml(&value).unwrap()).unwrap(),
            value
        );
        let inline = binary(BinaryOp::InlineDivide, number("1"), number("2"));
        assert_eq!(
            tree_from_mathml(&tree_to_mathml(&inline).unwrap()).unwrap(),
            inline
        );
        assert!(!tree_to_mathml(&inline).unwrap().contains("<ml:eval"));
    }

    #[test]
    fn unknown_fragments_are_not_flattened() {
        for source in [
            "<ml:id labels=\"VARIABLE\">x<ml:subscript>0</ml:subscript></ml:id>",
            "<ml:id labels=\"VARIABLE\" arbitrary=\"yes\">x</ml:id>",
            "<ml:imag symbol=\"arbitrary\">1</ml:imag>",
            "<ml:real arbitrary=\"yes\">1</ml:real>",
        ] {
            assert!(tree_from_mathml(source).is_err(), "{source}");
        }
    }

    #[test]
    fn namespace_context_and_escaping() {
        let source = format!(
            "<context xmlns:ml=\"{ML}\"><ml:id labels=\"VARIABLE\">a&amp;b</ml:id></context>"
        );
        let parsed = tree_from_mathml(&source).unwrap();
        assert!(tree_to_mathml(&parsed).unwrap().contains("a&amp;b"));
        assert!(tree_from_mathml("<ml:real>2</ml:real>").is_ok());
    }

    #[test]
    fn labels_follow_prior_definition_in_geometry_not_input_order() {
        let define = tree_to_mathml(&binary(BinaryOp::Define, id("m"), number("3"))).unwrap();
        let evaluate = tree_to_mathml(&binary(
            BinaryOp::Evaluate,
            id("m"),
            tree(NodeKind::Placeholder, vec![]),
        ))
        .unwrap();
        let xml = worksheet_xml(&[(2, 0., 100., evaluate), (1, 0., 0., define)]).unwrap();
        let document = roxmltree::Document::parse(&xml).unwrap();
        let identifiers: Vec<_> = document.descendants().filter(|n| math(*n, "id")).collect();
        assert!(
            identifiers
                .iter()
                .all(|id| id.attribute("labels") == Some("VARIABLE"))
        );
        assert!(worksheet_xml(&[(1, 0., 0., tree_to_mathml(&number("1")).unwrap())]).is_err());
        assert!(worksheet_xml(&[(1, f64::NAN, 0., String::new())]).is_err());
    }

    #[test]
    fn explicit_unit_label_wins_and_global_names_exist_above_position() {
        let evaluate = tree_to_mathml(&binary(
            BinaryOp::Evaluate,
            id("m"),
            tree(NodeKind::Placeholder, vec![]),
        ))
        .unwrap();
        let global = format!(
            "<ml:globalDefine xmlns:ml=\"{ML}\"><ml:id labels=\"VARIABLE\">m</ml:id><ml:real>7</ml:real></ml:globalDefine>"
        );
        let explicit = tree_to_mathml(&binary(
            BinaryOp::Evaluate,
            tree(
                NodeKind::AnnotatedIdentifier {
                    name: "m".into(),
                    labels: "UNIT".into(),
                },
                vec![],
            ),
            tree(NodeKind::Placeholder, vec![]),
        ))
        .unwrap();
        let xml = worksheet_xml(&[
            (1, 0., 0., evaluate),
            (2, 0., 100., global),
            (3, 0., 200., explicit),
        ])
        .unwrap();
        let document = roxmltree::Document::parse(&xml).unwrap();
        let identifiers: Vec<_> = document.descendants().filter(|n| math(*n, "id")).collect();
        assert_eq!(identifiers[0].attribute("labels"), Some("VARIABLE"));
        assert_eq!(identifiers[2].attribute("labels"), Some("UNIT"));
    }

    #[test]
    fn positioned_definition_and_override_reach_structural_evaluator() {
        let body = binary(
            BinaryOp::Add,
            binary(BinaryOp::Multiply, number("3"), id("m")),
            binary(BinaryOp::Multiply, number("20"), id("cm")),
        );
        let define = tree_to_mathml(&binary(BinaryOp::Define, id("distance"), body)).unwrap();
        let evaluate =
            tree_to_mathml(&binary(BinaryOp::Evaluate, id("distance"), id("cm"))).unwrap();
        let xml = worksheet_xml(&[(20, 10., 100., evaluate), (10, 10., 0., define)]).unwrap();
        let result = crate::evaluate_worksheet(&xml).unwrap();
        assert!(result.iter().all(|r| r.error.is_none()), "{result:?}");
        assert_eq!(result[1].region_id, "20");
        let Some(crate::Value::Scalar { quantity }) = &result[1].display_value else {
            panic!("Expected a structural scalar evaluation");
        };
        assert!((quantity.value.re - 320.).abs() < 1e-10);
        let Some(crate::Value::Scalar { quantity }) = &result[1].value else {
            panic!("Expected a physical scalar value");
        };
        assert!((quantity.value.re - 3.2).abs() < 1e-10);
    }
}
