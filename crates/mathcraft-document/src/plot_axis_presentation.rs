//! Read-only axis result association, never an evaluator environment or source.
use crate::{canvas::PlotScaling, native_plot_units::NativeUnitMonomial};
use serde::{Deserialize, Deserializer, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "origin", rename_all = "snake_case")]
pub enum PlotAxisPresentation {
    Calculated {
        source_scaling: Option<PlotScaling>,
        effective_factor: f64,
        effective_dimension: [f64; 7],
        residual_units: Option<NativeUnitMonomial>,
    },
    Native {
        source_scaling: Option<PlotScaling>,
        residual_units: Option<NativeUnitMonomial>,
    },
}
fn required_option<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<Option<T>, D::Error> {
    Option::deserialize(d)
}
#[derive(Deserialize)]
#[serde(tag = "origin", rename_all = "snake_case", deny_unknown_fields)]
enum Wire {
    Calculated {
        #[serde(deserialize_with = "required_option")]
        source_scaling: Option<PlotScaling>,
        effective_factor: f64,
        effective_dimension: [f64; 7],
        #[serde(deserialize_with = "required_option")]
        residual_units: Option<NativeUnitMonomial>,
    },
    Native {
        #[serde(deserialize_with = "required_option")]
        source_scaling: Option<PlotScaling>,
        #[serde(deserialize_with = "required_option")]
        residual_units: Option<NativeUnitMonomial>,
    },
}
impl<'de> Deserialize<'de> for PlotAxisPresentation {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let value = match Wire::deserialize(d)? {
            Wire::Calculated {
                source_scaling,
                effective_factor,
                effective_dimension,
                residual_units,
            } => Self::Calculated {
                source_scaling,
                effective_factor,
                effective_dimension,
                residual_units,
            },
            Wire::Native {
                source_scaling,
                residual_units,
            } => Self::Native {
                source_scaling,
                residual_units,
            },
        };
        value.validate().map_err(serde::de::Error::custom)?;
        Ok(value)
    }
}
impl PlotAxisPresentation {
    pub fn validate(&self) -> Result<(), String> {
        if let Self::Calculated {
            effective_factor,
            effective_dimension,
            ..
        } = self
            && (!effective_factor.is_finite()
                || *effective_factor <= 0.
                || effective_dimension
                    .iter()
                    .any(|d| !d.is_finite() || d.abs() > 1e6))
        {
            return Err(
                "Axis presentation requires a finite positive factor and bounded dimensions".into(),
            );
        }
        if let Some(units) = self.residual_units() {
            units.validate()?;
        }
        Ok(())
    }
    pub fn source_scaling(&self) -> Option<&PlotScaling> {
        match self {
            Self::Calculated { source_scaling, .. } | Self::Native { source_scaling, .. } => {
                source_scaling.as_ref()
            }
        }
    }
    pub fn matches_source(&self, source: Option<&PlotScaling>) -> bool {
        self.source_scaling() == source
    }
    pub fn residual_units(&self) -> Option<&NativeUnitMonomial> {
        match self {
            Self::Calculated { residual_units, .. } | Self::Native { residual_units, .. } => {
                residual_units.as_ref()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native_plot_units::NativeUnitReference;
    fn scaling(labels: &str, contextual: Option<bool>) -> PlotScaling {
        PlotScaling::from_tree(mathcraft_editor::PortableTree {
            kind: mathcraft_editor::NodeKind::NativeIdentifier {
                name: "m".into(),
                labels: labels.into(),
                contextual,
                subscript: Some("dc𝛼".into()),
            },
            children: vec![],
        })
        .unwrap()
        .unwrap()
    }
    #[test]
    fn native_association_is_inert_exact_and_does_not_infer_units_from_source() {
        let unit = scaling("UNIT", None);
        let variable = scaling("VARIABLE", None);
        let contextual = scaling("UNIT", Some(true));
        let presentation = PlotAxisPresentation::Native {
            source_scaling: Some(unit.clone()),
            residual_units: Some(NativeUnitMonomial {
                system: Some("original-system".into()),
                references: vec![NativeUnitReference {
                    unit: "unknown-native-unit𝛼".into(),
                    power_numerator: None,
                    power_denominator: Some(3),
                }],
            }),
        };
        presentation.validate().unwrap();
        assert!(presentation.matches_source(Some(&unit)));
        assert!(
            !presentation.matches_source(Some(&variable))
                && !presentation.matches_source(Some(&contextual))
                && !presentation.matches_source(None)
        );
        let source = serde_json::to_string(&presentation).unwrap();
        let reopened: PlotAxisPresentation = serde_json::from_str(&source).unwrap();
        assert_eq!(reopened, presentation);
        assert_eq!(
            reopened.residual_units().unwrap().references[0].unit,
            "unknown-native-unit𝛼"
        );
        assert!(
            PlotAxisPresentation::Native {
                source_scaling: None,
                residual_units: None
            }
            .matches_source(None)
        );
    }
    #[test]
    fn required_null_keys_unknowns_and_all_duplicates_are_strict() {
        let valid = r#"{"origin":"native","source_scaling":null,"residual_units":null}"#;
        assert!(serde_json::from_str::<PlotAxisPresentation>(valid).is_ok());
        for bad in [
            valid.replace("\"source_scaling\":null,", ""),
            valid.replace(",\"residual_units\":null", ""),
            valid.replace("\"origin\":", "\"unknown\":0,\"origin\":"),
            valid.replace(
                "\"origin\":\"native\"",
                "\"origin\":\"native\",\"origin\":\"native\"",
            ),
            valid.replace(
                "\"source_scaling\":null",
                "\"source_scaling\":null,\"source_scaling\":null",
            ),
            valid.replace(
                "\"residual_units\":null",
                "\"residual_units\":null,\"residual_units\":null",
            ),
            valid.replace("\"native\"", "\"unknown\""),
            valid.replace(
                "\"residual_units\":null",
                "\"residual_units\":{\"system\":null,\"references\":[]}",
            ),
        ] {
            assert!(
                serde_json::from_str::<PlotAxisPresentation>(&bad).is_err(),
                "{bad}"
            );
        }
    }
    #[test]
    fn calculated_factor_dimensions_and_native_metadata_are_bounded_on_wire_and_manual_values() {
        let make = |factor, dimension| PlotAxisPresentation::Calculated {
            source_scaling: None,
            effective_factor: factor,
            effective_dimension: dimension,
            residual_units: None,
        };
        let valid = make(f64::MIN_POSITIVE, [-0., 1e6, -1e6, 0., 0., 0., 0.]);
        valid.validate().unwrap();
        let wire = serde_json::to_string(&valid).unwrap();
        let reopened: PlotAxisPresentation = serde_json::from_str(&wire).unwrap();
        if let PlotAxisPresentation::Calculated {
            effective_dimension,
            ..
        } = reopened
        {
            assert_eq!(effective_dimension[0].to_bits(), (-0_f64).to_bits());
        } else {
            panic!()
        }
        for factor in [-0., 0., -1., f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(make(factor, [0.; 7]).validate().is_err());
        }
        for dimension in [1_000_001., f64::NAN, f64::INFINITY] {
            let mut dims = [0.; 7];
            dims[3] = dimension;
            assert!(make(1., dims).validate().is_err());
        }
        for bad in [
            wire.replace(
                "\"effective_factor\":",
                "\"effective_factor\":1,\"effective_factor\":",
            ),
            wire.replace(
                "\"effective_dimension\":",
                "\"effective_dimension\":[0,0,0,0,0,0,0],\"effective_dimension\":",
            ),
            wire.replace("[-0.0,1000000.0,-1000000.0,0.0,0.0,0.0,0.0]", "[0,0]"),
            wire.replace("2.2250738585072014e-308", "1e309"),
        ] {
            assert!(
                serde_json::from_str::<PlotAxisPresentation>(&bad).is_err(),
                "{bad}"
            );
        }
    }
}
