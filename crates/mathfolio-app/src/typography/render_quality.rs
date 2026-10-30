//! Lay out worksheet glyphs at their final on-screen density, while retaining
//! worksheet DIP geometry. A layer transform alone only stretches atlas texels.
use eframe::egui::{self, Color32, Context, FontId, Galley, Painter, text::LayoutJob};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FontCacheKey {
    pub pixels_per_point: f32,
    pub worksheet_zoom: f32,
    epoch: u64,
}
struct CachedGalley {
    galley: Arc<Galley>,
    last_used: u64,
}
struct State {
    pass: u64,
    zoom: f32,
    epoch: u64,
    sentinel: Option<Arc<Galley>>,
    layouts: HashMap<u64, CachedGalley>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            pass: u64::MAX,
            zoom: 1.,
            epoch: 0,
            sentinel: None,
            layouts: HashMap::new(),
        }
    }
}
fn state(ctx: &Context) -> Arc<Mutex<State>> {
    ctx.data_mut(|data| {
        data.get_temp_mut_or_default::<Arc<Mutex<State>>>(egui::Id::new("worksheet-font-density"))
            .clone()
    })
}
pub(crate) fn with_worksheet_zoom<R>(ctx: &Context, zoom: f32, paint: impl FnOnce() -> R) -> R {
    struct Restore {
        state: Arc<Mutex<State>>,
        previous: f32,
    }
    impl Drop for Restore {
        fn drop(&mut self) {
            self.state.lock().unwrap().zoom = self.previous;
        }
    }
    let state = state(ctx);
    let previous = {
        let mut s = state.lock().unwrap();
        let previous = s.zoom;
        s.zoom = if zoom.is_finite() {
            zoom.clamp(0.25, 4.)
        } else {
            1.
        };
        previous
    };
    let _restore = Restore { state, previous };
    paint()
}

pub(crate) fn font_cache_key(ctx: &Context) -> FontCacheKey {
    #[cfg(test)]
    let _timing = crate::frame_profile::scope(crate::frame_profile::Phase::FontKey);
    let cache = state(ctx);
    let mut cache = cache.lock().unwrap();
    let pass = ctx.cumulative_pass_nr();
    if cache.pass != pass {
        // The stock galley cache is replaced when Fonts recreates its atlas.
        // Touching this zero-length job each pass keeps its Arc stable during
        // ordinary atlas growth and changes it after recreation/font replacement.
        let sentinel = ctx.fonts_mut(|f| {
            f.layout_no_wrap(
                String::new(),
                FontId::proportional(1.),
                Color32::TRANSPARENT,
            )
        });
        if cache
            .sentinel
            .as_ref()
            .is_none_or(|old| !Arc::ptr_eq(old, &sentinel))
        {
            cache.epoch = cache.epoch.wrapping_add(1);
            cache.layouts.clear();
        }
        cache.sentinel = Some(sentinel);
        cache.pass = pass;
        cache
            .layouts
            .retain(|_, entry| pass.saturating_sub(entry.last_used) <= 2);
    }
    FontCacheKey {
        pixels_per_point: ctx.pixels_per_point(),
        worksheet_zoom: cache.zoom,
        epoch: cache.epoch,
    }
}

pub(crate) fn layout_job(painter: &Painter, job: LayoutJob) -> Arc<Galley> {
    #[cfg(test)]
    let _timing = crate::frame_profile::scope(crate::frame_profile::Phase::GlyphLayout);
    let ctx = painter.ctx();
    let key = font_cache_key(ctx);
    // Zooming out already downsamples sufficiently dense glyphs. Keep those
    // existing atlas entries instead of allocating lower-resolution duplicates.
    if key.worksheet_zoom <= 1. {
        return painter.layout_job(job);
    }
    let hash = egui::util::hash((
        &job,
        key.pixels_per_point.to_bits(),
        key.worksheet_zoom.to_bits(),
        key.epoch,
    ));
    let state = state(ctx);
    let mut state = state.lock().unwrap();
    let pass = ctx.cumulative_pass_nr();
    if let Some(cached) = state.layouts.get_mut(&hash)
        && *cached.galley.job == job
    {
        cached.last_used = pass;
        return cached.galley.clone();
    }
    let mut galley = ctx.fonts_mut(|fonts| {
        egui::epaint::text::layout(
            fonts.fonts,
            key.pixels_per_point * key.worksheet_zoom,
            Arc::new(job),
        )
    });
    // The mesh and positions remain in worksheet DIP, and the atlas contains
    // final-density texels. Context tessellation still uses the window's PPP;
    // this field is its consistency tag, not a request to rescale the mesh.
    galley.pixels_per_point = key.pixels_per_point;
    let galley = Arc::new(galley);
    if state.layouts.len() >= 8192 {
        state.layouts.clear();
    }
    state.layouts.insert(
        hash,
        CachedGalley {
            galley: galley.clone(),
            last_used: pass,
        },
    );
    galley
}

pub(crate) fn layout_no_wrap(
    painter: &Painter,
    text: String,
    font: FontId,
    color: Color32,
) -> Arc<Galley> {
    layout_job(painter, LayoutJob::simple(text, font, color, f32::INFINITY))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn zoom_rasterizes_at_final_density_without_scaling_document_or_chrome() {
        for os in [1., 1.5, 2.] {
            let ctx = Context::default();
            crate::configure(&ctx);
            let mut input = egui::RawInput::default();
            input
                .viewports
                .get_mut(&egui::ViewportId::ROOT)
                .unwrap()
                .native_pixels_per_point = Some(os);
            let output = ctx.run_ui(input, |ui| {
                let plain = layout_no_wrap(
                    ui.painter(),
                    "αxΩ".into(),
                    FontId::proportional(20.),
                    Color32::BLACK,
                );
                for zoom in [0.5, 1., 1.25, 2.] {
                    with_worksheet_zoom(&ctx, zoom, || {
                        let g = layout_no_wrap(
                            ui.painter(),
                            "αxΩ".into(),
                            FontId::proportional(20.),
                            Color32::BLACK,
                        );
                        let same = layout_no_wrap(
                            ui.painter(),
                            "αxΩ".into(),
                            FontId::proportional(20.),
                            Color32::BLACK,
                        );
                        assert!(Arc::ptr_eq(&g, &same));
                        assert!((g.size().x - plain.size().x).abs() < 2.);
                        assert!(
                            (g.rows[0].glyphs[0].pos.y - plain.rows[0].glyphs[0].pos.y).abs() <= 2./os,
                            "os={os} zoom={zoom} glyph_y={} plain_y={}",g.rows[0].glyphs[0].pos.y,plain.rows[0].glyphs[0].pos.y
                        );
                        let expected = ctx.fonts_mut(|f| {
                            egui::epaint::text::layout(f.fonts, os * zoom.max(1.), g.job.clone())
                        });
                        assert_eq!(
                            g.rows[0].glyphs[0].uv_rect,
                            expected.rows[0].glyphs[0].uv_rect
                        );
                        assert_eq!(ctx.pixels_per_point(), os);
                        assert_eq!(ctx.zoom_factor(), 1.);
                        let mut shape=egui::Shape::galley(egui::Pos2::ZERO,g.clone(),Color32::BLACK);
                        shape.transform(egui::emath::TSTransform::from_scaling(zoom));
                        let meshes=ctx.tessellate(vec![egui::epaint::ClippedShape {clip_rect:egui::Rect::EVERYTHING,shape}],os);
                        assert!(!meshes.is_empty());
                        assert!(meshes.iter().all(|p|matches!(&p.primitive,egui::epaint::Primitive::Mesh(mesh) if mesh.vertices.iter().all(|v|v.pos.x.is_finite()&&v.pos.y.is_finite()&&v.uv.x.is_finite()&&v.uv.y.is_finite()))));
                        if zoom == 2. {
                            let a = g.rows[0].glyphs[0].uv_rect;
                            let b = plain.rows[0].glyphs[0].uv_rect;
                            assert!((a.max[1] - a.min[1]) > (b.max[1] - b.min[1]));
                        }
                    });
                }
                assert_eq!(font_cache_key(&ctx).worksheet_zoom, 1.);
            });
            output.drop_without_applying_deltas();
        }
    }
    #[test]
    fn font_epoch_survives_atlas_growth_and_invalidates_after_font_replacement() {
        let ctx = Context::default();
        crate::configure(&ctx);
        let mut first = None;
        for n in 0..3 {
            let output = ctx.run_ui(egui::RawInput::default(), |ui| {
                let before = font_cache_key(&ctx);
                if n == 0 {
                    first = Some(before);
                    for size in [20., 40., 60., 80.] {
                        layout_no_wrap(
                            ui.painter(),
                            "ABCDEFGHIJKLMNOPQRSTUVWXYZ".into(),
                            FontId::proportional(size),
                            Color32::BLACK,
                        );
                    }
                } else if n == 1 {
                    assert_eq!(first, Some(before));
                    ctx.set_fonts(egui::FontDefinitions::default());
                } else {
                    assert_ne!(first.unwrap().epoch, before.epoch);
                }
            });
            output.drop_without_applying_deltas();
        }
    }
    #[test]
    fn zoom_scope_restores_after_nested_scopes_and_unwinding() {
        let ctx = Context::default();
        let output = ctx.run_ui(egui::RawInput::default(), |_| {
            with_worksheet_zoom(&ctx, 2., || {
                with_worksheet_zoom(&ctx, 0.5, || {
                    assert_eq!(font_cache_key(&ctx).worksheet_zoom, 0.5)
                });
                assert_eq!(font_cache_key(&ctx).worksheet_zoom, 2.);
            });
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                with_worksheet_zoom(&ctx, 2., || panic!("test"))
            }));
            assert_eq!(font_cache_key(&ctx).worksheet_zoom, 1.);
        });
        output.drop_without_applying_deltas();
    }
}
