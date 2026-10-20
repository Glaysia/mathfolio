//! Explicit CPU-only workload for an actual saved worksheet. This test does
//! not start eframe, a native window, an evaluator worker, or a screenshot tool.
use super::*;

fn frame(app: &mut WorksheetApp, ctx: &egui::Context, os_dpi: f32) {
    let mut input = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1100., 900.))),
        ..Default::default()
    };
    input
        .viewports
        .get_mut(&egui::ViewportId::ROOT)
        .unwrap()
        .native_pixels_per_point = Some(os_dpi);
    let output = ctx.run_ui(input, |ui| {
        app.chrome_ui(ui);
        egui::CentralPanel::default().show(ui, |ui| app.workspace_ui(ui));
    });
    output.drop_without_applying_deltas();
}

#[test]
#[ignore = "CPU profile requires a caller-provided actual MCDX fixture"]
fn benchmark_native_saved_worksheet_screen() {
    let source = std::env::var_os("MATHCRAFT_SCREEN_PERF_SOURCE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../artifacts/rea/detune-preserved-v1.mcdx")
        });
    assert!(
        source.is_file(),
        "set MATHCRAFT_SCREEN_PERF_SOURCE to an actual MCDX worksheet"
    );
    for os_dpi in [1., 2.] {
        let ctx = egui::Context::default();
        crate::configure(&ctx);
        let mut app = WorksheetApp::default();
        app.open(&source);
        assert!(!app.document.regions.is_empty());
        assert!(app.last_executed_regions.is_empty());
        for zoom in [1., 2.] {
            app.zoom = zoom;
            let cold = std::time::Instant::now();
            frame(&mut app, &ctx, os_dpi);
            let cold_us = cold.elapsed().as_micros();
            for _ in 0..6 {
                frame(&mut app, &ctx, os_dpi);
            }
            let start = std::time::Instant::now();
            for _ in 0..120 {
                frame(&mut app, &ctx, os_dpi);
            }
            println!(
                "SCREEN_NATIVE source={} regions={} saved={} native_ppp={os_dpi} zoom={zoom} cold_us={cold_us} frames=120 elapsed_us={} executed={}",
                source.file_name().unwrap().to_string_lossy(),
                app.document.regions.len(),
                app.saved_results.len(),
                start.elapsed().as_micros(),
                app.last_executed_regions.len()
            );
            assert!(app.last_executed_regions.is_empty());
            assert!(!app.dirty);
        }
    }
}
