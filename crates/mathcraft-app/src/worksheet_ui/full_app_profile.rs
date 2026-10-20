//! Artifact candidate: exact full App body CPU, no native Frame/window/GPU.
//! Install the narrow extraction/module/metadata patch beside this module.
use super::*;
use sha2::{Digest, Sha256};
use std::{cell::RefCell, path::Path, time::Instant};

const ML: &str = "http://schemas.mathsoft.com/math50";
const IDLE_FRAMES: usize = 10;
const MAX_SETTLE_FRAMES: usize = 48;
const STABLE_FRAMES: usize = 3;

#[derive(Clone, Copy)]
struct WorkspaceMetadata {
    scroll_id: egui::Id,
    layer: egui::LayerId,
    clip: Rect,
}
#[derive(Default)]
struct WorkspaceObservation {
    last: Option<WorkspaceMetadata>,
    passes: usize,
}
thread_local! {
    static WORKSPACE: RefCell<Option<WorkspaceObservation>> = const { RefCell::new(None) };
}
/// Called only by a cfg(test) hook in the actual workspace_ui. No timing here.
pub(super) fn observe_workspace(ui: &egui::Ui) {
    WORKSPACE.with_borrow_mut(|active| {
        if let Some(active) = active {
            active.passes += 1;
            active.last = Some(WorkspaceMetadata {
                scroll_id: ui.make_persistent_id(egui::IdSalt::new("free-worksheet")),
                layer: egui::LayerId::new(ui.layer_id().order, egui::Id::new("worksheet-paper")),
                clip: ui.clip_rect(),
            });
        }
    });
}

pub(super) struct ReadOnlyOracle {
    document: CanvasDocument,
    wire: Vec<u8>,
    saved: HashMap<u64, SavedCalculationRow>,
    pending: HashSet<u64>,
    generation: u64,
    selected: Option<u64>,
    editing: Option<u64>,
}
impl ReadOnlyOracle {
    pub(super) fn new(app: &WorksheetApp) -> Self {
        Self {
            document: app.document.clone(),
            wire: serde_json::to_vec(&app.document).unwrap(),
            saved: app.saved_results.clone(),
            pending: app.pending_regions.clone(),
            generation: app.background.submitted_generation(),
            selected: app.selected,
            editing: app.editing_region,
        }
    }
    // Outside every frame timer. Cache creation for newly visible content is
    // allowed; source/result/status/selection/command state is not changed.
    pub(super) fn check(&self, app: &WorksheetApp) {
        assert_eq!(app.document, self.document);
        assert_eq!(app.saved_results, self.saved);
        assert_eq!(app.pending_regions, self.pending);
        assert_eq!(app.background.submitted_generation(), self.generation);
        assert!(!app.background.running);
        assert!(app.last_executed_regions.is_empty());
        assert!(app.results.is_empty());
        assert!(app.plot_results.is_empty()); // The two LCC inputs have no plots.
        assert!(app.uncommitted_math.is_empty());
        assert!(app.inline_plots.is_empty());
        assert!(app.undo.is_empty() && app.redo.is_empty());
        assert!(!app.dirty);
        assert_eq!(app.selected, self.selected);
        assert_eq!(app.editing_region, self.editing);
        assert!(app.calculation_error.is_none());
    }
    pub(super) fn finish(&self, app: &WorksheetApp) {
        self.check(app);
        assert_eq!(serde_json::to_vec(&app.document).unwrap(), self.wire);
    }
}

struct Clock {
    frame: u64,
    native_ppp: f32,
}
impl Clock {
    fn raw(&mut self, events: Vec<egui::Event>) -> egui::RawInput {
        self.frame += 1;
        let mut raw = egui::RawInput {
            time: Some(self.frame as f64 / 60.),
            events,
            screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1200., 900.))),
            ..Default::default()
        };
        raw.viewports
            .entry(egui::ViewportId::ROOT)
            .or_default()
            .native_pixels_per_point = Some(self.native_ppp);
        raw
    }
}

struct FrameReceipt {
    json: serde_json::Value,
    scroll_id: egui::Id,
    offset: egui::Vec2,
    visible: Vec<u64>,
    scrolling: bool,
}
fn offset_bits(offset: egui::Vec2) -> [u32; 2] {
    [offset.x.to_bits(), offset.y.to_bits()]
}
fn check_phases(frame: &serde_json::Value) {
    let sum: u64 = frame["phases"]
        .as_object()
        .unwrap()
        .values()
        .map(|phase| phase["ns"].as_u64().unwrap())
        .sum();
    assert_eq!(sum, frame["accounted_ns"].as_u64().unwrap());
    assert_eq!(
        sum + frame["gap_ns"].as_u64().unwrap(),
        frame["total_ns"].as_u64().unwrap()
    );
}

fn frame(
    app: &mut WorksheetApp,
    ctx: &egui::Context,
    clock: &mut Clock,
    stage: &str,
    index: usize,
    events: Vec<egui::Event>,
) -> FrameReceipt {
    let expected_zoom = app.zoom;
    WORKSPACE.with_borrow_mut(|slot| *slot = Some(WorkspaceObservation::default()));
    let mut raw = clock.raw(events);
    let detailed = crate::frame_profile::Frame::start();
    let total = Instant::now();
    let start = Instant::now();
    eframe::App::raw_input_hook(app, ctx, &mut raw);
    let raw_hook_ns = start.elapsed().as_nanos();
    let start = Instant::now();
    let mut output = {
        let _scope = crate::frame_profile::scope(crate::frame_profile::Phase::Egui);
        ctx.run_ui(raw, |ui| app.app_ui_frame(ui))
    };
    let ui_ns = start.elapsed().as_nanos();
    let shape_count = output.shapes.len();
    let start = Instant::now();
    let primitives = {
        let _scope = crate::frame_profile::scope(crate::frame_profile::Phase::Tessellation);
        ctx.tessellate(std::mem::take(&mut output.shapes), output.pixels_per_point)
    };
    let tessellation_ns = start.elapsed().as_nanos();
    let (mut meshes, mut vertices, mut indices, mut callbacks) = (0, 0, 0, 0);
    for primitive in &primitives {
        match &primitive.primitive {
            egui::epaint::Primitive::Mesh(mesh) => {
                meshes += 1;
                vertices += mesh.vertices.len();
                indices += mesh.indices.len();
            }
            egui::epaint::Primitive::Callback(_) => callbacks += 1,
        }
    }
    let workspace = WORKSPACE.with_borrow_mut(Option::take);
    let metadata = workspace.as_ref().and_then(|value| value.last);
    let scroll_state =
        metadata.and_then(|metadata| egui::scroll_area::State::load(ctx, metadata.scroll_id));
    let layer_transform =
        metadata.and_then(|metadata| ctx.layer_transform_to_global(metadata.layer));
    let repaint_delay = output
        .viewport_output
        .get(&egui::ViewportId::ROOT)
        .map(|output| output.repaint_delay);
    let viewport_command_count = output
        .viewport_output
        .get(&egui::ViewportId::ROOT)
        .map_or(0, |output| output.commands.len());
    let ppp = output.pixels_per_point;
    let context_zoom = ctx.zoom_factor();
    let scrolling = ctx.input(|input| input.is_scrolling());
    let smooth_delta = ctx.input(|input| input.smooth_scroll_delta());
    let mut visible: Vec<_> = app.visible_regions.iter().copied().collect();
    visible.sort_unstable();
    let texture_sets = output.textures_delta.set.len();
    let texture_set_pixels: usize = output
        .textures_delta
        .set
        .values()
        .flat_map(|deltas| deltas.iter())
        .map(|delta| delta.image.size().into_iter().product::<usize>())
        .sum();
    let drop_start = Instant::now();
    drop(std::hint::black_box(primitives));
    output.drop_without_applying_deltas();
    let drop_ns = drop_start.elapsed().as_nanos();
    // Contiguous raw_input_hook+run_ui+tessellate+mesh/output drop. The inexpensive metadata
    // reads/counts above are included and land in the explicit phase gap.
    let whole_ns = total.elapsed().as_nanos();
    let phases = detailed.finish();
    // All our fallible metadata and contract checks are deliberately after
    // meshes/FullOutput have been explicitly disposed and the phase recorder
    // has closed. Preserve the exact original assertion on the valid path.
    let workspace = workspace.expect("actual workspace metadata observer was installed");
    let metadata = metadata.expect("actual full App body painted workspace");
    let scroll_id = metadata.scroll_id;
    let outer_clip = metadata.clip;
    let state = scroll_state.expect("actual workspace ScrollArea installed state");
    let transform = layer_transform.expect("actual workspace installed paper layer");
    let offset = state.offset;
    let velocity = state.velocity();
    assert_eq!(transform.scaling.to_bits(), expected_zoom.to_bits());
    assert!(transform.translation.x.is_finite() && transform.translation.y.is_finite());
    assert_eq!(ppp.to_bits(), clock.native_ppp.to_bits());
    assert!(workspace.passes > 0);
    let previous_repaint_causes: Vec<_> = ctx
        .repaint_causes()
        .into_iter()
        .map(|cause| cause.to_string())
        .collect();
    let repaint_requested = ctx.has_requested_repaint();
    let repaint_infinite = repaint_delay == Some(std::time::Duration::MAX);
    let repaint_after_ns = repaint_delay
        .filter(|delay| *delay != std::time::Duration::MAX)
        .map(|delay| delay.as_nanos().to_string());
    assert_eq!(context_zoom.to_bits(), 1.0_f32.to_bits());
    if let Some(detail) = &phases {
        check_phases(detail);
    }
    let receipt = serde_json::json!({
        "stage":stage,"index":index,"raw_hook_ns":raw_hook_ns,"ui_ns":ui_ns,
        "body_passes":workspace.passes,"native_pixels_per_point":clock.native_ppp,
        "repaint_requested_post_frame":repaint_requested,"repaint_after_ns":repaint_after_ns,
        "repaint_infinite":repaint_infinite,"previous_repaint_causes":previous_repaint_causes,
        "viewport_command_count":viewport_command_count,"tessellation_ns":tessellation_ns,
        "whole_ns":whole_ns,"drop_ns":drop_ns,"frame_phases":phases,"shape_count":shape_count,
        "mesh_count":meshes,"vertices":vertices,"indices":indices,"callbacks":callbacks,
        "texture_sets":texture_sets,"texture_set_pixels":texture_set_pixels,
        "pixels_per_point":ppp,"context_zoom":context_zoom,"worksheet_zoom":app.zoom,
        "actual_layer_scaling":transform.scaling,
        "actual_layer_translation":[transform.translation.x,transform.translation.y],
        "outer_clip":[outer_clip.min.x,outer_clip.min.y,outer_clip.max.x,outer_clip.max.y],
        "scroll_id":scroll_id.value(),"scroll_offset":[offset.x,offset.y],
        "scroll_velocity":[velocity.x,velocity.y],"input_scrolling":scrolling,
        "remaining_smooth_delta":[smooth_delta.x,smooth_delta.y],"visible_regions":visible,
        "editors":app.editors.len(),"saved_rows":app.saved_results.len(),
        "generation":app.background.submitted_generation(),"executed":app.last_executed_regions.len()
    });
    FrameReceipt {
        json: receipt,
        scroll_id,
        offset,
        visible,
        scrolling,
    }
}

fn exercise(
    app: &mut WorksheetApp,
    ctx: &egui::Context,
    clock: &mut Clock,
    path: &Path,
    name: &str,
    zoom: f32,
    lifecycle: &str,
) -> (serde_json::Value, egui::Id) {
    app.zoom = zoom;
    let detail = crate::frame_profile::Frame::start();
    let start = Instant::now();
    app.open(path);
    let open_ns = start.elapsed().as_nanos();
    let open_phases = detail.finish();
    if let Some(detail) = &open_phases {
        check_phases(detail);
    }
    assert!(app.status.starts_with("Opened"), "{}", app.status);
    let start = Instant::now();
    app.enable_background_calculation(ctx.clone());
    let enable_background_ns = start.elapsed().as_nanos();
    assert!(app.editors.is_empty());
    assert!(app.last_executed_regions.is_empty() && !app.background.running);
    let oracle = ReadOnlyOracle::new(app);
    let first = frame(app, ctx, clock, "first", 0, vec![]);
    assert!(
        first.offset.y.abs() <= f32::EPSILON,
        "fixture must begin at top"
    );
    oracle.check(app);
    let scroll_id = first.scroll_id;
    let mut rows = vec![first.json];
    for index in 0..IDLE_FRAMES {
        let idle = frame(app, ctx, clock, "initial_idle", index, vec![]);
        assert_eq!(offset_bits(idle.offset), [0., 0.].map(f32::to_bits));
        oracle.check(app);
        rows.push(idle.json);
    }
    let before = frame(
        app,
        ctx,
        clock,
        "pointer_before_wheel",
        0,
        vec![egui::Event::PointerMoved(pos2(600., 450.))],
    );
    oracle.check(app);
    let initial_offset = before.offset;
    let initial_visible = before.visible;
    rows.push(before.json);
    let wheel = frame(
        app,
        ctx,
        clock,
        "wheel",
        0,
        vec![egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: vec2(0., -480.),
            phase: egui::TouchPhase::Move,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    oracle.check(app);
    let mut previous_offset = wheel.offset;
    rows.push(wheel.json);
    let mut stable = 0;
    let mut settled = None;
    for index in 0..MAX_SETTLE_FRAMES {
        let sample = frame(app, ctx, clock, "scroll_settle", index, vec![]);
        oracle.check(app);
        if offset_bits(sample.offset) == offset_bits(previous_offset) && !sample.scrolling {
            stable += 1;
        } else {
            stable = 0;
        }
        previous_offset = sample.offset;
        if stable == STABLE_FRAMES {
            settled = Some((sample.offset, sample.visible.clone()));
        }
        rows.push(sample.json);
        if settled.is_some() {
            break;
        }
    }
    let (settled_offset, settled_visible) = settled.expect("wheel must settle in bounded frames");
    assert!(
        settled_offset.y > initial_offset.y + 32.,
        "actual ScrollArea did not move"
    );
    assert_ne!(
        settled_visible, initial_visible,
        "wheel offset must expose a changed worksheet viewport"
    );
    for index in 0..IDLE_FRAMES {
        let idle = frame(app, ctx, clock, "scrolled_idle", index, vec![]);
        assert_eq!(offset_bits(idle.offset), offset_bits(settled_offset));
        assert!(!idle.scrolling);
        oracle.check(app);
        rows.push(idle.json);
    }
    oracle.finish(app);
    (
        serde_json::json!({"name":name,"lifecycle":lifecycle,"zoom":zoom,"open_ns":open_ns,
        "open_phases":open_phases,"enable_background_ns":enable_background_ns,
        "native_pixels_per_point":clock.native_ppp,"source_bytes_unchanged":true,"calculation_executions":0,
        "top_visible_regions":initial_visible,"scrolled_visible_regions":settled_visible,
        "frames":rows}),
        scroll_id,
    )
}

fn source_path(name: &str) -> PathBuf {
    let env = if name == "1015_LCC_S" {
        "MATHCRAFT_FULL_APP_SOURCE_1015"
    } else {
        "MATHCRAFT_FULL_APP_SOURCE_1016"
    };
    std::env::var_os(env).map(PathBuf::from).unwrap_or_else(|| {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../artifacts/rea/unit-selected-sources")
            .join(format!("{name}.mcdx"))
    })
}
fn run(observer: bool) {
    let output = PathBuf::from(
        std::env::var_os("MATHCRAFT_FULL_APP_OUTPUT")
            .expect("set a new absolute JSON receipt filename per process"),
    );
    assert!(output.is_absolute());
    assert!(!output.exists(), "never overwrite evidence");
    std::fs::create_dir_all(output.parent().unwrap()).unwrap();
    let mut cases = vec![];
    for name in ["1015_LCC_S", "1016_LCC_S"] {
        let path = source_path(name);
        assert!(path.is_file(), "actual fixture missing: {}", path.display());
        for native_ppp in [1., 2.] {
            for zoom in [1., 2.] {
                let ctx = egui::Context::default();
                let start = Instant::now();
                crate::configure(&ctx);
                let configure_ns = start.elapsed().as_nanos();
                let mut app = WorksheetApp::default();
                let mut clock = Clock {
                    frame: 0,
                    native_ppp,
                };
                let (mut fresh, scroll_id) = exercise(
                    &mut app,
                    &ctx,
                    &mut clock,
                    &path,
                    name,
                    zoom,
                    "fresh_context",
                );
                egui::scroll_area::State::default().store(&ctx, scroll_id);
                let (mut warm, reused_id) = exercise(
                    &mut app,
                    &ctx,
                    &mut clock,
                    &path,
                    name,
                    zoom,
                    "same_context_reopen",
                );
                assert_eq!(reused_id, scroll_id);
                let bytes = std::fs::read(&path).unwrap();
                let input = serde_json::json!({"path":path,"bytes":bytes.len(),"sha256":format!("{:x}",Sha256::digest(&bytes))});
                fresh["input"] = input.clone();
                warm["input"] = input;
                fresh["configure_ns"] = serde_json::json!(configure_ns);
                warm["configure_ns"] = serde_json::Value::Null;
                cases.extend([fresh, warm]);
            }
        }
    }
    assert_eq!(cases.len(), 16);
    let report = serde_json::json!({"scope":"production raw_input_hook + exact extracted App::ui + tessellate",
        "frame_observer":observer,"workspace_metadata_observer_always":true,
        "gpu_executed":false,"window_or_gui_created":false,"original_runtime_executed":false,
        "source_calculation_executions":0,"initial_idle_count":IDLE_FRAMES,"scrolled_idle_count":IDLE_FRAMES,
        "max_scroll_settle_frames":MAX_SETTLE_FRAMES,"cases":cases});
    std::fs::write(&output, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    println!(
        "FULL_APP cases=16 readonly_calculations=0 observer={observer} output={}",
        output.display()
    );
}

#[test]
#[ignore = "Full actual-App CPU; run frozen binaries in coordinated quiet window"]
fn actual_lcc_full_app_cpu_off() {
    run(false);
}
#[test]
#[ignore = "Full actual-App CPU with existing exclusive phase observer"]
fn actual_lcc_full_app_cpu_on() {
    crate::frame_profile::record(|| run(true));
}

fn temporary() -> PathBuf {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let path = std::env::temp_dir().join(format!(
        "mathcraft-full-app-regression-{}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&path).unwrap();
    path
}
#[test]
fn full_app_frames_keep_saved_values_source_history_and_generation_inert() {
    // No installed target or ignored external input required. Generate small own
    // cache through the public wire, then exercise the exact production body.
    let root = temporary();
    let path = root.join("cached.mathcraft");
    let mut document = CanvasDocument {
        title: "Full-frame readonly".into(),
        ..Default::default()
    };
    let mut rows = vec![];
    for index in 0..64 {
        let id = document.add_math(0., index as f64 * 60.);
        let value = index + 1;
        let math = format!(
            "<ml:eval xmlns:ml='{ML}'><ml:real>{value}</ml:real><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>"
        );
        document
            .regions
            .iter_mut()
            .find(|region| region.id == id)
            .unwrap()
            .content = CanvasContent::Math(worksheet_math::tree_from_mathml(&math).unwrap());
        rows.push(SavedCalculationRow {
            region_id: id,
            status: SavedCalculationStatus::Synchronized,
            display: Some(
                crate::result_display::format(
                    &mathcraft_compat::Value::Scalar {
                        quantity: mathcraft_core::Quantity::scalar(value as f64),
                    },
                    &MathFormat::default(),
                )
                .to_saved(),
            ),
            error: None,
            warnings: vec![],
            symbolic_mathml: None,
            numeric_source: Some(SavedNumericSource::NativeMathml(format!(
                "<ml:result xmlns:ml='{ML}'><ml:real>{value}</ml:real></ml:result>"
            ))),
        });
    }
    document.calculation_cache = Some(mathcraft_document::canvas::SavedCalculationCache {
        source_signature: document.calculation_signature(),
        rows,
    });
    document.save(&path).unwrap();
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    let mut app = WorksheetApp::default();
    app.open(&path);
    app.enable_background_calculation(ctx.clone());
    assert!(app.status.starts_with("Opened"), "{}", app.status);
    assert_eq!(app.saved_results.len(), 64);
    assert!(app.pending_regions.is_empty());
    let oracle = ReadOnlyOracle::new(&app);
    let mut clock = Clock {
        frame: 0,
        native_ppp: 1.,
    };
    for index in 0..4 {
        let sample = frame(
            &mut app,
            &ctx,
            &mut clock,
            "ordinary_readonly",
            index,
            vec![],
        );
        assert_eq!(offset_bits(sample.offset), [0., 0.].map(f32::to_bits));
        oracle.check(&app);
    }
    let _ = frame(
        &mut app,
        &ctx,
        &mut clock,
        "pointer",
        0,
        vec![egui::Event::PointerMoved(pos2(600., 450.))],
    );
    oracle.check(&app);
    let wheel = frame(
        &mut app,
        &ctx,
        &mut clock,
        "wheel",
        0,
        vec![egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: vec2(0., -240.),
            phase: egui::TouchPhase::Move,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    oracle.check(&app);
    let mut moved = wheel.offset.y > 0.;
    for index in 0..8 {
        let sample = frame(&mut app, &ctx, &mut clock, "settle", index, vec![]);
        moved |= sample.offset.y > 0.;
        oracle.check(&app);
    }
    assert!(moved, "actual full App ScrollArea must react to wheel");
    oracle.finish(&app);
    drop(app);
    drop(ctx);
    std::fs::remove_file(&path).unwrap();
    std::fs::remove_dir(&root).unwrap();
}

#[path = "idle_repaint_probe.rs"]
mod idle_repaint_probe;
