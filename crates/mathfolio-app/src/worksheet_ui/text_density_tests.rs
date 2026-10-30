//! Actual workspace density regression across OS and worksheet scales.
//! Actual workspace paths, no GUI/window, no original runtime.
use super::*;

fn collect(shape: &egui::Shape, out: &mut Vec<Arc<egui::Galley>>) {
    match shape {
        egui::Shape::Text(text) => out.push(text.galley.clone()),
        egui::Shape::Vec(shapes) => {
            for shape in shapes {
                collect(shape, out)
            }
        }
        _ => {}
    }
}

#[test]
fn actual_workspace_owned_plot_and_page_text_have_final_density_without_source_work() {
    for (os, zoom) in [1_f32, 1.5, 2.]
        .into_iter()
        .flat_map(|os| [1_f32, 1.25, 2.].map(|zoom| (os, zoom)))
    {
        let ctx = egui::Context::default();
        crate::configure(&ctx);
        let mut app = WorksheetApp {
            automatic_calculation: false,
            zoom,
            expand_right: true,
            ..Default::default()
        };
        let id = app.document.add_plot(20., 20.);
        let CanvasContent::Plot(spec) = &mut app.document.regions[0].content else {
            panic!()
        };
        spec.source = mathfolio_document::canvas::PlotSource::single("density_x ", "density_y ");
        spec.x_axis.label = "density-title-x".into();
        spec.y_axis.label = "density-title-y".into();
        app.plot_results.insert(
            id,
            Ok(crate::plots::PlotData {
                traces: vec![crate::plots::TraceOutcome::Failed(
                    "density-diagnostic".into(),
                )],
            }),
        );
        let source = serde_json::to_vec(&app.document).unwrap();
        let history = (app.undo.len(), app.redo.len(), app.dirty);
        let generation = app.background.submitted_generation();
        let xml = app.background.preparation.full_dom_parses;
        let mut title_angles = Vec::new();
        let mut galleys = Vec::new();
        let mut transform = None;
        let mut input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(2200., 1500.))),
            ..Default::default()
        };
        input
            .viewports
            .get_mut(&egui::ViewportId::ROOT)
            .unwrap()
            .native_pixels_per_point = Some(os);
        let mut output = ctx.run_ui(input, |ui| {
            app.workspace_ui(ui);
            transform = ctx.layer_transform_to_global(egui::LayerId::new(
                ui.layer_id().order,
                egui::Id::new("worksheet-paper"),
            ));
        });
        for shape in &output.shapes {
            collect(&shape.shape, &mut galleys);
            collect_title_angles(&shape.shape, &mut title_angles);
        }
        // Explicit disposal precedes every assertion, including missing-text
        // assertions. Preserve one failure rather than egui drop double-panic.
        let _ = ctx.tessellate(std::mem::take(&mut output.shapes), output.pixels_per_point);
        output.drop_without_applying_deltas();
        let wanted = [
            "x",
            "y",
            "density_x ",
            "density_y ",
            "trace 1: density-diagnostic",
            "인쇄 영역 밖",
            "density-title-x",
            "density-title-y",
        ];
        let selected = wanted
            .iter()
            .map(|text| galleys.iter().find(|g| g.text() == *text).cloned())
            .collect::<Vec<_>>();
        let mut expected = Vec::new();
        let mut oracle_input = egui::RawInput::default();
        oracle_input
            .viewports
            .get_mut(&egui::ViewportId::ROOT)
            .unwrap()
            .native_pixels_per_point = Some(os);
        let oracle = ctx.run_ui(oracle_input, |_| {
            let final_ppp = ctx.pixels_per_point() * zoom;
            for galley in selected.iter().flatten() {
                let dense = ctx.fonts_mut(|fonts| {
                    egui::epaint::text::layout(fonts.fonts, final_ppp, galley.job.clone())
                });
                expected.push(
                    dense
                        .rows
                        .iter()
                        .flat_map(|row| row.glyphs.iter().map(|glyph| glyph.uv_rect))
                        .collect::<Vec<_>>(),
                );
            }
        });
        oracle.drop_without_applying_deltas();
        assert_eq!(ctx.pixels_per_point(), os);
        assert_eq!(transform.unwrap().scaling, zoom);
        assert!(
            title_angles
                .iter()
                .any(|(name, angle)| name == "density-title-y"
                    && (*angle + std::f32::consts::FRAC_PI_2).abs() < 1e-6)
        );
        assert!(
            title_angles
                .iter()
                .any(|(name, angle)| name == "density-title-x" && *angle == 0.)
        );
        assert!(
            selected.iter().all(Option::is_some),
            "missing production text: {:?}",
            wanted
                .iter()
                .zip(&selected)
                .filter_map(|(s, g)| g.is_none().then_some(*s))
                .collect::<Vec<_>>()
        );
        for (galley, expected) in selected.iter().flatten().zip(expected) {
            let actual = galley
                .rows
                .iter()
                .flat_map(|row| row.glyphs.iter().map(|glyph| glyph.uv_rect))
                .collect::<Vec<_>>();
            assert_eq!(
                actual,
                expected,
                "os={os} zoom={zoom} text={}",
                galley.text()
            );
        }
        assert_eq!(serde_json::to_vec(&app.document).unwrap(), source);
        assert_eq!((app.undo.len(), app.redo.len(), app.dirty), history);
        assert_eq!(app.background.submitted_generation(), generation);
        assert_eq!(app.background.preparation.full_dom_parses, xml);
        assert!(app.last_executed_regions.is_empty());
    }
}

#[test]
fn actual_legacy_field_galley_keeps_trailing_spaces_and_singleline_job_policy() {
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    let mut app = WorksheetApp {
        automatic_calculation: false,
        zoom: 2.,
        ..Default::default()
    };
    let id = app.document.add_plot(20., 20.);
    let CanvasContent::Plot(spec) = &mut app.document.regions[0].content else {
        panic!()
    };
    spec.source = mathfolio_document::canvas::PlotSource::single("legacy_x  ", "legacy_y  ");
    app.plot_results
        .insert(id, Err("not evaluated in drawing".into()));
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1600., 1200.))),
            ..Default::default()
        },
        |ui| app.workspace_ui(ui),
    );
    let mut galleys = Vec::new();
    for shape in &output.shapes {
        collect(&shape.shape, &mut galleys);
    }
    let _ = ctx.tessellate(std::mem::take(&mut output.shapes), output.pixels_per_point);
    output.drop_without_applying_deltas();
    for text in ["legacy_x  ", "legacy_y  "] {
        let galley = galleys.iter().find(|g| g.text() == text).unwrap();
        assert!(galley.job.keep_trailing_whitespace);
        assert!(
            galley
                .job
                .sections
                .iter()
                .all(|s| s.format.line_height.is_some())
        );
        assert_eq!(galley.rows.len(), 1);
    }
    assert!(app.undo.is_empty() && app.redo.is_empty());
    assert!(app.last_executed_regions.is_empty());
}

fn collect_title_angles(shape: &egui::Shape, out: &mut Vec<(String, f32)>) {
    match shape {
        egui::Shape::Text(text) if text.galley.text().starts_with("density-title-") => {
            out.push((text.galley.text().to_owned(), text.angle))
        }
        egui::Shape::Vec(shapes) => {
            for shape in shapes {
                collect_title_angles(shape, out)
            }
        }
        _ => {}
    }
}
