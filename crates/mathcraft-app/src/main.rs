use eframe::egui::{self, Color32, RichText};
use mathcraft_core::{Environment, Quantity};
use mathcraft_document::{Row, Worksheet};
use std::path::PathBuf;

const BLUE: Color32 = Color32::from_rgb(34, 84, 155);
const GREEN: Color32 = Color32::from_rgb(27, 112, 86);
const RED: Color32 = Color32::from_rgb(174, 55, 62);

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
                .with_inner_size([1320., 880.])
                .with_min_inner_size([900., 600.]),
            ..Default::default()
        },
        Box::new(|cc| {
            configure(&cc.egui_ctx);
            let mut app = MathcraftApp::new();
            let args: Vec<_> = std::env::args().skip(1).collect();
            if let Some(index) = args.iter().position(|a| a == "--open")
                && let Some(path) = args.get(index + 1)
            {
                app.open(PathBuf::from(path));
            }
            app.smoke = args.iter().any(|a| a == "--smoke-test");
            if let Some(index) = args.iter().position(|a| a == "--capture") {
                app.capture = args.get(index + 1).map(PathBuf::from);
            }
            Ok(Box::new(app))
        }),
    )
}
fn configure(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::light();
    visuals.panel_fill = Color32::from_rgb(242, 245, 249);
    visuals.selection.bg_fill = Color32::from_rgb(186, 211, 244);
    ctx.set_visuals(visuals);
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
    ctx.set_fonts(fonts);
}

enum Action {
    New,
    Demo,
    Open(PathBuf),
    Quit,
}
struct MathcraftApp {
    worksheet: Worksheet,
    path: Option<PathBuf>,
    status: String,
    dirty: bool,
    undo: Vec<Worksheet>,
    redo: Vec<Worksheet>,
    pending: Option<Action>,
    quitting: bool,
    plot: String,
    xmin: f64,
    xmax: f64,
    smoke: bool,
    frames: usize,
    capture: Option<PathBuf>,
    capture_sent: bool,
}
impl MathcraftApp {
    fn new() -> Self {
        Self {
            worksheet: Worksheet::demo(),
            path: None,
            status: "Ready · independent Rust prototype".into(),
            dirty: false,
            undo: vec![],
            redo: vec![],
            pending: None,
            quitting: false,
            plot: "response(x)".into(),
            xmin: 0.,
            xmax: 20.,
            smoke: false,
            frames: 0,
            capture: None,
            capture_sent: false,
        }
    }
    fn remember(&mut self, previous: Worksheet) {
        if self.undo.len() >= 50 {
            self.undo.remove(0);
        }
        self.undo.push(previous);
        self.redo.clear();
        self.dirty = true;
    }
    fn request(&mut self, action: Action) {
        if self.dirty {
            self.pending = Some(action);
        } else {
            self.apply(action);
        }
    }
    fn apply(&mut self, action: Action) {
        match action {
            Action::New => {
                self.worksheet = Worksheet::empty();
                self.plot = "sin(x)".into();
                self.path = None;
                self.reset_history();
            }
            Action::Demo => {
                self.worksheet = Worksheet::demo();
                self.plot = "response(x)".into();
                self.path = None;
                self.reset_history();
            }
            Action::Open(path) => self.open(path),
            Action::Quit => self.quitting = true,
        }
    }
    fn reset_history(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.dirty = false;
    }
    fn open(&mut self, path: PathBuf) {
        match Worksheet::load(&path) {
            Ok(doc) => {
                self.worksheet = doc;
                self.plot = "sin(x)".into();
                self.reset_history();
                if path.extension().and_then(|s| s.to_str()) == Some("mathcraft") {
                    self.path = Some(path.clone());
                } else {
                    self.path = None;
                }
                self.status = format!(
                    "Opened {} · {} regions",
                    path.display(),
                    self.worksheet.rows.len()
                );
            }
            Err(e) => self.status = format!("Could not open: {e}"),
        }
    }
    fn save(&mut self, save_as: bool) -> bool {
        let path = if save_as || self.path.is_none() {
            rfd::FileDialog::new()
                .add_filter("Mathcraft", &["mathcraft"])
                .set_file_name("worksheet.mathcraft")
                .save_file()
        } else {
            self.path.clone()
        };
        let Some(path) = path else {
            return false;
        };
        match self.worksheet.save(&path) {
            Ok(()) => {
                self.status = format!("Saved {}", path.display());
                self.path = Some(path);
                self.dirty = false;
                true
            }
            Err(e) => {
                self.status = format!("Save failed: {e}");
                false
            }
        }
    }
    fn undo(&mut self) {
        if let Some(previous) = self.undo.pop() {
            self.redo.push(self.worksheet.clone());
            self.worksheet = previous;
            self.dirty = true;
        }
    }
    fn redo(&mut self) {
        if let Some(next) = self.redo.pop() {
            self.undo.push(self.worksheet.clone());
            self.worksheet = next;
            self.dirty = true;
        }
    }
    fn plot_panel(&mut self, ui: &mut egui::Ui, env: &Environment) {
        ui.add_space(16.);
        ui.heading("Explore");
        ui.label("Real values · x is dimensionless");
        ui.add_space(12.);
        ui.label("Expression");
        ui.add(
            egui::TextEdit::singleline(&mut self.plot)
                .font(egui::TextStyle::Monospace)
                .desired_width(f32::INFINITY)
                .char_limit(256),
        );
        ui.horizontal(|ui| {
            ui.label("x");
            ui.add(egui::DragValue::new(&mut self.xmin).speed(0.1));
            ui.label("to");
            ui.add(egui::DragValue::new(&mut self.xmax).speed(0.1));
        });
        let mut plot_env = env.clone();
        let mut points = Vec::new();
        let mut plot_error = None;
        if self.xmax > self.xmin && (self.xmax - self.xmin).is_finite() {
            for index in 0..=300 {
                let x = self.xmin + (self.xmax - self.xmin) * f64::from(index) / 300.;
                plot_env.variables.insert("x".into(), Quantity::scalar(x));
                match plot_env.evaluate(&self.plot) {
                    Ok(q) if q.is_scalar() && q.value.im.abs() < 1e-10 => {
                        points.push([x, q.value.re])
                    }
                    Ok(_) => {
                        plot_error = Some("Graph requires real, dimensionless values".into());
                        break;
                    }
                    Err(e) => {
                        plot_error = Some(e);
                        break;
                    }
                }
            }
        } else {
            plot_error = Some("Choose a finite increasing x range".into());
        }
        if let Some(e) = plot_error {
            ui.colored_label(RED, e);
        } else {
            egui_plot::Plot::new("function_plot")
                .height(220.)
                .show(ui, |plot_ui| {
                    plot_ui.line(
                        egui_plot::Line::new(&self.plot, points)
                            .color(BLUE)
                            .width(2.),
                    );
                });
        }
        ui.add_space(20.);
        ui.separator();
        ui.heading("Variables");
        let mut variables: Vec<_> = env.variables.iter().collect();
        variables.sort_by_key(|(name, _)| name.as_str());
        egui::ScrollArea::vertical()
            .id_salt("variables")
            .max_height(220.)
            .show(ui, |ui| {
                for (name, value) in variables {
                    ui.label(RichText::new(name).monospace().color(BLUE));
                    ui.label(RichText::new(value.to_string()).size(11.));
                    ui.add_space(5.);
                }
            });
        ui.add_space(16.);
        ui.separator();
        ui.heading("Quick reference");
        for text in [
            "a := 3 m",
            "a + 20 cm -> cm",
            "f(x) := sin(x)",
            "sqrt(4 m^2)",
            "(3 + 4i) ohm",
            "2*pi*360 kHz",
            "# a comment",
        ] {
            ui.monospace(text);
        }
        ui.add_space(12.);
        ui.small("Prototype semantics: rows calculate from top to bottom. Matrices, affine temperature and symbolic algebra are not supported yet.");
    }
}
impl eframe::App for MathcraftApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if let Some(path) = &self.capture {
            let shot = ctx.input(|i| {
                i.events.iter().find_map(|e| {
                    if let egui::Event::Screenshot { image, .. } = e {
                        Some(image.clone())
                    } else {
                        None
                    }
                })
            });
            if let Some(shot) = shot {
                let bytes: Vec<u8> = shot.pixels.iter().flat_map(|p| p.to_array()).collect();
                if let Some(image) =
                    image::RgbaImage::from_raw(shot.size[0] as u32, shot.size[1] as u32, bytes)
                {
                    match image.save(path) {
                        Ok(()) => println!("Captured {}", path.display()),
                        Err(e) => eprintln!("Capture failed: {e}"),
                    }
                }
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            if self.frames >= 3 && !self.capture_sent {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
                self.capture_sent = true;
            }
            ctx.request_repaint();
        }
        if ctx.input(|i| i.viewport().close_requested()) && self.dirty && !self.quitting {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.pending = Some(Action::Quit);
        }
        if self.quitting {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        ctx.input_mut(|i| {
            if i.consume_key(egui::Modifiers::COMMAND, egui::Key::S) {
                self.save(false);
            }
            if i.consume_key(egui::Modifiers::COMMAND, egui::Key::Z) {
                self.undo();
            }
            if i.consume_key(egui::Modifiers::COMMAND, egui::Key::Y) {
                self.redo();
            }
        });
        egui::Panel::top("toolbar").exact_size(58.).show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                ui.label(RichText::new("MATHCRAFT").strong().size(20.).color(BLUE));
                ui.separator();
                if ui.button("New").clicked() {
                    self.request(Action::New);
                }
                if ui.button("Demo").clicked() {
                    self.request(Action::Demo);
                }
                if ui.button("Open / Import").clicked()
                    && let Some(path) = rfd::FileDialog::new()
                        .add_filter("Worksheets", &["mathcraft", "mcdx"])
                        .pick_file()
                {
                    self.request(Action::Open(path));
                }
                if ui.button("Save").clicked() {
                    self.save(false);
                }
                if ui.button("Save as").clicked() {
                    self.save(true);
                }
                ui.separator();
                if ui
                    .add_enabled(!self.undo.is_empty(), egui::Button::new("Undo"))
                    .clicked()
                {
                    self.undo();
                }
                if ui
                    .add_enabled(!self.redo.is_empty(), egui::Button::new("Redo"))
                    .clicked()
                {
                    self.redo();
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new("RUST  /  0.1 PROTOTYPE")
                            .small()
                            .color(Color32::GRAY),
                    );
                });
            });
        });
        let (env, results) = self.worksheet.calculate();
        let error_count = results.iter().filter(|r| r.is_err()).count();
        egui::Panel::bottom("status")
            .exact_size(30.)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.colored_label(
                        if error_count == 0 { GREEN } else { RED },
                        format!(
                            "{} rows · {} errors{}",
                            self.worksheet.rows.len(),
                            error_count,
                            if self.dirty { " · unsaved" } else { "" }
                        ),
                    );
                    ui.separator();
                    ui.small(&self.status);
                });
            });
        egui::Panel::right("inspector")
            .default_size(305.)
            .min_size(245.)
            .resizable(true)
            .show(ui, |ui| self.plot_panel(ui, &env));
        egui::CentralPanel::default().show(ui,|ui| {
            let before=self.worksheet.clone();let mut changed=false;let mut remove=None;let mut movement=None;
            egui::ScrollArea::vertical().id_salt("worksheet").show(ui,|ui| {
                ui.add_space(18.);
                ui.horizontal(|ui|{ui.label(RichText::new("WORKSHEET").small().color(BLUE));if self.path.is_none() && self.worksheet.rows.iter().any(|r|r.imported.is_some()) {ui.small("MCDX imported read-only · save a .mathcraft copy");}});
                changed|=ui.add(egui::TextEdit::singleline(&mut self.worksheet.title).font(egui::TextStyle::Heading).frame(egui::Frame::NONE).desired_width(f32::INFINITY).char_limit(1024)).changed();
                ui.label(RichText::new("Edit expressions to recalculate. Use := to define and -> to choose a display unit.").color(Color32::GRAY));ui.add_space(16.);
                for (index,row) in self.worksheet.rows.iter_mut().enumerate() {
                    let error=results.get(index).is_some_and(Result::is_err);
                    egui::Frame::new().fill(if row.note {Color32::from_rgb(233,239,248)}else{Color32::WHITE}).stroke(egui::Stroke::new(1.,if error {Color32::from_rgb(226,168,172)}else{Color32::from_rgb(220,226,235)})).corner_radius(7).inner_margin(12).show(ui,|ui|{
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(format!("{:02}",index+1)).size(11.).color(Color32::GRAY));
                            if row.note {changed|=ui.add(egui::TextEdit::multiline(&mut row.input).frame(egui::Frame::NONE).desired_rows(1).desired_width((ui.available_width()-90.).max(120.)).char_limit(4096)).changed();}
                            else {changed|=ui.add(egui::TextEdit::singleline(&mut row.input).font(egui::TextStyle::Monospace).frame(egui::Frame::NONE).desired_width((ui.available_width()-90.).max(120.)).char_limit(4096).hint_text("Enter an expression...")).changed();}
                            if ui.small_button("↑").on_hover_text("Move up").clicked() && index>0 {movement=Some((index,index-1));}
                            if ui.small_button("↓").on_hover_text("Move down").clicked() {movement=Some((index,index+1));}
                            if ui.small_button("×").on_hover_text("Delete row").clicked() {remove=Some(index);}
                        });
                        if !row.note {
                            ui.horizontal(|ui| {
                                ui.add_space(28.);
                                match results.get(index) {
                                    Some(Ok(Some(value)))=>{ui.label(RichText::new(format!("= {}",value.text)).size(16.).color(GREEN));},
                                    Some(Err(error))=>{ui.colored_label(RED,error);},
                                    _=>{},
                                }
                            });
                            if let Some(imported)=&row.imported
                                && let Some(cached)=&imported.cached_display {ui.small(format!("Original cached value: {cached} · not a fresh Mathcad calculation"));}
                        }
                    });
                    ui.add_space(8.);
                }
                ui.add_space(8.);ui.horizontal(|ui| {
                    if ui.button("+ Expression").clicked() && self.worksheet.rows.len()<1000 {self.worksheet.rows.push(Row::math(""));changed=true;}
                    if ui.button("+ Note").clicked() && self.worksheet.rows.len()<1000 {self.worksheet.rows.push(Row {input:String::new(),note:true,imported:None});changed=true;}
                });ui.add_space(30.);
            });
            if let Some(index)=remove {self.worksheet.rows.remove(index);changed=true;}
            if let Some((a,b))=movement&& b<self.worksheet.rows.len() {self.worksheet.rows.swap(a,b);changed=true;}
            if changed {self.remember(before);ctx.request_repaint();}
        });
        if self.pending.is_some() {
            egui::Window::new("Unsaved changes")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0., 0.])
                .show(&ctx, |ui| {
                    ui.label("Save this worksheet before continuing?");
                    ui.horizontal(|ui| {
                        if ui.button("Save and continue").clicked()
                            && self.save(false)
                            && let Some(action) = self.pending.take()
                        {
                            self.apply(action);
                        }
                        if ui.button("Discard changes").clicked()
                            && let Some(action) = self.pending.take()
                        {
                            self.apply(action);
                        }
                        if ui.button("Cancel").clicked() {
                            self.pending = None;
                        }
                    });
                });
        }
        self.frames += 1;
        if self.smoke {
            if self.frames >= 3 {
                println!("Mathcraft GUI rendered successfully");
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            } else {
                ctx.request_repaint();
            }
        }
    }
}
