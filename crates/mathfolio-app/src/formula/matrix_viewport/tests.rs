//! Unexecuted headless adoption regressions. All FullOutputs are dropped before assertions.
use super::*;
use crate::result_display::DisplayMath;

fn projection(rows: usize, cols: usize) -> MatrixProjection {
    MatrixProjection {
        full_rows: rows + 3,
        full_cols: cols + 3,
        row_offset: 1,
        col_offset: 1,
        rows,
        cols,
        cells: (0..rows * cols)
            .map(|n| DisplayMath::Text(format!("cell{n}"), MathStyle::Operator))
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
fn glyphs(layout: &FormulaBox) -> Vec<String> {
    layout
        .primitives
        .iter()
        .filter_map(|p| {
            if let Primitive::Text(_, g) = p {
                Some(g.text().to_owned())
            } else {
                None
            }
        })
        .collect()
}

#[test]
fn twelve_by_twelve_and_larger_keep_every_data_cell_and_separate_chrome() {
    for (rows, cols) in [(12, 12), (13, 17), (1, 1)] {
        let ctx = context();
        let e = editor();
        let p = projection(rows, cols);
        let before = p.clone();
        let mut captured = None;
        ctx.run_ui(egui::RawInput::default(), |ui| {
            let measured = measure(ui.painter(), e.root(), &p, 16.);
            captured = Some((glyphs(&measured), measured.size, measured.matrix_viewport));
        })
        .drop_without_applying_deltas();
        let (labels, bounds, geometry) = captured.unwrap();
        for i in 0..rows * cols {
            assert_eq!(
                labels.iter().filter(|s| *s == &format!("cell{i}")).count(),
                1
            );
        }
        assert_eq!(labels.iter().filter(|s| s.as_str() == "…").count(), 2);
        assert_eq!(labels.iter().filter(|s| s.as_str() == "⋮").count(), 2);
        assert!(bounds.is_finite());
        assert_eq!(geometry.unwrap().size, [rows, cols]);
        assert_eq!(p, before);
    }
}

#[test]
fn malformed_projection_is_explicitly_unavailable_and_not_interactive() {
    let ctx = context();
    let e = editor();
    let mut p = projection(2, 2);
    p.cells.pop();
    let mut captured = None;
    ctx.run_ui(egui::RawInput::default(), |ui| {
        let measured = measure(ui.painter(), e.root(), &p, 16.);
        captured = Some((glyphs(&measured), measured.matrix_viewport.is_none()));
    })
    .drop_without_applying_deltas();
    let (text, no_handle) = captured.unwrap();
    assert_eq!(text, ["[matrix view unavailable]"]);
    assert!(no_handle);
}

#[test]
fn retained_projection_matches_complete_window_and_reuses_idle_layout() {
    let ctx = context();
    let e = editor();
    let p = projection(12, 12);
    let mut captured = None;
    ctx.run_ui(egui::RawInput::default(), |ui| {
        let a = retained_layout(ui, &e, None, 16., Some(FormulaResult::MatrixViewport(&p)));
        let b = retained_layout(ui, &e, None, 16., Some(FormulaResult::MatrixViewport(&p)));
        let mut changed = p.clone();
        changed.cells[143] = DisplayMath::Text("last-cell-changed".into(), MathStyle::Operator);
        let c = retained_layout(
            ui,
            &e,
            None,
            16.,
            Some(FormulaResult::MatrixViewport(&changed)),
        );
        captured = Some((
            Arc::ptr_eq(&a, &b),
            Arc::ptr_eq(&b, &c),
            find(&a, Pos2::ZERO).is_some(),
        ));
    })
    .drop_without_applying_deltas();
    assert_eq!(captured.unwrap(), (true, false, true));
}

fn pointer(point: Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos: point,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    }
}
fn key(key: Key) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }
}
struct Frame {
    gesture: Option<MatrixViewGesture>,
    geometry: Option<Geometry>,
    changed: bool,
    evaluated: bool,
}
fn frame(
    ctx: &egui::Context,
    e: &mut Editor,
    p: Option<&MatrixProjection>,
    events: Vec<egui::Event>,
) -> Frame {
    let mut captured = None;
    ctx.run_ui(
        egui::RawInput {
            events,
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1600., 1200.))),
            ..Default::default()
        },
        |ui| {
            let drawn = formula_ui_with_actions(ui, e, p.map(FormulaResult::MatrixViewport), false);
            let layout = retained_layout(
                ui,
                e,
                None,
                11. * 96. / 72.,
                p.map(FormulaResult::MatrixViewport),
            );
            let geometry = find(&layout, drawn.response.rect.min + vec2(PADDING, PADDING));
            captured = Some(Frame {
                gesture: drawn.matrix_view_gesture,
                geometry,
                changed: drawn.response.changed(),
                evaluated: drawn.evaluation_requested,
            });
        },
    )
    .drop_without_applying_deltas();
    captured.unwrap()
}

#[test]
fn actual_bracket_drag_is_one_begin_preview_commit_and_source_inert() {
    let ctx = context();
    let mut e = editor();
    let p = projection(2, 3);
    let source = e.to_portable().unwrap();
    let caret = e.caret();
    let revision = e.arena().revision();
    let idle = frame(&ctx, &mut e, Some(&p), vec![]);
    let g = idle.geometry.unwrap();
    let start = g.bracket.right_bottom();
    frame(
        &ctx,
        &mut e,
        Some(&p),
        vec![egui::Event::PointerMoved(start)],
    );
    let begin = frame(&ctx, &mut e, Some(&p), vec![pointer(start, true)]);
    let moved = start + vec2(g.step.x * 2., g.step.y);
    let preview = frame(
        &ctx,
        &mut e,
        Some(&p),
        vec![egui::Event::PointerMoved(moved)],
    );
    let end = frame(&ctx, &mut e, Some(&p), vec![pointer(moved, false)]);
    assert_eq!(
        begin.gesture,
        Some(MatrixViewGesture::Begin {
            size: [2, 3],
            offset: [1, 1]
        })
    );
    assert_eq!(
        preview.gesture,
        Some(MatrixViewGesture::Preview {
            size: [3, 5],
            offset: [1, 1]
        })
    );
    assert_eq!(
        end.gesture,
        Some(MatrixViewGesture::Commit {
            size: [3, 5],
            offset: [1, 1]
        })
    );
    assert!(!begin.changed && !preview.changed && !end.changed);
    assert!(!begin.evaluated && !preview.evaluated && !end.evaluated);
    assert_eq!(e.to_portable().unwrap(), source);
    assert_eq!(e.caret(), caret);
    assert_eq!(e.arena().revision(), revision);
    assert!(!e.undo());
}

#[test]
fn click_without_motion_commits_same_window_for_controller_noop() {
    let ctx = context();
    let mut e = editor();
    let p = projection(2, 3);
    let start = frame(&ctx, &mut e, Some(&p), vec![])
        .geometry
        .unwrap()
        .bracket
        .right_bottom();
    frame(
        &ctx,
        &mut e,
        Some(&p),
        vec![egui::Event::PointerMoved(start)],
    );
    let a = frame(&ctx, &mut e, Some(&p), vec![pointer(start, true)]);
    let b = frame(&ctx, &mut e, Some(&p), vec![pointer(start, false)]);
    assert_eq!(
        a.gesture,
        Some(MatrixViewGesture::Begin {
            size: [2, 3],
            offset: [1, 1]
        })
    );
    assert_eq!(
        b.gesture,
        Some(MatrixViewGesture::Commit {
            size: [2, 3],
            offset: [1, 1]
        })
    );
}

#[test]
fn escape_pointer_loss_and_result_target_changes_cancel_exactly_once() {
    for mode in 0..5 {
        let ctx = context();
        let mut e = editor();
        let p = projection(2, 3);
        let start = frame(&ctx, &mut e, Some(&p), vec![])
            .geometry
            .unwrap()
            .bracket
            .right_bottom();
        frame(
            &ctx,
            &mut e,
            Some(&p),
            vec![egui::Event::PointerMoved(start)],
        );
        let a = frame(&ctx, &mut e, Some(&p), vec![pointer(start, true)]);
        let events = match mode {
            0 => vec![key(Key::Escape)],
            1 => vec![egui::Event::PointerGone],
            _ => vec![],
        };
        let mut other = p.clone();
        if mode == 3 {
            other.full_rows += 1;
        }
        if mode == 4 {
            other.row_offset += 1;
        }
        let visible = match mode {
            2 => None,
            3 | 4 => Some(&other),
            _ => Some(&p),
        };
        let b = frame(&ctx, &mut e, visible, events);
        let c = frame(&ctx, &mut e, visible, vec![pointer(start, false)]);
        assert!(matches!(a.gesture, Some(MatrixViewGesture::Begin { .. })));
        assert_eq!(b.gesture, Some(MatrixViewGesture::Cancel));
        assert_eq!(c.gesture, None);
    }
}

#[test]
fn continuation_glyphs_are_outside_the_complete_data_rectangle() {
    let ctx = context();
    let e = editor();
    let p = projection(12, 12);
    let mut captured = None;
    ctx.run_ui(egui::RawInput::default(), |ui| {
        let measured = measure(ui.painter(), e.root(), &p, 16.);
        let mut data = Rect::NOTHING;
        let mut chrome = Vec::new();
        for primitive in &measured.primitives {
            if let Primitive::Text(at, g) = primitive {
                let rect = Rect::from_min_size(at.to_pos2(), g.size());
                if g.text().starts_with("cell") {
                    data = data.union(rect);
                } else {
                    chrome.push(rect);
                }
            }
        }
        captured = Some((data, chrome, measured.size));
    })
    .drop_without_applying_deltas();
    let (data, chrome, bounds) = captured.unwrap();
    assert_eq!(chrome.len(), 4);
    for rect in chrome {
        assert!(!data.intersects(rect));
        assert!(Rect::from_min_size(Pos2::ZERO, bounds).contains_rect(rect));
    }
}

#[test]
fn source_revision_change_cancels_without_reverting_that_source() {
    let ctx = context();
    let mut e = editor();
    let p = projection(2, 3);
    let start = frame(&ctx, &mut e, Some(&p), vec![])
        .geometry
        .unwrap()
        .bracket
        .right_bottom();
    frame(
        &ctx,
        &mut e,
        Some(&p),
        vec![egui::Event::PointerMoved(start)],
    );
    frame(&ctx, &mut e, Some(&p), vec![pointer(start, true)]);
    e.select_subtree(e.root()).unwrap();
    e.insert_typed_operator(NodeKind::Binary(BinaryOp::Add));
    let after = e.to_portable().unwrap();
    let b = frame(&ctx, &mut e, Some(&p), vec![]);
    assert_eq!(b.gesture, Some(MatrixViewGesture::Cancel));
    assert_eq!(e.to_portable().unwrap(), after);
}

#[test]
fn resize_quantization_is_bounded_and_allows_size_preserving_offset_shift() {
    let mut e = editor();
    let g = Geometry {
        bracket: Rect::from_min_size(Pos2::ZERO, vec2(100., 100.)),
        step: vec2(10., 10.),
        shape: [1000, 1000],
        size: [12, 12],
        offset: [988, 988],
        markers: [None; 4],
    };
    let mut d = Drag {
        geometry: g,
        start: Pos2::ZERO,
        last: g.size,
        root: e.root(),
        revision: e.arena().revision(),
    };
    assert_eq!(d.requested(pos2(80., 80.)), [20, 20]);
    d.last = [20, 20];
    let mut shifted = g;
    shifted.offset = [980, 980];
    shifted.size = [20, 20];
    assert!(d.compatible(&e, Some(shifted)));
    let huge = d.requested(pos2(100000., 100000.));
    assert!(huge[0] * huge[1] <= 65536);
    assert_eq!(d.requested(pos2(-100000., -100000.)), [1, 1]);
    e.select_subtree(e.root()).unwrap();
    // Selection alone does not invalidate the underlying value/source revision.
    assert!(d.compatible(&e, Some(shifted)));
}

#[test]
fn readonly_matrix_and_legacy_results_have_no_drag_state_or_matrix_gesture() {
    let ctx = context();
    let mut e = editor();
    let p = projection(2, 3);
    let mut seen = None;
    ctx.run_ui(egui::RawInput::default(), |ui| {
        let r =
            formula_readonly_ui_styled(ui, &e, Some(FormulaResult::MatrixViewport(&p)), None, 11.);
        seen = Some(r.changed());
    })
    .drop_without_applying_deltas();
    assert_eq!(seen, Some(false));
    let mut seen = None;
    ctx.run_ui(egui::RawInput::default(), |ui| {
        let r = formula_ui_with_actions(ui, &mut e, Some(FormulaResult::Text("scalar")), false);
        seen = Some((
            r.matrix_view_gesture,
            r.evaluation_requested,
            r.response.changed(),
        ));
    })
    .drop_without_applying_deltas();
    assert_eq!(seen, Some((None, false, false)));
}
