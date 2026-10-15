//! Bounded XY plotting through the same structural worksheet evaluator as math.
use eframe::egui::{self, Color32, Response, Stroke};
use mathcraft_compat::{Value, editor_bridge};
use mathcraft_core::{Expr, Quantity};
use mathcraft_document::canvas::PlotSpec;
use mathcraft_editor::{BinaryOp, NodeKind, PortableTree};

const MAX_SAMPLES: usize = 2000;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PlotData {
    pub points: Vec<[f64; 2]>,
    /// Independent project policy, exposed to the author rather than hidden.
    pub implicit_domain: bool,
}

fn tree(kind: NodeKind, children: Vec<PortableTree>) -> PortableTree {
    PortableTree { kind, children }
}

fn expression_tree(
    expression: &Expr,
    replace: Option<(&str, f64)>,
) -> Result<PortableTree, String> {
    let convert = |e| expression_tree(e, replace);
    Ok(match expression {
        Expr::Number(value) => tree(NodeKind::Number(value.to_string()), vec![]),
        Expr::Name(name) => match replace {
            Some((symbol, value)) if name == symbol => {
                tree(NodeKind::Number(value.to_string()), vec![])
            }
            _ => tree(NodeKind::Identifier(name.clone()), vec![]),
        },
        Expr::Neg(a) => tree(NodeKind::UnaryNeg, vec![convert(a)?]),
        Expr::Binary(op, a, b) => tree(
            match op {
                '+' => NodeKind::Binary(BinaryOp::Add),
                '-' => NodeKind::Binary(BinaryOp::Subtract),
                '*' => NodeKind::Binary(BinaryOp::Multiply),
                '/' => NodeKind::Fraction,
                '^' => NodeKind::Power,
                _ => return Err(format!("Unsupported plot operator {op}")),
            },
            vec![convert(a)?, convert(b)?],
        ),
        Expr::Call(name, arguments) => {
            let mut children = vec![tree(NodeKind::Identifier(name.clone()), vec![])];
            children.extend(
                arguments
                    .iter()
                    .map(convert)
                    .collect::<Result<Vec<_>, _>>()?,
            );
            tree(
                NodeKind::Call {
                    argument_count: arguments.len(),
                },
                children,
            )
        }
    })
}

fn evaluation(expression: &Expr, replace: Option<(&str, f64)>) -> Result<String, String> {
    editor_bridge::tree_to_mathml(&tree(
        NodeKind::Binary(BinaryOp::Evaluate),
        vec![
            expression_tree(expression, replace)?,
            tree(NodeKind::Placeholder, vec![]),
        ],
    ))
}

fn contains_name(expression: &Expr, symbol: &str) -> bool {
    match expression {
        Expr::Name(name) => name == symbol,
        Expr::Neg(a) => contains_name(a, symbol),
        Expr::Binary(_, a, b) => contains_name(a, symbol) || contains_name(b, symbol),
        Expr::Call(_, arguments) => arguments.iter().any(|a| contains_name(a, symbol)),
        Expr::Number(_) => false,
    }
}

fn real_number(quantity: Quantity) -> Result<f64, String> {
    if !quantity.is_scalar() {
        return Err(
            "도표 축은 현재 무차원 값만 지원합니다. 단위가 있는 값을 단위로 나누어 주세요.".into(),
        );
    }
    if quantity.value.im != 0. {
        return Err("도표 축에 복소수를 사용할 수 없습니다.".into());
    }
    if !quantity.value.re.is_finite() {
        return Err("도표 축에 유한하지 않은 값이 있습니다.".into());
    }
    Ok(quantity.value.re)
}

fn numbers(value: &Value) -> Result<Vec<f64>, String> {
    match value {
        Value::Scalar { quantity } => Ok(vec![real_number(*quantity)?]),
        Value::Matrix {
            rows,
            cols,
            elements,
        } => {
            if *rows > MAX_SAMPLES {
                return Err("도표 표본은 최대 2000개입니다.".into());
            }
            if *cols == 0 || *rows == 0 {
                return Ok(Vec::new());
            }
            elements
                .chunks(*cols)
                .map(|row| real_number(row[0]))
                .collect()
        }
        Value::RangeValues { elements } => {
            if elements.len() > MAX_SAMPLES {
                return Err("도표 표본은 최대 2000개입니다.".into());
            }
            elements.iter().copied().map(real_number).collect()
        }
        Value::Range {
            start,
            step,
            end,
            count,
        } => {
            if *count > MAX_SAMPLES {
                return Err("도표 표본은 최대 2000개입니다.".into());
            }
            let start = real_number(*start)?;
            let step = real_number(*step)?;
            real_number(*end)?;
            (0..*count)
                .map(|i| {
                    let n = start + step * i as f64;
                    if n.is_finite() {
                        Ok(n)
                    } else {
                        Err("범위가 유한한 값의 한계를 넘었습니다.".into())
                    }
                })
                .collect()
        }
        Value::Symbolic { .. } => Err("도표 축에는 수치 값이 필요합니다.".into()),
    }
}

fn fault(error: &mathcraft_compat::Fault) -> String {
    format!("{}: {}", error.code, error.detail)
}

fn query(
    prior: &[(u64, f64, f64, String)],
    expressions: &[String],
    left: f64,
    top: f64,
) -> Result<Vec<mathcraft_compat::RegionEvaluation>, String> {
    // Temporary IDs are scoped to this evaluator invocation, never persisted.
    let mut next = 1_u64;
    let used: std::collections::HashSet<_> = prior.iter().map(|r| r.0).collect();
    let mut regions = prior.to_vec();
    let mut ids = Vec::with_capacity(expressions.len());
    for expression in expressions {
        while used.contains(&next) || ids.contains(&next) {
            next = next.checked_add(1).ok_or("도표 임시 ID 한계")?;
        }
        ids.push(next);
        regions.push((next, left, top, expression.clone()));
        next = next.checked_add(1).ok_or("도표 임시 ID 한계")?;
    }
    let xml = editor_bridge::worksheet_xml(&regions)?;
    let output = mathcraft_compat::evaluate_worksheet(&xml)?;
    let mut map: std::collections::HashMap<_, _> = output
        .into_iter()
        .map(|r| (r.region_id.clone(), r))
        .collect();
    ids.into_iter()
        .map(|id| {
            map.remove(&id.to_string())
                .ok_or_else(|| "도표 평가 결과가 없습니다.".into())
        })
        .collect()
}

/// Evaluate prior placed math plus temporary plot expressions. No source region
/// or variable is changed. The caller supplies the worksheet context preceding
/// this plot (plus any globally scoped definitions it wants visible).
pub(crate) fn evaluate(
    spec: &PlotSpec,
    prior: &[(u64, f64, f64, String)],
    plot_id: u64,
    left: f64,
    top: f64,
) -> Result<PlotData, String> {
    spec.validate()?;
    if !left.is_finite() || !top.is_finite() {
        return Err("도표 위치가 유한하지 않습니다.".into());
    }
    if prior.iter().any(|r| r.0 == plot_id) {
        return Err("도표 ID가 계산 영역 ID와 겹칩니다.".into());
    }
    let x = mathcraft_core::parse(&spec.x_expression).map_err(|e| format!("x축: {e}"))?;
    let y = mathcraft_core::parse(&spec.y_expression).map_err(|e| format!("y축: {e}"))?;
    let initial = query(
        prior,
        &[evaluation(&x, None)?, evaluation(&y, None)?],
        left,
        top,
    )?;
    let x_result = &initial[0];
    let mut implicit_domain = false;
    let xs = if let Some(error) = &x_result.error {
        if matches!(&x, Expr::Name(_))
            && error.code == "bad%_variable"
            && error.previous_region.is_none()
            && !error.unsupported
        {
            implicit_domain = true;
            (0..500).map(|i| -10. + i as f64 * 20. / 499.).collect()
        } else {
            return Err(format!("x축: {}", fault(error)));
        }
    } else {
        numbers(x_result.value.as_ref().ok_or("x축 결과가 없습니다.")?)
            .map_err(|e| format!("x축: {e}"))?
    };
    if xs.is_empty() {
        return Err("x축 벡터 또는 범위가 비어 있습니다.".into());
    }

    let ys = if let Expr::Name(symbol) = &x
        && (contains_name(&y, symbol) || implicit_domain)
    {
        // Plotting f(x) evaluates the axis expression pointwise even when the
        // existing x is a vector. It does not reinterpret vector powers as
        // matrix powers and never redefines x in the worksheet context.
        let expressions = xs
            .iter()
            .map(|n| evaluation(&y, Some((symbol, *n))))
            .collect::<Result<Vec<_>, _>>()?;
        query(prior, &expressions, left, top)?
            .into_iter()
            .enumerate()
            .map(|(i, result)| {
                if let Some(error) = result.error {
                    return Err(format!(
                        "y축 표본 {} (x={}): {}",
                        i + 1,
                        xs[i],
                        fault(&error)
                    ));
                }
                match result.value {
                    Some(Value::Scalar { quantity }) => real_number(quantity),
                    _ => Err("y축의 각 표본은 하나의 실수여야 합니다.".into()),
                }
            })
            .collect::<Result<Vec<_>, _>>()?
    } else {
        if let Some(error) = &initial[1].error {
            return Err(format!("y축: {}", fault(error)));
        }
        numbers(initial[1].value.as_ref().ok_or("y축 결과가 없습니다.")?)
            .map_err(|e| format!("y축: {e}"))?
    };
    if ys.is_empty() {
        return Err("y축 벡터 또는 범위가 비어 있습니다.".into());
    }
    let x_scalar = !implicit_domain && matches!(x_result.value, Some(Value::Scalar { .. }));
    let y_scalar = matches!(initial[1].value, Some(Value::Scalar { .. }));
    let count = if x_scalar {
        ys.len()
    } else if y_scalar {
        xs.len()
    } else {
        xs.len().min(ys.len())
    };
    let points = (0..count)
        .map(|i| {
            [
                xs[if x_scalar { 0 } else { i }],
                ys[if y_scalar && ys.len() == 1 { 0 } else { i }],
            ]
        })
        .collect();
    Ok(PlotData {
        points,
        implicit_domain,
    })
}

/// In-sheet axis expressions and interactive axes. The returned response is
/// marked changed for source/size edits, never merely for panning the view.
pub(crate) fn draw(
    ui: &mut egui::Ui,
    spec: &mut PlotSpec,
    data: &Result<PlotData, String>,
    focus: bool,
) -> Response {
    let size = egui::vec2(
        spec.width.clamp(120., 2000.),
        spec.height.clamp(100., 2000.),
    );
    let focus_id = ui.make_persistent_id("plot_had_focus");
    let had_focus = ui.data(|d| d.get_temp::<bool>(focus_id).unwrap_or(false));
    ui.data_mut(|d| d.insert_temp(focus_id, focus));
    let x_id = ui.make_persistent_id("plot_x");
    let y_id = ui.make_persistent_id("plot_y");
    let field_was_focused = ui.memory(|m| m.has_focus(x_id) || m.has_focus(y_id));
    let mut focused_field = None;
    let mut changed = false;
    let region = ui.allocate_ui_with_layout(size, egui::Layout::top_down(egui::Align::Min), |ui| {
        ui.set_width(size.x);
        ui.spacing_mut().item_spacing = egui::vec2(3., 2.);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("y").italics());
            let y = ui.add(
                egui::TextEdit::singleline(&mut spec.y_expression)
                    .id(y_id)
                    .desired_width(size.x - 27.)
                    .char_limit(4096)
                    .frame(if focus || field_was_focused {
                        egui::Frame::new()
                            .inner_margin(2.)
                            .stroke(Stroke::new(0.5, Color32::from_gray(170)))
                    } else {
                        egui::Frame::NONE
                    }),
            );
            if focus && !had_focus && !field_was_focused {
                y.request_focus();
            }
            if y.has_focus() {
                focused_field = Some(y.id);
            }
            changed |= y.changed();
        });
        match data {
            Ok(data) => {
                egui_plot::Plot::new(ui.id().with("xy"))
                    .width(size.x)
                    .height((size.y - 50.).max(45.))
                    .allow_scroll(false)
                    .allow_drag(true)
                    .allow_zoom(true)
                    .show_background(false)
                    .show_grid(false)
                    .show(ui, |plot| {
                        if data.points.len() == 1 {
                            plot.points(
                                egui_plot::Points::new("", data.points.clone())
                                    .radius(3.)
                                    .color(Color32::from_rgb(0, 70, 150)),
                            );
                        } else {
                            plot.line(
                                egui_plot::Line::new("", data.points.clone())
                                    .color(Color32::from_rgb(0, 70, 150))
                                    .width(1.5),
                            );
                        }
                    });
            }
            Err(error) => {
                let (rect, _) = ui.allocate_exact_size(
                    egui::vec2(size.x, (size.y - 50.).max(45.)),
                    egui::Sense::hover(),
                );
                ui.painter().rect_stroke(
                    rect,
                    0.,
                    Stroke::new(0.5, Color32::from_gray(170)),
                    egui::StrokeKind::Inside,
                );
                ui.scope_builder(egui::UiBuilder::new().max_rect(rect.shrink(10.)), |ui| {
                    ui.colored_label(Color32::from_rgb(170, 25, 25), error);
                });
            }
        }
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("x").italics());
            let x_response = ui.add(
                egui::TextEdit::singleline(&mut spec.x_expression)
                    .id(x_id)
                    .desired_width((size.x - 150.).max(60.))
                    .char_limit(4096)
                    .frame(if focus || field_was_focused {
                        egui::Frame::new()
                            .inner_margin(2.)
                            .stroke(Stroke::new(0.5, Color32::from_gray(170)))
                    } else {
                        egui::Frame::NONE
                    }),
            );
            changed |= x_response.changed();
            if x_response.has_focus() {
                focused_field = Some(x_response.id);
            }
            if data.as_ref().is_ok_and(|d| d.implicit_domain) {
                ui.label(
                    egui::RichText::new("−10 … 10 / 500점")
                        .size(10.)
                        .color(Color32::from_gray(110)),
                )
                .on_hover_text("미정의 독립 변수의 도표 전용 표본 범위입니다.");
            }
        });
    });
    let mut response = region.response;
    {
        let handle = egui::Rect::from_min_size(
            response.rect.max - egui::vec2(13., 13.),
            egui::vec2(13., 13.),
        );
        let resize = ui
            .interact(handle, ui.id().with("plot_resize"), egui::Sense::drag())
            .on_hover_cursor(egui::CursorIcon::ResizeNwSe);
        for d in [2., 6., 10.] {
            ui.painter().line_segment(
                [
                    handle.right_bottom() - egui::vec2(d, 1.),
                    handle.right_bottom() - egui::vec2(1., d),
                ],
                Stroke::new(0.7, Color32::GRAY),
            );
        }
        if resize.dragged() {
            spec.width = (spec.width + resize.drag_delta().x).clamp(120., 2000.);
            spec.height = (spec.height + resize.drag_delta().y).clamp(100., 2000.);
            // Record one document checkpoint for the resize gesture. Its
            // intermediate dimensions are view/layout changes, not new commands.
            changed |= resize.drag_started();
        }
    }
    // Keep the full plot's rectangle, but report the focused text field's ID so
    // the worksheet does not steal keyboard input or move it while selecting.
    if let Some(id) = focused_field {
        response.id = id;
    }
    if changed {
        response.mark_changed();
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    fn expr(source: &str) -> PortableTree {
        expression_tree(&mathcraft_core::parse(source).unwrap(), None).unwrap()
    }
    fn definition(id: u64, name: &str, rhs: PortableTree) -> (u64, f64, f64, String) {
        (
            id,
            0.,
            id as f64 * 10.,
            editor_bridge::tree_to_mathml(&tree(
                NodeKind::Binary(BinaryOp::Define),
                vec![tree(NodeKind::Identifier(name.into()), vec![]), rhs],
            ))
            .unwrap(),
        )
    }
    fn evaluate_plot(
        spec: PlotSpec,
        prior: &[(u64, f64, f64, String)],
    ) -> Result<PlotData, String> {
        evaluate(&spec, prior, 999, 0., 100.)
    }

    #[test]
    fn implicit_domain_and_whole_input_parser() {
        let data = evaluate_plot(PlotSpec::default(), &[]).unwrap();
        assert_eq!(data.points.len(), 500);
        assert!(data.implicit_domain);
        assert!((data.points[0][1] - (-10_f64).sin()).abs() < 1e-12);
        assert_eq!(data.points.first().unwrap()[0], -10.);
        assert_eq!(data.points.last().unwrap()[0], 10.);
        for invalid in ["sin(x) junk", "x:=2", "x;2", "[1,2]"] {
            assert!(
                evaluate_plot(
                    PlotSpec {
                        y_expression: invalid.into(),
                        ..Default::default()
                    },
                    &[]
                )
                .is_err()
            );
        }
    }

    #[test]
    fn prior_vector_and_function_are_evaluated_pointwise_without_rebinding() {
        let vector = tree(
            NodeKind::Matrix { rows: 3, cols: 1 },
            vec![expr("-2"), expr("0"), expr("3")],
        );
        let function = tree(
            NodeKind::Function { parameter_count: 1 },
            vec![
                tree(NodeKind::Identifier("f".into()), vec![]),
                tree(NodeKind::Identifier("t".into()), vec![]),
            ],
        );
        let f = (
            2,
            0.,
            20.,
            editor_bridge::tree_to_mathml(&tree(
                NodeKind::Binary(BinaryOp::Define),
                vec![function, expr("t^2+1")],
            ))
            .unwrap(),
        );
        let prior = vec![definition(1, "x", vector), f];
        let before = prior.clone();
        let data = evaluate_plot(
            PlotSpec {
                y_expression: "f(x)".into(),
                ..Default::default()
            },
            &prior,
        )
        .unwrap();
        assert_eq!(data.points, vec![[-2., 5.], [0., 1.], [3., 10.]]);
        assert!(!data.implicit_domain);
        assert_eq!(prior, before);
    }

    #[test]
    fn explicit_range_and_independent_vectors() {
        let range = tree(
            NodeKind::Range { has_step: true },
            vec![expr("0"), expr("0.5"), expr("1")],
        );
        let prior = vec![definition(1, "t", range)];
        let data = evaluate_plot(
            PlotSpec {
                x_expression: "t".into(),
                y_expression: "t^2".into(),
                ..Default::default()
            },
            &prior,
        )
        .unwrap();
        assert_eq!(data.points, vec![[0., 0.], [0.5, 0.25], [1., 1.]]);
        let v = |a, b| {
            tree(
                NodeKind::Matrix { rows: 2, cols: 1 },
                vec![expr(a), expr(b)],
            )
        };
        let data = evaluate_plot(
            PlotSpec {
                x_expression: "a".into(),
                y_expression: "b".into(),
                ..Default::default()
            },
            &[
                definition(1, "a", v("1", "2")),
                definition(2, "b", v("3", "5")),
            ],
        )
        .unwrap();
        assert_eq!(data.points, vec![[1., 3.], [2., 5.]]);
    }

    #[test]
    fn rejects_failed_bindings_units_complex_and_oversized_ranges() {
        let bad = definition(1, "x", expr("missing"));
        assert!(evaluate_plot(PlotSpec::default(), &[bad]).is_err());
        for y in ["2m", "sqrt(-1)", "1/(x-x)"] {
            assert!(
                evaluate_plot(
                    PlotSpec {
                        y_expression: y.into(),
                        ..Default::default()
                    },
                    &[]
                )
                .is_err(),
                "{y}"
            );
        }
        let range = tree(
            NodeKind::Range { has_step: false },
            vec![expr("1"), expr("2001")],
        );
        assert!(
            evaluate_plot(PlotSpec::default(), &[definition(1, "x", range)])
                .unwrap_err()
                .contains("2000")
        );
    }

    #[test]
    fn data_axes_take_first_matrix_column_and_truncate_unequal_vectors() {
        let matrix = tree(
            NodeKind::Matrix { rows: 3, cols: 2 },
            vec![
                expr("1"),
                expr("90"),
                expr("2"),
                expr("91"),
                expr("3"),
                expr("92"),
            ],
        );
        let y = tree(
            NodeKind::Matrix { rows: 2, cols: 1 },
            vec![expr("10"), expr("20")],
        );
        let data = evaluate_plot(
            PlotSpec {
                x_expression: "a".into(),
                y_expression: "b".into(),
                ..Default::default()
            },
            &[definition(1, "a", matrix), definition(2, "b", y)],
        )
        .unwrap();
        assert_eq!(data.points, vec![[1., 10.], [2., 20.]]);
    }
    #[test]
    fn resizing_reports_one_undo_command_for_the_whole_drag() {
        let ctx = egui::Context::default();
        crate::configure(&ctx);
        let mut spec = PlotSpec::default();
        let data = Ok(PlotData {
            points: vec![[0., 0.], [1., 1.]],
            implicit_domain: false,
        });
        let render = |ctx: &egui::Context, spec: &mut PlotSpec, events| {
            let mut response = None;
            let output = ctx.run_ui(
                egui::RawInput {
                    events,
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1000., 800.),
                    )),
                    ..Default::default()
                },
                |ui| {
                    response = Some(draw(ui, spec, &data, false));
                },
            );
            output.drop_without_applying_deltas();
            response.unwrap()
        };
        render(&ctx, &mut spec, vec![]);
        let initial = render(&ctx, &mut spec, vec![]);
        let handle = initial.rect.max - egui::vec2(3., 3.);
        let before = (spec.width, spec.height);
        render(&ctx, &mut spec, vec![egui::Event::PointerMoved(handle)]);
        let start = render(
            &ctx,
            &mut spec,
            vec![
                egui::Event::PointerMoved(handle),
                egui::Event::PointerButton {
                    pos: handle,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        let first = render(
            &ctx,
            &mut spec,
            vec![egui::Event::PointerMoved(handle + egui::vec2(20., 10.))],
        );
        let second = render(
            &ctx,
            &mut spec,
            vec![egui::Event::PointerMoved(handle + egui::vec2(40., 20.))],
        );
        let end = render(
            &ctx,
            &mut spec,
            vec![egui::Event::PointerButton {
                pos: handle + egui::vec2(40., 20.),
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        assert!(start.changed());
        assert!(!first.changed());
        assert!(!second.changed());
        assert!(!end.changed());
        assert_eq!((spec.width, spec.height), (before.0 + 40., before.1 + 20.));
    }
}
