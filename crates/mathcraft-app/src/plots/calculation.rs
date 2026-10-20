//! Plot evaluation against the preceding immutable worksheet context.
use super::*;
#[cfg(test)]
#[path = "calculation/typed_tests.rs"]
mod typed_tests;

/// Evaluate prior placed math plus temporary plot expressions. No source region
/// or variable is changed. The caller supplies the worksheet context preceding
/// this plot (plus any globally scoped definitions it wants visible).
fn evaluate_trace(
    trace: mathcraft_document::canvas::PlotPairRef<'_>,
    context: &mathcraft_compat::CalculationContext,
) -> Result<PlotTraceData, String> {
    if trace.x.expression.as_tree().is_some() || trace.y.expression.as_tree().is_some() {
        return evaluate_typed_trace(trace, context);
    }
    evaluate_legacy_trace(trace, context)
}

fn evaluate_legacy_trace(
    trace: mathcraft_document::canvas::PlotPairRef<'_>,
    context: &mathcraft_compat::CalculationContext,
) -> Result<PlotTraceData, String> {
    let x = mathcraft_core::parse(
        trace
            .x
            .expression
            .as_legacy_text()
            .ok_or("Expected legacy X expression")?,
    )
    .map_err(|e| format!("x축: {e}"))?;
    let y = mathcraft_core::parse(
        trace
            .y
            .expression
            .as_legacy_text()
            .ok_or("Expected legacy Y expression")?,
    )
    .map_err(|e| format!("y축: {e}"))?;
    let x_scale = axis_units::resolve(trace.x.scaling, context).map_err(|e| format!("x축: {e}"))?;
    let y_scale = axis_units::resolve(trace.y.scaling, context).map_err(|e| format!("y축: {e}"))?;
    let initial = query(context, &[evaluation(&x, None)?, evaluation(&y, None)?]);
    let x_result = &initial[0];
    let mut implicit_domain = false;
    let x_samples = if let Some(error) = &x_result.error {
        if matches!(&x, Expr::Name(_))
            && error.code == "bad%_variable"
            && error.previous_region.is_none()
            && !error.unsupported
        {
            if !axis_units::dimensionless(&x_scale.quantity) {
                return Err("물리 단위를 지정한 암시적 x축은 아직 지원하지 않습니다. 물리량 범위를 정의하세요.".into());
            }
            implicit_domain = true;
            axis_units::AxisSamples {
                quantities: (0..500)
                    .map(|i| {
                        Quantity::scalar((-10. + i as f64 * 20. / 499.) * x_scale.quantity.value.re)
                    })
                    .collect(),
                scalar: false,
            }
        } else {
            return Err(format!("x축: {}", fault(error)));
        }
    } else {
        axis_units::samples(x_result.value.as_ref().ok_or("x축 결과가 없습니다.")?)
            .map_err(|e| format!("x축: {e}"))?
    };
    if x_samples.quantities.is_empty() {
        return Err("x축 벡터 또는 범위가 비어 있습니다.".into());
    }
    let y_samples = if let Expr::Name(symbol) = &x
        && (contains_name(&y, symbol) || implicit_domain)
    {
        // Substitute physical quantities with forced UNIT identifiers, retaining
        // dimensional powers even when the worksheet shadows a unit's glyph.
        let expressions = x_samples
            .quantities
            .iter()
            .map(|n| evaluation(&y, Some((symbol, *n))))
            .collect::<Result<Vec<_>, _>>()?;
        let quantities = query(context, &expressions)
            .into_iter()
            .enumerate()
            .map(|(i, result)| {
                if let Some(error) = result.error {
                    return Err(format!(
                        "y축 표본 {} (x={}): {}",
                        i + 1,
                        x_samples.quantities[i].value.re,
                        fault(&error)
                    ));
                }
                match result.value {
                    Some(Value::Scalar { quantity }) => axis_units::checked(quantity),
                    _ => Err("y축의 각 표본은 하나의 실수여야 합니다.".into()),
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        axis_units::AxisSamples {
            quantities,
            scalar: false,
        }
    } else {
        if let Some(error) = &initial[1].error {
            return Err(format!("y축: {}", fault(error)));
        }
        axis_units::samples(initial[1].value.as_ref().ok_or("y축 결과가 없습니다.")?)
            .map_err(|e| format!("y축: {e}"))?
    };
    finish_trace(
        trace,
        &x_samples,
        &y_samples,
        &x_scale,
        &y_scale,
        implicit_domain,
    )
}

fn finish_trace(
    trace: mathcraft_document::canvas::PlotPairRef<'_>,
    x_samples: &axis_units::AxisSamples,
    y_samples: &axis_units::AxisSamples,
    x_scale: &axis_units::ResolvedScale<'_>,
    y_scale: &axis_units::ResolvedScale<'_>,
    implicit_domain: bool,
) -> Result<PlotTraceData, String> {
    if x_samples.quantities.is_empty() {
        return Err("x축 벡터 또는 범위가 비어 있습니다.".into());
    }
    if y_samples.quantities.is_empty() {
        return Err("y축 벡터 또는 범위가 비어 있습니다.".into());
    }
    let (xs, x_presentation) =
        axis_units::coordinates(&x_samples.quantities, x_scale).map_err(|e| format!("x축: {e}"))?;
    let (ys, y_presentation) =
        axis_units::coordinates(&y_samples.quantities, y_scale).map_err(|e| format!("y축: {e}"))?;
    let count = if x_samples.scalar {
        ys.len()
    } else if y_samples.scalar {
        xs.len()
    } else {
        xs.len().min(ys.len())
    };
    let points = (0..count)
        .map(|i| {
            [
                xs[if x_samples.scalar { 0 } else { i }],
                ys[if y_samples.scalar { 0 } else { i }],
            ]
        })
        .collect();
    Ok(PlotTraceData {
        points,
        implicit_domain,
        label: trace.label.to_owned(),
        style: *trace.style,
        presentation: TracePresentation::pair(x_presentation, y_presentation),
    })
}

fn prepared_axis(
    source: mathcraft_document::canvas::PlotExpressionRef<'_>,
    context: &mathcraft_compat::CalculationContext,
) -> Result<mathcraft_compat::PreparedPlotQuery, String> {
    let xml = match source {
        mathcraft_document::canvas::PlotExpressionRef::LegacyText(text) => evaluation(
            &mathcraft_core::parse(text).map_err(|e| e.to_string())?,
            None,
        )?,
        mathcraft_document::canvas::PlotExpressionRef::Typed(source) => {
            // The source tree goes directly to MathML, retaining authored
            // labels, contextual flags, suffixes and operator identities.
            editor_bridge::tree_to_mathml(&tree(
                NodeKind::Binary(BinaryOp::Evaluate),
                vec![source.clone(), tree(NodeKind::Placeholder, vec![])],
            ))?
        }
    };
    context.prepare_query(&xml).map_err(|e| fault(&e))
}

fn evaluate_typed_trace(
    trace: mathcraft_document::canvas::PlotPairRef<'_>,
    context: &mathcraft_compat::CalculationContext,
) -> Result<PlotTraceData, String> {
    let x = prepared_axis(trace.x.expression, context).map_err(|e| format!("x축: {e}"))?;
    let y = prepared_axis(trace.y.expression, context).map_err(|e| format!("y축: {e}"))?;
    let x_scale = axis_units::resolve(trace.x.scaling, context).map_err(|e| format!("x축: {e}"))?;
    let y_scale = axis_units::resolve(trace.y.scaling, context).map_err(|e| format!("y축: {e}"))?;
    let x_value = context.evaluate_prepared(&x);
    let y_value = context.evaluate_prepared(&y);
    // Preserve the existing app's bounded implicit-domain policy only for a
    // resolved VARIABLE with a pure undefined lookup. This does not recover
    // the original sampling domain or bind a synthetic variable into Scope.
    let implicit_domain = matches!(&x_value,Err(error) if error.code=="bad%_variable"
        && error.previous_region.is_none() && !error.unsupported)
        && x.root_symbol()
            .is_some_and(|symbol| symbol.label == "VARIABLE");
    let x_samples = if implicit_domain {
        if !axis_units::dimensionless(&x_scale.quantity) {
            return Err(
                "물리 단위를 지정한 암시적 x축은 아직 지원하지 않습니다. 물리량 범위를 정의하세요."
                    .into(),
            );
        }
        axis_units::AxisSamples {
            quantities: (0..500)
                .map(|i| {
                    Quantity::scalar((-10. + i as f64 * 20. / 499.) * x_scale.quantity.value.re)
                })
                .collect(),
            scalar: false,
        }
    } else {
        axis_units::samples(&x_value.map_err(|e| format!("x축: {}", fault(&e)))?)
            .map_err(|e| format!("x축: {e}"))?
    };
    if x_samples.quantities.is_empty() {
        return Err("x축 벡터 또는 범위가 비어 있습니다.".into());
    }
    let sample_symbol = match x.root_symbol() {
        Some(symbol)
            if symbol.label == "VARIABLE"
                && (y
                    .contains_value_symbol(symbol)
                    .map_err(|e| format!("y축: {}", fault(&e)))?
                    || implicit_domain) =>
        {
            Some(symbol)
        }
        _ => None,
    };
    let y_samples = if let Some(symbol) = sample_symbol {
        let quantities = x_samples
            .quantities
            .iter()
            .enumerate()
            .map(|(i, sample)| {
                let value = context
                    .evaluate_prepared_with_sample(&y, symbol, *sample)
                    .map_err(|e| {
                        format!("y축 표본 {} (x={}): {}", i + 1, sample.value.re, fault(&e))
                    })?;
                match value {
                    Value::Scalar { quantity } => axis_units::checked(quantity),
                    _ => Err("y축의 각 표본은 하나의 실수여야 합니다.".into()),
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        axis_units::AxisSamples {
            quantities,
            scalar: false,
        }
    } else {
        axis_units::samples(&y_value.map_err(|e| format!("y축: {}", fault(&e)))?)
            .map_err(|e| format!("y축: {e}"))?
    };
    finish_trace(
        trace,
        &x_samples,
        &y_samples,
        &x_scale,
        &y_scale,
        implicit_domain,
    )
}

#[cfg(test)]
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
    let xml = editor_bridge::worksheet_xml(prior)?;
    let mut calculator = mathcraft_compat::WorksheetCalculator::default();
    calculator.update(&xml, None)?;
    calculator.calculate(mathcraft_compat::CalculationRequest::Worksheet)?;
    let context = calculator.context_before(left, top)?;
    evaluate_in_context(spec, &context)
}

/// Evaluate axes against existing computed bindings. This never reruns worksheet
/// input for each trace or sample, and cannot mutate its source environment.
pub(crate) fn evaluate_in_context(
    spec: &PlotSpec,
    context: &mathcraft_compat::CalculationContext,
) -> Result<PlotData, String> {
    spec.validate()?;
    let mut traces = Vec::with_capacity(spec.trace_count());
    let mut total_points = 0usize;
    // Sharing an authored axis does not memoize its evaluation. Keep the
    // existing per-pair evaluator semantics until once-evaluation is proven.
    for trace in spec.paired_sources() {
        let incomplete = |source: mathcraft_document::canvas::PlotExpressionRef<'_>| {
            source.is_exact_placeholder() || source.as_tree().is_some() && !source.is_complete()
        };
        let outcome = if incomplete(trace.x.expression)
            || incomplete(trace.y.expression)
            || [trace.x.scaling, trace.y.scaling]
                .into_iter()
                .flatten()
                .any(|s| !s.is_complete())
        {
            // An editor/source placeholder is not a fabricated engine value or
            // error. Explicit native PH engine semantics remain unproven (0163).
            TraceOutcome::Incomplete
        } else {
            match evaluate_trace(trace, context) {
                Ok(data) => {
                    total_points = total_points
                        .checked_add(data.points.len())
                        .ok_or("도표 표본 수 한계")?;
                    if total_points > MAX_SAMPLES * PlotSpec::MAX_TRACES {
                        return Err("도표 전체 표본 수가 지원 한계를 넘었습니다.".into());
                    }
                    TraceOutcome::Ready(data)
                }
                Err(error) => TraceOutcome::Failed(error),
            }
        };
        traces.push(outcome);
    }
    Ok(PlotData { traces })
}
