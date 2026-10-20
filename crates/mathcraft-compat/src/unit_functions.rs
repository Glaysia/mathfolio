//! Bounded finite-real scalar UNIT functions.
//!
//! Installed SI forward/inverse wrappers and source evidence: analysis/0190.
//! This initial capability boundary does not assert that the original rejects
//! matrices, ranges or complex arguments. Delta units remain ordinary units.
use super::{Fault, Quantity, Result, Symbol, Value};

#[derive(Clone, Copy)]
enum TemperatureUnit {
    Celsius,
    Fahrenheit,
}

fn identify(name: &Symbol) -> Option<TemperatureUnit> {
    if name.label != "UNIT" {
        return None;
    }
    match name.name.as_str() {
        "°C" => Some(TemperatureUnit::Celsius),
        "°F" => Some(TemperatureUnit::Fahrenheit),
        _ => None,
    }
}

pub(super) fn is_builtin(name: &Symbol) -> bool {
    identify(name).is_some()
}

fn real_scalar(value: &Value) -> Result<Quantity> {
    let Value::Scalar { quantity } = value else {
        return Err(Fault::unsupported(
            "Affine temperature conversion currently supports only real scalars",
        ));
    };
    if quantity.value.im != 0.
        || !quantity.value.re.is_finite()
        || !quantity.dimension.iter().all(|power| power.is_finite())
    {
        return Err(Fault::unsupported(
            "Affine temperature conversion requires a finite real scalar",
        ));
    }
    Ok(*quantity)
}

/// Keep the installed SI operation order: Ffactor=(5/9)*K,
/// Coffset=273.15*K, Foffset=Coffset-32*Ffactor.
fn coefficients(unit: TemperatureUnit) -> Result<(Quantity, Quantity)> {
    let kelvin =
        mathcraft_core::unit("K").ok_or_else(|| Fault::unsupported("Kelvin unit unavailable"))?;
    let c_offset = Quantity::scalar(273.15)
        .multiply(kelvin, false)
        .map_err(super::scalars::core_error)?;
    match unit {
        TemperatureUnit::Celsius => Ok((kelvin, c_offset)),
        TemperatureUnit::Fahrenheit => {
            let factor = Quantity::scalar(5. / 9.)
                .multiply(kelvin, false)
                .map_err(super::scalars::core_error)?;
            let offset = c_offset
                .add(
                    Quantity::scalar(32.)
                        .multiply(factor, false)
                        .map_err(super::scalars::core_error)?,
                    true,
                )
                .map_err(super::scalars::core_error)?;
            Ok((factor, offset))
        }
    }
}

pub(super) fn forward(name: &Symbol, args: &[Value]) -> Result<Value> {
    let unit = identify(name).ok_or_else(|| Fault::unknown(name))?;
    let [argument] = args else {
        return Err(Fault::unsupported(
            "Affine temperature function requires one argument",
        ));
    };
    let input = real_scalar(argument)?;
    if !input.is_scalar() {
        return Err(Fault::new(
            "bad%_dimensions",
            "Affine temperature input must be dimensionless",
        ));
    }
    let (factor, offset) = coefficients(unit)?;
    let canonical = input
        .multiply(factor, false)
        .and_then(|scaled| scaled.add(offset, false))
        .map_err(super::scalars::core_error)?;
    Ok(Value::scalar(canonical))
}

pub(super) fn inverse_display(name: &Symbol, value: &Value) -> Result<Value> {
    let unit = identify(name).ok_or_else(|| Fault::unknown(name))?;
    let canonical = real_scalar(value)?;
    let (factor, offset) = coefficients(unit)?;
    if !canonical.same_dimension(&factor) {
        return Err(Fault::new(
            "bad%_dimensions",
            "Affine temperature display unit has incompatible dimensions",
        ));
    }
    let displayed = canonical
        .add(offset, true)
        .and_then(|shifted| shifted.multiply(factor, true))
        .map_err(super::scalars::core_error)?;
    Ok(Value::scalar(displayed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use num_complex::Complex64;

    fn symbol(name: &str, label: &str) -> Symbol {
        Symbol {
            name: name.into(),
            label: label.into(),
        }
    }

    #[test]
    fn labelled_identity_does_not_capture_variables_aliases_or_literal_suffixes() {
        assert!(is_builtin(&symbol("°C", "UNIT")));
        assert!(is_builtin(&symbol("°F", "UNIT")));
        for name in [
            symbol("°C", "VARIABLE"),
            symbol("°C", "FUNCTION"),
            symbol("°C\u{1f}dc", "UNIT"),
            symbol("degC", "UNIT"),
            symbol("℃", "UNIT"),
        ] {
            assert!(!is_builtin(&name));
        }
    }

    #[test]
    fn unsupported_shapes_are_not_shifted_elementwise_or_as_a_range_step() {
        let scalar = Quantity::scalar(1.);
        let values = [
            Value::Matrix {
                rows: 1,
                cols: 1,
                elements: vec![scalar],
            },
            Value::Range {
                start: scalar,
                step: scalar,
                end: scalar,
                count: 1,
            },
            Value::RangeValues {
                elements: vec![scalar],
            },
            Value::scalar(Quantity {
                value: Complex64::new(1., 2.),
                dimension: [0.; 7],
            }),
            Value::scalar(Quantity::scalar(f64::INFINITY)),
        ];
        for value in &values {
            assert!(
                forward(&symbol("°C", "UNIT"), std::slice::from_ref(value))
                    .unwrap_err()
                    .unsupported
            );
            assert!(
                inverse_display(&symbol("°F", "UNIT"), value)
                    .unwrap_err()
                    .unsupported
            );
        }
    }

    #[test]
    fn argument_count_and_dimensions_fail_without_a_fabricated_conversion() {
        let name = symbol("°C", "UNIT");
        assert!(forward(&name, &[]).is_err());
        assert!(
            forward(
                &name,
                &[
                    Value::scalar(Quantity::scalar(1.)),
                    Value::scalar(Quantity::scalar(1.))
                ]
            )
            .is_err()
        );
        let meter = Value::scalar(mathcraft_core::unit("m").unwrap());
        assert_eq!(
            forward(&name, std::slice::from_ref(&meter))
                .unwrap_err()
                .code,
            "bad%_dimensions"
        );
        assert_eq!(
            inverse_display(&name, &meter).unwrap_err().code,
            "bad%_dimensions"
        );
    }

    #[test]
    fn conversion_does_not_invent_an_absolute_zero_domain_check() {
        let name = symbol("°C", "UNIT");
        let value = forward(&name, &[Value::scalar(Quantity::scalar(-300.))]).unwrap();
        let Value::Scalar { quantity } = value else {
            panic!("not scalar")
        };
        assert!((quantity.value.re - -26.85).abs() < 1e-11);
        assert_eq!(
            quantity.dimension,
            mathcraft_core::unit("K").unwrap().dimension
        );
    }
}
