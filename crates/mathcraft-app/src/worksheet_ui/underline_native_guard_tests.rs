use super::*;

#[test]
fn native_unknown_keyboard_is_reserved_but_explicit_toolbar_assigns_and_undo_restores_absence() {
    let mut f = Fixture::new();
    let c = f.context();
    assert!(!c.underline_enabled && c.underline_set_enabled);
    let original = f.xml();
    let wire = f.wire().cloned();
    let history = f.app.undo.len();
    let generation = f.app.background.submitted_generation();
    f.frame(vec![key(
        egui::Key::U,
        egui::Modifiers::CTRL | egui::Modifiers::COMMAND,
    )]);
    assert_eq!(f.xml(), original);
    assert_eq!(f.app.undo.len(), history);
    f.command(|selection_key| RibbonAction::SetTextUnderline {
        value: true,
        selection_key,
    });
    assert_eq!(
        attr_on_text(&f.xml(), "CCL", "TextDecorations").as_deref(),
        Some("Underline")
    );
    assert_eq!(f.text(), ORIGINAL);
    assert_eq!(f.app.undo.len(), history + 1);
    assert!(f.context().underline_enabled);
    f.app.undo();
    assert_eq!(f.xml(), original);
    assert_eq!(f.wire(), wire.as_ref());
    assert!(!f.context().underline_enabled);
    f.app.redo();
    assert_eq!(
        attr_on_text(&f.xml(), "CCL", "TextDecorations").as_deref(),
        Some("Underline")
    );
    // Unknown prefix retains the previous toolbar checked value; explicit
    // clear may write None there, without changing the underlined CCL.
    f.select(0, 1);
    let c = f.context();
    assert!(c.retained_underline && c.underline_set_enabled && !c.underline_enabled);
    let prior = f.xml();
    let h = f.app.undo.len();
    f.frame(vec![key(
        egui::Key::U,
        egui::Modifiers::CTRL | egui::Modifiers::COMMAND,
    )]);
    assert_eq!(f.xml(), prior);
    assert_eq!(f.app.undo.len(), h);
    f.command(|selection_key| RibbonAction::SetTextUnderline {
        value: false,
        selection_key,
    });
    assert_eq!(
        attr_on_text(&f.xml(), "L", "TextDecorations").as_deref(),
        Some("None")
    );
    assert_eq!(
        attr_on_text(&f.xml(), "CCL", "TextDecorations").as_deref(),
        Some("Underline")
    );
    assert_eq!(f.app.undo.len(), h + 1);
    assert_eq!(f.app.background.submitted_generation(), generation);
    assert!(f.app.last_executed_regions.is_empty() && !f.app.background.running);
}

#[test]
fn native_explicit_none_keyboard_formats_and_undo_restores_exact_source() {
    let original = include_str!("../../../../tests/reference/rich-text/sample-01.xaml");
    let xml = original.replace("<Run ", "<Run TextDecorations=\"None\" ");
    assert_ne!(xml, original);
    let package = replaced_package_xaml(&xml);
    let mut f = Fixture::with_package(Some(&package));
    assert!(f.context().underline_enabled);
    let before = f.xml();
    let history = f.app.undo.len();
    f.frame(vec![key(
        egui::Key::U,
        egui::Modifiers::CTRL | egui::Modifiers::COMMAND,
    )]);
    assert_eq!(
        attr_on_text(&f.xml(), "CCL", "TextDecorations").as_deref(),
        Some("Underline")
    );
    assert_eq!(f.app.undo.len(), history + 1);
    f.app.undo();
    assert_eq!(f.xml(), before);
    f.app.redo();
    assert_eq!(
        attr_on_text(&f.xml(), "CCL", "TextDecorations").as_deref(),
        Some("Underline")
    );
}

#[test]
fn unrelated_focused_field_keeps_ctrl_u_and_worksheet_source_unchanged() {
    let mut f = Fixture::new();
    let source = f.xml();
    let history = f.app.undo.len();
    let event = key(
        egui::Key::U,
        egui::Modifiers::CTRL | egui::Modifiers::COMMAND,
    );
    let mut remaining = Vec::new();
    let output = f.ctx.run_ui(
        egui::RawInput {
            events: vec![event.clone()],
            ..Default::default()
        },
        |ui| {
            ui.memory_mut(|m| m.request_focus(egui::Id::new("unrelated-search-field")));
            f.app.rich_text_shortcuts(ui.ctx());
            remaining = ui.input(|i| i.events.clone());
        },
    );
    output.drop_without_applying_deltas();
    assert_eq!(remaining, vec![event]);
    assert_eq!(f.xml(), source);
    assert_eq!(f.app.undo.len(), history);
}

#[test]
fn imported_absent_decoration_actual_ribbon_click_publishes_one_source_edit() {
    fn find_u(shape: &egui::Shape) -> Option<egui::Pos2> {
        match shape {
            egui::Shape::Text(t) if t.galley.text() == "U" => Some(t.pos + t.galley.size() * 0.5),
            egui::Shape::Vec(v) => v.iter().find_map(find_u),
            _ => None,
        }
    }
    fn draw(f: &mut Fixture, events: Vec<egui::Event>) -> Option<egui::Pos2> {
        f.frame += 1;
        let mut raw = egui::RawInput {
            events,
            time: Some(f64::from(f.frame)),
            screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1200., 900.))),
            ..Default::default()
        };
        f.app.prepare_plot_input(&f.ctx, &mut raw);
        let output = f.ctx.run_ui(raw, |ui| f.app.app_ui_frame(ui));
        let point = output.shapes.iter().find_map(|s| find_u(&s.shape));
        output.drop_without_applying_deltas();
        point
    }
    let mut f = Fixture::new();
    f.app.ribbon.tab = ribbon::RibbonTab::TextFormatting;
    f.app.ribbon.tools_open = true;
    let c = f.context();
    assert!(!c.underline_enabled && c.underline_set_enabled);
    // The complete ribbon's horizontal ScrollArea needs its initial layout
    // pass before the newly selected tab emits its visible button shapes.
    draw(&mut f, vec![]);
    f.select(1, 4);
    let point = draw(&mut f, vec![]).expect("Actual U glyph after disposing output");
    let history = f.app.undo.len();
    let before = f.xml();
    let mut events = vec![egui::Event::PointerMoved(point)];
    for pressed in [true, false] {
        events.push(egui::Event::PointerButton {
            pos: point,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }
    draw(&mut f, events);
    assert_eq!(
        attr_on_text(&f.xml(), "CCL", "TextDecorations").as_deref(),
        Some("Underline")
    );
    assert_eq!(f.app.undo.len(), history + 1);
    f.app.undo();
    assert_eq!(f.xml(), before);
    assert!(f.app.last_executed_regions.is_empty());
}
