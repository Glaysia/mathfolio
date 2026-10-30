//! Diagnostic regression: first callback frame is still required to paint all
//! original golden labels. Extra frames provide telemetry, not a warm-up waiver.
use super::*;

#[derive(Debug)]
struct Text {
    value: String,
    bounds: Rect,
    mesh: Rect,
    clip: Rect,
}
struct Sample {
    observed: Rect,
    queried: Rect,
    text: Vec<Text>,
    markers: Vec<(usize, Option<Rect>)>,
    cached_height: Option<f32>,
    cached_width: Option<f32>,
}
impl Sample {
    fn paint(&self, filter: Option<Rect>) -> Paint {
        Paint {
            texts: self
                .text
                .iter()
                .filter(|t| {
                    t.clip.intersects(t.bounds)
                        && filter.is_none_or(|r| r.expand(1.).contains(t.bounds.center()))
                })
                .map(|t| t.value.clone())
                .collect(),
            rect: filter.unwrap_or(self.observed),
            focus: false,
            ime_cursor: None,
        }
    }
    fn dump(&self, frame: usize) {
        eprintln!(
            "CALLBACK_LAYOUT frame={frame} observed={:?} queried={:?} cached_height={:?} cached_width={:?} markers={:?}",
            self.observed, self.queried, self.cached_height, self.cached_width, self.markers
        );
        for t in &self.text {
            if t.value.contains("200") || t.value.contains("201") {
                eprintln!(
                    "CALLBACK_LABEL frame={frame} text={:?} bounds={:?} mesh={:?} clip={:?} inside_observed={} inside_queried={} clip_bounds={} clip_mesh={}",
                    t.value,
                    t.bounds,
                    t.mesh,
                    t.clip,
                    self.observed.expand(1.).contains(t.bounds.center()),
                    self.queried.expand(1.).contains(t.bounds.center()),
                    t.clip.intersects(t.bounds),
                    t.clip.intersects(t.mesh)
                );
            }
        }
    }
}

fn sample(f: &mut Fixture) -> Sample {
    f.tick += 1;
    let mut raw = egui::RawInput {
        time: Some(f.tick as f64 / 60.),
        screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(2400., 2200.))),
        ..Default::default()
    };
    eframe::App::raw_input_hook(&mut f.app, &f.ctx, &mut raw);
    FORMULA_RESPONSES.with_borrow_mut(HashMap::clear);
    let output = f.ctx.run_ui(raw, |ui| f.app.app_ui_frame(ui));
    let observed = FORMULA_RESPONSES.with_borrow(|responses| responses.get(&f.owner).cloned());
    let queried = formula_id(f).and_then(|id| f.ctx.read_response(id));
    let rect = |response: &egui::Response| {
        f.ctx
            .layer_transform_to_global(response.layer_id)
            .unwrap_or_default()
            * response.rect
    };
    let observed_rect = observed.as_ref().map(rect);
    let queried_rect = queried.as_ref().map(rect);
    let markers = observed.as_ref().map(|r| {
        let nav = r.id.with("matrix-result-navigator");
        (0usize..4)
            .map(|i| {
                (
                    i,
                    f.ctx
                        .read_response(nav.with(("marker", i)))
                        .as_ref()
                        .map(rect),
                )
            })
            .collect::<Vec<_>>()
    });
    fn collect(shape: &egui::Shape, clip: Rect, text: &mut Vec<Text>) {
        match shape {
            egui::Shape::Text(s) => text.push(Text {
                value: s.galley.text().to_owned(),
                bounds: Rect::from_min_size(s.pos, s.galley.size()),
                mesh: s.galley.mesh_bounds.translate(s.pos.to_vec2()),
                clip,
            }),
            egui::Shape::Vec(children) => {
                for child in children {
                    collect(child, clip, text);
                }
            }
            _ => {}
        }
    }
    let mut text = Vec::new();
    for s in &output.shapes {
        collect(&s.shape, s.clip_rect, &mut text);
    }
    output.drop_without_applying_deltas();
    Sample {
        observed: observed_rect.expect("real widget response was not tapped"),
        queried: queried_rect.expect("Context response missing after real widget"),
        text,
        markers: markers.unwrap(),
        cached_height: f.app.region_heights.get(&f.owner).copied(),
        cached_width: f.app.region_widths.get(&f.owner).copied(),
    }
}

#[test]
fn origin_callback_first_frame_shape_clip_and_both_response_rects_agree_on_all_golden_labels() {
    let mut f = known(true);
    labels(&paint(&mut f), 1000);
    let source = serde_json::to_vec(&f.app.document).unwrap();
    let before_values = values(&f.app);
    let generation = f.app.background.submitted_generation();
    deliver(&mut f, Some(2000), false);
    // Do not replace first-frame assertion with a warm-up. Preserve all three
    // frames before asserting so a failure can identify transient-only effects.
    let samples = [sample(&mut f), sample(&mut f), sample(&mut f)];
    for (i, s) in samples.iter().enumerate() {
        s.dump(i);
    }
    let first = &samples[0];
    // 1. All actual output Text shapes whose clip intersects their logical
    // bounds, independent of any response rectangle collector.
    labels(&first.paint(None), 2000);
    // 2. The Response returned synchronously by the actual formula widget.
    labels(&first.paint(Some(first.observed)), 2000);
    // 3. Context::read_response after ctx.run_ui (the existing collector).
    labels(&first.paint(Some(first.queried)), 2000);
    assert!(first.markers.iter().all(|(_, r)| r.is_some()));
    // These marker responses are queried from Context, so they establish
    // widget presence only. Exact current hit geometry is covered by the
    // renderer's synchronous Geometry/actual pointer regressions, not this tap.
    assert_eq!(values(&f.app), before_values);
    assert_eq!(serde_json::to_vec(&f.app.document).unwrap(), source);
    assert_eq!(f.app.background.submitted_generation(), generation);
    assert!(f.app.last_executed_regions.is_empty() && !f.app.background.running);
    assert!(f.app.undo.is_empty() && f.app.redo.is_empty());
}

mod pending_cycle_tests;
