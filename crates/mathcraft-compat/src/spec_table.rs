//! Calculation-only table columns. Original source/padding stays document-owned.
//! Static contracts and bounded normalization: docs/analysis/0054-spec-table-calculation.md.
use super::*;
use mathcraft_editor::{BinaryOp, NodeKind, PortableTree};

#[derive(Clone, Debug)]
pub(super) struct SpecTable {
    columns: Vec<Result<Statement>>,
}

fn namespace_context(node: Node<'_, '_>, source: &str) -> String {
    let namespaces = node
        .namespaces()
        .map(|namespace| {
            let name = namespace
                .name()
                .map(|name| format!(":{name}"))
                .unwrap_or_default();
            format!(" xmlns{name}=\"{}\"", escaped(namespace.uri()))
        })
        .collect::<String>();
    format!("<context{namespaces}>{}</context>", &source[node.range()])
}

fn escaped(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('"', "&quot;")
}

fn tree_of_column(math: Node<'_, '_>, source: &str) -> Result<PortableTree> {
    check_attributes(math, &["resultRef"])?;
    let expressions = children(math);
    if expressions.iter().any(|node| {
        node.tag_name().namespace() != Some(ML)
            && !node.has_tag_name((WS, "resultFormat"))
            && !node.has_tag_name((WS, "formattingOverride"))
    }) {
        return Err(Fault::unsupported("Unknown table math sibling"));
    }
    let expressions: Vec<_> = expressions
        .iter()
        .copied()
        .filter(|n| n.tag_name().namespace() == Some(ML))
        .collect();
    let [definition] = expressions.as_slice() else {
        return Err(Fault::unsupported("Table column requires one definition"));
    };
    if !is_math(*definition, "define") {
        return Err(Fault::unsupported(
            "Table columns support normal definitions only",
        ));
    }
    let operands = children(*definition);
    if operands.len() == 2 && is_math(operands[0], "placeholder") {
        check_attributes(*definition, &[])?;
        let header = editor_bridge::tree_from_mathml(&namespace_context(operands[0], source))
            .map_err(Fault::unsupported)?;
        let data = editor_bridge::tree_from_mathml(&namespace_context(operands[1], source))
            .map_err(Fault::unsupported)?;
        return Ok(PortableTree {
            kind: NodeKind::Binary(BinaryOp::Define),
            children: vec![header, data],
        });
    }
    editor_bridge::tree_from_mathml(&namespace_context(*definition, source))
        .map_err(Fault::unsupported)
}

fn label_column(
    mut tree: PortableTree,
    definitions: &mut HashMap<String, String>,
    normalize: bool,
) -> std::result::Result<String, String> {
    if !matches!(
        tree.kind,
        NodeKind::Binary(mathcraft_editor::BinaryOp::Define)
    ) || tree.children.len() != 2
    {
        return Err("Table column is not a definition".into());
    }
    if matches!(tree.children[0].kind, NodeKind::Function { .. }) {
        return Err("Table headers must be identifiers".into());
    }
    let body = &mut tree.children[1];
    if !matches!(body.kind, NodeKind::Binary(BinaryOp::Scale)) || body.children.len() != 2 {
        return Err("Table column requires data and a scaling cell".into());
    }
    // Reuse the proven unit-slot constraints without turning an explicitly
    // labelled VARIABLE into UNIT or changing the original document tree.
    let unit_query = PortableTree {
        kind: NodeKind::Binary(BinaryOp::Evaluate),
        children: vec![
            PortableTree {
                kind: NodeKind::Number("1".into()),
                children: vec![],
            },
            body.children[1].clone(),
        ],
    };
    let unit_xml = editor_bridge::tree_to_mathml(&unit_query)?;
    let unit_xml = editor_bridge::label_fragment(&unit_xml, &mut definitions.clone())?;
    let unit_query = editor_bridge::tree_from_mathml(&unit_xml)?;
    body.children[1] = unit_query.children[1].clone();
    if normalize {
        let data = &mut body.children[0];
        if matches!(
            data.kind,
            NodeKind::Matrix {
                display: Some(_),
                ..
            }
        ) {
            return Err("Source matrix display cannot be normalized as table data".into());
        }
        let NodeKind::Matrix { rows, cols: 1, .. } = data.kind else {
            return Err("Table data must be a nonempty column matrix".into());
        };
        if rows == 0 || data.children.len() != rows {
            return Err("Table column matrix dimensions do not match its cells".into());
        }
        // Empty padding is not an entered zero. Keep one empty cell so an
        // entirely unfilled named column fails instead of defining a value.
        while data.children.len() > 1
            && data
                .children
                .last()
                .is_some_and(|cell| matches!(cell.kind, NodeKind::Placeholder))
        {
            data.children.pop();
        }
        data.kind = NodeKind::Matrix {
            rows: data.children.len(),
            cols: 1,
            display: None,
        };
        if data.children.len() == 1 {
            *data = data.children.remove(0);
        }
        if matches!(body.children[1].kind, NodeKind::Placeholder) {
            *body = body.children.remove(0);
        }
    }
    let xml = editor_bridge::tree_to_mathml(&tree)?;
    editor_bridge::label_fragment(&xml, definitions)
}

pub(super) fn label_fragment(
    table: Node<'_, '_>,
    source: &str,
    definitions: &mut HashMap<String, String>,
) -> std::result::Result<String, String> {
    check_attributes(table, &["disable-calc"]).map_err(|fault| fault.detail)?;
    let disabled = table.attribute("disable-calc");
    if disabled.is_some_and(|value| !matches!(value, "true" | "false" | "1" | "0")) {
        return Err("Invalid table disable-calc flag".into());
    }
    let mut xml = format!("<spec-table xmlns=\"{WS}\" xmlns:ml=\"{ML}\"");
    if let Some(disabled) = disabled {
        xml.push_str(&format!(" disable-calc=\"{disabled}\""));
    }
    xml.push('>');
    let columns = children(table);
    if columns.is_empty()
        || columns.len() > 256
        || columns
            .iter()
            .any(|column| !column.has_tag_name((WS, "math")))
    {
        return Err("Invalid table column count/type".into());
    }
    let mut inactive_definitions = definitions.clone();
    let definitions = if matches!(disabled, Some("true" | "1")) {
        &mut inactive_definitions
    } else {
        definitions
    };
    for math in columns {
        let tree = tree_of_column(math, source).map_err(|fault| fault.detail)?;
        // A newly inserted or cleared native column has an unnamed header.
        // Keep that incomplete statement local to the table evaluator; failing
        // label resolution here would prevent unrelated worksheet calculation.
        // It contributes no declaration to subsequent contextual name lookup.
        let definition = if tree
            .children
            .first()
            .is_some_and(|header| matches!(header.kind, NodeKind::Placeholder))
        {
            format!(
                "<ml:define><ml:placeholder/>{}</ml:define>",
                editor_bridge::tree_to_mathml(&tree.children[1])?
            )
        } else {
            label_column(tree, definitions, false)?
        };
        xml.push_str("<math");
        if let Some(reference) = math.attribute("resultRef") {
            xml.push_str(&format!(" resultRef=\"{}\"", escaped(reference)));
        }
        xml.push('>');
        xml.push_str(&definition);
        xml.push_str("</math>");
    }
    xml.push_str("</spec-table>");
    Ok(xml)
}

pub(super) fn parse(
    table: Node<'_, '_>,
    source: &str,
    definitions: &mut HashMap<String, String>,
) -> Result<SpecTable> {
    check_attributes(table, &["disable-calc"])?;
    if table
        .attribute("disable-calc")
        .is_some_and(|value| !matches!(value, "true" | "false" | "1" | "0"))
    {
        return Err(Fault::unsupported("Invalid table disable-calc flag"));
    }
    let columns = children(table);
    if columns.is_empty() || columns.len() > 256 {
        return Err(Fault::unsupported("Invalid table column count"));
    }
    let columns = columns
        .into_iter()
        .map(|math| {
            if !math.has_tag_name((WS, "math")) {
                return Err(Fault::unsupported("Unknown table child"));
            }
            let tree = tree_of_column(math, source)?;
            let xml = label_column(tree, definitions, true).map_err(Fault::unsupported)?;
            let wrapped = format!("<context xmlns:ml=\"{ML}\">{xml}</context>");
            let document = roxmltree::Document::parse(&wrapped)
                .map_err(|error| Fault::unsupported(error.to_string()))?;
            let root = document
                .root_element()
                .children()
                .find(Node::is_element)
                .ok_or_else(|| Fault::unsupported("Empty table definition"))?;
            parse_statement(root)
        })
        .collect();
    Ok(SpecTable { columns })
}

impl SpecTable {
    pub(super) fn binding_names(&self) -> Vec<Symbol> {
        self.columns
            .iter()
            .flat_map(statement_binding_names)
            .collect()
    }

    pub(super) fn evaluate(
        &self,
        row: &mut RegionEvaluation,
        scope: &mut Scope,
        global_symbols: &std::collections::HashSet<Symbol>,
    ) {
        for (index, statement) in self.columns.iter().enumerate() {
            if let Err(fault) = check_cancelled() {
                row.error = Some(fault);
                return;
            }
            let mut column = row.clone();
            clear_result(&mut column);
            let mut input = (column, statement.clone());
            *scope = evaluate_local(
                std::slice::from_mut(&mut input),
                std::mem::take(scope),
                global_symbols,
            );
            let column = input.0;
            row.redefinition_warning |= column.redefinition_warning;
            if let Some(mut fault) = column.error {
                if fault.is_cancelled() {
                    row.error = Some(fault);
                    return;
                }
                if row.error.is_none() {
                    fault.detail = format!("Table column {}: {}", index + 1, fault.detail);
                    row.error = Some(fault);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancellation_during_later_column_keeps_entire_table_pending_then_resumes() {
        let source = format!(
            "<spec-table xmlns=\"{WS}\" xmlns:ml=\"{ML}\"><math><ml:define><ml:id>a</ml:id><ml:apply><ml:scale/><ml:matrix rows=\"1\" cols=\"1\"><ml:real>42</ml:real></ml:matrix><ml:placeholder/></ml:apply></ml:define></math><math><ml:define><ml:id>b</ml:id><ml:apply><ml:scale/><ml:matrix rows=\"1\" cols=\"1\"><ml:program><ml:for><ml:id>i</ml:id><ml:range><ml:real>1</ml:real><ml:real>2000</ml:real></ml:range><ml:program><ml:real>1</ml:real></ml:program></ml:for><ml:real>7</ml:real></ml:program></ml:matrix><ml:placeholder/></ml:apply></ml:define></math></spec-table>"
        );
        let query = "<ml:eval><ml:id>a</ml:id><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>";
        let xml = editor_bridge::worksheet_xml(&[(1, 0., 0., source), (2, 0., 40., query.into())])
            .unwrap();
        let mut calculator = WorksheetCalculator::default();
        calculator.update(&xml, None).unwrap();
        let control = CalculationControl::stop_after_checks(40);
        let stopped = calculator
            .calculate_with_control(CalculationRequest::Worksheet, &control)
            .unwrap();
        assert!(stopped.cancelled);
        assert_eq!(stopped.pending_region_ids, ["1", "2"]);
        assert!(stopped.executed_region_ids.is_empty());
        assert!(stopped.rows[0].error.is_none());
        let resumed = calculator.calculate(CalculationRequest::Worksheet).unwrap();
        assert!(
            resumed.rows.iter().all(|row| row.error.is_none()),
            "{resumed:?}"
        );
        assert_eq!(resumed.executed_region_ids, ["1", "2"]);
        let Some(Value::Scalar { quantity }) = resumed.rows[1].value else {
            panic!("{resumed:?}")
        };
        assert_eq!(quantity.value.re, 42.);
    }
    #[test]
    fn source_display_table_normalization_rejects_without_consuming_cells() {
        let xml = format!(
            "<ml:define xmlns:ml=\"{ML}\"><ml:id labels=\"VARIABLE\">a</ml:id><ml:apply><ml:scale/><ml:matrix rows=\"2\" cols=\"1\"><ml:display size=\"1,1\"/><ml:real>7</ml:real><ml:placeholder/></ml:matrix><ml:placeholder/></ml:apply></ml:define>"
        );
        let source = editor_bridge::tree_from_mathml(&xml).unwrap();
        let before = source.clone();
        let error = label_column(source.clone(), &mut HashMap::new(), true).unwrap_err();
        assert!(error.contains("Source matrix display cannot be normalized"));
        assert_eq!(source, before);
        let unchanged = label_column(source, &mut HashMap::new(), false).unwrap();
        assert!(unchanged.contains("<ml:display size=\"1,1\"/>"));
        assert!(unchanged.contains("<ml:real>7</ml:real><ml:placeholder/>"));
    }
}
