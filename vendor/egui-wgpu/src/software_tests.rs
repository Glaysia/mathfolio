// SPDX-License-Identifier: AGPL-3.0-only

use super::*;
use egui::epaint::{ImageDelta, Primitive};
use egui::{Mesh, Rect, TextureId, TextureOptions, pos2};

fn jobs(rect: Rect, clip: Rect, color: Color32) -> Vec<ClippedPrimitive> {
    let mut mesh = Mesh::with_texture(TextureId::Managed(0));
    mesh.add_colored_rect(rect, color);
    vec![ClippedPrimitive {
        clip_rect: clip,
        primitive: Primitive::Mesh(mesh),
    }]
}
fn full(color: Color32) -> TexturesDelta {
    let mut delta = TexturesDelta::default();
    delta.set.insert(
        TextureId::Managed(0),
        vec![ImageDelta::full(
            ColorImage::filled([1, 1], color),
            TextureOptions::LINEAR,
        )]
        .into(),
    );
    delta
}
fn draw(
    raster: &mut EguiSoftwareRender,
    buffer: &mut [[u8; 4]],
    side: usize,
    jobs: &[ClippedPrimitive],
    mut delta: TexturesDelta,
    ppp: f32,
) {
    raster.render(
        &mut BufferMutRef::new(buffer, side, side),
        jobs,
        &delta,
        ppp,
    );
    delta.clear();
}

#[test]
fn partial_texture_batches_replace_visible_pixels_without_stale_cache() {
    let mut raster = EguiSoftwareRender::new(ColorFieldOrder::Bgra).with_caching(false);
    let mut buffer = vec![[0, 0, 0, 255]; 64];
    let jobs = jobs(
        Rect::from_min_max(pos2(0., 0.), pos2(8., 8.)),
        Rect::EVERYTHING,
        Color32::WHITE,
    );
    draw(&mut raster, &mut buffer, 8, &jobs, full(Color32::RED), 1.);
    assert_eq!(buffer[27], [0, 0, 255, 255]);
    let mut delta = TexturesDelta::default();
    delta.set.insert(
        TextureId::Managed(0),
        vec![
            ImageDelta::partial(
                [0, 0],
                ColorImage::filled([1, 1], Color32::GREEN),
                TextureOptions::LINEAR,
            ),
            ImageDelta::partial(
                [0, 0],
                ColorImage::filled([1, 1], Color32::BLUE),
                TextureOptions::LINEAR,
            ),
        ]
        .into(),
    );
    draw(&mut raster, &mut buffer, 8, &jobs, delta, 1.);
    assert_eq!(buffer[27], [255, 0, 0, 255]);
}

#[test]
fn large_paper_is_rendered_and_clipped_at_physical_dpi() {
    let mut raster = EguiSoftwareRender::new(ColorFieldOrder::Bgra).with_caching(false);
    let background = [32, 64, 128, 255];
    let mut buffer = vec![background; 400];
    let jobs = jobs(
        Rect::from_min_max(pos2(-10., -10.), pos2(100_000., 100_000.)),
        Rect::from_min_max(pos2(2., 3.), pos2(6., 7.)),
        Color32::WHITE,
    );
    draw(
        &mut raster,
        &mut buffer,
        20,
        &jobs,
        full(Color32::WHITE),
        2.,
    );
    for y in 0..20 {
        for x in 0..20 {
            let expected = if (4..12).contains(&x) && (6..14).contains(&y) {
                [255; 4]
            } else {
                background
            };
            assert_eq!(buffer[y * 20 + x], expected, "pixel {x},{y}");
        }
    }
}

#[test]
fn translucent_overlapping_shapes_keep_premultiplied_colors() {
    let mut raster = EguiSoftwareRender::new(ColorFieldOrder::Bgra).with_caching(false);
    let mut buffer = vec![[255, 0, 0, 255]; 64];
    let jobs = jobs(
        Rect::from_min_max(pos2(0., 0.), pos2(8., 8.)),
        Rect::EVERYTHING,
        Color32::from_rgba_unmultiplied(255, 0, 0, 128),
    );
    draw(&mut raster, &mut buffer, 8, &jobs, full(Color32::WHITE), 1.);
    let actual = buffer[27];
    assert!(
        actual[0].abs_diff(127) <= 1
            && actual[1] == 0
            && actual[2].abs_diff(128) <= 1
            && actual[3] == 255,
        "{actual:?}"
    );
}
