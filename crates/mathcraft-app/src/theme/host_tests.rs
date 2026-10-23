use super::*;

fn custom() -> HostTheme {
    let mut h = HostTheme::for_theme(Theme::Dark);
    h.workspace = [12, 34, 56, 255];
    h.panel = [20, 40, 60, 255];
    h.surface = [30, 50, 70, 255];
    h.text = [230, 220, 210, 255];
    h.muted_text = [150, 160, 170, 255];
    h.accent = [0, 220, 180, 255];
    h.button = [220, 200, 80, 255];
    h.button_text = [20, 25, 30, 255];
    h.high_contrast = true;
    h.font_size = 16.;
    h.corner_radius = 3;
    h
}

#[test]
fn follows_host_colours_button_contrast_and_high_contrast_focus() {
    let ctx = egui::Context::default();
    initialize(&ctx, Theme::Light);
    let h = custom();
    assert!(set_host_theme(&ctx, Some(h)).unwrap());
    assert_eq!(theme_mode(&ctx), ThemeMode::FollowHost);
    let p = palette(&ctx);
    assert_eq!(p.workspace.to_array(), h.workspace);
    assert_eq!(p.panel.to_array(), h.panel);
    assert_eq!(p.button.to_array(), h.button);
    assert_eq!(p.button_text.to_array(), h.button_text);
    let s = ctx.global_style();
    assert!(s.visuals.dark_mode);
    assert_eq!(s.visuals.text_color().to_array(), h.text);
    assert_eq!(s.visuals.weak_text_color().to_array(), h.muted_text);
    assert_eq!(
        s.visuals.widgets.hovered.bg_stroke.color.to_array(),
        h.focus
    );
    assert_eq!(s.visuals.widgets.hovered.bg_stroke.width, 1.5);
    assert_eq!(s.text_styles[&egui::TextStyle::Body].size, 16.);
}

#[test]
fn manual_override_retains_latest_host_and_restores_it_on_follow() {
    let ctx = egui::Context::default();
    initialize(&ctx, Theme::Light);
    set_host_theme(&ctx, Some(custom())).unwrap();
    set_theme_mode(&ctx, ThemeMode::Light);
    assert!(!ctx.global_style().visuals.dark_mode);
    let builtin = palette(&ctx).panel;
    let mut next = custom();
    next.panel = [1, 2, 3, 255];
    set_host_theme(&ctx, Some(next)).unwrap();
    assert_eq!(palette(&ctx).panel, builtin);
    initialize(&ctx, Theme::Dark); // Reinstalling fonts must not reset an override.
    assert_eq!(theme_mode(&ctx), ThemeMode::Light);
    set_theme_mode(&ctx, ThemeMode::FollowHost);
    assert_eq!(palette(&ctx).panel.to_array(), next.panel);
    set_host_theme(&ctx, None).unwrap();
    assert!(ctx.global_style().visuals.dark_mode); // Requested fallback, no stale host colours.
    assert_ne!(palette(&ctx).panel.to_array(), next.panel);
}

#[test]
fn invalid_updates_are_atomic_and_unchanged_updates_do_not_reset_custom_style() {
    let ctx = egui::Context::default();
    initialize(&ctx, Theme::Light);
    let h = custom();
    set_host_theme(&ctx, Some(h)).unwrap();
    ctx.global_style_mut(|s| s.spacing.item_spacing.x = 42.);
    assert!(!set_host_theme(&ctx, Some(h)).unwrap());
    assert_eq!(ctx.global_style().spacing.item_spacing.x, 42.);
    for invalid in [
        HostTheme {
            font_size: f32::NAN,
            ..h
        },
        HostTheme {
            panel: [0, 0, 0, 0],
            ..h
        },
        HostTheme {
            corner_radius: 255,
            ..h
        },
    ] {
        assert!(set_host_theme(&ctx, Some(invalid)).is_err());
        assert_eq!(palette(&ctx).panel.to_array(), h.panel);
    }
}

#[test]
fn semantic_theme_is_serializable_and_isolated_per_surface() {
    let first = egui::Context::default();
    let second = egui::Context::default();
    initialize(&first, Theme::Light);
    initialize(&second, Theme::Light);
    let h = custom();
    let decoded: HostTheme = serde_json::from_slice(&serde_json::to_vec(&h).unwrap()).unwrap();
    assert_eq!(decoded, h);
    set_host_theme(&first, Some(decoded)).unwrap();
    assert!(!has_host_theme(&second));
    assert_ne!(palette(&first).panel, palette(&second).panel);
    assert_eq!(
        serde_json::to_string(&ThemeMode::FollowHost).unwrap(),
        "\"follow_host\""
    );
}

#[test]
fn fallback_tokens_preserve_translucent_text_instead_of_multiplying_alpha_twice() {
    for theme in [Theme::Light, Theme::Dark] {
        let ctx = egui::Context::default();
        initialize(&ctx, theme);
        let expected = match theme {
            Theme::Light => egui::Visuals::light(),
            Theme::Dark => egui::Visuals::dark(),
        }
        .weak_text_color();
        set_host_theme(&ctx, Some(HostTheme::for_theme(theme))).unwrap();
        let actual = ctx.global_style().visuals.weak_text_color();
        for (a, b) in actual.to_array().into_iter().zip(expected.to_array()) {
            assert!(
                a.abs_diff(b) <= 1,
                "host token conversion must preserve default text opacity: {actual:?} vs {expected:?}"
            );
        }
    }
}

#[test]
fn nord_preset_works_without_a_host_and_host_following_remains_independent() {
    let ctx = egui::Context::default();
    initialize(&ctx, Theme::Light);
    set_theme_mode(&ctx, ThemeMode::Nord);
    assert!(!has_host_theme(&ctx));
    assert_eq!(palette(&ctx).workspace.to_array(), [0x2e, 0x34, 0x40, 255]);
    assert_eq!(palette(&ctx).button.to_array(), [0x88, 0xc0, 0xd0, 255]);
    assert!(ctx.global_style().visuals.dark_mode);
    set_host_theme(&ctx, Some(custom())).unwrap();
    assert_eq!(palette(&ctx).workspace.to_array(), [0x2e, 0x34, 0x40, 255]);
    set_theme_mode(&ctx, ThemeMode::FollowHost);
    assert_eq!(palette(&ctx).workspace.to_array(), custom().workspace);
    assert_eq!(serde_json::to_string(&ThemeMode::Nord).unwrap(), "\"nord\"");
}
