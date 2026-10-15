//! Bounded worksheet-local math presentation, separate from semantic MathML.
use crate::canvas::{MathFormat, ResultFormat};
use roxmltree::Node;
const WS: &str = "http://schemas.mathsoft.com/worksheet50";
pub fn read_math_format(math: Node<'_, '_>) -> MathFormat {
    let mut style = MathFormat::default();
    if let Some(format) = math
        .children()
        .find(|n| n.has_tag_name((WS, "formattingOverride")))
        && let Some(points) = format
            .attribute("FontSize")
            .and_then(|n| n.parse::<f32>().ok())
        && points.is_finite()
        && (1. ..=200.).contains(&points)
    {
        style.font_points = points;
        style.explicit = true;
    }
    if let Some(format) = math
        .children()
        .find(|n| n.has_tag_name((WS, "resultFormat")))
        && let Some(number) = format.children().find(|n| {
            n.tag_name().namespace() == Some(WS)
                && matches!(
                    n.tag_name().name(),
                    "general" | "decimal" | "scientific" | "engineering"
                )
        })
    {
        style.result_format = match number.tag_name().name() {
            "decimal" => ResultFormat::Decimal,
            "scientific" => ResultFormat::Scientific,
            "engineering" => ResultFormat::Engineering,
            _ => ResultFormat::General,
        };
        if let Some(precision) = number
            .attribute("precision")
            .and_then(|n| n.parse::<u8>().ok())
            .filter(|n| *n <= 15)
        {
            style.precision = precision;
        }
        style.explicit = true;
    }
    style
}
pub(crate) fn style_name(style: ResultFormat) -> &'static str {
    match style {
        ResultFormat::General => "general",
        ResultFormat::Decimal => "decimal",
        ResultFormat::Scientific => "scientific",
        ResultFormat::Engineering => "engineering",
    }
}
pub(crate) fn number_xml(style: &MathFormat) -> String {
    let name = style_name(style.result_format);
    let notation = if matches!(
        style.result_format,
        ResultFormat::Scientific | ResultFormat::Engineering
    ) {
        " use-e-notation=\"false\""
    } else {
        ""
    };
    let exp = if style.result_format == ResultFormat::General {
        " exponential-threshold=\"3\""
    } else {
        ""
    };
    format!(
        "<{name} precision=\"{}\" show-trailing-zeros=\"false\" radix=\"dec\" zero-threshold=\"15\" imaginary-value=\"i\" complex-threshold=\"10\"{notation}{exp}/>",
        style.precision
    )
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
            explicit: true,
        };
        let xml = format!("<math xmlns=\"{WS}\">{}</math>", local_xml(&style));
        let doc = roxmltree::Document::parse(&xml).unwrap();
        assert_eq!(read_math_format(doc.root_element()), style);
        assert!(local_xml(&MathFormat::default()).is_empty());
    }
}
