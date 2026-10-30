//! Headless test-only workspace CPU diagnosis; no native window or renderer.
//! Calls the actual workspace_ui, including its ScrollArea/zoom/layer path.
use super::*;
use sha2::{Digest, Sha256};
use std::{path::Path, time::Instant};

const IDLE_FRAMES: usize = 10;
const MAX_SETTLE_FRAMES: usize = 48;
const STABLE_FRAMES: usize = 3;

struct ReadOnlyOracle {
    document: CanvasDocument,
    wire: Vec<u8>,
    saved: HashMap<u64, SavedCalculationRow>,
    pending: HashSet<u64>,
    generation: u64,
    selected: Option<u64>,
    editing: Option<u64>,
}
impl ReadOnlyOracle {
    fn new(app: &WorksheetApp) -> Self {
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
    fn check(&self, app: &WorksheetApp) {
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
    fn finish(&self, app: &WorksheetApp) {
        self.check(app);
        assert_eq!(serde_json::to_vec(&app.document).unwrap(), self.wire);
    }
}

struct Clock {
    frame: u64,
}
impl Clock {
    fn raw(&mut self, events: Vec<egui::Event>) -> egui::RawInput {
        self.frame += 1;
        egui::RawInput {
            time: Some(self.frame as f64 / 60.),
            events,
            screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1200., 900.))),
            ..Default::default()
        }
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
    let mut metadata = None;
    let raw = clock.raw(events);
    let detailed = crate::frame_profile::Frame::start();
    let total = Instant::now();
    let start = Instant::now();
    let mut output = {
        let _scope = crate::frame_profile::scope(crate::frame_profile::Phase::Egui);
        ctx.run_ui(raw, |ui| {
            // ScrollArea stores an IdSalt (already hashed) and hashes that
            // value with the UI id in begin; a direct string gives another id.
            let scroll_id = ui.make_persistent_id(egui::IdSalt::new("free-worksheet"));
            let layer = egui::LayerId::new(ui.layer_id().order, egui::Id::new("worksheet-paper"));
            app.workspace_ui(ui);
            // Capture fallible test metadata only. An assertion inside this
            // callback or while FullOutput is live can trigger a second egui
            // drop invariant panic and obscure the original contract failure.
            let state = egui::scroll_area::State::load(ctx, scroll_id);
            let transform = ctx.layer_transform_to_global(layer);
            metadata = Some((scroll_id, state, transform, ui.clip_rect()));
        })
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
    // Contiguous run_ui+tessellate+mesh/output drop. The inexpensive metadata
    // reads/counts above are included and land in the explicit phase gap.
    let whole_ns = total.elapsed().as_nanos();
    let phases = detailed.finish();
    // All our fallible metadata and contract checks are deliberately after
    // meshes/FullOutput have been explicitly disposed and the phase recorder
    // has closed. Preserve the exact original assertion on the valid path.
    let (scroll_id, state, transform, outer_clip) =
        metadata.expect("actual workspace UI callback completed");
    let state = state.expect("actual workspace ScrollArea installed its state");
    let transform = transform.expect("actual workspace installed its paper layer transform");
    let offset = state.offset;
    let velocity = state.velocity();
    assert_eq!(transform.scaling.to_bits(), expected_zoom.to_bits());
    assert!(transform.translation.x.is_finite() && transform.translation.y.is_finite());
    assert!(ppp.is_finite() && ppp > 0.);
    assert_eq!(context_zoom.to_bits(), 1.0_f32.to_bits());
    if let Some(detail) = &phases {
        check_phases(detail);
    }
    let receipt = serde_json::json!({
        "stage":stage,"index":index,"ui_ns":ui_ns,"tessellation_ns":tessellation_ns,
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
        "open_phases":open_phases,"source_bytes_unchanged":true,"calculation_executions":0,
        "top_visible_regions":initial_visible,"scrolled_visible_regions":settled_visible,
        "frames":rows}),
        scroll_id,
    )
}

fn run(observer: bool) {
    let output = PathBuf::from(
        std::env::var_os("MATHFOLIO_LCC_WORKSPACE_OUTPUT")
            .expect("set a new receipt JSON path per process"),
    );
    assert!(!output.exists(), "never overwrite evidence");
    std::fs::create_dir_all(output.parent().unwrap()).unwrap();
    let mut cases = vec![];
    for name in ["1015_LCC_S", "1016_LCC_S"] {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../artifacts/rea/unit-selected-sources")
            .join(format!("{name}.mcdx"));
        assert!(path.is_file(), "actual original fixture must exist");
        for zoom in [1., 1.5, 2.] {
            let context = egui::Context::default();
            crate::configure(&context); // outside open/frame timers; fonts lazy per Context
            let mut app = WorksheetApp::default();
            app.enable_background_calculation(context.clone());
            let mut clock = Clock { frame: 0 };
            let (mut fresh, scroll_id) = exercise(
                &mut app,
                &context,
                &mut clock,
                &path,
                name,
                zoom,
                "fresh_context",
            );
            // Retain the same Context/font atlas, explicitly reset only the
            // viewport fixture before reopening the same source at top.
            egui::scroll_area::State::default().store(&context, scroll_id);
            let (mut warm, reused_id) = exercise(
                &mut app,
                &context,
                &mut clock,
                &path,
                name,
                zoom,
                "same_context_reopen",
            );
            assert_eq!(reused_id, scroll_id);
            // Hash the actual compile-time-fixture path after timed opens,
            // rather than pre-reading it and claiming a cold disk read.
            let bytes = std::fs::read(&path).unwrap();
            let input = serde_json::json!({"path":path,"bytes":bytes.len(),
                "sha256":format!("{:x}",Sha256::digest(&bytes))});
            fresh["input"] = input.clone();
            warm["input"] = input;
            cases.extend([fresh, warm]);
        }
    }
    assert_eq!(cases.len(), 12);
    let report = serde_json::json!({"frame_observer":observer,"tessellation_always_enabled":true,
        "gpu_executed":false,"window_or_gui_created":false,"source_calculation_executions":0,
        "initial_idle_count":IDLE_FRAMES,"scrolled_idle_count":IDLE_FRAMES,
        "max_scroll_settle_frames":MAX_SETTLE_FRAMES,"cases":cases});
    std::fs::write(&output, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    println!(
        "LCC_WORKSPACE cases=12 readonly_calculations=0 observer={observer} output={}",
        output.display()
    );
}

#[test]
#[ignore = "Actual workspace lifecycle CPU; coordinate quiet window"]
fn actual_lcc_workspace_cpu_off() {
    run(false);
}
#[test]
#[ignore = "Actual workspace lifecycle CPU and exclusive phase collector"]
fn actual_lcc_workspace_cpu_on() {
    crate::frame_profile::record(|| run(true));
}
