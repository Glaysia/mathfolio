//! Retain shared glyph meshes when only the painter opacity changes.
use eframe::egui::{self, Color32, Galley, Painter, Pos2};
use std::sync::Arc;
pub(crate) fn paint_galley(
    painter: &Painter,
    origin: Pos2,
    galley: Arc<Galley>,
    fallback: Color32,
) {
    let opacity = painter.opacity();
    if opacity > 0. && opacity < 1. && painter.is_visible() && !galley.is_empty() {
        // Keep inherited disabled tint and clip on the clone. Only defer the
        // opacity factor; nontext shapes still use their ordinary painter.
        let mut text = painter.clone();
        text.set_opacity(1.);
        text.add(
            egui::epaint::TextShape::new(origin, galley, fallback).with_opacity_factor(opacity),
        );
    } else {
        painter.galley(origin, galley, fallback);
    }
}

/// Operator selection uses the native caret color on glyph geometry, retaining
/// the shared galley and the painter's disabled tint/opacity and clip.
pub(crate) fn paint_galley_override(
    painter: &Painter,
    origin: Pos2,
    galley: Arc<Galley>,
    color: Color32,
) {
    if !painter.is_visible() || galley.is_empty() || painter.opacity() <= 0. {
        return;
    }
    let mut text = painter.clone();
    text.set_opacity(1.);
    let mut shape =
        egui::epaint::TextShape::new(origin, galley, color).with_opacity_factor(painter.opacity());
    shape.override_text_color = Some(color);
    text.add(shape);
}
#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{Rect, vec2};
    fn meshes(
        ctx: &egui::Context,
        optimized: bool,
        opacity: f32,
        disabled: bool,
        clip: Rect,
    ) -> Vec<egui::ClippedPrimitive> {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(600., 300.))),
                ..Default::default()
            },
            |ui| {
                if disabled {
                    ui.disable();
                }
                let mut painter = ui.painter().clone();
                painter.set_clip_rect(clip);
                painter.set_opacity(opacity);
                // Explicit colors, placeholder fallback, underlines/background and
                // non-Latin glyphs exercise the same tessellation color pipeline.
                let mut job = egui::text::LayoutJob::default();
                for (text, color) in [
                    ("𝑥 µ 한글", Color32::BLACK),
                    (" m²", Color32::from_rgb(0, 0, 139)),
                    (" π", Color32::from_rgb(85, 107, 47)),
                    (" fallback", Color32::PLACEHOLDER),
                ] {
                    job.append(
                        text,
                        0.,
                        egui::TextFormat {
                            font_id: crate::typography::MathStyle::Variable.font(18.),
                            color,
                            underline: egui::Stroke::new(0.8, color),
                            background: Color32::from_rgba_unmultiplied(40, 60, 80, 70),
                            ..Default::default()
                        },
                    );
                }
                let galley = crate::typography::layout_job(&painter, job);
                let p = Pos2::new(10.25, 20.125);
                if optimized {
                    paint_galley(&painter, p, galley, Color32::from_rgb(80, 30, 10));
                } else {
                    painter.galley(p, galley, Color32::from_rgb(80, 30, 10));
                }
                painter.line_segment(
                    [p, p + vec2(75., 40.)],
                    egui::Stroke::new(1.5, Color32::BLUE),
                );
                painter.rect_stroke(
                    Rect::from_min_size(p, vec2(30., 25.)),
                    0.,
                    egui::Stroke::new(0.8, Color32::BLACK),
                    egui::StrokeKind::Inside,
                );
            },
        );
        let shapes = std::mem::take(&mut output.shapes);
        let result = ctx.tessellate(shapes, output.pixels_per_point);
        output.drop_without_applying_deltas();
        result
    }
    #[test]
    fn opacity_factor_preserves_exact_mesh_colors_geometry_clip_and_disabled_tint() {
        for dpi in [1., 2.] {
            for opacity in [0., 0.45, 0.25 * 0.8, 1.] {
                for disabled in [false, true] {
                    let ctx = egui::Context::default();
                    crate::configure(&ctx);
                    ctx.set_pixels_per_point(dpi);
                    for clip in [
                        Rect::from_min_size(Pos2::ZERO, vec2(500., 300.)),
                        Rect::from_min_size(Pos2::new(13., 25.), vec2(25., 14.)),
                    ] {
                        let before = meshes(&ctx, false, opacity, disabled, clip);
                        let after = meshes(&ctx, true, opacity, disabled, clip);
                        assert_eq!(before.len(), after.len());
                        for (a, b) in before.iter().zip(after.iter()) {
                            assert_eq!(a.clip_rect, b.clip_rect);
                            match (&a.primitive, &b.primitive) {
                                (
                                    egui::epaint::Primitive::Mesh(a),
                                    egui::epaint::Primitive::Mesh(b),
                                ) => {
                                    assert_eq!(a.vertices, b.vertices);
                                    assert_eq!(a.indices, b.indices);
                                    assert_eq!(a.texture_id, b.texture_id);
                                }
                                _ => panic!("unexpected callback"),
                            }
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn pending_text_keeps_original_shared_galley_and_vertex_colors() {
        let ctx = egui::Context::default();
        crate::configure(&ctx);
        let mut source = None;
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let mut painter = ui.painter().clone();
            painter.set_opacity(0.45);
            let galley = painter.layout_no_wrap(
                "shared mesh".into(),
                crate::typography::body_font(),
                Color32::BLACK,
            );
            source = Some(galley.clone());
            paint_galley(&painter, Pos2::ZERO, galley, Color32::BLACK);
        });
        let source = source.unwrap();
        let painted = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) => Some(text),
                _ => None,
            })
            .unwrap();
        assert!(Arc::ptr_eq(&source, &painted.galley));
        assert_eq!(painted.opacity_factor, 0.45);
        output.drop_without_applying_deltas();
    }

    #[test]
    fn selected_operator_override_matches_native_painter_mesh_for_opacity_tint_and_clip() {
        let ctx = egui::Context::default();
        crate::configure(&ctx);
        for opacity in [0., 0.25, 1.] {
            for disabled in [false, true] {
                let mut meshes = vec![];
                for optimized in [false, true] {
                    let mut output = ctx.run_ui(Default::default(), |ui| {
                        if disabled {
                            ui.disable();
                        }
                        let mut painter = ui.painter().clone();
                        painter.set_opacity(opacity);
                        painter
                            .set_clip_rect(Rect::from_min_size(Pos2::new(9., 19.), vec2(17., 20.)));
                        let galley = painter.layout_no_wrap(
                            "+≤".into(),
                            crate::typography::body_font(),
                            Color32::BLACK,
                        );
                        let origin = Pos2::new(10.25, 20.125);
                        let color = Color32::from_rgb(0, 128, 255);
                        if optimized {
                            paint_galley_override(&painter, origin, galley, color);
                        } else {
                            painter.galley_with_override_text_color(origin, galley, color);
                        }
                    });
                    meshes.push(
                        ctx.tessellate(std::mem::take(&mut output.shapes), output.pixels_per_point),
                    );
                    output.drop_without_applying_deltas();
                }
                assert_eq!(meshes[0].len(), meshes[1].len());
                for (a, b) in meshes[0].iter().zip(&meshes[1]) {
                    assert_eq!(a.clip_rect, b.clip_rect);
                    match (&a.primitive, &b.primitive) {
                        (egui::epaint::Primitive::Mesh(a), egui::epaint::Primitive::Mesh(b)) => {
                            assert_eq!(a.vertices, b.vertices);
                            assert_eq!(a.indices, b.indices);
                            assert_eq!(a.texture_id, b.texture_id);
                        }
                        _ => panic!("unexpected callback"),
                    }
                }
            }
        }
    }
}
