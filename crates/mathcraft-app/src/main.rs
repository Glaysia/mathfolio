use eframe::egui;
mod formula;
mod page;
mod ribbon;
mod typography;
use mathcraft_compat::editor_bridge as worksheet_math;
mod worksheet_ui;

fn main() -> eframe::Result {
    eframe::run_native(
        "Mathcraft",
        eframe::NativeOptions {
            renderer: if cfg!(target_os = "windows") {
                eframe::Renderer::Wgpu
            } else {
                eframe::Renderer::Glow
            },
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([1180., 900.])
                .with_min_inner_size([850., 600.]),
            ..Default::default()
        },
        Box::new(|cc| {
            configure(&cc.egui_ctx);
            let mut app = worksheet_ui::WorksheetApp::default();
            let args: Vec<_> = std::env::args().skip(1).collect();
            if let Some(i) = args.iter().position(|a| a == "--open")
                && let Some(path) = args.get(i + 1)
            {
                app.open(std::path::Path::new(path));
            }
            app.smoke = args.iter().any(|a| a == "--smoke-test");
            if let Some(i) = args.iter().position(|a| a == "--capture") {
                app.capture = args.get(i + 1).map(Into::into);
            }
            Ok(Box::new(app))
        }),
    )
}

fn configure(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::light();
    visuals.panel_fill = egui::Color32::from_rgb(248, 249, 251);
    visuals.window_fill = egui::Color32::from_rgb(255, 255, 255);
    visuals.selection.bg_fill = egui::Color32::from_rgb(220, 233, 250);
    visuals.selection.stroke.color = egui::Color32::from_rgb(36, 97, 173);
    ctx.set_visuals(visuals);
    ctx.global_style_mut(|style| {
        style.spacing.item_spacing = egui::vec2(4., 3.);
        style.spacing.button_padding = egui::vec2(5., 2.);
        style.spacing.interact_size.y = 22.;
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
    // Worksheet zoom is separate from accessibility / operating-system UI scale.
    ctx.options_mut(|options| options.zoom_with_keyboard = false);
    let mut fonts = egui::FontDefinitions::default();
    for path in [
        "C:\\Windows\\Fonts\\malgun.ttf",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/nanum/NanumGothic.ttf",
    ] {
        if let Ok(bytes) = std::fs::read(path) {
            fonts
                .font_data
                .insert("cjk".into(), egui::FontData::from_owned(bytes).into());
            for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
                fonts.families.entry(family).or_default().push("cjk".into());
            }
            break;
        }
    }
    for path in [
        "C:/Windows/Fonts/segoeui.ttf",
        "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
    ] {
        if let Ok(bytes) = std::fs::read(path) {
            fonts
                .font_data
                .insert("ui".into(), egui::FontData::from_owned(bytes).into());
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, "ui".into());
            break;
        }
    }
    typography::install(&mut fonts);
    ctx.set_fonts(fonts);
}
