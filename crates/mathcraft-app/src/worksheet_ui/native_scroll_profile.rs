//! Opt-in native surface diagnostic, never part of ordinary release builds.
//! Drives the real App, input queue and ScrollArea through a bounded gesture.
use super::*;
use serde_json::{Value, json};
use std::{io::Write, time::Instant};

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Request {
    pub(crate) scene: PathBuf,
    output: PathBuf,
    #[serde(default)]
    glow: bool,
    #[serde(default)]
    blank: bool,
    latency: Option<u32>,
    immediate: Option<bool>,
    snapshot: Option<PathBuf>,
    #[serde(default)]
    force_gpu: bool,
    #[serde(default)]
    hold: bool,
    zoom: Option<f32>,
}
impl Request {
    pub(crate) fn load() -> Option<Self> {
        let path = std::env::current_exe()
            .unwrap()
            .with_file_name("native-profile-request.json");
        if !path.exists() {
            return None;
        }
        let request: Self = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert!(request.scene.is_absolute() && request.scene.is_file());
        assert!(request.output.is_absolute() && !request.output.exists());
        if let Some(snapshot) = &request.snapshot {
            assert!(snapshot.is_absolute() && !snapshot.exists());
        }
        Some(request)
    }
    pub(crate) fn configure(&self, options: &mut eframe::NativeOptions) {
        if self.glow {
            options.renderer = eframe::Renderer::Glow;
        }
        #[cfg(windows)]
        {
            if self.force_gpu {
                options.wgpu_options.windows_cpu_fallback = false;
            }
            if let Some(latency) = self.latency {
                assert!((1..=3).contains(&latency));
                options.wgpu_options.surface.desired_maximum_frame_latency = Some(latency);
            }
            if self.immediate == Some(true) {
                options.wgpu_options.surface.present_mode = eframe::wgpu::PresentMode::AutoNoVsync;
            }
        }
        #[cfg(not(windows))]
        let _ = (self.latency, self.immediate, self.force_gpu);
    }
}

pub(crate) struct ProfiledApp {
    app: WorksheetApp,
    output: PathBuf,
    graphics: Value,
    source: Vec<u8>,
    saved: HashMap<u64, SavedCalculationRow>,
    generation: u64,
    started: Instant,
    last: Option<Instant>,
    raw_at: Instant,
    interval_ms: Option<f64>,
    index: usize,
    rows: Vec<Value>,
    finished: bool,
    blank: bool,
    snapshot: Option<PathBuf>,
    hold: bool,
}

impl ProfiledApp {
    pub(crate) fn new(
        cc: &eframe::CreationContext<'_>,
        mut app: WorksheetApp,
        request: Request,
    ) -> Self {
        let output = request.output;
        if let Some(zoom) = request.zoom {
            assert!((0.5..=2.).contains(&zoom));
            app.zoom = zoom;
        }
        assert!(output.is_absolute(), "absolute fresh output path required");
        assert!(!output.exists(), "diagnostic never overwrites output");
        assert_eq!(
            app.path.as_deref(),
            Some(request.scene.as_path()),
            "diagnostic document did not open"
        );
        cc.egui_ctx
            .data_mut(|data| data.insert_temp(egui::Id::new("native-software-statistics"), true));
        let graphics = {
            #[cfg(windows)]
            {
                if let Some(render) = cc.wgpu_render_state.as_ref() {
                    let info = render.adapter.get_info();
                    json!({"backend":format!("{:?}",info.backend),"adapter":info.name,
                    "device_type":format!("{:?}",info.device_type),"driver":info.driver,
                    "driver_info":info.driver_info,"target_format":format!("{:?}",render.target_format),
                    "present_mode":format!("{:?}",render.surface_config.present_mode),
                        "frame_latency":render.surface_config.desired_maximum_frame_latency})
                } else {
                    use eframe::glow::HasContext;
                    let gl = cc.gl.as_ref().expect("GL render state");
                    json!({"backend":"Glow","adapter":unsafe{gl.get_parameter_string(eframe::glow::RENDERER)},"version":unsafe{gl.get_parameter_string(eframe::glow::VERSION)}})
                }
            }
            #[cfg(not(windows))]
            {
                let _ = cc;
                json!({"backend":"Glow"})
            }
        };
        let now = Instant::now();
        Self {
            source: serde_json::to_vec(&app.document).unwrap(),
            saved: app.saved_results.clone(),
            generation: app.background.submitted_generation(),
            app,
            output,
            graphics,
            started: now,
            last: None,
            raw_at: now,
            interval_ms: None,
            index: 0,
            rows: Vec::with_capacity(112),
            finished: false,
            blank: request.blank,
            snapshot: request.snapshot,
            hold: request.hold,
        }
    }

    fn finish(&mut self) {
        if self.finished {
            return;
        }
        let unchanged = self.source == serde_json::to_vec(&self.app.document).unwrap()
            && self.saved == self.app.saved_results
            && self.generation == self.app.background.submitted_generation()
            && self.app.undo.is_empty()
            && self.app.redo.is_empty()
            && !self.app.dirty;
        let report = json!({"status":if self.index >= 112 && unchanged {"complete"}else{"incomplete"},
            "graphics":self.graphics,"frames":self.rows,"source_unchanged":unchanged,
            "frame_count":self.index,"seconds":self.started.elapsed().as_secs_f64(),
            "gui":true,"blank_control":self.blank,"scripted_input":true,"scope":"raw-input start intervals and App UI CPU; display timing requires external PresentMon"});
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&self.output)
            .unwrap();
        file.write_all(&serde_json::to_vec_pretty(&report).unwrap())
            .unwrap();
        self.finished = true;
    }
}

impl eframe::App for ProfiledApp {
    fn raw_input_hook(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput) {
        if self.finished {
            self.app.raw_input_hook(ctx, raw);
            return;
        }
        if let Some(path) = &self.snapshot {
            for event in &raw.events {
                if let egui::Event::Screenshot { image, .. } = event {
                    let bytes: Vec<u8> = image.pixels.iter().flat_map(|p| p.to_array()).collect();
                    image::RgbaImage::from_raw(image.size[0] as u32, image.size[1] as u32, bytes)
                        .unwrap()
                        .save(path)
                        .unwrap();
                }
            }
        }
        let now = Instant::now();
        self.interval_ms = self
            .last
            .map(|last| now.duration_since(last).as_secs_f64() * 1000.);
        self.last = Some(now);
        self.raw_at = now;
        self.index += 1;
        // Preserve native timestamps and dimensions. This is an explicitly
        // synthetic bounded gesture, not a measurement of OS wheel latency.
        raw.events.clear();
        raw.events.push(egui::Event::PointerMoved(pos2(600., 450.)));
        if (17..=96).contains(&self.index) {
            raw.events.push(egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: vec2(0., if self.index <= 56 { -12. } else { 12. }),
                phase: egui::TouchPhase::Move,
                modifiers: egui::Modifiers::NONE,
            });
        }
        self.app.raw_input_hook(ctx, raw);
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        if self.finished {
            self.app.ui(ui, frame);
            return;
        }
        let start = Instant::now();
        if self.blank {
            ui.painter().rect_filled(ui.max_rect(), 0., Color32::WHITE);
        } else {
            self.app.ui(ui, frame);
        }
        let elapsed = start.elapsed().as_secs_f64() * 1000.;
        #[cfg(windows)]
        let software = ui
            .ctx()
            .data(|data| {
                data.get_temp::<eframe::egui_wgpu::software::FrameStats>(egui::Id::new(
                    "native-software-last-frame",
                ))
            })
            .map(|s| json!({"raster_ms":s.raster_ms,"present_ms":s.present_ms,"size":s.size}));
        #[cfg(not(windows))]
        let software: Option<Value> = None;
        self.rows.push(json!({"frame":self.index,"start_ms":self.raw_at.duration_since(self.started).as_secs_f64()*1000.,
            "interval_ms":self.interval_ms,"ui_ms":elapsed,"previous_eframe_cpu_ms":frame.info().cpu_usage.map(|v|v*1000.),
            "visible_regions":self.app.visible_regions.len(),"pixels_per_point":ui.ctx().pixels_per_point(),
            "viewport":[ui.ctx().viewport_rect().width(),ui.ctx().viewport_rect().height()],"zoom":self.app.zoom,"software":software}));
        if self.index == 10 && self.snapshot.is_some() {
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
        }
        if self.index >= 112 {
            self.finish();
            if !self.hold {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            }
        } else {
            ui.ctx().request_repaint();
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.finish();
    }
}
