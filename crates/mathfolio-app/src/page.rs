//! Body-coordinate mapping recovered from Prime's page model and BAML grid.
use eframe::egui::{Pos2, Vec2, pos2, vec2};
use mathfolio_document::canvas::CanvasDocument;

pub(crate) const METRIC_MARGIN: f32 = (2.5 * 96. / 2.54) as f32;

#[derive(Clone, Copy, Debug)]
pub(crate) struct PageGeometry {
    pub width: f32,
    pub height: f32,
    pub grid: f32,
    /// left, top, right, bottom; body coordinates exclude these margins.
    pub margins: [f32; 4],
    body_height_dips: f64,
}
impl PageGeometry {
    pub fn new(doc: &CanvasDocument, margins: [f32; 4]) -> Self {
        Self {
            width: doc.width as f32,
            height: doc.height as f32,
            grid: doc.grid as f32,
            margins,
            body_height_dips: (((doc.height - f64::from(margins[1]) - f64::from(margins[3]))
                / doc.grid)
                .round_ties_even()
                * doc.grid)
                .max(doc.grid),
        }
    }
    pub fn body_width(self) -> f32 {
        (self.width - self.margins[0] - self.margins[2]).max(self.grid)
    }
    pub(crate) fn body_height_exact(self) -> f64 {
        self.body_height_dips
    }
    pub fn body_height(self) -> f32 {
        self.body_height_dips as f32
    }
    pub fn stride(self) -> f32 {
        self.height + self.grid
    }
    pub fn body_origin(self, page: usize) -> Pos2 {
        pos2(
            self.margins[0],
            page as f32 * self.stride() + self.margins[1],
        )
    }
    pub fn to_paper(self, x: f64, y: f64) -> Pos2 {
        let page = ((y.max(0.) + 1e-8) / self.body_height_dips).floor() as usize;
        self.body_origin(page)
            + vec2(
                x as f32,
                (y - page as f64 * self.body_height_dips).max(0.) as f32,
            )
    }
    pub fn to_body(self, point: Pos2) -> Option<(f64, f64)> {
        let page = (point.y / self.stride()).floor().max(0.) as usize;
        let local = point - self.body_origin(page);
        if local.x < 0.
            || local.x > self.body_width()
            || local.y < 0.
            || local.y >= self.body_height()
        {
            return None;
        }
        Some((
            f64::from(local.x),
            f64::from(local.y) + page as f64 * self.body_height_dips,
        ))
    }
    pub fn to_body_clamped(self, point: Pos2) -> (f64, f64) {
        let page = (point.y / self.stride()).floor().max(0.) as usize;
        let local = point - self.body_origin(page);
        (
            f64::from(local.x.max(0.)),
            f64::from(local.y.clamp(0., self.body_height())) + page as f64 * self.body_height_dips,
        )
    }
    pub fn major_spacing(self) -> f32 {
        if ((self.width - 816.).abs() < 0.1 && (self.height - 1056.).abs() < 0.1)
            || ((self.width - 1056.).abs() < 0.1 && (self.height - 816.).abs() < 0.1)
        {
            96.
        } else {
            2. * 96. / 2.54
        }
    }
    pub fn size(self, bottom: f64) -> Vec2 {
        let pages = (bottom / self.body_height_dips).ceil().max(1.) as f32;
        vec2(self.width, pages * self.stride() - self.grid)
    }
}

pub(crate) fn standard_margins(doc: &CanvasDocument) -> [f32; 4] {
    if let Some(setup) = &doc.page_setup {
        return setup.margins.map(|v| v as f32);
    }
    let us = (doc.width - 816.).abs() < 0.1 && (doc.height - 1056.).abs() < 0.1
        || (doc.width - 1056.).abs() < 0.1 && (doc.height - 816.).abs() < 0.1;
    [if us { 96. } else { METRIC_MARGIN }; 4]
}

/// Observed A4/Letter settings; unsupported/custom paper sizes retain document dimensions.
pub(crate) fn source_settings(source: &str) -> Option<(Vec2, [f32; 4])> {
    let xml = roxmltree::Document::parse(source.trim_start_matches('\u{feff}')).ok()?;
    let model = xml
        .descendants()
        .find(|n| n.tag_name().name() == "pageModel")?;
    let paper = mathfolio_document::canvas::PaperSize::from_code(model.attribute("paper-code")?)?;
    let [width, height] = paper.size(model.attribute("orientation") == Some("Landscape"));
    let margin = if matches!(
        paper,
        mathfolio_document::canvas::PaperSize::Letter
            | mathfolio_document::canvas::PaperSize::Legal
            | mathfolio_document::canvas::PaperSize::Ledger
    ) {
        96.
    } else {
        METRIC_MARGIN
    };
    let factor = match model.attribute("margin-type") {
        Some("Narrow") => 0.5,
        Some("Wide") => 2.,
        _ => 1.,
    };
    let margins = match model.attribute("margin-type") {
        Some("Custom") => parse_margins(model.attribute("page-margin")?)?,
        Some("Legacy") => [115.2; 4],
        _ => [margin * factor; 4],
    };
    if margins.iter().any(|v| !v.is_finite() || *v < 0.)
        || f64::from(margins[0] + margins[2]) >= width
        || f64::from(margins[1] + margins[3]) >= height
    {
        return None;
    }
    Some((vec2(width as f32, height as f32), margins))
}

fn parse_margins(text: &str) -> Option<[f32; 4]> {
    let values = text
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|v| !v.is_empty())
        .map(str::parse::<f32>)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    match values.as_slice() {
        [all] => Some([*all; 4]),
        [horizontal, vertical] => Some([*horizontal, *vertical, *horizontal, *vertical]),
        [left, top, right, bottom] => Some([*left, *top, *right, *bottom]),
        _ => None,
    }
}

pub(crate) fn view_settings(source: &str) -> (bool, bool) {
    let parsed = roxmltree::Document::parse(source.trim_start_matches('\u{feff}'));
    let model = parsed.as_ref().ok().and_then(|xml| {
        xml.descendants()
            .find(|n| n.tag_name().name() == "pageModel")
    });
    let grid = !model.is_some_and(|n| matches!(n.attribute("show-grid"), Some("false" | "0")));
    let major =
        model.is_some_and(|n| matches!(n.attribute("useMajorGridLines"), Some("true" | "1")));
    (grid, major)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn body_coordinates_cross_pages_without_becoming_paper_coordinates() {
        let doc = CanvasDocument::default();
        let page = PageGeometry::new(&doc, standard_margins(&doc));
        for (x, y) in [
            (0., 0.),
            (37., 250.),
            (30., f64::from(page.body_height()) + 42.),
        ] {
            let point = page.to_paper(x, y);
            let (xx, yy) = page.to_body(point).unwrap();
            assert!((xx - x).abs() < 0.001 && (yy - y).abs() < 0.001);
        }
        let boundary =
            ((doc.height - 2. * 2.5 * 96. / 2.54) / doc.grid).round_ties_even() * doc.grid;
        assert!((page.to_paper(0., boundary).y - page.body_origin(1).y).abs() < 0.001);
        assert!(page.to_body(pos2(1., 100.)).is_none());
        assert!(page.to_body(pos2(100., page.height + 1.)).is_none());
        assert!(
            (page.body_height() / page.grid - (page.body_height() / page.grid).round()).abs()
                < 0.001
        );
    }
    #[test]
    fn custom_paper_margins_match_native_invariant_thickness_and_legacy_rule() {
        for (value, expected) in [
            ("12", [12.; 4]),
            ("12,24", [12., 24., 12., 24.]),
            ("1,2,3,4", [1., 2., 3., 4.]),
        ] {
            let source = format!(
                "<presentation><pageModel paper-code=\"Legal\" orientation=\"Landscape\" margin-type=\"Custom\" page-margin=\"{value}\"/></presentation>"
            );
            let (size, margins) = source_settings(&source).unwrap();
            assert_eq!(size, vec2(1344., 816.));
            assert_eq!(margins, expected);
        }
        for value in ["-1", "NaN", "1,2,3", "10000"] {
            assert!(source_settings(&format!("<p><pageModel paper-code=\"A4\" margin-type=\"Custom\" page-margin=\"{value}\"/></p>")).is_none());
        }
        assert_eq!(
            source_settings("<p><pageModel paper-code=\"A4\" margin-type=\"Legacy\"/></p>")
                .unwrap()
                .1,
            [115.2; 4]
        );
    }

    #[test]
    fn saved_page_settings_keep_orientation_margins_and_screen_grid_state() {
        let source = r#"<presentation><pageModel paper-code="Letter" orientation="Landscape" margin-type="Narrow" show-grid="false" useMajorGridLines="true"/></presentation>"#;
        let (size, margins) = source_settings(source).unwrap();
        assert_eq!(size, vec2(1056., 816.));
        assert_eq!(margins, [48.; 4]);
        assert_eq!(view_settings(source), (false, true));
        assert_eq!(
            view_settings("<presentation><pageModel/></presentation>"),
            (true, false)
        );
        let doc = CanvasDocument::default();
        let page = PageGeometry::new(&doc, standard_margins(&doc));
        let start = page.to_paper(30., f64::from(page.body_height()) - 10.);
        let destination = page.to_paper(30., f64::from(page.body_height()) + 20.);
        let (_, y) = page.to_body_clamped(start + (destination - start));
        assert!((y - f64::from(page.body_height()) - 20.).abs() < 0.001);
    }
}
