//! Windows fallback for CPU adapters. Shares eframe's existing window/input loop.
// SPDX-License-Identifier: AGPL-3.0-only
use egui::{ClippedPrimitive, Color32, ColorImage, Context, Id, TexturesDelta};
use egui_software_backend::{BufferMutRef, ColorFieldOrder, EguiSoftwareRender};
use std::{num::NonZeroU32, sync::Arc, time::Instant};
use winit::window::Window;

/// Previous paint timings for the opt-in native diagnostic.
#[derive(Clone, Copy, Debug)]
pub struct FrameStats {
    pub raster_ms: f64,
    pub present_ms: f64,
    pub size: [u32; 2],
}

pub(crate) struct Surface {
    // Surface before display context: native resources release in this order.
    surface: softbuffer::Surface<Arc<Window>, Arc<Window>>,
    _display: softbuffer::Context<Arc<Window>>,
    raster: EguiSoftwareRender,
    window: Arc<Window>,
    size: [u32; 2],
}

impl Surface {
    pub(crate) fn new(window: Arc<Window>) -> Result<Self, softbuffer::SoftBufferError> {
        let display = softbuffer::Context::new(Arc::clone(&window))?;
        let surface = softbuffer::Surface::new(&display, Arc::clone(&window))?;
        Ok(Self {
            surface,
            _display: display,
            window,
            size: [0, 0],
            // Avoid mesh-hash collisions or stale pixels after texture updates.
            // The direct SIMD rasterizer is already fast enough for this UI.
            raster: EguiSoftwareRender::new(ColorFieldOrder::Bgra).with_caching(false),
        })
    }

    pub(crate) fn paint(
        &mut self,
        ctx: &Context,
        jobs: &[ClippedPrimitive],
        deltas: &mut TexturesDelta,
        pixels_per_point: f32,
        clear: [f32; 4],
        capture: bool,
    ) -> Result<Option<ColorImage>, softbuffer::SoftBufferError> {
        let size = self.window.inner_size();
        let (Some(width), Some(height)) =
            (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
        else {
            return Ok(None);
        };
        let size = [width.get(), height.get()];
        if self.size != size {
            self.surface.resize(width, height)?;
            self.size = size;
        }
        let mut buffer = self.surface.buffer_mut()?;
        let channel = |c: f32| (c.clamp(0., 1.) * 255.).round() as u32;
        buffer.fill(
            channel(clear[2])
                | channel(clear[1]) << 8
                | channel(clear[0]) << 16
                | channel(clear[3]) << 24,
        );
        let started = Instant::now();
        self.raster.render(
            &mut BufferMutRef::new(
                bytemuck::cast_slice_mut(&mut buffer),
                width.get() as usize,
                height.get() as usize,
            ),
            jobs,
            deltas,
            pixels_per_point,
        );
        let raster_ms = started.elapsed().as_secs_f64() * 1000.;
        // Texture deltas belong to the CPU renderer on this path, never WGPU.
        deltas.clear();
        let screenshot = capture.then(|| {
            ColorImage::new(
                [width.get() as usize, height.get() as usize],
                buffer
                    .iter()
                    .map(|&p| Color32::from_rgb((p >> 16) as u8, (p >> 8) as u8, p as u8))
                    .collect(),
            )
        });
        let start = Instant::now();
        self.window.pre_present_notify();
        buffer.present()?;
        let present_ms = start.elapsed().as_secs_f64() * 1000.;
        // Instrumentation is enabled by the diagnostic App, absent ordinarily.
        if ctx.data(|data| data.get_temp::<bool>(Id::new("native-software-statistics")))
            == Some(true)
        {
            ctx.data_mut(|data| {
                data.insert_temp(
                    Id::new("native-software-last-frame"),
                    FrameStats {
                        raster_ms,
                        present_ms,
                        size,
                    },
                )
            });
        }
        Ok(screenshot)
    }
}

#[cfg(test)]
#[path = "software_tests.rs"]
mod tests;
