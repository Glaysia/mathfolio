//! Host-independent chrome styling. Worksheet source colours stay in the document.
use eframe::egui::{self, Color32};
mod host;
pub(crate) use host::initialize;
pub use host::{
    HostTheme, ThemeMode, has_host_theme, set_host_theme, set_theme, set_theme_mode, theme_mode,
};
#[cfg(test)]
mod host_tests;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Theme {
    #[default]
    Light,
    Dark,
}

pub(crate) struct Palette {
    pub workspace: Color32,
    pub panel: Color32,
    pub surface: Color32,
    pub edge: Color32,
    pub accent: Color32,
    pub button: Color32,
    pub button_text: Color32,
}

pub(crate) fn palette(ctx: &egui::Context) -> Palette {
    if let Some(h) = host::active_theme(ctx) {
        return Palette {
            workspace: host::color(h.workspace),
            panel: host::color(h.panel),
            surface: host::color(h.surface),
            edge: host::color(h.border),
            accent: host::color(h.accent),
            button: host::color(h.button),
            button_text: host::color(h.button_text),
        };
    }
    if ctx.global_style().visuals.dark_mode {
        Palette {
            workspace: Color32::from_rgb(24, 26, 33),
            panel: Color32::from_rgb(34, 36, 45),
            surface: Color32::from_rgb(43, 45, 56),
            edge: Color32::from_rgb(65, 68, 82),
            accent: Color32::from_rgb(158, 147, 255),
            button: Color32::from_rgb(101, 83, 207),
            button_text: Color32::WHITE,
        }
    } else {
        Palette {
            workspace: Color32::from_rgb(241, 242, 247),
            panel: Color32::WHITE,
            surface: Color32::from_rgb(247, 247, 252),
            edge: Color32::from_rgb(225, 226, 236),
            accent: Color32::from_rgb(101, 83, 207),
            button: Color32::from_rgb(101, 83, 207),
            button_text: Color32::WHITE,
        }
    }
}

/// Applies only UI style, without loading files, creating windows or changing document state.
fn apply_theme(ctx: &egui::Context, theme: Theme) {
    ctx.set_visuals(match theme {
        Theme::Light => egui::Visuals::light(),
        Theme::Dark => egui::Visuals::dark(),
    });
    let p = palette(ctx);
    ctx.global_style_mut(|style| {
        style.visuals.panel_fill = p.panel;
        style.visuals.window_fill = p.panel;
        style.visuals.window_stroke = egui::Stroke::new(1., p.edge);
        style.visuals.window_corner_radius = egui::CornerRadius::same(12);
        style.visuals.selection.bg_fill = if theme == Theme::Dark {
            Color32::from_rgb(75, 65, 128)
        } else {
            Color32::from_rgb(231, 226, 255)
        };
        style.visuals.selection.stroke.color = p.accent;
        style.visuals.hyperlink_color = p.accent;
        style.visuals.widgets.inactive.weak_bg_fill = p.surface;
        style.visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1., p.edge);
        for widget in [
            &mut style.visuals.widgets.noninteractive,
            &mut style.visuals.widgets.inactive,
            &mut style.visuals.widgets.hovered,
            &mut style.visuals.widgets.active,
            &mut style.visuals.widgets.open,
        ] {
            widget.corner_radius = egui::CornerRadius::same(6);
        }
        style.spacing.item_spacing = egui::vec2(7., 6.);
        style.spacing.button_padding = egui::vec2(9., 5.);
        style.spacing.interact_size.y = 28.;
        style
            .text_styles
            .insert(egui::TextStyle::Body, egui::FontId::proportional(13.333333));
        style.text_styles.insert(
            egui::TextStyle::Button,
            egui::FontId::proportional(13.333333),
        );
        style
            .text_styles
            .insert(egui::TextStyle::Small, egui::FontId::proportional(12.));
    });
}
