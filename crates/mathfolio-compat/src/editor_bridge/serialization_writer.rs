//! A single output buffer for the proven plain subset. The original serializer
//! remains the fallback and independent byte/error oracle.
use super::*;
use std::fmt::Write;

fn escaped(output: &mut String, text: &str) {
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
        output.push_str(&text[start..index]);
        output.push_str(replacement);
        start = index + 1;
    }
    output.push_str(&text[start..]);
}
fn open(output: &mut String, tag: &str, root: bool) {
    output.push_str("<ml:");
    output.push_str(tag);
    if root {
        output.push_str(" xmlns:ml=\"");
        output.push_str(ML);
        output.push('"');
    }
}
fn close(output: &mut String, tag: &str) {
    output.push_str("</ml:");
    output.push_str(tag);
    output.push('>');
}
fn write_node<const TRACE: bool>(
    output: &mut String,
    node: &PortableTree,
    root: bool,
    depth: usize,
    plan: Option<&typed::Plan>,
    trace: &mut typed::TraceCursor,
) -> Result<(), String> {
    if depth > 128 {
        return Err("Math tree exceeds serialization depth".into());
    }
    let kind = plan
        .and_then(|p| p.replacement.as_ref())
        .unwrap_or(&node.kind);
    let source_start = output.len();
    let mut child = |output: &mut String, i: usize| {
        if TRACE {
            trace.push(i);
        }
        let result = write_node::<TRACE>(
            output,
            &node.children[i],
            false,
            depth + 1,
            plan.and_then(|p| p.children.get(&i)),
            trace,
        );
        if TRACE {
            trace.pop();
        }
        result
    };
    match kind {
        NodeKind::Placeholder => {
            open(output, "placeholder", root);
            output.push_str("/>");
        }
        NodeKind::StringLiteral(value) => {
            open(output, "str", root);
            output.push_str(" xml:space=\"preserve\">");
            // XML normalizes literal CR. Preserve it as character data, never
            // as markup or an editor gesture.
            let mut parts = value.split('\r');
            escaped(output, parts.next().unwrap_or(""));
            for part in parts {
                output.push_str("&#13;");
                escaped(output, part);
            }
            close(output, "str");
        }
        NodeKind::Identifier(name) => {
            open(output, "id", root);
            output.push_str(" xml:space=\"preserve\">");
            escaped(output, name);
            close(output, "id");
        }
        NodeKind::AnnotatedIdentifier { name, labels } => {
            open(output, "id", root);
            output.push_str(" labels=\"");
            escaped(output, labels);
            output.push_str("\" xml:space=\"preserve\">");
            escaped(output, name);
            close(output, "id");
        }
        NodeKind::NativeIdentifier {
            name,
            labels,
            contextual,
            subscript,
        } => {
            open(output, "id", root);
            output.push_str(" labels=\"");
            escaped(output, labels);
            output.push('"');
            if let Some(value) = contextual {
                output.push_str(" label-is-contextual=\"");
                output.push_str(if *value { "true" } else { "false" });
                output.push('"');
            }
            output.push_str(" xml:space=\"preserve\">");
            if let Some(subscript) = subscript {
                output.push_str("<p:Span xmlns:p=\"");
                output.push_str(XAML);
                output.push_str("\">");
                escaped(output, name);
                output.push_str("<ptc:Subscript xmlns:ptc=\"");
                output.push_str(PTC);
                output.push_str("\">");
                escaped(output, subscript);
                output.push_str("</ptc:Subscript></p:Span>");
            } else {
                escaped(output, name);
            }
            close(output, "id");
        }
        NodeKind::BasedNumber { literal, base } => {
            let digits = base.digits(literal).ok_or("Invalid based numeric lexeme")?;
            open(output, "real", root);
            write!(output, " base=\"{}\">", base.radix()).unwrap();
            escaped(output, digits);
            close(output, "real");
        }
        NodeKind::Number(number) => {
            let value: f64 = number.parse().map_err(|_| "Invalid number literal")?;
            if !value.is_finite() {
                return Err("Nonfinite number literal".into());
            }
            open(output, "real", root);
            output.push('>');
            escaped(output, number);
            close(output, "real");
        }
        NodeKind::Imaginary(number) | NodeKind::ImaginarySymbol { value: number, .. } => {
            if !number.parse::<f64>().is_ok_and(f64::is_finite) {
                return Err("Invalid imaginary literal".into());
            }
            open(output, "imag", root);
            if let NodeKind::ImaginarySymbol { symbol, .. } = kind {
                if symbol != "i" && symbol != "j" {
                    return Err("Unsupported imaginary symbol".into());
                }
                output.push_str(" symbol=\"");
                escaped(output, symbol);
                output.push('"');
            }
            output.push('>');
            escaped(output, number);
            close(output, "imag");
        }
        NodeKind::Group => {
            open(output, "parens", root);
            output.push('>');
            child(output, 0)?;
            close(output, "parens");
        }
        NodeKind::Matrix {
            rows,
            cols,
            display,
        } => {
            open(output, "matrix", root);
            write!(output, " rows=\"{rows}\" cols=\"{cols}\">").unwrap();
            super::write_source_matrix_display(output, *display);
            for col in 0..*cols {
                for row in 0..*rows {
                    child(output, row * cols + col)?;
                }
            }
            close(output, "matrix");
        }
        NodeKind::Range { has_step } => {
            open(output, "range", root);
            output.push('>');
            if *has_step {
                output.push_str("<ml:sequence>");
                child(output, 0)?;
                child(output, 1)?;
                output.push_str("</ml:sequence>");
                child(output, 2)?;
            } else {
                child(output, 0)?;
                child(output, 1)?;
            }
            close(output, "range");
        }
        NodeKind::Binary(definition @ (BinaryOp::Define | BinaryOp::GlobalDefine)) => {
            // Eligibility guarantees a direct named target. The original path
            // still owns indexed/function and malformed declaration errors.
            if depth != 0
                || !matches!(
                    node.children[0].kind,
                    NodeKind::Identifier(_)
                        | NodeKind::AnnotatedIdentifier { .. }
                        | NodeKind::NativeIdentifier { .. }
                )
            {
                if TRACE {
                    trace.disable();
                }
                output.push_str(&serialize_with_plan(node, root, depth, plan)?);
                return Ok(());
            }
            let tag = if *definition == BinaryOp::GlobalDefine {
                "globalDefine"
            } else {
                "define"
            };
            open(output, tag, root);
            output.push('>');
            child(output, 0)?;
            child(output, 1)?;
            close(output, tag);
        }
        NodeKind::Binary(BinaryOp::Evaluate) => {
            if depth > 1 {
                return Err("Nested evaluation needs raw preservation".into());
            }
            open(output, "eval", root);
            output.push('>');
            child(output, 0)?;
            output.push_str("<ml:unitOverride>");
            child(output, 1)?;
            output.push_str("</ml:unitOverride>");
            close(output, "eval");
        }
        // Call and Index serialize arguments before callee/base in the old
        // implementation. Retain that fallible ordering through its exact
        // subtree fallback rather than adding a reordered validation pass.
        NodeKind::Call { .. } | NodeKind::Index { .. } => {
            if TRACE {
                trace.disable();
            }
            output.push_str(&serialize_with_plan(node, root, depth, plan)?)
        }
        kind => {
            let (operator, arity) = match kind {
                NodeKind::UnaryNeg => ("neg", 1),
                NodeKind::LogicalNot => ("not", 1),
                NodeKind::Absolute => ("absval", 1),
                NodeKind::Factorial => ("factorial", 1),
                NodeKind::Percent => ("percent", 1),
                NodeKind::Degree => ("degree_op", 1),
                NodeKind::Transpose => ("transpose", 1),
                NodeKind::Conjugate => ("conjugate", 1),
                NodeKind::Vectorize => ("vectorize", 1),
                NodeKind::Determinant => ("determinant", 1),
                NodeKind::Row => ("matrow", 2),
                NodeKind::Column => ("matcol", 2),
                NodeKind::Root => ("nthRoot", 2),
                NodeKind::Binary(BinaryOp::Add) => ("plus", 2),
                NodeKind::Binary(BinaryOp::Subtract) => ("minus", 2),
                NodeKind::Binary(BinaryOp::Multiply) => ("mult", 2),
                NodeKind::Binary(BinaryOp::CrossProduct) => ("crossProduct", 2),
                NodeKind::Binary(BinaryOp::Polar) => ("polar", 2),
                NodeKind::Binary(BinaryOp::Scale) => ("scale", 2),
                NodeKind::Binary(BinaryOp::InlineDivide) => ("div inline=\"true\"", 2),
                NodeKind::Binary(BinaryOp::Equal) => ("equal", 2),
                NodeKind::Binary(BinaryOp::NotEqual) => ("notEqual", 2),
                NodeKind::Binary(BinaryOp::LessThan) => ("lessThan", 2),
                NodeKind::Binary(BinaryOp::LessOrEqual) => ("lessOrEqual", 2),
                NodeKind::Binary(BinaryOp::GreaterThan) => ("greaterThan", 2),
                NodeKind::Binary(BinaryOp::GreaterOrEqual) => ("greaterOrEqual", 2),
                NodeKind::Binary(BinaryOp::And) => ("and", 2),
                NodeKind::Binary(BinaryOp::Or) => ("or", 2),
                NodeKind::Binary(BinaryOp::Xor) => ("xor", 2),
                NodeKind::Fraction => ("div", 2),
                NodeKind::Power => ("pow", 2),
                _ => {
                    if TRACE {
                        trace.disable();
                    }
                    output.push_str(&serialize_with_plan(node, root, depth, plan)?);
                    return Ok(());
                }
            };
            open(output, "apply", root);
            output.push_str("><ml:");
            output.push_str(operator);
            output.push_str("/>");
            for index in 0..arity {
                child(output, index)?;
            }
            close(output, "apply");
        }
    }
    if TRACE
        && matches!(
            kind,
            NodeKind::Identifier(_)
                | NodeKind::AnnotatedIdentifier { .. }
                | NodeKind::NativeIdentifier { .. }
        )
    {
        trace.record(source_start..output.len());
    }
    Ok(())
}
pub(super) fn serialize(
    node: &PortableTree,
    root: bool,
    depth: usize,
    plan: Option<&typed::Plan>,
) -> Result<String, String> {
    let mut output = String::new();
    write_node::<false>(
        &mut output,
        node,
        root,
        depth,
        plan,
        &mut typed::TraceCursor::disabled(),
    )?;
    // Keep retained proof capacity equal to its exact bytes, rather than
    // retaining geometric growth slack for every worksheet region.
    Ok(output.into_boxed_str().into_string())
}

pub(super) fn serialize_traced(
    node: &PortableTree,
    budget: usize,
) -> Result<(String, Option<typed::IdentifierSpans>), String> {
    let mut output = String::new();
    let mut trace = typed::TraceCursor::new(budget);
    write_node::<true>(&mut output, node, true, 0, None, &mut trace)?;
    Ok((output.into_boxed_str().into_string(), trace.finish()))
}

pub(super) fn append_identifier(
    output: &mut String,
    node: &PortableTree,
    plan: &typed::Plan,
    depth: usize,
) -> Result<(), String> {
    debug_assert!(matches!(
        node.kind,
        NodeKind::Identifier(_)
            | NodeKind::AnnotatedIdentifier { .. }
            | NodeKind::NativeIdentifier { .. }
    ));
    write_node::<false>(
        output,
        node,
        false,
        depth,
        Some(plan),
        &mut typed::TraceCursor::disabled(),
    )
}
