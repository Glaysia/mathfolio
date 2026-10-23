//! Application-level input isolation and host-independent chrome regressions.
use super::*;

fn frame(app: &mut WorksheetApp, ctx: &egui::Context, events: Vec<egui::Event>) {
    ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1180., 900.))),
            events,
            ..Default::default()
        },
        |ui| app.app_ui_frame(ui),
    )
    .drop_without_applying_deltas();
}

fn key(key: egui::Key, modifiers: egui::Modifiers) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers,
    }
}

fn fixture() -> (WorksheetApp, egui::Context, u64) {
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    let mut app = WorksheetApp {
        automatic_calculation: false,
        ..Default::default()
    };
    let id = app.insert((38.4, 38.4), false);
    frame(&mut app, &ctx, vec![]);
    frame(&mut app, &ctx, vec![egui::Event::Text("alpha".into())]);
    (app, ctx, id)
}

#[test]
fn help_search_and_shortcuts_do_not_edit_commit_calculate_or_create_regions() {
    let (mut app, ctx, id) = fixture();
    let source = serde_json::to_vec(&app.document).unwrap();
    let before = app.editors[&id].to_portable().unwrap();
    let undo = app.undo.len();
    let generation = app.background.submitted_generation();
    frame(
        &mut app,
        &ctx,
        vec![key(egui::Key::F1, egui::Modifiers::NONE)],
    );
    assert!(app.show_shortcuts);
    frame(&mut app, &ctx, vec![]);
    frame(&mut app, &ctx, vec![egui::Event::Text("아래첨자".into())]);
    assert_eq!(app.ribbon.help_query, "아래첨자");
    for (code, modifiers) in [
        (egui::Key::T, egui::Modifiers::COMMAND),
        (egui::Key::Z, egui::Modifiers::COMMAND),
        (egui::Key::S, egui::Modifiers::COMMAND),
        (egui::Key::F5, egui::Modifiers::NONE),
        (egui::Key::Enter, egui::Modifiers::NONE),
    ] {
        frame(&mut app, &ctx, vec![key(code, modifiers)]);
    }
    assert_eq!(serde_json::to_vec(&app.document).unwrap(), source);
    assert_eq!(app.editors[&id].to_portable().unwrap(), before);
    assert_eq!(app.undo.len(), undo);
    assert_eq!(app.background.submitted_generation(), generation);
    assert!(app.pending.is_none());
    frame(
        &mut app,
        &ctx,
        vec![key(egui::Key::Escape, egui::Modifiers::NONE)],
    );
    assert!(!app.show_shortcuts);
    frame(&mut app, &ctx, vec![egui::Event::Text("x".into())]);
    assert_ne!(app.editors[&id].to_portable().unwrap(), before);
}

#[test]
fn themes_inspector_and_all_tools_leave_document_and_calculation_unchanged() {
    let (mut app, ctx, _) = fixture();
    app.commit_math_edits();
    let source = serde_json::to_vec(&app.document).unwrap();
    let undo = app.undo.len();
    let generation = app.background.submitted_generation();
    for theme in [crate::Theme::Dark, crate::Theme::Light] {
        crate::set_theme(&ctx, theme);
        app.ribbon.inspector_open = true;
        app.ribbon.tools_open = true;
        frame(&mut app, &ctx, vec![]);
        app.ribbon.inspector_open = false;
        app.ribbon.tools_open = false;
        frame(&mut app, &ctx, vec![]);
    }
    assert_eq!(serde_json::to_vec(&app.document).unwrap(), source);
    assert_eq!(app.undo.len(), undo);
    assert_eq!(app.background.submitted_generation(), generation);
    assert!(!ctx.options(|o| o.zoom_with_keyboard));
}

#[test]
fn command_search_preserves_existing_shortcut_meanings() {
    let subscript = crate::command_catalog::find(RibbonAction::LiteralSubscript).unwrap();
    assert!(subscript.matches("아래첨자"));
    assert!(subscript.matches("Ctrl+-"));
    let redo = crate::command_catalog::find(RibbonAction::Redo).unwrap();
    assert_eq!(redo.keys, "Ctrl+Y");
    assert_eq!(
        crate::command_catalog::find(RibbonAction::CalculateNow)
            .unwrap()
            .keys,
        "F5"
    );
    assert_eq!(
        crate::command_catalog::find(RibbonAction::Recalculate)
            .unwrap()
            .keys,
        "Ctrl+F5"
    );
}

#[test]
fn inspector_numeric_draft_waits_for_enter_and_owns_shortcuts() {
    let (mut app, ctx, id) = fixture();
    app.commit_math_edits();
    app.ribbon.inspector_open = true;
    frame(&mut app, &ctx, vec![]);
    let precision_input = app.ribbon.input_ids[1];
    let source = serde_json::to_vec(&app.document).unwrap();
    let count = app.document.regions.len();
    ctx.memory_mut(|m| m.request_focus(precision_input));
    frame(&mut app, &ctx, vec![]);
    frame(
        &mut app,
        &ctx,
        vec![key(egui::Key::A, egui::Modifiers::COMMAND)],
    );
    frame(&mut app, &ctx, vec![egui::Event::Text("7".into())]);
    assert_eq!(serde_json::to_vec(&app.document).unwrap(), source);
    frame(
        &mut app,
        &ctx,
        vec![key(egui::Key::T, egui::Modifiers::COMMAND)],
    );
    assert_eq!(app.document.regions.len(), count);
    frame(
        &mut app,
        &ctx,
        vec![key(egui::Key::Enter, egui::Modifiers::NONE)],
    );
    assert_eq!(
        app.document
            .regions
            .iter()
            .find(|r| r.id == id)
            .unwrap()
            .format
            .precision,
        7
    );
    assert_eq!(app.selected, Some(id));
    assert_eq!(app.document.regions.len(), count);
}

#[test]
fn embedded_width_keeps_header_actions_inside_the_viewport() {
    fn locate(shape: &egui::Shape, labels: &mut Vec<(String, Rect)>) {
        match shape {
            egui::Shape::Text(text) => labels.push((
                text.galley.text().to_owned(),
                Rect::from_min_size(text.pos, text.galley.size()),
            )),
            egui::Shape::Vec(shapes) => {
                for shape in shapes {
                    locate(shape, labels);
                }
            }
            _ => {}
        }
    }
    for width in [350., 600., 1180.] {
        let ctx = egui::Context::default();
        crate::configure_surface(&ctx, crate::native_fonts::load(), crate::Theme::Light);
        let mut app = WorksheetApp::default();
        for pass in 0..3 {
            let output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(width, 800.))),
                    ..Default::default()
                },
                |ui| app.app_ui_frame(ui),
            );
            let mut labels = Vec::new();
            for clipped in &output.shapes {
                locate(&clipped.shape, &mut labels);
            }
            output.drop_without_applying_deltas();
            for title in [
                "파일",
                "저장",
                if width < 644. {
                    "명령"
                } else {
                    "명령 검색"
                },
                "속성",
            ] {
                if let Some((_, rect)) = labels.iter().find(|(text, _)| text == title) {
                    assert!(
                        rect.min.x >= 0. && rect.max.x <= width,
                        "{title} at {width}: {rect:?}"
                    );
                } else if pass == 2 {
                    panic!("missing header action {title} at width {width}");
                }
            }
        }
    }
}

#[test]
fn live_host_theme_does_not_commit_a_formula_or_change_document_colours() {
    let (mut app, ctx, id) = fixture();
    let document = app.document.clone();
    let editor = app.editors[&id].to_portable().unwrap();
    let history = app.undo.len();
    let generation = app.background.submitted_generation();
    let zoom = app.zoom;
    let mut host = crate::HostTheme::for_theme(crate::Theme::Dark);
    host.workspace = [10, 25, 35, 255];
    host.accent = [20, 220, 160, 255];
    for mode in [
        crate::ThemeMode::FollowHost,
        crate::ThemeMode::Light,
        crate::ThemeMode::Dark,
        crate::ThemeMode::Nord,
        crate::ThemeMode::FollowHost,
    ] {
        crate::set_host_theme(&ctx, Some(host)).unwrap();
        crate::set_theme_mode(&ctx, mode);
        frame(&mut app, &ctx, vec![]);
        host.panel[0] += 1;
    }
    assert_eq!(app.document, document);
    assert_eq!(app.editors[&id].to_portable().unwrap(), editor);
    assert_eq!(app.undo.len(), history);
    assert_eq!(app.background.submitted_generation(), generation);
    assert_eq!(app.zoom, zoom);
    assert_eq!(
        crate::theme::palette(&ctx).workspace.to_array(),
        host.workspace
    );
}

#[test]
fn host_button_tokens_reach_the_actual_calculate_button() {
    fn scan(shape: &egui::Shape, h: &crate::HostTheme, fill: &mut bool, text: &mut bool) {
        match shape {
            egui::Shape::Rect(rect) => *fill |= rect.fill.to_array() == h.button,
            egui::Shape::Text(t) if t.galley.text() == "계산  F5" => {
                *text |= t
                    .galley
                    .job
                    .sections
                    .iter()
                    .any(|s| s.format.color.to_array() == h.button_text);
            }
            egui::Shape::Vec(shapes) => {
                for shape in shapes {
                    scan(shape, h, fill, text);
                }
            }
            _ => {}
        }
    }
    let (mut app, ctx, _) = fixture();
    let mut h = crate::HostTheme::for_theme(crate::Theme::Dark);
    h.button = [240, 210, 30, 255];
    h.button_text = [20, 30, 40, 255];
    crate::set_host_theme(&ctx, Some(h)).unwrap();
    let (mut fill, mut text) = (false, false);
    for _ in 0..3 {
        let output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1180., 900.))),
                ..Default::default()
            },
            |ui| app.app_ui_frame(ui),
        );
        for s in &output.shapes {
            scan(&s.shape, &h, &mut fill, &mut text);
        }
        output.drop_without_applying_deltas();
    }
    assert!(fill, "primary button must use host background");
    assert!(
        text,
        "primary button must use host foreground rather than fixed white"
    );
}
