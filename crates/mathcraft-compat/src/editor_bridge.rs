//! A bounded structural bridge, not a Mathcad file converter.
//!
//! Explicit identifier labels survive editing. Newly typed `Identifier` nodes
//! remain unlabelled in stored fragments; `worksheet_xml` assigns a limited,
//! independent label policy using earlier definitions and the prototype catalog.
//! This does not implement Prime's contextual/wildcard label inference.
//! Unsupported attributes or structures return an error so callers retain raw XML.

use mathcraft_editor::{BinaryOp, NodeKind, PortableTree};
use roxmltree::Node;
use std::collections::{HashMap, HashSet};

const ML: &str = "http://schemas.mathsoft.com/math50";
const WS: &str = "http://schemas.mathsoft.com/worksheet50";
const XML: &str = "http://www.w3.org/XML/1998/namespace";
const MAX_XML: usize = 8 * 1024 * 1024;

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
            attributes(node, &[("labels", None), ("space", Some(XML))])?;
            if node
                .attribute((XML, "space"))
                .is_some_and(|value| value != "preserve")
            {
                return Err("Unsupported identifier whitespace policy".into());
            }
            if node.children().any(|n| !n.is_text()) {
                return Err("Structured identifier or comment requires raw preservation".into());
            }
            let name = node.text().unwrap_or("").to_string();
            let kind = match node.attribute("labels") {
                Some(labels) => NodeKind::AnnotatedIdentifier {
                    name,
                    labels: labels.into(),
                },
                None => NodeKind::Identifier(name),
            };
            tree(kind, vec![])
        }
        "real" => {
            attributes(node, &[])?;
            if node.children().any(|n| !n.is_text()) {
                return Err("Structured numeric literal requires raw preservation".into());
            }
            let text = node.text().unwrap_or("");
            let value: f64 = text.parse().map_err(|_| "Invalid real literal")?;
            if !value.is_finite() {
                return Err("Nonfinite literal is outside this editor subset".into());
            }
            tree(NodeKind::Number(text.into()), vec![])
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
            if nodes.len() != 2 || !math(nodes[0], "id") {
                return Err("Only an identifier definition can be structurally edited".into());
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
            if depth != 0 {
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
            if nodes.len() != 3 || nodes[0].tag_name().namespace() != Some(ML) {
                return Err("Unsupported operator/arity".into());
            }
            let op = nodes[0];
            if !elements(op)?.is_empty() {
                return Err("Operator carries unknown content".into());
            }
            let kind = match op.tag_name().name() {
                "plus" => NodeKind::Binary(BinaryOp::Add),
                "minus" => NodeKind::Binary(BinaryOp::Subtract),
                "mult" => NodeKind::Binary(BinaryOp::Multiply),
                "pow" => NodeKind::Power,
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
            tree(
                kind,
                vec![parse(nodes[1], depth + 1)?, parse(nodes[2], depth + 1)?],
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
        let children = elements(root)?;
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
        NodeKind::Number(number) => {
            let value: f64 = number.parse().map_err(|_| "Invalid number literal")?;
            if !value.is_finite() {
                return Err("Nonfinite number literal".into());
            }
            format!("<ml:real{ns}>{}</ml:real>", escape(number))
        }
        NodeKind::Group => format!("<ml:parens{ns}>{}</ml:parens>", child(0)?),
        NodeKind::Binary(BinaryOp::Define) => {
            if depth != 0
                || !matches!(
                    node.children[0].kind,
                    NodeKind::Identifier(_) | NodeKind::AnnotatedIdentifier { .. }
                )
            {
                return Err("Only top-level identifier definitions are supported".into());
            }
            format!("<ml:define{ns}>{}{}</ml:define>", child(0)?, child(1)?)
        }
        NodeKind::Binary(BinaryOp::Evaluate) => {
            if depth != 0 {
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
    serialize(tree, true, 0)
}

fn inferred_label(name: &str, definitions: &HashMap<String, String>) -> String {
    if let Some(label) = definitions.get(name) {
        return label.clone();
    }
    if name == "ORIGIN" {
        return "SYSTEM".into();
    }
    if name == "π" {
        return "CONSTANT".into();
    }
    if mathcraft_core::unit(name).is_some() {
        return "UNIT".into();
    }
    if matches!(name, "sin" | "cos" | "tan" | "ln" | "exp" | "sqrt" | "abs") {
        return "FUNCTION".into();
    }
    "VARIABLE".into()
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
        children.first().copied().filter(|n| math(*n, "id"))
    } else {
        None
    };
    let mut patches = Vec::new();
    for id in root.descendants().filter(|n| math(*n, "id")) {
        if id.attribute("labels").is_some() {
            continue;
        }
        if id.children().any(|n| !n.is_text()) {
            return Err("Automatic labels for structured identifiers are unsupported".into());
        }
        let name = id.text().unwrap_or("");
        let label = if Some(id) == definition {
            if name == "ORIGIN" {
                "SYSTEM".into()
            } else {
                "VARIABLE".into()
            }
        } else {
            inferred_label(name, definitions)
        };
        let start = id.range().start;
        let tag_end = wrapped[start..]
            .find(|c: char| c.is_whitespace() || c == '>' || c == '/')
            .ok_or("Invalid identifier tag")?
            + start;
        patches.push((tag_end, format!(" labels=\"{}\"", escape(&label))));
    }
    if let Some(id) = definition {
        let name = id.text().unwrap_or("").to_string();
        let label = id
            .attribute("labels")
            .map(str::to_string)
            .unwrap_or_else(|| {
                if name == "ORIGIN" {
                    "SYSTEM".into()
                } else {
                    "VARIABLE".into()
                }
            });
        definitions.insert(name, label);
    }
    let mut output = wrapped;
    for (at, text) in patches.into_iter().rev() {
        output.insert_str(at, &text);
    }
    let start = output.find('>').ok_or("Invalid context")? + 1;
    let end = output.rfind("</context>").ok_or("Invalid context")?;
    Ok(output[start..end].into())
}

/// Build calculation-only worksheet XML from explicitly positioned statements.
/// Regions retain their supplied IDs and order. The limited automatic label
/// policy runs in geometric order; it never changes explicit imported labels.
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
            if let Some(id) = global
                .children()
                .find(Node::is_element)
                .filter(|n| math(*n, "id"))
            {
                let name = id.text().unwrap_or("").to_string();
                let label = id.attribute("labels").unwrap_or("VARIABLE").to_string();
                definitions.insert(name, label);
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
            "<ml:id labels=\"VARIABLE\" label-is-contextual=\"true\">x</ml:id>",
            "<ml:id labels=\"VARIABLE\">x<ml:subscript>0</ml:subscript></ml:id>",
            "<ml:apply><ml:scale/><ml:real>1</ml:real><ml:id labels=\"UNIT\">m</ml:id></ml:apply>",
            "<ml:imag>1</ml:imag>",
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
