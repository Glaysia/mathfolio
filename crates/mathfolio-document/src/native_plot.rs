//! Bounded native XY source XML, independently implemented from the installed
//! writer/reader contract in analysis/0146. This never decodes cached curves.
use crate::canvas::{
    PlotAxisSpec, PlotEquationRef, PlotExpression, PlotExpressionRef, PlotGraphSize,
    PlotLegendPosition, PlotLineStyle, PlotNativeGeometry, PlotScaling, PlotSource, PlotSpec,
    PlotTraceAppearance, PlotTraceStyle,
};
use mathfolio_compat::editor_bridge;
use mathfolio_core::Expr;
use mathfolio_editor::{BinaryOp, NodeKind, PortableTree};
use roxmltree::{Document, Node};

const WS: &str = "http://schemas.mathsoft.com/worksheet50";
const ML: &str = "http://schemas.mathsoft.com/math50";
const MAX_BYTES: usize = 256 * 1024;
const MAX_NODES: u32 = 8192;

// Existing Mathfolio renderer policy (plots.rs): graph body = region height
// minus this UI allowance, minimum 45 DIP. Neither number is a Prime constant.
const UI_HEIGHT_ALLOWANCE: f32 = 50.;
const MIN_GRAPH_HEIGHT: f32 = 45.;

#[derive(Clone, Debug, PartialEq)]
pub struct ParsedPlot {
    pub spec: PlotSpec,
    pub trace_result_refs: Vec<Option<i32>>,
}

fn graph_height(region_height: f32) -> Result<f32, String> {
    let graph = (region_height - UI_HEIGHT_ALLOWANCE).max(MIN_GRAPH_HEIGHT);
    if !graph.is_finite() || (graph + UI_HEIGHT_ALLOWANCE).to_bits() != region_height.to_bits() {
        return Err("Native XY graph/region height cannot roundtrip exactly".into());
    }
    Ok(graph)
}

fn region_height(graph: f32) -> Result<f32, String> {
    let region = graph + UI_HEIGHT_ALLOWANCE;
    if !region.is_finite() || graph_height(region)?.to_bits() != graph.to_bits() {
        return Err(
            "Native XY graph height exceeds exact UI representation; retain raw source".into(),
        );
    }
    Ok(region)
}

fn tree(kind: NodeKind, children: Vec<PortableTree>) -> PortableTree {
    PortableTree { kind, children }
}

fn expression_tree(expr: &Expr) -> Result<PortableTree, String> {
    Ok(match expr {
        Expr::Number(value) => tree(NodeKind::Number(value.to_string()), vec![]),
        Expr::Name(name) => tree(NodeKind::Identifier(name.clone()), vec![]),
        Expr::Neg(child) => tree(NodeKind::UnaryNeg, vec![expression_tree(child)?]),
        Expr::Binary(op, left, right) => tree(
            match op {
                '+' => NodeKind::Binary(BinaryOp::Add),
                '-' => NodeKind::Binary(BinaryOp::Subtract),
                '*' => NodeKind::Binary(BinaryOp::Multiply),
                '/' => NodeKind::Fraction,
                '^' => NodeKind::Power,
                _ => return Err("Unsupported native plot operator".into()),
            },
            vec![expression_tree(left)?, expression_tree(right)?],
        ),
        Expr::Call(name, args) => {
            let mut children = vec![tree(NodeKind::Identifier(name.clone()), vec![])];
            children.extend(
                args.iter()
                    .map(expression_tree)
                    .collect::<Result<Vec<_>, _>>()?,
            );
            tree(
                NodeKind::Call {
                    argument_count: args.len(),
                },
                children,
            )
        }
    })
}

fn source_tree(source: &str) -> Result<PortableTree, String> {
    if source.is_empty() {
        return Ok(tree(NodeKind::Placeholder, vec![]));
    }
    expression_tree(&mathfolio_core::parse(source)?)
}

fn expression_ref_tree(source: PlotExpressionRef<'_>) -> Result<PortableTree, String> {
    match source {
        PlotExpressionRef::LegacyText(s) => source_tree(s),
        PlotExpressionRef::Typed(t) => Ok(t.clone()),
    }
}
// Compare full native expression shape, not its projection to arithmetic text.
// ResultRefs belong to the worksheet math wrappers and are checked separately.
fn same_math_node(a: Node<'_, '_>, b: Node<'_, '_>) -> bool {
    if a.tag_name() != b.tag_name() {
        return false;
    }
    let attributes = |n: Node<'_, '_>| {
        n.attributes()
            .map(|a| {
                (
                    (a.namespace().unwrap_or("").to_owned(), a.name().to_owned()),
                    a.value().to_owned(),
                )
            })
            .collect::<std::collections::BTreeMap<_, _>>()
    };
    let mut aa = attributes(a);
    let mut bb = attributes(b);
    // xml:space=preserve is an equivalent encoding only when all actual leaf
    // text remains identical; no other unknown attribute may disappear.
    for attrs in [&mut aa, &mut bb] {
        let key = (
            "http://www.w3.org/XML/1998/namespace".into(),
            "space".into(),
        );
        if attrs.get(&key).is_some_and(|v| v == "preserve") {
            attrs.remove(&key);
        }
    }
    if aa != bb {
        return false;
    }
    fn content<'a, 'input>(node: Node<'a, 'input>) -> Vec<Node<'a, 'input>> {
        let structural = node.tag_name().namespace() == Some(ML)
            && node.tag_name().name() != "id"
            && node.children().any(|n| n.is_element());
        node.children()
            .filter(|n| {
                n.is_element()
                    || (n.is_text() && (!structural || !n.text().unwrap_or("").trim().is_empty()))
            })
            .collect::<Vec<_>>()
    }
    let ac = content(a);
    let bc = content(b);
    ac.len() == bc.len()
        && ac.into_iter().zip(bc).all(|(a, b)| {
            if a.is_element() && b.is_element() {
                same_math_node(a, b)
            } else {
                a.is_text() && b.is_text() && a.text() == b.text()
            }
        })
}
fn typed_expression_from_native(math: Node<'_, '_>) -> Result<PlotExpression, String> {
    let tree = editor_bridge::tree_from_mathml_node(math)?;
    let value = PlotExpression::from_tree(tree)?;
    let xml = editor_bridge::tree_to_mathml(value.as_tree().expect("typed constructor"))?;
    let wrapper = format!("<math xmlns=\"{WS}\" xmlns:ml=\"{ML}\">{xml}</math>");
    let document = Document::parse(&wrapper).map_err(|e| e.to_string())?;
    let original = math
        .children()
        .find(Node::is_element)
        .ok_or("Missing plot expression")?;
    let generated = document
        .root_element()
        .children()
        .find(Node::is_element)
        .ok_or("Missing generated expression")?;
    if !same_math_node(original, generated) {
        return Err("Native XY expression metadata/syntax cannot roundtrip without loss".into());
    }
    Ok(value)
}
fn line_name(line: PlotLineStyle) -> &'static str {
    match line {
        PlotLineStyle::None => "None",
        PlotLineStyle::Solid => "Solid",
        PlotLineStyle::Dotted => "Dot",
        PlotLineStyle::Dashed => "Dash",
        PlotLineStyle::DashDot => "DashDot",
        PlotLineStyle::DashDotDot => "DashDotDot",
    }
}

/// Ordered equation sources, preserving distinct equal-text equations and a
/// shorter axis's shared last equation. Never serialize expanded trace pairs.
fn source_axes(spec: &PlotSpec) -> [Vec<PlotEquationRef<'_>>; 2] {
    [
        spec.equation_sources(true).collect(),
        spec.equation_sources(false).collect(),
    ]
}

/// Serialize source only. Region identity/position/disabled state and cached
/// trace result IDs belong to the owning export transaction.
pub fn payload(spec: &PlotSpec) -> Result<String, String> {
    spec.validate()?;
    let graph = match &spec.native_geometry {
        Some(geometry) => geometry.graph_size.as_ref().map(|size| {
            format!(
                "<graph-size width=\"{}\" height=\"{}\"/>",
                size.width, size.height
            )
        }),
        None => Some(format!(
            "<graph-size width=\"{}\" height=\"{}\"/>",
            spec.width,
            graph_height(spec.height)?
        )),
    }
    .unwrap_or_default();
    if !spec.x_axis.label.is_empty()
        || !spec.y_axis.label.is_empty()
        || spec.x_axis.range.is_some()
        || spec.y_axis.range.is_some()
    {
        return Err("Native XY axis labels/ranges are not supported yet".into());
    }
    let traces = spec.paired_sources().collect::<Vec<_>>();
    let defaults = PlotTraceStyle::default();
    let mut trace_xml = String::new();
    let mut x = String::new();
    let mut y = String::new();
    for trace in &traces {
        if !trace.label.is_empty()
            || trace.style.marker != defaults.marker
            || trace.style.marker_filled != defaults.marker_filled
            || trace.style.marker_size != defaults.marker_size
            || trace.style.marker_frequency != defaults.marker_frequency
        {
            return Err("Native XY labels/marker settings need raw preservation".into());
        }
        let [r, g, b, a] = trace.style.color;
        trace_xml.push_str(&format!("<trace><traceStyle color=\"#{a:02X}{r:02X}{g:02X}{b:02X}\" symbol=\"none\" line-weight=\"{}\" line-style=\"{}\">lines</traceStyle></trace>",trace.style.line_width,line_name(trace.style.line)));
    }
    let [xs, ys] = source_axes(spec);
    for (sources, axis_xml) in [(xs, &mut x), (ys, &mut y)] {
        for source in sources {
            let expression =
                editor_bridge::tree_to_mathml(&expression_ref_tree(source.expression)?)?;
            let scaling = match source.scaling {
                Some(value) => editor_bridge::tree_to_mathml(value.as_tree())?,
                None => "<ml:placeholder/>".into(),
            };
            axis_xml.push_str(&format!(
                "<plotEquation><math>{expression}</math><math>{scaling}</math></plotEquation>"
            ));
        }
    }
    let axis = |name: &str, equations: &str, spec: &PlotAxisSpec| {
        let position = spec
            .legend_position
            .map(|p| format!(" legend-position=\"{}\"", legend_name(p)))
            .unwrap_or_default();
        let width = spec
            .legend_width
            .map(|w| format!(" legendWidth=\"{w}\""))
            .unwrap_or_default();
        format!(
            "<{name} rank=\"1\"{position}><axisLine position=\"origin\"{width}/><axisGrid><gridFrequency>11</gridFrequency><gridLabels display=\"true\"/><gridLines display=\"false\"/><tickMarks display=\"true\"/></axisGrid><plotEquations>{equations}</plotEquations><xyDomain scale-type=\"linear\" auto-scale=\"true\"><startValue><ml:placeholder/></startValue><endValue><ml:placeholder/></endValue></xyDomain></{name}>"
        )
    };
    let xml = format!(
        "<plot xmlns=\"{WS}\" xmlns:ml=\"{ML}\" show-axis-expressions=\"true\"><xyPlot><traces>{trace_xml}</traces>{graph}<axes>{}{}</axes></xyPlot></plot>",
        axis("xAxis", &x, &spec.x_axis),
        axis("yAxis", &y, &spec.y_axis)
    );
    if xml.len() > MAX_BYTES {
        return Err("Native plot XML exceeds budget".into());
    }
    // Native source is always embedded in an owning region. Reserve that
    // element in both depth and node budgets; validating only the payload
    // would admit a boundary source that its own exported package cannot read.
    checked_document(&format!("<region xmlns=\"{WS}\">{xml}</region>"))?;
    Ok(xml)
}

fn attrs(node: Node<'_, '_>, allowed: &[&str]) -> Result<(), String> {
    if node
        .attributes()
        .any(|a| a.namespace().is_some() || !allowed.contains(&a.name()))
    {
        return Err(format!(
            "Unsupported native plot attributes on {}",
            node.tag_name().name()
        ));
    }
    Ok(())
}

fn children<'a, 'input>(
    node: Node<'a, 'input>,
    names: &[&str],
) -> Result<Vec<Node<'a, 'input>>, String> {
    if node
        .children()
        .any(|c| c.is_text() && !c.text().unwrap_or("").trim().is_empty())
    {
        return Err("Unexpected native plot text".into());
    }
    let values: Vec<_> = node.children().filter(Node::is_element).collect();
    if values.len() != names.len()
        || values
            .iter()
            .zip(names)
            .any(|(n, name)| !n.has_tag_name((WS, *name)))
    {
        return Err(format!(
            "Unsupported native plot children on {}",
            node.tag_name().name()
        ));
    }
    Ok(values)
}

fn empty(node: Node<'_, '_>, allowed: &[&str]) -> Result<(), String> {
    attrs(node, allowed)?;
    children(node, &[])?;
    Ok(())
}

fn attr_eq(node: Node<'_, '_>, name: &str, expected: &str) -> Result<(), String> {
    if node.attribute(name) != Some(expected) {
        return Err(format!("Unsupported native plot {name}"));
    }
    Ok(())
}

fn placeholder(node: Node<'_, '_>) -> Result<(), String> {
    if node.has_tag_name((WS, "math")) {
        attrs(node, &["resultRef"])?;
        optional_ref(node)?;
    } else {
        attrs(node, &[])?;
    }
    if node
        .children()
        .any(|child| child.is_text() && !child.text().unwrap_or("").trim().is_empty())
    {
        return Err("Unexpected placeholder wrapper text".into());
    }
    let values: Vec<_> = node.children().filter(Node::is_element).collect();
    if values.len() != 1 || !values[0].has_tag_name((ML, "placeholder")) {
        return Err("Native XY nondefault scaling/domain needs raw preservation".into());
    }
    empty(values[0], &[])
}

fn legend_name(position: PlotLegendPosition) -> &'static str {
    match position {
        PlotLegendPosition::Bottom => "PlotBoundaryBottom",
        PlotLegendPosition::Top => "PlotBoundaryTop",
        PlotLegendPosition::Left => "PlotBoundaryLeft",
        PlotLegendPosition::Right => "PlotBoundaryRight",
    }
}
fn read_legend_position(axis: Node<'_, '_>) -> Result<Option<PlotLegendPosition>, String> {
    axis.attribute("legend-position")
        .map(|value| match (axis.tag_name().name(), value) {
            ("xAxis", "PlotBoundaryBottom") => Ok(PlotLegendPosition::Bottom),
            ("xAxis", "PlotBoundaryTop") => Ok(PlotLegendPosition::Top),
            ("yAxis", "PlotBoundaryLeft") => Ok(PlotLegendPosition::Left),
            ("yAxis", "PlotBoundaryRight") => Ok(PlotLegendPosition::Right),
            _ => Err("Native XY legend position needs raw preservation".into()),
        })
        .transpose()
}
fn read_legend_width(axis_line: Node<'_, '_>) -> Result<Option<f64>, String> {
    axis_line
        .attribute("legendWidth")
        .map(|value| {
            let width = value
                .trim_matches([' ', '\t', '\r', '\n'])
                .parse::<f64>()
                .map_err(|_| "Invalid native XY legend width")?;
            if !width.is_finite() || width < 0. {
                return Err("Native XY legend width needs raw preservation".into());
            }
            Ok(width)
        })
        .transpose()
}
type AxisSource = (PlotExpression, Option<PlotScaling>);
fn read_axis(axis: Node<'_, '_>) -> Result<(Vec<AxisSource>, PlotAxisSpec), String> {
    attrs(axis, &["rank", "legend-position"])?;
    let legend_position = read_legend_position(axis)?;
    attr_eq(axis, "rank", "1")?;
    let nodes = children(axis, &["axisLine", "axisGrid", "plotEquations", "xyDomain"])?;
    empty(nodes[0], &["position", "legendWidth"])?;
    attr_eq(nodes[0], "position", "origin")?;
    let legend_width = read_legend_width(nodes[0])?;
    attrs(nodes[1], &[])?;
    let grid = children(
        nodes[1],
        &["gridFrequency", "gridLabels", "gridLines", "tickMarks"],
    )?;
    attrs(grid[0], &[])?;
    if grid[0].children().any(|n| n.is_element()) || grid[0].text().unwrap_or("").trim() != "11" {
        return Err("Unsupported native XY grid frequency".into());
    }
    for (node, display) in grid[1..].iter().zip(["true", "false", "true"]) {
        empty(*node, &["display"])?;
        attr_eq(*node, "display", display)?;
    }
    attrs(nodes[2], &[])?;
    let count = nodes[2].children().filter(Node::is_element).count();
    if !(1..=PlotSpec::MAX_TRACES).contains(&count) {
        return Err("Native XY axis equation count exceeds supported limits".into());
    }
    let equations = children(nodes[2], &vec!["plotEquation"; count])?;
    let mut expressions = Vec::with_capacity(count);
    for equation in equations {
        attrs(equation, &[])?;
        let maths = children(equation, &["math", "math"])?;
        attrs(maths[0], &["resultRef"])?;
        optional_ref(maths[0])?;
        if maths[0].children().filter(Node::is_element).count() != 1
            || maths[0]
                .children()
                .any(|child| child.is_text() && !child.text().unwrap_or("").trim().is_empty())
        {
            return Err("Native XY expression formatting/metadata needs raw preservation".into());
        }
        if maths[0]
            .children()
            .any(|child| child.has_tag_name((ML, "placeholder")))
        {
            placeholder(maths[0])?;
        }
        let expression = typed_expression_from_native(maths[0])?;
        attrs(maths[1], &["resultRef"])?;
        optional_ref(maths[1])?;
        if maths[1].children().filter(Node::is_element).count() != 1
            || maths[1]
                .children()
                .any(|child| child.is_text() && !child.text().unwrap_or("").trim().is_empty())
        {
            return Err("Native XY scaling formatting/metadata needs raw preservation".into());
        }
        let scaling = PlotScaling::from_tree(editor_bridge::tree_from_mathml_node(maths[1])?)?;
        expressions.push((expression, scaling));
    }
    attrs(nodes[3], &["scale-type", "auto-scale"])?;
    attr_eq(nodes[3], "scale-type", "linear")?;
    attr_eq(nodes[3], "auto-scale", "true")?;
    let limits = children(nodes[3], &["startValue", "endValue"])?;
    placeholder(limits[0])?;
    placeholder(limits[1])?;
    Ok((
        expressions,
        PlotAxisSpec {
            legend_position,
            legend_width,
            ..Default::default()
        },
    ))
}

fn same_expression(left: &PortableTree, right: &PortableTree) -> bool {
    let kind_matches = match (&left.kind, &right.kind) {
        (NodeKind::Number(a), NodeKind::Number(b)) => match (a.parse::<f64>(), b.parse::<f64>()) {
            (Ok(a), Ok(b)) => a.is_finite() && b.is_finite() && a.to_bits() == b.to_bits(),
            _ => false,
        },
        (a, b) => a == b,
    };
    kind_matches
        && left.children.len() == right.children.len()
        && left
            .children
            .iter()
            .zip(&right.children)
            .all(|(a, b)| same_expression(a, b))
}

fn optional_ref(node: Node<'_, '_>) -> Result<Option<i32>, String> {
    node.attribute("resultRef")
        .map(crate::native::parse_result_id)
        .transpose()
}

fn model_number(value: &str) -> Result<f32, String> {
    let original = value
        .parse::<f64>()
        .map_err(|_| "Invalid native XY number")?;
    let model = value
        .parse::<f32>()
        .map_err(|_| "Invalid native XY model number")?;
    let exported = model
        .to_string()
        .parse::<f64>()
        .map_err(|_| "Invalid native XY exported number")?;
    if !original.is_finite() || !model.is_finite() || original.to_bits() != exported.to_bits() {
        return Err(
            "Native XY precision exceeds PlotSpec representation; retain raw source".into(),
        );
    }
    Ok(model)
}

fn checked_document(xml: &str) -> Result<Document<'_>, String> {
    if xml.len() > MAX_BYTES {
        return Err("Native plot XML exceeds budget".into());
    }
    let doc = Document::parse_with_options(
        xml,
        roxmltree::ParsingOptions {
            allow_dtd: false,
            nodes_limit: MAX_NODES,
            ..Default::default()
        },
    )
    .map_err(|e| e.to_string())?;
    if doc.descendants().any(|n| n.ancestors().count() > 64) {
        return Err("Native plot depth exceeds budget".into());
    }
    // The bounded bridge and scalar property readers consume simple text.
    // A comment/PI can split one lexical value into several text nodes; do not
    // accept a first fragment while silently dropping the remaining value.
    if doc.descendants().any(|n| n.is_comment() || n.is_pi()) {
        return Err("Native XY comments/processing instructions need raw preservation".into());
    }
    Ok(doc)
}

/// Decode the source and its inert trace references with one XML parse.
pub fn parse_with_references(xml: &str) -> Result<ParsedPlot, String> {
    let doc = checked_document(xml)?;
    let region = doc.root_element();
    if !region.has_tag_name((WS, "region")) {
        return Err("Expected owning native plot region".into());
    }
    attrs(
        region,
        &[
            "region-id",
            "left",
            "top",
            "width",
            "height",
            "actualWidth",
            "actualHeight",
        ],
    )?;
    let plot = children(region, &["plot"])?[0];
    attrs(plot, &["show-axis-expressions", "disable-calc"])?;
    if plot
        .attribute("show-axis-expressions")
        .is_some_and(|v| v != "true")
    {
        return Err("Hidden XY source expressions need raw preservation".into());
    }
    if plot
        .attribute("disable-calc")
        .is_some_and(|v| !matches!(v, "true" | "false" | "1" | "0"))
    {
        return Err("Invalid native XY disabled flag".into());
    }
    let xy = children(plot, &["xyPlot"])?[0];
    attrs(xy, &[])?;
    let elements: Vec<_> = xy.children().filter(Node::is_element).collect();
    let has_graph = elements
        .get(1)
        .is_some_and(|n| n.has_tag_name((WS, "graph-size")));
    let nodes = if has_graph {
        children(xy, &["traces", "graph-size", "axes"])?
    } else {
        children(xy, &["traces", "axes"])?
    };
    attrs(nodes[0], &[])?;
    let count = nodes[0].children().filter(Node::is_element).count();
    if !(1..=PlotSpec::MAX_TRACES).contains(&count) {
        return Err("Native XY trace count exceeds supported limits".into());
    }
    let trace_nodes = children(nodes[0], &vec!["trace"; count])?;
    let mut styles = Vec::with_capacity(count);
    let mut result_refs = Vec::with_capacity(count);
    for trace in trace_nodes {
        attrs(trace, &["resultRef"])?;
        result_refs.push(optional_ref(trace)?);
        let style = children(trace, &["traceStyle"])?[0];
        attrs(style, &["color", "symbol", "line-weight", "line-style"])?;
        if style.children().any(|n| n.is_element()) || style.text().unwrap_or("").trim() != "lines"
        {
            return Err("Unsupported native XY trace type".into());
        }
        attr_eq(style, "symbol", "none")?;
        let color = style.attribute("color").ok_or("Missing native XY color")?;
        let hex = color.strip_prefix('#').ok_or("Invalid native XY color")?;
        if !matches!(hex.len(), 6 | 8) || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("Invalid native XY color".into());
        }
        let bits = u32::from_str_radix(hex, 16).map_err(|_| "Invalid native XY color")?;
        let color = [
            (bits >> 16) as u8,
            (bits >> 8) as u8,
            bits as u8,
            if hex.len() == 8 {
                (bits >> 24) as u8
            } else {
                255
            },
        ];
        let line = match style.attribute("line-style") {
            Some("Solid") => PlotLineStyle::Solid,
            Some("Dot") => PlotLineStyle::Dotted,
            Some("Dash") => PlotLineStyle::Dashed,
            Some("DashDot") => PlotLineStyle::DashDot,
            Some("DashDotDot") => PlotLineStyle::DashDotDot,
            _ => return Err("Unsupported native XY line style".into()),
        };
        let line_width = model_number(
            style
                .attribute("line-weight")
                .ok_or("Missing native XY line weight")?,
        )?;
        styles.push(PlotTraceStyle {
            color,
            line,
            line_width,
            ..Default::default()
        });
    }
    let graph_size = if has_graph {
        empty(nodes[1], &["width", "height"])?;
        Some(PlotGraphSize {
            width: source_number(nodes[1], "width")?.ok_or("Missing native XY graph width")?,
            height: source_number(nodes[1], "height")?.ok_or("Missing native XY graph height")?,
        })
    } else {
        None
    };
    let geometry = PlotNativeGeometry {
        graph_size,
        requested_width: source_number(region, "width")?,
        requested_height: source_number(region, "height")?,
        actual_width: source_number(region, "actualWidth")?,
        actual_height: source_number(region, "actualHeight")?,
    };
    let [width, height] = geometry.allocation()?;
    let axes_node = nodes[if has_graph { 2 } else { 1 }];
    attrs(axes_node, &[])?;
    let axes = children(axes_node, &["xAxis", "yAxis"])?;
    let (x_expressions, x_axis) = read_axis(axes[0])?;
    let (y_expressions, y_axis) = read_axis(axes[1])?;
    if x_expressions.len().max(y_expressions.len()) != count {
        return Err("Native XY traces do not match the axis equation lists".into());
    }
    let nx = x_expressions.len();
    let ny = y_expressions.len();
    let source = PlotSource::AxisLists(crate::canvas::PlotAxisLists {
        next_equation_id: (nx + ny + 1) as u64,
        x: x_expressions
            .into_iter()
            .enumerate()
            .map(|(i, (source, scaling))| crate::canvas::PlotAxisEquation {
                id: (i + 1) as u64,
                source,
                scaling,
            })
            .collect(),
        y: y_expressions
            .into_iter()
            .enumerate()
            .map(|(i, (source, scaling))| crate::canvas::PlotAxisEquation {
                id: (nx + i + 1) as u64,
                source,
                scaling,
            })
            .collect(),
        appearances: styles
            .into_iter()
            .map(|style| PlotTraceAppearance {
                label: String::new(),
                style,
            })
            .collect(),
    });
    let spec = PlotSpec {
        source,
        width,
        height,
        native_geometry: Some(Box::new(geometry)),
        x_axis,
        y_axis,
    };
    spec.validate()?;
    Ok(ParsedPlot {
        spec,
        trace_result_refs: result_refs,
    })
}

/// Compare complete modeled plot source without generating or parsing XML.
/// Legacy expression fields are authoritative only when `traces` is empty,
/// following PlotSpec::resolved_traces. No axis expression is evaluated.
pub fn same_source(left: &PlotSpec, right: &PlotSpec) -> Result<bool, String> {
    left.validate()?;
    right.validate()?;
    if left.width.to_bits() != right.width.to_bits()
        || left.height.to_bits() != right.height.to_bits()
        || !same_geometry(
            left.native_geometry.as_deref(),
            right.native_geometry.as_deref(),
        )
        || !same_axis(&left.x_axis, &right.x_axis)
        || !same_axis(&left.y_axis, &right.y_axis)
    {
        return Ok(false);
    }
    let left_traces = left.paired_sources().collect::<Vec<_>>();
    let right_traces = right.paired_sources().collect::<Vec<_>>();
    if left_traces.len() != right_traces.len() {
        return Ok(false);
    }
    for (left, right) in left_traces.iter().zip(&right_traces) {
        if left.label != right.label || left.style != right.style {
            return Ok(false);
        }
    }
    for (left, right) in source_axes(left).iter().zip(source_axes(right).iter()) {
        if left.len() != right.len() {
            return Ok(false);
        }
        for (left, right) in left.iter().zip(right.iter()) {
            let expression_equal = match (left.expression, right.expression) {
                (PlotExpressionRef::Typed(a), PlotExpressionRef::Typed(b)) => a == b,
                (a, b) => same_expression(&expression_ref_tree(a)?, &expression_ref_tree(b)?),
            };
            if left.scaling != right.scaling || !expression_equal {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

fn source_number(node: Node<'_, '_>, name: &str) -> Result<Option<f64>, String> {
    node.attribute(name)
        .map(|v| {
            v.parse::<f64>()
                .map_err(|_| format!("Invalid native XY {name}"))
        })
        .transpose()
}
fn same_geometry(a: Option<&PlotNativeGeometry>, b: Option<&PlotNativeGeometry>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => a.same_source(b),
        _ => false,
    }
}
/// Read-only old-source adapter. Native absence is never synthesized into a
/// graph and source precision not supported by the old reader cannot match.
pub fn matches_retained_source(spec: &PlotSpec, previous: &PlotSpec) -> Result<bool, String> {
    if same_source(spec, previous)? {
        return Ok(true);
    }
    if spec.native_geometry.is_some() {
        return Ok(false);
    }
    let Some(graph) = previous
        .native_geometry
        .as_ref()
        .and_then(|g| g.graph_size.as_ref())
    else {
        return Ok(false);
    };
    let projection = (|| {
        let mut old = previous.clone();
        old.width = model_number(&graph.width.to_string())?;
        old.height = region_height(model_number(&graph.height.to_string())?)?;
        old.native_geometry = None;
        same_source(spec, &old)
    })();
    Ok(projection.unwrap_or(false))
}

fn same_axis(left: &PlotAxisSpec, right: &PlotAxisSpec) -> bool {
    left.label == right.label
        && left.legend_position == right.legend_position
        && match (left.legend_width, right.legend_width) {
            (None, None) => true,
            (Some(a), Some(b)) => a.to_bits() == b.to_bits(),
            _ => false,
        }
        && match (left.range, right.range) {
            (None, None) => true,
            (Some(left), Some(right)) => {
                left.min.to_bits() == right.min.to_bits()
                    && left.max.to_bits() == right.max.to_bits()
            }
            _ => false,
        }
}

/// Decode only the positively supported source subset. An error means the
/// caller must retain the original opaque region, rather than simplify it.
pub fn parse(owning_region_xml: &str) -> Result<PlotSpec, String> {
    parse_with_references(owning_region_xml).map(|parsed| parsed.spec)
}

/// Known, syntactically validated source metadata. Cached data is deliberately
/// not read here; callers must resolve the reference in its result-part scope.
pub fn trace_result_refs(owning_region_xml: &str) -> Result<Vec<Option<i32>>, String> {
    parse_with_references(owning_region_xml).map(|parsed| parsed.trace_result_refs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas::PlotTraceSpec;
    fn paired(traces: Vec<PlotTraceSpec>) -> PlotSource {
        PlotSource::legacy("x", "sin(x)", traces)
    }
    fn region(payload: &str) -> String {
        format!("<region xmlns=\"{WS}\" region-id=\"1\" left=\"2\" top=\"3\">{payload}</region>")
    }

    #[test]
    fn legacy_graph_generation_is_compatible_but_native_outer_is_not_inferred() {
        for height in [100., 260., 320., 2000., 260.2] {
            let spec = PlotSpec {
                width: 420.2,
                height,
                ..Default::default()
            };
            let xml = payload(&spec).unwrap();
            let doc = Document::parse(&xml).unwrap();
            let graph = doc
                .descendants()
                .find(|n| n.has_tag_name((WS, "graph-size")))
                .unwrap();
            assert_eq!(
                graph
                    .attribute("width")
                    .unwrap()
                    .parse::<f32>()
                    .unwrap()
                    .to_bits(),
                spec.width.to_bits()
            );
            assert_eq!(
                graph
                    .attribute("height")
                    .unwrap()
                    .parse::<f32>()
                    .unwrap()
                    .to_bits(),
                (height - 50.).max(45.).to_bits()
            );
            let restored = parse(&region(&xml)).unwrap();
            assert_eq!(restored.height.to_bits(), 260_f32.to_bits());
            assert!(matches_retained_source(&spec, &restored).unwrap());
        }
        assert!(
            payload(&PlotSpec::default())
                .unwrap()
                .contains("height=\"210\"")
        );
    }

    #[test]
    fn native_graph_dimensions_are_independent_f64_source_without_ui_allowance() {
        let original = region(&payload(&PlotSpec::default()).unwrap());
        for height in ["49", "1950.5", "210.2", "210.00000000001", "210.125"] {
            let source = original.replace("height=\"210\"", &format!("height=\"{height}\""));
            let parsed = parse(&source).unwrap();
            assert_eq!(parsed.height, 260.);
            assert_eq!(
                parsed
                    .native_geometry
                    .as_ref()
                    .unwrap()
                    .graph_size
                    .as_ref()
                    .unwrap()
                    .height
                    .to_bits(),
                height.parse::<f64>().unwrap().to_bits()
            );
            assert!(
                payload(&parsed)
                    .unwrap()
                    .contains(&format!("height=\"{height}\""))
            );
        }
    }

    #[test]
    fn parsed_source_returns_spec_and_reference_vector_together() {
        let spec = PlotSpec {
            source: paired(vec![
                PlotTraceSpec::default(),
                PlotTraceSpec {
                    y_expression: "cos(x)".into(),
                    ..Default::default()
                },
            ]),
            ..Default::default()
        };
        let xml =
            region(&payload(&spec).unwrap()).replacen("<trace>", "<trace resultRef=\"8\">", 1);
        let parsed = parse_with_references(&xml).unwrap();
        assert_eq!(parsed.trace_result_refs, [Some(8), None]);
        assert_eq!(parse(&xml).unwrap(), parsed.spec);
        assert_eq!(trace_result_refs(&xml).unwrap(), parsed.trace_result_refs);
        assert!(matches_retained_source(&spec, &parsed.spec).unwrap());
    }

    #[test]
    fn source_equality_compares_structural_expressions_and_every_modeled_presentation_field() {
        let base = PlotSpec::default();
        let equivalent = PlotSpec {
            source: PlotSource::single(" (x) ", "sin( (x) )"),
            ..Default::default()
        };
        assert!(matches_retained_source(&base, &equivalent).unwrap());
        let mut candidates = vec![];
        let mut changed = base.clone();
        changed.width += 1.;
        candidates.push(changed);
        let mut changed = base.clone();
        changed.height += 1.;
        candidates.push(changed);
        let mut changed = base.clone();
        changed.x_axis.label = "axis".into();
        candidates.push(changed);
        let mut changed = base.clone();
        changed.y_axis.range = Some(crate::canvas::PlotRange { min: -1., max: 2. });
        candidates.push(changed);
        let mut changed = base.clone();
        *changed.primary_y_expression_mut().unwrap() = "cos(x)".into();
        candidates.push(changed);
        for modify in 0..5 {
            let mut changed = base.clone();
            changed.promote_legacy_trace();
            match modify {
                0 => changed.legacy_traces_mut().unwrap()[0].label = "trace".into(),
                1 => changed.legacy_traces_mut().unwrap()[0].style.color[0] = 12,
                2 => changed.legacy_traces_mut().unwrap()[0].style.line = PlotLineStyle::Dotted,
                3 => changed.legacy_traces_mut().unwrap()[0].style.line_width = 2.,
                _ => {
                    changed.legacy_traces_mut().unwrap()[0]
                        .style
                        .marker_frequency = 2
                }
            }
            candidates.push(changed);
        }
        for changed in candidates {
            assert!(!same_source(&base, &changed).unwrap());
        }
        let invalid = PlotSpec {
            source: PlotSource::single("x @ y", "sin(x)"),
            ..Default::default()
        };
        assert!(same_source(&base, &invalid).is_err());
        let mut materialized = base.clone();
        materialized.promote_legacy_trace();
        assert!(matches_retained_source(&base, &materialized).unwrap());
    }

    #[test]
    fn source_roundtrip_keeps_arithmetic_calls_color_dimensions_and_style() {
        for line in [
            PlotLineStyle::Solid,
            PlotLineStyle::Dotted,
            PlotLineStyle::Dashed,
            PlotLineStyle::DashDot,
            PlotLineStyle::DashDotDot,
        ] {
            let original = PlotSpec {
                width: 560.,
                height: 320.,
                source: paired(vec![PlotTraceSpec {
                    x_expression: "t".into(),
                    y_expression: "sin(t)+t^2/(1+t)".into(),
                    style: PlotTraceStyle {
                        line,
                        color: [12, 34, 56, 78],
                        line_width: 2.75,
                        ..Default::default()
                    },
                    ..Default::default()
                }]),
                ..Default::default()
            };
            let xml = payload(&original).unwrap();
            assert!(!xml.contains("resultRef"));
            let parsed = parse(&region(&xml)).unwrap();
            assert_eq!(parsed.width, 420.);
            assert_eq!(parsed.height, 260.);
            assert_eq!(
                *parsed.paired_sources().next().unwrap().style,
                original.resolved_traces().unwrap()[0].style
            );
            assert_eq!(
                expression_ref_tree(parsed.paired_sources().next().unwrap().y.expression).unwrap(),
                source_tree(&original.resolved_traces().unwrap()[0].y_expression).unwrap()
            );
        }
    }

    #[test]
    fn typed_axis_preserves_label_context_literal_suffix_and_nary_structure() {
        use crate::canvas::{PlotAxis, PlotExpression};
        let id = tree(
            NodeKind::NativeIdentifier {
                name: "I".into(),
                labels: "VARIABLE".into(),
                contextual: Some(false),
                subscript: Some("dc𝛼".into()),
            },
            vec![],
        );
        let value = tree(
            NodeKind::Group,
            vec![tree(
                NodeKind::Matrix {
                    rows: 1,
                    cols: 2,
                    display: None,
                },
                vec![id.clone(), tree(NodeKind::Number("2.00".into()), vec![])],
            )],
        );
        let mut spec = PlotSpec::typed_blank_xy();
        spec.set_axis_expression(
            PlotAxis::X,
            0,
            PlotExpression::from_tree(value.clone()).unwrap(),
        )
        .unwrap();
        spec.set_axis_expression(PlotAxis::Y, 0, PlotExpression::from_tree(id).unwrap())
            .unwrap();
        let encoded = payload(&spec).unwrap();
        let owning = region(&encoded).replacen(
            "left=\"2\" top=\"3\"",
            "left=\"2\" top=\"3\" width=\"420\" height=\"260\"",
            1,
        );
        let restored = parse(&owning).unwrap();
        assert_eq!(restored.primary_x_expression().as_tree(), Some(&value));
        assert!(restored.primary_x_expression().as_legacy_text().is_none());
        assert!(restored.resolved_traces().is_err());
        assert!(same_source(&spec, &restored).unwrap());
        assert_eq!(payload(&restored).unwrap(), encoded);
        for changed in [
            encoded.replacen(
                "labels=\"VARIABLE\"",
                "labels=\"VARIABLE\" future=\"drop\"",
                1,
            ),
            encoded.replacen(
                "label-is-contextual=\"false\"",
                "label-is-contextual=\"false\" another=\"drop\"",
                1,
            ),
        ] {
            assert_ne!(changed, encoded);
            assert!(parse(&region(&changed)).is_err());
        }
    }
    #[test]
    fn typed_and_legacy_sources_compare_without_flattening_or_changing_legacy_json() {
        let legacy = PlotSpec::default();
        let before = serde_json::to_string(&legacy).unwrap();
        let typed = parse(&region(&payload(&legacy).unwrap())).unwrap();
        assert!(typed.primary_x_expression().as_tree().is_some());
        assert!(matches_retained_source(&legacy, &typed).unwrap());
        assert_eq!(serde_json::to_string(&legacy).unwrap(), before);
        let xml = region(&payload(&typed).unwrap());
        let labeled = xml.replacen("<ml:id xmlns:ml=", "<ml:id labels=\"UNIT\" xmlns:ml=", 1);
        assert_ne!(labeled, xml);
        let explicit = parse(&labeled).unwrap();
        assert!(!same_source(&typed, &explicit).unwrap());
        assert!(
            matches!(explicit.primary_x_expression().as_tree().unwrap().kind,NodeKind::AnnotatedIdentifier{ref labels,..} if labels=="UNIT")
        );
    }
    #[test]
    fn typed_native_depth_boundary_roundtrips_or_rejects_before_export() {
        use crate::canvas::{PlotAxis, PlotExpression};
        let nested = |groups| {
            let mut expression = tree(NodeKind::Identifier("x".into()), vec![]);
            for _ in 0..groups {
                expression = tree(NodeKind::Group, vec![expression]);
            }
            expression
        };
        let mut supported = PlotSpec::typed_blank_xy();
        let expression = nested(53);
        supported
            .set_axis_expression(
                PlotAxis::X,
                0,
                PlotExpression::from_tree(expression.clone()).unwrap(),
            )
            .unwrap();
        let encoded = payload(&supported).unwrap();
        let reopened = parse(&region(&encoded)).unwrap();
        assert_eq!(reopened.primary_x_expression().as_tree(), Some(&expression));
        assert_eq!(payload(&reopened).unwrap(), encoded);
        // The next expression is a valid bounded document tree, but its native
        // package would exceed the reader's XML depth after the region wrapper.
        let mut too_deep = supported.clone();
        too_deep
            .set_axis_expression(
                PlotAxis::X,
                0,
                PlotExpression::from_tree(nested(54)).unwrap(),
            )
            .unwrap();
        too_deep.validate().unwrap();
        assert!(payload(&too_deep).unwrap_err().contains("depth"));
        assert_eq!(
            supported.primary_x_expression().as_tree(),
            Some(&expression)
        );
    }
    #[test]
    fn references_are_inert_optional_metadata_and_validated_as_xsd_int() {
        let xml = region(&payload(&PlotSpec::default()).unwrap());
        assert_eq!(trace_result_refs(&xml).unwrap(), [None]);
        let referenced = xml.replace("<trace>", "<trace resultRef=\"-2147483648\">");
        assert_eq!(trace_result_refs(&referenced).unwrap(), [Some(i32::MIN)]);
        assert!(parse(&referenced).is_ok());
        assert_eq!(
            trace_result_refs(&xml.replace("<trace>", "<trace resultRef=\" +17 \" >")).unwrap(),
            [Some(17)]
        );
        let math_refs = referenced.replace("<math>", "<math resultRef=\"17\">");
        assert!(parse(&math_refs).is_ok());
        assert!(parse(&xml.replace("<trace>", "<trace resultRef=\"2147483648\">")).is_err());
    }

    #[test]
    fn source_literals_allow_equivalent_numeric_lexemes_but_not_extra_math_formatting() {
        let spec = PlotSpec {
            source: PlotSource::single("2", "2.5"),
            ..Default::default()
        };
        let xml = region(&payload(&spec).unwrap());
        let original_lexical = xml.replace(">2</ml:real>", ">2.000</ml:real>");
        assert!(parse(&original_lexical).is_ok());
        let extra = xml.replace(
            "</ml:real></math>",
            "</ml:real><formattingOverride/></math>",
        );
        assert!(parse(&extra).is_err());
        let scaling_text = xml.replace(
            "<math><ml:placeholder/>",
            "<math>discarded<ml:placeholder/>",
        );
        assert!(parse(&scaling_text).is_err());
        assert!(
            parse(&xml.replace("line-weight=\"1.5\"", "line-weight=\"1.234567890123\"")).is_err()
        );
        assert!(parse(&xml.replace("width=\"420\"", "width=\"420.0000000001\"")).is_ok());
        let decimal = PlotSpec {
            width: 420.2,
            source: paired(vec![PlotTraceSpec {
                style: PlotTraceStyle {
                    line_width: 1.2,
                    ..Default::default()
                },
                ..Default::default()
            }]),
            ..Default::default()
        };
        assert!(parse(&region(&payload(&decimal).unwrap())).is_ok());
    }

    #[test]
    fn writer_rejects_unsupported_settings_without_mutating_source() {
        let base = PlotSpec::default();
        let mut cases = vec![];
        let mut s = base.clone();
        s.x_axis.label = "x".into();
        cases.push(s);
        let mut s = base.clone();
        s.y_axis.range = Some(crate::canvas::PlotRange { min: 0., max: 1. });
        cases.push(s);
        let mut s = base.clone();
        s.promote_legacy_trace();
        s.legacy_traces_mut().unwrap()[0].label = "curve".into();
        cases.push(s);
        let mut s = base.clone();
        s.promote_legacy_trace();
        s.legacy_traces_mut().unwrap()[0].style.marker_size = 4.;
        cases.push(s);
        let mut s = base.clone();
        *s.legacy_traces_mut().unwrap() = vec![PlotTraceSpec::default(); PlotSpec::MAX_TRACES + 1];
        cases.push(s);
        for value in cases {
            let before = value.clone();
            assert!(payload(&value).is_err());
            assert_eq!(value, before);
        }
    }

    #[test]
    fn multiple_traces_keep_ordinal_expressions_independent_styles_and_reference_order() {
        let original = PlotSpec {
            source: paired(
                (0..PlotSpec::MAX_TRACES)
                    .map(|i| PlotTraceSpec {
                        x_expression: if i < 2 { "t".into() } else { format!("t+{i}") },
                        y_expression: format!("t^{i}"),
                        style: PlotTraceStyle {
                            color: [i as u8, 50, 200, 255],
                            line_width: 0.5 + i as f32 / 2.,
                            ..Default::default()
                        },
                        ..Default::default()
                    })
                    .collect(),
            ),
            ..Default::default()
        };
        let xml = region(&payload(&original).unwrap());
        let parsed = parse(&xml).unwrap();
        assert_eq!(parsed.trace_count(), 16);
        assert_eq!(xml.matches("<plotEquation>").count(), 32);
        for (new, old) in parsed.paired_sources().zip(original.paired_sources()) {
            assert_eq!(new.style, old.style);
            assert!(same_expression(
                &expression_ref_tree(new.x.expression).unwrap(),
                &expression_ref_tree(old.x.expression).unwrap()
            ));
            assert!(same_expression(
                &expression_ref_tree(new.y.expression).unwrap(),
                &expression_ref_tree(old.y.expression).unwrap()
            ));
        }
        let mut with_refs = xml.clone();
        for i in 0..16 {
            with_refs =
                with_refs.replacen("<trace>", &format!("<trace resultRef=\"{}\">", 100 + i), 1);
        }
        assert_eq!(
            trace_result_refs(&with_refs).unwrap(),
            (100..116).map(Some).collect::<Vec<_>>()
        );
        // A shorter original X list shares its last equation by min-index.
        // Keep its actual 15/16 shape instead of expanding or deduplicating.
        let doc = Document::parse(&xml).unwrap();
        let second = doc
            .descendants()
            .find(|n| n.has_tag_name((WS, "xAxis")))
            .unwrap()
            .descendants()
            .filter(|n| n.has_tag_name((WS, "plotEquation")))
            .nth(1)
            .unwrap();
        let mut fewer_x = xml.clone();
        fewer_x.replace_range(second.range(), "");
        let shorter = parse(&fewer_x).unwrap();
        assert_eq!(shorter.axis_lists().unwrap().x.len(), 15);
        assert_eq!(shorter.axis_lists().unwrap().y.len(), 16);
        let paired = shorter.paired_sources().collect::<Vec<_>>();
        assert_eq!(paired[14].x.expression, paired[15].x.expression);
        let shorter_roundtrip = parse(&region(&payload(&shorter).unwrap())).unwrap();
        assert_eq!(shorter_roundtrip.axis_lists().unwrap().x.len(), 15);
        assert_eq!(shorter_roundtrip.axis_lists().unwrap().y.len(), 16);
        assert!(matches_retained_source(&shorter, &shorter_roundtrip).unwrap());
        let excess = xml.replacen("</traces>", "<trace><traceStyle/></trace></traces>", 1);
        assert!(parse(&excess).is_err());
    }

    #[test]
    fn parser_rejects_unknown_metadata_domain_labels_and_trace_variants() {
        let xml = region(&payload(&PlotSpec::default()).unwrap());
        for changed in [
            xml.replace("<xyPlot>", "<xyPlot future=\"x\">"),
            xml.replace("<traces>", "<traces><future/>"),
            xml.replace("symbol=\"none\"", "symbol=\"circle\""),
            xml.replace("rank=\"1\"", "rank=\"2\""),
            xml.replace("scale-type=\"linear\"", "scale-type=\"log\""),
            xml.replace("<plotEquation>", "<plotEquation><title/>"),
            xml.replace(">x</ml:id>", ">x<!--split-->future</ml:id>"),
            xml.replace(
                ">lines</traceStyle>",
                ">lines<!--split-->future</traceStyle>",
            ),
            xml.replace(
                ">11</gridFrequency>",
                ">11<?split value?>12</gridFrequency>",
            ),
        ] {
            assert!(parse(&changed).is_err(), "{changed}");
        }
    }

    #[test]
    fn empty_axis_equation_is_an_exact_native_placeholder_without_metadata_loss() {
        for (x, y) in [
            (vec![""], vec![""]),
            (vec!["x"], vec!["x", ""]),
            (vec!["", "x"], vec!["x"]),
        ] {
            let count = x.len().max(y.len());
            let spec = PlotSpec {
                source: PlotSource::axis_lists(
                    x.into_iter().map(String::from).collect(),
                    y.into_iter().map(String::from).collect(),
                    vec![PlotTraceAppearance::default(); count],
                )
                .unwrap(),
                ..Default::default()
            };
            let xml = region(&payload(&spec).unwrap());
            let parsed = parse(&xml).unwrap();
            assert!(matches_retained_source(&spec, &parsed).unwrap());
            for (old, new) in source_axes(&spec).iter().zip(source_axes(&parsed)) {
                assert_eq!(old.len(), new.len());
                for (a, b) in old.iter().zip(new) {
                    assert_eq!(
                        expression_ref_tree(a.expression).unwrap(),
                        expression_ref_tree(b.expression).unwrap()
                    );
                    assert!(b.expression.as_tree().is_some());
                }
            }
            assert!(xml.contains("<ml:placeholder"));
        }
        let spec = PlotSpec {
            source: PlotSource::single("", "x"),
            ..Default::default()
        };
        let xml = region(&payload(&spec).unwrap());
        for malformed in [
            xml.replacen("<ml:placeholder", "<ml:placeholder future=\"metadata\"", 1),
            xml.replacen(
                "<ml:placeholder/>",
                "<ml:placeholder>discarded</ml:placeholder>",
                1,
            ),
        ] {
            assert_ne!(malformed, xml);
            assert!(parse(&malformed).is_err());
        }
        let whitespace = PlotSpec {
            source: PlotSource::single(" ", "x"),
            ..Default::default()
        };
        assert!(
            payload(&whitespace).is_err(),
            "only exact empty source represents PH"
        );
    }

    #[test]
    fn namespaces_and_structural_budgets_do_not_accept_lookalikes() {
        let xml = region(&payload(&PlotSpec::default()).unwrap());
        assert!(parse(&xml.replace(WS, "urn:fake")).is_err());
        assert!(parse(&format!("<!DOCTYPE region [<!ENTITY x 'unsafe'>]>{xml}")).is_err());
        assert!(parse(&" ".repeat(MAX_BYTES + 1)).is_err());
        assert!(
            parse(&xml.replace(
                "<xyPlot>",
                &format!("<xyPlot>{}", "<future/>".repeat(MAX_NODES as usize))
            ))
            .is_err()
        );
        let deep = PlotSpec {
            source: PlotSource::single("x", format!("{}x", "-".repeat(80))),
            ..Default::default()
        };
        assert!(payload(&deep).is_err());
    }
    #[test]
    fn shared_x_and_y_native_roundtrips_keep_axis_shapes_min_index_and_result_reference_order() {
        for (nx, ny) in [(1, 3), (3, 1), (2, 3), (3, 2), (15, 16)] {
            let spec = PlotSpec {
                source: PlotSource::axis_lists(
                    (0..nx).map(|i| format!("t+{i}")).collect(),
                    (0..ny).map(|i| format!("sin(t)+{i}")).collect(),
                    (0..nx.max(ny))
                        .map(|i| crate::canvas::PlotTraceAppearance {
                            style: PlotTraceStyle {
                                color: [i as u8, 33, 77, 255],
                                ..Default::default()
                            },
                            ..Default::default()
                        })
                        .collect(),
                )
                .unwrap(),
                ..Default::default()
            };
            let mut xml = region(&payload(&spec).unwrap());
            assert_eq!(xml.matches("<plotEquation>").count(), nx + ny);
            for i in 0..nx.max(ny) {
                xml = xml.replacen("<trace>", &format!("<trace resultRef=\"{}\">", 410 + i), 1);
            }
            let parsed = parse_with_references(&xml).unwrap();
            let graph = parsed.spec.axis_lists().unwrap();
            assert_eq!((graph.x.len(), graph.y.len()), (nx, ny));
            assert_eq!(
                parsed.trace_result_refs,
                (410..410 + nx.max(ny) as i32).map(Some).collect::<Vec<_>>()
            );
            assert!(matches_retained_source(&spec, &parsed.spec).unwrap());
            assert_eq!(
                payload(&parsed.spec)
                    .unwrap()
                    .matches("<plotEquation>")
                    .count(),
                nx + ny
            );
            let original_ids: Vec<_> = graph.x.iter().chain(&graph.y).map(|e| e.id).collect();
            let mut edited = parsed.spec;
            edited
                .set_axis_expression_text(crate::canvas::PlotAxis::X, 0, "2*t")
                .unwrap();
            edited
                .set_axis_expression_text(crate::canvas::PlotAxis::Y, 0, "3*t")
                .unwrap();
            let pairs = edited.paired_sources().collect::<Vec<_>>();
            assert_eq!(pairs[0].x.expression.as_legacy_text(), Some("2*t"));
            assert_eq!(pairs[0].y.expression.as_legacy_text(), Some("3*t"));
            if nx == 1 {
                assert!(
                    pairs
                        .iter()
                        .all(|p| p.x.expression.as_legacy_text() == Some("2*t"))
                );
            }
            if ny == 1 {
                assert!(
                    pairs
                        .iter()
                        .all(|p| p.y.expression.as_legacy_text() == Some("3*t"))
                );
            }
            let graph = edited.axis_lists().unwrap();
            assert_eq!(
                original_ids,
                graph
                    .x
                    .iter()
                    .chain(&graph.y)
                    .map(|e| e.id)
                    .collect::<Vec<_>>()
            );
            let restored = parse(&region(&payload(&edited).unwrap())).unwrap();
            assert!(matches_retained_source(&edited, &restored).unwrap());
            assert_eq!(
                (
                    restored.axis_lists().unwrap().x.len(),
                    restored.axis_lists().unwrap().y.len()
                ),
                (nx, ny)
            );
        }
    }
    #[test]
    fn same_resolved_pairs_do_not_erase_shared_source_shape_or_merge_equal_native_equations() {
        let shared = PlotSpec {
            source: PlotSource::axis_lists(
                vec!["t".into()],
                vec!["sin(t)".into(), "cos(t)".into()],
                vec![crate::canvas::PlotTraceAppearance::default(); 2],
            )
            .unwrap(),
            ..Default::default()
        };
        let independent = PlotSpec {
            source: PlotSource::axis_lists(
                vec!["t".into(), "t".into()],
                vec!["sin(t)".into(), "cos(t)".into()],
                vec![crate::canvas::PlotTraceAppearance::default(); 2],
            )
            .unwrap(),
            ..Default::default()
        };
        assert_eq!(
            shared.resolved_traces().unwrap(),
            independent.resolved_traces().unwrap()
        );
        assert!(!same_source(&shared, &independent).unwrap());
        let mut independent = parse(&region(&payload(&independent).unwrap())).unwrap();
        // Equal native lists remain independent typed slots; no text deduplication.
        assert_eq!(source_axes(&independent)[0].len(), 2);
        independent.promote_axis_lists().unwrap();
        let graph = independent.axis_lists().unwrap();
        assert_eq!(graph.x.len(), 2);
        assert_ne!(graph.x[0].id, graph.x[1].id);
        independent
            .set_axis_expression_text(crate::canvas::PlotAxis::X, 0, "2*t")
            .unwrap();
        assert_eq!(
            independent
                .paired_sources()
                .nth(1)
                .unwrap()
                .x
                .expression
                .as_tree(),
            Some(&source_tree("t").unwrap())
        );
        // Owner-local IDs are not native IEquationItem identities or source text.
        let mut rebased = shared.clone();
        let graph = rebased.axis_lists_mut().unwrap();
        graph.x[0].id = 777;
        graph.next_equation_id = 1000;
        assert!(matches_retained_source(&shared, &rebased).unwrap());
        assert_eq!(payload(&shared).unwrap(), payload(&rebased).unwrap());
    }
    #[test]
    fn shared_native_source_still_rejects_empty_axes_and_trace_count_mismatch() {
        let spec = PlotSpec {
            source: PlotSource::axis_lists(
                vec!["t".into()],
                vec!["sin(t)".into(), "cos(t)".into()],
                vec![crate::canvas::PlotTraceAppearance::default(); 2],
            )
            .unwrap(),
            ..Default::default()
        };
        let xml = region(&payload(&spec).unwrap());
        let doc = Document::parse(&xml).unwrap();
        let equation = doc
            .descendants()
            .find(|n| n.has_tag_name((WS, "xAxis")))
            .unwrap()
            .descendants()
            .find(|n| n.has_tag_name((WS, "plotEquation")))
            .unwrap();
        let mut empty = xml.clone();
        empty.replace_range(equation.range(), "");
        assert!(parse(&empty).is_err());
        let trace = doc
            .descendants()
            .find(|n| n.has_tag_name((WS, "trace")))
            .unwrap();
        let mut missing = xml.clone();
        missing.replace_range(trace.range(), "");
        assert!(parse(&missing).is_err());
    }
    #[test]
    fn boundary_legend_positions_and_finite_double_width_roundtrip_without_default_augmentation() {
        let absent = PlotSpec::default();
        let old = payload(&absent).unwrap();
        assert!(!old.contains("legend-position") && !old.contains("legendWidth"));
        let parsed = parse(&region(&old)).unwrap();
        assert!(parsed.x_axis.legend_position.is_none() && parsed.y_axis.legend_position.is_none());
        for x in [PlotLegendPosition::Bottom, PlotLegendPosition::Top] {
            for y in [PlotLegendPosition::Left, PlotLegendPosition::Right] {
                for width in [
                    0.,
                    -0.,
                    12.5,
                    123.45678901234567,
                    f64::from_bits(1),
                    f64::MAX,
                ] {
                    let mut spec = PlotSpec::default();
                    spec.x_axis.legend_position = Some(x);
                    spec.y_axis.legend_position = Some(y);
                    spec.x_axis.legend_width = Some(width);
                    spec.y_axis.legend_width = Some(width);
                    let xml = payload(&spec).unwrap();
                    let restored = parse(&region(&xml)).unwrap();
                    assert_eq!(restored.x_axis.legend_position, Some(x));
                    assert_eq!(restored.y_axis.legend_position, Some(y));
                    assert_eq!(
                        restored.x_axis.legend_width.unwrap().to_bits(),
                        width.to_bits()
                    );
                    assert_eq!(
                        restored.y_axis.legend_width.unwrap().to_bits(),
                        width.to_bits()
                    );
                    assert!(matches_retained_source(&spec, &restored).unwrap());
                    assert_eq!(payload(&restored).unwrap(), xml);
                }
            }
        }
        let explicit = PlotSpec {
            x_axis: PlotAxisSpec {
                legend_position: Some(PlotLegendPosition::Bottom),
                ..Default::default()
            },
            y_axis: PlotAxisSpec {
                legend_position: Some(PlotLegendPosition::Left),
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(!same_source(&absent, &explicit).unwrap());
        let mut plus = explicit.clone();
        plus.x_axis.legend_width = Some(0.);
        let mut minus = plus.clone();
        minus.x_axis.legend_width = Some(-0.);
        assert!(!same_source(&plus, &minus).unwrap());
        let blank = PlotSpec::blank_xy();
        let xml = payload(&blank).unwrap();
        assert!(xml.contains("legend-position=\"PlotBoundaryRight\""));
        assert!(xml.contains("color=\"#FF00008B\"") && xml.contains("line-weight=\"1\""));
        let source = region(&xml).replacen("<region ", "<region width=\"420\" height=\"260\" ", 1);
        assert!(same_source(&blank, &parse(&source).unwrap()).unwrap());
    }
    #[test]
    fn unsupported_legend_positions_widths_and_axis_line_metadata_retain_raw_errors() {
        let source = region(&payload(&PlotSpec::default()).unwrap());
        for position in [
            "AxisAttachedTopRight",
            "AxisAttachedBottomRight",
            "AxisAttachedTopLeft",
            "AxisAttachedBottomLeft",
            "PlotBoundaryLeft",
            "plotboundarybottom",
            "XBottom",
            "",
        ] {
            assert!(
                parse(&source.replace(
                    "<xAxis rank=\"1\">",
                    &format!("<xAxis rank=\"1\" legend-position=\"{position}\">")
                ))
                .is_err(),
                "{position}"
            );
        }
        for width in ["-1", "NaN", "INF", "-INF", "1e9999", "not-a-double"] {
            assert!(
                parse(&source.replacen(
                    "<axisLine position=\"origin\"/>",
                    &format!("<axisLine position=\"origin\" legendWidth=\"{width}\"/>"),
                    1
                ))
                .is_err(),
                "{width}"
            );
        }
        let wrong_y = source.replace(
            "<yAxis rank=\"1\">",
            "<yAxis rank=\"1\" legend-position=\"PlotBoundaryBottom\">",
        );
        assert!(parse(&wrong_y).is_err());
        assert!(
            parse(&source.replacen(
                "<axisLine position=\"origin\"/>",
                "<axisLine position=\"origin\" legendWidth=\"10\" future=\"1\"/>",
                1
            ))
            .is_err()
        );
        assert!(
            parse(&source.replacen(
                "<axisLine position=\"origin\"/>",
                "<axisLine position=\"origin\" xmlns:q=\"urn:future\" q:legendWidth=\"10\"/>",
                1
            ))
            .is_err()
        );
        let whitespace = source.replacen(
            "<axisLine position=\"origin\"/>",
            "<axisLine position=\"origin\" legendWidth=\" 12.5 \"/>",
            1,
        );
        assert_eq!(parse(&whitespace).unwrap().x_axis.legend_width, Some(12.5));
    }
}
