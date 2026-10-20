//! Artifact-only candidate tests; not executed during static preparation.
use super::*;

fn format_context(key: u64) -> TextFormatContext {
    TextFormatContext {
        enabled: true,
        selection_key: key,
        retained_bold: false,
        retained_italic: false,
        retained_underline: false,
        underline_enabled: true,
        underline_set_enabled: true,
        font_points: Some(12.),
        color_rgba: Some([20, 40, 60, 128]),
    }
}

#[test]
fn point_validation_keeps_original_positive_bounds_and_rejects_nondecimal_drafts() {
    for (text, expected) in [("1", 1.), ("1638", 1638.), ("12.3", 12.3), ("12.", 12.)] {
        assert_eq!(parse_text_font_points(text), Some(expected));
    }
    for text in [
        "", "0", "1638.1", "12.34", "NaN", "inf", "1e2", " 12", "12 ", "+12", "12,5",
    ] {
        assert_eq!(parse_text_font_points(text), None, "{text}");
    }
}

#[test]
fn invalid_size_restores_last_valid_and_typing_alone_emits_no_action() {
    let mut state = TextFormatDraft::default();
    state.sync(Some(format_context(11)));
    state.size = "18.5".into();
    assert_eq!(state.source_points, Some(12.));
    assert_eq!(
        state.commit_size(),
        Some(RibbonAction::SetTextFontSize {
            points: 18.5,
            selection_key: 11
        })
    );
    state.size = "broken".into();
    assert_eq!(state.commit_size(), None);
    assert_eq!(state.size, "18.5");
}

#[test]
fn changing_selection_discards_uncommitted_size_and_color_and_keeps_target_key() {
    let mut state = TextFormatDraft::default();
    state.sync(Some(format_context(11)));
    state.size = "18.5".into();
    state.color = [1, 2, 3, 4];
    state.color_dirty = true;
    state.sync(Some(format_context(12)));
    assert_eq!(state.size, "12");
    assert_eq!(state.commit_size(), None);
    assert_eq!(state.commit_color(), None);
    state.size = "20".into();
    assert_eq!(
        state.commit_size(),
        Some(RibbonAction::SetTextFontSize {
            points: 20.,
            selection_key: 12
        })
    );
    state.sync(None);
    assert_eq!(state.commit_size(), None);
    assert_eq!(state.commit_color(), None);
}

#[test]
fn color_commit_preserves_unmultiplied_alpha_and_only_emits_once() {
    let mut state = TextFormatDraft::default();
    state.sync(Some(format_context(5)));
    state.color = [200, 100, 50, 128];
    state.color_dirty = true;
    assert_eq!(
        state.commit_color(),
        Some(RibbonAction::SetTextColor {
            rgba: [200, 100, 50, 128],
            selection_key: 5
        })
    );
    assert_eq!(state.commit_color(), None);
    assert_eq!(state.last_used_color, Some([200, 100, 50, 128]));
}

fn context(value: Option<TextFormatContext>) -> RibbonContext<'static> {
    RibbonContext {
        document_name: "test",
        can_undo: false,
        can_redo: false,
        has_selection: false,
        can_insert_area: false,
        can_page_break: false,
        can_remove_page_break_above: false,
        can_remove_page_break_below: false,
        zoom: 1.,
        grid: false,
        font_points: 11.,
        precision: 3,
        result_format: 0,
        number_format: MathFormat::default(),
        matrix_indices: None,
        has_math: false,
        has_plot: false,
        draft_view: false,
        in_matrix: false,
        in_table: false,
        can_label: false,
        has_errors: false,
        automatic_calculation: false,
        bin_oct_hex_mode: Some(false),
        calculation_running: false,
        region_calculation_disabled: None,
        show_warnings: false,
        has_warnings: false,
        text_format: value,
        paragraph_alignment: None,
    }
}

fn frame(
    ctx: &egui::Context,
    draft: &mut TextFormatDraft,
    value: Option<TextFormatContext>,
    events: Vec<egui::Event>,
    time: f64,
) -> Vec<RibbonAction> {
    let mut actions = vec![];
    draft.sync(value);
    let output = ctx.run_ui(
        egui::RawInput {
            events,
            time: Some(time),
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1000., 200.))),
            ..Default::default()
        },
        |ui| {
            text_format_tab(ui, &context(value), draft, &mut actions);
        },
    );
    output.drop_without_applying_deltas();
    actions
}

#[test]
fn actual_size_field_commits_on_enter_not_on_each_text_event() {
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    let mut draft = TextFormatDraft::default();
    let value = Some(format_context(91));
    assert!(frame(&ctx, &mut draft, value, vec![], 1.).is_empty());
    ctx.memory_mut(|memory| memory.request_focus(egui::Id::new("ribbon-rich-text-font-points")));
    let key = |key, modifiers| egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    };
    assert!(
        frame(
            &ctx,
            &mut draft,
            value,
            vec![
                key(egui::Key::A, egui::Modifiers::COMMAND),
                egui::Event::Text("18.5".into())
            ],
            2.
        )
        .is_empty()
    );
    assert_eq!(draft.size, "18.5");
    assert_eq!(
        frame(
            &ctx,
            &mut draft,
            value,
            vec![key(egui::Key::Enter, egui::Modifiers::NONE)],
            3.
        ),
        vec![RibbonAction::SetTextFontSize {
            points: 18.5,
            selection_key: 91
        }]
    );
    assert!(frame(&ctx, &mut draft, value, vec![], 4.).is_empty());
}

#[test]
fn disabled_context_cannot_emit_font_actions_from_keyboard_delivery() {
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    let mut draft = TextFormatDraft::default();
    frame(&ctx, &mut draft, Some(format_context(1)), vec![], 1.);
    ctx.memory_mut(|memory| memory.request_focus(egui::Id::new("ribbon-rich-text-font-points")));
    assert!(
        frame(
            &ctx,
            &mut draft,
            None,
            vec![
                egui::Event::Text("20".into()),
                egui::Event::Key {
                    key: egui::Key::Enter,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE
                }
            ],
            2.
        )
        .is_empty()
    );
    assert_eq!(draft.selection_key, None);
}

#[test]
fn actual_underline_button_uses_explicit_set_capability_not_keyboard_known_value() {
    fn find_u(shape: &egui::Shape) -> Option<Pos2> {
        match shape {
            egui::Shape::Text(t) if t.galley.text() == "U" => Some(t.pos + t.galley.size() * 0.5),
            egui::Shape::Vec(v) => v.iter().find_map(find_u),
            _ => None,
        }
    }
    for (set_enabled, toggle_known, retained) in [
        (true, false, false),
        (true, false, true),
        (true, true, false),
        (false, true, true),
    ] {
        let ctx = egui::Context::default();
        crate::configure(&ctx);
        let mut draft = TextFormatDraft::default();
        let mut c = format_context(821);
        c.underline_enabled = toggle_known;
        c.underline_set_enabled = set_enabled;
        c.retained_underline = retained;
        draft.sync(Some(c));
        let mut actions = Vec::new();
        let output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1000., 200.))),
                ..Default::default()
            },
            |ui| text_format_tab(ui, &context(Some(c)), &mut draft, &mut actions),
        );
        let point = output.shapes.iter().find_map(|s| find_u(&s.shape));
        output.drop_without_applying_deltas();
        let point = point.expect("Actual U glyph");
        assert!(actions.is_empty());
        let mut events = vec![egui::Event::PointerMoved(point)];
        for pressed in [true, false] {
            events.push(egui::Event::PointerButton {
                pos: point,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            });
        }
        let actions = frame(&ctx, &mut draft, Some(c), events, 2.);
        if set_enabled {
            assert!(
                matches!(actions.as_slice(), [RibbonAction::SetTextUnderline { value, selection_key: 821 }] if *value != retained)
            );
        } else {
            assert!(actions.is_empty());
        }
    }
}
