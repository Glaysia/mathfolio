//! Physical samples and user scaling remain separate from plot coordinates.
//! Automatic integer SI products are an independent bounded engine policy.
use super::{MAX_SAMPLES, tree};
use mathcraft_compat::{CalculationContext, Value, editor_bridge};
use mathcraft_core::Quantity;
use mathcraft_document::{
    canvas::PlotScaling,
    native_plot_units::{NativeUnitMonomial, NativeUnitReference},
    plot_axis_presentation::PlotAxisPresentation,
};
use mathcraft_editor::{BinaryOp, NodeKind, PortableTree};
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TracePresentation {
    pub x: Option<Box<PlotAxisPresentation>>,
    pub y: Option<Box<PlotAxisPresentation>>,
}
impl TracePresentation {
    pub(crate) fn pair(
        x: Option<Box<PlotAxisPresentation>>,
        y: Option<Box<PlotAxisPresentation>>,
    ) -> Option<Arc<Self>> {
        (x.is_some() || y.is_some()).then(|| Arc::new(Self { x, y }))
    }
    pub(super) fn validate(&self) -> Result<(), String> {
        for axis in [&self.x, &self.y].into_iter().flatten() {
            axis.validate()?;
        }
        Ok(())
    }
}
pub(super) struct AxisSamples {
    pub quantities: Vec<Quantity>,
    pub scalar: bool,
}
pub(super) fn checked(quantity: Quantity) -> Result<Quantity, String> {
    if quantity.value.im != 0. {
        return Err("도표 축에 복소수를 사용할 수 없습니다.".into());
    }
    if !quantity.value.re.is_finite() {
        return Err("도표 축에 유한하지 않은 값이 있습니다.".into());
    }
    if quantity
        .dimension
        .iter()
        .any(|d| !d.is_finite() || d.abs() > 1e6)
    {
        return Err("도표 축 차원이 지원 범위를 벗어났습니다.".into());
    }
    Ok(quantity)
}
pub(super) fn samples(value: &Value) -> Result<AxisSamples, String> {
    let scalar = matches!(value, Value::Scalar { .. });
    let quantities = match value {
        Value::Scalar { quantity } => vec![checked(*quantity)?],
        Value::Matrix {
            rows,
            cols,
            elements,
        } => {
            if *rows > MAX_SAMPLES {
                return Err("도표 표본은 최대 2000개입니다.".into());
            }
            if *rows == 0 || *cols == 0 {
                Vec::new()
            } else {
                elements
                    .chunks(*cols)
                    .map(|row| checked(row[0]))
                    .collect::<Result<Vec<_>, _>>()?
            }
        }
        Value::RangeValues { elements } => {
            if elements.len() > MAX_SAMPLES {
                return Err("도표 표본은 최대 2000개입니다.".into());
            }
            elements
                .iter()
                .copied()
                .map(checked)
                .collect::<Result<Vec<_>, _>>()?
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
            let (start, step, end) = (checked(*start)?, checked(*step)?, checked(*end)?);
            if !start.same_dimension(&step) || !start.same_dimension(&end) {
                return Err("도표 범위의 시작·간격·끝 단위가 다릅니다.".into());
            }
            (0..*count)
                .map(|index| {
                    let mut value = start;
                    value.value.re = start.value.re + index as f64 * step.value.re;
                    checked(value)
                })
                .collect::<Result<Vec<_>, _>>()?
        }
        Value::String { .. } | Value::Symbolic { .. } => {
            return Err("도표 축에는 수치 값이 필요합니다.".into());
        }
    };
    Ok(AxisSamples { quantities, scalar })
}
pub(super) struct ResolvedScale<'a> {
    pub quantity: Quantity,
    pub source: Option<&'a PlotScaling>,
}
pub(super) fn resolve<'a>(
    source: Option<&'a PlotScaling>,
    context: &CalculationContext,
) -> Result<ResolvedScale<'a>, String> {
    let quantity = if let Some(source) = source {
        if !source.is_complete() {
            return Err("단위/배율 식이 아직 완성되지 않았습니다.".into());
        }
        let xml = editor_bridge::tree_to_mathml(&tree(
            NodeKind::Binary(BinaryOp::Evaluate),
            vec![
                source.as_tree().clone(),
                tree(NodeKind::Placeholder, vec![]),
            ],
        ))?;
        match context
            .evaluate_mathml(&xml)
            .map_err(|fault| super::fault(&fault))?
        {
            Value::Scalar { quantity } => checked(quantity)?,
            _ => return Err("도표 단위/배율은 하나의 실수 물리량이어야 합니다.".into()),
        }
    } else {
        Quantity::scalar(1.)
    };
    if quantity.value.re <= 0. {
        return Err("현재 도표 단위/배율은 유한한 양수만 지원합니다.".into());
    }
    Ok(ResolvedScale { quantity, source })
}
pub(super) fn dimensionless(quantity: &Quantity) -> bool {
    quantity.dimension.iter().all(|d| *d == 0.)
}
// Canonical names/SI glyph associations are independently extracted facts from
// DefaultUnitDescriptorTable, not inferred from display glyphs (0170 follow-up).
const SI_BASE: [(&str, &str); 7] = [
    ("meter", "m"),
    ("kilogram", "kg"),
    ("second", "s"),
    ("ampere", "A"),
    ("kelvin", "K"),
    ("mole", "mol"),
    ("candela", "cd"),
];
fn si_units(dimension: [f64; 7]) -> Result<Option<NativeUnitMonomial>, String> {
    let mut references = Vec::new();
    for ((canonical, _), power) in SI_BASE.into_iter().zip(dimension) {
        if power == 0. {
            continue;
        }
        if !power.is_finite() || power.fract() != 0. || power.abs() > 1e6 {
            return Err(
                "자동 도표 단위는 현재 정수 SI 기본 차원만 지원합니다. 단위를 직접 지정하세요."
                    .into(),
            );
        }
        references.push(NativeUnitReference {
            unit: canonical.into(),
            power_numerator: (power != 1.).then_some(power as i32),
            power_denominator: None,
        });
    }
    if references.is_empty() {
        Ok(None)
    } else {
        let units = NativeUnitMonomial {
            system: None,
            references,
        };
        units.validate()?;
        Ok(Some(units))
    }
}
pub(super) fn coordinates(
    values: &[Quantity],
    scale: &ResolvedScale<'_>,
) -> Result<(Vec<f64>, Option<Box<PlotAxisPresentation>>), String> {
    let first = *values
        .first()
        .ok_or("도표 축 벡터 또는 범위가 비어 있습니다.")?;
    if values.iter().any(|q| !first.same_dimension(q)) {
        return Err("도표의 한 축 표본에 서로 다른 단위 차원이 섞여 있습니다.".into());
    }
    let residual_units = if dimensionless(&scale.quantity) {
        si_units(first.dimension)?
    } else if first.same_dimension(&scale.quantity) {
        None
    } else {
        return Err("도표 축과 지정한 단위/배율의 차원이 다릅니다.".into());
    };
    let coordinates = values
        .iter()
        .map(|q| {
            checked(*q)?;
            let value = q.value.re / scale.quantity.value.re;
            if value.is_finite() {
                Ok(value)
            } else {
                Err("변환한 도표 좌표가 유한하지 않습니다.".into())
            }
        })
        .collect::<Result<Vec<_>, String>>()?;
    let presentation = if scale.source.is_none() && dimensionless(&first) {
        None
    } else {
        let metadata = PlotAxisPresentation::Calculated {
            source_scaling: scale.source.cloned(),
            effective_factor: scale.quantity.value.re,
            effective_dimension: first.dimension,
            residual_units,
        };
        metadata.validate()?;
        Some(Box::new(metadata))
    };
    Ok((coordinates, presentation))
}
/// Physical point substitution uses forced UNIT identifiers. Variable bindings
/// named m/s/etc cannot silently change a sample's dimension.
pub(super) fn quantity_tree(quantity: Quantity) -> Result<PortableTree, String> {
    let quantity = checked(quantity)?;
    let mut value = tree(NodeKind::Number(quantity.value.re.to_string()), vec![]);
    for ((_, glyph), power) in SI_BASE.into_iter().zip(quantity.dimension) {
        if power == 0. {
            continue;
        }
        let mut unit = tree(
            NodeKind::AnnotatedIdentifier {
                name: glyph.into(),
                labels: "UNIT".into(),
            },
            vec![],
        );
        if power != 1. {
            unit = tree(
                NodeKind::Power,
                vec![unit, tree(NodeKind::Number(power.to_string()), vec![])],
            );
        }
        value = tree(NodeKind::Binary(BinaryOp::Multiply), vec![value, unit]);
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn qty(value: f64, unit: &str) -> Quantity {
        Quantity::scalar(value)
            .multiply(mathcraft_core::unit(unit).unwrap(), false)
            .unwrap()
    }
    #[test]
    fn physical_vectors_and_linear_scale_keep_quantities_until_conversion() {
        let values = Value::Matrix {
            rows: 2,
            cols: 1,
            elements: vec![qty(1., "m"), qty(2., "m")],
        };
        let samples = samples(&values).unwrap();
        assert!(!samples.scalar && samples.quantities[1].dimension[0] == 1.);
        let inches = ResolvedScale {
            quantity: qty(1., "in"),
            source: None,
        };
        let (points, metadata) = coordinates(&samples.quantities, &inches).unwrap();
        assert_eq!(points, vec![1. / 0.0254, 2. / 0.0254]);
        assert!(metadata.unwrap().residual_units().is_none());
        let (points, _) = coordinates(
            &samples.quantities,
            &ResolvedScale {
                quantity: qty(0.1, "m"),
                source: None,
            },
        )
        .unwrap();
        assert_eq!(points, vec![10., 20.]);
    }
    #[test]
    fn auto_si_is_residual_metadata_and_does_not_become_user_source() {
        let (points, metadata) = coordinates(
            &[qty(1., "m"), qty(2., "m")],
            &ResolvedScale {
                quantity: Quantity::scalar(0.1),
                source: None,
            },
        )
        .unwrap();
        assert_eq!(points, vec![10., 20.]);
        let metadata = metadata.unwrap();
        assert!(metadata.source_scaling().is_none());
        assert_eq!(
            metadata.residual_units().unwrap().references[0].unit,
            "meter"
        );
        assert!(
            coordinates(
                &[qty(1., "m"), qty(2., "s")],
                &ResolvedScale {
                    quantity: Quantity::scalar(1.),
                    source: None
                }
            )
            .is_err()
        );
    }
    #[test]
    fn physical_substitution_tree_cannot_be_shadowed_by_variable_unit_names() {
        let xml = editor_bridge::tree_to_mathml(&quantity_tree(qty(2., "m")).unwrap()).unwrap();
        assert!(xml.contains("labels=\"UNIT\"") && xml.contains(">m<"));
        let (_, metadata) = coordinates(
            &[Quantity::scalar(-0.)],
            &ResolvedScale {
                quantity: Quantity::scalar(1.),
                source: None,
            },
        )
        .unwrap();
        assert!(metadata.is_none());
    }
}
