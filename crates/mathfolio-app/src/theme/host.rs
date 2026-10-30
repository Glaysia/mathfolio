//! Semantic host tokens, deliberately independent of CSS/DOM and plugin APIs.
use super::{Theme, apply_theme};
use eframe::egui::{self, Color32};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeMode {
    #[default]
    FollowHost,
    Light,
    Dark,
    Nord,
}

/// RGBA sRGB values resolved by the host, not CSS strings. Base backgrounds
/// must be opaque (resolve CSS transparency against the container first).
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct HostTheme {
    pub dark: bool,
    pub high_contrast: bool,
    pub workspace: [u8; 4],
    pub panel: [u8; 4],
    pub surface: [u8; 4],
    pub border: [u8; 4],
    pub text: [u8; 4],
    pub muted_text: [u8; 4],
    pub accent: [u8; 4],
    pub button: [u8; 4],
    pub button_text: [u8; 4],
    pub hover: [u8; 4],
    pub selection: [u8; 4],
    pub selection_text: [u8; 4],
    pub focus: [u8; 4],
    pub error: [u8; 4],
    pub warning: [u8; 4],
    /// UI logical pixels; never changes worksheet font sizes or print layout.
    pub font_size: f32,
    pub corner_radius: u8,
}

impl HostTheme {
    /// Nord's published palette; semantic placement is Mathfolio's own mapping.
    /// https://www.nordtheme.com/docs/colors-and-palettes/
    pub fn nord() -> Self {
        Self {
            workspace: [0x2e, 0x34, 0x40, 255],
            panel: [0x3b, 0x42, 0x52, 255],
            surface: [0x43, 0x4c, 0x5e, 255],
            border: [0x4c, 0x56, 0x6a, 255],
            text: [0xec, 0xef, 0xf4, 255],
            muted_text: [0xd8, 0xde, 0xe9, 255],
            accent: [0x88, 0xc0, 0xd0, 255],
            button: [0x88, 0xc0, 0xd0, 255],
            button_text: [0x2e, 0x34, 0x40, 255],
            hover: [0x43, 0x4c, 0x5e, 255],
            selection: [0x43, 0x4c, 0x5e, 255],
            selection_text: [0xec, 0xef, 0xf4, 255],
            focus: [0x88, 0xc0, 0xd0, 255],
            error: [0xbf, 0x61, 0x6a, 255],
            warning: [0xeb, 0xcb, 0x8b, 255],
            ..Self::for_theme(Theme::Dark)
        }
    }

    /// A complete fallback token set; adapters override the tokens they receive.
    pub fn for_theme(theme: Theme) -> Self {
        let dark = theme == Theme::Dark;
        let v = if dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        };
        let rgb = |r, g, b| [r, g, b, 255];
        Self {
            dark,
            high_contrast: false,
            workspace: if dark {
                rgb(24, 26, 33)
            } else {
                rgb(241, 242, 247)
            },
            panel: if dark {
                rgb(34, 36, 45)
            } else {
                rgb(255, 255, 255)
            },
            surface: if dark {
                rgb(43, 45, 56)
            } else {
                rgb(247, 247, 252)
            },
            border: if dark {
                rgb(65, 68, 82)
            } else {
                rgb(225, 226, 236)
            },
            text: v.text_color().to_srgba_unmultiplied(),
            muted_text: v.weak_text_color().to_srgba_unmultiplied(),
            accent: if dark {
                rgb(158, 147, 255)
            } else {
                rgb(101, 83, 207)
            },
            button: rgb(101, 83, 207),
            button_text: rgb(255, 255, 255),
            hover: v.widgets.hovered.weak_bg_fill.to_srgba_unmultiplied(),
            selection: if dark {
                rgb(75, 65, 128)
            } else {
                rgb(231, 226, 255)
            },
            selection_text: if dark {
                rgb(238, 236, 255)
            } else {
                rgb(64, 48, 146)
            },
            focus: if dark {
                rgb(158, 147, 255)
            } else {
                rgb(101, 83, 207)
            },
            error: v.error_fg_color.to_srgba_unmultiplied(),
            warning: v.warn_fg_color.to_srgba_unmultiplied(),
            font_size: 13.333333,
            corner_radius: 6,
        }
    }

    fn validate(self) -> Result<(), &'static str> {
        if !self.font_size.is_finite() || !(10. ..=24.).contains(&self.font_size) {
            return Err("UI font size must be finite and between 10 and 24 logical pixels");
        }
        if self.corner_radius > 16 {
            return Err("UI corner radius must be at most 16");
        }
        if [self.workspace, self.panel, self.surface, self.button]
            .iter()
            .any(|c| c[3] != 255)
        {
            return Err(
                "Resolve host base background colours to opaque RGBA before supplying them",
            );
        }
        Ok(())
    }
}

#[derive(Clone, Copy, PartialEq)]
struct State {
    mode: ThemeMode,
    host: Option<HostTheme>,
    fallback: Theme,
}

fn key() -> egui::Id {
    egui::Id::new("mathfolio-host-theme-v1")
}
fn state(ctx: &egui::Context) -> State {
    ctx.data(|d| d.get_temp::<State>(key())).unwrap_or(State {
        mode: ThemeMode::FollowHost,
        host: None,
        fallback: if ctx.global_style().visuals.dark_mode {
            Theme::Dark
        } else {
            Theme::Light
        },
    })
}

pub fn theme_mode(ctx: &egui::Context) -> ThemeMode {
    state(ctx).mode
}
pub fn has_host_theme(ctx: &egui::Context) -> bool {
    state(ctx).host.is_some()
}

pub(super) fn active_theme(ctx: &egui::Context) -> Option<HostTheme> {
    selected_tokens(state(ctx))
}

fn selected_tokens(s: State) -> Option<HostTheme> {
    match s.mode {
        ThemeMode::FollowHost => s.host,
        ThemeMode::Nord => Some(HostTheme::nord()),
        ThemeMode::Light | ThemeMode::Dark => None,
    }
}

pub(crate) fn initialize(ctx: &egui::Context, fallback: Theme) {
    let mut s = state(ctx);
    s.fallback = fallback;
    ctx.data_mut(|d| d.insert_temp(key(), s));
    apply(ctx, s);
}

/// Stores the latest host theme. Manual light/dark overrides are preserved.
/// Identical updates do not restyle the surface or request another frame.
pub fn set_host_theme(ctx: &egui::Context, host: Option<HostTheme>) -> Result<bool, &'static str> {
    if let Some(theme) = host {
        theme.validate()?;
    }
    let mut s = state(ctx);
    if s.host == host {
        return Ok(false);
    }
    s.host = host;
    ctx.data_mut(|d| d.insert_temp(key(), s));
    if s.mode == ThemeMode::FollowHost {
        apply(ctx, s);
        ctx.request_repaint();
    }
    Ok(true)
}

pub fn set_theme_mode(ctx: &egui::Context, mode: ThemeMode) {
    let mut s = state(ctx);
    if s.mode == mode {
        return;
    }
    s.mode = mode;
    ctx.data_mut(|d| d.insert_temp(key(), s));
    apply(ctx, s);
    ctx.request_repaint();
}

/// Compatibility API for an explicit built-in theme selection.
pub fn set_theme(ctx: &egui::Context, theme: Theme) {
    let mut s = state(ctx);
    s.mode = match theme {
        Theme::Light => ThemeMode::Light,
        Theme::Dark => ThemeMode::Dark,
    };
    ctx.data_mut(|d| d.insert_temp(key(), s));
    apply(ctx, s);
}

pub(super) fn color(c: [u8; 4]) -> Color32 {
    Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3])
}

fn apply(ctx: &egui::Context, s: State) {
    let host = selected_tokens(s);
    let base = match s.mode {
        ThemeMode::Light => Theme::Light,
        ThemeMode::Dark => Theme::Dark,
        ThemeMode::Nord => Theme::Dark,
        ThemeMode::FollowHost => {
            host.map_or(
                s.fallback,
                |h| if h.dark { Theme::Dark } else { Theme::Light },
            )
        }
    };
    apply_theme(ctx, base);
    let Some(h) = host else {
        return;
    };
    ctx.global_style_mut(|style| {
        let v = &mut style.visuals;
        // Explicit rich-text colours (including worksheet source) remain explicit.
        v.override_text_color = None;
        v.weak_text_color = Some(color(h.muted_text));
        v.faint_bg_color = color(h.surface);
        v.extreme_bg_color = color(h.surface);
        v.text_edit_bg_color = Some(color(h.surface));
        v.selection.bg_fill = color(h.selection);
        v.selection.stroke = egui::Stroke::new(
            if h.high_contrast { 2. } else { 1. },
            color(h.selection_text),
        );
        v.hyperlink_color = color(h.accent);
        v.warn_fg_color = color(h.warning);
        v.error_fg_color = color(h.error);
        v.text_cursor.stroke.color = color(h.text);
        v.window_corner_radius = egui::CornerRadius::same(h.corner_radius);
        for widget in [
            &mut v.widgets.noninteractive,
            &mut v.widgets.inactive,
            &mut v.widgets.hovered,
            &mut v.widgets.active,
            &mut v.widgets.open,
        ] {
            widget.fg_stroke.color = color(h.text);
            widget.bg_stroke =
                egui::Stroke::new(if h.high_contrast { 1.5 } else { 1. }, color(h.border));
            widget.corner_radius = egui::CornerRadius::same(h.corner_radius);
        }
        v.widgets.noninteractive.bg_fill = color(h.surface);
        v.widgets.noninteractive.weak_bg_fill = color(h.surface);
        v.widgets.inactive.bg_fill = color(h.surface);
        for widget in [
            &mut v.widgets.hovered,
            &mut v.widgets.active,
            &mut v.widgets.open,
        ] {
            widget.bg_fill = color(h.hover);
            widget.weak_bg_fill = color(h.hover);
            widget.bg_stroke.color = color(h.focus);
        }
        for text_style in [egui::TextStyle::Body, egui::TextStyle::Button] {
            style
                .text_styles
                .insert(text_style, egui::FontId::proportional(h.font_size));
        }
        style.text_styles.insert(
            egui::TextStyle::Small,
            egui::FontId::proportional((h.font_size * 0.9).max(10.)),
        );
        style.text_styles.insert(
            egui::TextStyle::Heading,
            egui::FontId::proportional(h.font_size * 1.35),
        );
    });
}
