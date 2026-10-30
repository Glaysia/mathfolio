//! Worksheet/local math presentation, separate from semantic MathML.
use crate::canvas::{ComplexFormat, MathFormat, ResultFormat};
use crate::native::NativePackage;
use roxmltree::Node;
const WS: &str = "http://schemas.mathsoft.com/worksheet50";
const PRESENTATION: &str = "http://schemas.ptc.com/mathcad/settings/presentation10";

pub fn read_math_format(math: Node<'_, '_>) -> MathFormat {
    read_math_format_with_defaults(math, MathFormat::default())
}

/// Missing local attributes inherit the worksheet's effective defaults.
pub fn read_math_format_with_defaults(math: Node<'_, '_>, mut style: MathFormat) -> MathFormat {
    style.explicit = false;
    if let Some(format) = math
        .children()
        .find(|n| n.has_tag_name((WS, "formattingOverride")))
    {
        read_font(format, &mut style);
    }
    if let Some(format) = math
        .children()
        .find(|n| n.has_tag_name((WS, "resultFormat")))
        && let Some(number) = number_node(format)
    {
        read_number(number, &mut style);
    }
    style
}

fn read_font(node: Node<'_, '_>, style: &mut MathFormat) {
    if let Some(points) = node
        .attribute("FontSize")
        .and_then(|n| n.parse::<f32>().ok())
        && points.is_finite()
        && (1. ..=200.).contains(&points)
    {
        style.font_points = points;
        style.explicit = true;
    }
}

/// Presentation FontSize is points, unlike mathStyle/font-size in DIP.
pub fn read_presentation_math_format(presentation: Node<'_, '_>) -> MathFormat {
    let mut style = MathFormat::default();
    if let Some(rendering) = presentation
        .children()
        .find(|n| n.has_tag_name((PRESENTATION, "mathRendering")))
    {
        if let Some(font) = rendering
            .children()
            .find(|n| n.has_tag_name((PRESENTATION, "formatting")))
        {
            read_font(font, &mut style);
        }
        if let Some(results) = rendering
            .children()
            .find(|n| n.has_tag_name((WS, "results")))
            && let Some(number) = number_node(results)
        {
            read_number(number, &mut style);
        }
    }
    style.explicit = false;
    style
}

pub fn read_worksheet_math_format_defaults(package: &NativePackage) -> Result<MathFormat, String> {
    let Some(xml) = package
        .original_xml_part("mathcad/settings/presentation.xml")
        .map_err(|e| e.to_string())?
    else {
        return Ok(MathFormat::default());
    };
    let document = roxmltree::Document::parse_with_options(
        &xml,
        roxmltree::ParsingOptions {
            allow_dtd: false,
            nodes_limit: 250_000,
            ..Default::default()
        },
    )
    .map_err(|e| e.to_string())?;
    Ok(read_presentation_math_format(document.root_element()))
}

pub(crate) fn number_node<'a, 'input>(format: Node<'a, 'input>) -> Option<Node<'a, 'input>> {
    format.children().find(|n| {
        n.tag_name().namespace() == Some(WS) && !matches!(n.tag_name().name(), "matrix" | "unit")
    })
}

fn read_number(number: Node<'_, '_>, style: &mut MathFormat) {
    style.result_format = match number.tag_name().name() {
        "general" => ResultFormat::General,
        "decimal" => ResultFormat::Decimal,
        "scientific" => ResultFormat::Scientific,
        "engineering" => ResultFormat::Engineering,
        "percent" => ResultFormat::Percent,
        // Fraction and round-trip metadata remain in the preserved source.
        _ => return,
    };
    if let Some(value) = number
        .attribute("precision")
        .and_then(|n| n.parse::<u8>().ok())
        .filter(|n| *n <= 15)
    {
        style.precision = value;
    }
    for (name, target) in [
        ("show-trailing-zeros", &mut style.show_trailing_zeros),
        ("use-e-notation", &mut style.use_e_notation),
        ("use-engineering-format", &mut style.use_engineering_format),
    ] {
        if let Some(value) = number.attribute(name).and_then(|n| match n {
            "true" | "1" => Some(true),
            "false" | "0" => Some(false),
            _ => None,
        }) {
            *target = value;
        }
    }
    if let Some(value) = number
        .attribute("exponential-threshold")
        .and_then(|n| n.parse().ok())
        .filter(|n| *n <= 17)
    {
        style.exponential_threshold = value;
    }
    if let Some(value) = number
        .attribute("zero-threshold")
        .and_then(|n| n.parse().ok())
        .filter(|n| *n <= 307)
    {
        style.zero_threshold = value;
    }
    if let Some(value) = number
        .attribute("complex-threshold")
        .and_then(|n| n.parse().ok())
        .filter(|n| *n <= 307)
    {
        style.complex_threshold = value;
    }
    if let Some(value) = number
        .attribute("imaginary-value")
        .and_then(|value| match value {
            "i" => Some(ComplexFormat::I),
            "j" => Some(ComplexFormat::J),
            "polarRadians" => Some(ComplexFormat::PolarRadians),
            "polarPiRadians" => Some(ComplexFormat::PolarPiRadians),
            "polarDegrees" => Some(ComplexFormat::PolarDegrees),
            _ => None,
        })
    {
        style.complex_format = value;
    }
    style.explicit = true;
}

pub(crate) fn style_name(style: ResultFormat) -> &'static str {
    match style {
        ResultFormat::General => "general",
        ResultFormat::Decimal => "decimal",
        ResultFormat::Scientific => "scientific",
        ResultFormat::Engineering => "engineering",
        ResultFormat::Percent => "percent",
    }
}
pub(crate) fn complex_name(style: ComplexFormat) -> &'static str {
    match style {
        ComplexFormat::I => "i",
        ComplexFormat::J => "j",
        ComplexFormat::PolarRadians => "polarRadians",
        ComplexFormat::PolarPiRadians => "polarPiRadians",
        ComplexFormat::PolarDegrees => "polarDegrees",
    }
}

pub(crate) fn number_attributes(style: &MathFormat) -> Vec<(&'static str, String)> {
    let mut attrs = vec![
        ("precision", style.precision.to_string()),
        ("show-trailing-zeros", style.show_trailing_zeros.to_string()),
        ("radix", "dec".into()),
        ("imaginary-value", complex_name(style.complex_format).into()),
    ];
    match style.result_format {
        ResultFormat::General => attrs.extend([
            ("zero-threshold", style.zero_threshold.to_string()),
            (
                "exponential-threshold",
                style.exponential_threshold.to_string(),
            ),
            ("complex-threshold", style.complex_threshold.to_string()),
            // XSD-backed General option; the observed Prime writer omits it.
            (
                "use-engineering-format",
                style.use_engineering_format.to_string(),
            ),
        ]),
        ResultFormat::Scientific | ResultFormat::Engineering => {
            attrs.push(("use-e-notation", style.use_e_notation.to_string()))
        }
        ResultFormat::Decimal | ResultFormat::Percent => {}
    }
    // Common XSD attributes omitted by observed Prime writers outside General;
    // retain explicitly nondefault supported values rather than losing them.
    if style.result_format != ResultFormat::General {
        if style.zero_threshold != MathFormat::default().zero_threshold {
            attrs.push(("zero-threshold", style.zero_threshold.to_string()));
        }
        if style.complex_threshold != MathFormat::default().complex_threshold {
            attrs.push(("complex-threshold", style.complex_threshold.to_string()));
        }
    }
    attrs
}

pub(crate) fn number_xml(style: &MathFormat) -> String {
    let attrs = number_attributes(style)
        .into_iter()
        .map(|(k, v)| format!(" {k}=\"{v}\""))
        .collect::<String>();
    format!("<{}{attrs}/>", style_name(style.result_format))
}
pub(crate) fn number_format_eq(left: &MathFormat, right: &MathFormat) -> bool {
    let mut left = *left;
    left.font_points = right.font_points;
    left.explicit = right.explicit;
    left == *right
}
pub(crate) fn local_xml(style: &MathFormat) -> String {
    if !style.explicit {
        return String::new();
    }
    format!(
        "<resultFormat>{}</resultFormat><formattingOverride FontSize=\"{}\"/>",
        number_xml(style),
        style.font_points
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn local_presentation_roundtrips_in_point_units_outside_mathml() {
        let style = MathFormat {
            font_points: 18.,
            precision: 7,
            result_format: ResultFormat::Engineering,
            use_e_notation: true,
            show_trailing_zeros: true,
            complex_format: ComplexFormat::PolarDegrees,
            explicit: true,
            ..Default::default()
        };
        let xml = format!("<math xmlns=\"{WS}\">{}</math>", local_xml(&style));
        let doc = roxmltree::Document::parse(&xml).unwrap();
        assert_eq!(read_math_format(doc.root_element()), style);
        assert!(local_xml(&MathFormat::default()).is_empty());
    }
    #[test]
    fn percentage_and_every_native_complex_style_roundtrip() {
        for complex_format in [
            ComplexFormat::I,
            ComplexFormat::J,
            ComplexFormat::PolarRadians,
            ComplexFormat::PolarPiRadians,
            ComplexFormat::PolarDegrees,
        ] {
            let style = MathFormat {
                explicit: true,
                result_format: ResultFormat::Percent,
                complex_format,
                precision: 5,
                show_trailing_zeros: true,
                ..Default::default()
            };
            let xml = format!("<math xmlns=\"{WS}\">{}</math>", local_xml(&style));
            let doc = roxmltree::Document::parse(&xml).unwrap();
            assert_eq!(read_math_format(doc.root_element()), style);
            assert!(xml.contains("<percent "));
            assert!(!xml.contains("zero-threshold"));
        }
    }
    #[test]
    fn worksheet_defaults_and_partial_region_overrides_inherit_per_attribute() {
        let xml = format!(
            "<presentation xmlns=\"{PRESENTATION}\" xmlns:ws=\"{WS}\"><mathRendering><formatting FontSize=\"17\"/><ws:results><ws:general precision=\"8\" show-trailing-zeros=\"true\" imaginary-value=\"j\" exponential-threshold=\"5\" zero-threshold=\"12\" complex-threshold=\"7\" use-engineering-format=\"true\"/></ws:results></mathRendering></presentation>"
        );
        let doc = roxmltree::Document::parse(&xml).unwrap();
        let defaults = read_presentation_math_format(doc.root_element());
        assert!(!defaults.explicit);
        assert_eq!(defaults.font_points, 17.);
        assert_eq!(defaults.precision, 8);
        let local = format!(
            "<math xmlns=\"{WS}\"><resultFormat><percent precision=\"4\" imaginary-value=\"polarDegrees\"/></resultFormat></math>"
        );
        let local = roxmltree::Document::parse(&local).unwrap();
        let got = read_math_format_with_defaults(local.root_element(), defaults);
        assert_eq!(
            got,
            MathFormat {
                explicit: true,
                result_format: ResultFormat::Percent,
                precision: 4,
                complex_format: ComplexFormat::PolarDegrees,
                ..defaults
            }
        );
        let absent = format!("<math xmlns=\"{WS}\"/>");
        let absent = roxmltree::Document::parse(&absent).unwrap();
        assert_eq!(
            read_math_format_with_defaults(absent.root_element(), defaults),
            defaults
        );
    }
}
