//! Artifact-only Windows worksheet_ui child. No surface/HWND/GL/readback.
//! Initialization runner recipe is in README; this is the exact frame core.
use super::*;
use eframe::{
    egui_wgpu::{Renderer, RendererOptions, ScreenDescriptor},
    wgpu,
};
use std::time::{Duration, Instant};

const WIDTH: u32 = 1200;
const HEIGHT: u32 = 900;
const ATLAS_SIDE: usize = 2048;
const BUDGET: u64 = 64 * 1024 * 1024;

// This diagnostic applies uploads/frees manually. Always acknowledge the
// commands, including an early admission error, before epaint's Drop check.
// Retaining the real error must not be replaced by an unapplied-delta panic.
struct PendingTextures(egui::TexturesDelta);
impl std::ops::Deref for PendingTextures {
    type Target = egui::TexturesDelta;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl Drop for PendingTextures {
    fn drop(&mut self) {
        self.0.clear();
    }
}

pub(super) struct OffscreenFrames {
    renderer: Renderer,
    _target: wgpu::Texture,
    view: wgpu::TextureView,
    vertex_capacity: u64,
    index_capacity: u64,
    atlas: [usize; 2],
    frame: u64,
}

impl OffscreenFrames {
    /// Caller initializes a DX12/Vulkan-only headless device first. No surface.
    /// BGRA8Unorm is an explicit offscreen policy, not observed GUI swap format.
    pub fn new(device: &wgpu::Device, options: RendererOptions) -> Result<Self, String> {
        if options.msaa_samples > 1 || options.depth_stencil_format.is_some() {
            return Err("Bounded probe supports no MSAA/depth allocation".into());
        }
        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("bounded-headless-no-readback"),
            size: wgpu::Extent3d {
                width: WIDTH,
                height: HEIGHT,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Bgra8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = target.create_view(&wgpu::TextureViewDescriptor::default());
        Ok(Self {
            renderer: Renderer::new(device, wgpu::TextureFormat::Bgra8Unorm, options),
            _target: target,
            view,
            vertex_capacity: (std::mem::size_of::<egui::epaint::Vertex>() * 1024) as u64,
            index_capacity: (std::mem::size_of::<u32>() * 1024 * 3) as u64,
            atlas: [0, 0],
            frame: 0,
        })
    }

    /// Builds the actual own app scene then renders it into an unread texture.
    /// FullOutput is explicitly disposed BEFORE all fallible GPU/budget work.
    pub fn frame(
        &mut self,
        app: &mut WorksheetApp,
        ctx: &egui::Context,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<serde_json::Value, String> {
        let frame_start = Instant::now();
        let mut raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(
                egui::Pos2::ZERO,
                vec2(WIDTH as f32, HEIGHT as f32),
            )),
            max_texture_side: Some(ATLAS_SIDE),
            time: Some(self.frame as f64 / 60.),
            ..Default::default()
        };
        raw.viewports
            .get_mut(&egui::ViewportId::ROOT)
            .ok_or("Root metadata missing")?
            .native_pixels_per_point = Some(1.);
        let raw_start = Instant::now();
        eframe::App::raw_input_hook(app, ctx, &mut raw);
        let raw_input_ns = raw_start.elapsed().as_nanos();
        let ui_start = Instant::now();
        let mut output = ctx.run_ui(raw, |ui| app.app_ui_frame(ui));
        let ui_ns = ui_start.elapsed().as_nanos();
        let ppp = output.pixels_per_point;
        let shapes = std::mem::take(&mut output.shapes);
        let deltas = PendingTextures(std::mem::take(&mut output.textures_delta));
        let repaint_delay_ns = output
            .viewport_output
            .get(&egui::ViewportId::ROOT)
            .and_then(|viewport| u64::try_from(viewport.repaint_delay.as_nanos()).ok());
        let repaint_delay_infinite = output
            .viewport_output
            .get(&egui::ViewportId::ROOT)
            .map(|viewport| viewport.repaint_delay == Duration::MAX);
        let drop_start = Instant::now();
        output.drop_without_applying_deltas();
        let output_dispose_ns = drop_start.elapsed().as_nanos();
        let visible_math_regions = app
            .document
            .regions
            .iter()
            .filter(|region| {
                app.visible_regions.contains(&region.id)
                    && matches!(&region.content, CanvasContent::Math(_))
            })
            .count();
        if visible_math_regions == 0 {
            return Err("No visible worksheet Math region in the fixed LCC scene".into());
        }
        let tess_start = Instant::now();
        let meshes = ctx.tessellate(shapes, ppp);
        let tessellation_ns = tess_start.elapsed().as_nanos();
        if ppp.to_bits() != 1f32.to_bits() {
            return Err("Probe fixed target requires PPP1".into());
        }
        let (vertex_bytes, index_bytes) = admit_meshes(&meshes)?;
        let next_vertex = if vertex_bytes > self.vertex_capacity {
            (self.vertex_capacity * 2).max(vertex_bytes)
        } else {
            self.vertex_capacity
        };
        let next_index = if index_bytes > self.index_capacity {
            (self.index_capacity * 2).max(index_bytes)
        } else {
            self.index_capacity
        };
        let mut texture_updates = 0usize;
        let mut texture_delta_bytes = 0usize;
        let mut full_replacements = 0u64;
        let mut atlas_peak_pixels = self.atlas[0] * self.atlas[1];
        if deltas
            .free
            .iter()
            .any(|id| *id != egui::TextureId::Managed(0))
        {
            return Err("Only atlas0 free admitted".into());
        }
        if deltas.free.len() > 1 {
            return Err("Duplicate atlas0 free forbidden".into());
        }
        for (id, updates) in &deltas.set {
            if *id != egui::TextureId::Managed(0) {
                return Err("Only atlas0 upload admitted".into());
            }
            for delta in updates {
                let [w, h] = delta.image.size();
                if w == 0 || h == 0 || w > ATLAS_SIDE || h > ATLAS_SIDE {
                    return Err("Atlas upload dimension cap exceeded".into());
                }
                match &delta.image {
                    egui::epaint::ImageData::Color(image) if image.pixels.len() == w * h => {}
                    _ => return Err("Atlas pixel count mismatch".into()),
                }
                if let Some([x, y]) = delta.pos {
                    if x.checked_add(w).is_none_or(|n| n > self.atlas[0])
                        || y.checked_add(h).is_none_or(|n| n > self.atlas[1])
                    {
                        return Err("Atlas sub-update out of bounds".into());
                    }
                } else {
                    full_replacements += 1;
                    if full_replacements > 2 {
                        return Err("At most two atlas replacements per frame".into());
                    }
                    self.atlas = [w, h];
                    atlas_peak_pixels = atlas_peak_pixels.max(w * h);
                }
                texture_updates += 1;
                texture_delta_bytes = texture_delta_bytes
                    .checked_add(w * h * 4)
                    .ok_or("Texture delta sum overflow")?;
                if texture_updates > 64 || texture_delta_bytes > 32 * 1024 * 1024 {
                    return Err("Bounded atlas update count/bytes exceeded".into());
                }
            }
        }
        if !deltas.free.is_empty() && self.atlas.contains(&0) {
            return Err("Free of missing atlas0 forbidden".into());
        }
        if !meshes.is_empty() && self.atlas.contains(&0) {
            return Err("Mesh font atlas is unavailable".into());
        }
        // Prior atlas plus each full replacement, conservatively at the largest
        // encountered extent. Previous frame completion was already awaited.
        // Buffer old+new generations and4KiB uniform remain separate. Not RSS.
        let explicit_resource_bound = u64::from(WIDTH) * u64::from(HEIGHT) * 4
            + atlas_peak_pixels as u64 * 4 * (full_replacements + 1)
            + (next_vertex + next_index) * 2
            + 4096;
        if explicit_resource_bound > BUDGET {
            return Err("Explicit resource64MiB budget exceeded".into());
        }
        self.vertex_capacity = next_vertex;
        self.index_capacity = next_index;
        let texture_start = Instant::now();
        for (id, updates) in &deltas.set {
            for delta in updates {
                self.renderer.update_texture(device, queue, *id, delta);
            }
        }
        let update_texture_ns = texture_start.elapsed().as_nanos();
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("headless-egui-frame"),
        });
        let screen = ScreenDescriptor {
            size_in_pixels: [WIDTH, HEIGHT],
            pixels_per_point: ppp,
        };
        let buffers_start = Instant::now();
        let callbacks = self
            .renderer
            .update_buffers(device, queue, &mut encoder, &meshes, &screen);
        let update_buffers_ns = buffers_start.elapsed().as_nanos();
        if !callbacks.is_empty() {
            return Err("Unexpected callback command buffers".into());
        }
        let encode_start = Instant::now();
        {
            let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("headless-no-surface"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            self.renderer
                .render(&mut pass.forget_lifetime(), &meshes, &screen);
        }
        let command = encoder.finish();
        let encode_ns = encode_start.elapsed().as_nanos();
        let submit_start = Instant::now();
        let submission = queue.submit([command]);
        let submit_host_ns = submit_start.elapsed().as_nanos();
        let wait_start = Instant::now();
        let completion = device.poll(wgpu::PollType::Wait {
            submission_index: Some(submission),
            timeout: Some(Duration::from_secs(10)),
        });
        let completion_wait_ns = wait_start.elapsed().as_nanos();
        for id in &deltas.free {
            self.renderer.free_texture(id);
            if *id == egui::TextureId::Managed(0) {
                self.atlas = [0, 0];
            }
        }
        completion.map_err(|e| format!("Submission completion unavailable: {e}"))?;
        let mesh_count = meshes.len();
        let data_drop_start = Instant::now();
        drop(meshes);
        drop(deltas);
        let data_drop_ns = data_drop_start.elapsed().as_nanos();
        let recorded_phase_sum_ns = raw_input_ns
            + ui_ns
            + output_dispose_ns
            + tessellation_ns
            + update_texture_ns
            + update_buffers_ns
            + encode_ns
            + submit_host_ns
            + completion_wait_ns
            + data_drop_ns;
        let whole_frame_wall_ns = frame_start.elapsed().as_nanos();
        let unaccounted_wall_ns = whole_frame_wall_ns
            .checked_sub(recorded_phase_sum_ns)
            .ok_or("Frame timing accounting invalid")?;
        let row = serde_json::json!({"frame":self.frame,"warm":self.frame<2,
            "raw_input_hook_host_ns":raw_input_ns,"ui_host_ns":ui_ns,"output_dispose_host_ns":output_dispose_ns,
            "tessellation_host_ns":tessellation_ns,"update_texture_host_ns":update_texture_ns,
            "update_buffers_host_ns":update_buffers_ns,"encode_host_ns":encode_ns,"submit_host_ns":submit_host_ns,
            "completion_wait_wall_ns":completion_wait_ns,"gpu_timestamp_ns":null,"mesh_count":mesh_count,
            "worksheet_visible":true,"visible_math_regions":visible_math_regions,"mesh_finite_and_nonempty":true,
            "vertex_bytes":vertex_bytes,"index_bytes":index_bytes,"texture_updates":texture_updates,"texture_delta_bytes":texture_delta_bytes,
            "explicit_resource_bound_bytes":explicit_resource_bound,"atlas_full_replacements":full_replacements,
            "repaint_delay_ns":repaint_delay_ns,"repaint_delay_infinite":repaint_delay_infinite,
            "whole_frame_wall_ns":whole_frame_wall_ns,"recorded_phase_sum_ns":recorded_phase_sum_ns,
            "unaccounted_wall_ns":unaccounted_wall_ns,"data_drop_host_ns":data_drop_ns,
            "render_format":"Bgra8Unorm","surface":false,"readback":false});
        self.frame += 1;
        Ok(row)
    }
}

// Appended after the reviewed OffscreenFrames implementation.
#[path = "offscreen_wgpu_initialize.rs"]
mod initialize;
use sha2::{Digest, Sha256};
use std::{
    future::Future,
    io::{Read, Write},
    sync::Arc,
    task::{Context as TaskContext, Poll, Wake, Waker},
};

struct ThreadWake(std::thread::Thread);
impl Wake for ThreadWake {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        self.0.unpark();
    }
}
fn wait_future<F: Future>(future: F) -> Result<F::Output, String> {
    let waker = Waker::from(Arc::new(ThreadWake(std::thread::current())));
    let mut context = TaskContext::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if Instant::now() >= deadline {
            return Err("Future deadline exceeded".into());
        }
        if let Poll::Ready(value) = future.as_mut().poll(&mut context) {
            return Ok(value);
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        std::thread::park_timeout(remaining.min(Duration::from_millis(50)));
    }
}
fn digest_bytes(value: &[u8]) -> String {
    format!("{:x}", Sha256::digest(value))
}
fn digest_file(path: &Path, maximum: u64) -> Result<(u64, String), String> {
    let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    if file.metadata().map_err(|e| e.to_string())?.len() > maximum {
        return Err("Hash input too large".into());
    }
    let mut bytes = 0u64;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let count = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        bytes = bytes
            .checked_add(count as u64)
            .filter(|n| *n <= maximum)
            .ok_or("Growing hash input too large")?;
        hash.update(&buffer[..count]);
    }
    Ok((bytes, format!("{:x}", hash.finalize())))
}
fn bounded_text(value: &str) -> String {
    value.chars().take(1024).collect()
}
fn panic_text(value: Box<dyn std::any::Any + Send>) -> String {
    value
        .downcast_ref::<String>()
        .map(|s| bounded_text(s))
        .or_else(|| value.downcast_ref::<&str>().map(|s| bounded_text(s)))
        .unwrap_or_else(|| "non-string panic".into())
}

// Insert before run_scene. Test-only own source checks, outside row timers.
fn read_bounded_scene(path: &Path) -> Result<(Vec<u8>, std::fs::File), String> {
    use std::os::windows::fs::OpenOptionsExt;
    const MAXIMUM: u64 = 16 * 1024 * 1024;
    // FILE_SHARE_READ only: keep this reader alive through App::open, whose
    // native importer reads the path again. Existing/future ordinary writers
    // and delete/replace handles are incompatible; never change source bytes.
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(1)
        .open(path)
        .map_err(|e| e.to_string())?;
    if file.metadata().map_err(|e| e.to_string())?.len() > MAXIMUM {
        return Err("Scene import cap16MiB exceeded".into());
    }
    let mut bytes = Vec::new();
    (&mut file)
        .take(MAXIMUM + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAXIMUM {
        return Err("Growing scene import cap16MiB exceeded".into());
    }
    Ok((bytes, file))
}

fn admit_meshes(meshes: &[egui::ClippedPrimitive]) -> Result<(u64, u64), String> {
    let mut vertices = 0u64;
    let mut indices = 0u64;
    for job in meshes {
        let egui::epaint::Primitive::Mesh(mesh) = &job.primitive else {
            return Err("GPU callbacks forbidden in bounded probe".into());
        };
        if mesh.texture_id != egui::TextureId::Managed(0) {
            return Err("Only font atlas0 admitted".into());
        }
        let size = mesh
            .vertices
            .len()
            .checked_mul(std::mem::size_of::<egui::epaint::Vertex>())
            .ok_or("Vertex size overflow")?;
        vertices = vertices
            .checked_add(size as u64)
            .ok_or("Vertex budget overflow")?;
        let size = mesh
            .indices
            .len()
            .checked_mul(std::mem::size_of::<u32>())
            .ok_or("Index size overflow")?;
        indices = indices
            .checked_add(size as u64)
            .ok_or("Index budget overflow")?;
        if vertices
            .checked_add(indices)
            .is_none_or(|n| n > 2 * 1024 * 1024)
        {
            return Err("Mesh upload cap2MiB exceeded".into());
        }
    }
    if vertices == 0 || indices == 0 {
        return Err("Empty tessellation is not a valid fixed-scene workload".into());
    }
    let mut nondegenerate = false;
    for job in meshes {
        let egui::epaint::Primitive::Mesh(mesh) = &job.primitive else {
            unreachable!()
        };
        if !job.clip_rect.min.is_finite()
            || !job.clip_rect.max.is_finite()
            || !job.clip_rect.is_positive()
            || mesh.indices.len() % 3 != 0
            || mesh
                .indices
                .iter()
                .any(|index| *index as usize >= mesh.vertices.len())
            || mesh
                .vertices
                .iter()
                .any(|vertex| !vertex.pos.is_finite() || !vertex.uv.is_finite())
        {
            return Err("Nonfinite or invalid triangle mesh".into());
        }
        for triangle in mesh.indices.as_chunks::<3>().0 {
            let a = mesh.vertices[triangle[0] as usize].pos;
            let b = mesh.vertices[triangle[1] as usize].pos;
            let c = mesh.vertices[triangle[2] as usize].pos;
            let area = (f64::from(b.x) - f64::from(a.x)) * (f64::from(c.y) - f64::from(a.y))
                - (f64::from(b.y) - f64::from(a.y)) * (f64::from(c.x) - f64::from(a.x));
            nondegenerate |= area != 0.;
        }
    }
    if !nondegenerate {
        return Err("No nondegenerate triangle in fixed-scene tessellation".into());
    }
    Ok((vertices, indices))
}

struct EditorState {
    tree: PortableTree,
    revision: u64,
    caret: mathcraft_editor::Caret,
    selection: mathcraft_editor::Selection,
    scope_direction: Direction,
    typeover: Option<mathcraft_editor::NodeId>,
    excluded_operand: Option<mathcraft_editor::NodeId>,
    expressions_only: bool,
    radix_mode: bool,
}
impl EditorState {
    fn capture(editor: &Editor) -> Result<Self, String> {
        Ok(Self {
            tree: editor
                .to_portable()
                .map_err(|e| format!("Editor source export: {e:?}"))?,
            revision: editor.arena().revision(),
            caret: editor.caret(),
            selection: editor.selection(),
            scope_direction: editor.scope_direction(),
            typeover: editor.typeover_target(),
            excluded_operand: editor.typeover_excluded_operand(),
            expressions_only: editor.expressions_only(),
            radix_mode: editor.bin_oct_hex_mode(),
        })
    }
    fn check(&self, editor: &Editor) -> Result<(), String> {
        let current = Self::capture(editor)?;
        if current.tree != self.tree
            || current.revision != self.revision
            || current.caret != self.caret
            || current.selection != self.selection
            || current.scope_direction != self.scope_direction
            || current.typeover != self.typeover
            || current.excluded_operand != self.excluded_operand
            || current.expressions_only != self.expressions_only
            || current.radix_mode != self.radix_mode
        {
            return Err("Existing Math editor source/caret/config changed".into());
        }
        Ok(())
    }
}
#[derive(Default)]
struct EditorSourceOracle(HashMap<u64, EditorState>);
impl EditorSourceOracle {
    fn check_and_extend(&mut self, app: &WorksheetApp) -> Result<(), String> {
        for (id, state) in &self.0 {
            state.check(
                app.editors
                    .get(id)
                    .ok_or("Previously observed Math editor disappeared")?,
            )?;
        }
        for (id, editor) in &app.editors {
            if self.0.contains_key(id) {
                continue;
            }
            let state = EditorState::capture(editor)?;
            let source = app
                .document
                .regions
                .iter()
                .find(|region| region.id == *id)
                .ok_or("Editor owner missing")?;
            if !matches!(&source.content, CanvasContent::Math(tree) if tree == &state.tree) {
                return Err("Newly cached editor differs from committed Math source".into());
            }
            self.0.insert(*id, state);
        }
        Ok(())
    }
}

#[test]
fn offscreen_mesh_guard_rejects_empty_nonfinite_and_invalid_indices_without_device() {
    assert!(admit_meshes(&[]).is_err());
    let vertex = egui::epaint::Vertex {
        pos: egui::pos2(1., 1.),
        uv: egui::pos2(0., 0.),
        color: egui::Color32::WHITE,
    };
    let mesh = egui::epaint::Mesh {
        texture_id: egui::TextureId::Managed(0),
        vertices: vec![
            vertex,
            egui::epaint::Vertex {
                pos: egui::pos2(9., 1.),
                ..vertex
            },
            egui::epaint::Vertex {
                pos: egui::pos2(1., 9.),
                ..vertex
            },
        ],
        indices: vec![0, 1, 2],
    };
    let good = egui::ClippedPrimitive {
        clip_rect: Rect::from_min_size(egui::Pos2::ZERO, vec2(10., 10.)),
        primitive: egui::epaint::Primitive::Mesh(mesh.clone()),
    };
    assert!(admit_meshes(&[good]).is_ok());
    for fault in 0..3 {
        let mut bad = mesh.clone();
        match fault {
            0 => bad.vertices[0].pos.x = f32::NAN,
            1 => bad.vertices[1].uv.y = f32::INFINITY,
            _ => bad.indices[2] = 99,
        }
        assert!(
            admit_meshes(&[egui::ClippedPrimitive {
                clip_rect: Rect::from_min_size(egui::Pos2::ZERO, vec2(10., 10.)),
                primitive: egui::epaint::Primitive::Mesh(bad)
            }])
            .is_err()
        );
    }
}

fn run_scene(report: &mut serde_json::Value) -> Result<(), String> {
    let path = std::env::var_os("MATHCRAFT_GRAPHICS_SCENE")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../artifacts/rea/unit-selected-sources/1015_LCC_S.mcdx")
        });
    let (bytes, scene_read_guard) = read_bounded_scene(&path)?;
    let hash = digest_bytes(&bytes);
    if hash != "217994a631de58e096a3cfdb8a8251aca91501e62151f869950b99f738fc18b5" {
        return Err("This bounded scene probe requires the retained 1015 input hash".into());
    }
    report["input"] = serde_json::json!({"path":path,"bytes":bytes.len(),"sha256":hash});
    let open_start = Instant::now();
    let mut app = WorksheetApp {
        automatic_calculation: false,
        zoom: 1.,
        ..Default::default()
    };
    app.open(&path);
    report["open_host_wall_ns"] = serde_json::json!(open_start.elapsed().as_nanos());
    if !app.status.starts_with("Opened") {
        return Err(format!("Scene open: {}", bounded_text(&app.status)));
    }
    if app.document.native_archive.as_deref() != Some(bytes.as_slice()) {
        return Err("Loaded scene differs from hashed input".into());
    }
    drop(scene_read_guard);
    let math_regions = app
        .document
        .regions
        .iter()
        .filter(|r| matches!(&r.content, CanvasContent::Math(_)))
        .count();
    if math_regions == 0 {
        return Err("Fixed LCC document has no parsed Math source".into());
    }
    report["workload"] = serde_json::json!({"document_regions":app.document.regions.len(),"math_regions":math_regions});
    let expected_zoom = app.zoom.to_bits();
    if expected_zoom != 1f32.to_bits() {
        return Err("Fixed scene requires initial worksheet zoom1".into());
    }
    let mut editor_oracle = EditorSourceOracle::default();
    editor_oracle.check_and_extend(&app)?;
    let oracle = super::full_app_profile::ReadOnlyOracle::new(&app);
    let dom = app.background.preparation.full_dom_parse_count();
    oracle.check(&app);
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    let initialize_start = Instant::now();
    let (_instance, device, queue, options, gpu_report) =
        wait_future(initialize::initialize_primary_only())??;
    report["initialize_host_wall_ns"] = serde_json::json!(initialize_start.elapsed().as_nanos());
    report["graphics"] = gpu_report;
    let constructor_start = Instant::now();
    let mut frames = OffscreenFrames::new(&device, options)?;
    report["renderer_constructor_host_wall_ns"] =
        serde_json::json!(constructor_start.elapsed().as_nanos());
    report["full_dom_initial"] = dom.into();
    report["submitted_generation"] = app.background.submitted_generation().into();
    for _ in 0..12 {
        let frame = frames.frame(&mut app, &ctx, &device, &queue);
        // The frame function disposes FullOutput before all GPU/budget checks.
        oracle.check(&app);
        if app.zoom.to_bits() != expected_zoom {
            return Err("Worksheet zoom changed".into());
        }
        editor_oracle.check_and_extend(&app)?;
        assert_eq!(app.background.preparation.full_dom_parse_count(), dom);
        {
            let mut row = frame?;
            row["readonly_invariants"] = true.into();
            row["existing_math_editor_source_state"] = true.into();
            row["observed_math_editors"] = editor_oracle.0.len().into();
            row["full_dom_parses"] = dom.into();
            report["frames"]
                .as_array_mut()
                .ok_or("Receipt frames unavailable")?
                .push(row);
        }
    }
    oracle.finish(&app);
    assert_eq!(app.background.preparation.full_dom_parse_count(), dom);
    let rows = report["frames"]
        .as_array()
        .ok_or("Receipt frames unavailable")?;
    assert_eq!(rows.len(), 12);
    let idle = &rows[2..];
    let mut summary = serde_json::Map::new();
    for key in [
        "raw_input_hook_host_ns",
        "ui_host_ns",
        "tessellation_host_ns",
        "update_texture_host_ns",
        "update_buffers_host_ns",
        "encode_host_ns",
        "submit_host_ns",
        "completion_wait_wall_ns",
        "whole_frame_wall_ns",
    ] {
        let mut values = idle
            .iter()
            .map(|row| row[key].as_u64().ok_or("Missing bounded timing"))
            .collect::<Result<Vec<_>, _>>()?;
        values.sort_unstable();
        summary.insert(key.into(), serde_json::json!({"minimum_ns":values[0],"middle_pair_ns":[values[4],values[5]],"maximum_ns":values[9]}));
    }
    report["idle_descriptive_only"] = serde_json::Value::Object(summary);
    report["source_wire_unchanged"] = true.into();
    report["math_editor_oracle"] = "new entries match document source; existing root/revision/caret/selection/scope/typeover/modes retained; internal Editor undo stacks not exposed".into();
    report["readonly_invariants"] = true.into();
    report["full_dom_final"] = dom.into();
    Ok(())
}

#[test]
#[ignore = "Optional own-App offscreen WGPU diagnostic; external process timeout mandatory"]
fn offscreen_full_app_wgpu_probe() {
    assert_eq!(
        std::env::var("MATHCRAFT_GRAPHICS_PROCESS_GUARD").as_deref(),
        Ok("bounded-child-v2"),
        "Use the external timeout runner"
    );
    let directory = std::env::var_os("MATHCRAFT_GRAPHICS_OUTPUT")
        .map(PathBuf::from)
        .expect("Set new MATHCRAFT_GRAPHICS_OUTPUT directory");
    assert!(!directory.exists(), "Receipt directory must be new");
    std::fs::create_dir(&directory).expect("Create exclusive receipt directory");
    let executable = std::env::current_exe().expect("Current harness path");
    let (exe_bytes, exe_hash) =
        digest_file(&executable, 256 * 1024 * 1024).expect("Bounded harness identity");
    let mut report = serde_json::json!({"status":"starting","scope":"actual own raw_input_hook/App::ui/tessellation plus unread offscreen renderer",
        "original_runtime_executed":false,"gui":false,"surface":false,"window_created_by_probe":false,"gl_enabled":false,
        "readback":false,"image_export":false,"gpu_timestamp_queries":false,"actual_gui_selected":null,
        "host_timings_are_wall_not_pure_cpu":true,"completion_wait_is_wall_not_gpu_timestamp":true,
        "renderer_present_compositor_remote_display_absent":true,"driver_internal_allocations_or_windows_not_observed":true,
        "configuration":"NativeOptions defaults except explicit primary-only headless request and Bgra8Unorm target",
        "limits":{"json_bytes":262144,"fixture_bytes":16777216,"mesh_upload_bytes":2097152,"atlas_side":2048,
            "known_explicit_resource_bytes":67108864,"strict_process_or_RSS_bound":false,"wait_seconds":10,"future_seconds":15,
            "outer_process_timeout_required":true},
        "source":{"probe_sha256":digest_bytes(include_bytes!("offscreen_wgpu_probe.rs")),
            "initialize_sha256":digest_bytes(include_bytes!("offscreen_wgpu_initialize.rs")),
            "oracle_sha256":digest_bytes(include_bytes!("full_app_profile.rs")),
            "app_sha256":digest_bytes(include_bytes!("../worksheet_ui.rs")),
            "preparation_sha256":digest_bytes(include_bytes!("edit_commit.rs"))},
        "harness":{"path":executable,"bytes":exe_bytes,"sha256":exe_hash},"frames":[],"warm_frames":2,"idle_frames":10,
        "calculation_executions":0,"calculation_zero_basis":"App dispatch/state invariant, not a separate engine API instrumentation counter"});
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run_scene(&mut report)));
    let invariant_or_backend_panic = match result {
        Ok(Ok(())) => {
            report["status"] = "complete".into();
            false
        }
        Ok(Err(error)) => {
            report["status"] = "unsupported-or-device-error".into();
            report["error"] = bounded_text(&error).into();
            false
        }
        Err(error) => {
            report["status"] = "panic-or-invariant-failure".into();
            report["error"] = panic_text(error).into();
            true
        }
    };
    let bytes = serde_json::to_vec_pretty(&report).expect("Serialize bounded receipt");
    assert!(bytes.len() <= 262144, "Receipt unexpectedly large");
    let destination = directory.join("observation.json");
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&destination)
        .expect("New receipt");
    file.write_all(&bytes).expect("Write receipt");
    file.sync_all().expect("Sync receipt");
    println!(
        "OFFSCREEN_WGPU status={} frames={} original_runtime=false gui=false readback=false",
        report["status"],
        report["frames"].as_array().map_or(0, Vec::len)
    );
    assert!(
        !invariant_or_backend_panic,
        "See offscreen receipt; no complete timing claim"
    );
}
