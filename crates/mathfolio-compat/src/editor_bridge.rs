//! A bounded structural bridge, not a Mathcad file converter.
//!
//! Explicit identifier labels and contextual display metadata survive editing.
//! `worksheet_xml` applies proven structural label constraints, then a bounded,
//! independent lookup policy using visible definitions and the prototype catalog.
//! Cached contextual labels do not become explicit user choices. This does not
//! implement Prime's complete contextual/wildcard label resolution.
//! Unsupported attributes or structures return an error so callers retain raw XML.

use mathfolio_editor::{BinaryOp, NodeKind, NumericBase, PortableTree};
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
/// None (`labels="*"`, or the shipped converter's `NONE` alias) is stored
/// independently of contextual display labels,
/// but neither is an explicit calculation namespace. Keep the raw decoder and
/// writer lossless; only the bounded calculation labeller uses this predicate.
pub(crate) fn explicit_identifier_label(
    labels: Option<&str>,
    contextual: Option<bool>,
) -> Option<&str> {
    labels.filter(|label| contextual != Some(true) && !matches!(*label, "*" | "NONE"))
}

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
    let mut output = String::with_capacity(text.len());
    let mut start = 0;
    for (index, byte) in text.bytes().enumerate() {
        let replacement = match byte {
            b'&' => "&amp;",
            b'<' => "&lt;",
            b'>' => "&gt;",
            b'"' => "&quot;",
            b'\'' => "&apos;",
            _ => continue,
        };
        // ASCII delimiters cannot split a UTF-8 code point. Copy untouched
        // spans byte-for-byte, including entity-looking input and Unicode.
        output.push_str(&text[start..index]);
        output.push_str(replacement);
        start = index + 1;
    }
    output.push_str(&text[start..]);
    output
}

#[cfg(test)]
#[path = "editor_bridge/radix_literal_tests.rs"]
mod radix_literal_tests;

#[cfg(test)]
#[path = "editor_bridge/escape_profile.rs"]
mod escape_profile;

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

fn definition_identifier<'a, 'input>(target: Node<'a, 'input>) -> Option<Node<'a, 'input>> {
    if math(target, "id") {
        Some(target)
    } else if math(target, "function") {
        target.children().find(|node| math(*node, "id"))
    } else if math(target, "apply") {
        let nodes: Vec<_> = target.children().filter(Node::is_element).collect();
        if nodes.first().is_some_and(|node| {
            math(*node, "indexer") || math(*node, "matrow") || math(*node, "matcol")
        }) {
            nodes.get(1).copied().filter(|node| math(*node, "id"))
        } else {
            None
        }
    } else {
        None
    }
}

fn tree(kind: NodeKind, children: Vec<PortableTree>) -> PortableTree {
    PortableTree { kind, children }
}

#[derive(Default)]
struct Parser<'a> {
    identifier_overrides: Option<&'a HashMap<roxmltree::NodeId, NodeKind>>,
}

impl Parser<'_> {
    fn parse_program(&self, node: Node<'_, '_>, depth: usize) -> Result<PortableTree, String> {
        attributes(node, &[])?;
        let statements = elements(node)?;
        if statements.is_empty() || statements.len() > 4096 {
            return Err("Program line count outside supported bounds".into());
        }
        Ok(tree(
            NodeKind::Program {
                statement_count: statements.len(),
            },
            statements
                .iter()
                .map(|statement| self.parse(*statement, depth + 1))
                .collect::<Result<_, _>>()?,
        ))
    }

    fn parse_program_wrapper(
        &self,
        node: Node<'_, '_>,
        wrapper: &str,
        depth: usize,
    ) -> Result<PortableTree, String> {
        if !math(node, wrapper) {
            return Err(format!("Expected {wrapper} program wrapper"));
        }
        attributes(node, &[])?;
        let children = elements(node)?;
        if children.len() != 1 {
            return Err(format!("Invalid {wrapper} program wrapper"));
        }
        self.parse_control_body(children[0], depth)
    }

    fn parse_control_body(
        &self,
        child: Node<'_, '_>,
        depth: usize,
    ) -> Result<PortableTree, String> {
        if math(child, "program") {
            self.parse_program(child, depth + 1)
        } else {
            // Original native reader dispatches then/else's one child and the
            // descriptor repair wraps a scalar body in BODY. BODY writes program;
            // retain that canonical structure without flattening nested programs.
            Ok(tree(
                NodeKind::Program { statement_count: 1 },
                vec![self.parse(child, depth + 1)?],
            ))
        }
    }

    fn parse_conditional_pair(
        &self,
        parts: &[Node<'_, '_>],
        depth: usize,
    ) -> Result<Vec<PortableTree>, String> {
        if parts.len() != 2 || !math(parts[0], "test") || !math(parts[1], "then") {
            return Err("Invalid conditional test/then pair".into());
        }
        attributes(parts[0], &[])?;
        let test = elements(parts[0])?;
        if test.len() != 1 {
            return Err("Invalid if test".into());
        }
        Ok(vec![
            self.parse(test[0], depth + 1)?,
            self.parse_program_wrapper(parts[1], "then", depth + 1)?,
        ])
    }

    fn parse_conditional(&self, node: Node<'_, '_>, depth: usize) -> Result<PortableTree, String> {
        attributes(node, &[])?;
        let parts = elements(node)?;
        if parts.len() < 2 {
            return Err("Conditional requires a test/then pair".into());
        }
        let mut children = self.parse_conditional_pair(&parts[..2], depth)?;
        let mut alternate = false;
        let mut has_else = false;
        let mut branch_count = 1;
        for (index, part) in parts[2..].iter().copied().enumerate() {
            if math(part, "elseif") || math(part, "alsoif") {
                let mode = math(part, "alsoif");
                if has_else {
                    return Err("Misplaced conditional branches".into());
                }
                branch_count += 1;
                if branch_count > 4096 {
                    return Err("Conditional branch count outside supported bounds".into());
                }
                attributes(part, &[])?;
                children.extend(self.parse_conditional_pair(&elements(part)?, depth + 1)?);
                // The shipped XML reader has one flag for the entire pair-nary
                // node: any alsoif sets it. Subsequent writing uses one uniform
                // spelling. This native import normalization is separate from
                // direct raw calculation XML's stricter accepted input policy.
                alternate |= mode;
            } else if math(part, "else") && index + 3 == parts.len() && !has_else {
                children.push(self.parse_program_wrapper(part, "else", depth + 1)?);
                has_else = true;
            } else {
                return Err("Unsupported conditional branch".into());
            }
        }
        Ok(tree(
            NodeKind::If {
                has_else,
                branch_count,
                alternate,
            },
            children,
        ))
    }

    fn parse_calculus_apply(
        &self,
        apply: Node<'_, '_>,
        nodes: &[Node<'_, '_>],
        depth: usize,
    ) -> Result<Option<PortableTree>, String> {
        let Some(op) = nodes.first().copied() else {
            return Ok(None);
        };
        let operator = op.tag_name().name();
        if !matches!(
            operator,
            "integral" | "derivative" | "partDerivative" | "summation" | "product"
        ) {
            return Ok(None);
        }
        attributes(apply, &[])?;
        attributes(
            op,
            if operator == "integral" {
                &[("algorithm", None), ("auto-algorithm", None)]
            } else {
                &[]
            },
        )?;
        if !elements(op)?.is_empty() {
            return Err("Calculus operator carries unknown content".into());
        }
        let lambda = *nodes.get(1).ok_or("Missing calculus lambda")?;
        if !math(lambda, "lambda") {
            return Err("Calculus requires lambda".into());
        }
        attributes(lambda, &[])?;
        let lambda_parts = elements(lambda)?;
        if lambda_parts.len() != 2 || !math(lambda_parts[0], "boundVars") {
            return Err("Invalid calculus lambda".into());
        }
        attributes(lambda_parts[0], &[])?;
        let variables = elements(lambda_parts[0])?;
        if variables.len() != 1 {
            return Err("Only one calculus variable is supported".into());
        }
        let body = self.parse(lambda_parts[1], depth + 1)?;
        let variable = self.parse(variables[0], depth + 1)?;
        if matches!(operator, "derivative" | "partDerivative") {
            if nodes.len() > 3 {
                return Err("Invalid derivative arity".into());
            }
            let degree = if let Some(degree) = nodes.get(2) {
                if !math(*degree, "degree") {
                    return Err("Invalid derivative degree wrapper".into());
                }
                attributes(*degree, &[])?;
                let value = elements(*degree)?;
                if value.len() != 1 {
                    return Err("Invalid derivative degree".into());
                }
                self.parse(value[0], depth + 1)?
            } else {
                tree(NodeKind::Placeholder, vec![])
            };
            return Ok(Some(tree(
                NodeKind::Derivative {
                    partial: operator == "partDerivative",
                },
                vec![body, variable, degree],
            )));
        }
        if nodes.len() != 4 {
            return Err("Definite calculus operator requires two bounds".into());
        }
        let parse_bound = |node: Node<'_, '_>, name: &str| -> Result<PortableTree, String> {
            if !math(node, name) {
                return Err(format!("Expected {name}"));
            }
            attributes(node, &[])?;
            let value = elements(node)?;
            if value.len() != 1 {
                return Err(format!("Invalid {name}"));
            }
            self.parse(value[0], depth + 1)
        };
        let lower = parse_bound(nodes[2], "lowerBound")?;
        let upper = parse_bound(nodes[3], "upperBound")?;
        let kind = match operator {
            "integral" => NodeKind::Integral,
            "summation" => NodeKind::Summation,
            "product" => NodeKind::Product,
            _ => unreachable!(),
        };
        Ok(Some(tree(kind, vec![body, variable, lower, upper])))
    }

    fn parse(&self, node: Node<'_, '_>, depth: usize) -> Result<PortableTree, String> {
        if depth > 128 || node.tag_name().namespace() != Some(ML) {
            return Err("Unsupported expression namespace/depth".into());
        }
        let result = match node.tag_name().name() {
            "id" => {
                let (name, labels, contextual, subscript) = identifier_parts(node)?;
                let kind = if let Some(kind) = self
                    .identifier_overrides
                    .and_then(|overrides| overrides.get(&node.id()))
                {
                    kind.clone()
                } else {
                    match (labels, contextual, subscript) {
                        (Some(labels), None, None) => {
                            NodeKind::AnnotatedIdentifier { name, labels }
                        }
                        (None, None, None) => NodeKind::Identifier(name),
                        (Some(labels), contextual, subscript) => NodeKind::NativeIdentifier {
                            name,
                            labels,
                            contextual,
                            subscript,
                        },
                        _ => {
                            return Err(
                                "Unlabelled native identifier requires raw preservation".into()
                            );
                        }
                    }
                };
                tree(kind, vec![])
            }
            "str" => {
                attributes(node, &[("space", Some(XML))])?;
                if node
                    .attribute((XML, "space"))
                    .is_some_and(|s| s != "preserve")
                    || node.children().any(|n| !n.is_text())
                {
                    return Err("Unsupported string literal metadata/content".into());
                }
                tree(
                    NodeKind::StringLiteral(node.children().filter_map(|n| n.text()).collect()),
                    vec![],
                )
            }
            "real" | "imag" => {
                attributes(
                    node,
                    if math(node, "imag") {
                        &[("symbol", None)]
                    } else {
                        &[("base", None)]
                    },
                )?;
                if node.children().any(|n| !n.is_text()) {
                    return Err("Structured numeric literal requires raw preservation".into());
                }
                let text = node.text().unwrap_or("");
                if math(node, "real")
                    && let Some(source_base) = node.attribute("base")
                {
                    match source_base {
                        "10" => {} // E124f treats base 10 as decimal; writer omits its default attr.
                        "2" | "8" | "16" => {
                            let base =
                                NumericBase::from_radix(source_base.parse().unwrap()).unwrap();
                            let (negative, digits) =
                                text.strip_prefix('-').map_or((false, text), |s| (true, s));
                            let number = tree(
                                NodeKind::based_number(digits, base).map_err(|e| e.to_string())?,
                                vec![],
                            );
                            return Ok(if negative {
                                tree(NodeKind::UnaryNeg, vec![number])
                            } else {
                                number
                            });
                        }
                        _ => {
                            return Err("Unsupported numeric base requires raw preservation".into());
                        }
                    }
                }
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
                tree(NodeKind::Group, vec![self.parse(nodes[0], depth + 1)?])
            }
            definition @ ("define" | "globalDefine") => {
                if depth != 0 {
                    return Err("Nested definition requires raw preservation".into());
                }
                attributes(node, &[])?;
                let nodes = elements(node)?;
                if nodes.len() != 2 || definition_identifier(nodes[0]).is_none() {
                    return Err("Unsupported definition target".into());
                }
                tree(
                    NodeKind::Binary(if definition == "globalDefine" {
                        BinaryOp::GlobalDefine
                    } else {
                        BinaryOp::Define
                    }),
                    nodes
                        .iter()
                        .map(|n| self.parse(*n, depth + 1))
                        .collect::<Result<_, _>>()?,
                )
            }
            "eval" => {
                if depth != 0
                    && !(depth == 1
                        && node
                            .parent_element()
                            .is_some_and(|p| math(p, "define") || math(p, "globalDefine")))
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
                    vec![
                        self.parse(nodes[0], depth + 1)?,
                        self.parse(units[0], depth + 1)?,
                    ],
                )
            }
            "program" => return self.parse_program(node, depth),
            "localDefine" => {
                attributes(node, &[("style", None)])?;
                if node
                    .attribute("style")
                    .is_some_and(|style| !matches!(style, "default" | "left-arrow" | "equal"))
                {
                    return Err("Unsupported local definition style".into());
                }
                let nodes = elements(node)?;
                if nodes.len() != 2 {
                    return Err("Invalid local definition arity".into());
                }
                tree(
                    NodeKind::LocalDefine,
                    nodes
                        .iter()
                        .map(|child| self.parse(*child, depth + 1))
                        .collect::<Result<_, _>>()?,
                )
            }
            "if" => self.parse_conditional(node, depth)?,
            "tryCatch" => {
                attributes(node, &[])?;
                let bodies = elements(node)?;
                if bodies.len() != 2 {
                    return Err("Try/on error requires exactly two bodies".into());
                }
                tree(
                    NodeKind::Try,
                    vec![
                        self.parse_control_body(bodies[0], depth + 1)?,
                        self.parse_control_body(bodies[1], depth + 1)?,
                    ],
                )
            }
            "for" => {
                attributes(node, &[])?;
                let nodes = elements(node)?;
                if nodes.len() != 3 || !math(nodes[2], "program") {
                    return Err("Invalid for variable/iterable/body structure".into());
                }
                tree(
                    NodeKind::For,
                    vec![
                        self.parse(nodes[0], depth + 1)?,
                        self.parse(nodes[1], depth + 1)?,
                        self.parse_program(nodes[2], depth + 1)?,
                    ],
                )
            }
            "while" => {
                attributes(node, &[])?;
                let nodes = elements(node)?;
                if nodes.len() != 2 || !math(nodes[1], "program") {
                    return Err("Invalid while condition/body structure".into());
                }
                tree(
                    NodeKind::While,
                    vec![
                        self.parse(nodes[0], depth + 1)?,
                        self.parse_program(nodes[1], depth + 1)?,
                    ],
                )
            }
            "return" => {
                attributes(node, &[])?;
                let nodes = elements(node)?;
                if nodes.len() != 1 {
                    return Err("Invalid return arity".into());
                }
                tree(NodeKind::Return, vec![self.parse(nodes[0], depth + 1)?])
            }
            "break" | "continue" => {
                attributes(node, &[])?;
                if !elements(node)?.is_empty() {
                    return Err("Loop control takes no arguments".into());
                }
                tree(
                    if math(node, "break") {
                        NodeKind::Break
                    } else {
                        NodeKind::Continue
                    },
                    vec![],
                )
            }
            "symEval" => {
                attributes(
                    node,
                    &[("hide-lhs", None), ("hide-keywords", None), ("style", None)],
                )?;
                let nodes = elements(node)?;
                let Some(input) = nodes.first() else {
                    return Err("Missing symbolic input".into());
                };
                let mut children = vec![self.parse(*input, depth + 1)?];
                for command in nodes.iter().skip(1) {
                    if math(*command, "symResult") {
                        attributes(*command, &[])?;
                        if elements(*command)?.len() != 1 || *command != *nodes.last().unwrap() {
                            return Err("Invalid cached symbolic result".into());
                        }
                        continue;
                    }
                    if !math(*command, "command") {
                        return Err("Invalid symbolic command".into());
                    }
                    attributes(*command, &[])?;
                    let args = elements(*command)?;
                    if args.is_empty() || args.len() > 2 {
                        return Err("Invalid symbolic command arity".into());
                    }
                    children.push(tree(
                        NodeKind::SymbolicCommand {
                            argument_count: args.len() - 1,
                        },
                        args.iter()
                            .map(|arg| self.parse(*arg, depth + 1))
                            .collect::<Result<_, _>>()?,
                    ));
                }
                tree(
                    NodeKind::SymbolicEvaluation {
                        command_count: children.len() - 1,
                    },
                    children,
                )
            }
            "apply" => {
                attributes(node, &[])?;
                let nodes = elements(node)?;
                if nodes.is_empty() || nodes[0].tag_name().namespace() != Some(ML) {
                    return Err("Unsupported operator/arity".into());
                }
                let op = nodes[0];
                if let Some(calculus) = self.parse_calculus_apply(node, &nodes, depth)? {
                    return Ok(calculus);
                }
                if math(op, "id") {
                    let arguments = if nodes.len() == 2 && math(nodes[1], "sequence") {
                        attributes(nodes[1], &[])?;
                        elements(nodes[1])?
                    } else {
                        nodes[1..].to_vec()
                    };
                    let mut children = vec![self.parse(op, depth + 1)?];
                    for arg in &arguments {
                        children.push(self.parse(*arg, depth + 1)?);
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
                    "crossProduct" => NodeKind::Binary(BinaryOp::CrossProduct),
                    "polar" => NodeKind::Binary(BinaryOp::Polar),
                    "scale" => NodeKind::Binary(BinaryOp::Scale),
                    "pow" => NodeKind::Power,
                    "neg" => NodeKind::UnaryNeg,
                    "not" => NodeKind::LogicalNot,
                    "equal" => NodeKind::Binary(BinaryOp::Equal),
                    "notEqual" => NodeKind::Binary(BinaryOp::NotEqual),
                    "lessThan" => NodeKind::Binary(BinaryOp::LessThan),
                    "lessOrEqual" => NodeKind::Binary(BinaryOp::LessOrEqual),
                    "greaterThan" => NodeKind::Binary(BinaryOp::GreaterThan),
                    "greaterOrEqual" => NodeKind::Binary(BinaryOp::GreaterOrEqual),
                    "and" => NodeKind::Binary(BinaryOp::And),
                    "or" => NodeKind::Binary(BinaryOp::Or),
                    "xor" => NodeKind::Binary(BinaryOp::Xor),
                    "absval" => NodeKind::Absolute,
                    "factorial" => NodeKind::Factorial,
                    "percent" => NodeKind::Percent,
                    "degree_op" => NodeKind::Degree,
                    "transpose" => NodeKind::Transpose,
                    "conjugate" => NodeKind::Conjugate,
                    "vectorize" => NodeKind::Vectorize,
                    "determinant" => NodeKind::Determinant,
                    "matrow" => NodeKind::Row,
                    "matcol" => NodeKind::Column,
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
                        let mut children = vec![self.parse(nodes[1], depth + 1)?];
                        for i in &indices {
                            children.push(self.parse(*i, depth + 1)?);
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
                let arity = if matches!(
                    kind,
                    NodeKind::UnaryNeg
                        | NodeKind::LogicalNot
                        | NodeKind::Absolute
                        | NodeKind::Factorial
                        | NodeKind::Percent
                        | NodeKind::Degree
                        | NodeKind::Transpose
                        | NodeKind::Conjugate
                        | NodeKind::Vectorize
                        | NodeKind::Determinant
                ) {
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
                        .map(|n| self.parse(*n, depth + 1))
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
                let mut children = vec![self.parse(nodes[0], depth + 1)?];
                for p in &params {
                    children.push(self.parse(*p, depth + 1)?);
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
                let display = nodes
                    .first()
                    .filter(|n| n.tag_name().name() == "display")
                    .map(|n| crate::source_matrix_display::read_display(*n, [rows, cols]))
                    .transpose()?;
                let nodes = if display.is_some() {
                    &nodes[1..]
                } else {
                    nodes.as_slice()
                };
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
                        row_major.push(self.parse(nodes[col * rows + row], depth + 1)?);
                    }
                }
                tree(
                    NodeKind::Matrix {
                        rows,
                        cols,
                        display,
                    },
                    row_major,
                )
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
                        .map(|n| self.parse(*n, depth + 1))
                        .collect::<Result<_, _>>()?,
                )
            }
            name => return Err(format!("Unsupported MathML {name}; retain raw XML")),
        };
        Ok(result)
    }
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
    tree_from_mathml_node(roots.remove(0))
}

/// Decode an already parsed fragment using the same validation as
/// `tree_from_mathml`. Its document supplies namespace context; no standalone
/// copy or second XML parse is necessary for each region in a worksheet.
pub fn tree_from_mathml_node(mut root: Node<'_, '_>) -> Result<PortableTree, String> {
    if !root.is_element() || root.range().len() > MAX_XML {
        return Err("Invalid MathML element/import budget".into());
    }
    if math(root, "symResult") {
        attributes(root, &[])?;
        let children = elements(root)?;
        if children.len() != 1 {
            return Err("Expected one expression in symbolic result".into());
        }
        root = children[0];
    } else if root.tag_name().namespace() != Some(ML) {
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
    let tree = Parser::default().parse(root, 0)?;
    tree.validate()
        .map_err(|e| format!("Invalid structural tree: {e:?}"))?;
    Ok(tree)
}

fn serialize(node: &PortableTree, root: bool, depth: usize) -> Result<String, String> {
    serialize_with_plan(node, root, depth, None)
}

fn serialize_with_plan(
    node: &PortableTree,
    root: bool,
    depth: usize,
    plan: Option<&typed::Plan>,
) -> Result<String, String> {
    if depth > 128 {
        return Err("Math tree exceeds serialization depth".into());
    }
    let ns = if root {
        format!(" xmlns:ml=\"{ML}\"")
    } else {
        String::new()
    };
    let child = |i: usize| {
        serialize_with_plan(
            &node.children[i],
            false,
            depth + 1,
            plan.and_then(|p| p.children.get(&i)),
        )
    };
    let kind = plan
        .and_then(|p| p.replacement.as_ref())
        .unwrap_or(&node.kind);
    let text = match kind {
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
        NodeKind::StringLiteral(value) => format!(
            "<ml:str{ns} xml:space=\"preserve\">{}</ml:str>",
            escape(value).replace('\r', "&#13;")
        ),
        NodeKind::BasedNumber { literal, base } => {
            let digits = base.digits(literal).ok_or("Invalid based numeric lexeme")?;
            format!(
                "<ml:real{ns} base=\"{}\">{}</ml:real>",
                base.radix(),
                escape(digits)
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
            let symbol = if let NodeKind::ImaginarySymbol { symbol, .. } = kind {
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
        NodeKind::LogicalNot => format!("<ml:apply{ns}><ml:not/>{}</ml:apply>", child(0)?),
        NodeKind::Absolute => format!("<ml:apply{ns}><ml:absval/>{}</ml:apply>", child(0)?),
        NodeKind::Factorial => format!("<ml:apply{ns}><ml:factorial/>{}</ml:apply>", child(0)?),
        NodeKind::Percent => format!("<ml:apply{ns}><ml:percent/>{}</ml:apply>", child(0)?),
        NodeKind::Degree => format!("<ml:apply{ns}><ml:degree_op/>{}</ml:apply>", child(0)?),
        NodeKind::Transpose => format!("<ml:apply{ns}><ml:transpose/>{}</ml:apply>", child(0)?),
        NodeKind::Conjugate => format!("<ml:apply{ns}><ml:conjugate/>{}</ml:apply>", child(0)?),
        NodeKind::Vectorize => format!("<ml:apply{ns}><ml:vectorize/>{}</ml:apply>", child(0)?),
        NodeKind::Determinant => format!("<ml:apply{ns}><ml:determinant/>{}</ml:apply>", child(0)?),
        NodeKind::Row | NodeKind::Column => format!(
            "<ml:apply{ns}><ml:{}/>{}{}</ml:apply>",
            if node.kind == NodeKind::Row {
                "matrow"
            } else {
                "matcol"
            },
            child(0)?,
            child(1)?
        ),
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
        NodeKind::Matrix {
            rows,
            cols,
            display,
        } => {
            let mut content = String::new();
            write_source_matrix_display(&mut content, *display);
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
        NodeKind::Program { statement_count } => {
            let statements = (0..*statement_count)
                .map(child)
                .collect::<Result<Vec<_>, _>>()?
                .join("");
            format!("<ml:program{ns}>{statements}</ml:program>")
        }
        NodeKind::LocalDefine => {
            if matches!(node.children[0].kind, NodeKind::Function { .. })
                && node.children[0].children.iter().any(|child| {
                    !matches!(
                        child.kind,
                        NodeKind::Identifier(_)
                            | NodeKind::AnnotatedIdentifier { .. }
                            | NodeKind::NativeIdentifier { .. }
                    )
                })
            {
                return Err(
                    "Local function name and parameters must be complete identifiers".into(),
                );
            }
            format!(
                "<ml:localDefine{ns}>{}{}</ml:localDefine>",
                child(0)?,
                child(1)?
            )
        }
        NodeKind::If {
            has_else,
            branch_count,
            alternate,
        } => {
            let mut output = format!("<ml:if{ns}>");
            let branch = if *alternate { "alsoif" } else { "elseif" };
            for index in 0..*branch_count {
                if index != 0 {
                    output.push_str(&format!("<ml:{branch}>"));
                }
                output.push_str(&format!(
                    "<ml:test>{}</ml:test><ml:then>{}</ml:then>",
                    child(index * 2)?,
                    child(index * 2 + 1)?
                ));
                if index != 0 {
                    output.push_str(&format!("</ml:{branch}>"));
                }
            }
            if *has_else {
                output.push_str(&format!("<ml:else>{}</ml:else>", child(branch_count * 2)?));
            }
            output.push_str("</ml:if>");
            output
        }
        NodeKind::For => format!(
            "<ml:for{ns}>{}{}{}</ml:for>",
            child(0)?,
            child(1)?,
            child(2)?
        ),
        NodeKind::While => {
            format!("<ml:while{ns}>{}{}</ml:while>", child(0)?, child(1)?)
        }
        NodeKind::Return => format!("<ml:return{ns}>{}</ml:return>", child(0)?),
        NodeKind::Try => format!("<ml:tryCatch{ns}>{}{}</ml:tryCatch>", child(0)?, child(1)?),
        NodeKind::Break => format!("<ml:break{ns}/>"),
        NodeKind::Continue => format!("<ml:continue{ns}/>"),
        NodeKind::Integral | NodeKind::Summation | NodeKind::Product => {
            let op = match &node.kind {
                NodeKind::Integral => "integral",
                NodeKind::Summation => "summation",
                NodeKind::Product => "product",
                _ => unreachable!(),
            };
            format!(
                "<ml:apply{ns}><ml:{op}/><ml:lambda><ml:boundVars>{}</ml:boundVars>{}</ml:lambda><ml:lowerBound>{}</ml:lowerBound><ml:upperBound>{}</ml:upperBound></ml:apply>",
                child(1)?,
                child(0)?,
                child(2)?,
                child(3)?
            )
        }
        NodeKind::Derivative { partial } => {
            let op = if *partial {
                "partDerivative"
            } else {
                "derivative"
            };
            format!(
                "<ml:apply{ns}><ml:{op}/><ml:lambda><ml:boundVars>{}</ml:boundVars>{}</ml:lambda><ml:degree>{}</ml:degree></ml:apply>",
                child(1)?,
                child(0)?,
                child(2)?
            )
        }
        NodeKind::SymbolicEvaluation { command_count } => {
            let commands = (1..=*command_count)
                .map(child)
                .collect::<Result<Vec<_>, _>>()?
                .join("")
                .replace(" xml:space=\"preserve\"", "");
            let input = child(0)?.replace(" xml:space=\"preserve\"", "");
            format!("<ml:symEval{ns}>{input}{commands}</ml:symEval>")
        }
        NodeKind::SymbolicCommand { argument_count } => {
            let arguments = (0..=*argument_count)
                .map(child)
                .collect::<Result<Vec<_>, _>>()?
                .join("");
            format!("<ml:command{ns}>{arguments}</ml:command>")
        }
        NodeKind::Binary(definition @ (BinaryOp::Define | BinaryOp::GlobalDefine)) => {
            if depth != 0
                || !matches!(
                    node.children[0].kind,
                    NodeKind::Identifier(_)
                        | NodeKind::AnnotatedIdentifier { .. }
                        | NodeKind::NativeIdentifier { .. }
                        | NodeKind::Function { .. }
                        | NodeKind::Index { .. }
                        | NodeKind::Row
                        | NodeKind::Column
                )
            {
                return Err(
                    "Only top-level identifier, function, indexed or array slice definitions are supported"
                        .into(),
                );
            }
            if matches!(
                node.children[0].kind,
                NodeKind::Index { .. } | NodeKind::Row | NodeKind::Column
            ) && !matches!(
                node.children[0].children[0].kind,
                NodeKind::Identifier(_)
                    | NodeKind::AnnotatedIdentifier { .. }
                    | NodeKind::NativeIdentifier { .. }
            ) {
                return Err(
                    "An indexed or array slice definition target must be a named array".into(),
                );
            }
            let tag = if *definition == BinaryOp::GlobalDefine {
                "globalDefine"
            } else {
                "define"
            };
            format!("<ml:{tag}{ns}>{}{}</ml:{tag}>", child(0)?, child(1)?)
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
                NodeKind::Binary(BinaryOp::CrossProduct) => "crossProduct",
                NodeKind::Binary(BinaryOp::Polar) => "polar",
                NodeKind::Binary(BinaryOp::Scale) => "scale",
                NodeKind::Binary(BinaryOp::InlineDivide) => "div inline=\"true\"",
                NodeKind::Binary(BinaryOp::Equal) => "equal",
                NodeKind::Binary(BinaryOp::NotEqual) => "notEqual",
                NodeKind::Binary(BinaryOp::LessThan) => "lessThan",
                NodeKind::Binary(BinaryOp::LessOrEqual) => "lessOrEqual",
                NodeKind::Binary(BinaryOp::GreaterThan) => "greaterThan",
                NodeKind::Binary(BinaryOp::GreaterOrEqual) => "greaterOrEqual",
                NodeKind::Binary(BinaryOp::And) => "and",
                NodeKind::Binary(BinaryOp::Or) => "or",
                NodeKind::Binary(BinaryOp::Xor) => "xor",
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
    tree_to_mathml_proven(tree).map(|(source, _)| source)
}

fn validate_mathml_tree(tree: &PortableTree) -> Result<bool, String> {
    tree.validate()
        .map_err(|e| format!("Invalid structural tree: {e:?}"))?;
    fn context(
        node: &PortableTree,
        allow_eval: bool,
        root: bool,
        inside_program: bool,
    ) -> Result<(), String> {
        if matches!(node.kind, NodeKind::Binary(BinaryOp::Evaluate)) && !allow_eval {
            return Err("Evaluation is only supported at the region root or definition RHS".into());
        }
        if matches!(
            node.kind,
            NodeKind::Binary(BinaryOp::Define | BinaryOp::GlobalDefine)
        ) && !root
        {
            return Err("Nested definitions require raw preservation".into());
        }
        if inside_program && matches!(node.kind, NodeKind::SymbolicEvaluation { .. }) {
            return Err("Symbolic evaluation cannot be used inside a program".into());
        }
        if matches!(node.kind, NodeKind::LocalDefine) && !inside_program {
            return Err("Local definitions require a program".into());
        }
        for (i, child) in node.children.iter().enumerate() {
            context(
                child,
                matches!(
                    node.kind,
                    NodeKind::Binary(BinaryOp::Define | BinaryOp::GlobalDefine)
                ) && i == 1,
                false,
                inside_program || matches!(node.kind, NodeKind::Program { .. }),
            )?;
        }
        Ok(())
    }
    context(tree, true, true, false)?;
    Ok(typed::eligible_root(tree))
}

fn tree_to_mathml_proven(tree: &PortableTree) -> Result<(String, bool), String> {
    let eligible = validate_mathml_tree(tree)?;
    let source = if eligible {
        serialization_writer::serialize(tree, true, 0, None)?
    } else {
        serialize(tree, true, 0)?
    };
    Ok((source, eligible))
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
    if mathfolio_core::unit(name).is_some() {
        return "UNIT".into();
    }
    if mathfolio_core::units::constant_lookup(name).is_some() {
        return "CONSTANT".into();
    }
    if crate::scalars::is_builtin(name)
        || crate::strings::is_builtin(name)
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

fn declaration_label_parts(
    name: &str,
    labels: Option<&str>,
    contextual: Option<bool>,
    subscript: Option<&str>,
    direct: bool,
) -> String {
    if let Some(label) = explicit_identifier_label(labels, contextual) {
        label.into()
    } else if subscript.is_none() && system_identifier(name) && direct {
        "SYSTEM".into()
    } else {
        "VARIABLE".into()
    }
}

fn declaration_label(id: Node<'_, '_>) -> Result<String, String> {
    let (name, labels, contextual, subscript) = identifier_parts(id)?;
    Ok(declaration_label_parts(
        &name,
        labels.as_deref(),
        contextual,
        subscript.as_deref(),
        id.parent_element()
            .is_some_and(|p| math(p, "define") || math(p, "globalDefine")),
    ))
}

#[derive(Default)]
struct LabelFacts<'a> {
    declaration: bool,
    direct_declaration: bool,
    structural: Option<&'a str>,
    binder: Option<&'a str>,
    call_target: bool,
}

fn resolve_identifier_label(
    name: &str,
    labels: Option<&str>,
    contextual: Option<bool>,
    subscript: Option<&str>,
    facts: LabelFacts<'_>,
    definitions: &HashMap<String, String>,
    capture: Option<&mut worksheet_cache::FragmentContext>,
) -> Option<String> {
    if explicit_identifier_label(labels, contextual).is_some() {
        return None;
    }
    let identity = identifier_identity(name, subscript);
    Some(if facts.declaration {
        declaration_label_parts(
            name,
            labels,
            contextual,
            subscript,
            facts.direct_declaration,
        )
    } else if let Some(label) = facts.structural {
        label.into()
    } else if let Some(label) = facts.binder {
        label.into()
    } else {
        if let Some(capture) = capture {
            capture.observe(identity.clone(), definitions);
        }
        if facts.call_target
            && !definitions.contains_key(&identity)
            && (crate::scalars::is_builtin(&identity)
                || crate::strings::is_builtin(&identity)
                || crate::arrays::is_builtin(&identity)
                || crate::statistics::is_builtin(&identity)
                || crate::numerics::is_builtin(&identity))
        {
            "FUNCTION".into()
        } else {
            inferred_label(&identity, definitions)
        }
    })
}

// Descriptor-table constraints apply to an entire child subtree. Index operands
// interrupt this inheritance: Prime's ForceUnlabeled... keeps them unresolved.
// Here they continue through bounded lookup, rather than inheriting UNIT.
fn structural_label(id: Node<'_, '_>) -> Option<&'static str> {
    let mut child = id;
    while let Some(parent) = child.parent_element() {
        if math(parent, "apply") {
            let children: Vec<_> = parent.children().filter(Node::is_element).collect();
            if children.first().is_some_and(|op| {
                math(*op, "indexer") || math(*op, "matrow") || math(*op, "matcol")
            }) && children
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
        if math(parent, "for") && parent.first_element_child() == Some(child) {
            return Some("VARIABLE");
        }
        if (math(parent, "define") || math(parent, "globalDefine") || math(parent, "localDefine"))
            && parent.children().find(Node::is_element) == Some(child)
        {
            return Some("VARIABLE");
        }
        child = parent;
    }
    None
}

#[path = "editor_bridge/serialization_writer.rs"]
mod serialization_writer;

#[path = "editor_bridge/typed.rs"]
mod typed;
pub use typed::{MathmlTraces, TypedMathml};
#[cfg(test)]
#[path = "editor_bridge/worksheet_assembly_tests.rs"]
mod worksheet_assembly_tests;
#[path = "editor_bridge/worksheet_cache.rs"]
mod worksheet_cache;
pub use worksheet_cache::{
    DisplayProjection, GeneratedRegion, GeneratedWorksheet, WorksheetXmlCache,
};

pub(crate) fn label_fragment(
    xml: &str,
    definitions: &mut HashMap<String, String>,
) -> Result<String, String> {
    let wrapped = format!("<context xmlns:ml=\"{ML}\" xmlns=\"{WS}\">{xml}</context>");
    let document = roxmltree::Document::parse(&wrapped).map_err(|e| e.to_string())?;
    label_parsed_fragment(document.root_element(), &wrapped, definitions, None)
}

fn label_parsed_fragment(
    context: Node<'_, '_>,
    wrapped: &str,
    definitions: &mut HashMap<String, String>,
    mut capture: Option<&mut worksheet_cache::FragmentContext>,
) -> Result<String, String> {
    let roots = elements(context)?;
    if roots.len() == 1 && roots[0].has_tag_name((WS, "spec-table")) {
        return crate::spec_table::label_fragment(roots[0], wrapped, definitions);
    }
    if roots.len() != 1
        || !(math(roots[0], "define")
            || math(roots[0], "eval")
            || math(roots[0], "globalDefine")
            || math(roots[0], "symEval"))
    {
        return Err(
            "Calculation accepts only explicit define/eval/globalDefine/symEval regions".into(),
        );
    }
    let root = roots[0];
    if let Some(capture) = capture.as_deref_mut() {
        capture.cacheable = true;
    }
    let definition = if math(root, "define") || math(root, "globalDefine") {
        let children: Vec<_> = root.children().filter(Node::is_element).collect();
        children.first().copied().and_then(definition_identifier)
    } else {
        None
    };
    // Bound variables shadow the worksheet environment. An overlay avoids
    // cloning every preceding definition for every worksheet region (quadratic
    // work when a document contains thousands of definitions).
    let mut bound_scopes: HashMap<roxmltree::NodeId, HashMap<String, String>> = HashMap::new();
    for bound in root.descendants().filter(|n| math(*n, "boundVars")) {
        let Some(parent) = bound.parent_element() else {
            continue;
        };
        let scope = if math(parent, "function") {
            parent.parent_element().filter(|n| {
                math(*n, "define") || math(*n, "globalDefine") || math(*n, "localDefine")
            })
        } else if math(parent, "lambda") {
            Some(parent)
        } else {
            None
        };
        let Some(scope) = scope else { continue };
        for parameter in bound.children().filter(|n| math(*n, "id")) {
            let (name, labels, contextual, sub) = identifier_parts(parameter)?;
            bound_scopes.entry(scope.id()).or_default().insert(
                identifier_identity(&name, sub.as_deref()),
                explicit_identifier_label(labels.as_deref(), contextual)
                    .unwrap_or("VARIABLE")
                    .into(),
            );
        }
    }
    // A program for-loop binds its identifier in its body, while its iteration
    // source still resolves in the surrounding scope.
    for scope in root.descendants().filter(|n| math(*n, "for")) {
        if let Some(parameter) = scope.first_element_child().filter(|n| math(*n, "id"))
            && let Some(body) = scope.children().filter(Node::is_element).nth(2)
        {
            let (name, labels, contextual, sub) = identifier_parts(parameter)?;
            bound_scopes.entry(body.id()).or_default().insert(
                identifier_identity(&name, sub.as_deref()),
                explicit_identifier_label(labels.as_deref(), contextual)
                    .unwrap_or("VARIABLE")
                    .into(),
            );
        }
    }
    // Program assignment targets are local names even when their spelling is
    // also a unit. Only the base name is declared; row/column/index operands
    // keep their own lookup context through structural_label above.
    let mut program_locals: HashMap<roxmltree::NodeId, HashMap<String, String>> = HashMap::new();
    for assignment in root.descendants().filter(|n| math(*n, "localDefine")) {
        if let (Some(program), Some(id)) = (
            assignment.ancestors().find(|n| math(*n, "program")),
            assignment
                .first_element_child()
                .and_then(definition_identifier),
        ) {
            let (name, labels, contextual, subscript) = identifier_parts(id)?;
            program_locals.entry(program.id()).or_default().insert(
                identifier_identity(&name, subscript.as_deref()),
                explicit_identifier_label(labels.as_deref(), contextual)
                    .unwrap_or("VARIABLE")
                    .into(),
            );
        }
    }
    let mut overrides = capture
        .as_ref()
        .filter(|c| c.decode_tree)
        .map(|_| HashMap::new());
    let mut patches = Vec::new();
    for id in root.descendants().filter(|n| math(*n, "id")) {
        if id.ancestors().any(|ancestor| math(ancestor, "command")) {
            continue;
        }
        let (name, labels, contextual, subscript) = identifier_parts(id)?;
        if explicit_identifier_label(labels.as_deref(), contextual).is_some() {
            continue;
        }
        let identity = identifier_identity(&name, subscript.as_deref());
        let facts = LabelFacts {
            declaration: Some(id) == definition,
            direct_declaration: id
                .parent_element()
                .is_some_and(|p| math(p, "define") || math(p, "globalDefine")),
            structural: structural_label(id),
            binder: id
                .ancestors()
                .find_map(|scope| {
                    bound_scopes
                        .get(&scope.id())
                        .or_else(|| program_locals.get(&scope.id()))
                        .and_then(|names| names.get(&identity))
                })
                .map(String::as_str),
            call_target: id.parent().is_some_and(|p| {
                math(p, "apply") && p.children().find(Node::is_element) == Some(id)
            }),
        };
        let Some(label) = resolve_identifier_label(
            &name,
            labels.as_deref(),
            contextual,
            subscript.as_deref(),
            facts,
            definitions,
            capture.as_deref_mut(),
        ) else {
            continue;
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
        if let Some(overrides) = overrides.as_mut() {
            let NodeKind::NativeIdentifier {
                name,
                labels,
                subscript,
                ..
            } = node.kind
            else {
                unreachable!()
            };
            let kind = match subscript {
                None => NodeKind::AnnotatedIdentifier { name, labels },
                Some(subscript) => NodeKind::NativeIdentifier {
                    name,
                    labels,
                    contextual: None,
                    subscript: Some(subscript),
                },
            };
            overrides.insert(id.id(), kind);
        }
    }
    if let Some(id) = definition {
        let (name, _, _, subscript) = identifier_parts(id)?;
        let identity = identifier_identity(&name, subscript.as_deref());
        let label = declaration_label(id)?;
        if let Some(capture) = capture.as_deref_mut() {
            capture.defined = Some((identity.clone(), label.clone()));
        }
        definitions.insert(identity, label);
    }
    if let Some(overrides) = overrides.as_ref() {
        let decoded = Parser {
            identifier_overrides: Some(overrides),
        }
        .parse(root, 0)
        .and_then(|tree| {
            tree.validate()
                .map_err(|e| format!("Invalid structural tree: {e:?}"))?;
            Ok(tree)
        });
        capture.as_mut().unwrap().decoded_tree = Some(decoded);
    }
    #[cfg(test)]
    let _timing = worksheet_cache::profile::scope(worksheet_cache::profile::Phase::Patch);
    // A cold cache may parse independent context nodes in one document. Node
    // ranges then address that shared document; only this context is copied.
    let context_range = context.range();
    let mut output = wrapped[context_range.clone()].to_owned();
    for (range, text) in patches.into_iter().rev() {
        output.replace_range(
            range.start - context_range.start..range.end - context_range.start,
            &text,
        );
    }
    let start = output.find('>').ok_or("Invalid context")? + 1;
    let end = output.rfind("</context>").ok_or("Invalid context")?;
    Ok(output[start..end].into())
}

/// Validate and return the exact stable geometric order used by worksheet_xml.
/// Positions supplied here are already loaded worksheet coordinates; native
/// placement-grid snapping belongs to the owning import/evaluation layer.
pub fn worksheet_region_order(regions: &[(u64, f64, f64, String)]) -> Result<Vec<usize>, String> {
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
    Ok(order)
}

#[path = "editor_bridge/area_worksheet.rs"]
mod area_worksheet;
pub use area_worksheet::worksheet_xml_with_areas;

/// Build calculation-only worksheet XML from explicitly positioned statements.
/// Regions retain their supplied IDs and order. The limited automatic label
/// policy runs in geometric order; it never changes explicit imported labels.
/// Contextual labels are cached display feedback, so only this calculation copy
/// resolves them again. The stored expression and its metadata stay unchanged.
pub fn worksheet_xml(regions: &[(u64, f64, f64, String)]) -> Result<String, String> {
    worksheet_xml_with(
        regions,
        |_, _, xml, definitions| label_fragment(xml, definitions),
        None,
    )
}

fn worksheet_xml_with(
    regions: &[(u64, f64, f64, String)],
    mut label: impl FnMut(usize, u64, &str, &mut HashMap<String, String>) -> Result<String, String>,
    mut metadata: Option<&mut Vec<GeneratedRegion>>,
) -> Result<String, String> {
    let order = {
        #[cfg(test)]
        let _timing = worksheet_cache::profile::scope(worksheet_cache::profile::Phase::Order);
        worksheet_region_order(regions)?
    };
    let mut definitions = HashMap::new();
    // Globals participate before all normal regions. This only chooses a
    // missing label; the compatibility evaluator owns validity/conflict rules.
    {
        #[cfg(test)]
        let _timing = worksheet_cache::profile::scope(worksheet_cache::profile::Phase::Globals);
        for (_, _, _, xml) in regions {
            // XML local names cannot be written using character/entity escapes.
            // A top-level MathML globalDefine therefore requires these exact
            // bytes. False positives still take the full parser path; all other
            // fragments are validated by label_fragment below, as before.
            if !xml.contains("globalDefine") {
                continue;
            }
            let context = format!("<context xmlns:ml=\"{ML}\" xmlns=\"{WS}\">{xml}</context>");
            let document = roxmltree::Document::parse(&context).map_err(|e| e.to_string())?;
            for global in document
                .root_element()
                .children()
                .filter(|n| math(*n, "globalDefine"))
            {
                if let Some(target) = global.children().find(Node::is_element) {
                    let id = definition_identifier(target);
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
    }
    let mut labelled = vec![String::new(); regions.len()];
    for index in order {
        labelled[index] = label(index, regions[index].0, &regions[index].3, &mut definitions)?;
    }
    #[cfg(test)]
    let _timing = worksheet_cache::profile::scope(worksheet_cache::profile::Phase::Assemble);
    let prefix = format!("<worksheet xmlns=\"{WS}\" xmlns:ml=\"{ML}\"><regions>");
    // All fragments have already been labelled, preserving the original error
    // order and cache effects. Reserve only a bounded output-size hint.
    let capacity = labelled
        .iter()
        .fold(prefix.len(), |sum, xml| sum.saturating_add(xml.len()))
        .saturating_add(regions.len().saturating_mul(96))
        .min(MAX_XML);
    let mut output = String::with_capacity(capacity);
    output.push_str(&prefix);
    for ((id, left, top, _), math) in regions.iter().zip(labelled) {
        use std::fmt::Write;
        write!(output, "<region region-id=\"{id}\" left=\"").unwrap();
        let left_start = output.len();
        write!(output, "{left}").unwrap();
        let left_end = output.len();
        output.push_str("\" top=\"");
        let top_start = output.len();
        write!(output, "{top}").unwrap();
        let top_end = output.len();
        output.push_str("\">");
        let math_start = output.len();
        if math.starts_with("<spec-table ") {
            output.push_str(&math);
        } else {
            use std::fmt::Write;
            write!(output, "<math resultRef=\"{id}\">").unwrap();
            output.push_str(&math);
            output.push_str("</math>");
        }
        if let Some(metadata) = metadata.as_mut() {
            metadata.push(GeneratedRegion {
                id: *id,
                left: left_start..left_end,
                top: top_start..top_end,
                math: (!math.starts_with("<spec-table ")).then_some(math_start..output.len()),
            });
        }
        output.push_str("</region>");
        if output.len() > MAX_XML {
            return Err("Worksheet exceeds XML budget".into());
        }
    }
    output.push_str("</regions></worksheet>");
    Ok(output)
}

#[cfg(test)]
mod parsed_node_bridge_tests {
    use super::*;

    #[test]
    fn parsed_node_matches_string_bridge_with_inherited_namespaces_and_validation() {
        for fragment in [
            "<ml:eval><ml:id>m</ml:id><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>",
            "<math resultRef='2'><ml:define><ml:id>한글</ml:id><ml:real>3</ml:real></ml:define><resultFormat/></math>",
            "<math unsupported='x'><ml:real>2</ml:real></math>",
            "<math><ml:real>2</ml:real><ml:real>3</ml:real></math>",
            "<ml:id labels='VARIABLE' label-is-contextual='invalid'>m</ml:id>",
        ] {
            let wrapped = format!("<context xmlns='{WS}' xmlns:ml='{ML}'>{fragment}</context>");
            let document = roxmltree::Document::parse(&wrapped).unwrap();
            let node = document.root_element().first_element_child().unwrap();
            assert_eq!(
                tree_from_mathml_node(node),
                tree_from_mathml(fragment),
                "{fragment}"
            );
        }
    }

    #[test]
    fn global_prepass_fast_filter_preserves_late_globals_and_malformed_rejection() {
        let eval = "<ml:eval><ml:id>m</ml:id><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>";
        let global = "<ml:globalDefine><ml:id>m</ml:id><ml:real>7</ml:real></ml:globalDefine>";
        let output =
            worksheet_xml(&[(1, 0., 0., eval.into()), (2, 0., 999., global.into())]).unwrap();
        let document = roxmltree::Document::parse(&output).unwrap();
        let m = document
            .descendants()
            .find(|n| math(*n, "eval"))
            .unwrap()
            .first_element_child()
            .unwrap();
        assert_eq!(m.attribute("labels"), Some("VARIABLE"));
        assert!(worksheet_xml(&[(1, 0., 0., "<ml:eval><ml:id>m</ml:eval>".into())]).is_err());
        let false_positive = "<ml:eval><ml:id>globalDefine</ml:id><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>";
        assert!(worksheet_xml(&[(1, 0., 0., false_positive.into())]).is_ok());
    }

    #[test]
    fn shared_region_order_is_stable_and_preserves_coordinate_validation() {
        let rows = vec![
            (2, 10., 10., "".into()),
            (1, 20., 0., "".into()),
            (3, 20., 0., "".into()),
        ];
        assert_eq!(worksheet_region_order(&rows).unwrap(), vec![1, 2, 0]);
        assert!(worksheet_region_order(&[(1, f64::NAN, 0., "".into())]).is_err());
        assert!(worksheet_region_order(&[(1, 0., 0., "".into()), (1, 1., 1., "".into())]).is_err());
    }
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
    fn variable(name: &str) -> PortableTree {
        tree(
            NodeKind::AnnotatedIdentifier {
                name: name.into(),
                labels: "VARIABLE".into(),
            },
            vec![],
        )
    }

    #[test]
    fn program_calculus_and_symbolic_trees_roundtrip_to_executable_mathml() {
        let program = tree(
            NodeKind::Program { statement_count: 2 },
            vec![
                tree(NodeKind::LocalDefine, vec![variable("x"), number("2")]),
                tree(
                    NodeKind::Return,
                    vec![binary(BinaryOp::Add, variable("x"), number("3"))],
                ),
            ],
        );
        let program_xml = tree_to_mathml(&program).unwrap();
        assert_eq!(tree_from_mathml(&program_xml).unwrap(), program);
        let parsed_program = crate::program::from_mathml(&program_xml).unwrap();
        let value = crate::program::execute(&parsed_program, &[], 100).unwrap();
        assert!((value.as_quantity().unwrap().value.re - 5.0).abs() < 1e-12);

        for (calculus, expected) in [
            (
                tree(
                    NodeKind::Integral,
                    vec![variable("x"), variable("x"), number("0"), number("1")],
                ),
                0.5,
            ),
            (
                tree(
                    NodeKind::Summation,
                    vec![variable("x"), variable("x"), number("1"), number("3")],
                ),
                6.0,
            ),
            (
                tree(
                    NodeKind::Product,
                    vec![variable("x"), variable("x"), number("1"), number("3")],
                ),
                6.0,
            ),
        ] {
            let xml = tree_to_mathml(&calculus).unwrap();
            assert_eq!(tree_from_mathml(&xml).unwrap(), calculus);
            let evaluation = binary(
                BinaryOp::Evaluate,
                calculus,
                tree(NodeKind::Placeholder, vec![]),
            );
            let worksheet =
                worksheet_xml(&[(1, 0.0, 0.0, tree_to_mathml(&evaluation).unwrap())]).unwrap();
            let evaluated = crate::evaluate_worksheet(&worksheet).unwrap();
            let quantity = evaluated[0].value.as_ref().unwrap().as_quantity().unwrap();
            assert!((quantity.value.re - expected).abs() < 1e-8);
        }

        let derivative = tree(
            NodeKind::Derivative { partial: false },
            vec![
                tree(NodeKind::Power, vec![variable("x"), number("2")]),
                variable("x"),
                tree(NodeKind::Placeholder, vec![]),
            ],
        );
        let derivative_xml = tree_to_mathml(&derivative).unwrap();
        assert_eq!(tree_from_mathml(&derivative_xml).unwrap(), derivative);
        let derivative_eval = binary(
            BinaryOp::Evaluate,
            derivative,
            tree(NodeKind::Placeholder, vec![]),
        );
        let derivative_worksheet = worksheet_xml(&[
            (
                1,
                0.0,
                0.0,
                tree_to_mathml(&binary(BinaryOp::Define, variable("x"), number("3"))).unwrap(),
            ),
            (2, 0.0, 20.0, tree_to_mathml(&derivative_eval).unwrap()),
        ])
        .unwrap();
        let derivative_values = crate::evaluate_worksheet(&derivative_worksheet).unwrap();
        let derivative_quantity = derivative_values[1]
            .value
            .as_ref()
            .unwrap()
            .as_quantity()
            .unwrap();
        assert!((derivative_quantity.value.re - 6.0).abs() < 1e-6);

        let symbolic = tree(
            NodeKind::SymbolicEvaluation { command_count: 1 },
            vec![
                binary(BinaryOp::Add, variable("x"), number("0")),
                tree(
                    NodeKind::SymbolicCommand { argument_count: 0 },
                    vec![id("simplify")],
                ),
            ],
        );
        let symbolic_xml = tree_to_mathml(&symbolic).unwrap();
        assert_eq!(tree_from_mathml(&symbolic_xml).unwrap(), symbolic);
        let symbolic_result = crate::symbolics::evaluate_mathml(&symbolic_xml);
        assert!(symbolic_result.is_ok(), "{symbolic_result:?}");
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
    fn label_overlay_shadows_global_names_without_cloning_or_mutating_them() {
        let mut definitions = HashMap::from([
            ("m".to_owned(), "UNIT".to_owned()),
            ("s".to_owned(), "CONSTANT".to_owned()),
            ("sin".to_owned(), "VARIABLE".to_owned()),
        ]);
        let xml = "<ml:define><ml:function><ml:id>f</ml:id><ml:boundVars><ml:id>m</ml:id></ml:boundVars></ml:function><ml:apply><ml:plus/><ml:id>m</ml:id><ml:id>s</ml:id></ml:apply></ml:define>";
        let output = label_fragment(xml, &mut definitions).unwrap();
        let wrapped = format!("<root xmlns:ml=\"{ML}\">{output}</root>");
        let doc = roxmltree::Document::parse(&wrapped).unwrap();
        for node in doc.descendants().filter(|n| math(*n, "id")) {
            if node.text() == Some("m") {
                assert_eq!(node.attribute("labels"), Some("VARIABLE"));
            }
            if node.text() == Some("s") {
                assert_eq!(node.attribute("labels"), Some("CONSTANT"));
            }
        }
        assert_eq!(definitions["m"], "UNIT");
        assert_eq!(definitions["s"], "CONSTANT");
        let call=label_fragment("<ml:eval><ml:apply><ml:id>sin</ml:id><ml:real>1</ml:real></ml:apply><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>",&mut definitions).unwrap();
        assert!(call.contains("labels=\"VARIABLE\""));
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
#[cfg(test)]
#[path = "editor_bridge/string_literal_tests.rs"]
mod string_literal_tests;

fn write_source_matrix_display(
    output: &mut String,
    display: Option<mathfolio_editor::SourceMatrixDisplay>,
) {
    use std::fmt::Write as _;
    let Some(display) = display else {
        return;
    };
    output.push_str("<ml:display");
    if let Some([row, col]) = display.offset() {
        write!(output, " offset=\"{row},{col}\"").unwrap();
    }
    if let Some([rows, cols]) = display.size() {
        write!(output, " size=\"{rows},{cols}\"").unwrap();
    }
    output.push_str("/>");
}
#[cfg(test)]
#[path = "editor_bridge/source_matrix_display_tests.rs"]
mod source_matrix_display_tests;
