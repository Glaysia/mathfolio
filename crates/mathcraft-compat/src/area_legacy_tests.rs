//! Frozen r132 parse oracle; keep independent of the new Area collector.
use super::*;
fn legacy_parse_worksheet(
    xml: &str,
    grid: Option<PlacementGrid>,
) -> std::result::Result<(Vec<WorksheetInput>, Vec<String>), String> {
    if xml.len() > 8 * 1024 * 1024 {
        return Err("Worksheet XML exceeds 8 MiB".into());
    }
    let doc = roxmltree::Document::parse(xml).map_err(|e| e.to_string())?;
    if !doc.root_element().has_tag_name((WS, "worksheet")) {
        return Err("Expected worksheet50 root".into());
    }
    let regions = doc
        .root_element()
        .children()
        .find(|n| n.has_tag_name((WS, "regions")))
        .ok_or("Missing regions")?;
    let mut inputs = Vec::new();
    let mut table_labels = HashMap::new();
    for region in regions.children().filter(Node::is_element) {
        let content = children(region);
        if content.len() == 1
            && content[0].has_tag_name((WS, "spec-table"))
            && matches!(content[0].attribute("disable-calc"), Some("true" | "1"))
        {
            continue;
        }
        if !content.is_empty()
            && content
                .iter()
                .all(|n| n.has_tag_name((WS, "text")) || n.has_tag_name((WS, "image")))
        {
            continue;
        }
        if inputs.len() >= 10000 {
            return Err("Region budget exceeded".into());
        }
        let coordinate = |name| -> std::result::Result<f64, String> {
            let value: f64 = region
                .attribute(name)
                .ok_or_else(|| format!("Missing {name}"))?
                .parse()
                .map_err(|_| format!("Invalid {name}"))?;
            if !value.is_finite() {
                return Err(format!("Nonfinite {name}"));
            }
            if let Some(grid) = grid {
                grid.snap(value)
            } else {
                Ok(value)
            }
        };
        let math = region.children().find(|n| n.has_tag_name((WS, "math")));
        let statement = if let Some(math) = math {
            let all = children(math);
            let c: Vec<_> = all
                .iter()
                .copied()
                .filter(|n| n.tag_name().namespace() == Some(ML))
                .collect();
            if math
                .attributes()
                .any(|a| a.namespace().is_some() || a.name() != "resultRef")
            {
                Err(Fault::unsupported("Math region execution metadata"))
            } else if all.iter().any(|n| {
                n.tag_name().namespace() != Some(ML)
                    && !n.has_tag_name((WS, "resultFormat"))
                    && !n.has_tag_name((WS, "formattingOverride"))
            }) {
                Err(Fault::unsupported("Unknown math region sibling"))
            } else if c.len() == 1 {
                parse_statement(c[0])
            } else {
                Err(Fault::unsupported("Math region has multiple roots"))
            }
        } else if content.len() == 1 && content[0].has_tag_name((WS, "solveblock")) {
            solve::parse_block(content[0], grid).map(Statement::SolveBlock)
        } else if content.len() == 1 && content[0].has_tag_name((WS, "spec-table")) {
            spec_table::parse(content[0], xml, &mut table_labels).map(Statement::SpecTable)
        } else {
            Err(Fault::unsupported(
                "Non-math/nested region requires document model",
            ))
        };
        for symbol in statement_binding_names(&statement) {
            table_labels.insert(symbol.name, symbol.label);
        }
        inputs.push((
            RegionEvaluation {
                region_id: region.attribute("region-id").unwrap_or("").into(),
                result_ref: math
                    .and_then(|n| n.attribute("resultRef"))
                    .map(str::to_string)
                    .or_else(|| match &statement {
                        Ok(Statement::SolveBlock(block)) => block.solver_result_ref(),
                        _ => None,
                    }),
                left: coordinate("left")?,
                top: coordinate("top")?,
                value: None,
                display_value: None,
                error: None,
                redefinition_warning: false,
                matrix_origin: None,
            },
            statement,
            xml[region.range()].to_owned(),
        ));
    }
    // Exact overlap uses stable input order as an explicit independent policy.
    // Original tie behavior is not established beyond the one retained probe.
    inputs.sort_by(|a, b| {
        a.0.top
            .total_cmp(&b.0.top)
            .then(a.0.left.total_cmp(&b.0.left))
    });
    let (inputs, signatures) = inputs
        .into_iter()
        .map(|(row, statement, source)| ((row, statement), source))
        .unzip();
    Ok((inputs, signatures))
}

fn compare(xml: &str, grid: Option<PlacementGrid>) {
    let expected = legacy_parse_worksheet(xml, grid);
    let actual = parse_worksheet(xml, grid);
    match (expected, actual) {
        (Err(a), Err(b)) => assert_eq!(a, b),
        (Ok((old, signatures)), Ok((new, new_signatures, positions))) => {
            assert_eq!(signatures, new_signatures);
            assert_eq!(format!("{old:?}"), format!("{new:?}"));
            assert_eq!(
                positions,
                new.iter().map(|(r, _)| (r.left, r.top)).collect::<Vec<_>>()
            );
            let mut legacy = WorksheetCalculator {
                pending: vec![true; old.len()],
                scopes: vec![None; old.len()],
                inputs: old,
                ..Default::default()
            };
            let expected = legacy.calculate(CalculationRequest::Worksheet).unwrap();
            let mut modern = WorksheetCalculator::default();
            modern.update(xml, grid).unwrap();
            let actual = modern.calculate(CalculationRequest::Worksheet).unwrap();
            assert_eq!(
                serde_json::to_value(expected.rows).unwrap(),
                serde_json::to_value(actual.rows).unwrap()
            );
            assert_eq!(expected.executed_region_ids, actual.executed_region_ids);
        }
        (a, b) => panic!("Legacy/current outcome differs: {a:?} / {b:?}"),
    }
}
fn sheet(body: &str) -> String {
    format!("<worksheet xmlns='{WS}' xmlns:ml='{ML}'><regions>{body}</regions></worksheet>")
}
#[test]
fn non_area_source_order_table_labels_and_exact_legacy_rows_errors_are_preserved() {
    let table = "<region region-id='table' left='0' top='10'><spec-table><math><ml:define><ml:id>a</ml:id><ml:apply><ml:scale/><ml:matrix rows='1' cols='1'><ml:real>7</ml:real></ml:matrix><ml:id>m</ml:id></ml:apply></ml:define></math></spec-table></region>";
    let definition = "<region region-id='definition' left='0' top='0'><math><ml:define><ml:id labels='VARIABLE'>m</ml:id><ml:real>3</ml:real></ml:define></math></region>";
    let query = "<region region-id='query' left='0' top='20'><math resultRef='saved'><ml:eval><ml:id>a</ml:id><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval><resultFormat/></math></region>";
    for xml in [
        sheet(&(table.to_owned() + definition + query)),
        sheet(&(definition.to_owned() + table + query)),
        sheet(
            "<region region-id='u' left='-0' top='0'><unknown/></region><region region-id='v' left='0' top='0'><math future='x'><ml:real>4</ml:real></math></region>",
        ),
        sheet(
            "<region><text/></region><region><image/></region><region><spec-table disable-calc='1'/></region>",
        ),
        sheet("<region left='0' top='NaN'><math><ml:real>7</ml:real></math></region>"),
        "<wrong/>".into(),
        format!("<worksheet xmlns='{WS}'/>"),
    ] {
        compare(&xml, None);
    }
}
#[test]
fn non_area_coordinate_and_budget_error_precedence_matches_frozen_parser() {
    let region = "<region left='0' top='0'><unknown/></region>";
    let mut body = region.repeat(10000);
    body.push_str("<region left='NaN'><unknown/></region>");
    compare(&sheet(&body), None);
    compare(&sheet(&region.replace("left='0'", "left='invalid'")), None);
    compare(&"x".repeat(8 * 1024 * 1024 + 1), None);
    let grid=PlacementGrid::from_presentation("<p:presentation xmlns:p='http://schemas.ptc.com/mathcad/settings/presentation10'><p:pageModel paper-code='Letter' grid-size='Fine'/></p:presentation>").unwrap();
    compare(&sheet(&region.replace("left='0'", "left='-1'")), Some(grid));
    compare(
        &sheet(&(region.to_owned() + &region.replace("top='0'", "top='9.7'"))),
        Some(grid),
    );
}
