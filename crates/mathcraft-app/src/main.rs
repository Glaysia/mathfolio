use eframe::egui;

fn main() -> eframe::Result {
    eframe::run_native(
        "Mathcraft",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_inner_size([960.0, 640.0]),
            ..Default::default()
        },
        Box::new(|_| Ok(Box::<MathcraftApp>::default())),
    )
}

#[derive(Default)]
struct MathcraftApp {
    text: String,
}

impl eframe::App for MathcraftApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui.heading("Mathcraft");
        ui.label("Worksheet prototype");
        ui.label("Type notes here. Calculation and document saving are not implemented yet.");
        ui.separator();
        ui.add(
            egui::TextEdit::multiline(&mut self.text)
                .hint_text("Start typing...")
                .desired_width(f32::INFINITY)
                .desired_rows(24),
        );
    }
}
