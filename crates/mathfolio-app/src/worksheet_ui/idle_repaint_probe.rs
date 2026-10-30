//! Artifact-only CPU child of full_app_profile; no renderer/device/window.
use super::*;
use std::{
    collections::BTreeMap,
    io::{Read, Write},
};

const INPUT_SHA: &str = "217994a631de58e096a3cfdb8a8251aca91501e62151f869950b99f738fc18b5";
const WARM: usize = 2;
const INITIAL_IDLE: usize = 10;
const EXTENDED_IDLE: usize = 60;

#[derive(Default)]
struct GeometrySnapshot {
    widths: BTreeMap<u64, u32>,
    heights: BTreeMap<u64, u32>,
}
impl GeometrySnapshot {
    fn capture(app: &WorksheetApp) -> Self {
        fn map(values: &HashMap<u64, f32>) -> BTreeMap<u64, u32> {
            assert!(values.len() <= 512, "Fixed-scene geometry cap");
            values
                .iter()
                .map(|(&id, &value)| {
                    assert!(value.is_finite() && value >= 0., "Invalid cached geometry");
                    (id, value.to_bits())
                })
                .collect()
        }
        Self {
            widths: map(&app.region_widths),
            heights: map(&app.region_heights),
        }
    }
}
fn changes(before: &BTreeMap<u64, u32>, after: &BTreeMap<u64, u32>) -> Vec<serde_json::Value> {
    let keys: std::collections::BTreeSet<_> = before.keys().chain(after.keys()).copied().collect();
    keys.into_iter().filter_map(|id| {
        let old = before.get(&id).copied();
        let new = after.get(&id).copied();
        if old == new { return None; }
        let old_value = old.map(f32::from_bits);
        let new_value = new.map(f32::from_bits);
        // Corresponds only to the endpoint condition in worksheet_ui. It does
        // not observe an intermediate write which changed back in a later pass.
        let exceeds_half_pixel = match (old_value, new_value) {
            (None, Some(_)) => Some(true),
            (Some(a), Some(b)) => Some((a - b).abs() > 0.5),
            _ => None,
        };
        Some(serde_json::json!({"id":id,"old_bits":old,"new_bits":new,
            "old":old_value,"new":new_value,"endpoint_would_trigger_half_pixel":exceeds_half_pixel}))
    }).collect()
}

fn bounded_scene(path: &Path) -> (Vec<u8>, std::fs::File) {
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(1); // FILE_SHARE_READ; hold across App::open.
    }
    let mut file = options.open(path).expect("Open bounded source");
    assert!(file.metadata().unwrap().len() <= 16 * 1024 * 1024);
    let mut bytes = Vec::new();
    (&mut file)
        .take(16 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .unwrap();
    assert!(bytes.len() <= 16 * 1024 * 1024);
    assert_eq!(format!("{:x}", Sha256::digest(&bytes)), INPUT_SHA);
    (bytes, file)
}

#[test]
#[ignore = "Optional CPU-only idle repaint/source-geometry diagnostic; frozen harness and outer timeout"]
fn actual_lcc_idle_repaint_causes_cpu() {
    assert_eq!(
        std::env::var("MATHFOLIO_IDLE_REPAINT_GUARD").as_deref(),
        Ok("bounded-cpu-v1")
    );
    let output = PathBuf::from(
        std::env::var_os("MATHFOLIO_IDLE_REPAINT_OUTPUT").expect("New absolute JSON path"),
    );
    assert!(output.is_absolute() && !output.exists());
    let path = std::env::var_os("MATHFOLIO_IDLE_REPAINT_SCENE")
        .map(PathBuf::from)
        .unwrap_or_else(|| source_path("1015_LCC_S"));
    let (bytes, read_guard) = bounded_scene(&path);
    let mut app = WorksheetApp {
        automatic_calculation: false,
        zoom: 1.,
        ..Default::default()
    };
    app.open(&path);
    assert!(app.status.starts_with("Opened"), "{}", app.status);
    assert_eq!(
        app.document.native_archive.as_deref(),
        Some(bytes.as_slice())
    );
    drop(read_guard);
    assert!(
        app.document
            .regions
            .iter()
            .any(|r| matches!(&r.content, CanvasContent::Math(_)))
    );
    let oracle = ReadOnlyOracle::new(&app);
    let dom = app.background.preparation.full_dom_parse_count();
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    let mut clock = Clock {
        frame: 0,
        native_ppp: 1.,
    };
    let mut geometry = GeometrySnapshot::capture(&app);
    let mut rows = Vec::new();
    for index in 0..WARM + INITIAL_IDLE + EXTENDED_IDLE {
        let stage = if index < WARM {
            "warm"
        } else if index < WARM + INITIAL_IDLE {
            "initial_idle"
        } else {
            "extended_idle"
        };
        let mut sample = frame(&mut app, &ctx, &mut clock, stage, index, vec![]);
        // FullOutput/meshes have been explicitly disposed inside frame() before
        // these assertions and metadata allocations. No GPU path exists here.
        oracle.check(&app);
        assert_eq!(app.zoom.to_bits(), 1f32.to_bits());
        assert_eq!(app.background.preparation.full_dom_parse_count(), dom);
        let visible_math = app
            .document
            .regions
            .iter()
            .filter(|region| {
                app.visible_regions.contains(&region.id)
                    && matches!(&region.content, CanvasContent::Math(_))
            })
            .count();
        assert!(visible_math > 0);
        for key in ["mesh_count", "vertices", "indices"] {
            assert!(
                sample.json[key].as_u64().is_some_and(|n| n > 0),
                "Empty actual workload"
            );
        }
        let after = GeometrySnapshot::capture(&app);
        sample.json["geometry_endpoints"] = serde_json::json!({
            "width_changes":changes(&geometry.widths, &after.widths),
            "height_changes":changes(&geometry.heights, &after.heights),
            "width_entries":after.widths.len(),"height_entries":after.heights.len()});
        sample.json["visible_math_regions"] = visible_math.into();
        sample.json["source_invariants"] = true.into();
        geometry = after;
        rows.push(sample.json);
    }
    oracle.finish(&app);
    let immediate = rows
        .iter()
        .filter(|row| row["repaint_after_ns"].as_str() == Some("0"))
        .count();
    let mut causes = BTreeMap::<String, usize>::new();
    for row in &rows {
        for cause in row["previous_repaint_causes"].as_array().unwrap() {
            *causes
                .entry(cause.as_str().unwrap().to_owned())
                .or_default() += 1;
        }
    }
    let report = serde_json::json!({"status":"complete","frames":rows,
        "warm_frames":WARM,"initial_idle_frames":INITIAL_IDLE,"extended_idle_frames":EXTENDED_IDLE,
        "input":{"path":path,"bytes":bytes.len(),"sha256":INPUT_SHA},
        "source":{"probe_sha256":format!("{:x}",Sha256::digest(include_bytes!("idle_repaint_probe.rs"))),
            "frame_helper_sha256":format!("{:x}",Sha256::digest(include_bytes!("full_app_profile.rs"))),
            "app_sha256":format!("{:x}",Sha256::digest(include_bytes!("../worksheet_ui.rs")))},
        "immediate_repaint_frames":immediate,"previous_pass_cause_occurrences":causes,
        "geometry_final":{"width_bits":geometry.widths,"height_bits":geometry.heights},
        "full_dom_initial":dom,"full_dom_final":app.background.preparation.full_dom_parse_count(),
        "source_wire_unchanged":true,"source_calculation_executions":0,
        "calculation_zero_basis":"existing App source/result/generation/history dispatch oracle",
        "no_input":true,"gpu_executed":false,"gui":false,"surface":false,"readback":false,
        "original_runtime":false,"causes_are_previous_pass":true,"metadata_outside_frame_timing":true,
        "endpoint_geometry_not_all_intrapass_writes":true,"finite_mesh_certification":false,
        "not_a_timing_acceptance_or_gui_bottleneck_claim":true});
    let encoded = serde_json::to_vec_pretty(&report).unwrap();
    assert!(encoded.len() <= 4 * 1024 * 1024, "Bounded receipt");
    let mut destination = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&output)
        .unwrap();
    destination.write_all(&encoded).unwrap();
    destination.sync_all().unwrap();
    println!(
        "IDLE_REPAINT_CPU frames=72 warm=2 initial_idle=10 extended_idle=60 source_calculations=0 gpu=false gui=false output={}",
        output.display()
    );
}
