//! Print existing worksheet presentation without running the calculator.
use super::*;
#[cfg(not(target_arch = "wasm32"))]
use crate::printing::NativePrintOptions;
use crate::printing::{self, PageSelection, PrintPlan, PrintSettings};
use mathcraft_document::canvas::{PageSetup, PaperSize};
#[cfg(not(target_arch = "wasm32"))]
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, TryRecvError};

#[derive(Clone, Copy, Default, PartialEq)]
enum Selection {
    #[default]
    All,
    Current,
    Range,
    Selected,
}

pub(super) struct PrintUiState {
    pub(super) open: bool,
    pub(super) current_page: usize,
    pdf: bool,
    selection: Selection,
    range: String,
    copies: u16,
    dpi: u16,
    printers: Vec<String>,
    printer: Option<String>,
    query: Option<Receiver<Result<Vec<String>, String>>>,
    job: Option<Receiver<Result<String, String>>>,
    setup: Option<PageSetup>,
}
impl Default for PrintUiState {
    fn default() -> Self {
        Self {
            open: false,
            current_page: 0,
            pdf: false,
            selection: Selection::All,
            range: "1".into(),
            copies: 1,
            dpi: 300,
            printers: vec![],
            printer: None,
            query: None,
            job: None,
            setup: None,
        }
    }
}
#[cfg(not(target_arch = "wasm32"))]
fn parse_range(text: &str) -> Result<PageSelection, String> {
    let parts = text.trim().split('-').collect::<Vec<_>>();
    let parse = |s: &str| {
        s.trim()
            .parse::<usize>()
            .ok()
            .filter(|n| *n > 0)
            .ok_or_else(|| "페이지 번호 또는 1-3 형태의 범위를 입력하세요.".to_string())
    };
    let (first, last) = match parts.as_slice() {
        [one] => {
            let n = parse(one)?;
            (n, n)
        }
        [a, b] => (parse(a)?, parse(b)?),
        _ => return Err("페이지 번호 또는 1-3 형태의 범위를 입력하세요.".into()),
    };
    if first > last {
        return Err("페이지 범위의 시작이 끝보다 큽니다.".into());
    }
    Ok(PageSelection::Range { first, last })
}

impl WorksheetApp {
    /// Headless test packet producer using the same snapshot/vector writer.
    #[cfg(test)]
    pub(super) fn export_vector_pdf_for_test(&mut self, path: &Path) -> Result<(), String> {
        let mut snapshot = self.print_snapshot();
        let plan = snapshot.plan(PageSelection::All, 300)?;
        let pages = printing::render_vector_pages(&plan, |ui, page, total| {
            snapshot.paint(ui, page, total)
        })?;
        printing::write_pdf(path, &pages)
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn open_print(&mut self, pdf: bool) {
        if let Err(error) = self.commit_table_edits(None) {
            self.status = error;
            return;
        }
        self.print_ui.open = true;
        self.print_ui.pdf = pdf;
        if !pdf && self.print_ui.query.is_none() {
            let (send, receive) = mpsc::channel();
            self.print_ui.query = Some(receive);
            std::thread::spawn(move || {
                let _ = send.send(printing::list_printers());
            });
        }
    }
    pub(super) fn open_page_setup(&mut self) {
        let landscape = self.document.width > self.document.height;
        let paper = PaperSize::ALL
            .into_iter()
            .find(|p| {
                let s = p.size(landscape);
                (s[0] - self.document.width).abs() < 1. && (s[1] - self.document.height).abs() < 1.
            })
            .unwrap_or(PaperSize::A4);
        self.print_ui.setup = Some(self.document.page_setup.clone().unwrap_or(PageSetup {
            paper,
            landscape,
            margins: self.page_margins.map(f64::from),
        }));
    }
    fn commit_page_setup(&mut self, setup: PageSetup) -> Result<(), String> {
        setup.validate()?;
        if self.document.page_setup.as_ref() == Some(&setup) {
            return Ok(());
        }
        let before = self.snapshot();
        let [width, height] = setup.paper.size(setup.landscape);
        self.document.width = width;
        self.document.height = height;
        let fine = self.document.grid < 12.;
        let us = matches!(
            setup.paper,
            PaperSize::Letter | PaperSize::Legal | PaperSize::Ledger
        );
        self.document.grid =
            if us { 0.2 * 96. } else { 0.5 * 96. / 2.54 } * if fine { 0.5 } else { 1. };
        self.document.page_setup = Some(setup);
        self.region_heights.clear();
        self.region_widths.clear();
        self.remember(before);
        self.plot_signature = self.document.active_calculation_signature();
        Ok(())
    }
    pub(super) fn printing_ui(&mut self, ctx: &egui::Context) {
        if let Some(receiver) = &self.print_ui.query {
            match receiver.try_recv() {
                Ok(result) => {
                    match result {
                        Ok(names) => self.print_ui.printers = names,
                        Err(e) => self.status = e,
                    };
                    self.print_ui.query = None;
                }
                Err(TryRecvError::Disconnected) => self.print_ui.query = None,
                Err(TryRecvError::Empty) => {
                    ctx.request_repaint_after(std::time::Duration::from_millis(100))
                }
            }
        }
        if let Some(receiver) = &self.print_ui.job {
            match receiver.try_recv() {
                Ok(result) => {
                    self.status = result.unwrap_or_else(|e| format!("인쇄 실패: {e}"));
                    self.print_ui.job = None;
                }
                Err(TryRecvError::Disconnected) => {
                    self.status = "인쇄 작업이 완료되지 못했습니다.".into();
                    self.print_ui.job = None;
                }
                Err(TryRecvError::Empty) => {
                    ctx.request_repaint_after(std::time::Duration::from_millis(100))
                }
            }
        }
        let mut open = self.print_ui.open;
        let mut submit = false;
        if open {
            egui::Window::new(if self.print_ui.pdf {
                "PDF로 저장"
            } else {
                "인쇄"
            })
            .id(egui::Id::new("print-settings"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                if !self.print_ui.pdf {
                    ui.horizontal(|ui| {
                        ui.label("프린터");
                        egui::ComboBox::from_id_salt("printer-name")
                            .selected_text(
                                self.print_ui
                                    .printer
                                    .as_deref()
                                    .unwrap_or("시스템 기본 프린터"),
                            )
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut self.print_ui.printer,
                                    None,
                                    "시스템 기본 프린터",
                                );
                                for n in &self.print_ui.printers {
                                    ui.selectable_value(
                                        &mut self.print_ui.printer,
                                        Some(n.clone()),
                                        n,
                                    );
                                }
                            });
                    });
                    ui.horizontal(|ui| {
                        ui.label("매수");
                        ui.add(egui::DragValue::new(&mut self.print_ui.copies).range(1..=999));
                    });
                }
                ui.separator();
                ui.radio_value(&mut self.print_ui.selection, Selection::All, "모두");
                ui.radio_value(
                    &mut self.print_ui.selection,
                    Selection::Current,
                    "현재 페이지",
                );
                ui.horizontal(|ui| {
                    ui.radio_value(&mut self.print_ui.selection, Selection::Range, "페이지");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.print_ui.range).desired_width(100.),
                    );
                });
                ui.add_enabled_ui(self.selected.is_some(), |ui| {
                    ui.radio_value(
                        &mut self.print_ui.selection,
                        Selection::Selected,
                        "선택 영역이 있는 페이지",
                    );
                });
                if !self.print_ui.pdf && cfg!(target_os = "windows") {
                    ui.horizontal(|ui| {
                        ui.label("해상도");
                        egui::ComboBox::from_id_salt("print-dpi")
                            .selected_text(format!("{} dpi", self.print_ui.dpi))
                            .show_ui(ui, |ui| {
                                for dpi in [150, 300, 600] {
                                    ui.selectable_value(
                                        &mut self.print_ui.dpi,
                                        dpi,
                                        format!("{dpi} dpi"),
                                    );
                                }
                            });
                    });
                }
                submit = ui
                    .add_enabled(
                        self.print_ui.job.is_none(),
                        egui::Button::new(if self.print_ui.pdf {
                            "저장…"
                        } else {
                            "인쇄"
                        }),
                    )
                    .clicked();
            });
        }
        self.print_ui.open = open;
        if submit {
            let path = if self.print_ui.pdf {
                #[cfg(not(target_arch = "wasm32"))]
                let path = rfd::FileDialog::new()
                    .add_filter("PDF", &["pdf"])
                    .set_file_name(format!("{}.pdf", self.document.title))
                    .save_file();
                #[cfg(target_arch = "wasm32")]
                let path: Option<PathBuf> = None;
                if path.is_none() {
                    return;
                }
                path
            } else {
                None
            };
            match self.begin_print(path) {
                Ok(()) => self.print_ui.open = false,
                Err(e) => self.status = format!("인쇄 실패: {e}"),
            }
        }
        if let Some(mut setup) = self.print_ui.setup.clone() {
            let mut setup_open = true;
            let mut apply = false;
            egui::Window::new("페이지 설정")
                .id(egui::Id::new("page-setup"))
                .open(&mut setup_open)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("용지");
                        egui::ComboBox::from_id_salt("paper-size")
                            .selected_text(setup.paper.code())
                            .show_ui(ui, |ui| {
                                for paper in PaperSize::ALL {
                                    ui.selectable_value(&mut setup.paper, paper, paper.code());
                                }
                            });
                    });
                    ui.horizontal(|ui| {
                        ui.radio_value(&mut setup.landscape, false, "세로");
                        ui.radio_value(&mut setup.landscape, true, "가로");
                    });
                    ui.label("여백 (mm)");
                    egui::Grid::new("paper-margins").show(ui, |ui| {
                        for (i, name) in
                            ["왼쪽", "위쪽", "오른쪽", "아래쪽"].into_iter().enumerate()
                        {
                            ui.label(name);
                            let mut mm = setup.margins[i] * 25.4 / 96.;
                            if ui
                                .add(
                                    egui::DragValue::new(&mut mm)
                                        .range(0. ..=500.)
                                        .speed(0.1)
                                        .fixed_decimals(1),
                                )
                                .changed()
                            {
                                setup.margins[i] = mm * 96. / 25.4;
                            }
                            ui.end_row();
                        }
                    });
                    ui.horizontal(|ui| {
                        for (name, factor) in [("표준", 1.), ("좁게", 0.5), ("넓게", 2.)] {
                            if ui.button(name).clicked() {
                                let inch = matches!(
                                    setup.paper,
                                    PaperSize::Letter | PaperSize::Legal | PaperSize::Ledger
                                );
                                setup.margins = [if inch {
                                    96. * factor
                                } else {
                                    2.5 * 96. / 2.54 * factor
                                }; 4];
                            }
                        }
                        if ui.button("Legacy").clicked() {
                            setup.margins = [115.2; 4];
                        }
                    });
                    if let Err(e) = setup.validate() {
                        ui.colored_label(Color32::DARK_RED, e);
                    }
                    apply = ui
                        .add_enabled(setup.validate().is_ok(), egui::Button::new("확인"))
                        .clicked();
                });
            if apply {
                if let Err(e) = self.commit_page_setup(setup) {
                    self.status = e;
                }
                self.print_ui.setup = None;
            } else {
                self.print_ui.setup = setup_open.then_some(setup);
            }
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    fn begin_print(&mut self, path: Option<PathBuf>) -> Result<(), String> {
        if self.print_ui.job.is_some() {
            return Err("다른 인쇄 작업이 진행 중입니다.".into());
        }
        let pages = match self.print_ui.selection {
            Selection::All => PageSelection::All,
            Selection::Current => PageSelection::Indices(vec![self.print_ui.current_page]),
            Selection::Range => parse_range(&self.print_ui.range)?,
            Selection::Selected => PageSelection::All,
        };
        let selected = if self.print_ui.selection == Selection::Selected {
            Some(self.selected.ok_or("인쇄할 영역을 선택하세요.")?)
        } else {
            None
        };
        let mut snapshot = self.print_snapshot();
        let dpi = if path.is_some() || !cfg!(target_os = "windows") {
            96
        } else {
            self.print_ui.dpi
        };
        let options = NativePrintOptions {
            job_name: self.document.title.clone(),
            printer: self.print_ui.printer.clone(),
            copies: self.print_ui.copies,
        };
        let (send, receive) = mpsc::channel();
        self.print_ui.job = Some(receive);
        self.status = "인쇄 페이지 준비 중…".into();
        std::thread::spawn(move || {
            let result = (|| {
                let mut plan = snapshot.plan(pages, dpi)?;
                if let Some(id) = selected {
                    let pages = snapshot.selected_pages(id)?;
                    plan = snapshot.plan(pages, dpi)?;
                }
                let total_pages = plan.page_count();
                let selected_pages = plan.source_pages().len();
                let pages = if path.is_some() || !cfg!(target_os = "windows") {
                    printing::render_vector_pages(&plan, |ui, page, total| {
                        snapshot.paint(ui, page, total)
                    })?
                } else {
                    printing::render_pages(&plan, |ui, page, total| {
                        snapshot.paint(ui, page, total)
                    })?
                };
                let count = pages.len();
                if let Some(path) = path {
                    printing::write_pdf(&path, &pages)?;
                    Ok(format!("PDF 저장: {} ({count}페이지)", path.display()))
                } else {
                    printing::submit_native(&pages, &options)?;
                    Ok(format!(
                        "인쇄 대기열로 {selected_pages}/{total_pages}페이지 전송"
                    ))
                }
            })();
            let _ = send.send(result);
        });
        Ok(())
    }
    fn print_geometry(&self) -> crate::page::PageGeometry {
        let margins = self
            .document
            .page_setup
            .as_ref()
            .map(|s| s.margins.map(|v| v as f32))
            .unwrap_or(self.page_margins);
        crate::page::PageGeometry::new(&self.document, margins)
    }
    fn print_snapshot(&mut self) -> PrintSnapshot {
        self.cancel_matrix_view_drag();
        // Printing needs presentation, never another copy of the source ZIP,
        // saved numeric sources or each editor's complete undo history.
        let document = CanvasDocument {
            version: self.document.version,
            title: self.document.title.clone(),
            width: self.document.width,
            height: self.document.height,
            grid: self.document.grid,
            next_id: self.document.next_id,
            regions: self.document.regions.clone(),
            page_sections: self.document.page_sections.clone(),
            page_setup: self.document.page_setup.clone(),
            native_archive: None,
            calculation_cache: None,
            bin_oct_hex_mode: self.document.bin_oct_hex_mode,
            matrix_result_views: self.document.matrix_result_views.clone(),
        };
        let mut displays = HashMap::new();
        let mut matrix_displays = HashMap::new();
        let mut matrix_indices = HashMap::new();
        let mut matrix_errors = HashMap::new();
        let mut editors = HashMap::new();
        let mut areas = HashMap::new();
        for r in &document.regions {
            if let CanvasContent::Opaque { xml, kind } = &r.content
                && kind == "Area"
            {
                let mut cache = area_presentation::AreaPresentationCache::default();
                let snapshot = (|| {
                    cache.refresh(xml, r.format)?;
                    let empty = HashMap::new();
                    let empty_saved = HashMap::new();
                    let prepared = cache.prepare(
                        r.id,
                        self.area_results.get(&r.id).unwrap_or(&empty),
                        self.area_saved.get(&r.id).unwrap_or(&empty_saved),
                        &self.area_pending,
                    );
                    let native = self
                        .area_visuals
                        .get(&r.id)
                        .map(Vec::as_slice)
                        .unwrap_or(&[]);
                    let mut views = prepared.borrow_views(native);
                    if let Some(labelled) = self
                        .committed_input
                        .as_ref()
                        .and_then(|input| input.area_labelled.get(&r.id))
                    {
                        for (key, view) in &mut views {
                            view.labelled = labelled.get(key);
                        }
                    }
                    let geometry = self.print_geometry();
                    let width = (geometry.width
                        - geometry.margins[0]
                        - geometry.margins[2]
                        - r.left as f32
                        - 15.)
                        .max(100.);
                    let error = views.values().any(|view| view.error.is_some())
                        || self
                            .results
                            .get(&r.id)
                            .is_some_and(|row| row.error.is_some())
                        || self
                            .saved_results
                            .get(&r.id)
                            .is_some_and(|row| row.error.is_some());
                    area_print::AreaPrintSnapshot::new(
                        xml,
                        &views,
                        width,
                        r.format.font_points,
                        error,
                    )
                })();
                areas.insert(r.id, snapshot);
            }
            let window = self
                .matrix_view_sources
                .window(r.id, self.requested_matrix_view(r.id));
            let indices_metadata = self.result_matrix_indices(r.id);
            let display = matrix_result_view::cached_formula_display(
                &mut self.display_cache,
                r.id,
                r.format,
                self.results.get(&r.id),
                self.saved_results.get(&r.id),
                if matches!(r.content, CanvasContent::Math(_)) {
                    window
                } else {
                    None
                },
            );
            let display =
                match matrix_result_view::indexed_formula_display(display, indices_metadata) {
                    Ok(shown) => shown,
                    Err(error) if indices_metadata.is_some_and(|(_, show)| show) => {
                        matrix_errors.insert(r.id, error);
                        display
                    }
                    // No persistent labels are requested and print is unfocused.
                    // Preserve complete cells rather than invent overflowing labels.
                    Err(_) => display,
                };
            match display {
                Some(formula::FormulaResult::MatrixViewportWithIndices {
                    projection,
                    indices,
                }) => {
                    matrix_displays.insert(r.id, projection.clone());
                    matrix_indices.insert(r.id, indices);
                }
                Some(formula::FormulaResult::MatrixViewport(view)) => {
                    matrix_displays.insert(r.id, view.clone());
                }
                Some(formula::FormulaResult::Formatted(view)) => {
                    displays.insert(r.id, view.clone());
                }
                Some(formula::FormulaResult::Text(error)) => {
                    matrix_errors.insert(r.id, error.to_owned());
                }
                _ => {}
            }
            if let CanvasContent::Math(tree) = &r.content {
                editors.insert(
                    r.id,
                    self.editors
                        .get(&r.id)
                        .map(Editor::clone_without_history)
                        .unwrap_or_else(|| Editor::from_portable(tree).unwrap_or_default()),
                );
            }
        }
        let file_name = self
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| document.title.clone());
        let file_path = self
            .path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| document.title.clone());
        PrintSnapshot {
            #[cfg(target_arch = "wasm32")]
            fonts: self.host.fonts.clone(),
            document,
            native_visuals: self.native_visuals.clone(),
            geometry: self.print_geometry(),
            displays,
            matrix_displays,
            matrix_indices,
            matrix_errors,
            editors,
            areas,
            labels: self.labelled_trees.clone(),
            plots: self.plot_results.clone(),
            heights: HashMap::new(),
            alignment_offsets: HashMap::new(),
            file_name,
            file_path,
            saved_date: self.saved_date.clone(),
            show_grid: self.show_grid,
            use_major_grid: self.use_major_grid,
        }
    }
}

struct PrintSnapshot {
    #[cfg(target_arch = "wasm32")]
    fonts: egui::FontDefinitions,
    document: CanvasDocument,
    native_visuals: Arc<NativeVisuals>,
    geometry: crate::page::PageGeometry,
    displays: HashMap<u64, crate::result_display::FormattedResult>,
    matrix_displays: HashMap<u64, crate::result_display::MatrixProjection>,
    matrix_indices: HashMap<u64, formula::MatrixIndexLabels>,
    matrix_errors: HashMap<u64, String>,
    editors: HashMap<u64, Editor>,
    areas: HashMap<u64, Result<area_print::AreaPrintSnapshot, String>>,
    labels: HashMap<u64, PortableTree>,
    plots: HashMap<u64, Result<crate::plots::PlotData, String>>,
    heights: HashMap<u64, f32>,
    alignment_offsets: HashMap<u64, f32>,
    file_name: String,
    file_path: String,
    saved_date: crate::page_fields::SavedDate,
    show_grid: bool,
    use_major_grid: bool,
}
impl PrintSnapshot {
    fn matrix_result(&self, owner: u64) -> Option<formula::FormulaResult<'_>> {
        self.matrix_displays.get(&owner).map(|projection| {
            self.matrix_indices.get(&owner).map_or(
                formula::FormulaResult::MatrixViewport(projection),
                |indices| formula::FormulaResult::MatrixViewportWithIndices {
                    projection,
                    indices: *indices,
                },
            )
        })
    }
    #[cfg(not(target_arch = "wasm32"))]
    fn selected_pages(&self, id: u64) -> Result<PageSelection, String> {
        let region = self
            .document
            .regions
            .iter()
            .find(|r| r.id == id)
            .ok_or("선택 영역이 없습니다.")?;
        if region.left >= f64::from(self.geometry.width - self.geometry.margins[0]) {
            return Err("선택 영역이 인쇄 범위 밖에 있습니다.".into());
        }
        let height = f64::from(
            *self
                .heights
                .get(&id)
                .ok_or("선택 영역을 아직 조판하지 않았습니다.")?,
        );
        let body = self.geometry.body_height_exact();
        let top = region.top + f64::from(self.alignment_offsets.get(&id).copied().unwrap_or(0.));
        let first = (top.max(0.) / body).floor() as usize;
        let end = ((top + height) / body).ceil().max(first as f64 + 1.) as usize;
        Ok(PageSelection::Indices((first..end).collect()))
    }
    fn plan(&mut self, pages: PageSelection, dpi: u16) -> Result<PrintPlan, String> {
        for region in &self.document.regions {
            if region.left < f64::from(self.geometry.width - self.geometry.margins[0])
                && let Some(error) = self.matrix_errors.get(&region.id)
            {
                return Err(format!("행렬 결과 표시 범위를 인쇄할 수 없습니다: {error}"));
            }
            if mathcraft_document::page_break::is_page_break(region)? {
                continue;
            }
            if region.left < f64::from(self.geometry.width - self.geometry.margins[0])
                && self
                    .plots
                    .get(&region.id)
                    .is_some_and(|result| result.as_ref().is_ok_and(|data| data.has_unavailable()))
            {
                return Err("도표의 지원되지 않는 저장 곡선을 복원할 수 없습니다".into());
            }
            if let Some(area) = self.areas.get(&region.id) {
                area.as_ref().map_err(Clone::clone)?;
                continue;
            }
            if region.left < f64::from(self.geometry.width - self.geometry.margins[0])
                && let Some(Err(error)) = self.native_visuals.region(region)
            {
                return Err(format!(
                    "원본 영역 {}의 서식을 복원할 수 없어 출력하지 못했습니다: {error}",
                    region.source_region.as_deref().unwrap_or("?")
                ));
            }
        }
        let context = egui::Context::default();
        #[cfg(not(target_arch = "wasm32"))]
        crate::configure(&context);
        #[cfg(target_arch = "wasm32")]
        crate::configure_surface(&context, self.fonts.clone(), crate::Theme::Light);
        let mut bottom = 0f64;
        let mut output = context.run_ui(egui::RawInput::default(), |ui| {
            for r in &self.document.regions {
                if r.left >= f64::from(self.geometry.width - self.geometry.margins[0]) {
                    continue;
                }
                if mathcraft_document::page_break::is_page_break(r).unwrap_or(false) {
                    self.heights.insert(r.id, 2.);
                    self.alignment_offsets.insert(r.id, 0.);
                    // A break at B starts the next page even when there are no
                    // subsequent regions. Its editing marker supplies no ink.
                    bottom = bottom.max(r.top + 2.);
                    continue;
                }
                let visual = self.native_visuals.region(r).and_then(|v| v.as_ref().ok());
                let width = visual
                    .map(|v| v.size[0])
                    .filter(|w| *w > 0.)
                    .unwrap_or(340.);
                let native_source = r.source_region.as_ref().is_some_and(|id| self.native_visuals.regions.contains_key(id));
                let mut alignment_offset = 0.;
                let height = if let Some(Ok(area))=self.areas.get(&r.id) {area.measure(ui).y}
                else if let Some(visual) = visual.filter(|v| v.document.is_some()) {
                    alignment_offset = crate::native_presentation::alignment_offset(ui, visual, width, matches!(&r.content, CanvasContent::Opaque {kind, ..} if kind == "spec-table"));
                    crate::native_presentation::measure(ui, visual, width).y
                } else {
                    match &r.content {
                        CanvasContent::Math(_) => {
                            let (size, baseline) = formula::measured_geometry_readonly_styled(
                                ui,
                                &self.editors[&r.id],
                                self.matrix_result(r.id)
                                    .or_else(|| self.displays.get(&r.id).map(formula::FormulaResult::Formatted)),
                                self.labels.get(&r.id),
                                r.format.font_points,
                            );
                            if native_source { alignment_offset = -baseline; }
                            size.y
                        }
                        CanvasContent::Text(text) => {
                            let galley = ui.painter().layout(text.clone(), typography::body_font(), Color32::BLACK, 340.);
                            if native_source { alignment_offset = -galley.rows.first().and_then(|row| row.glyphs.first().map(|g| row.pos.y + g.pos.y)).unwrap_or(0.); }
                            galley.size().y
                        }
                        CanvasContent::Plot(spec) => spec.height,
                        CanvasContent::Opaque { .. } => self
                            .displays
                            .get(&r.id)
                            .map(|result| {
                                formula::readonly_result_ui(ui, result, r.format.font_points)
                                    .rect
                                    .height()
                            })
                            .unwrap_or(22.),
                    }
                };
                self.heights.insert(r.id, height);
                self.alignment_offsets.insert(r.id, alignment_offset);
                bottom = bottom.max(r.top + f64::from(alignment_offset + height));
            }
        });
        output.textures_delta.clear();
        PrintPlan::new(&PrintSettings {
            paper_size: [self.geometry.width, self.geometry.height],
            margins: self.geometry.margins,
            body_height: self.geometry.body_height_exact(),
            content_bottom: bottom,
            pages,
            dpi,
        })
    }
    fn paint(&mut self, ui: &mut egui::Ui, page: usize, pages: usize) {
        let g = self.geometry;
        let start = page as f64 * g.body_height_exact();
        let body = Rect::from_min_size(
            pos2(g.margins[0], g.margins[1]),
            vec2(g.body_width(), g.body_height()),
        );
        // The native 1015 PDF prints the spec-table through the right margin.
        // Margins position content; only the physical sheet edge clips it.
        // Vertical body bounds still divide consecutive worksheet pages.
        let content_clip = Rect::from_min_max(body.min, pos2(g.width, body.bottom()));
        if self.show_grid {
            let stroke = |distance: f32| {
                let major = distance / g.major_spacing();
                if self.use_major_grid && (major - major.round()).abs() < 0.001 {
                    Stroke::new(1.2, Color32::from_rgb(181, 218, 249))
                } else {
                    Stroke::new(0.6, Color32::from_rgb(189, 222, 248))
                }
            };
            let grid = g.grid.max(6.);
            let mut x = body.left();
            while x <= body.right() {
                ui.painter().line_segment(
                    [pos2(x, body.top()), pos2(x, body.bottom())],
                    stroke(x - body.left()),
                );
                x += grid;
            }
            let mut y = body.top();
            while y <= body.bottom() {
                ui.painter().line_segment(
                    [pos2(body.left(), y), pos2(body.right(), y)],
                    stroke(y - body.top()),
                );
                y += grid;
            }
        }
        for r in &self.document.regions {
            if mathcraft_document::page_break::is_page_break(r).unwrap_or(false) {
                continue;
            }
            let top = r.top + f64::from(self.alignment_offsets.get(&r.id).copied().unwrap_or(0.));
            if r.left >= f64::from(g.width - g.margins[0])
                || top >= start + g.body_height_exact()
                || top + f64::from(self.heights.get(&r.id).copied().unwrap_or(100.)) <= start
            {
                continue;
            }
            let mut origin = body.min + vec2(r.left as f32, (top - start) as f32);
            if matches!(r.content, CanvasContent::Math(_))
                && r.source_region
                    .as_ref()
                    .is_some_and(|id| self.native_visuals.regions.contains_key(id))
            {
                origin.x += 2.;
            }
            let visual = self.native_visuals.region(r).and_then(|v| v.as_ref().ok());
            if matches!(r.content, CanvasContent::Text(_))
                && visual.is_none()
                && r.source_region
                    .as_ref()
                    .is_some_and(|id| self.native_visuals.regions.contains_key(id))
            {
                origin.x += 5.;
            }
            let width = visual
                .map(|v| v.size[0])
                .filter(|w| *w > 0.)
                .unwrap_or(340.);
            let mut child = ui.new_child(
                UiBuilder::new()
                    .id_salt(("print-region", r.id))
                    .max_rect(Rect::from_min_size(origin, vec2(width, 200.)))
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            );
            child.set_clip_rect(content_clip);
            if let Some(Ok(area)) = self.areas.get(&r.id) {
                area.paint(&mut child, origin, content_clip);
                continue;
            }
            if let Some(visual) = visual.filter(|v| v.document.is_some()) {
                crate::native_presentation::draw(&mut child, visual, width);
                continue;
            }
            let background_slot = visual
                .filter(|v| v.background.is_some())
                .map(|_| child.painter().add(egui::Shape::Noop));
            let display = self.displays.get(&r.id);
            match &r.content {
                CanvasContent::Math(_) => {
                    formula::formula_readonly_ui_styled(
                        &mut child,
                        &self.editors[&r.id],
                        self.matrix_result(r.id)
                            .or_else(|| display.map(formula::FormulaResult::Formatted)),
                        self.labels.get(&r.id),
                        r.format.font_points,
                    );
                }
                CanvasContent::Text(text) => {
                    child.add(
                        egui::Label::new(
                            egui::RichText::new(text)
                                .font(typography::body_font())
                                .color(Color32::BLACK),
                        )
                        .wrap(),
                    );
                }
                CanvasContent::Plot(spec) => {
                    let missing = Err("그래프 계산 대기 중".into());
                    crate::plots::draw_print(
                        &mut child,
                        spec,
                        self.plots.get(&r.id).unwrap_or(&missing),
                    );
                }
                CanvasContent::Opaque { .. } => {
                    if let Some(display) = display {
                        formula::readonly_result_ui(&mut child, display, r.format.font_points);
                    } else {
                        child.label("가져온 영역 · 읽기 전용");
                    }
                }
            }
            if let (Some(slot), Some(visual)) = (background_slot, visual)
                && let Some([red, green, blue, alpha]) = visual.background
            {
                let rect = child.min_rect().union(Rect::from_min_size(
                    origin
                        - vec2(
                            if matches!(r.content, CanvasContent::Math(_)) {
                                2.
                            } else {
                                0.
                            },
                            0.,
                        ),
                    vec2(visual.size[0], visual.size[1]),
                ));
                child.painter().set(
                    slot,
                    egui::Shape::rect_filled(
                        rect,
                        0.,
                        Color32::from_rgba_unmultiplied(red, green, blue, alpha),
                    ),
                );
            }
        }
        for (band, footer) in [
            (&self.document.page_sections.header, false),
            (&self.document.page_sections.footer, true),
        ] {
            for r in &band.regions {
                let text = match &r.content {
                    PageBandContent::Text(text) => text.clone(),
                    PageBandContent::Field(PageField::FileName) => self.file_name.clone(),
                    PageBandContent::Field(PageField::FilePath) => self.file_path.clone(),
                    PageBandContent::Field(PageField::DateSaved(format)) => {
                        self.saved_date.format(*format)
                    }
                    PageBandContent::Field(PageField::PageNumber(format)) => {
                        let number = self
                            .document
                            .page_sections
                            .starting_page_number
                            .saturating_add(page as u32);
                        let total = self
                            .document
                            .page_sections
                            .starting_page_number
                            .saturating_add(pages.saturating_sub(1) as u32)
                            .saturating_add(self.document.page_sections.total_pages_offset);
                        match format {
                            PageNumberFormat::Number => number.to_string(),
                            PageNumberFormat::PageNumber => format!("Page {number}"),
                            PageNumberFormat::AbbreviatedPageNumber => format!("p. {number}"),
                            PageNumberFormat::NumberOfTotal => format!("{number} of {total}"),
                            PageNumberFormat::PageNumberOfTotal => {
                                format!("Page {number} of {total}")
                            }
                            PageNumberFormat::DashedNumber => format!("- {number} -"),
                            PageNumberFormat::ParenthesizedNumber => format!("({number})"),
                            PageNumberFormat::NumberParenthesizedTotal => {
                                format!("{number} ({total})")
                            }
                        }
                    }
                };
                ui.painter().text(
                    pos2(
                        r.left as f32,
                        if footer {
                            g.height - g.margins[3] + r.top as f32
                        } else {
                            r.top as f32
                        },
                    ),
                    egui::Align2::LEFT_TOP,
                    text,
                    typography::body_font(),
                    Color32::BLACK,
                );
            }
        }
    }
}

#[cfg(test)]
#[path = "native_result_integration.rs"]
mod native_result_integration;

#[cfg(test)]
#[path = "inline_plot_integration.rs"]
mod inline_plot_integration;
#[cfg(test)]
#[path = "native_plot_integration.rs"]
mod native_plot_integration;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_break_counts_next_blank_page_but_adds_no_print_ink_or_evaluation() {
        use mathcraft_document::page_break::{PageBreakLayout, insert_page_break};
        let mut app = WorksheetApp::default();
        let body = app.page_geometry().body_height_exact();
        let candidate = insert_page_break(
            &app.document,
            PageBreakLayout {
                body_height: body,
                grid_height: app.document.grid,
            },
            [0., 20.],
            &[],
        )
        .unwrap();
        app.document = candidate.document;
        let before = app.document.clone();
        let mut snapshot = app.print_snapshot();
        snapshot.show_grid = false;
        let plan = snapshot.plan(PageSelection::All, 300).unwrap();
        assert_eq!(plan.page_count(), 2);
        assert_eq!(snapshot.heights[&candidate.break_id], 2.);
        let context = egui::Context::default();
        crate::configure(&context);
        for page in 0..2 {
            context
                .run_ui(Default::default(), |ui| {
                    let prior = ui.painter().add(egui::Shape::Noop);
                    snapshot.paint(ui, page, 2);
                    let after = ui.painter().add(egui::Shape::Noop);
                    assert_eq!(
                        after.0,
                        prior.0 + 1,
                        "Editing marker must supply no PDF line or placeholder"
                    );
                })
                .drop_without_applying_deltas();
        }
        assert!(app.last_executed_regions.is_empty() && app.results.is_empty());
        assert_eq!(app.document, before);
    }

    #[test]
    fn worksheet_print_routes_nested_saved_area_through_vector_snapshot_without_execution() {
        let mut app = WorksheetApp::default();
        let id = app.document.add_math(20., 20.);
        let xml = r#"<region xmlns="http://schemas.mathsoft.com/worksheet50" xmlns:m="http://schemas.mathsoft.com/math50" region-id="100"><Area IsCollapsed="false" IsBorderVisible="true" AreaHeight="150"><regions><region region-id="1" left="20" top="30"><math><m:eval><m:real>7</m:real><m:unitOverride><m:placeholder/></m:unitOverride></m:eval></math></region></regions></Area></region>"#;
        app.document.regions[0].content = CanvasContent::Opaque {
            xml: xml.into(),
            kind: "Area".into(),
        };
        app.recalculate();
        app.persist_calculation_cache();
        app.last_executed_regions.clear();
        let before = app.document.clone();
        let mut snapshot = app.print_snapshot();
        let plan = snapshot.plan(PageSelection::All, 300).unwrap();
        assert!(snapshot.heights[&id] >= 150.);
        let pages =
            printing::render_vector_pages(&plan, |ui, page, total| snapshot.paint(ui, page, total))
                .unwrap();
        let path = std::env::temp_dir().join(format!(
            "mathcraft-area-route-{}-{}.pdf",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        printing::write_pdf(&path, &pages).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        std::fs::remove_file(path).unwrap();
        let pdf = String::from_utf8_lossy(&bytes);
        assert!(pdf.contains("/ToUnicode") && !pdf.contains("/Subtype /Image"));
        assert_eq!(app.document, before);
        assert!(app.last_executed_regions.is_empty());
    }

    /// Export a caller-selected real document through the application's exact
    /// cold-open/print path. This test creates no window and executes no math.
    #[test]
    #[ignore]
    fn export_native_vector_pdf_probe() {
        let input = std::env::var_os("MATHCRAFT_NATIVE_PDF_INPUT")
            .expect("Set MATHCRAFT_NATIVE_PDF_INPUT to the original MCDX path");
        let output = std::env::var_os("MATHCRAFT_NATIVE_PDF_OUTPUT")
            .expect("Set MATHCRAFT_NATIVE_PDF_OUTPUT to a new PDF path");
        let mut app = WorksheetApp::default();
        app.open(Path::new(&input));
        assert!(app.document.native_archive.is_some(), "{}", app.status);
        assert!(app.last_executed_regions.is_empty());
        let signature = app.document.calculation_signature();
        let mut snapshot = app.print_snapshot();
        assert!(snapshot.document.native_archive.is_none());
        assert!(snapshot.document.calculation_cache.is_none());
        assert!(Arc::ptr_eq(&snapshot.native_visuals, &app.native_visuals));
        let plan = snapshot.plan(PageSelection::All, 300).unwrap();
        let started = std::time::Instant::now();
        let pages =
            printing::render_vector_pages(&plan, |ui, page, total| snapshot.paint(ui, page, total))
                .unwrap();
        printing::write_pdf(Path::new(&output), &pages).unwrap();
        let tables = snapshot.document.regions.iter().filter_map(|r| {
            if !matches!(&r.content, CanvasContent::Opaque {kind, ..} if kind == "spec-table") { return None; }
            let visual = snapshot.native_visuals.region(r)?.as_ref().ok()?;
            let document = visual.document.as_ref()?;
            let table = document.blocks.iter().find_map(|b| if let mathcraft_document::native_visual::Block::Table(t) = b { Some(t) } else { None })?;
            let page = (r.top / snapshot.geometry.body_height_exact()).floor() as usize;
            let top = r.top + f64::from(snapshot.alignment_offsets.get(&r.id).copied().unwrap_or(0.)) - page as f64 * snapshot.geometry.body_height_exact();
            Some(serde_json::json!({"source_region":r.source_region,"rows":table.rows,"columns":table.columns.len(),"cells":table.cells.len(),"page":page,"pdf_rect":[(f64::from(snapshot.geometry.margins[0])+r.left)*0.75,(f64::from(snapshot.geometry.margins[1])+top)*0.75,(f64::from(snapshot.geometry.margins[0])+r.left+f64::from(visual.size[0]))*0.75,(f64::from(snapshot.geometry.margins[1])+top+f64::from(snapshot.heights[&r.id]))*0.75]}))
        }).collect::<Vec<_>>();
        let errors = snapshot
            .native_visuals
            .regions
            .iter()
            .filter_map(|(id, visual)| {
                visual
                    .as_ref()
                    .err()
                    .map(|e| serde_json::json!({"source_region":id,"error":e}))
            })
            .collect::<Vec<_>>();
        std::fs::write(Path::new(&output).with_extension("json"), serde_json::to_vec_pretty(&serde_json::json!({"source":Path::new(&input),"regions":app.document.regions.len(),"restored":snapshot.displays.len(),"pages":pages.len(),"tables":tables,"native_visual_errors":errors})).unwrap()).unwrap();
        println!(
            "NATIVE_VECTOR regions={} restored={} pending={} pages={} elapsed_us={} status={}",
            app.document.regions.len(),
            snapshot.displays.len(),
            app.pending_regions.len(),
            pages.len(),
            started.elapsed().as_micros(),
            app.status
        );
        assert_eq!(app.document.calculation_signature(), signature);
        assert!(app.last_executed_regions.is_empty());
        assert!(app.results.is_empty());
    }

    #[test]
    fn native_rich_table_and_math_background_reach_the_print_painter() {
        use mathcraft_document::native_visual::{RegionVisual, TextStyle, read_flow_xml};
        let rich = read_flow_xml(r##"<FlowDocument xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation" Background="#FF662D91" Foreground="#FFFFFFFF"><Paragraph><Run>Reference table</Run></Paragraph><Table><TableRowGroup><TableRow><TableCell BorderThickness="1" BorderBrush="#FF000000"><Paragraph><Run>Wire</Run></Paragraph></TableCell><TableCell BorderThickness="1" BorderBrush="#FF000000"><Paragraph><Run>Current</Run></Paragraph></TableCell></TableRow><TableRow><TableCell><Paragraph><Run>1 mm</Run></Paragraph></TableCell><TableCell><Paragraph><Run>10 A</Run></Paragraph></TableCell></TableRow></TableRowGroup></Table></FlowDocument>"##, TextStyle::default()).unwrap();
        let mut app = WorksheetApp::default();
        let table = app.document.add_opaque(0., 0., "", "text");
        app.document.regions[0].source_region = Some("table".into());
        let math = app.document.add_math(0., 160.);
        app.document.regions[1].source_region = Some("math".into());
        app.native_visuals = Arc::new(NativeVisuals {
            regions: HashMap::from([
                (
                    "table".into(),
                    Ok(RegionVisual {
                        size: [300., 120.],
                        background: Some([102, 45, 145, 255]),
                        document: Some(rich),
                    }),
                ),
                (
                    "math".into(),
                    Ok(RegionVisual {
                        size: [120., 30.],
                        background: Some([255, 242, 0, 255]),
                        document: None,
                    }),
                ),
            ]),
        });
        let mut snapshot = app.print_snapshot();
        snapshot.plan(PageSelection::All, 300).unwrap();
        assert!(snapshot.heights[&table] >= 120.);
        let context = egui::Context::default();
        crate::configure(&context);
        let output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1000., 1500.))),
                ..Default::default()
            },
            |ui| snapshot.paint(ui, 0, 1),
        );
        fn collect(shape: &egui::Shape, text: &mut String, fills: &mut Vec<Color32>) {
            match shape {
                egui::Shape::Text(t) => {
                    text.push_str(t.galley.text());
                    text.push('\n');
                }
                egui::Shape::Rect(r) => fills.push(r.fill),
                egui::Shape::Vec(shapes) => {
                    for s in shapes {
                        collect(s, text, fills)
                    }
                }
                _ => (),
            }
        }
        let mut text = String::new();
        let mut fills = Vec::new();
        for shape in &output.shapes {
            collect(&shape.shape, &mut text, &mut fills);
        }
        assert!(
            text.contains("Reference table") && text.contains("Wire") && text.contains("10 A"),
            "{text}"
        );
        assert!(!text.contains("가져온 영역"));
        assert!(fills.contains(&Color32::from_rgb(102, 45, 145)));
        assert!(fills.contains(&Color32::from_rgb(255, 242, 0)));
        assert!(snapshot.editors.contains_key(&math));
        output.drop_without_applying_deltas();
        assert!(app.last_executed_regions.is_empty());
    }
    #[test]
    fn saved_date_field_is_copied_into_print_snapshot_and_painted_as_a_date() {
        let mut app = WorksheetApp {
            saved_date: crate::page_fields::SavedDate::from_timestamp(
                std::time::UNIX_EPOCH + std::time::Duration::from_secs(951_825_600),
            )
            .unwrap(),
            ..Default::default()
        };
        let expected = app.saved_date.format(DateSavedFormat::YearMonthDay);
        app.document
            .add_footer_field(0., 0., PageField::DateSaved(DateSavedFormat::YearMonthDay));
        let mut snapshot = app.print_snapshot();
        app.saved_date = crate::page_fields::SavedDate::default();
        assert_eq!(
            snapshot.saved_date.format(DateSavedFormat::YearMonthDay),
            expected
        );
        let context = egui::Context::default();
        crate::configure(&context);
        let output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1200., 1400.))),
                ..Default::default()
            },
            |ui| snapshot.paint(ui, 0, 1),
        );
        fn has_text(shape: &egui::Shape, expected: &str) -> bool {
            match shape {
                egui::Shape::Text(text) => text.galley.text() == expected,
                egui::Shape::Vec(shapes) => shapes.iter().any(|shape| has_text(shape, expected)),
                _ => false,
            }
        }
        assert!(
            output
                .shapes
                .iter()
                .any(|shape| has_text(&shape.shape, &expected))
        );
        output.drop_without_applying_deltas();
    }
    #[test]
    fn print_range_is_single_or_contiguous_and_rejects_comma_lists() {
        assert_eq!(
            parse_range(" 2 ").unwrap(),
            PageSelection::Range { first: 2, last: 2 }
        );
        assert_eq!(
            parse_range("1 - 3").unwrap(),
            PageSelection::Range { first: 1, last: 3 }
        );
        for invalid in ["0", "-1", "3-1", "1,3", "1-2-3", "", "two"] {
            assert!(parse_range(invalid).is_err());
        }
    }
    #[test]
    fn printing_uses_the_same_percent_presentation_after_cold_save_reopen() {
        let mut app = WorksheetApp::default();
        let id = app.document.add_math(0., 20.);
        let mut editor = Editor::new();
        formula::input_text(&mut editor, "0.94359=");
        app.document.regions[0].content = CanvasContent::Math(editor.to_portable().unwrap());
        app.recalculate();
        app.persist_calculation_cache();
        app.results.clear();
        app.restore_saved_calculations();
        app.selected = Some(id);
        app.ribbon_command(&egui::Context::default(), RibbonAction::SetResultFormat(4));
        let snapshot = app.print_snapshot();
        assert_eq!(snapshot.displays[&id].cells[0].plain(), "94.359%");
        assert!(app.last_executed_regions.is_empty());
        assert!(app.results.is_empty());
    }

    #[test]
    fn print_snapshot_keeps_pending_results_without_calculation_and_excludes_draft_only_pages() {
        let mut app = WorksheetApp::default();
        let id = app.document.add_math(0., 20.);
        let mut editor = Editor::new();
        formula::input_text(&mut editor, "2+3=");
        app.document.regions[0].content = CanvasContent::Math(editor.to_portable().unwrap());
        app.recalculate();
        app.pending_regions.insert(id);
        app.document.add_text(1500., 100000., "Draft-only");
        let before = app.last_executed_regions.clone();
        let mut snapshot = app.print_snapshot();
        let plan = snapshot.plan(PageSelection::All, 72).unwrap();
        assert_eq!(plan.page_count(), 1);
        assert!(snapshot.displays.contains_key(&id));
        assert_eq!(app.last_executed_regions, before);
        let pages =
            printing::render_pages(&plan, |ui, page, total| snapshot.paint(ui, page, total))
                .unwrap();
        assert_eq!(pages.len(), 1);
    }
    #[test]
    fn engineering_example_prints_cached_units_matrices_and_plots_without_reexecution() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/engineering-worksheet.mathcraft");
        let mut app = WorksheetApp {
            document: CanvasDocument::load(&path).unwrap(),
            insertion: None,
            ..Default::default()
        };
        app.recalculate();
        assert!(app.calculation_error.is_none());
        assert!(app.results.values().all(|row| row.error.is_none()));
        assert!(app.plot_results.values().all(Result::is_ok));
        let executed = app.last_executed_regions.clone();
        let mut snapshot = app.print_snapshot();
        let plan = snapshot.plan(PageSelection::All, 150).unwrap();
        let pages =
            printing::render_pages(&plan, |ui, page, total| snapshot.paint(ui, page, total))
                .unwrap();
        assert!(!pages.is_empty());
        assert_eq!(app.last_executed_regions, executed);
        if let Some(directory) = std::env::var_os("MATHCRAFT_PRINT_TEST_CAPTURE") {
            std::fs::create_dir_all(&directory).unwrap();
            printing::write_pdf(
                &Path::new(&directory).join("engineering-worksheet.pdf"),
                &pages,
            )
            .unwrap();
        }
    }

    #[test]
    fn selection_prints_every_page_touched_by_selected_content() {
        let mut app = WorksheetApp::default();
        let body = app.print_geometry().body_height_exact();
        let id = app
            .document
            .add_text(0., body - 10., "first\nsecond\nthird\nfourth");
        let mut snapshot = app.print_snapshot();
        let plan = snapshot.plan(PageSelection::All, 72).unwrap();
        assert_eq!(plan.page_count(), 2);
        assert_eq!(
            snapshot.selected_pages(id).unwrap(),
            PageSelection::Indices(vec![0, 1])
        );
        snapshot.document.regions[0].left = 2000.;
        assert!(snapshot.selected_pages(id).is_err());
    }

    #[test]
    fn page_setup_preserves_calculated_results_and_has_undo_snapshot() {
        let mut app = WorksheetApp::default();
        app.document.add_text(0., 0., "example");
        let before = app.document.clone();
        let setup = PageSetup {
            paper: PaperSize::Letter,
            landscape: true,
            margins: [48.; 4],
        };
        app.commit_page_setup(setup.clone()).unwrap();
        assert_eq!(app.document.width, 1056.);
        assert_eq!(app.print_geometry().margins, [48.; 4]);
        assert_eq!(app.document.page_setup, Some(setup));
        assert!(matches!(
            &app.undo.last().unwrap().document,
            DocumentHistory::Full(document) if document.page_setup.is_none()
        ));
        assert_eq!(before.regions, app.document.regions);
        assert!(app.last_executed_regions.is_empty());
    }

    #[test]
    fn right_margin_content_prints_until_the_physical_sheet_edge() {
        let mut app = WorksheetApp::default();
        let geometry = app.print_geometry();
        let id = app
            .document
            .add_text(f64::from(geometry.body_width() + 5.), 0., "Right margin");
        let mut snapshot = app.print_snapshot();
        snapshot.plan(PageSelection::All, 300).unwrap();
        assert!(snapshot.heights.contains_key(&id));
        assert_eq!(
            snapshot.selected_pages(id).unwrap(),
            PageSelection::Indices(vec![0])
        );
        let context = egui::Context::default();
        crate::configure(&context);
        let output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1000., 1500.))),
                ..Default::default()
            },
            |ui| snapshot.paint(ui, 0, 1),
        );
        let shape = output
            .shapes
            .iter()
            .find(|s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text() == "Right margin"))
            .unwrap();
        assert_eq!(shape.clip_rect.right(), geometry.width);
        assert!(shape.clip_rect.right() > geometry.width - geometry.margins[2]);
        output.drop_without_applying_deltas();
        snapshot.document.regions[0].left = f64::from(geometry.width - geometry.margins[0]) + 1.;
        assert!(snapshot.selected_pages(id).is_err());
    }

    #[test]
    fn native_equation_location_is_its_baseline_in_canvas_and_print() {
        use mathcraft_document::native_visual::RegionVisual;
        let mut app = WorksheetApp::default();
        let native = app.document.add_math(0., 100.);
        app.document.regions[0].content = CanvasContent::Math(PortableTree {
            kind: NodeKind::Number("12".into()),
            children: vec![],
        });
        app.document.regions[0].source_region = Some("native".into());
        let own = app.document.add_math(0., 200.);
        app.document.regions[1].content = app.document.regions[0].content.clone();
        app.native_visuals = Arc::new(NativeVisuals {
            regions: HashMap::from([(
                "native".into(),
                Ok(RegionVisual {
                    size: [40., 25.6],
                    background: None,
                    document: None,
                }),
            )]),
        });
        let expected = app.print_geometry().margins[1] + app.document.regions[0].top as f32;
        fn baselines(shape: &egui::Shape, values: &mut Vec<f32>) {
            match shape {
                egui::Shape::Text(t) if t.galley.text() == "12" => {
                    let row = &t.galley.rows[0];
                    values.push(t.pos.y + row.pos.y + row.glyphs[0].pos.y);
                }
                egui::Shape::Vec(shapes) => {
                    for s in shapes {
                        baselines(s, values);
                    }
                }
                _ => (),
            }
        }
        for print in [false, true] {
            let context = egui::Context::default();
            crate::configure(&context);
            let output = if print {
                let mut snapshot = app.print_snapshot();
                snapshot.plan(PageSelection::All, 300).unwrap();
                assert!(snapshot.alignment_offsets[&native] < 0.);
                assert_eq!(snapshot.alignment_offsets[&own], 0.);
                context.run_ui(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(
                            egui::Pos2::ZERO,
                            vec2(1200., 1500.),
                        )),
                        ..Default::default()
                    },
                    |ui| snapshot.paint(ui, 0, 1),
                )
            } else {
                context.run_ui(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(
                            egui::Pos2::ZERO,
                            vec2(1200., 1500.),
                        )),
                        ..Default::default()
                    },
                    |ui| app.page_ui(ui),
                )
            };
            let mut values = Vec::new();
            for s in &output.shapes {
                baselines(&s.shape, &mut values);
            }
            assert!(
                values.iter().any(|v| (*v - expected).abs() < 0.05),
                "print={print}, expected={expected}, actual={values:?}"
            );
            output.drop_without_applying_deltas();
        }
        assert!(app.last_executed_regions.is_empty());
    }
}

#[cfg(test)]
#[path = "shared_plot_integration.rs"]
mod shared_plot_integration;

#[cfg(test)]
#[path = "plot_outcome_integration.rs"]
mod plot_outcome_integration;

#[cfg(test)]
#[path = "plot_geometry_integration.rs"]
mod plot_geometry_integration;

#[cfg(test)]
#[path = "plot_slot_edit_integration.rs"]
mod plot_slot_edit_integration;

#[cfg(test)]
#[path = "plot_unit_integration.rs"]
mod plot_unit_integration;

#[cfg(test)]
#[path = "plot_slot_key_integration.rs"]
mod plot_slot_key_integration;

#[cfg(test)]
#[path = "string_value_integration.rs"]
mod string_value_integration;
#[cfg(test)]
#[path = "typed_plot_integration.rs"]
mod typed_plot_integration;

#[cfg(test)]
#[path = "numeric_entry_mode_integration.rs"]
mod numeric_entry_mode_integration;
#[cfg(test)]
#[path = "temperature_integration.rs"]
mod temperature_integration;

#[cfg(test)]
#[path = "matrix_view_packet_tests.rs"]
mod matrix_view_packet_tests;

#[cfg(test)]
#[path = "matrix_live_indices_packet_tests.rs"]
mod matrix_live_indices_packet_tests;

#[cfg(target_arch = "wasm32")]
impl WorksheetApp {
    pub(super) fn open_print(&mut self, pdf: bool) {
        self.host_print_request(pdf);
    }
    fn begin_print(&mut self, _path: Option<PathBuf>) -> Result<(), String> {
        Err("Printing is provided by the host".into())
    }
    pub(crate) fn host_pdf(&mut self) -> Result<Vec<u8>, String> {
        self.commit_table_edits(None)?;
        let mut snapshot = self.print_snapshot();
        let plan = snapshot.plan(PageSelection::All, 96)?;
        let pages = printing::render_host_vector_pages(
            &plan,
            snapshot.fonts.clone(),
            |ui, page, total| snapshot.paint(ui, page, total),
        )?;
        printing::pdf_bytes(&pages)
    }
}
