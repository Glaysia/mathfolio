//! Proposed headless tests; author did not execute Cargo or app/GPU runtime.
use super::*;
use crate::result_display::DisplayMath;

fn projection() -> MatrixProjection {
    MatrixProjection {
        full_rows: 20,
        full_cols: 20,
        row_offset: 2,
        col_offset: 3,
        rows: 12,
        cols: 12,
        cells: (0..144)
            .map(|i| DisplayMath::Text(format!("cell-{i}"), MathStyle::Operator))
            .collect(),
        hidden_above: true,
        hidden_below: true,
        hidden_left: true,
        hidden_right: true,
    }
}
fn editor() -> Editor {
    Editor::from_portable(&PortableTree {
        kind: NodeKind::Binary(BinaryOp::Evaluate),
        children: vec![
            PortableTree {
                kind: NodeKind::Number("1".into()),
                children: vec![],
            },
            PortableTree {
                kind: NodeKind::Placeholder,
                children: vec![],
            },
        ],
    })
    .unwrap()
}
fn context() -> egui::Context {
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    ctx
}
fn texts(layout: &FormulaBox) -> Vec<(String, bool, Rect)> {
    layout
        .primitives
        .iter()
        .filter_map(|p| match p {
            Primitive::Text(at, g) | Primitive::FocusedText(at, g) => Some((
                g.text().to_owned(),
                matches!(p, Primitive::FocusedText(..)),
                Rect::from_min_size(Pos2::ZERO + *at, g.size()),
            )),
            _ => None,
        })
        .collect()
}
fn painted(shapes: &[egui::epaint::ClippedShape]) -> Vec<String> {
    fn visit(shape: &egui::Shape, text: &mut Vec<String>) {
        match shape {
            egui::Shape::Text(s) => text.push(s.galley.text().to_owned()),
            egui::Shape::Vec(children) => {
                for child in children {
                    visit(child, text);
                }
            }
            _ => {}
        }
    }
    let mut text = vec![];
    for shape in shapes {
        visit(&shape.shape, &mut text);
    }
    text
}

#[test]
fn all_144_data_cells_survive_and_signed_visible_and_endpoint_labels_are_separate() {
    let ctx = context();
    let e = editor();
    let p = projection();
    let before = p.clone();
    let mut captured = None;
    ctx.run_ui(egui::RawInput::default(), |ui| {
        let labels = MatrixIndexLabels::for_projection(&p, -3, true).unwrap();
        let layout = measure_indexed(ui.painter(), e.root(), &p, labels, 16.);
        captured = Some((texts(&layout), layout.size, layout.matrix_viewport));
    })
    .drop_without_applying_deltas();
    let (text, size, geometry) = captured.unwrap();
    assert!(size.is_finite());
    for i in 0..144 {
        assert_eq!(
            text.iter()
                .filter(|(s, _, _)| s == &format!("cell-{i}"))
                .count(),
            1
        );
    }
    assert_eq!(
        text.iter()
            .filter(|(s, _, _)| s.starts_with("cell-"))
            .count(),
        144
    );
    let labels: Vec<_> = text
        .iter()
        .filter(|(s, _, _)| !s.starts_with("cell-") && s != "…" && s != "⋮")
        .collect();
    assert_eq!(labels.len(), 28); // 12 visible + two hidden endpoints per axis.
    assert!(labels.iter().any(|(s, _, _)| s == "-1")); // origin -3 + row offset 2.
    assert!(labels.iter().any(|(s, _, _)| s == "11")); // origin -3 + col offset 3 + 11.
    assert_eq!(labels.iter().filter(|(s, _, _)| s == "-3\n⋮").count(), 2);
    assert_eq!(labels.iter().filter(|(s, _, _)| s == " ⋮\n16").count(), 2);
    assert!(labels.iter().all(|(_, focused, _)| !focused));
    assert_eq!(geometry.unwrap().size, [12, 12]);
    assert_eq!(p, before);
}

#[test]
fn fully_visible_axis_has_no_indices_even_when_requested() {
    let ctx = context();
    let e = editor();
    let mut p = projection();
    p.full_rows = 12;
    p.row_offset = 0;
    p.hidden_above = false;
    p.hidden_below = false;
    let mut captured = None;
    ctx.run_ui(egui::RawInput::default(), |ui| {
        let labels = MatrixIndexLabels::for_projection(&p, 71, true).unwrap();
        let base = measure(ui.painter(), e.root(), &p, 16.);
        let layout = measure_indexed(ui.painter(), e.root(), &p, labels, 16.);
        captured = Some((
            texts(&layout),
            base.size,
            layout.size,
            layout.matrix_viewport,
        ));
    })
    .drop_without_applying_deltas();
    let (text, old, new, g) = captured.unwrap();
    assert_eq!(
        text.iter()
            .filter(|(s, _, _)| !s.starts_with("cell-") && s != "…")
            .count(),
        14
    );
    assert!(new.x >= old.x); // Column slot growth is independent of row gutter.
    assert!(new.y > old.y);
    assert_eq!(g.unwrap().bracket.left(), 1.);
}

#[test]
fn heading_admission_rejects_overflow_and_budget_instead_of_wrapping_or_truncating() {
    let p = projection();
    assert!(MatrixIndexLabels::for_projection(&p, i32::MAX - 18, true).is_err());
    assert!(MatrixIndexLabels::for_projection(&p, i32::MIN, true).is_ok());
    let mut large = p.clone();
    large.rows = 4096;
    large.cols = 1;
    large.full_rows = 5000;
    large.full_cols = 1;
    large.row_offset = 1;
    large.col_offset = 0;
    large.hidden_left = false;
    large.hidden_right = false;
    large.cells = (0..4096)
        .map(|_| DisplayMath::Text("x".into(), MathStyle::Operator))
        .collect();
    assert!(
        MatrixIndexLabels::for_projection(&large, 0, false)
            .unwrap_err()
            .contains("budget")
    );
    let mut malformed = p.clone();
    malformed.cells.pop();
    assert!(MatrixIndexLabels::for_projection(&malformed, 0, true).is_err());
    assert_eq!(p, projection());
}

struct Frame {
    text: Vec<String>,
    layout: Arc<FormulaBox>,
    owner: egui::Id,
    geometry: Geometry,
    gesture: Option<MatrixViewGesture>,
    changed: bool,
    evaluated: bool,
}
fn frame(
    ctx: &egui::Context,
    e: &mut Editor,
    p: &MatrixProjection,
    labels: Option<MatrixIndexLabels>,
    focus: bool,
    events: Vec<egui::Event>,
) -> Frame {
    let mut captured = None;
    let output = ctx.run_ui(
        egui::RawInput {
            events,
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(2000., 1600.))),
            ..Default::default()
        },
        |ui| {
            let result = Some(match labels {
                Some(indices) => FormulaResult::MatrixViewportWithIndices {
                    projection: p,
                    indices,
                },
                None => FormulaResult::MatrixViewport(p),
            });
            let drawn = formula_ui_with_actions(ui, e, result, focus);
            let layout = retained_layout(ui, e, None, 11. * 96. / 72., result);
            let geometry = find(&layout, drawn.response.rect.min + vec2(PADDING, PADDING)).unwrap();
            captured = Some((
                layout,
                drawn.response.id,
                geometry,
                drawn.matrix_view_gesture,
                drawn.response.changed(),
                drawn.evaluation_requested,
            ));
        },
    );
    let text = painted(&output.shapes);
    output.drop_without_applying_deltas();
    let (layout, owner, geometry, gesture, changed, evaluated) = captured.unwrap();
    Frame {
        text,
        layout,
        owner,
        geometry,
        gesture,
        changed,
        evaluated,
    }
}
fn button(pos: Pos2, pressed: bool) -> Vec<egui::Event> {
    vec![
        egui::Event::PointerMoved(pos),
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        },
    ]
}

#[test]
fn actual_focus_paints_headings_without_layout_rebuild_or_source_mutation() {
    let ctx = context();
    let mut e = editor();
    let p = projection();
    let source = e.to_portable().unwrap();
    let caret = e.caret();
    let revision = e.arena().revision();
    let labels = Some(MatrixIndexLabels::for_projection(&p, 1000, false).unwrap());
    let a = frame(&ctx, &mut e, &p, labels, false, vec![]);
    assert!(!a.text.iter().any(|s| s == "1002"));
    let b = frame(&ctx, &mut e, &p, labels, true, vec![]);
    assert!(b.text.iter().any(|s| s == "1002"));
    ctx.memory_mut(|m| m.surrender_focus(b.owner));
    let c = frame(&ctx, &mut e, &p, labels, false, vec![]);
    assert!(!c.text.iter().any(|s| s == "1002"));
    assert!(Arc::ptr_eq(&a.layout, &b.layout) && Arc::ptr_eq(&b.layout, &c.layout));
    assert_eq!(a.geometry.bracket, c.geometry.bracket);
    assert_eq!(e.to_portable().unwrap(), source);
    assert_eq!(e.caret(), caret);
    assert_eq!(e.arena().revision(), revision);
    assert!(!e.undo());
    assert!(!a.changed && !b.changed && !c.changed && !a.evaluated && !b.evaluated && !c.evaluated);
}

#[test]
fn show_indices_persists_without_focus_while_cold_variant_never_labels() {
    let ctx = context();
    let mut e = editor();
    let p = projection();
    let cold = frame(&ctx, &mut e, &p, None, true, vec![]);
    assert!(!cold.text.iter().any(|s| s == "1002"));
    ctx.memory_mut(|m| m.surrender_focus(cold.owner));
    let labels = Some(MatrixIndexLabels::for_projection(&p, 1000, true).unwrap());
    let live = frame(&ctx, &mut e, &p, labels, false, vec![]);
    assert!(live.text.iter().any(|s| s == "1002"));
    assert!(!Arc::ptr_eq(&cold.layout, &live.layout));
    assert_eq!(
        live.text.iter().filter(|s| s.starts_with("cell-")).count(),
        144
    );
}

#[test]
fn cache_key_includes_origin_and_persisted_show_flag_but_not_focus() {
    let ctx = context();
    let mut e = editor();
    let p = projection();
    let make = |origin, show| Some(MatrixIndexLabels::for_projection(&p, origin, show).unwrap());
    let a = frame(&ctx, &mut e, &p, make(0, false), false, vec![]);
    let b = frame(&ctx, &mut e, &p, make(1, false), false, vec![]);
    let c = frame(&ctx, &mut e, &p, make(1, true), false, vec![]);
    let d = frame(&ctx, &mut e, &p, make(1, true), true, vec![]);
    assert!(!Arc::ptr_eq(&a.layout, &b.layout));
    assert!(!Arc::ptr_eq(&b.layout, &c.layout));
    assert!(Arc::ptr_eq(&c.layout, &d.layout));
}

#[test]
fn gutters_keep_all_heading_galleys_inside_and_marker_rects_at_drawn_glyphs() {
    let ctx = context();
    let e = editor();
    let p = projection();
    let mut captured = None;
    ctx.run_ui(egui::RawInput::default(), |ui| {
        let labels = MatrixIndexLabels::for_projection(&p, -1234567890, true).unwrap();
        let layout = measure_indexed(ui.painter(), e.root(), &p, labels, 16.);
        captured = Some((texts(&layout), layout.size, layout.matrix_viewport.unwrap()));
    })
    .drop_without_applying_deltas();
    let (text, size, g) = captured.unwrap();
    let outer = Rect::from_min_size(Pos2::ZERO, size).expand(0.02);
    assert!(text.iter().all(|(_, _, rect)| outer.contains_rect(*rect)));
    assert!(g.bracket.left() > 1. && g.bracket.top() > 0.);
    for rect in g.markers.into_iter().flatten() {
        assert!(
            text.iter()
                .any(|(s, _, drawn)| (s == "…" || s == "⋮") && *drawn == rect)
        );
        assert!(g.bracket.contains_rect(rect));
    }
    let shifted = g.translated(vec2(100., 200.));
    assert_eq!(shifted.bracket, g.bracket.translate(vec2(100., 200.)));
    for (before, after) in g.markers.into_iter().zip(shifted.markers) {
        assert_eq!(after, before.map(|r| r.translate(vec2(100., 200.))));
    }
}

#[test]
fn indexed_bracket_retains_actual_begin_preview_commit_without_editing_source() {
    let ctx = context();
    let mut e = editor();
    let p = projection();
    let source = e.to_portable().unwrap();
    let labels = Some(MatrixIndexLabels::for_projection(&p, 1, false).unwrap());
    let a = frame(&ctx, &mut e, &p, labels, false, vec![]);
    let at = a.geometry.bracket.right_bottom();
    frame(
        &ctx,
        &mut e,
        &p,
        labels,
        false,
        vec![egui::Event::PointerMoved(at)],
    );
    let begin = frame(&ctx, &mut e, &p, labels, false, button(at, true));
    let to = at + vec2(a.geometry.step.x, a.geometry.step.y);
    let preview = frame(
        &ctx,
        &mut e,
        &p,
        labels,
        false,
        vec![egui::Event::PointerMoved(to)],
    );
    let commit = frame(&ctx, &mut e, &p, labels, false, button(to, false));
    assert_eq!(
        begin.gesture,
        Some(MatrixViewGesture::Begin {
            size: [12, 12],
            offset: [2, 3]
        })
    );
    assert_eq!(
        preview.gesture,
        Some(MatrixViewGesture::Preview {
            size: [13, 13],
            offset: [2, 3]
        })
    );
    assert_eq!(
        commit.gesture,
        Some(MatrixViewGesture::Commit {
            size: [13, 13],
            offset: [2, 3]
        })
    );
    assert_eq!(e.to_portable().unwrap(), source);
    assert!(!begin.changed && !preview.changed && !commit.changed);
}

#[test]
fn translated_marker_still_opens_real_navigator_popup() {
    let ctx = context();
    let mut e = editor();
    let p = projection();
    let source = e.to_portable().unwrap();
    let labels = Some(MatrixIndexLabels::for_projection(&p, 1, true).unwrap());
    let a = frame(&ctx, &mut e, &p, labels, false, vec![]);
    let marker = a.geometry.markers[0].unwrap().center();
    frame(
        &ctx,
        &mut e,
        &p,
        labels,
        false,
        vec![egui::Event::PointerMoved(marker)],
    );
    let press = frame(&ctx, &mut e, &p, labels, false, button(marker, true));
    let release = frame(&ctx, &mut e, &p, labels, false, button(marker, false));
    frame(&ctx, &mut e, &p, labels, false, vec![]);
    frame(&ctx, &mut e, &p, labels, false, vec![]);
    let id = a.owner.with("matrix-result-navigator");
    assert!(ctx.read_response(id.with("full")).is_some());
    assert!(ctx.read_response(id.with("window")).is_some());
    assert_eq!(press.gesture, None);
    assert_eq!(release.gesture, None);
    assert_eq!(e.to_portable().unwrap(), source);
}
