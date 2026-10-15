//! Prime's recovered type styles with independently licensed cross-platform faces.
//! Font shapes use STIX Two, not the proprietary Mathcad font. See assets/fonts/stix.
use eframe::egui::{self, Color32, FontFamily, FontId};

pub(crate) const MATH_SIZE: f32 = 11. * 96. / 72.;
pub(crate) const INK: Color32 = Color32::BLACK;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MathStyle {
    Operator,
    Variable,
    Unit,
    Constant,
    Function,
    Keyword,
}
impl MathStyle {
    pub(crate) fn from_label(label: &str) -> Self {
        match label.split_whitespace().next().unwrap_or("") {
            "UNIT" => Self::Unit,
            "CONSTANT" => Self::Constant,
            "FUNCTION" => Self::Function,
            "SYSTEM" | "KEYWORD" => Self::Keyword,
            _ => Self::Variable,
        }
    }
    pub(crate) fn color(self) -> Color32 {
        match self {
            Self::Unit => Color32::from_rgb(0, 0, 139),
            Self::Constant => Color32::from_rgb(85, 107, 47),
            _ => INK,
        }
    }
    pub(crate) fn font(self, size: f32) -> FontId {
        let family = match self {
            Self::Variable => "math-italic",
            Self::Unit | Self::Constant => "math-bold-italic",
            Self::Keyword => "math-bold",
            Self::Operator | Self::Function => "math-regular",
        };
        FontId::new(size, FontFamily::Name(family.into()))
    }
}

/// Prime rounds scripts in points to two decimals, with a four-point minimum.
pub(crate) fn script_size(size: f32, ratio: f32) -> f32 {
    ((size * 72. / 96. * ratio * 100.).round() / 100.).max(4.) * 96. / 72.
}

pub(crate) fn body_font() -> FontId {
    FontId::new(MATH_SIZE, FontFamily::Name("worksheet-text".into()))
}

pub(crate) fn install(fonts: &mut egui::FontDefinitions) {
    let mut body_faces = fonts.families[&FontFamily::Proportional].clone();
    for path in [
        "C:/Windows/Fonts/tahoma.ttf",
        "/usr/share/fonts/truetype/msttcorefonts/Tahoma.ttf",
        "/usr/share/fonts/truetype/liberation2/LiberationSans-Regular.ttf",
    ] {
        if let Ok(bytes) = std::fs::read(path) {
            fonts.font_data.insert(
                "worksheet-text".into(),
                egui::FontData::from_owned(bytes).into(),
            );
            body_faces.insert(0, "worksheet-text".into());
            break;
        }
    }
    fonts
        .families
        .insert(FontFamily::Name("worksheet-text".into()), body_faces);
    for (name, bytes) in [
        (
            "math-regular",
            include_bytes!("../../../assets/fonts/stix/STIXTwoMath-Regular.ttf").as_slice(),
        ),
        (
            "math-italic",
            include_bytes!("../../../assets/fonts/stix/STIXTwoText-Italic.ttf").as_slice(),
        ),
        (
            "math-bold-italic",
            include_bytes!("../../../assets/fonts/stix/STIXTwoText-BoldItalic.ttf").as_slice(),
        ),
        (
            "math-bold",
            include_bytes!("../../../assets/fonts/stix/STIXTwoText-Bold.ttf").as_slice(),
        ),
    ] {
        fonts
            .font_data
            .insert(name.into(), egui::FontData::from_static(bytes).into());
        let mut faces = vec![name.to_owned()];
        if name != "math-regular" {
            faces.push("math-regular".into());
        }
        faces.extend(fonts.families[&FontFamily::Proportional].iter().cloned());
        fonts.families.insert(FontFamily::Name(name.into()), faces);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recovered_script_sizes_keep_point_rounding_and_minimum() {
        assert!((script_size(MATH_SIZE, 7. / 11.) - 28. / 3.).abs() < 0.0001);
        assert!((script_size(MATH_SIZE, 0.7) - 10.266667).abs() < 0.0001);
        assert!((script_size(5., 7. / 11.) - 16. / 3.).abs() < 0.0001);
    }
}
