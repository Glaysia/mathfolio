use super::*;

fn frame(
    app: &mut WorksheetApp,
    ctx: &egui::Context,
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    ctx.run_ui(
        egui::RawInput {
            events,
            screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(2400., 2700.))),
            ..Default::default()
        },
        |ui| {
            app.region_clipboard_shortcuts(ui.ctx());
            app.chrome_ui(ui);
            egui::CentralPanel::default().show(ui, |ui| {
                let layer =
                    egui::LayerId::new(ui.layer_id().order, egui::Id::new("worksheet-paper"));
                ui.ctx()
                    .data_mut(|d| d.insert_temp(egui::Id::new("selection-test-layer"), layer));
                app.workspace_ui(ui);
            });
        },
    )
}
fn button(point: egui::Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos: point,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    }
}
fn key(key: egui::Key) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }
}
fn world(ctx: &egui::Context, point: egui::Pos2) -> egui::Pos2 {
    ctx.layer_transform_to_global(
        ctx.data(|d| d.get_temp::<egui::LayerId>(egui::Id::new("selection-test-layer")))
            .unwrap(),
    )
    .unwrap()
        * point
}
fn rect(app: &WorksheetApp, id: u64) -> Rect {
    let r = app.document.regions.iter().find(|r| r.id == id).unwrap();
    app.region_selection.bounds[&id]
        .translate(app.page_geometry().to_paper(r.left, r.top).to_vec2())
}
fn fixture(zoom: f32) -> (WorksheetApp, egui::Context, Vec<u64>) {
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    let mut app = WorksheetApp {
        zoom,
        automatic_calculation: false,
        insertion: None,
        ..Default::default()
    };
    let mut ids = Vec::new();
    for (x, y, text) in [
        (100., 100., "1234"),
        (200., 180., "5678"),
        (500., 350., "9999"),
    ] {
        let id = app.document.add_math(x, y);
        let mut e = Editor::new();
        formula::input_text(&mut e, text);
        if ids.len() == 1 {
            app.document.regions.last_mut().unwrap().content = CanvasContent::Text(text.into());
        } else {
            app.document.regions.last_mut().unwrap().content =
                CanvasContent::Math(e.to_portable().unwrap());
            app.editors.insert(id, e);
        }
        ids.push(id);
    }
    frame(&mut app, &ctx, vec![]).drop_without_applying_deltas();
    frame(&mut app, &ctx, vec![]).drop_without_applying_deltas();
    (app, ctx, ids)
}
fn drag(app: &mut WorksheetApp, ctx: &egui::Context, from: egui::Pos2, to: egui::Pos2) {
    let (from, to) = (world(ctx, from), world(ctx, to));
    frame(
        app,
        ctx,
        vec![egui::Event::PointerMoved(from), button(from, true)],
    )
    .drop_without_applying_deltas();
    frame(app, ctx, vec![egui::Event::PointerMoved(to)]).drop_without_applying_deltas();
    frame(app, ctx, vec![button(to, false)]).drop_without_applying_deltas();
}
fn select_two(app: &mut WorksheetApp, ctx: &egui::Context, ids: &[u64]) {
    let bounds = rect(app, ids[0]).union(rect(app, ids[1])).expand(20.);
    drag(app, ctx, bounds.left_top(), bounds.right_bottom());
    assert_eq!(app.selected_region_ids(), ids[..2]);
}
fn copy(app: &mut WorksheetApp, ctx: &egui::Context) -> String {
    let out = frame(app, ctx, vec![egui::Event::Copy]);
    let value = out
        .platform_output
        .commands
        .iter()
        .find_map(|c| match c {
            egui::OutputCommand::CopyText(s) => Some(s.clone()),
            _ => None,
        })
        .expect("OS clipboard packet");
    out.drop_without_applying_deltas();
    value
}

#[test]
fn cad_window_and_crossing_depend_only_on_horizontal_direction_at_three_zooms() {
    for zoom in [0.5, 1., 2.] {
        for reverse_y in [false, true] {
            for crossing in [false, true] {
                let (mut app, ctx, ids) = fixture(zoom);
                let a = rect(&app, ids[0]);
                let b = rect(&app, ids[1]);
                let (left, right) = (a.left() - 20., b.center().x);
                let (top, bottom) = (a.top() - 20., b.bottom() + 20.);
                let start = pos2(
                    if crossing { right } else { left },
                    if reverse_y { bottom } else { top },
                );
                let end = pos2(
                    if crossing { left } else { right },
                    if reverse_y { top } else { bottom },
                );
                let before = app.document.clone();
                let generation = app.background.submitted_generation();
                drag(&mut app, &ctx, start, end);
                assert_eq!(
                    app.selected_region_ids(),
                    if crossing {
                        ids[..2].to_vec()
                    } else {
                        vec![ids[0]]
                    },
                    "zoom={zoom} crossing={crossing} reverse_y={reverse_y}"
                );
                assert_eq!(app.document, before);
                assert!(app.undo.is_empty());
                assert!(!app.dirty);
                assert_eq!(app.background.submitted_generation(), generation);
                assert!(app.committed_input.is_none());
            }
        }
    }
}

#[test]
fn group_drag_previews_only_then_commits_once_with_relative_offsets_and_undo_redo() {
    for zoom in [0.5, 1., 2.] {
        let (mut app, ctx, ids) = fixture(zoom);
        select_two(&mut app, &ctx, &ids);
        let before = app.document.clone();
        let generation = app.background.submitted_generation();
        let from = world(&ctx, rect(&app, ids[1]).center());
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(from), button(from, true)],
        )
        .drop_without_applying_deltas();
        for delta in [vec2(22., 12.), vec2(50., 35.), vec2(80., 55.)] {
            frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(from + delta * zoom)],
            )
            .drop_without_applying_deltas();
            assert_eq!(app.document, before);
            assert!(app.undo.is_empty());
            assert!(!app.dirty);
            assert_eq!(app.background.submitted_generation(), generation);
        }
        frame(
            &mut app,
            &ctx,
            vec![button(from + vec2(80., 55.) * zoom, false)],
        )
        .drop_without_applying_deltas();
        assert_eq!(app.undo.len(), 1);
        assert!(app.dirty);
        assert_ne!(app.document.regions[0].left, before.regions[0].left);
        assert_eq!(app.document.regions[2], before.regions[2]);
        let dx = app.document.regions[0].left - before.regions[0].left;
        let dy = app.document.regions[0].top - before.regions[0].top;
        assert!((app.document.regions[1].left - before.regions[1].left - dx).abs() < 1e-10);
        assert!((app.document.regions[1].top - before.regions[1].top - dy).abs() < 1e-10);
        let moved = app.document.regions.clone();
        app.undo();
        assert_eq!(app.document.regions, before.regions);
        assert_eq!(app.selected_region_ids(), ids[..2]);
        app.redo();
        assert_eq!(app.document.regions, moved);
    }
}

#[test]
fn copy_paste_cut_and_delete_are_group_transactions_with_fresh_ids_and_stable_sources() {
    let (mut app, ctx, ids) = fixture(1.);
    select_two(&mut app, &ctx, &ids);
    let original = app.document.regions.clone();
    let text = copy(&mut app, &ctx);
    assert!(text.starts_with(region_selection::CLIPBOARD_PREFIX));
    assert!(app.undo.is_empty());
    assert!(!app.dirty);
    assert!(app.paste_region(&text));
    assert_eq!(app.document.regions.len(), 5);
    assert_eq!(app.undo.len(), 1);
    assert_eq!(&app.document.regions[..3], &original);
    for (i, original) in original.iter().take(2).enumerate() {
        assert_eq!(app.document.regions[3 + i].content, original.content);
        assert_ne!(app.document.regions[3 + i].id, original.id);
    }
    assert!(
        ((app.document.regions[4].left - app.document.regions[3].left)
            - (original[1].left - original[0].left))
            .abs()
            < 1e-10
    );
    let pasted = app.selected_region_ids();
    assert_eq!(pasted.len(), 2);
    let out = frame(&mut app, &ctx, vec![key(egui::Key::Delete)]);
    out.drop_without_applying_deltas();
    assert_eq!(app.document.regions, original);
    assert_eq!(app.undo.len(), 2);
    app.undo();
    assert_eq!(app.document.regions.len(), 5);
    assert_eq!(app.selected_region_ids(), pasted);
    let out = frame(&mut app, &ctx, vec![egui::Event::Cut]);
    assert!(out.platform_output.commands.iter().any(|c|matches!(c,egui::OutputCommand::CopyText(s) if s.starts_with(region_selection::CLIPBOARD_PREFIX))));
    out.drop_without_applying_deltas();
    assert_eq!(app.document.regions, original);
    app.undo();
    assert_eq!(app.document.regions.len(), 5);
    assert_eq!(app.selected_region_ids(), pasted);
}

#[test]
fn invalid_item_or_offset_rejects_the_whole_group_without_history_or_id_consumption() {
    let (mut app, ctx, ids) = fixture(1.);
    select_two(&mut app, &ctx, &ids);
    let text = copy(&mut app, &ctx);
    let packet: serde_json::Value = serde_json::from_str(
        text.strip_prefix(region_selection::CLIPBOARD_PREFIX)
            .unwrap(),
    )
    .unwrap();
    for invalid in [
        serde_json::json!({"content":{"Text":"x"},"format":{"font_points":-5}}),
        serde_json::json!({"content":{"Math":{"kind":"NoSuchKind","children":[]}}}),
    ] {
        let mut packet = packet.clone();
        packet["items"][1]["region"] = invalid;
        let before = app.document.clone();
        let selected = app.selected_region_ids();
        assert!(app.paste_region(&format!("{}{packet}", region_selection::CLIPBOARD_PREFIX)));
        assert_eq!(app.document, before);
        assert_eq!(app.selected_region_ids(), selected);
        assert!(app.undo.is_empty());
        assert!(!app.dirty);
    }
    let mut packet = packet;
    packet["items"][1]["left"] = serde_json::json!(1e100);
    let before = app.document.clone();
    app.paste_region(&format!("{}{packet}", region_selection::CLIPBOARD_PREFIX));
    assert_eq!(app.document, before);
}

#[test]
fn escape_cancels_group_move_and_a_blank_click_clears_group_selection() {
    let (mut app, ctx, ids) = fixture(1.);
    select_two(&mut app, &ctx, &ids);
    let before = app.document.clone();
    let from = world(&ctx, rect(&app, ids[0]).center());
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(from), button(from, true)],
    )
    .drop_without_applying_deltas();
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(from + vec2(80., 60.))],
    )
    .drop_without_applying_deltas();
    frame(&mut app, &ctx, vec![key(egui::Key::Escape)]).drop_without_applying_deltas();
    frame(&mut app, &ctx, vec![button(from + vec2(80., 60.), false)])
        .drop_without_applying_deltas();
    assert_eq!(app.document, before);
    assert!(app.undo.is_empty());
    let blank = world(&ctx, pos2(450., 500.));
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(blank), button(blank, true)],
    )
    .drop_without_applying_deltas();
    frame(&mut app, &ctx, vec![button(blank, false)]).drop_without_applying_deltas();
    assert!(app.selected_region_ids().is_empty());
    assert_eq!(app.document, before);
}

#[test]
fn crossing_can_start_in_the_blank_space_immediately_right_of_a_region() {
    let (mut app, ctx, ids) = fixture(1.);
    let r = rect(&app, ids[0]);
    drag(
        &mut app,
        &ctx,
        pos2(r.right() + 3., r.top() - 3.),
        pos2(r.left() - 3., r.bottom() + 3.),
    );
    assert_eq!(app.selected_region_ids(), vec![ids[0]]);
    assert!(!app.dirty);
    assert!(app.undo.is_empty());
}

#[test]
fn marquee_can_start_in_page_margins_and_outside_the_paper_at_three_zooms() {
    for zoom in [0.5, 1., 2.] {
        for case in 0..7 {
            let (mut app, ctx, ids) = fixture(zoom);
            let geometry = app.page_geometry();
            let a = rect(&app, ids[0]);
            let b = rect(&app, ids[1]);
            let start = match case {
                0 => pos2(geometry.margins[0] / 2., a.top() - 20.),
                1 => pos2(a.left() - 20., geometry.margins[1] / 2.),
                2 => pos2(-12., a.top() - 20.),
                3 => pos2(a.left() - 20., -12.),
                4 => pos2(geometry.width - geometry.margins[2] / 2., a.top() - 20.),
                5 => pos2(geometry.width + 12., a.top() - 20.),
                _ => pos2(geometry.width + 12., app.paper_size().y + 12.),
            };
            let end = if case >= 4 {
                pos2(
                    a.left() - 20.,
                    if case == 6 { -12. } else { b.bottom() + 20. },
                )
            } else {
                b.right_bottom() + vec2(20., 20.)
            };
            let before = app.document.clone();
            let generation = app.background.submitted_generation();
            drag(&mut app, &ctx, start, end);
            assert_eq!(
                app.selected_region_ids(),
                ids[..if case == 6 { 3 } else { 2 }],
                "case={case} zoom={zoom}"
            );
            assert_eq!(app.document, before);
            assert!(app.undo.is_empty());
            assert!(!app.dirty);
            assert_eq!(app.background.submitted_generation(), generation);
        }
    }
}

#[test]
fn pointer_press_outside_the_workspace_cannot_start_a_marquee() {
    let (mut app, ctx, ids) = fixture(1.);
    let end = world(&ctx, rect(&app, ids[1]).right_bottom() + vec2(20., 20.));
    let outside = pos2(-50., -50.);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(outside), button(outside, true)],
    )
    .drop_without_applying_deltas();
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]).drop_without_applying_deltas();
    assert!(!app.region_selection.busy());
    frame(&mut app, &ctx, vec![button(end, false)]).drop_without_applying_deltas();
    assert!(app.selected_region_ids().is_empty());
}

#[test]
fn marquee_can_start_in_the_gap_between_pages() {
    let (mut app, ctx, _) = fixture(1.);
    let geometry = app.page_geometry();
    let id = app
        .document
        .add_math(100., geometry.body_height_exact() + 100.);
    frame(&mut app, &ctx, vec![]).drop_without_applying_deltas();
    frame(&mut app, &ctx, vec![]).drop_without_applying_deltas();
    let bounds = rect(&app, id);
    let before = app.document.clone();
    drag(
        &mut app,
        &ctx,
        pos2(bounds.left() - 20., geometry.height + geometry.grid / 2.),
        bounds.right_bottom() + vec2(20., 20.),
    );
    assert_eq!(app.selected_region_ids(), vec![id]);
    assert_eq!(app.document, before);
    assert!(app.undo.is_empty());
    assert!(!app.dirty);
}

#[test]
fn page_view_toggle_owns_its_press_instead_of_starting_a_marquee() {
    for zoom in [0.5, 1., 2.] {
        let (mut app, ctx, _) = fixture(zoom);
        app.expand_right = true;
        frame(&mut app, &ctx, vec![]).drop_without_applying_deltas();
        frame(&mut app, &ctx, vec![]).drop_without_applying_deltas();
        let point = world(
            &ctx,
            pos2(app.document.width as f32 + 1. / zoom, -12. / zoom),
        );
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(point), button(point, true)],
        )
        .drop_without_applying_deltas();
        assert!(!app.region_selection.busy(), "toggle press zoom={zoom}");
        frame(&mut app, &ctx, vec![button(point, false)]).drop_without_applying_deltas();
        assert!(!app.expand_right);
        assert!(app.selected_region_ids().is_empty());
    }
}

#[test]
fn floating_scrollbar_press_scrolls_without_starting_a_marquee() {
    let (mut app, ctx, _) = fixture(1.);
    app.document.add_math(100., 10_000.);
    frame(&mut app, &ctx, vec![]).drop_without_applying_deltas();
    frame(&mut app, &ctx, vec![]).drop_without_applying_deltas();
    let id = app.region_selection.scroll_area.unwrap();
    let bar = ctx
        .read_response(id.with(1_usize))
        .expect("vertical scrollbar");
    let point = ctx
        .layer_transform_to_global(bar.layer_id)
        .unwrap_or_default()
        * bar.rect.center();
    let before = app.document.clone();
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(point), button(point, true)],
    )
    .drop_without_applying_deltas();
    assert!(!app.region_selection.busy(), "scrollbar owns the press");
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(point - vec2(0., 40.))],
    )
    .drop_without_applying_deltas();
    assert!(!app.region_selection.busy());
    frame(&mut app, &ctx, vec![button(point - vec2(0., 40.), false)])
        .drop_without_applying_deltas();
    assert!(egui::scroll_area::State::load(&ctx, id).unwrap().offset.y > 0.);
    assert_eq!(app.document, before);
    assert!(app.undo.is_empty());
    assert!(!app.dirty);
}

#[test]
fn group_preview_extends_the_scrollable_paper_without_committing_positions() {
    let (mut app, ctx, ids) = fixture(1.);
    select_two(&mut app, &ctx, &ids);
    let size = app.paper_size();
    let before = app.document.clone();
    let start = world(&ctx, rect(&app, ids[0]).center());
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), button(start, true)],
    )
    .drop_without_applying_deltas();
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start + vec2(0., 1800.))],
    )
    .drop_without_applying_deltas();
    assert!(app.paper_size().y > size.y);
    assert_eq!(app.document, before);
    assert!(app.undo.is_empty());
}

#[test]
fn expanded_paper_group_movement_can_cross_the_a4_boundary() {
    let (mut app, ctx, ids) = fixture(1.);
    app.expand_right = true;
    frame(&mut app, &ctx, vec![]).drop_without_applying_deltas();
    select_two(&mut app, &ctx, &ids);
    let original = app.document.regions.clone();
    let start = world(&ctx, rect(&app, ids[0]).center());
    let end = start + vec2(1000., 0.);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), button(start, true)],
    )
    .drop_without_applying_deltas();
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]).drop_without_applying_deltas();
    frame(&mut app, &ctx, vec![button(end, false)]).drop_without_applying_deltas();
    assert!(app.document.regions[0].left > app.document.width);
    assert!(
        (app.document.regions[1].left
            - app.document.regions[0].left
            - (original[1].left - original[0].left))
            .abs()
            < 1e-10
    );
    assert_eq!(app.document.regions[2], original[2]);
    assert_eq!(app.undo.len(), 1);
}
