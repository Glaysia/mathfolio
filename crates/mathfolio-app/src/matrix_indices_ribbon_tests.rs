use super::*;
#[test]
fn actual_indices_button_emits_explicit_inverse_and_disabled_button_emits_nothing() {
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    let mut time = 0.;
    let mut draw = |current, events| {
        // Three frames represent a click, not a one-second held press. egui
        // deliberately excludes holds exceeding max_click_duration from clicks.
        time += 1. / 60.;
        let mut actions = Vec::new();
        let mut rect = Rect::NOTHING;
        let output = ctx.run_ui(
            egui::RawInput {
                time: Some(time),
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(500., 200.))),
                events,
                ..Default::default()
            },
            |ui| {
                let response = matrix_indices_button(ui, current, &mut actions);
                rect = response.rect;
            },
        );
        output.drop_without_applying_deltas();
        (rect, actions)
    };
    let current = Some(MatrixIndicesContext {
        owner: 42,
        show_indices: false,
    });
    let (rect, _) = draw(current, vec![]);
    let pos = rect.center();
    let event = |pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    let _ = draw(current, vec![egui::Event::PointerMoved(pos), event(true)]);
    let (_, actions) = draw(current, vec![event(false)]);
    assert!(matches!(
        actions.as_slice(),
        [RibbonAction::SetMatrixIndices {
            owner: 42,
            show: true
        }]
    ));
    let selected = Some(MatrixIndicesContext {
        owner: 43,
        show_indices: true,
    });
    let _ = draw(selected, vec![event(true)]);
    let (_, actions) = draw(selected, vec![event(false)]);
    assert!(matches!(
        actions.as_slice(),
        [RibbonAction::SetMatrixIndices {
            owner: 43,
            show: false
        }]
    ));
    let _ = draw(None, vec![event(true)]);
    let (_, actions) = draw(None, vec![event(false)]);
    assert!(actions.is_empty());
}
