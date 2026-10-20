//! Vector formula presentation of authored scaling and inert residual units.
use super::*;
use mathcraft_document::{canvas::PlotScaling, native_plot_units::NativeUnitMonomial};
use mathcraft_editor::Editor;
use std::sync::Arc;
#[derive(Clone)]
struct Source {
    source: PlotScaling,
    editor: Arc<Editor>,
}
#[derive(Clone)]
struct Units {
    units: NativeUnitMonomial,
    editor: Arc<Editor>,
}
fn glyph(canonical: &str) -> &str {
    // Positive shipped DefaultUnitDescriptorTable associations (0170). Ambiguous
    // degree aliases and unknown names stay lexical; these are not engine units.
    match canonical {
        "meter" => "m",
        "kilogram" => "kg",
        "second" => "s",
        "ampere" => "A",
        "kelvin" => "K",
        "mole" => "mol",
        "candela" => "cd",
        "volt" => "V",
        "ohm" => "Ω",
        "henry" => "H",
        "farad" => "F",
        "coulomb" => "C",
        "siemens" => "S",
        "joule" => "J",
        "newton" => "N",
        "hertz" => "Hz",
        "lux" => "lx",
        "weber" => "Wb",
        "tesla" => "T",
        "watt" => "W",
        "pascal" => "Pa",
        "liter" => "L",
        "radian" => "rad",
        "steradian" => "sr",
        "rankine" => "R",
        "deltaC" => "Δ°C",
        "deltaF" => "Δ°F",
        "degC" => "°C",
        "degF" => "°F",
        _ => canonical,
    }
}
fn units_tree(units: &NativeUnitMonomial) -> Result<PortableTree, String> {
    units.validate()?;
    let mut values = Vec::new();
    for reference in &units.references {
        let mut value = tree(
            NodeKind::AnnotatedIdentifier {
                name: glyph(&reference.unit).into(),
                labels: "UNIT".into(),
            },
            vec![],
        );
        if reference.power_numerator.is_some() || reference.power_denominator.is_some() {
            let numerator = reference.power_numerator.unwrap_or(1);
            let mut exponent = tree(NodeKind::Number(numerator.to_string()), vec![]);
            if let Some(denominator) = reference.power_denominator {
                exponent = tree(
                    NodeKind::Fraction,
                    vec![
                        exponent,
                        tree(NodeKind::Number(denominator.to_string()), vec![]),
                    ],
                );
            }
            value = tree(NodeKind::Power, vec![value, exponent]);
        }
        values.push(value);
    }
    let mut values = values.into_iter();
    let mut value = values.next().ok_or("Empty axis units")?;
    for unit in values {
        value = tree(NodeKind::Binary(BinaryOp::Multiply), vec![value, unit]);
    }
    Ok(value)
}
#[derive(Clone)]
struct ExpressionSource {
    tree: PortableTree,
    editor: Arc<Editor>,
}
pub(super) fn expression(
    ui: &mut egui::Ui,
    source: mathcraft_document::canvas::PlotExpressionRef<'_>,
    id: egui::Id,
) {
    match source {
        mathcraft_document::canvas::PlotExpressionRef::LegacyText(text) => {
            ui.add(egui::Label::new(egui::RichText::new(text).color(Color32::BLACK)).truncate());
        }
        mathcraft_document::canvas::PlotExpressionRef::Typed(tree) => {
            let editor = ui.data_mut(|data| {
                if let Some(cached) = data.get_temp::<Arc<ExpressionSource>>(id)
                    && cached.tree == *tree
                {
                    return cached.editor.clone();
                }
                let editor =
                    Arc::new(Editor::from_portable(tree).expect("validated plot first math"));
                data.insert_temp(
                    id,
                    Arc::new(ExpressionSource {
                        tree: tree.clone(),
                        editor: editor.clone(),
                    }),
                );
                editor
            });
            crate::formula::formula_readonly_ui_styled(ui, &editor, None, None, 11.);
        }
    }
}
pub(super) fn source(ui: &mut egui::Ui, scaling: Option<&PlotScaling>, id: egui::Id) {
    let Some(scaling) = scaling else { return };
    let editor = ui.data_mut(|data| {
        if let Some(cached) = data.get_temp::<Arc<Source>>(id)
            && cached.source == *scaling
        {
            return cached.editor.clone();
        }
        let editor =
            Arc::new(Editor::from_portable(scaling.as_tree()).expect("validated plot scaling"));
        data.insert_temp(
            id,
            Arc::new(Source {
                source: scaling.clone(),
                editor: editor.clone(),
            }),
        );
        editor
    });
    crate::formula::formula_readonly_ui_styled(ui, &editor, None, None, 11.);
}
pub(super) fn residual(
    ui: &mut egui::Ui,
    spec: &PlotSpec,
    displayed: &PlotSpec,
    data: &Result<PlotData, String>,
    field: inline_edit::AxisField,
) {
    // Shared source does not authorize sharing cached results. Select the first
    // actual attached curve and require the exact source scaling association.
    let equation = displayed.equation_sources(field.x).nth(field.index);
    let Some(equation) = equation else { return };
    let Some(data) = data.as_ref().ok() else {
        return;
    };
    let result_index = if let Some(id) = field.id {
        let Some(lists) = spec.axis_lists() else {
            return;
        };
        let Some(index) = (if field.x { &lists.x } else { &lists.y })
            .iter()
            .position(|e| e.id == id)
        else {
            return;
        };
        index
    } else {
        if spec.axis_lists().is_some() {
            return;
        }
        field.index
    };
    let axis_count = spec.equation_sources(field.x).count();
    let metadata = data.traces.iter().enumerate().find_map(|(i, outcome)| {
        if i.min(axis_count.saturating_sub(1)) != result_index {
            return None;
        }
        let presentation = outcome.curve()?.presentation.as_ref()?;
        let metadata = if field.x {
            presentation.x.as_ref()?
        } else {
            presentation.y.as_ref()?
        };
        metadata
            .matches_source(equation.scaling)
            .then_some(metadata)
    });
    let Some(units) = metadata.and_then(|m| m.residual_units()) else {
        return;
    };
    let id = ui.id().with(("axis-residual", field.x, field.index));
    let editor = ui.data_mut(|data| {
        if let Some(cached) = data.get_temp::<Arc<Units>>(id)
            && cached.units == *units
        {
            return Some(cached.editor.clone());
        }
        let editor = Arc::new(Editor::from_portable(&units_tree(units).ok()?).ok()?);
        data.insert_temp(
            id,
            Arc::new(Units {
                units: units.clone(),
                editor: editor.clone(),
            }),
        );
        Some(editor)
    });
    if let Some(editor) = editor {
        crate::formula::formula_readonly_ui_styled(ui, &editor, None, None, 11.);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use mathcraft_document::native_plot_units::NativeUnitReference;
    #[test]
    fn residual_units_are_typed_presentation_only_with_exact_rational_powers_and_unknown_lexemes() {
        let units = NativeUnitMonomial {
            system: Some("uninterpretedSystem".into()),
            references: vec![
                NativeUnitReference {
                    unit: "meter".into(),
                    power_numerator: Some(-2),
                    power_denominator: Some(3),
                },
                NativeUnitReference {
                    unit: "unknown𝛼".into(),
                    power_numerator: None,
                    power_denominator: None,
                },
            ],
        };
        let before = units.clone();
        let source = editor_bridge::tree_to_mathml(&units_tree(&units).unwrap()).unwrap();
        assert!(
            source.contains(">m<")
                && source.contains(">-2<")
                && source.contains(">3<")
                && source.contains("unknown𝛼")
        );
        assert_eq!(units, before);
        assert_eq!(glyph("degree"), "degree");
        assert_eq!(glyph("degC"), "°C");
    }
}
