//! Shared Mathcraft editor surface and host adapters.
#[cfg(all(test, target_os = "windows"))]
mod graphics_adapter_probe;
use eframe::egui;
mod calculation_worker;
mod command_catalog;
mod theme;
pub use theme::{
    HostTheme, Theme, ThemeMode, has_host_theme, set_host_theme, set_theme, set_theme_mode,
    theme_mode,
};
mod formula;
#[cfg(test)]
#[path = "worksheet_ui/frame_profile.rs"]
mod frame_profile;
mod native_presentation;
mod page;
mod page_fields;
mod plots;
mod printing;
mod result_display;
mod ribbon;
mod typography;
use mathcraft_compat::editor_bridge as worksheet_math;
mod worksheet_ui;

#[cfg(not(target_arch = "wasm32"))]
pub fn run_native() -> eframe::Result {
    #[cfg(feature = "native-profile")]
    let profile = worksheet_ui::native_scroll_profile::Request::load();
    #[allow(unused_mut)]
    let mut options = eframe::NativeOptions {
        #[cfg(target_os = "windows")]
        renderer: eframe::Renderer::Wgpu,
        #[cfg(not(target_os = "windows"))]
        renderer: eframe::Renderer::Glow,
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1180., 900.])
            .with_min_inner_size([850., 600.]),
        ..Default::default()
    };
    #[cfg(windows)]
    {
        options.wgpu_options.windows_cpu_fallback = true;
    }
    #[cfg(feature = "native-profile")]
    if let Some(request) = &profile {
        request.configure(&mut options);
    }
    eframe::run_native(
        "Mathcraft",
        options,
        Box::new(move |cc| {
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
            #[cfg(feature = "native-profile")]
            if let Some(request) = &profile {
                app.open(&request.scene);
            }
            app.enable_background_calculation(cc.egui_ctx.clone());
            #[cfg(feature = "native-profile")]
            if let Some(request) = profile {
                return Ok(
                    Box::new(worksheet_ui::native_scroll_profile::ProfiledApp::new(
                        cc, app, request,
                    )) as Box<dyn eframe::App>,
                );
            }
            Ok(Box::new(app))
        }),
    )
}

#[cfg(not(target_arch = "wasm32"))]
mod native_fonts;

#[cfg(not(target_arch = "wasm32"))]
fn configure(ctx: &egui::Context) {
    configure_surface(ctx, native_fonts::load(), Theme::Light);
}

/// Configures a surface using supplied fonts, without OS IO. Initially follows
/// host tokens; `fallback_theme` is used until a host provides them. Reconfiguring
/// fonts preserves any explicit light/dark override selected on this surface.
pub fn configure_surface(
    ctx: &egui::Context,
    mut fonts: egui::FontDefinitions,
    fallback_theme: Theme,
) {
    theme::initialize(ctx, fallback_theme);
    // Worksheet zoom must not steal Ctrl+- (literal identifier subscript).
    ctx.options_mut(|options| options.zoom_with_keyboard = false);
    typography::install(&mut fonts);
    native_presentation::install_fonts(&mut fonts);
    ctx.set_fonts(fonts);
}
