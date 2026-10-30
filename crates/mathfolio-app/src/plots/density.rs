//! Own worksheet text only. egui_plot-owned ticks are not modified.
use eframe::egui::{self, Color32, FontId, Response, TextBuffer, text::LayoutJob};
use std::sync::Arc;

pub(super) fn axis_label(ui: &mut egui::Ui, text: &str) -> Response {
    let mut job = LayoutJob::default();
    egui::RichText::new(text).italics().append_to(
        &mut job,
        ui.style(),
        egui::FontSelection::Default,
        ui.text_valign(),
    );
    let galley = crate::typography::layout_job(ui.painter(), job);
    ui.add(egui::Label::new(galley))
}

/// Same default single-line TextEdit job settings as locked egui0.36.2:
/// default font, inactive/override text color, trailing whitespace, row height.
/// No events, IDs, cursor, focus, text storage or undo state are touched.
pub(super) fn singleline(ui: &egui::Ui, text: &dyn TextBuffer, _width: f32) -> Arc<egui::Galley> {
    let font = egui::FontSelection::default().resolve(ui.style());
    let color = ui
        .visuals()
        .override_text_color
        .unwrap_or_else(|| ui.visuals().widgets.inactive.text_color());
    let line_height =
        ui.fonts_mut(|fonts| fonts.row_height(&font)) + ui.spacing().extra_text_line_spacing;
    let mut job = LayoutJob::simple_singleline(text.as_str().to_owned(), font, color);
    job.keep_trailing_whitespace = true;
    for section in &mut job.sections {
        section.format.line_height = Some(line_height);
    }
    crate::typography::layout_job(ui.painter(), job)
}

pub(super) fn painter_text(
    painter: &egui::Painter,
    pos: egui::Pos2,
    text: String,
    font: FontId,
    color: Color32,
) {
    let galley = crate::typography::layout_no_wrap(painter, text, font, color);
    painter.galley(pos, galley, color);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn axis_labels_retain_stock_font_color_valign_and_single_glyph_geometry() {
        for override_color in [None, Some(Color32::from_rgb(29, 71, 113))] {
            for valign in [egui::Align::TOP, egui::Align::Center, egui::Align::BOTTOM] {
                for width in [4., 24., 100.] {
                    let ctx = egui::Context::default();
                    crate::configure(&ctx);
                    let mut sizes = None;
                    let output = ctx.run_ui(egui::RawInput::default(), |ui| {
                        ui.visuals_mut().override_text_color = override_color;
                        let mut stock = None;
                        let mut dense = None;
                        for (is_stock, result) in [(true, &mut stock), (false, &mut dense)] {
                            ui.allocate_ui_with_layout(
                                egui::vec2(width, 40.),
                                egui::Layout::left_to_right(valign),
                                |ui| {
                                    *result = Some(if is_stock {
                                        ui.label(egui::RichText::new("x").italics())
                                    } else {
                                        axis_label(ui, "x")
                                    });
                                },
                            );
                        }
                        sizes = Some((stock.unwrap().rect.size(), dense.unwrap().rect.size()));
                    });
                    // Extract actual painted stock/own jobs before disposal;
                    // assertions always follow disposal to avoid a second panic.
                    let jobs = output
                        .shapes
                        .iter()
                        .filter_map(|s| {
                            if let egui::Shape::Text(text) = &s.shape {
                                (text.galley.text() == "x").then(|| text.galley.clone())
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>();
                    output.drop_without_applying_deltas();
                    let (stock_size, dense_size) = sizes.unwrap();
                    assert_eq!(stock_size, dense_size);
                    assert_eq!(jobs.len(), 2);
                    assert_eq!(jobs[0].job.sections, jobs[1].job.sections);
                    assert_eq!(jobs[0].size(), jobs[1].size());
                    assert_eq!(jobs[0].rows.len(), jobs[1].rows.len());
                }
            }
        }
    }
}

/// Match AxisWidget's WidgetText::Text.into_galley(Extend, infinity, Body)
/// policy exactly, changing only raster density through the own helper.
/// A prebuilt Galley is accepted by x_axis_label/y_axis_label. AxisWidget still
/// owns title placement, thickness/gap calculation and Y rotation.
pub(super) fn axis_title(ui: &egui::Ui, text: &str) -> Arc<egui::Galley> {
    let font = egui::FontSelection::default()
        .resolve_with_fallback(ui.style(), egui::TextStyle::Body.into());
    let line_height =
        ui.fonts_mut(|fonts| fonts.row_height(&font)) + ui.spacing().extra_text_line_spacing;
    let color = ui
        .visuals()
        .override_text_color
        .unwrap_or(Color32::PLACEHOLDER);
    let job = LayoutJob::simple_format(
        text.to_owned(),
        egui::TextFormat {
            font_id: font,
            color,
            valign: ui.text_valign(),
            line_height: Some(line_height),
            ..Default::default()
        },
    );
    crate::typography::layout_job(ui.painter(), job)
}

#[cfg(test)]
mod title_tests {
    use super::*;
    #[test]
    fn axis_title_matches_stock_string_policy_with_override_and_valign() {
        let ctx = egui::Context::default();
        crate::configure(&ctx);
        let mut jobs = Vec::new();
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            ui.style_mut().override_font_id = Some(FontId::proportional(17.));
            ui.visuals_mut().override_text_color =
                Some(Color32::from_rgba_unmultiplied(40, 80, 120, 128));
            ui.spacing_mut().extra_text_line_spacing = 2.;
            for valign in [egui::Align::Min, egui::Align::Center, egui::Align::Max] {
                ui.with_layout(egui::Layout::left_to_right(valign), |ui| {
                    crate::typography::with_worksheet_zoom(ui.ctx(), 2., || {
                        let actual = axis_title(ui, "Axis 한😀\nsecond");
                        let stock = egui::WidgetText::from("Axis 한😀\nsecond").into_galley(
                            ui,
                            Some(egui::TextWrapMode::Extend),
                            f32::INFINITY,
                            egui::TextStyle::Body,
                        );
                        jobs.push((actual.job.clone(), stock.job.clone()));
                    });
                });
            }
        });
        output.drop_without_applying_deltas();
        assert_eq!(jobs.len(), 3);
        for (actual, stock) in jobs {
            assert_eq!(actual, stock);
        }
    }
}
