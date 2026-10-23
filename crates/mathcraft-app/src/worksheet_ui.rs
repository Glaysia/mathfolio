//! A worksheet surface: positioned structural formulas and text share one document.
//! The initial math/cursor subset remains explicitly narrower than Prime.
use crate::{
    formula,
    ribbon::{self, RibbonAction, RibbonContext, RibbonState},
    typography, worksheet_math,
};
use eframe::egui::{self, Color32, FontId, Rect, Sense, Stroke, UiBuilder, pos2, vec2};
#[cfg(test)]
use mathcraft_compat::Value;
use mathcraft_compat::{
    CalculationRequest, CalculationUpdate, RegionEvaluation, WorksheetCalculator,
    solve_builder::{SolveBlockBuilder, SolveMethod},
};
use mathcraft_document::{
    canvas::{
        CanvasContent, CanvasDocument, CanvasRegion, DateSavedFormat, MathFormat,
        MatrixResultTarget, MatrixResultViewport, PageBand, PageBandContent, PageBandRegion,
        PageField, PageNumberFormat, PageSections, PlotAxisSpec, PlotLineStyle, PlotMarker,
        PlotRange, PlotSource, PlotSpec, PlotTraceSpec, PlotTraceStyle, ResultFormat,
        SavedCalculationCache, SavedCalculationRow, SavedCalculationStatus, SavedNumericSource,
    },
    native::NativePackage,
    native_visual::NativeVisuals,
};
use mathcraft_editor::{BinaryOp, Direction, Editor, NodeKind, PortableTree};
use original_shortcuts::consume_exact_key;
#[cfg(feature = "native-profile")]
pub(crate) mod native_scroll_profile;
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::Arc,
};

const BLUE: Color32 = Color32::from_rgb(0, 128, 255);
const RED: Color32 = Color32::from_rgb(175, 40, 45);
const AMBER: Color32 = Color32::from_rgb(181, 118, 24);

struct HistoryEntry {
    document: DocumentHistory,
    selected: Option<u64>,
    insertion: Option<(f64, f64)>,
    editing_region: Option<u64>,
    active_editor: Option<(u64, Editor)>,
    active_area_editor: Option<(u64, String, Editor)>,
    text_cursor: Option<(u64, egui::text::CCursorRange)>,
    text_affinity: Option<(u64, rich_text_edit_map::RunAffinity)>,
}

enum DocumentHistory {
    Full(Box<CanvasDocument>),
    Region(Box<CanvasRegion>),
    NumericInputMode(Option<bool>),
    MatrixResultView {
        target: MatrixResultTarget,
        viewport: Option<MatrixResultViewport>,
    },
}
impl DocumentHistory {
    fn regions(&self) -> &[CanvasRegion] {
        match self {
            Self::Full(document) => &document.regions,
            Self::Region(region) => std::slice::from_ref(region.as_ref()),
            Self::NumericInputMode(_) | Self::MatrixResultView { .. } => &[],
        }
    }
}

#[derive(Clone, Copy)]
enum RegionAction {
    Edit,
    Copy,
    Cut,
    Duplicate,
    Delete,
    SetCalculationDisabled(bool),
}

enum Pending {
    New,
    Open(PathBuf),
    Close,
}

#[derive(Clone)]
struct PageSectionsDraft {
    sections: PageSections,
    next_id: u64,
}

#[derive(Clone)]
struct PlotDraft {
    id: u64,
    spec: PlotSpec,
    topology_changed: bool,
}

#[derive(Clone)]
struct SolveGuessDraft {
    name: String,
    value: String,
}

#[derive(Clone)]
struct SolveEqualityDraft {
    left: String,
    right: String,
}

#[derive(Clone)]
struct SolveDraft {
    method: SolveMethod,
    guesses: Vec<SolveGuessDraft>,
    equalities: Vec<SolveEqualityDraft>,
    output: String,
}

impl Default for SolveDraft {
    fn default() -> Self {
        Self {
            method: SolveMethod::Find,
            guesses: vec![
                SolveGuessDraft {
                    name: "p".into(),
                    value: "0".into(),
                },
                SolveGuessDraft {
                    name: "q".into(),
                    value: "0".into(),
                },
            ],
            equalities: vec![
                SolveEqualityDraft {
                    left: "p+q".into(),
                    right: "5".into(),
                },
                SolveEqualityDraft {
                    left: "p-q".into(),
                    right: "1".into(),
                },
            ],
            output: "answer".into(),
        }
    }
}

#[derive(Clone)]
struct CalculationInput {
    strings_use_origin: Option<bool>,
    source: Arc<str>,
    faults: HashMap<u64, String>,
    signature: String,
    region_ids: HashSet<u64>,
    areas: Arc<area_calculation_input::AreaIndex>,
    area_labelled: Arc<HashMap<u64, HashMap<String, PortableTree>>>,
}

struct CachedDisplay {
    format: MathFormat,
    value: Option<crate::result_display::FormattedResult>,
    legacy_initialized: bool,
    matrix: matrix_result_view::MatrixCache,
}
impl CachedDisplay {
    fn new(format: MathFormat) -> Self {
        Self {
            format,
            value: None,
            legacy_initialized: false,
            matrix: Default::default(),
        }
    }
    fn prepare_legacy(
        &mut self,
        format: MathFormat,
        result: Option<&RegionEvaluation>,
        saved: Option<&SavedCalculationRow>,
    ) {
        if self.legacy_initialized && self.format == format {
            return;
        }
        self.legacy_initialized = true;
        self.format = format;
        self.value = result
            .and_then(|r| r.display_value.as_ref().or(r.value.as_ref()))
            .map(|value| crate::result_display::format(value, &format))
            .or_else(|| saved.and_then(|row| numeric_cache::display(row, &format)))
            .or_else(|| {
                saved
                    .and_then(|r| r.display.as_deref())
                    .and_then(crate::result_display::FormattedResult::from_saved)
            });
    }
}

fn cached_display<'a>(
    cache: &'a mut HashMap<u64, CachedDisplay>,
    id: u64,
    format: MathFormat,
    result: Option<&RegionEvaluation>,
    saved: Option<&SavedCalculationRow>,
) -> Option<&'a crate::result_display::FormattedResult> {
    let entry = cache
        .entry(id)
        .or_insert_with(|| CachedDisplay::new(format));
    entry.prepare_legacy(format, result, saved);
    entry.value.as_ref()
}

fn unmeasured_height_bound(
    region: &CanvasRegion,
    result: Option<&RegionEvaluation>,
    cached: Option<&CachedDisplay>,
    has_saved: bool,
) -> Option<f32> {
    fn nodes(tree: &PortableTree) -> usize {
        1 + tree.children.iter().map(nodes).sum::<usize>()
    }
    match &region.content {
        CanvasContent::Plot(spec) => Some(spec.height + 100.),
        CanvasContent::Math(tree) => {
            let rows = if let Some(display) = cached {
                // Cached display trees can contain arbitrarily nested fractions;
                // leave these to exact measurement rather than underestimating.
                if display.value.is_some() || display.matrix.shape().is_some() {
                    return None;
                }
                0
            } else if let Some(value) =
                result.and_then(|row| row.display_value.as_ref().or(row.value.as_ref()))
            {
                match value {
                    mathcraft_compat::Value::Scalar { .. }
                    | mathcraft_compat::Value::String { .. } => 1,
                    mathcraft_compat::Value::Matrix { rows, .. } => (*rows).min(12),
                    mathcraft_compat::Value::RangeValues { elements } => elements.len().min(12),
                    mathcraft_compat::Value::Range { .. } => 3,
                    mathcraft_compat::Value::Symbolic { .. } => return None,
                }
            } else if has_saved {
                return None;
            } else {
                0
            };
            // Every structural node is allowed four full-size text lines, plus
            // a generous allowance for numeric/unit result rows. This is an
            // upper bound, not guessed measured geometry or saved print layout.
            let font = region.format.font_points.clamp(1., 200.) * 96. / 72.;
            Some(font * (4 * nodes(tree) + 32 * rows) as f32 + 100.)
        }
        _ => None,
    }
}

#[derive(Clone, Copy)]
enum CalculationCommand {
    Worksheet,
    Now,
    Automatic,
    Expression(u64),
}

fn calculatable(region: &CanvasRegion) -> bool {
    match &region.content {
        CanvasContent::Math(tree) => matches!(
            tree.kind,
            NodeKind::Binary(BinaryOp::Define | BinaryOp::GlobalDefine | BinaryOp::Evaluate)
                | NodeKind::SymbolicEvaluation { .. }
        ),
        CanvasContent::Plot(_) => true,
        CanvasContent::Opaque { kind, .. } => matches!(
            kind.as_str(),
            "math" | "solveblock" | "spec-table" | "specTable" | "Area"
        ),
        CanvasContent::Text(_) => false,
    }
}

fn calculation_enabled(region: &CanvasRegion) -> bool {
    !region.calculation_disabled && calculatable(region)
}

fn is_spec_table(region: &CanvasRegion) -> bool {
    matches!(&region.content, CanvasContent::Opaque {kind, ..} if matches!(kind.as_str(), "spec-table" | "specTable"))
}

fn native_alignment(region: &CanvasRegion, visuals: &NativeVisuals) -> bool {
    region
        .source_region
        .as_ref()
        .is_some_and(|id| visuals.regions.contains_key(id))
        || (is_spec_table(region) && visuals.region(region).is_some())
}

fn expects_display_result(region: &CanvasRegion) -> bool {
    match &region.content {
        CanvasContent::Math(tree) => match tree.kind {
            NodeKind::Binary(BinaryOp::Evaluate) | NodeKind::SymbolicEvaluation { .. } => true,
            NodeKind::Binary(BinaryOp::Define | BinaryOp::GlobalDefine) => tree
                .children
                .get(1)
                .is_some_and(|rhs| matches!(rhs.kind, NodeKind::Binary(BinaryOp::Evaluate))),
            _ => false,
        },
        CanvasContent::Opaque { kind, .. }
            if matches!(kind.as_str(), "spec-table" | "specTable") =>
        {
            false
        }
        CanvasContent::Opaque { kind, .. } if kind == "Area" => false,
        CanvasContent::Plot(_) | CanvasContent::Opaque { .. } => true,
        CanvasContent::Text(_) => false,
    }
}

pub struct WorksheetApp {
    document: CanvasDocument,
    native_visuals: Arc<NativeVisuals>,
    page_margins: [f32; 4],
    editors: HashMap<u64, Editor>,
    table_editors: HashMap<u64, spec_table_ui::TableEditor>,
    area_editors: HashMap<u64, area_ui::AreaEditor>,
    inline_plots: HashMap<u64, crate::plots::InlinePlotEdit>,
    plot_palette_cursors: HashMap<u64, usize>,
    plot_equation_floors: HashMap<u64, u64>,
    plot_widget_generations: HashMap<u64, u64>,
    retired_plot_widgets: Vec<egui::Id>,
    plot_input_queue: plot_input_queue::PlotInputQueue,
    plot_resize_start: Option<(u64, Box<PlotSpec>)>,
    area_saved: HashMap<u64, HashMap<String, SavedCalculationRow>>,
    area_visuals: HashMap<u64, Vec<mathcraft_document::native_visual::AreaChildVisualEntry>>,
    area_presentations: HashMap<u64, area_presentation::AreaPresentationCache>,
    area_prepared: HashMap<u64, (u64, area_presentation::PreparedAreaViews)>,
    area_epoch: u64,
    text_cursors: HashMap<u64, egui::text::CCursorRange>,
    text_composition: Option<plain_text_input::Composition>,
    rich_text: rich_text_input::State,
    paragraph_alignment: paragraph_alignment_ui::State,
    region_heights: HashMap<u64, f32>,
    region_widths: HashMap<u64, f32>,
    expand_right: bool,
    results: HashMap<u64, RegionEvaluation>,
    calculator: WorksheetCalculator,
    background: background::BackgroundState,
    saved_results: HashMap<u64, SavedCalculationRow>,
    area_results: HashMap<u64, HashMap<String, RegionEvaluation>>,
    area_pending: HashSet<(u64, String)>,
    display_cache: HashMap<u64, CachedDisplay>,
    matrix_view_sources: matrix_result_view::SourceFormats,
    matrix_view_drag: Option<matrix_result_view::Drag>,
    print_ui: print_ui::PrintUiState,
    saved_date: crate::page_fields::SavedDate,
    pending_regions: HashSet<u64>,
    uncommitted_math: HashSet<u64>,
    committed_input: Option<CalculationInput>,
    visible_regions: HashSet<u64>,
    visible_calculation_end: Option<u64>,
    plot_signature: String,
    last_executed_regions: Vec<String>,
    labelled_trees: HashMap<u64, PortableTree>,
    plot_results: HashMap<u64, Result<crate::plots::PlotData, String>>,
    automatic_calculation: bool,
    native_bin_oct_hex_mode: Option<bool>,
    calculation_pending: bool,
    page_sections_draft: Option<PageSectionsDraft>,
    plot_draft: Option<PlotDraft>,
    solve_draft: Option<SolveDraft>,
    ribbon: RibbonState,
    calculation_error: Option<String>,
    path: Option<PathBuf>,
    native_save_target: Option<native_save_ui::NativeTarget>,
    status: String,
    dirty: bool,
    selected: Option<u64>,
    editing_region: Option<u64>,
    insertion: Option<(f64, f64)>,
    page_break_request: Option<page_break_ui::QueuedPageBreak>,
    page_break_markers: page_break_ui::MarkerCache,
    text_mode: bool,
    focus_new: Option<u64>,
    undo: Vec<HistoryEntry>,
    redo: Vec<HistoryEntry>,
    dragging: Option<(u64, f64, f64)>,
    drag_preview: Option<(u64, f64, f64)>,
    drag_pointer_origin: Option<egui::Pos2>,
    pointer_history: Option<HistoryEntry>,
    pending: Option<Pending>,
    closing: bool,
    pub smoke: bool,
    pub capture: Option<PathBuf>,
    frames: usize,
    capture_sent: bool,
    zoom: f32,
    show_grid: bool,
    show_warnings: bool,
    use_major_grid: bool,
    show_shortcuts: bool,
    reveal: Option<(f64, f64)>,
    menu_resume: Option<u64>,
    last_title: String,
}
impl Default for WorksheetApp {
    fn default() -> Self {
        Self {
            document: CanvasDocument::default(),
            native_visuals: Arc::default(),
            page_margins: [crate::page::METRIC_MARGIN; 4],
            editors: HashMap::new(),
            table_editors: HashMap::new(),
            area_editors: HashMap::new(),
            inline_plots: HashMap::new(),
            plot_palette_cursors: HashMap::new(),
            plot_equation_floors: HashMap::new(),
            plot_widget_generations: HashMap::new(),
            retired_plot_widgets: Vec::new(),
            plot_input_queue: Default::default(),
            plot_resize_start: None,
            area_saved: HashMap::new(),
            area_visuals: HashMap::new(),
            area_presentations: HashMap::new(),
            area_prepared: HashMap::new(),
            area_epoch: 0,
            text_cursors: HashMap::new(),
            text_composition: None,
            rich_text: Default::default(),
            paragraph_alignment: Default::default(),
            region_heights: HashMap::new(),
            region_widths: HashMap::new(),
            expand_right: false,
            results: HashMap::new(),
            calculator: WorksheetCalculator::default(),
            background: background::BackgroundState::default(),
            saved_results: HashMap::new(),
            area_results: HashMap::new(),
            area_pending: HashSet::new(),
            display_cache: HashMap::new(),
            matrix_view_sources: Default::default(),
            matrix_view_drag: None,
            print_ui: print_ui::PrintUiState::default(),
            saved_date: crate::page_fields::SavedDate::default(),
            pending_regions: HashSet::new(),
            uncommitted_math: HashSet::new(),
            committed_input: None,
            visible_regions: HashSet::new(),
            visible_calculation_end: None,
            plot_signature: String::new(),
            last_executed_regions: Vec::new(),
            labelled_trees: HashMap::new(),
            plot_results: HashMap::new(),
            automatic_calculation: true,
            native_bin_oct_hex_mode: Some(false),
            calculation_pending: false,
            page_sections_draft: None,
            plot_draft: None,
            solve_draft: None,
            ribbon: RibbonState::default(),
            calculation_error: None,
            path: None,
            native_save_target: None,
            status: String::new(),
            dirty: false,
            selected: None,
            editing_region: None,
            insertion: Some((0., CanvasDocument::default().grid)),
            page_break_request: None,
            page_break_markers: Default::default(),
            text_mode: false,
            focus_new: None,
            undo: vec![],
            redo: vec![],
            dragging: None,
            drag_preview: None,
            drag_pointer_origin: None,
            pointer_history: None,
            pending: None,
            closing: false,
            smoke: false,
            capture: None,
            frames: 0,
            capture_sent: false,
            zoom: 1.,
            show_grid: true,
            show_warnings: true,
            use_major_grid: false,
            show_shortcuts: false,
            reveal: None,
            menu_resume: None,
            last_title: String::new(),
        }
    }
}
fn result_format_index(style: ResultFormat) -> u8 {
    match style {
        ResultFormat::General => 0,
        ResultFormat::Decimal => 1,
        ResultFormat::Scientific => 2,
        ResultFormat::Engineering => 3,
        ResultFormat::Percent => 4,
    }
}

fn plot_line_name(style: PlotLineStyle) -> &'static str {
    match style {
        PlotLineStyle::None => "없음",
        PlotLineStyle::Solid => "실선",
        PlotLineStyle::Dotted => "점선",
        PlotLineStyle::Dashed => "파선",
        PlotLineStyle::DashDot => "일점쇄선",
        PlotLineStyle::DashDotDot => "이점쇄선",
    }
}

fn plot_marker_name(marker: PlotMarker) -> &'static str {
    match marker {
        PlotMarker::None => "없음",
        PlotMarker::Cross => "가위표",
        PlotMarker::Plus => "더하기",
        PlotMarker::Circle => "원",
        PlotMarker::Square => "사각형",
        PlotMarker::Diamond => "마름모",
        PlotMarker::Point => "점",
        PlotMarker::Star => "별",
        PlotMarker::Triangle => "삼각형",
    }
}

fn plot_axis_ui(ui: &mut egui::Ui, title: &str, axis: &mut PlotAxisSpec) {
    ui.strong(title);
    ui.horizontal(|ui| {
        ui.label("레이블");
        ui.text_edit_singleline(&mut axis.label);
    });
    let mut explicit = axis.range.is_some();
    if ui.checkbox(&mut explicit, "범위 지정").changed() {
        axis.range = explicit.then_some(PlotRange {
            min: -10.,
            max: 10.,
        });
    }
    if let Some(range) = &mut axis.range {
        ui.horizontal(|ui| {
            ui.label("최소");
            ui.add(egui::DragValue::new(&mut range.min).speed(0.1));
            ui.label("최대");
            ui.add(egui::DragValue::new(&mut range.max).speed(0.1));
        });
    }
}

fn solve_expression(source: &str) -> Result<PortableTree, String> {
    if source.trim().is_empty() {
        return Err("풀이 식을 입력하세요".into());
    }
    let mut editor = Editor::new();
    formula::input_text(&mut editor, source);
    let mut tree = editor
        .to_portable()
        .map_err(|error| format!("풀이 식 `{source}`: {error}"))?;
    label_solve_expression(&mut tree, false);
    Ok(tree)
}

fn label_solve_expression(tree: &mut PortableTree, function_position: bool) {
    if let NodeKind::Identifier(name) = &tree.kind {
        let labels = if function_position {
            "FUNCTION"
        } else if mathcraft_core::unit(name).is_some() {
            "UNIT"
        } else if mathcraft_core::units::constant_lookup(name).is_some() {
            "CONSTANT"
        } else {
            "VARIABLE"
        };
        tree.kind = NodeKind::AnnotatedIdentifier {
            name: name.clone(),
            labels: labels.into(),
        };
    }
    let call = matches!(tree.kind, NodeKind::Call { .. });
    for (index, child) in tree.children.iter_mut().enumerate() {
        label_solve_expression(child, call && index == 0);
    }
}

fn solve_block_summary(xml: &str) -> (String, Vec<String>, usize, Option<String>) {
    let Ok(document) = roxmltree::Document::parse(xml) else {
        return ("풀이".into(), Vec::new(), 0, None);
    };
    let mut method = "풀이".to_string();
    let mut guesses = Vec::new();
    let mut constraints = 0;
    let mut output = None;
    for region in document
        .descendants()
        .filter(|node| node.is_element() && node.tag_name().name() == "region")
    {
        match region.attribute("solve-block-category") {
            Some("guess-value") => {
                if let Some(name) = region.descendants().find(|node| {
                    node.is_element()
                        && node.tag_name().name() == "id"
                        && node.attribute("labels") == Some("VARIABLE")
                }) {
                    guesses.push(name.text().unwrap_or("").to_string());
                }
            }
            Some("constraint") => constraints += 1,
            Some("solver") => {
                for name in region
                    .descendants()
                    .filter(|node| node.is_element() && node.tag_name().name() == "id")
                {
                    match name.attribute("labels") {
                        Some("KEYWORD") => method = name.text().unwrap_or("풀이").to_string(),
                        Some("VARIABLE") if output.is_none() => {
                            output = Some(name.text().unwrap_or("").to_string())
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    (method, guesses, constraints, output)
}
impl WorksheetApp {
    fn region_has_error(&self, id: u64) -> bool {
        self.plot_results.get(&id).is_some_and(|result| {
            result
                .as_ref()
                .is_ok_and(|data| data.has_failures() || data.has_unavailable())
        }) || (!self.pending_regions.contains(&id)
            && (self
                .saved_results
                .get(&id)
                .is_some_and(|row| row.error.is_some())
                || self.results.get(&id).is_some_and(|row| row.error.is_some())
                || self.plot_results.get(&id).is_some_and(Result::is_err)))
    }
    #[cfg(test)]
    fn display_result(
        &mut self,
        id: u64,
        format: MathFormat,
    ) -> Option<&crate::result_display::FormattedResult> {
        cached_display(
            &mut self.display_cache,
            id,
            format,
            self.results.get(&id),
            self.saved_results.get(&id),
        )
    }
    fn error_region_ids(&self) -> Vec<u64> {
        let mut regions: Vec<_> = self
            .document
            .regions
            .iter()
            .filter(|region| !region.calculation_disabled && self.region_has_error(region.id))
            .collect();
        regions.sort_by(|a, b| {
            a.top
                .total_cmp(&b.top)
                .then(a.left.total_cmp(&b.left))
                .then(a.id.cmp(&b.id))
        });
        regions.into_iter().map(|region| region.id).collect()
    }

    fn navigate_error(&mut self, forward: bool) {
        let ids = self.error_region_ids();
        if ids.is_empty() {
            self.status = "워크시트에 이동할 오류가 없습니다".into();
            return;
        }
        let next = match self
            .selected
            .and_then(|id| ids.iter().position(|x| *x == id))
        {
            Some(index) if forward => (index + 1) % ids.len(),
            Some(index) => (index + ids.len() - 1) % ids.len(),
            None if forward => 0,
            None => ids.len() - 1,
        };
        let id = ids[next];
        let Some(region) = self.document.regions.iter().find(|region| region.id == id) else {
            return;
        };
        self.selected = Some(id);
        self.editing_region = None;
        self.insertion = None;
        self.focus_new = None;
        self.reveal = Some((region.left, region.top));
        self.status = self
            .results
            .get(&id)
            .and_then(|result| result.error.as_ref())
            .map(|error| error.detail.clone())
            .or_else(|| {
                self.saved_results
                    .get(&id)
                    .and_then(|row| row.error.clone())
            })
            .or_else(|| {
                self.plot_results
                    .get(&id)
                    .and_then(|result| result.as_ref().err().cloned())
            })
            .or_else(|| {
                self.plot_results
                    .get(&id)?
                    .as_ref()
                    .ok()?
                    .traces
                    .iter()
                    .enumerate()
                    .find_map(|(index, trace)| {
                        trace
                            .diagnostic()
                            .map(|message| format!("곡선 {}: {message}", index + 1))
                    })
            })
            .unwrap_or_else(|| "계산 오류".into());
    }

    fn snapshot(&self) -> HistoryEntry {
        self.snapshot_with_document(DocumentHistory::Full(Box::new(CanvasDocument {
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
        })))
    }
    fn input_snapshot(&self) -> HistoryEntry {
        let active = self.editing_region.or(self.focus_new).or(self.selected);
        if self.insertion.is_none()
            && let Some(region) =
                active.and_then(|id| self.document.regions.iter().find(|r| r.id == id))
            && (matches!(
                region.content,
                CanvasContent::Math(_) | CanvasContent::Text(_)
            ) || matches!(&region.content, CanvasContent::Opaque {kind, ..} if matches!(kind.as_str(), "spec-table" | "specTable" | "Area")))
        {
            self.snapshot_with_document(DocumentHistory::Region(Box::new(region.clone())))
        } else {
            self.snapshot()
        }
    }
    fn math_command_snapshot(&self) -> HistoryEntry {
        // Ribbon commands mutate the selected math region. A pending focus
        // request can still name an older region until the next page frame.
        if let Some(region) = self.selected.and_then(|id| {
            self.document
                .regions
                .iter()
                .find(|r| r.id == id && matches!(r.content, CanvasContent::Math(_)))
        }) {
            self.snapshot_with_document(DocumentHistory::Region(Box::new(region.clone())))
        } else {
            self.snapshot()
        }
    }
    fn snapshot_with_document(&self, document: DocumentHistory) -> HistoryEntry {
        HistoryEntry {
            text_affinity: self
                .selected
                .and_then(|id| self.rich_text.affinity(id).map(|affinity| (id, affinity))),
            text_cursor: self
                .selected
                .and_then(|id| self.text_cursors.get(&id).map(|range| (id, *range))),
            selected: self.selected,
            insertion: self.insertion,
            editing_region: self.editing_region,
            active_editor: self.selected.and_then(|id| {
                self.editors
                    .get(&id)
                    .map(|e| (id, e.clone_without_history()))
            }),
            active_area_editor: self.selected.and_then(|id| {
                self.area_editors
                    .get(&id)?
                    .active_snapshot()
                    .map(|(key, editor)| (id, key, editor))
            }),
            document,
        }
    }
    fn remember(&mut self, before: HistoryEntry) {
        if self.undo.len() >= 50 {
            self.undo.remove(0);
        }
        self.undo.push(before);
        self.redo.clear();
        self.dirty = true;
    }
    fn restore(&mut self, state: HistoryEntry) {
        self.cancel_matrix_view_drag();
        if let DocumentHistory::MatrixResultView { target, viewport } = &state.document {
            match self.set_matrix_result_view(target.owner_id, *viewport) {
                Ok(_) => self.dirty = true,
                Err(error) => self.status = error,
            }
            return;
        }
        if let DocumentHistory::NumericInputMode(mode) = &state.document {
            self.document.bin_oct_hex_mode = *mode;
            self.dirty = true;
            return;
        }
        self.text_composition = None;
        self.rich_text.invalidate_target();
        self.paragraph_alignment.invalidate();
        if let DocumentHistory::Region(region) = &state.document {
            self.rich_text.editors.remove(&region.id);
            self.rich_text.authored_editors.remove(&region.id);
        } else {
            self.rich_text = Default::default();
            self.paragraph_alignment.invalidate();
        }
        self.plot_resize_start = None;
        self.page_break_request = None;
        self.page_break_markers = Default::default();
        if state
            .document
            .regions()
            .iter()
            .any(|r| r.calculation_disabled)
        {
            self.persist_calculation_cache();
            if let Some(cache) = &self.document.calculation_cache {
                for mut row in
                    cache
                        .rows
                        .iter()
                        .filter(|row| {
                            state.document.regions().iter().any(|region| {
                                region.id == row.region_id && region.calculation_disabled
                            })
                        })
                        .cloned()
                {
                    row.status = SavedCalculationStatus::Pending;
                    self.saved_results.insert(row.region_id, row);
                }
            }
        }
        let restored_region = match &state.document {
            DocumentHistory::Region(region) => Some(region.id),
            DocumentHistory::Full(_) => None,
            DocumentHistory::NumericInputMode(_) | DocumentHistory::MatrixResultView { .. } => {
                unreachable!("handled before source restore")
            }
        };
        match state.document {
            DocumentHistory::Full(mut document) => {
                document.native_archive = self.document.native_archive.take();
                self.document = *document;
                self.editors.clear();
                self.text_composition = None;
                self.text_cursors.clear();
                self.region_heights.clear();
                self.region_widths.clear();
            }
            DocumentHistory::Region(region) => {
                let id = region.id;
                let Some(current) = self.document.regions.iter_mut().find(|r| r.id == id) else {
                    self.status = "실행 취소할 영역을 찾을 수 없습니다".into();
                    return;
                };
                *current = *region;
                self.editors.remove(&id);
                self.text_cursors.remove(&id);
                self.region_heights.remove(&id);
                self.region_widths.remove(&id);
            }
            DocumentHistory::NumericInputMode(_) | DocumentHistory::MatrixResultView { .. } => {
                unreachable!("handled before source restore")
            }
        }
        for (id, edit) in &self.inline_plots {
            let cursor = self.plot_palette_cursors.entry(*id).or_default();
            *cursor = (*cursor).max(edit.palette_cursor());
            let floor = self.plot_equation_floors.entry(*id).or_default();
            *floor = (*floor).max(edit.equation_id_floor());
        }
        let existing: HashSet<_> = self.document.regions.iter().map(|r| r.id).collect();
        self.plot_palette_cursors
            .retain(|id, _| existing.contains(id));
        self.retire_plot_widgets(restored_region);
        self.retain_plot_equation_floors();
        if let Some(id) = restored_region {
            self.inline_plots.remove(&id);
            self.table_editors.remove(&id);
            self.area_editors.remove(&id);
            self.area_presentations.remove(&id);
            self.area_prepared.remove(&id);
        } else {
            self.inline_plots.clear();
            self.table_editors.clear();
            self.area_editors.clear();
            self.area_presentations.clear();
            self.area_prepared.clear();
        }
        let tables: Vec<_> = self
            .document
            .regions
            .iter()
            .filter(|r| restored_region.is_none_or(|id| r.id == id) && is_spec_table(r))
            .map(|r| r.id)
            .collect();
        for id in tables {
            self.refresh_table_visual(id);
        }
        let texts: Vec<_> = self
            .document
            .regions
            .iter()
            .filter(|r| {
                matches!(r.content, CanvasContent::Text(_))
                    && restored_region.is_none_or(|id| r.id == id)
            })
            .map(|r| r.id)
            .collect();
        for id in texts {
            self.refresh_rich_text_visual(id);
        }
        if let Some((id, cursor)) = state.text_cursor {
            self.text_cursors.insert(id, cursor);
        }
        if let Some((id, affinity)) = state.text_affinity {
            self.restore_rich_text_affinity(id, &affinity);
        }
        self.selected = state
            .selected
            .filter(|id| self.document.regions.iter().any(|r| r.id == *id));
        self.insertion = state.insertion;
        self.editing_region = state.editing_region.filter(|id| Some(*id) == self.selected);
        self.focus_new = self.editing_region;
        self.dragging = None;
        self.drag_preview = None;
        self.drag_pointer_origin = None;
        if let Some((id, editor)) = state.active_editor
            && self.document.regions.iter().any(|r| r.id == id)
        {
            self.editors.insert(id, editor);
        }
        if let Some((id, key, editor)) = state.active_area_editor
            && let Some(CanvasContent::Opaque { xml, kind }) = self
                .document
                .regions
                .iter()
                .find(|r| r.id == id)
                .map(|r| &r.content)
            && kind == "Area"
            && let Ok(mut area) = area_ui::AreaEditor::new(xml)
            && area.restore_active_snapshot(&key, editor).is_ok()
        {
            self.area_editors.insert(id, area);
        }
        self.dirty = true;
        self.uncommitted_math.clear();
        if let Some(id) = self.editing_region.filter(|id| {
            self.document.regions.iter().any(|r| {
                r.id == *id
                    && (matches!(r.content, CanvasContent::Math(_))
                        || self
                            .area_editors
                            .get(id)
                            .is_some_and(|area| area.has_draft_changes()))
            })
        }) {
            self.stage_math_edit(id, false);
        } else {
            self.calculation_changed(false);
        }
    }
    fn undo(&mut self) {
        self.cancel_matrix_view_drag();
        self.retire_authored_select_all_inputs();
        if self
            .editing_region
            .and_then(|id| self.inline_plots.get_mut(&id))
            .is_some_and(|state| state.undo_structure())
        {
            return;
        }
        if let Some(state) = self
            .editing_region
            .and_then(|id| self.inline_plots.get_mut(&id))
            && state.has_changes()
        {
            state.cancel();
            return;
        }
        if self
            .editing_region
            .and_then(|id| self.table_editors.get_mut(&id))
            .is_some_and(|editor| editor.history(false))
        {
            return;
        }
        if let Some(previous) = self.undo.pop() {
            self.redo.push(self.inverse_snapshot(&previous.document));
            self.restore(previous);
        }
    }
    fn redo(&mut self) {
        self.cancel_matrix_view_drag();
        self.retire_authored_select_all_inputs();
        if self
            .editing_region
            .and_then(|id| self.inline_plots.get_mut(&id))
            .is_some_and(|state| state.redo_structure())
        {
            return;
        }
        if self
            .editing_region
            .and_then(|id| self.table_editors.get_mut(&id))
            .is_some_and(|editor| editor.history(true))
        {
            return;
        }
        if let Some(next) = self.redo.pop() {
            self.undo.push(self.inverse_snapshot(&next.document));
            self.restore(next);
        }
    }
    fn inverse_snapshot(&self, document: &DocumentHistory) -> HistoryEntry {
        if let DocumentHistory::MatrixResultView { target, .. } = document {
            return self.snapshot_with_document(DocumentHistory::MatrixResultView {
                target: target.clone(),
                viewport: self.document.matrix_result_views.get(target).copied(),
            });
        }
        if matches!(document, DocumentHistory::NumericInputMode(_)) {
            return self.snapshot_with_document(DocumentHistory::NumericInputMode(
                self.document.bin_oct_hex_mode,
            ));
        }
        if let DocumentHistory::Region(region) = document
            && let Some(current) = self.document.regions.iter().find(|r| r.id == region.id)
        {
            self.snapshot_with_document(DocumentHistory::Region(Box::new(current.clone())))
        } else {
            self.snapshot()
        }
    }
    fn request(&mut self, action: Pending) {
        if self.dirty
            || self
                .inline_plots
                .values()
                .any(crate::plots::InlinePlotEdit::has_changes)
        {
            self.pending = Some(action);
        } else {
            self.apply(action);
        }
    }
    fn apply(&mut self, action: Pending) {
        match action {
            Pending::New => {
                self.retire_plot_widgets(None);
                let retired_widgets = std::mem::take(&mut self.retired_plot_widgets);
                let mut background = std::mem::take(&mut self.background);
                background.reset();
                let view = (
                    self.zoom,
                    self.show_grid,
                    self.automatic_calculation,
                    self.show_warnings,
                );
                *self = Self::default();
                self.retired_plot_widgets = retired_widgets;
                self.background = background;
                (
                    self.zoom,
                    self.show_grid,
                    self.automatic_calculation,
                    self.show_warnings,
                ) = view;
            }
            Pending::Open(path) => self.open(&path),
            Pending::Close => self.closing = true,
        }
    }
    pub fn open(&mut self, path: &Path) {
        self.page_break_request = None;
        self.page_break_markers = Default::default();
        #[cfg(test)]
        let load_timing = crate::frame_profile::scope(crate::frame_profile::Phase::OpenLoad);
        let result = if path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("mcdx"))
        {
            import_native(path).and_then(|(document, package)| {
                // Capture revision before clearing/installing any app state.
                let target = native_save_ui::NativeTarget::from_open(path, &document)?;
                Ok((document, Some(package), Some(target)))
            })
        } else {
            CanvasDocument::load(path).map(|document| (document, None, None))
        };
        #[cfg(test)]
        drop(load_timing);
        match result {
            Ok((document, mut native_package, native_target)) => {
                let fresh_native = native_package.is_some();
                #[cfg(test)]
                let visual_timing =
                    crate::frame_profile::scope(crate::frame_profile::Phase::OpenVisuals);
                self.background.reset();
                self.saved_date =
                    crate::page_fields::SavedDate::from_path(path).unwrap_or_default();
                self.page_margins = crate::page::standard_margins(&document);
                self.native_visuals = Arc::default();
                let mut visual_warning = None;
                #[cfg(test)]
                let mut archive_timing = Some(crate::frame_profile::scope(
                    crate::frame_profile::Phase::VisualArchive,
                ));
                if native_package.is_none() {
                    native_package = document
                        .native_archive
                        .as_deref()
                        .and_then(|bytes| NativePackage::from_bytes(bytes).ok());
                }
                #[cfg(test)]
                drop(archive_timing.take());
                if let Some(package) = native_package.as_ref() {
                    #[cfg(test)]
                    let decode_timing =
                        crate::frame_profile::scope(crate::frame_profile::Phase::VisualDecode);
                    match NativeVisuals::from_package(package) {
                        Ok(mut visuals) => {
                            // Positively parsed plots use their structural
                            // source/curve renderer, outside the rich-text
                            // native visual decoder's supported object set.
                            for region in &document.regions {
                                if matches!(region.content, CanvasContent::Plot(_))
                                    && let Some(id) = &region.source_region
                                {
                                    visuals.regions.remove(id);
                                }
                            }
                            let failed = visuals.regions.values().filter(|v| v.is_err()).count();
                            if failed > 0 {
                                visual_warning =
                                    Some(format!(" · 원본 서식 {failed}개 영역 지원 필요"));
                            }
                            self.native_visuals = Arc::new(visuals);
                        }
                        Err(error) => visual_warning = Some(format!(" · 원본 서식: {error}")),
                    }
                    #[cfg(test)]
                    drop(decode_timing);
                    #[cfg(test)]
                    let _settings_timing =
                        crate::frame_profile::scope(crate::frame_profile::Phase::VisualSettings);
                    if let Ok(Some(source)) =
                        package.original_xml_part("mathcad/settings/presentation.xml")
                    {
                        if document.page_setup.is_none()
                            && let Some((_, margins)) = crate::page::source_settings(&source)
                        {
                            self.page_margins = margins;
                        }
                        (self.show_grid, self.use_major_grid) = crate::page::view_settings(&source);
                    }
                }
                #[cfg(test)]
                drop(archive_timing);
                #[cfg(test)]
                drop(visual_timing);
                #[cfg(test)]
                let _restore_timing =
                    crate::frame_profile::scope(crate::frame_profile::Phase::OpenRestore);
                self.retire_plot_widgets(None);
                self.native_bin_oct_hex_mode = if let Some(package) = native_package.as_ref() {
                    mathcraft_document::native_calculation::bin_oct_hex_mode(package).ok()
                } else if document.native_archive.is_none() {
                    Some(false)
                } else {
                    None
                };
                self.document = document;
                self.matrix_view_drag = None;
                self.matrix_view_sources = matrix_result_view::SourceFormats::from_document(
                    &self.document,
                    native_package.as_ref(),
                );
                self.rich_text = Default::default();
                self.paragraph_alignment.invalidate();
                self.editors.clear();
                self.table_editors.clear();
                self.area_editors.clear();
                self.inline_plots.clear();
                self.plot_palette_cursors.clear();
                self.plot_equation_floors.clear();
                self.plot_widget_generations.clear();
                self.plot_input_queue.clear();
                self.plot_resize_start = None;
                self.area_presentations.clear();
                self.area_prepared.clear();
                self.area_visuals.clear();
                #[cfg(test)]
                let table_timing =
                    crate::frame_profile::scope(crate::frame_profile::Phase::OpenTables);
                let tables: Vec<_> = self.document.regions.iter().filter(|r| matches!(&r.content, CanvasContent::Opaque { kind, .. } if matches!(kind.as_str(), "spec-table" | "specTable"))).map(|r| r.id).collect();
                for id in tables {
                    // Only a fresh import proves these canvas bytes/defaults
                    // came from this package. The successful native visual has
                    // already decoded that whole table with the same parser.
                    // Edited/own/local/error tables keep their normal refresh;
                    // standalone namespace overhead still obeys its 8 MiB cap.
                    let freshly_decoded=fresh_native && self.document.regions.iter().find(|r|r.id==id).is_some_and(|region| {
                        matches!(&region.content,CanvasContent::Opaque{xml,kind} if kind=="spec-table" && xml.len()<=8*1024*1024)
                            && self.native_visuals.region(region).is_some_and(Result::is_ok)
                    });
                    if !freshly_decoded {
                        self.refresh_table_visual(id);
                    }
                }
                #[cfg(test)]
                drop(table_timing);
                self.text_composition = None;
                self.text_cursors.clear();
                let rich_ids: Vec<_> = self
                    .document
                    .regions
                    .iter()
                    .filter(|r| r.rich_text.is_some() || r.authored_text.is_some())
                    .map(|r| r.id)
                    .collect();
                for id in rich_ids {
                    self.refresh_rich_text_visual(id);
                }
                self.region_heights.clear();
                self.region_widths.clear();
                self.undo.clear();
                self.redo.clear();
                self.results.clear();
                self.dirty = false;
                self.selected = None;
                self.editing_region = None;
                self.focus_new = None;
                self.dragging = None;
                self.drag_preview = None;
                self.drag_pointer_origin = None;
                self.page_sections_draft = None;
                self.insertion = None;
                self.path = Some(path.into());
                self.native_save_target = native_target;
                self.status = format!("Opened {}", path.display());
                if let Some(package) = native_package.as_ref() {
                    self.restore_saved_calculations_for_source(Some(package), fresh_native);
                } else {
                    self.restore_saved_calculations();
                }
                if let Some(warning) = visual_warning {
                    self.status.push_str(&warning);
                }
            }
            Err(e) => self.status = format!("Could not open: {e}"),
        }
    }
    fn sync_editors(&mut self) -> Result<(), String> {
        self.cancel_matrix_view_drag();
        if self.commit_plot_edits(None)? {
            self.calculation_changed(false);
        }
        self.commit_table_edits(None)?;
        self.commit_area_edits(None)?;
        for region in &mut self.document.regions {
            if let Some(editor) = self.editors.get(&region.id) {
                region.content = CanvasContent::Math(
                    editor
                        .to_portable()
                        .map_err(|e| format!("Region {}: {e:?}", region.id))?,
                );
            }
        }
        Ok(())
    }
    fn save(&mut self, save_as: bool) -> bool {
        let mut prompts = native_save_ui::SystemSavePrompts;
        self.save_with_prompts(save_as, &mut prompts)
    }
    fn accept_calculation_update(&mut self, update: CalculationUpdate) {
        self.cancel_matrix_view_drag();
        self.display_cache.clear();
        self.area_epoch = self.area_epoch.wrapping_add(1);
        let area_index = self
            .committed_input
            .as_ref()
            .map(|input| input.areas.clone());
        self.area_pending.clear();
        if let Some(index) = &area_index {
            for id in &update.pending_region_ids {
                if let Some(address) = index.children.get(id) {
                    self.area_pending
                        .insert((address.owner, address.key.clone()));
                }
            }
        }
        let plot_ids: HashSet<_> = self
            .document
            .regions
            .iter()
            .filter(|region| {
                !region.calculation_disabled && matches!(region.content, CanvasContent::Plot(_))
            })
            .map(|region| region.id)
            .collect();
        let pending_plots: Vec<_> = self
            .pending_regions
            .iter()
            .copied()
            .filter(|id| plot_ids.contains(id))
            .collect();
        self.pending_regions = update
            .pending_region_ids
            .iter()
            .filter_map(|id| id.parse().ok().or_else(|| area_index.as_ref()?.owner(id)))
            .collect();
        self.pending_regions.extend(pending_plots);
        self.pending_regions
            .extend(self.uncommitted_math.iter().copied());
        for row in &update.rows {
            if let Ok(id) = row.region_id.parse::<u64>()
                && !self.pending_regions.contains(&id)
            {
                self.saved_results.remove(&id);
            }
        }
        self.last_executed_regions
            .extend(update.executed_region_ids);
        self.area_results.clear();
        for row in &update.rows {
            if let Some(address) = area_index
                .as_ref()
                .and_then(|index| index.children.get(&row.region_id))
            {
                self.area_results
                    .entry(address.owner)
                    .or_default()
                    .insert(address.key.clone(), row.clone());
            }
        }
        self.results = update
            .rows
            .into_iter()
            .filter_map(|row| row.region_id.parse().ok().map(|id| (id, row)))
            .collect();
    }

    fn recalculate(&mut self) {
        self.calculate_command(CalculationCommand::Worksheet);
    }

    fn set_selected_calculation_disabled(&mut self, disabled: bool) {
        let Some(id) = self.selected else {
            return;
        };
        if !self.document.regions.iter().any(|r| {
            r.id == id && r.supports_calculation_toggle() && r.calculation_disabled != disabled
        }) {
            return;
        }
        let before = self.snapshot();
        // Retain the last presentation while removing its executable row from
        // the session. This is an intentionally pending, dimmed result.
        self.persist_calculation_cache();
        if let Some(mut row) = self
            .document
            .calculation_cache
            .as_ref()
            .and_then(|c| c.rows.iter().find(|r| r.region_id == id))
            .cloned()
        {
            row.status = SavedCalculationStatus::Pending;
            self.saved_results.insert(id, row);
        }
        let region = self
            .document
            .regions
            .iter_mut()
            .find(|r| r.id == id)
            .unwrap();
        region.calculation_disabled = disabled;
        region.calculation_enabled_override = !disabled;
        self.pending_regions.remove(&id);
        self.display_cache.remove(&id);
        self.remember(before);
        self.calculation_changed(false);
        self.status = if disabled {
            "영역 계산 비활성화"
        } else {
            "영역 계산 활성화"
        }
        .into();
    }

    fn calculate_command(&mut self, command: CalculationCommand) {
        self.calculate_command_with_active(command, None);
    }

    fn calculate_command_with_active(&mut self, command: CalculationCommand, forced: Option<u64>) {
        let area_active = self.editing_region.or(self.selected).and_then(|id| {
            self.area_editors
                .get(&id)?
                .active_child()
                .map(|key| (id, key.to_owned()))
        });
        if let CalculationCommand::Expression(id) = command
            && let Some((owner, key)) = &area_active
            && *owner == id
            && self.area_editors[owner].math_sources().any(|child| {
                area_ui::child_key(&child.region_path) == *key && child.calculation_disabled
            })
        {
            self.last_executed_regions.clear();
            return;
        }
        // Cell input is a draft until an explicit calculation command commits it.
        // Automatic calculation never commits a different cell's pending text.
        if !matches!(command, CalculationCommand::Automatic)
            && let Err(error) = self.commit_plot_edits(None)
        {
            self.status = error;
            return;
        }
        if !matches!(command, CalculationCommand::Automatic)
            && let Err(error) = self.commit_table_edits(None)
        {
            self.status = error;
            return;
        }
        if !matches!(command, CalculationCommand::Automatic)
            && let Err(error) = self.commit_area_edits(None)
        {
            self.status = error;
            return;
        }
        self.last_executed_regions.clear();
        if let CalculationCommand::Expression(id) = command
            && self
                .document
                .regions
                .iter()
                .any(|r| r.id == id && r.calculation_disabled)
        {
            return;
        }
        if matches!(command, CalculationCommand::Now) && self.visible_calculation_end.is_none() {
            return;
        }
        if matches!(command, CalculationCommand::Now)
            && self.document.active_calculation_signature() == self.plot_signature
            && self
                .editing_region
                .filter(|id| {
                    self.document
                        .regions
                        .iter()
                        .any(|r| r.id == *id && !r.calculation_disabled)
                })
                .is_none_or(|id| !self.visible_regions.contains(&id))
            && let Some(end) = self
                .visible_calculation_end
                .and_then(|id| self.document.regions.iter().find(|r| r.id == id))
            && !self.document.regions.iter().any(|r| {
                self.pending_regions.contains(&r.id)
                    && (r.top < end.top || (r.top == end.top && r.left <= end.left))
            })
        {
            // Saved synchronized presentations are not a reason to run input
            // again merely because the live evaluator has not been initialized.
            return;
        }
        if matches!(
            command,
            CalculationCommand::Worksheet | CalculationCommand::Expression(_)
        ) || (matches!(command, CalculationCommand::Now)
            && self
                .editing_region
                .is_some_and(|id| self.visible_regions.contains(&id)))
        {
            self.uncommitted_math.clear();
        }
        self.calculation_error = None;
        let input = match self.refresh_calculation_input() {
            Ok(input) => input,
            Err(error) => {
                self.calculation_error = Some(error);
                return;
            }
        };
        let active_target = |id| {
            if let Some((owner, key)) = &area_active
                && *owner == id
            {
                input.areas.child_target(id, key)
            } else {
                Some(input.areas.target(id))
            }
        };
        let through = match command {
            CalculationCommand::Now => self.visible_calculation_end,
            CalculationCommand::Expression(id) => Some(id),
            CalculationCommand::Worksheet | CalculationCommand::Automatic => self
                .document
                .regions
                .iter()
                .filter(|region| {
                    calculation_enabled(region)
                        && input.region_ids.contains(&region.id)
                        && !matches!(region.content, CanvasContent::Plot(_))
                })
                .max_by(|a, b| a.top.total_cmp(&b.top).then(a.left.total_cmp(&b.left)))
                .map(|region| region.id),
        };
        // Prime forces a visible active math item, then processes the dirty visible prefix.
        let active = forced
            .or(match command {
                CalculationCommand::Expression(id) => Some(id),
                CalculationCommand::Now => self
                    .editing_region
                    .filter(|id| self.visible_regions.contains(id)),
                _ => None,
            })
            .filter(|id| {
                self.document.regions.iter().any(|r| {
                    r.id == *id
                        && input.region_ids.contains(&r.id)
                        && calculation_enabled(r)
                        && !matches!(r.content, CanvasContent::Plot(_))
                })
            });
        let mut requests = Vec::new();
        if matches!(command, CalculationCommand::Worksheet) {
            requests.push(CalculationRequest::Worksheet);
        } else {
            if let Some(id) = active
                && let Some(target) = active_target(id)
            {
                requests.push(CalculationRequest::ActiveRegion(target));
            }
            // Plot nodes are not engine rows. Select the final math item at/before the visible bound.
            if let Some(bound) =
                through.and_then(|id| self.document.regions.iter().find(|r| r.id == id))
            {
                let end = self
                    .document
                    .regions
                    .iter()
                    .filter(|r| {
                        calculation_enabled(r)
                            && input.region_ids.contains(&r.id)
                            && !matches!(r.content, CanvasContent::Plot(_))
                    })
                    .filter(|r| r.top < bound.top || (r.top == bound.top && r.left <= bound.left))
                    .max_by(|a, b| a.top.total_cmp(&b.top).then(a.left.total_cmp(&b.left)));
                if let Some(end) = end {
                    let target = if matches!(command,CalculationCommand::Expression(id) if id==end.id)
                    {
                        active_target(end.id)
                    } else {
                        Some(input.areas.target(end.id))
                    };
                    if let Some(target) = target {
                        requests.push(CalculationRequest::ThroughRegion(target));
                    }
                }
            }
        }
        if requests.is_empty() {
            requests.push(CalculationRequest::Prerequisites);
        }
        if let Some(active) =
            active.and_then(|id| self.document.regions.iter().find(|r| r.id == id))
        {
            let global = matches!(&active.content, CanvasContent::Opaque { xml, .. } if xml.contains("globalDefine"));
            self.mark_regions_pending(
                (self
                    .document
                    .regions
                    .iter()
                    .filter(|r| {
                        !r.calculation_disabled && matches!(r.content, CanvasContent::Plot(_))
                    })
                    .filter(|r| global || (r.top, r.left) >= (active.top, active.left))
                    .map(|r| r.id))
                .collect::<Vec<_>>(),
            );
        }
        if self.background.enabled() {
            let plots = self.background_plot_jobs(command, through);
            self.dispatch_background(input, requests, plots, command);
            return;
        }
        for request in requests {
            match self.calculator.calculate(request) {
                Ok(update) => self.accept_calculation_update(update),
                Err(error) => {
                    self.calculation_error = Some(error);
                    return;
                }
            }
        }
        let end_position = through
            .and_then(|id| self.document.regions.iter().find(|r| r.id == id))
            .map(|r| (r.top, r.left));
        for region in &self.document.regions {
            if !region.calculation_disabled
                && let CanvasContent::Plot(spec) = &region.content
            {
                let in_range = matches!(
                    command,
                    CalculationCommand::Worksheet | CalculationCommand::Automatic
                ) || end_position.is_some_and(|(top, left)| {
                    region.top < top || (region.top == top && region.left <= left)
                });
                let force = matches!(command, CalculationCommand::Worksheet)
                    || (matches!(command, CalculationCommand::Now)
                        && self.editing_region == Some(region.id)
                        && self.visible_regions.contains(&region.id));
                if !in_range || (!force && !self.pending_regions.contains(&region.id)) {
                    continue;
                }
                let result = self
                    .calculator
                    .context_before(region.left, region.top)
                    .and_then(|context| crate::plots::evaluate_in_context(spec, &context));
                self.plot_results.insert(region.id, result);
                if let Some(source) = &region.source_region {
                    Arc::make_mut(&mut self.native_visuals)
                        .regions
                        .remove(source);
                }
                self.pending_regions.remove(&region.id);
                self.saved_results.remove(&region.id);
                self.last_executed_regions.push(region.id.to_string());
            }
        }
        self.apply_calculation_faults(
            input.faults,
            matches!(command, CalculationCommand::Worksheet),
        );
    }

    fn apply_calculation_faults(&mut self, faults: HashMap<u64, String>, whole: bool) {
        let faults = faults
            .into_iter()
            .filter(|(id, _)| {
                !self.pending_regions.contains(id) && (whole || self.results.contains_key(id))
            })
            .collect::<HashMap<_, _>>();
        for (id, detail) in faults {
            if let Some(row) = self.results.get_mut(&id) {
                if let Some(error) = &mut row.error {
                    error.detail = detail;
                }
                row.value = None;
                row.display_value = None;
            } else if let Some(region) = self.document.regions.iter().find(|r| r.id == id) {
                self.results.insert(
                    id,
                    RegionEvaluation {
                        region_id: id.to_string(),
                        result_ref: None,
                        left: region.left,
                        top: region.top,
                        value: None,
                        display_value: None,
                        error: Some(mathcraft_compat::Fault {
                            code: "incomplete".into(),
                            detail,
                            previous_region: None,
                            unsupported: true,
                        }),
                        redefinition_warning: false,
                        matrix_origin: None,
                    },
                );
            }
        }
        self.calculation_pending = !self.pending_regions.is_empty();
    }

    fn calculation_changed(&mut self, force: bool) {
        self.uncommitted_math.clear();
        // A disabled item remains editable, but changing only that item must
        // not wake even a cold evaluator restored from saved presentations.
        if self.editing_region.or(self.selected).is_some_and(|id| {
            self.document
                .regions
                .iter()
                .any(|r| r.id == id && r.calculation_disabled)
        }) && self.document.active_calculation_signature() == self.plot_signature
        {
            self.last_executed_regions.clear();
            return;
        }
        if force && let Some(id) = self.editing_region.or(self.selected) {
            let command = if self.automatic_calculation {
                CalculationCommand::Automatic
            } else {
                CalculationCommand::Expression(id)
            };
            self.calculate_command_with_active(command, Some(id));
        } else if self.automatic_calculation {
            self.calculate_command(CalculationCommand::Automatic);
        } else {
            self.last_executed_regions.clear();
            match self.refresh_calculation_input() {
                Ok(input) if self.background.enabled() => {
                    self.dispatch_background(input, vec![], vec![], CalculationCommand::Automatic)
                }
                Ok(_) => self.calculation_error = None,
                Err(error) => self.calculation_error = Some(error),
            }
        }
    }

    fn persist_calculation_cache(&mut self) {
        let mut rows = Vec::new();
        for region in &self.document.regions {
            if matches!(&region.content,CanvasContent::Opaque{kind,..}if kind=="Area")
                && self
                    .results
                    .get(&region.id)
                    .is_none_or(|row| row.error.is_none())
            {
                let empty_live = HashMap::new();
                let empty_saved = HashMap::new();
                if let Ok(row) = area_cache::save_parent(
                    region,
                    self.area_results.get(&region.id).unwrap_or(&empty_live),
                    self.area_saved.get(&region.id).unwrap_or(&empty_saved),
                    &self.area_pending,
                    !self.uncommitted_math.is_empty(),
                ) {
                    rows.push(row);
                    continue;
                }
            }
            if !calculatable(region) {
                continue;
            }
            let mut row =
                self.saved_results
                    .get(&region.id)
                    .cloned()
                    .unwrap_or(SavedCalculationRow {
                        region_id: region.id,
                        status: SavedCalculationStatus::Pending,
                        display: None,
                        error: None,
                        warnings: Vec::new(),
                        symbolic_mathml: None,
                        numeric_source: None,
                    });
            if let Some(result) = self.results.get(&region.id) {
                if let Some(error) = &result.error {
                    row.numeric_source = None;
                    row.error = Some(error.detail.clone());
                    row.display = None;
                    row.symbolic_mathml = None;
                } else if let Some(value) = result.display_value.as_ref().or(result.value.as_ref())
                {
                    row.numeric_source = numeric_cache::source(value);
                    row.display =
                        Some(crate::result_display::format(value, &region.format).to_saved());
                    row.error = None;
                }
                if result.redefinition_warning {
                    row.warnings = vec!["이 이름은 앞선 정의를 다시 정의합니다.".into()];
                }
            }
            if !self.results.contains_key(&region.id)
                && let Some(display) = numeric_cache::display(&row, &region.format)
            {
                row.display = Some(display.to_saved());
            }
            if let Some(plot) = self.plot_results.get(&region.id) {
                match plot {
                    Ok(data) => {
                        row.display = Some(data.to_saved());
                        row.error = None;
                    }
                    Err(error) => {
                        row.error = Some(error.clone());
                        row.display = None;
                    }
                }
            }
            row.status = if region.calculation_disabled
                || self.pending_regions.contains(&region.id)
                || !self.uncommitted_math.is_empty()
            {
                SavedCalculationStatus::Pending
            } else {
                SavedCalculationStatus::Synchronized
            };
            rows.push(row);
        }
        self.document.calculation_cache = Some(SavedCalculationCache {
            source_signature: self.document.calculation_signature(),
            rows,
        });
    }

    fn restore_saved_calculations(&mut self) {
        self.restore_saved_calculations_with_package(None);
    }

    fn restore_saved_calculations_with_package(&mut self, native_package: Option<&NativePackage>) {
        self.restore_saved_calculations_for_source(native_package, false);
    }

    fn restore_saved_calculations_for_source(
        &mut self,
        native_package: Option<&NativePackage>,
        fresh_native: bool,
    ) {
        self.uncommitted_math.clear();
        self.committed_input = None;
        self.display_cache.clear();
        self.calculator = WorksheetCalculator::default();
        self.results.clear();
        self.saved_results.clear();
        self.area_results.clear();
        self.area_pending.clear();
        self.area_saved.clear();
        self.area_prepared.clear();
        self.area_epoch = self.area_epoch.wrapping_add(1);
        self.plot_results.clear();
        self.labelled_trees.clear();
        self.background.preparation = Default::default();
        self.pending_regions.clear();
        self.visible_regions.clear();
        self.visible_calculation_end = None;
        self.last_executed_regions.clear();
        #[cfg(test)]
        let signature_timing =
            crate::frame_profile::scope(crate::frame_profile::Phase::RestoreSignature);
        self.plot_signature = self.document.active_calculation_signature();
        #[cfg(test)]
        drop(signature_timing);
        self.calculation_error = None;
        #[cfg(test)]
        let saved_timing = crate::frame_profile::scope(crate::frame_profile::Phase::RestoreSaved);
        if let Some(cache) = self.document.saved_calculation_cache() {
            self.saved_results = cache
                .rows
                .iter()
                .cloned()
                .map(|row| (row.region_id, row))
                .collect();
            if let Some(package) = native_package {
                native_plot_warnings::restore(self, package);
            }
        } else if self.document.calculation_cache.is_none() {
            if let Some(package) = native_package {
                self.restore_native_presentations_from_package(package);
            } else {
                self.restore_native_presentations();
            }
        }
        #[cfg(test)]
        drop(saved_timing);
        if self
            .document
            .regions
            .iter()
            .any(|region| matches!(&region.content,CanvasContent::Opaque{kind,..}if kind=="Area"))
        {
            self.area_saved = area_cache::restore_saved(&self.document, &self.saved_results);
            let owned_package = if native_package.is_none() {
                self.document
                    .native_archive
                    .as_deref()
                    .and_then(|bytes| NativePackage::from_bytes(bytes).ok())
            } else {
                None
            };
            if let Some(package) = native_package.or(owned_package.as_ref())
                && let Ok((saved, visuals)) = area_cache::restore_native(&self.document, package)
            {
                self.area_visuals = visuals;
                if self.document.calculation_cache.is_none() {
                    self.area_saved = saved;
                }
            }
            let empty_live = HashMap::new();
            let empty_saved = HashMap::new();
            for region in &self.document.regions {
                if matches!(&region.content,CanvasContent::Opaque{kind,..}if kind=="Area") {
                    if let Some(saved) = self.area_saved.get(&region.id) {
                        for (key, row) in saved {
                            if row.status == SavedCalculationStatus::Pending {
                                self.area_pending.insert((region.id, key.clone()));
                            }
                        }
                    }
                    if self.document.calculation_cache.is_none()
                        && let Ok(row) = area_cache::save_parent(
                            region,
                            &empty_live,
                            self.area_saved.get(&region.id).unwrap_or(&empty_saved),
                            &self.area_pending,
                            false,
                        )
                    {
                        self.saved_results.insert(region.id, row);
                    }
                }
            }
        }
        for region in &self.document.regions {
            if !calculatable(region) {
                continue;
            }
            let mut restored_plot = None;
            if let Some(row) = self.saved_results.get_mut(&region.id) {
                if let CanvasContent::Plot(spec) = &region.content {
                    restored_plot = row.display.as_deref().and_then(|display| {
                        crate::plots::PlotData::from_saved_for_spec(display, spec)
                    });
                    if restored_plot
                        .as_ref()
                        .is_some_and(|(_, data)| data.has_pending())
                    {
                        row.status = SavedCalculationStatus::Pending;
                    }
                }
                if region.calculation_disabled {
                    row.status = SavedCalculationStatus::Pending;
                }
                if row.display.is_none()
                    && let Some(symbolic) = row.symbolic_mathml.as_deref()
                {
                    row.display =
                        crate::result_display::from_native_result(symbolic, &region.format)
                            .map(|display| display.to_saved());
                }
                let supported = row.error.is_some()
                    || !expects_display_result(region)
                    || matches!(region.content, CanvasContent::Math(_))
                        && row.numeric_source.as_ref().is_some_and(|source| {
                            inert_string_result::validate_source(source).is_some()
                        })
                    || row.display.as_deref().is_some_and(|display| {
                        if matches!(region.content, CanvasContent::Plot(_)) {
                            restored_plot.is_some()
                        } else {
                            crate::result_display::FormattedResult::from_saved(display).is_some()
                        }
                    });
                if !supported {
                    row.status = SavedCalculationStatus::Pending;
                }
            }
            let row = self.saved_results.get(&region.id);
            if !region.calculation_disabled
                && row.is_none_or(|row| row.status == SavedCalculationStatus::Pending)
            {
                self.pending_regions.insert(region.id);
            }
            if matches!(region.content, CanvasContent::Plot(_))
                && let Some(row) = row
            {
                if let Some((version, mut data)) = restored_plot.take() {
                    if region.calculation_disabled
                        || (version == 1 && row.status == SavedCalculationStatus::Pending)
                    {
                        data.invalidate();
                    }
                    self.plot_results.insert(region.id, Ok(data));
                } else if let Some(error) = &row.error {
                    self.plot_results.insert(region.id, Err(error.clone()));
                }
            }
            if let CanvasContent::Plot(spec) = &region.content
                && !self.plot_results.contains_key(&region.id)
                && row.is_none_or(|saved| saved.error.is_none())
                && !matches!(self.native_visuals.region(region), Some(Err(_)))
            {
                let traces = spec
                    .paired_sources()
                    .map(|trace| {
                        if trace.x.expression.is_exact_placeholder()
                            || trace.y.expression.is_exact_placeholder()
                            || trace.x.expression.as_tree().is_some()
                                && !trace.x.expression.is_complete()
                            || trace.y.expression.as_tree().is_some()
                                && !trace.y.expression.is_complete()
                        {
                            crate::plots::TraceOutcome::Incomplete
                        } else {
                            crate::plots::TraceOutcome::Pending { previous: None }
                        }
                    })
                    .collect();
                self.plot_results
                    .insert(region.id, Ok(crate::plots::PlotData { traces }));
            }
        }
        // Only the document returned directly by this open's import_native is
        // already a projection of that same package. Retained own-format
        // archives still need the whole-source proof once after all restores.
        if !fresh_native {
            plot_restore_context::invalidate_unproven(self, native_package);
        }
        self.calculation_pending = !self.pending_regions.is_empty();
    }

    fn restore_native_presentations(&mut self) {
        let Some(bytes) = &self.document.native_archive else {
            return;
        };
        #[cfg(test)]
        let archive_timing =
            crate::frame_profile::scope(crate::frame_profile::Phase::RestoreArchive);
        let Ok(package) = NativePackage::from_bytes(bytes) else {
            return;
        };
        #[cfg(test)]
        drop(archive_timing);
        self.restore_native_presentations_from_package(&package);
    }

    fn restore_native_presentations_from_package(&mut self, package: &NativePackage) {
        #[cfg(test)]
        let regions_timing =
            crate::frame_profile::scope(crate::frame_profile::Phase::RestoreRegions);
        let regions = package.regions();
        #[cfg(test)]
        drop(regions_timing);
        #[cfg(test)]
        let caches_timing = crate::frame_profile::scope(crate::frame_profile::Phase::RestoreCaches);
        let caches = package.caches();
        #[cfg(test)]
        drop(caches_timing);
        let (Ok(regions), Ok(caches)) = (regions, caches) else {
            return;
        };
        #[cfg(test)]
        let dom_timing = crate::frame_profile::scope(crate::frame_profile::Phase::RestoreDom);
        let source = package.worksheet_xml().ok();
        let native_document = source
            .as_deref()
            .and_then(|source| roxmltree::Document::parse(source).ok());
        #[cfg(test)]
        drop(dom_timing);
        #[cfg(test)]
        let _display_timing =
            crate::frame_profile::scope(crate::frame_profile::Phase::RestoreDisplays);
        let plot_cache = if self
            .document
            .regions
            .iter()
            .any(|region| matches!(region.content, CanvasContent::Plot(_)))
        {
            Some(native_plot_source::CacheIndex::new(
                package, &caches, &regions,
            ))
        } else {
            None
        };
        let mut plot_sources = HashMap::new();
        let mut plot_regions = HashMap::new();
        if plot_cache.is_some() {
            if let Some(document) = &native_document {
                for node in document.descendants().filter(|node| {
                    node.has_tag_name(("http://schemas.mathsoft.com/worksheet50", "region"))
                }) {
                    if let Some(id) = node.attribute("region-id") {
                        plot_sources.entry(id).or_insert(node);
                    }
                }
            }
            for native in regions
                .iter()
                .filter(|region| region.part == package.worksheet_part_name())
            {
                if let Some(id) = native.region_id.as_deref() {
                    plot_regions.entry(id).or_insert(native);
                }
            }
        }
        for region in &self.document.regions {
            let Some(source_id) = &region.source_region else {
                continue;
            };
            let native = if matches!(region.content, CanvasContent::Plot(_)) {
                plot_regions.get(source_id.as_str()).copied()
            } else {
                regions.iter().find(|r| {
                    r.part == package.worksheet_part_name()
                        && r.region_id.as_ref() == Some(source_id)
                })
            };
            let Some(native) = native else {
                continue;
            };
            if matches!(region.content, CanvasContent::Plot(_)) {
                let restored = match (&plot_cache, &source) {
                    (Some(Ok(index)), Some(source)) => plot_sources
                        .get(source_id.as_str())
                        .copied()
                        .ok_or_else(|| "Native XY source region is missing".to_string())
                        .and_then(|node| index.restore_row(region, &standalone(node, source))),
                    (Some(Err(error)), _) => Err(error.clone()),
                    _ => Err("Native XY source cannot be inspected".into()),
                };
                match restored {
                    Ok(Some(row)) => {
                        self.saved_results.insert(region.id, row);
                    }
                    Ok(None) => {}
                    Err(error) => {
                        Arc::make_mut(&mut self.native_visuals)
                            .regions
                            .insert(source_id.clone(), Err(error));
                        self.status.push_str(" · 원본 그래프 저장 결과 지원 필요");
                    }
                }
                continue;
            }
            if matches!(&region.content, CanvasContent::Opaque {kind, ..} if kind == "spec-table")
                && let (Some(source), Some(document)) = (&source, &native_document)
                && let Some(original) = document.descendants().find(|node| {
                    node.has_tag_name(("http://schemas.mathsoft.com/worksheet50", "region"))
                        && node.attribute("region-id") == Some(source_id)
                })
                && let Some(row) =
                    spec_table_ui::native_saved_row(region, &standalone(original, source), &caches)
            {
                self.saved_results.insert(region.id, row);
                continue;
            }
            let Some(result_ref) = native.math.as_ref().and_then(|m| m.result_ref.as_ref()) else {
                continue;
            };
            let Some(cache) = caches.iter().find(|c| c.matches_result_ref(result_ref)) else {
                continue;
            };
            let mut numeric_source = None;
            let display = cache
                .result_mathml
                .as_deref()
                .and_then(|xml| {
                    // Native chosen display units already scaled the literal. The
                    // source RHS draws an explicit unit; do not draw it twice.
                    let explicit_unit = match &region.content {
                        CanvasContent::Math(tree) => {
                            let evaluation = if matches!(
                                tree.kind,
                                NodeKind::Binary(BinaryOp::Define | BinaryOp::GlobalDefine)
                            ) {
                                tree.children.get(1)
                            } else {
                                Some(tree)
                            };
                            evaluation.is_some_and(|tree| {
                                matches!(tree.kind, NodeKind::Binary(BinaryOp::Evaluate))
                                    && tree.children.get(1).is_some_and(|unit| {
                                        !matches!(unit.kind, NodeKind::Placeholder)
                                    })
                            })
                        }
                        _ => false,
                    };
                    let numeric = explicit_unit
                        .then(|| area_cache::explicit_unit_numeric(xml))
                        .flatten();
                    let source = numeric.as_deref().unwrap_or(xml);
                    let display =
                        crate::result_display::from_cached_native_result(source, &region.format);
                    if display.is_some() || inert_string_result::validate_native(source).is_some() {
                        numeric_source = Some(SavedNumericSource::NativeMathml(source.to_owned()));
                    }
                    display
                })
                .map(|display| display.to_saved());
            let error = cache.errors.first().map(|error| {
                error
                    .resource_string
                    .clone()
                    .or_else(|| error.code.clone())
                    .unwrap_or_else(|| "저장된 계산 오류".into())
            });
            let status = if cache.calculation_status.as_deref() == Some("Synchronized")
                && (display.is_some()
                    || numeric_source.is_some()
                    || error.is_some()
                    || !expects_display_result(region))
            {
                SavedCalculationStatus::Synchronized
            } else {
                SavedCalculationStatus::Pending
            };
            let numeric_source = if error.is_none() {
                numeric_source
            } else {
                None
            };
            self.saved_results.insert(
                region.id,
                SavedCalculationRow {
                    region_id: region.id,
                    status,
                    display: if error.is_none() { display } else { None },
                    error,
                    warnings: cache
                        .warnings
                        .iter()
                        .map(|_| "저장된 재정의 경고".into())
                        .collect(),
                    symbolic_mathml: None,
                    numeric_source,
                },
            );
        }
    }

    fn insert(&mut self, position: (f64, f64), text: bool) -> u64 {
        let before = self.snapshot();
        let id = if text {
            self.document
                .add_text(position.0, position.1, String::new())
        } else {
            self.document.add_math(position.0, position.1)
        };
        self.remember(before);
        self.selected = Some(id);
        self.focus_new = Some(id);
        self.reveal = Some(position);
        self.insertion = None;
        id
    }
    fn remove_selected(&mut self) {
        self.cancel_matrix_view_drag();
        if let Some(id) = self.selected {
            if self
                .document
                .regions
                .iter()
                .any(|region| region.id == id && page_break_ui::is_marker(region))
            {
                return;
            }
            let before = self.snapshot();
            self.selected = None;
            self.document.regions.retain(|r| r.id != id);
            self.document.matrix_result_views.remove_owner(id);
            self.editors.remove(&id);
            self.table_editors.remove(&id);
            self.area_editors.remove(&id);
            self.retire_plot_widgets(Some(id));
            if let Some(edit) = self.inline_plots.remove(&id) {
                let floor = self.plot_equation_floors.entry(id).or_default();
                *floor = (*floor).max(edit.equation_id_floor());
            }
            self.plot_palette_cursors.remove(&id);
            self.area_saved.remove(&id);
            self.area_visuals.remove(&id);
            self.area_presentations.remove(&id);
            self.area_prepared.remove(&id);
            self.area_pending.retain(|(owner, _)| *owner != id);
            Arc::make_mut(&mut self.native_visuals).remove_local_region(id);
            self.text_cursors.remove(&id);
            self.region_heights.remove(&id);
            self.region_widths.remove(&id);
            self.display_cache.remove(&id);
            self.saved_results.remove(&id);
            self.labelled_trees.remove(&id);
            self.uncommitted_math.remove(&id);
            self.editing_region = None;
            self.remember(before);
            self.retain_plot_equation_floors();
            self.calculation_changed(false);
        }
    }
    fn duplicate_selected(&mut self) {
        self.cancel_matrix_view_drag();
        let rich = match self
            .selected
            .map(|id| self.clipboard_rich_text(id))
            .transpose()
        {
            Ok(wire) => wire.flatten(),
            Err(error) => {
                self.status = error;
                return;
            }
        };
        let content = self
            .selected
            .and_then(|id| self.document.regions.iter().find(|r| r.id == id))
            .cloned();
        if let Some(mut region) = content
            && !matches!(region.content, CanvasContent::Opaque { .. })
        {
            let original_id = region.id;
            let duplicate_view = if matches!(region.content, CanvasContent::Math(_)) {
                match self.matrix_view_for_duplicate(original_id) {
                    Ok(view) => view,
                    Err(error) => {
                        self.status = error;
                        return;
                    }
                }
            } else {
                None
            };
            region.id = self.document.next_id;
            let mut next_views = self.document.matrix_result_views.clone();
            if let Some(view) = duplicate_view
                && let Err(error) = next_views.set(MatrixResultTarget::math(region.id), view)
            {
                self.status = error;
                return;
            }
            // set() checks complete metadata/count budgets before any document
            // mutation. The new target is the fresh known Math clone below;
            // all existing target memberships remain unchanged.
            region.left += self.document.grid * 2.;
            region.top += self.document.grid * 2.;
            region.source_region = None;
            region.rich_text = rich;
            if matches!(region.content, CanvasContent::Text(_))
                && let Err(error) = self.validate_rich_text_append(&region)
            {
                self.status = error;
                return;
            }
            let before = self.snapshot();
            self.document.next_id += 1;
            self.selected = Some(region.id);
            let id = region.id;
            self.document.regions.push(region);
            self.document.matrix_result_views = next_views;
            self.refresh_rich_text_visual(id);
            self.remember(before);
            self.calculation_changed(false);
        }
    }
    fn copy_region(&mut self, ctx: &egui::Context, cut: bool) {
        let rich = match self
            .selected
            .map(|id| self.clipboard_rich_text(id))
            .transpose()
        {
            Ok(wire) => wire.flatten(),
            Err(error) => {
                self.status = error;
                return;
            }
        };
        if let Some(region) = self
            .selected
            .and_then(|id| self.document.regions.iter().find(|r| r.id == id))
            .cloned()
            && (!matches!(region.content, CanvasContent::Opaque { .. }) || is_spec_table(&region))
        {
            let matrix_view = match self.clipboard_matrix_view(&region) {
                Ok(view) => view,
                Err(error) => {
                    self.status = error;
                    return;
                }
            };
            let mut envelope = serde_json::json!({"content":region.content,"format":region.format,"calculation_disabled":region.calculation_disabled});
            if let Some(view) = matrix_view {
                envelope["matrix_result_view"] =
                    serde_json::to_value(view).expect("Validated matrix viewport serialization");
            }
            if let Some(wire) = rich {
                envelope["rich_text"] =
                    serde_json::to_value(wire).expect("Rich source wire serialization");
            }
            if let Some(wire) = &region.authored_text {
                envelope["authored_text"] =
                    serde_json::to_value(wire).expect("Authored source wire serialization");
            }
            let Ok(value) = serde_json::to_string(&envelope) else {
                return;
            };
            if value.len() > 8 * 1024 * 1024 {
                self.status = "Clipboard region is too large".into();
                return;
            }
            ctx.copy_text(format!("mathcraft-region:{value}"));
            if cut {
                self.remove_selected();
            }
        }
    }
    fn paste_region(&mut self, text: &str) -> bool {
        if let Some(xml) = text.strip_prefix("mathcraft-table:") {
            if xml.len() > 8 * 1024 * 1024
                || mathcraft_document::spec_table::SpecTable::parse(xml).is_err()
            {
                self.status = "Clipboard table is invalid or too large".into();
                return true;
            }
            let format = self.worksheet_math_defaults();
            let payload = serde_json::json!({"content": CanvasContent::Opaque { xml: xml.into(), kind: "spec-table".into() }, "format":format});
            return self.paste_region(&format!("mathcraft-region:{payload}"));
        }
        let encoded = if let Some(encoded) = text.strip_prefix("mathcraft-region:") {
            encoded
        } else if let Some(encoded) = text.strip_prefix("mathcraft-tree:") {
            if encoded.len() > 8 * 1024 * 1024 {
                return true;
            }
            let tree = serde_json::from_str::<PortableTree>(encoded);
            if let Ok(tree) = tree
                && let Ok(json) = serde_json::to_string(&CanvasContent::Math(tree))
            {
                return self.paste_region(&format!("mathcraft-region:{json}"));
            }
            self.status = "Clipboard formula is invalid".into();
            return true;
        } else {
            return false;
        };
        if encoded.len() > 8 * 1024 * 1024 {
            self.status = "Clipboard region is too large".into();
            return true;
        }
        let matrix_view = match matrix_region_clipboard::read_view(encoded) {
            Ok(view) => view,
            Err(error) => {
                self.status = format!("Clipboard matrix result view is invalid: {error}");
                return true;
            }
        };
        let parsed = serde_json::from_str::<serde_json::Value>(encoded);
        let (mut content, format, calculation_disabled, rich_text, authored_text) = match parsed {
            Ok(value) => {
                let rich = value
                    .get("rich_text")
                    .cloned()
                    .map(
                        serde_json::from_value::<
                            Option<Box<mathcraft_document::rich_text_source::RichTextWire>>,
                        >,
                    )
                    .transpose();
                let authored = value
                    .get("authored_text")
                    .cloned()
                    .map(
                        serde_json::from_value::<
                            Option<Box<mathcraft_document::rich_text_source::AuthoredTextWire>>,
                        >,
                    )
                    .transpose();
                let disabled = value
                    .get("calculation_disabled")
                    .cloned()
                    .map(serde_json::from_value::<bool>)
                    .transpose();
                let format = value
                    .get("format")
                    .cloned()
                    .map(serde_json::from_value::<MathFormat>)
                    .transpose();
                let content = serde_json::from_value::<CanvasContent>(
                    value.get("content").cloned().unwrap_or(value),
                );
                match (content, format, disabled, rich, authored) {
                    (Ok(content), Ok(format), Ok(disabled), Ok(rich), Ok(authored)) => (
                        content,
                        format.unwrap_or_default(),
                        disabled.unwrap_or(false),
                        rich.flatten(),
                        authored.flatten(),
                    ),
                    _ => {
                        self.status = "Clipboard region is invalid".into();
                        return true;
                    }
                }
            }
            Err(_) => {
                self.status = "Clipboard region is invalid".into();
                return true;
            }
        };
        if let CanvasContent::Opaque { kind, .. } = &mut content
            && kind == "specTable"
        {
            *kind = "spec-table".into();
        }
        let valid = match &content {
            CanvasContent::Math(tree) => tree.validate().is_ok(),
            CanvasContent::Text(text) => text.len() <= 1024 * 1024,
            CanvasContent::Plot(plot) => plot.validate().is_ok(),
            CanvasContent::Opaque { xml, kind }
                if matches!(kind.as_str(), "spec-table" | "specTable") =>
            {
                mathcraft_document::spec_table::SpecTable::parse(xml).is_ok()
            }
            _ => false,
        };
        if !valid
            || (matrix_view.is_some() && !matches!(content, CanvasContent::Math(_)))
            || ((rich_text.is_some() || authored_text.is_some())
                && !matches!(content, CanvasContent::Text(_)))
            || (rich_text.is_some() && authored_text.is_some())
            || (calculation_disabled && matches!(content, CanvasContent::Text(_)))
            || !format.font_points.is_finite()
            || !(1. ..=200.).contains(&format.font_points)
            || format.precision > 15
        {
            self.status = "Clipboard region is invalid or unsupported".into();
            return true;
        }
        if matches!(&content, CanvasContent::Opaque {kind, ..} if kind == "spec-table")
            && let Err(error) = self.commit_table_edits(None)
        {
            self.status = error;
            return true;
        }
        let position = self.insertion.unwrap_or_else(|| {
            self.selected
                .and_then(|id| self.document.regions.iter().find(|r| r.id == id))
                .map(|r| {
                    (
                        r.left + self.document.grid * 2.,
                        r.top + self.document.grid * 2.,
                    )
                })
                .unwrap_or((0., self.document.grid))
        });
        let id = self.document.next_id;
        let candidate = CanvasRegion {
            id,
            left: position.0,
            top: position.1,
            content,
            source_region: None,
            rich_text,
            authored_text,
            format,
            calculation_disabled,
            calculation_enabled_override: false,
        };
        if matches!(candidate.content, CanvasContent::Text(_))
            && let Err(error) = self.validate_rich_text_append(&candidate)
        {
            self.status = error;
            return true;
        }
        // Metadata-only clipboard requests do not promise a calculated result.
        // Validate DTO/map/type budgets now; actual value projection is deferred
        // until a complete result exists, exactly as for an own saved request.
        let mut next_views = None;
        if let Some(view) = matrix_view {
            let mut views = self.document.matrix_result_views.clone();
            if let Err(error) = views.set(MatrixResultTarget::math(id), view) {
                self.status = error;
                return true;
            }
            next_views = Some(views);
        }
        let before = self.snapshot();
        self.document.next_id += 1;
        self.document.regions.push(candidate);
        if let Some(views) = next_views {
            self.document.matrix_result_views = views;
        }
        self.refresh_rich_text_visual(id);
        self.selected = Some(id);
        self.insertion = None;
        if self.document.regions.last().is_some_and(is_spec_table) {
            self.editing_region = None;
            self.focus_new = None;
            self.refresh_table_visual(id);
        }
        self.remember(before);
        self.calculation_changed(false);
        true
    }
    fn prepare_native_export(&mut self) -> Result<(), String> {
        self.sync_editors()?;
        self.commit_math_edits();
        self.persist_calculation_cache();
        Ok(())
    }
    fn export_mcdx(&mut self) {
        if let Err(error) = self.prepare_native_export() {
            self.status = format!("Could not export: {error}");
            return;
        }
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Mathcad worksheet", &["mcdx"])
            .set_file_name("worksheet.mcdx")
            .save_file()
        {
            match mathcraft_document::export::export_mcdx(&self.document, &path) {
                Ok(()) => self.status = format!("Exported {}", path.display()),
                Err(error) => self.status = format!("Could not export: {error}"),
            }
        }
    }
    fn insert_template(&mut self, tree: PortableTree) {
        self.apply_editor_command(|editor| {
            let prior = editor
                .arena()
                .nodes()
                .map(|(id, _)| id.index())
                .max()
                .unwrap_or(0);
            match tree.kind {
                NodeKind::Fraction => {
                    editor.insert_fraction();
                }
                NodeKind::Power => {
                    editor.insert_power();
                }
                NodeKind::Root => {
                    editor.insert_root();
                }
                NodeKind::Absolute => {
                    editor.insert_absolute();
                }
                NodeKind::Index { indices: 1 } => {
                    editor.insert_index();
                }
                _ => {
                    let _ = editor.replace_selection(&tree);
                }
            }
            let empty = editor
                .arena()
                .nodes()
                .find(|(id, node)| id.index() > prior && *node.kind() == NodeKind::Placeholder)
                .map(|(id, _)| id);
            if let Some(empty) = empty {
                let _ = editor.place_caret(empty, Direction::Right);
            }
        });
    }
    fn choose_open(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Worksheets", &["mathcraft", "mcdx"])
            .pick_file()
        {
            self.request(Pending::Open(path));
        }
    }
    fn begin_region(&mut self, text: bool) {
        let position = self.insertion.unwrap_or_else(|| {
            self.selected
                .and_then(|id| self.document.regions.iter().find(|r| r.id == id))
                .map(|r| (r.left, r.top + self.document.grid * 5.))
                .unwrap_or((0., self.document.grid))
        });
        self.text_mode = text;
        if text {
            if let Err(error) = self.insert_authored_text(position, "") {
                self.status = format!("텍스트 삽입: {error}");
            }
        } else {
            self.insert(position, false);
        }
        // Text insertion is a command; clicking elsewhere resumes math writing.
        self.text_mode = false;
    }
    fn chrome_ui(&mut self, ui: &mut egui::Ui) {
        let editing_before = self.editing_region;
        if self.editing_region.is_none() && self.focus_new.is_none()
            && (self.selected.is_some() || self.insertion.is_some())
            && self.menu_resume.is_none() && !ui.ctx().any_popup_open()
            && ui.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Key {
                key: egui::Key::Tab | egui::Key::ArrowLeft | egui::Key::ArrowRight | egui::Key::ArrowUp | egui::Key::ArrowDown,
                pressed: true, modifiers, ..
            } if !modifiers.command && !modifiers.alt)))
        {
            // Suppress automatic widget focus traversal for page-owned navigation,
            // including the first frame after switching out of formula editing.
            ui.memory_mut(|m| m.move_focus(egui::FocusDirection::None));
        }
        let name = self
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.document.title.clone());
        let active_format = self
            .selected
            .and_then(|id| self.document.regions.iter().find(|r| r.id == id))
            .map(|r| r.format)
            .unwrap_or_default();
        let in_table = self.selected.is_some_and(|id| {
            self.document
                .regions
                .iter()
                .any(|region| region.id == id && is_spec_table(region))
        });
        let has_math = in_table
            || self.selected.is_some_and(|id| {
                self.document
                    .regions
                    .iter()
                    .any(|r| r.id == id && matches!(r.content, CanvasContent::Math(_)))
            });
        let has_plot = self.selected.is_some_and(|id| {
            self.document
                .regions
                .iter()
                .any(|r| r.id == id && matches!(r.content, CanvasContent::Plot(_)))
        });
        let active_editor = self.selected.and_then(|id| {
            self.editors.get(&id).or_else(|| {
                self.table_editors
                    .get(&id)
                    .and_then(|table| table.active_editor())
            })
        });
        let can_label = active_editor.is_some_and(|editor| {
            let target = match editor.selection() {
                mathcraft_editor::Selection::Subtree(n) => n,
                mathcraft_editor::Selection::None => editor.caret().node,
                mathcraft_editor::Selection::MatrixCells(_) => return false,
            };
            matches!(
                editor.arena().get(target).unwrap().kind(),
                NodeKind::Identifier(_)
                    | NodeKind::AnnotatedIdentifier { .. }
                    | NodeKind::NativeIdentifier { .. }
            )
        });
        let in_matrix = active_editor.is_some_and(|editor| {
            if matches!(
                editor.selection(),
                mathcraft_editor::Selection::MatrixCells(_)
            ) {
                return true;
            }
            let mut target = Some(editor.caret().node);
            while let Some(id) = target {
                let node = editor.arena().get(id).unwrap();
                if matches!(node.kind(), NodeKind::Matrix { .. }) {
                    return editor.selection() != mathcraft_editor::Selection::Subtree(id);
                }
                target = node.parent();
            }
            false
        });
        let has_errors = self
            .document
            .regions
            .iter()
            .any(|region| self.region_has_error(region.id));
        let has_warnings = self.document.regions.iter().any(|r| {
            !r.calculation_disabled
                && !self.pending_regions.contains(&r.id)
                && (self
                    .results
                    .get(&r.id)
                    .is_some_and(|row| row.redefinition_warning)
                    || self
                        .saved_results
                        .get(&r.id)
                        .is_some_and(|row| !row.warnings.is_empty()))
        });
        let can_insert_area = self.can_insert_empty_area();
        let can_page_break = self.can_request_page_break();
        let (can_remove_page_break_above, can_remove_page_break_below) =
            self.page_break_removal_availability();
        let bin_oct_hex_mode = self.numeric_input_mode();
        let text_format = self.rich_text_context();
        let paragraph_alignment = self.paragraph_alignment_context();
        let matrix_indices = self.matrix_indices_context();
        let commands = ribbon::show(
            ui,
            &mut self.ribbon,
            &RibbonContext {
                document_name: &name,
                can_undo: !self.undo.is_empty(),
                can_redo: !self.redo.is_empty(),
                has_selection: self.selected.is_some(),
                can_insert_area,
                can_page_break,
                can_remove_page_break_above,
                can_remove_page_break_below,
                zoom: self.zoom,
                grid: self.show_grid,
                font_points: active_format.font_points,
                precision: active_format.precision,
                result_format: result_format_index(active_format.result_format),
                number_format: active_format,
                matrix_indices,
                has_math,
                has_plot,
                draft_view: self.expand_right,
                in_matrix,
                in_table,
                can_label,
                has_errors,
                automatic_calculation: self.automatic_calculation,
                bin_oct_hex_mode,
                calculation_running: self.background.running,
                region_calculation_disabled: self
                    .selected
                    .and_then(|id| self.document.regions.iter().find(|r| r.id == id))
                    .filter(|r| r.supports_calculation_toggle())
                    .map(|r| r.calculation_disabled),
                show_warnings: self.show_warnings,
                has_warnings,
                text_format,
                paragraph_alignment,
            },
        );
        for command in commands {
            self.ribbon_command(ui.ctx(), command);
        }
        let hint = if self.status.is_empty() {
            self.context_hint().to_string()
        } else {
            self.status.clone()
        };
        let errors = self
            .document
            .regions
            .iter()
            .filter(|region| self.region_has_error(region.id))
            .count();
        let calculation_error = self.calculation_error.clone();
        let calculation_pending = self.calculation_pending;
        let calculation_running = self.background.running;
        let warnings = if !self.show_warnings {
            0
        } else {
            self.document
                .regions
                .iter()
                .filter(|r| {
                    !r.calculation_disabled
                        && !self.pending_regions.contains(&r.id)
                        && (self
                            .results
                            .get(&r.id)
                            .is_some_and(|row| row.redefinition_warning)
                            || self
                                .saved_results
                                .get(&r.id)
                                .is_some_and(|row| !row.warnings.is_empty()))
                })
                .count()
        };
        let dirty = self.dirty;
        egui::Panel::bottom("worksheet-status").exact_size(25.)
            .frame(egui::Frame::new().fill(Color32::from_rgb(232, 238, 243)).inner_margin(egui::Margin::symmetric(10, 3)))
            .show(ui, |ui| {
                egui::containers::Sides::new().shrink_left().truncate().show(ui,
                    |ui| {
                        if calculation_running {
                            ui.colored_label(Color32::from_rgb(54, 99, 145), "계산 중…");
                        } else if calculation_pending {
                            ui.colored_label(Color32::from_rgb(124, 91, 23), "계산 대기 중");
                        } else if let Some(error) = &calculation_error {
                            ui.colored_label(RED, "계산을 완료하지 못했습니다").on_hover_text(error);
                        } else if errors > 0 {
                            ui.colored_label(RED, format!("오류 {errors}개")).on_hover_text("수식의 오류 표시에 마우스를 올리면 상세 내용을 볼 수 있습니다.");
                        } else if warnings > 0 {
                            ui.colored_label(AMBER, format!("재정의 경고 {warnings}개"));
                        } else { ui.small(if dirty { "변경됨" } else { "준비" }); }
                        ui.separator();
                        ui.add(egui::Label::new(&hint).truncate()).on_hover_text(&hint);
                    },
                    |ui| {
                        egui::ComboBox::from_id_salt("worksheet-zoom").width(68.)
                            .selected_text(format!("{}%", (self.zoom * 100.).round()))
                            .show_ui(ui, |ui| {
                                for zoom in [0.5, 0.75, 1., 1.25, 1.5, 2.] {
                                    ui.selectable_value(&mut self.zoom, zoom, format!("{}%", (zoom * 100.) as i32));
                                }
                                if ui.button("너비 맞춤").clicked() { self.fit_width(ui.ctx()); ui.close(); }
                            });
                        if ui.selectable_label(self.show_grid, "격자").on_hover_text("배치 격자 점 표시 / 숨기기").clicked() { self.show_grid = !self.show_grid; }
                        ui.small(format!("{}개 영역", self.document.regions.len()));
                    });
            });
        if ui.ctx().any_popup_open() {
            self.menu_resume = self.menu_resume.or(editing_before);
        } else if let Some(id) = self.menu_resume.take()
            && self.selected == Some(id)
            && self.insertion.is_none()
            && self.focus_new.is_none()
        {
            self.focus_new = Some(id);
            ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        }
    }
    fn ensure_math_region(&mut self) -> u64 {
        if let Some(id) = self.selected.filter(|id| {
            self.document
                .regions
                .iter()
                .any(|r| r.id == *id && matches!(r.content, CanvasContent::Math(_)))
        }) {
            return id;
        }
        let position = self.insertion.unwrap_or_else(|| {
            self.selected
                .and_then(|id| self.document.regions.iter().find(|r| r.id == id))
                .map(|r| (r.left, r.top + self.document.grid * 5.))
                .unwrap_or((0., self.document.grid))
        });
        let id = self.document.add_math(position.0, position.1);
        self.selected = Some(id);
        self.focus_new = Some(id);
        self.insertion = None;
        self.reveal = Some(position);
        id
    }
    fn insert_identifier(&mut self, name: &str, label: &str) {
        self.insert_atom(NodeKind::AnnotatedIdentifier {
            name: name.into(),
            labels: label.into(),
        });
    }
    fn insert_unit(&mut self, name: &str) {
        if self.selected.is_some_and(|id| {
            self.document.regions.iter().any(|region| {
                region.id == id
                    && matches!(&region.content, CanvasContent::Opaque {kind, ..} if kind == "Area")
            }) && self
                .area_editors
                .get(&id)
                .is_none_or(|area| area.active_editor().is_none())
        }) {
            // A blank Area insertion needs an owner-level creation command.
            // Do not silently create a root worksheet region outside it.
            return;
        }
        self.apply_editor_command(|editor| {
            unit_input::insert(editor, name);
        });
    }
    fn apply_editor_command(&mut self, command: impl FnOnce(&mut Editor)) {
        if let Some(id) = self.selected
            && self
                .area_editors
                .get(&id)
                .is_some_and(|area| area.active_editor().is_some())
        {
            let before = self.input_snapshot();
            let area = self.area_editors.get_mut(&id).unwrap();
            let editor = area.active_editor_mut().unwrap();
            let revision = editor.arena().revision();
            editor.edit_group(command);
            let changed = editor.arena().revision() != revision;
            if changed {
                area.mark_active_changed();
                self.remember(before);
                self.stage_math_edit(id, false);
            }
            self.focus_new = Some(id);
            return;
        }
        if self.selected.is_some_and(|id| {
            self.document
                .regions
                .iter()
                .any(|region| region.id == id && is_spec_table(region))
        }) {
            self.apply_table_editor_command(command);
            return;
        }
        let before = self.math_command_snapshot();
        let count = self.document.regions.len();
        let id = self.ensure_math_region();
        let created = count != self.document.regions.len();
        let Some(region) = self.document.regions.iter_mut().find(|r| r.id == id) else {
            return;
        };
        let CanvasContent::Math(tree) = &mut region.content else {
            return;
        };
        let editor = self
            .editors
            .entry(id)
            .or_insert_with(|| Editor::from_portable(tree).unwrap_or_default());
        let revision = editor.arena().revision();
        editor.edit_group(command);
        let changed = revision != editor.arena().revision() || created;
        if changed {
            match editor.to_portable() {
                Ok(value) => *tree = value,
                Err(error) => {
                    self.status = format!("Could not insert: {error}");
                    return;
                }
            }
        }
        self.selected = Some(id);
        self.focus_new = Some(id);
        self.insertion = None;
        if changed {
            self.remember(before);
            self.stage_math_edit(id, false);
        }
    }
    fn insert_atom(&mut self, kind: NodeKind) {
        self.apply_editor_command(|editor| {
            if editor.selection() == mathcraft_editor::Selection::None
                && editor.arena().get(editor.caret().node).unwrap().kind() != &NodeKind::Placeholder
            {
                editor.insert_infix(BinaryOp::Multiply);
            }
            let _ = editor.replace_selection(&PortableTree {
                kind,
                children: vec![],
            });
        });
    }
    fn change_math_format(&mut self, change: impl FnOnce(&mut MathFormat)) {
        let Some(id) = self.selected else {
            return;
        };
        let Some(region) = self
            .document
            .regions
            .iter()
            .find(|region| region.id == id && matches!(region.content, CanvasContent::Math(_)))
        else {
            return;
        };
        let before = self.snapshot_with_document(DocumentHistory::Region(Box::new(region.clone())));
        if let Some(region) = self
            .document
            .regions
            .iter_mut()
            .find(|r| r.id == id && matches!(r.content, CanvasContent::Math(_)))
        {
            let prior = region.format;
            change(&mut region.format);
            region.format.explicit = true;
            let mut numeric_prior = prior;
            numeric_prior.font_points = region.format.font_points;
            numeric_prior.explicit = region.format.explicit;
            let numeric_changed = numeric_prior != region.format;
            if prior != region.format {
                self.remember(before);
                self.display_cache.remove(&id);
                if numeric_changed
                    && !self
                        .results
                        .get(&id)
                        .is_some_and(|r| r.display_value.is_some() || r.value.is_some())
                    && self
                        .saved_results
                        .get(&id)
                        .is_some_and(|r| r.display.is_some() && r.numeric_source.is_none())
                {
                    self.pending_regions.insert(id);
                    self.calculation_pending = true;
                    self.status = "이전 표시 캐시에는 원래 정밀도의 결과가 없습니다. 서식을 갱신하려면 계산이 필요합니다.".into();
                }
            }
            self.focus_new = Some(id);
        }
    }
    fn set_label(&mut self, label: &str) {
        if !self.selected.is_some_and(|id| {
            self.document.regions.iter().any(|r| {
                r.id == id && (matches!(r.content, CanvasContent::Math(_)) || is_spec_table(r))
            })
        }) {
            return;
        }
        self.apply_editor_command(|editor| {
            editor.set_identifier_label(label);
        });
    }
    fn matrix_command(&mut self, action: RibbonAction) {
        if self.table_matrix_command(action) {
            return;
        }
        let Some(id) = self.selected else {
            return;
        };
        let Some(region) = self
            .document
            .regions
            .iter()
            .find(|region| region.id == id && matches!(region.content, CanvasContent::Math(_)))
        else {
            return;
        };
        let before = self.snapshot_with_document(DocumentHistory::Region(Box::new(region.clone())));
        let Some(editor) = self.editors.get_mut(&id) else {
            return;
        };
        let changed = match action {
            RibbonAction::InsertMatrixRow => editor.insert_matrix_row(Direction::Indeterminate),
            RibbonAction::InsertMatrixColumn => {
                editor.insert_matrix_column(Direction::Indeterminate)
            }
            RibbonAction::DeleteMatrixRow => editor.delete_matrix_row(),
            RibbonAction::DeleteMatrixColumn => editor.delete_matrix_column(),
            RibbonAction::ClearCells => editor.clear_matrix_cells(),
            _ => false,
        };
        if changed {
            if let Ok(tree) = editor.to_portable()
                && let Some(region) = self.document.regions.iter_mut().find(|r| r.id == id)
            {
                region.content = CanvasContent::Math(tree);
            }
            self.remember(before);
            self.stage_math_edit(id, false);
        }
        self.focus_new = Some(id);
    }
    fn ribbon_command(&mut self, ctx: &egui::Context, action: RibbonAction) {
        if let RibbonAction::SetMatrixIndices { owner, show } = action {
            if let Err(error) = self.set_matrix_indices(owner, show) {
                self.status = error;
            }
            return;
        }
        if let RibbonAction::SetParagraphAlignment {
            value,
            selection_key,
        } = action
        {
            if let Err(error) = self.paragraph_alignment_command(ctx, value, selection_key) {
                self.status = format!("문단 정렬: {error}");
            }
            return;
        }
        if matches!(
            action,
            RibbonAction::SetTextBold { .. }
                | RibbonAction::SetTextItalic { .. }
                | RibbonAction::SetTextUnderline { .. }
                | RibbonAction::SetTextFontSize { .. }
                | RibbonAction::SetTextColor { .. }
        ) {
            self.rich_text_command(ctx, action);
            return;
        }
        if matches!(action, RibbonAction::Undo | RibbonAction::Redo) {
            plain_text_input::cancel(&mut self.text_composition, ctx);
        }
        let numeric_mode = self.numeric_input_mode().unwrap_or(false);
        if input_intent::ribbon_needs_baseline(action)
            && !matches!(
                action,
                RibbonAction::InsertPageBreak
                    | RibbonAction::RemovePageBreakAbove
                    | RibbonAction::RemovePageBreakBelow
            )
        {
            self.retain_committed_input();
        }
        let p = || PortableTree {
            kind: NodeKind::Placeholder,
            children: vec![],
        };
        let binary = |kind| PortableTree {
            kind,
            children: vec![p(), p()],
        };
        let editing = self.editing_region.or(self.menu_resume).or(self.focus_new);
        match action {
            RibbonAction::SetParagraphAlignment { .. } | RibbonAction::SetMatrixIndices { .. } => {
                unreachable!()
            }
            RibbonAction::SetTextBold { .. }
            | RibbonAction::SetTextItalic { .. }
            | RibbonAction::SetTextUnderline { .. }
            | RibbonAction::SetTextFontSize { .. }
            | RibbonAction::SetTextColor { .. } => unreachable!(),
            RibbonAction::New => self.request(Pending::New),
            RibbonAction::Open => self.choose_open(),
            RibbonAction::Save => {
                self.save(false);
            }
            RibbonAction::SaveAs => {
                self.save(true);
            }
            RibbonAction::ExportMcdx => self.export_mcdx(),
            RibbonAction::Print => self.open_print(false),
            RibbonAction::ExportPdf => self.open_print(true),
            RibbonAction::PageSetup => self.open_page_setup(),
            RibbonAction::Undo => self.undo(),
            RibbonAction::Redo => self.redo(),
            RibbonAction::Cut | RibbonAction::Copy => {
                if editing.is_some() {
                    self.focus_new = editing;
                    ctx.send_viewport_cmd(if action == RibbonAction::Cut {
                        egui::ViewportCommand::RequestCut
                    } else {
                        egui::ViewportCommand::RequestCopy
                    });
                } else {
                    self.copy_region(ctx, action == RibbonAction::Cut);
                }
            }
            RibbonAction::Paste => {
                self.focus_new = editing;
                ctx.send_viewport_cmd(egui::ViewportCommand::RequestPaste);
            }
            RibbonAction::Delete => self.remove_selected(),
            RibbonAction::InsertPlot => {
                let before = self.snapshot();
                let position = self.insertion.unwrap_or_else(|| {
                    self.selected
                        .and_then(|id| self.document.regions.iter().find(|r| r.id == id))
                        .map(|r| (r.left, r.top + self.document.grid * 5.))
                        .unwrap_or((0., self.document.grid))
                });
                let id = self.document.add_plot(position.0, position.1);
                self.document
                    .regions
                    .iter_mut()
                    .find(|region| region.id == id)
                    .unwrap()
                    .content = CanvasContent::Plot(PlotSpec::typed_blank_xy());
                self.selected = Some(id);
                self.focus_new = Some(id);
                self.insertion = None;
                self.reveal = Some(position);
                self.remember(before);
                self.calculation_changed(false);
            }
            RibbonAction::InsertSolveBlock => {
                self.solve_draft = Some(SolveDraft::default());
            }
            RibbonAction::EditPlot => {
                match self.commit_plot_edits(None) {
                    Ok(true) => self.calculation_changed(false),
                    Ok(false) => (),
                    Err(error) => {
                        self.status = format!("도표 입력 오류: {error}");
                        return;
                    }
                }
                if let Some(id) = self.selected
                    && let Some(CanvasContent::Plot(spec)) = self
                        .document
                        .regions
                        .iter()
                        .find(|region| region.id == id)
                        .map(|region| &region.content)
                {
                    let mut spec = spec.clone();
                    spec.promote_legacy_trace();
                    self.plot_draft = Some(PlotDraft {
                        id,
                        spec,
                        topology_changed: false,
                    });
                }
            }
            RibbonAction::InsertPageBreak => {
                self.queue_page_break(page_break_ui::PageBreakRequest::Insert)
            }
            RibbonAction::RemovePageBreakAbove => {
                self.queue_page_break(page_break_ui::PageBreakRequest::Above)
            }
            RibbonAction::RemovePageBreakBelow => {
                self.queue_page_break(page_break_ui::PageBreakRequest::Below)
            }
            RibbonAction::InsertMath => self.begin_region(false),
            RibbonAction::InsertText => self.begin_region(true),
            RibbonAction::InsertTable => self.insert_spec_table(),
            RibbonAction::InsertArea => {
                if let Err(error) = self.insert_empty_area() {
                    self.status = error;
                }
            }
            RibbonAction::LiteralSubscript => self.apply_editor_command(|editor| {
                editor.toggle_literal_subscript();
            }),
            RibbonAction::Fraction => self.insert_template(binary(NodeKind::Fraction)),
            RibbonAction::Power => self.insert_template(binary(NodeKind::Power)),
            RibbonAction::Root => self.insert_template(PortableTree {
                kind: NodeKind::Root,
                children: vec![
                    PortableTree {
                        kind: NodeKind::Number("2".into()),
                        children: vec![],
                    },
                    p(),
                ],
            }),
            RibbonAction::Absolute => self.apply_editor_command(|editor| {
                editor.insert_determinant();
            }),
            RibbonAction::Factorial => self.apply_editor_command(|editor| {
                editor.insert_factorial();
            }),
            RibbonAction::Percent => self.apply_editor_command(|editor| {
                editor.insert_percent();
            }),
            RibbonAction::Degree => self.apply_editor_command(|editor| {
                editor.insert_degree();
            }),
            RibbonAction::Polar => self.apply_editor_command(|editor| {
                editor.insert_polar();
            }),
            RibbonAction::Transpose => self.apply_editor_command(|editor| {
                editor.insert_transpose();
            }),
            RibbonAction::Conjugate => self.apply_editor_command(|editor| {
                editor.insert_conjugate();
            }),
            RibbonAction::Norm => self.apply_editor_command(|editor| {
                editor.insert_norm();
            }),
            RibbonAction::RowOperator => self.apply_editor_command(|editor| {
                editor.insert_row_operator();
            }),
            RibbonAction::ColumnOperator => self.apply_editor_command(|editor| {
                editor.insert_column_operator();
            }),
            RibbonAction::Vectorize => self.apply_editor_command(|editor| {
                editor.insert_vectorize();
            }),
            RibbonAction::CrossProduct => self.apply_editor_command(|editor| {
                editor.insert_cross_product();
            }),
            RibbonAction::GlobalDefine => self.apply_editor_command(|editor| {
                editor.insert_global_definition();
            }),
            RibbonAction::Matrix => self.apply_editor_command(|editor| {
                editor.insert_native_matrix();
            }),
            RibbonAction::Index => self.insert_template(binary(NodeKind::Index { indices: 1 })),
            RibbonAction::Range => {
                self.insert_template(binary(NodeKind::Range { has_step: false }))
            }
            RibbonAction::Function => self.insert_template(PortableTree {
                kind: NodeKind::Function { parameter_count: 1 },
                children: vec![p(), p()],
            }),
            RibbonAction::Program => {
                self.apply_editor_command(|editor| {
                    editor.insert_program();
                });
            }
            RibbonAction::LocalDefine => {
                self.apply_editor_command(|editor| {
                    editor.insert_local_definition();
                });
            }
            RibbonAction::If => {
                self.apply_editor_command(|editor| {
                    editor.insert_if();
                });
            }
            RibbonAction::ElseIf => {
                self.apply_editor_command(|editor| {
                    editor.insert_else_if();
                });
            }
            RibbonAction::AlsoIf => {
                self.apply_editor_command(|editor| {
                    editor.insert_also_if();
                });
            }
            RibbonAction::Else => {
                self.apply_editor_command(|editor| {
                    editor.insert_else();
                });
            }
            RibbonAction::ForLoop => {
                self.apply_editor_command(|editor| {
                    editor.insert_for_loop();
                });
            }
            RibbonAction::WhileLoop => {
                self.apply_editor_command(|editor| {
                    editor.insert_while_loop();
                });
            }
            RibbonAction::Return => {
                self.apply_editor_command(|editor| {
                    editor.insert_return();
                });
            }
            RibbonAction::Break => {
                self.apply_editor_command(|editor| {
                    editor.insert_break();
                });
            }
            RibbonAction::Continue => {
                self.apply_editor_command(|editor| {
                    editor.insert_continue();
                });
            }
            RibbonAction::Try => {
                self.apply_editor_command(|editor| {
                    editor.insert_try();
                });
            }
            RibbonAction::Integral => {
                self.apply_editor_command(|editor| {
                    editor.insert_integral();
                });
            }
            RibbonAction::Derivative => {
                self.apply_editor_command(|editor| {
                    editor.insert_derivative(false);
                });
            }
            RibbonAction::PartialDerivative => {
                self.apply_editor_command(|editor| {
                    editor.insert_derivative(true);
                });
            }
            RibbonAction::Summation => {
                self.apply_editor_command(|editor| {
                    editor.insert_summation();
                });
            }
            RibbonAction::Product => {
                self.apply_editor_command(|editor| {
                    editor.insert_product();
                });
            }
            RibbonAction::SymbolicEvaluate => {
                self.apply_editor_command(|editor| {
                    editor.insert_symbolic_evaluation();
                });
            }
            RibbonAction::Define | RibbonAction::Evaluate | RibbonAction::InsertSymbol(_) => {
                let text = match action {
                    RibbonAction::Define => ":",
                    RibbonAction::Evaluate => "=",
                    RibbonAction::InsertSymbol(s) => s,
                    _ => unreachable!(),
                };
                if self.selected.is_some_and(|id| {
                    self.document
                        .regions
                        .iter()
                        .any(|r| r.id == id && is_spec_table(r))
                }) {
                    self.apply_table_editor_command(|editor| {
                        editor.set_bin_oct_hex_mode(numeric_mode);
                        formula::input_text_with_actions(editor, text);
                    });
                    return;
                }
                let before = self.math_command_snapshot();
                let id = self.ensure_math_region();
                {
                    if let Some(region) = self.document.regions.iter_mut().find(|r| r.id == id)
                        && let CanvasContent::Math(tree) = &mut region.content
                    {
                        let editor = self
                            .editors
                            .entry(id)
                            .or_insert_with(|| Editor::from_portable(tree).unwrap_or_default());
                        editor.set_bin_oct_hex_mode(numeric_mode);
                        let outcome = formula::input_text_with_actions(editor, text);
                        if let Ok(value) = editor.to_portable() {
                            *tree = value;
                        }
                        if outcome.document_changed {
                            self.remember(before);
                        }
                        if outcome.document_changed || outcome.evaluation_requested {
                            self.stage_math_edit(id, outcome.evaluation_requested);
                        }
                        self.focus_new = Some(id);
                    }
                }
            }
            RibbonAction::InsertIdentifier { name, label } => self.insert_identifier(name, label),
            RibbonAction::InsertUnit(name) => self.insert_unit(name),
            RibbonAction::InsertImaginary => self.insert_atom(NodeKind::Imaginary("1".into())),
            RibbonAction::InsertFunction(name) => {
                let count = if name == "atan2" { 2 } else { 1 };
                let mut children = vec![PortableTree {
                    kind: NodeKind::Identifier(name.into()),
                    children: vec![],
                }];
                children.extend((0..count).map(|_| p()));
                self.insert_template(PortableTree {
                    kind: NodeKind::Call {
                        argument_count: count,
                    },
                    children,
                });
            }
            RibbonAction::Recalculate => {
                self.recalculate();
                self.focus_new = editing;
            }
            RibbonAction::CalculateNow => {
                self.calculate_command(CalculationCommand::Now);
                self.focus_new = editing;
            }
            RibbonAction::StopAllCalculations => self.stop_all_calculations(),
            RibbonAction::SetAutomaticCalculation(enabled) => {
                self.automatic_calculation = enabled;
                if enabled && self.calculation_pending {
                    self.calculate_command(CalculationCommand::Automatic);
                } else if !enabled {
                    self.pause_automatic_calculations();
                }
                self.focus_new = editing;
            }
            RibbonAction::SetBinOctHexMode(enabled) => {
                self.set_numeric_input_mode(ctx, enabled);
                self.focus_new = editing;
            }
            RibbonAction::SetRegionCalculationDisabled(disabled) => {
                self.set_selected_calculation_disabled(disabled);
                self.focus_new = editing;
            }
            RibbonAction::PreviousError => self.navigate_error(false),
            RibbonAction::NextError => self.navigate_error(true),
            RibbonAction::ToggleWarnings => self.show_warnings = !self.show_warnings,
            RibbonAction::Zoom(zoom) => self.zoom = zoom.clamp(0.5, 2.),
            RibbonAction::ToggleGrid => self.show_grid = !self.show_grid,
            RibbonAction::SetDraftView(enabled) => self.expand_right = enabled,
            RibbonAction::EditPageSections => {
                self.page_sections_draft = Some(PageSectionsDraft {
                    sections: self.document.page_sections.clone(),
                    next_id: self.document.next_id,
                });
            }
            RibbonAction::SetFontSize(size) => {
                self.change_math_format(|f| f.font_points = size.clamp(4., 72.))
            }
            RibbonAction::SetPrecision(precision) => {
                self.change_math_format(|f| f.precision = precision.min(15))
            }
            RibbonAction::SetResultFormat(style) => self.change_math_format(|f| {
                f.result_format = match style {
                    1 => ResultFormat::Decimal,
                    2 => ResultFormat::Scientific,
                    3 => ResultFormat::Engineering,
                    4 => ResultFormat::Percent,
                    _ => ResultFormat::General,
                }
            }),
            RibbonAction::SetComplexFormat(value) => {
                self.change_math_format(|f| f.complex_format = value)
            }
            RibbonAction::SetTrailingZeros(value) => {
                self.change_math_format(|f| f.show_trailing_zeros = value)
            }
            RibbonAction::SetZeroThreshold(value) => {
                self.change_math_format(|f| f.zero_threshold = value.min(307))
            }
            RibbonAction::SetComplexThreshold(value) => {
                self.change_math_format(|f| f.complex_threshold = value.min(307))
            }
            RibbonAction::SetExponentialThreshold(value) => {
                self.change_math_format(|f| f.exponential_threshold = value.min(15))
            }
            RibbonAction::SetLabel(label) => self.set_label(label),
            RibbonAction::InsertMatrixRow
            | RibbonAction::InsertMatrixColumn
            | RibbonAction::DeleteMatrixRow
            | RibbonAction::DeleteMatrixColumn
            | RibbonAction::ClearCells => self.matrix_command(action),
        }
    }

    fn page_sections_ui(&mut self, ctx: &egui::Context) {
        let Some(draft) = self.page_sections_draft.as_mut() else {
            return;
        };
        let mut apply = false;
        let mut cancel = false;
        egui::Window::new("머리글 및 바닥글")
            .id(egui::Id::new("page-sections-editor"))
            .collapsible(false)
            .resizable(true)
            .default_width(620.)
            .show(ctx, |ui| {
                ui.label(
                    "각 항목은 페이지마다 반복됩니다. 위치는 본문 좌표와 같은 포인트 단위입니다.",
                );
                ui.separator();
                page_band_ui(ui, "머리글", &mut draft.sections.header, &mut draft.next_id);
                ui.separator();
                page_band_ui(ui, "바닥글", &mut draft.sections.footer, &mut draft.next_id);
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("시작 페이지");
                    ui.add(
                        egui::DragValue::new(&mut draft.sections.starting_page_number)
                            .range(1..=999_999),
                    );
                    ui.label("전체 페이지 보정");
                    ui.add(
                        egui::DragValue::new(&mut draft.sections.total_pages_offset)
                            .range(0..=999_999),
                    );
                });
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("적용").clicked() {
                        apply = true;
                    }
                    if ui.button("취소").clicked() {
                        cancel = true;
                    }
                });
            });
        if apply {
            self.commit_page_sections_draft();
        } else if cancel {
            self.page_sections_draft = None;
        }
    }

    fn commit_page_sections_draft(&mut self) -> bool {
        let Some(mut draft) = self.page_sections_draft.take() else {
            return false;
        };
        let before = self.snapshot();
        draft.sections.explicit = true;
        self.document.page_sections = draft.sections;
        self.document.next_id = self.document.next_id.max(draft.next_id);
        self.remember(before);
        self.status = "머리글과 바닥글을 적용했습니다".into();
        true
    }

    fn plot_settings_ui(&mut self, ctx: &egui::Context) {
        let Some(draft) = self.plot_draft.as_mut() else {
            return;
        };
        let mut apply = false;
        let mut cancel = false;
        let mut remove = None;
        egui::Window::new("XY 도표 설정")
            .id(egui::Id::new("plot-settings-editor"))
            .collapsible(false)
            .resizable(true)
            .default_width(660.)
            .show(ctx, |ui| {
                ui.label("여러 곡선의 식, 범례, 선과 표식, 축 범위를 한 번에 편집합니다.");
                ui.separator();
                egui::ScrollArea::vertical()
                    .max_height(360.)
                    .show(ui, |ui| match &mut draft.spec.source {
                        PlotSource::Legacy { traces, .. } => {
                            let trace_count = traces.len();
                            for (index, trace) in traces.iter_mut().enumerate() {
                                egui::CollapsingHeader::new(format!("곡선 {}", index + 1))
                                    .default_open(index == 0)
                                    .show(ui, |ui| {
                                        egui::Grid::new(("plot-trace", index))
                                            .num_columns(2)
                                            .spacing(vec2(8., 4.))
                                            .show(ui, |ui| {
                                                ui.label("x 식");
                                                ui.text_edit_singleline(&mut trace.x_expression);
                                                ui.end_row();
                                                ui.label("y 식");
                                                ui.text_edit_singleline(&mut trace.y_expression);
                                                ui.end_row();
                                                plot_trace_appearance_ui(
                                                    ui,
                                                    index,
                                                    &mut trace.label,
                                                    &mut trace.style,
                                                );
                                            });
                                        if trace_count > 1
                                            && ui.small_button("이 곡선 삭제").clicked()
                                        {
                                            remove = Some(index);
                                        }
                                    });
                            }
                        }
                        PlotSource::AxisLists(lists) => {
                            for (axis, equations) in [("x", &mut lists.x), ("y", &mut lists.y)] {
                                egui::Grid::new(("plot-axis-equations", axis))
                                    .num_columns(2)
                                    .show(ui, |ui| {
                                        for (index, equation) in equations.iter_mut().enumerate() {
                                            ui.label(format!("{axis} 식 {}", index + 1));
                                            ui.push_id(equation.id, |ui| {
                                                if let Some(text) =
                                                    equation.source.as_legacy_text_mut()
                                                {
                                                    ui.text_edit_singleline(text);
                                                } else {
                                                    crate::plots::readonly_expression_ui(
                                                        ui,
                                                        &equation.source,
                                                        ui.id().with("source"),
                                                    );
                                                }
                                            });
                                            ui.end_row();
                                        }
                                    });
                            }
                            for (index, appearance) in lists.appearances.iter_mut().enumerate() {
                                egui::CollapsingHeader::new(format!("곡선 {}", index + 1))
                                    .default_open(index == 0)
                                    .show(ui, |ui| {
                                        egui::Grid::new(("plot-trace", index))
                                            .num_columns(2)
                                            .spacing(vec2(8., 4.))
                                            .show(ui, |ui| {
                                                plot_trace_appearance_ui(
                                                    ui,
                                                    index,
                                                    &mut appearance.label,
                                                    &mut appearance.style,
                                                );
                                            });
                                    });
                            }
                        }
                    });
                if let Some(index) = remove.take()
                    && draft.spec.remove_trace(index).is_ok()
                {
                    draft.topology_changed = true;
                }
                if matches!(draft.spec.source, PlotSource::Legacy { .. })
                    && ui
                        .add_enabled(
                            draft.spec.trace_count() < PlotSpec::MAX_TRACES,
                            egui::Button::new("+ 곡선 추가"),
                        )
                        .clicked()
                {
                    let index = draft.spec.trace_count();
                    let palette = [
                        [196, 50, 55, 255],
                        [34, 125, 69, 255],
                        [134, 72, 170, 255],
                        [204, 120, 20, 255],
                    ];
                    let mut trace = PlotTraceSpec::default();
                    trace.style.color = palette[index % palette.len()];
                    trace.label = format!("곡선 {}", index + 1);
                    if draft.spec.add_trace(trace).is_ok() {
                        draft.topology_changed = true;
                    }
                }
                ui.separator();
                ui.columns(2, |columns| {
                    plot_axis_ui(&mut columns[0], "x축", &mut draft.spec.x_axis);
                    plot_axis_ui(&mut columns[1], "y축", &mut draft.spec.y_axis);
                });
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("적용").clicked() {
                        apply = true;
                    }
                    if ui.button("취소").clicked() {
                        cancel = true;
                    }
                });
            });
        if apply {
            self.commit_plot_draft();
        } else if cancel {
            self.plot_draft = None;
        }
    }

    fn commit_plot_draft(&mut self) -> bool {
        let Some(draft) = self.plot_draft.as_ref() else {
            return false;
        };
        if let Err(error) = draft.spec.validate() {
            self.status = format!("도표 설정 오류: {error}");
            return false;
        }
        if !draft.topology_changed && self.document.regions.iter().any(|region| {
            region.id == draft.id
                && matches!(&region.content,CanvasContent::Plot(spec) if spec==&draft.spec || {let mut normalized=spec.clone();normalized.promote_legacy_trace();normalized==draft.spec})
        }) {
            self.plot_draft = None;
            return false;
        }
        let draft = self.plot_draft.take().unwrap();
        let before = self.snapshot();
        let Some(region) = self
            .document
            .regions
            .iter_mut()
            .find(|region| region.id == draft.id)
        else {
            self.status = "도표 영역을 찾을 수 없습니다".into();
            return false;
        };
        let changed_topology = draft.topology_changed
            || matches!(&region.content,CanvasContent::Plot(current) if !plot_lifecycle::same_plot_topology(current,&draft.spec));
        region.content = CanvasContent::Plot(draft.spec);
        self.remember(before);
        if changed_topology {
            self.discard_plot_previous_curves(draft.id);
        }
        self.mark_plot_commit_pending(draft.id);
        self.background.invalidate_reply();
        self.calculation_changed(false);
        self.status = "도표 설정을 적용했습니다".into();
        true
    }

    fn solve_block_ui(&mut self, ctx: &egui::Context) {
        let Some(draft) = self.solve_draft.as_mut() else {
            return;
        };
        let mut create = false;
        let mut cancel = false;
        let mut remove_guess = None;
        let mut remove_equality = None;
        egui::Window::new("풀이 구간")
            .id(egui::Id::new("solve-block-editor"))
            .collapsible(false)
            .resizable(true)
            .default_width(620.)
            .show(ctx, |ui| {
                ui.label("초깃값과 등식 제약을 입력하면 구조화된 Find/Minerr 구간을 만듭니다.");
                ui.horizontal(|ui| {
                    ui.label("방법");
                    ui.selectable_value(&mut draft.method, SolveMethod::Find, "Find");
                    ui.selectable_value(&mut draft.method, SolveMethod::Minerr, "Minerr");
                    ui.separator();
                    ui.label("결과 이름");
                    ui.text_edit_singleline(&mut draft.output);
                });
                ui.separator();
                ui.strong("초깃값");
                let guess_count = draft.guesses.len();
                for (index, guess) in draft.guesses.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut guess.name)
                                .desired_width(110.)
                                .hint_text("변수"),
                        );
                        ui.label(":=");
                        ui.add(
                            egui::TextEdit::singleline(&mut guess.value)
                                .desired_width(300.)
                                .hint_text("초깃값"),
                        );
                        if guess_count > 1 && ui.small_button("삭제").clicked() {
                            remove_guess = Some(index);
                        }
                    });
                }
                if let Some(index) = remove_guess.take() {
                    draft.guesses.remove(index);
                }
                if ui
                    .add_enabled(draft.guesses.len() < 16, egui::Button::new("+ 초깃값"))
                    .clicked()
                {
                    draft.guesses.push(SolveGuessDraft {
                        name: format!("x{}", draft.guesses.len() + 1),
                        value: "0".into(),
                    });
                }
                ui.separator();
                ui.strong("등식 제약");
                let equality_count = draft.equalities.len();
                for (index, equality) in draft.equalities.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut equality.left)
                                .desired_width(235.)
                                .hint_text("왼쪽 식"),
                        );
                        ui.label("=");
                        ui.add(
                            egui::TextEdit::singleline(&mut equality.right)
                                .desired_width(235.)
                                .hint_text("오른쪽 식"),
                        );
                        if equality_count > 1 && ui.small_button("삭제").clicked() {
                            remove_equality = Some(index);
                        }
                    });
                }
                if let Some(index) = remove_equality.take() {
                    draft.equalities.remove(index);
                }
                if ui
                    .add_enabled(
                        draft.equalities.len() < 64,
                        egui::Button::new("+ 등식 제약"),
                    )
                    .clicked()
                {
                    draft.equalities.push(SolveEqualityDraft {
                        left: String::new(),
                        right: String::new(),
                    });
                }
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("풀이 구간 만들기").clicked() {
                        create = true;
                    }
                    if ui.button("취소").clicked() {
                        cancel = true;
                    }
                });
            });
        if create {
            self.commit_solve_draft();
        } else if cancel {
            self.solve_draft = None;
        }
    }

    fn commit_solve_draft(&mut self) -> bool {
        let Some(draft) = self.solve_draft.as_ref() else {
            return false;
        };
        let build = (|| -> Result<String, String> {
            let mut builder = SolveBlockBuilder::new(draft.method);
            for guess in &draft.guesses {
                builder
                    .guess(guess.name.trim(), solve_expression(&guess.value)?)
                    .map_err(|error| error.to_string())?;
            }
            for equality in &draft.equalities {
                builder
                    .equality(
                        solve_expression(&equality.left)?,
                        solve_expression(&equality.right)?,
                    )
                    .map_err(|error| error.to_string())?;
            }
            builder
                .solve_for(draft.guesses.iter().map(|guess| guess.name.trim()))
                .map_err(|error| error.to_string())?;
            if !draft.output.trim().is_empty() {
                builder
                    .export_as(draft.output.trim())
                    .map_err(|error| error.to_string())?;
            }
            builder
                .build()
                .and_then(|model| model.to_solveblock_xml())
                .map_err(|error| error.to_string())
        })();
        let xml = match build {
            Ok(xml) => xml,
            Err(error) => {
                self.status = format!("풀이 구간 오류: {error}");
                return false;
            }
        };
        let before = self.snapshot();
        let position = self.insertion.unwrap_or_else(|| {
            self.selected
                .and_then(|id| self.document.regions.iter().find(|region| region.id == id))
                .map(|region| (region.left, region.top + self.document.grid * 5.))
                .unwrap_or((0., self.document.grid))
        });
        let id = self
            .document
            .add_opaque(position.0, position.1, xml, "solveblock");
        self.solve_draft = None;
        self.selected = Some(id);
        self.focus_new = None;
        self.insertion = None;
        self.reveal = Some(position);
        self.remember(before);
        self.calculation_changed(false);
        self.status = "풀이 구간을 만들었습니다".into();
        true
    }

    fn context_hint(&self) -> &'static str {
        if self.editing_region.is_some() {
            if self
                .selected
                .and_then(|id| self.document.regions.iter().find(|r| r.id == id))
                .is_some_and(|r| matches!(r.content, CanvasContent::Text(_)))
            {
                "텍스트 편집 · Esc 선택으로 돌아가기"
            } else if self.selected.is_some_and(|id| {
                self.document
                    .regions
                    .iter()
                    .any(|r| r.id == id && matches!(r.content, CanvasContent::Plot(_)))
            }) {
                "도표 편집 · 축의 식 입력 · Esc 영역 선택"
            } else {
                "수식 편집 · Space 범위 선택 · Enter 다음 영역"
            }
        } else if self.selected.is_some() {
            "방향키 이동 · Enter 편집 · Delete 삭제 · 우클릭 메뉴"
        } else if self.insertion.is_some() {
            if self.text_mode {
                "텍스트 입력 · Ctrl+Shift+M 수식"
            } else {
                "수식 입력 · : 정의 · = 계산 · Ctrl+T 텍스트"
            }
        } else {
            "클릭 후 입력 · Ctrl+T 텍스트 · F1 단축키"
        }
    }
    fn fit_width(&mut self, ctx: &egui::Context) {
        self.zoom =
            ((ctx.content_rect().width() - 64.) / self.document.width as f32).clamp(0.5, 2.);
    }
    fn global_editing_shortcuts(&mut self, ctx: &egui::Context) {
        self.paragraph_alignment_shortcuts(ctx);
        self.rich_text_shortcuts(ctx);
        self.page_break_shortcuts(ctx);
        if ctx.input_mut(|input| {
            consume_exact_key(
                input,
                egui::Modifiers {
                    shift: true,
                    ..egui::Modifiers::COMMAND
                },
                egui::Key::A,
            )
        }) && let Err(error) = self.insert_empty_area()
        {
            self.status = error;
        }
        if ctx.input_mut(|i| consume_exact_key(i, egui::Modifiers::COMMAND, egui::Key::P)) {
            self.open_print(false);
        }
        if ctx.input_mut(|i| consume_exact_key(i, egui::Modifiers::COMMAND, egui::Key::Num2)) {
            self.ribbon_command(ctx, RibbonAction::InsertPlot);
        }
        if ctx.input_mut(|i| consume_exact_key(i, egui::Modifiers::NONE, egui::Key::F1)) {
            self.show_shortcuts = !self.show_shortcuts;
        }
        if ctx.input_mut(|i| consume_exact_key(i, egui::Modifiers::CTRL, egui::Key::F5)) {
            self.recalculate();
        } else if ctx.input_mut(|i| consume_exact_key(i, egui::Modifiers::NONE, egui::Key::F5)) {
            self.calculate_command(CalculationCommand::Now);
        }
        if ctx.input_mut(|i| consume_exact_key(i, egui::Modifiers::COMMAND, egui::Key::T)) {
            self.begin_region(true);
        }
        if ctx.input_mut(|i| {
            consume_exact_key(
                i,
                egui::Modifiers {
                    shift: true,
                    ..egui::Modifiers::COMMAND
                },
                egui::Key::M,
            )
        }) {
            self.begin_region(false);
        }
        if ctx.input_mut(|i| consume_exact_key(i, egui::Modifiers::CTRL, egui::Key::F1)) {
            self.ribbon.minimized = !self.ribbon.minimized;
        }
    }
    fn region_keyboard(&mut self, ui: &mut egui::Ui, previous_editing: Option<u64>) {
        if self.pending.is_some() || ui.ctx().any_popup_open() {
            return;
        }
        let escape = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        let toggle = if self.can_request_page_break() {
            ui.input_mut(|i| consume_exact_key(i, egui::Modifiers::COMMAND, egui::Key::Enter))
        } else {
            ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Enter))
        };
        if escape || (toggle && self.editing_region.is_some()) {
            // egui can release focus for Escape before this widget is visited.
            // Preserve the region selection for the first Escape in either case.
            if self
                .editing_region
                .take()
                .or(previous_editing.filter(|id| Some(*id) == self.selected))
                .is_none()
            {
                self.insertion = self
                    .selected
                    .and_then(|id| self.document.regions.iter().find(|r| r.id == id))
                    .map(|r| (r.left, r.top));
                self.selected = None;
            }
            self.focus_new = None;
            ui.memory_mut(|m| {
                if let Some(id) = m.focused() {
                    m.surrender_focus(id);
                }
            });
            return;
        }
        if self.editing_region.is_some() {
            return;
        }
        if self.can_request_page_break() {
            if toggle {
                self.queue_page_break(page_break_ui::PageBreakRequest::Insert);
                return;
            }
            if ui.input_mut(|i| consume_exact_key(i, egui::Modifiers::NONE, egui::Key::Backspace)) {
                self.queue_page_break(page_break_ui::PageBreakRequest::Above);
                return;
            }
            if ui.input_mut(|i| consume_exact_key(i, egui::Modifiers::NONE, egui::Key::Delete)) {
                self.queue_page_break(page_break_ui::PageBreakRequest::Below);
                return;
            }
        }
        if (toggle || ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter)))
            && self.selected.is_some()
        {
            self.focus_new = self.selected;
            ui.ctx().request_repaint();
            return;
        }
        let direction = ui.input_mut(|i| {
            for (key, x, y) in [
                (egui::Key::ArrowLeft, -1., 0.),
                (egui::Key::ArrowRight, 1., 0.),
                (egui::Key::ArrowUp, 0., -1.),
                (egui::Key::ArrowDown, 0., 1.),
            ] {
                if i.consume_key(
                    egui::Modifiers {
                        shift: true,
                        ..egui::Modifiers::NONE
                    },
                    key,
                ) {
                    return Some((x * 5., y * 5.));
                }
                if i.consume_key(egui::Modifiers::NONE, key) {
                    return Some((x, y));
                }
            }
            None
        });
        if let Some((x, y)) = direction {
            self.nudge(x, y);
        }
        let tab = ui.input_mut(|i| {
            if i.consume_key(
                egui::Modifiers {
                    shift: true,
                    ..egui::Modifiers::NONE
                },
                egui::Key::Tab,
            ) {
                Some(false)
            } else if i.consume_key(egui::Modifiers::NONE, egui::Key::Tab) {
                Some(true)
            } else {
                None
            }
        });
        if let Some(forward) = tab {
            let mut regions: Vec<_> = self
                .document
                .regions
                .iter()
                .filter(|region| !page_break_ui::is_marker(region))
                .collect();
            regions.sort_by(|a, b| {
                a.top
                    .total_cmp(&b.top)
                    .then(a.left.total_cmp(&b.left))
                    .then(a.id.cmp(&b.id))
            });
            if !regions.is_empty() {
                let here = regions.iter().position(|r| Some(r.id) == self.selected);
                let next = match (here, forward) {
                    (Some(i), true) => (i + 1) % regions.len(),
                    (Some(i), false) => (i + regions.len() - 1) % regions.len(),
                    (None, true) => 0,
                    (None, false) => regions.len() - 1,
                };
                self.selected = Some(regions[next].id);
                self.insertion = None;
                self.reveal = Some((regions[next].left, regions[next].top));
            }
        }
    }
    fn nudge(&mut self, x: f64, y: f64) {
        let step = self.document.grid;
        let width = if self.expand_right {
            f64::from(self.paper_size().x)
        } else {
            self.document.width
        };
        if let Some(region) = self
            .selected
            .and_then(|id| self.document.regions.iter().find(|r| r.id == id))
        {
            let id = region.id;
            let left = (region.left + x * step).clamp(0., (width - step).max(0.));
            let top = (region.top + y * step).max(0.);
            if region.left == left && region.top == top {
                return;
            }
            let before = self.snapshot();
            if self.document.move_region(id, left, top).is_ok() {
                self.remember(before);
                self.calculation_changed(false);
                self.reveal = Some((left, top));
            }
        } else if let Some((left, top)) = self.insertion {
            self.insertion = Some((
                (left + x * step).clamp(0., (width - step).max(0.)),
                (top + y * step).max(0.),
            ));
            self.reveal = self.insertion;
        }
    }
    fn shortcuts_ui(&mut self, ctx: &egui::Context) {
        if self.show_shortcuts {
            egui::Window::new("키보드로 작성하기")
                .open(&mut self.show_shortcuts)
                .resizable(false)
                .show(ctx, |ui| {
                    egui::Grid::new("shortcut-list")
                        .spacing(vec2(24., 10.))
                        .show(ui, |ui| {
                            for (key, action) in [
                                (": / =", "정의 / 계산"),
                                ("/ · ^ · √", "분수 · 거듭제곱 · 제곱근"),
                                ("Space", "부분식 선택 범위 확장"),
                                ("Tab / Shift+Tab", "다음 / 이전 입력 칸"),
                                ("↑ / ↓", "수식의 위 / 아래 입력 칸"),
                                ("Shift+← / →", "문자 범위 선택"),
                                ("Enter / Esc", "다음 수식 / 영역 선택"),
                                ("Ctrl+T", "텍스트 영역 삽입"),
                                ("Ctrl+2", "XY 도표 삽입"),
                                ("Shift+Enter / Space", "행렬 행 / 열 삽입"),
                                ("Ctrl+Z / Ctrl+Y", "실행 취소 / 다시 실행"),
                                ("Ctrl+−", "이름의 문자 아래첨자 전환"),
                                ("Ctrl+F1", "리본 최소화 / 복원"),
                                ("F5", "지금 계산 · 보이는 영역"),
                                ("Ctrl+F5", "워크시트 전체 다시 계산"),
                            ] {
                                ui.label(egui::RichText::new(key).strong());
                                ui.label(action);
                                ui.end_row();
                            }
                        });
                });
        }
    }

    fn page_field_preview(&self, field: PageField, page: usize, pages: usize) -> String {
        let number = self.document.page_sections.starting_page_number + page as u32;
        let total = self
            .document
            .page_sections
            .starting_page_number
            .saturating_add(pages.saturating_sub(1) as u32)
            .saturating_add(self.document.page_sections.total_pages_offset);
        match field {
            PageField::PageNumber(format) => match format {
                PageNumberFormat::Number => number.to_string(),
                PageNumberFormat::PageNumber => format!("Page {number}"),
                PageNumberFormat::AbbreviatedPageNumber => format!("p. {number}"),
                PageNumberFormat::NumberOfTotal => format!("{number} of {total}"),
                PageNumberFormat::PageNumberOfTotal => format!("Page {number} of {total}"),
                PageNumberFormat::DashedNumber => format!("- {number} -"),
                PageNumberFormat::ParenthesizedNumber => format!("({number})"),
                PageNumberFormat::NumberParenthesizedTotal => format!("{number} ({total})"),
            },
            PageField::DateSaved(format) => self.saved_date.format(format),
            PageField::FileName => self
                .path
                .as_ref()
                .and_then(|path| path.file_name())
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| self.document.title.clone()),
            PageField::FilePath => self
                .path
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| self.document.title.clone()),
        }
    }

    fn paint_page_band(
        &self,
        ui: &egui::Ui,
        sheet: Rect,
        page: usize,
        pages: usize,
        geometry: crate::page::PageGeometry,
    ) {
        for (band, footer) in [
            (&self.document.page_sections.header, false),
            (&self.document.page_sections.footer, true),
        ] {
            for region in &band.regions {
                let text = match &region.content {
                    PageBandContent::Text(text) => text.clone(),
                    PageBandContent::Field(field) => self.page_field_preview(*field, page, pages),
                };
                let base_y = if footer {
                    sheet.bottom() - geometry.margins[3]
                } else {
                    sheet.top()
                };
                ui.painter().galley(
                    pos2(
                        sheet.left() + region.left as f32,
                        base_y + region.top as f32,
                    ),
                    typography::layout_no_wrap(
                        ui.painter(),
                        text,
                        typography::body_font(),
                        Color32::from_rgb(70, 75, 78),
                    ),
                    Color32::from_rgb(70, 75, 78),
                );
            }
        }
    }

    fn page_geometry(&self) -> crate::page::PageGeometry {
        crate::page::PageGeometry::new(
            &self.document,
            if self.expand_right {
                [0.; 4]
            } else {
                self.document
                    .page_setup
                    .as_ref()
                    .map(|setup| setup.margins.map(|v| v as f32))
                    .unwrap_or(self.page_margins)
            },
        )
    }
    fn paper_size(&self) -> egui::Vec2 {
        let bottom = self
            .document
            .regions
            .iter()
            .map(|r| {
                r.top
                    + f64::from(self.region_heights.get(&r.id).copied().unwrap_or({
                        if let CanvasContent::Plot(p) = &r.content {
                            p.height
                        } else {
                            180.
                        }
                    }))
                    + 8.
            })
            .chain(self.insertion.map(|(_, y)| y + 80.))
            .chain(self.drag_preview.map(|(id, _, y)| {
                y + f64::from(self.region_heights.get(&id).copied().unwrap_or(180.)) + 8.
            }))
            .fold(0., f64::max);
        let mut size = self.page_geometry().size(bottom);
        if self.expand_right {
            size.x = (size.x * 2.).max(self.right_content_edge());
        }
        size
    }

    fn right_content_edge(&self) -> f32 {
        let geometry = self.page_geometry();
        self.document
            .regions
            .iter()
            .map(|region| {
                geometry.to_paper(region.left, region.top).x
                    + self.region_widths.get(&region.id).copied().unwrap_or(0.)
                    + 24.
            })
            .fold(geometry.width, f32::max)
    }
    fn workspace_ui(&mut self, ui: &mut egui::Ui) {
        #[cfg(test)]
        full_app_profile::observe_workspace(ui);
        if ui.rect_contains_pointer(ui.max_rect()) {
            self.zoom = (self.zoom * ui.input(|i| i.zoom_delta())).clamp(0.5, 2.);
        }
        let available = ui.available_width();
        egui::ScrollArea::both()
            .id_salt("free-worksheet")
            .show(ui, |ui| {
                let size = self.paper_size();
                let scaled = size * self.zoom;
                let page_left =
                    ((available - self.document.width as f32 * self.zoom) / 2.).max(24.);
                let width = available.max(page_left + scaled.x + 24.);
                let (bounds, _) =
                    ui.allocate_exact_size(vec2(width, scaled.y + 48.), Sense::hover());
                let origin = bounds.min + vec2(page_left, 24.);
                let toggle = Rect::from_min_size(
                    origin + vec2(self.document.width as f32 * self.zoom - 12., -22.),
                    vec2(26., 20.),
                );
                if (self.expand_right || self.right_content_edge() > self.document.width as f32)
                    && ui
                        .put(
                            toggle,
                            egui::Button::new(if self.expand_right { "◀" } else { "▶" }),
                        )
                        .on_hover_text(if self.expand_right {
                            "페이지 보기로 전환"
                        } else {
                            "초안 보기로 전환 · 인쇄 영역 밖 펼치기"
                        })
                        .clicked()
                {
                    self.expand_right = !self.expand_right;
                    ui.ctx().request_repaint();
                }
                let paper_rect = Rect::from_min_size(origin, scaled);
                ui.painter().rect_filled(
                    paper_rect.translate(vec2(0., 3.)).expand(2.),
                    2.,
                    Color32::from_black_alpha(12),
                );
                let layer =
                    egui::LayerId::new(ui.layer_id().order, egui::Id::new("worksheet-paper"));
                let transform = egui::emath::TSTransform::from_translation(origin.to_vec2())
                    * egui::emath::TSTransform::from_scaling(self.zoom);
                ui.ctx().set_sublayer(ui.layer_id(), layer);
                ui.ctx().set_transform_layer(layer, transform);
                let mut page = ui.new_child(
                    UiBuilder::new()
                        .id_salt("paper-content")
                        .layer_id(layer)
                        .max_rect(Rect::from_min_size(egui::Pos2::ZERO, size))
                        .layout(egui::Layout::top_down(egui::Align::Min)),
                );
                page.set_clip_rect(transform.inverse() * ui.clip_rect());
                typography::with_worksheet_zoom(ui.ctx(), self.zoom, || self.page_ui(&mut page));
                if let Some((x, y)) = self.reveal.take() {
                    let target = Rect::from_min_size(
                        origin + self.page_geometry().to_paper(x, y).to_vec2() * self.zoom,
                        vec2(120., 80.) * self.zoom,
                    );
                    ui.scroll_to_rect(target, None);
                }
            });
    }
    fn page_ui(&mut self, ui: &mut egui::Ui) {
        self.prepare_rich_text();
        let composing_text_owner = self
            .text_composition
            .as_ref()
            .map(|composition| composition.id);
        plain_text_input::reconcile(
            &mut self.text_composition,
            ui.ctx(),
            self.selected,
            self.focus_new,
        );
        formula::set_numeric_input_mode(ui.ctx(), self.numeric_input_mode().unwrap_or(false));
        self.release_retired_plot_widgets(ui.ctx());
        #[cfg(test)]
        let input_timing = crate::frame_profile::scope(crate::frame_profile::Phase::Input);
        // Native table-header Copy creates a table object, rather than a
        // matrix or text to insert into an equation. Route it at the worksheet
        // boundary even when the destination cell still has keyboard focus.
        let table_paste = ui.input(|input| {
            input.events.iter().find_map(|event| {
                if let egui::Event::Paste(text) = event {
                    text.starts_with("mathcraft-table:").then(|| text.clone())
                } else {
                    None
                }
            })
        });
        if let Some(text) = table_paste {
            let count = self.document.regions.len();
            self.paste_region(&text);
            if self.document.regions.len() != count {
                ui.memory_mut(|memory| {
                    if let Some(id) = memory.focused() {
                        memory.surrender_focus(id);
                    }
                });
            }
            ui.input_mut(|input| input.events.retain(|event| !matches!(event, egui::Event::Paste(text) if text.starts_with("mathcraft-table:"))));
        }
        let pointer_commits_table = ui.input(|input| {
            input
                .events
                .iter()
                .any(|event| matches!(event, egui::Event::PointerButton { .. }))
        }) && self
            .table_editors
            .values()
            .any(|editor| editor.has_draft_changes());
        let pointer_commits_area = ui.input(|input|input.pointer.any_pressed())
            && (self.area_editors.values().any(|area|area.has_draft_changes())
                || ui.input(|input|input.pointer.interact_pos()).is_some_and(|position| {
                    let origin=ui.available_rect_before_wrap().min;
                    let geometry=self.page_geometry();
                    self.document.regions.iter().filter(|region|matches!(&region.content,CanvasContent::Opaque{kind,..}if kind=="Area")).any(|region| {
                        Rect::from_min_size(origin+geometry.to_paper(region.left,region.top).to_vec2(),vec2(self.region_widths.get(&region.id).copied().unwrap_or(280.),self.region_heights.get(&region.id).copied().unwrap_or(22.))).contains(position)
                    })
                }));
        let can_edit =
            pointer_commits_table || pointer_commits_area || input_intent::page_may_edit(self, ui);
        if can_edit {
            self.retain_committed_input();
        }
        let before = can_edit.then(|| {
            if !pointer_commits_table && ui.input(|input| input.pointer.any_pressed()) {
                self.snapshot()
            } else {
                self.input_snapshot()
            }
        });
        // A blank-page press changes no source. Capture the affected region
        // only after a move handle actually owns a press, before any movement.
        if ui.input(|input| input.pointer.any_pressed()) {
            self.pointer_history = None;
        }
        let previous_editing = self.editing_region;
        let rich_headroom = can_edit
            .then_some(self.selected)
            .flatten()
            .filter(|id| {
                self.rich_text.editors.get(id).is_some_and(Result::is_ok)
                    || self
                        .rich_text
                        .authored_editors
                        .get(id)
                        .is_some_and(Result::is_ok)
            })
            .map(|id| self.document.text_source_headroom(id));
        #[cfg(test)]
        drop(input_timing);
        #[cfg(test)]
        let geometry_timing = crate::frame_profile::scope(crate::frame_profile::Phase::Geometry);
        let size = self.paper_size();
        let geometry = self.page_geometry();
        let pages = ((size.y + geometry.grid) / geometry.stride()).round() as usize;
        let (paper, background) = ui.allocate_exact_size(size, Sense::click());
        let grid = geometry.grid.max(6.);
        let visible = paper.intersect(ui.clip_rect());
        let first_page = ((visible.top() - paper.top()) / geometry.stride())
            .floor()
            .max(0.) as usize;
        self.print_ui.current_page = first_page;
        let last_page = (((visible.bottom() - paper.top()) / geometry.stride())
            .floor()
            .max(0.) as usize
            + 1)
        .min(pages);
        #[cfg(test)]
        drop(geometry_timing);
        #[cfg(test)]
        let background_timing =
            crate::frame_profile::scope(crate::frame_profile::Phase::Background);
        for page in first_page..last_page {
            let sheet = Rect::from_min_size(
                paper.min + vec2(0., page as f32 * geometry.stride()),
                vec2(size.x, geometry.height),
            );
            if !sheet.intersects(visible) {
                continue;
            }
            ui.painter()
                .rect_filled(sheet, 0., Color32::from_rgb(246, 250, 246));
            ui.painter().rect_stroke(
                sheet,
                0.,
                Stroke::new(0.5, Color32::from_gray(160)),
                egui::StrokeKind::Inside,
            );
            if !self.expand_right {
                self.paint_page_band(ui, sheet, page, pages, geometry);
            }
            if self.expand_right {
                let boundary = sheet.left() + geometry.width;
                ui.painter().extend(egui::Shape::dashed_line(
                    &[pos2(boundary, sheet.top()), pos2(boundary, sheet.bottom())],
                    Stroke::new(1., Color32::from_gray(150)),
                    5.,
                    4.,
                ));
                let outside_color = Color32::from_gray(110);
                let outside_galley = typography::layout_no_wrap(
                    ui.painter(),
                    "인쇄 영역 밖".into(),
                    typography::body_font(),
                    outside_color,
                );
                ui.painter().galley(
                    pos2(boundary + 12., sheet.top() + 8.),
                    outside_galley,
                    outside_color,
                );
            }
            if self.show_grid {
                let body_origin = paper.min + geometry.body_origin(page).to_vec2();
                let body = Rect::from_min_size(
                    body_origin,
                    vec2(
                        if self.expand_right {
                            size.x - geometry.margins[0]
                        } else {
                            geometry.body_width()
                        },
                        geometry.body_height(),
                    ),
                );
                let clip = body.intersect(visible);
                let grid_stroke = |distance: f32| {
                    let major = distance / geometry.major_spacing();
                    if self.use_major_grid && (major - major.round()).abs() < 0.001 {
                        Stroke::new(1.2, Color32::from_rgb(181, 218, 249))
                    } else {
                        Stroke::new(0.6, Color32::from_rgb(189, 222, 248))
                    }
                };
                let mut x =
                    body.left() + ((clip.left() - body.left()) / grid).ceil().max(0.) * grid;
                while x <= clip.right() {
                    ui.painter().line_segment(
                        [pos2(x, clip.top()), pos2(x, clip.bottom())],
                        grid_stroke(x - body.left()),
                    );
                    x += grid;
                }
                let mut y = body.top() + ((clip.top() - body.top()) / grid).ceil().max(0.) * grid;
                while y <= clip.bottom() {
                    ui.painter().line_segment(
                        [pos2(clip.left(), y), pos2(clip.right(), y)],
                        grid_stroke(y - body.top()),
                    );
                    y += grid;
                }
            }
        }
        #[cfg(test)]
        drop(background_timing);
        let mut changed = false;
        let mut math_edits = Vec::new();
        let mut text_edits = Vec::new();
        let mut table_commits = Vec::new();
        let mut area_commits = Vec::new();
        let mut area_drafts = Vec::new();
        let mut plot_commits = false;
        let mut plot_resizes = Vec::new();
        let mut plot_resize_changes = Vec::new();
        let mut matrix_gestures = Vec::new();
        let mut matrix_painted = None;
        let mut evaluation_requested = false;
        let mut current_formula_geometry = None;
        let mut geometry_changed = false;
        let mut hit_region = false;
        let mut context_selection = None;
        let mut context_action = None;
        self.visible_regions.clear();
        self.visible_calculation_end = None;
        let mut last_visible_position: Option<(f64, f64)> = None;
        self.editing_region = None;
        self.paint_page_break_markers(ui, paper, geometry);
        for region in &mut self.document.regions {
            if page_break_ui::is_marker(region) {
                continue;
            }
            #[cfg(test)]
            let _scan_timing = crate::frame_profile::scope(crate::frame_profile::Phase::RegionScan);
            if let Some((moving, x, y)) = self.dragging
                && moving == region.id
                && let Some(delta) = ui.input(|input| {
                    self.drag_pointer_origin
                        .zip(input.pointer.latest_pos())
                        .map(|(origin, current)| current - origin)
                })
            {
                let scale = ui
                    .ctx()
                    .layer_transform_from_global(ui.layer_id())
                    .map_or(1., |transform| transform.scaling);
                let target = geometry.to_paper(x, y) + delta * scale;
                let (x, y) = geometry.to_body_clamped(target);
                self.drag_preview = Some((moving, x, y));
            }
            let (paint_left, paint_top) = self
                .drag_preview
                .filter(|(id, _, _)| *id == region.id)
                .map(|(_, x, y)| (x, y))
                .unwrap_or((region.left, region.top));
            let mut origin = paper.min + geometry.to_paper(paint_left, paint_top).to_vec2();
            let native_visual = self.native_visuals.region(region);
            let visual_error = native_visual.and_then(|v| v.as_ref().err());
            let visual = native_visual.and_then(|v| v.as_ref().ok());
            let native_source = native_alignment(region, &self.native_visuals);
            let anchor_height_bound = if native_source {
                self.region_heights
                    .get(&region.id)
                    .copied()
                    .or_else(|| visual.map(|v| v.size[1]))
                    .unwrap_or(0.)
            } else {
                0.
            };
            let above = self
                .region_heights
                .get(&region.id)
                .copied()
                .or_else(|| {
                    unmeasured_height_bound(
                        region,
                        self.results.get(&region.id),
                        self.display_cache.get(&region.id),
                        self.saved_results.contains_key(&region.id),
                    )
                })
                .is_some_and(|height| origin.y + height < ui.clip_rect().top());
            if (origin.y - anchor_height_bound > ui.clip_rect().bottom() || above)
                && self.selected != Some(region.id)
                && self.focus_new != Some(region.id)
            {
                continue;
            }
            let left_of_view = self
                .region_widths
                .get(&region.id)
                .is_some_and(|width| origin.x + width + 20. < visible.left());
            if (origin.x > visible.right() || left_of_view)
                && self.selected != Some(region.id)
                && self.focus_new != Some(region.id)
            {
                continue;
            }
            let max = Rect::from_min_size(
                origin,
                vec2((paper.right() - origin.x - 15.).max(100.), 220.),
            );
            let id = region.id;
            let rich_visual = visual.filter(|v| {
                v.document.is_some()
                    && (matches!(region.content, CanvasContent::Opaque { .. })
                        || self.rich_text.editors.get(&id).is_some_and(Result::is_err)
                        || self
                            .rich_text
                            .authored_editors
                            .get(&id)
                            .is_some_and(Result::is_err)
                        || (previous_editing != Some(id) && self.focus_new != Some(id)))
            });
            let mut child = ui.new_child(
                UiBuilder::new()
                    .id_salt(("region", id))
                    .max_rect(max)
                    .layout(egui::Layout::left_to_right(egui::Align::Min)),
            );
            child.set_clip_rect(paper.intersect(ui.clip_rect()));
            let row_result = self.results.get(&id);
            let saved_result = self.saved_results.get(&id);
            let result_pending = region.calculation_disabled || self.pending_regions.contains(&id);
            if result_pending
                && !matches!(&region.content, CanvasContent::Opaque{kind,..}if kind=="Area")
            {
                child.set_opacity(0.45);
            }
            #[cfg(test)]
            let display_timing = crate::frame_profile::scope(crate::frame_profile::Phase::Display);
            let matrix_window = self.matrix_view_sources.window(
                id,
                matrix_result_view::requested_view(
                    &self.document.matrix_result_views,
                    self.matrix_view_drag.as_ref(),
                    id,
                ),
            );
            let matrix_indices_metadata = matrix_result_view::result_indices_metadata(
                &self.matrix_view_sources,
                id,
                result_pending,
                row_result,
                matrix_result_view::requested_view(
                    &self.document.matrix_result_views,
                    self.matrix_view_drag.as_ref(),
                    id,
                ),
            );
            let (display, formula_display) = if matches!(region.content, CanvasContent::Math(_)) {
                (
                    None,
                    matrix_result_view::cached_formula_display(
                        &mut self.display_cache,
                        id,
                        region.format,
                        row_result,
                        saved_result,
                        matrix_window,
                    ),
                )
            } else {
                let display = cached_display(
                    &mut self.display_cache,
                    id,
                    region.format,
                    row_result,
                    saved_result,
                );
                (display, display.map(formula::FormulaResult::Formatted))
            };
            let formula_display = match matrix_result_view::indexed_formula_display(
                formula_display,
                matrix_indices_metadata,
            ) {
                Ok(display) => display,
                Err(error) => {
                    self.status = error;
                    formula_display
                }
            };
            #[cfg(test)]
            drop(display_timing);
            if native_source {
                #[cfg(test)]
                let _timing = crate::frame_profile::scope(crate::frame_profile::Phase::Alignment);
                let offset = if let Some(visual) = rich_visual {
                    #[cfg(test)]
                    let _rich_timing =
                        crate::frame_profile::scope(crate::frame_profile::Phase::AlignmentRich);
                    let width = if visual.size[0] > 0. {
                        visual.size[0]
                    } else {
                        max.width()
                    };
                    crate::native_presentation::alignment_offset(
                        &child,
                        visual,
                        width,
                        matches!(&region.content, CanvasContent::Opaque {kind, ..} if kind == "spec-table"),
                    )
                } else {
                    match &region.content {
                        CanvasContent::Math(tree) => {
                            #[cfg(test)]
                            let _math_timing = crate::frame_profile::scope(
                                crate::frame_profile::Phase::AlignmentMath,
                            );
                            #[cfg(test)]
                            let editor_timing = crate::frame_profile::scope(
                                crate::frame_profile::Phase::AlignmentEditor,
                            );
                            let editor = self
                                .editors
                                .entry(id)
                                .or_insert_with(|| Editor::from_portable(tree).unwrap_or_default());
                            #[cfg(test)]
                            drop(editor_timing);
                            origin.x += 2.; // Native equation content starts four DIP after its location.
                            -formula::baseline_offset_styled(
                                &child,
                                editor,
                                formula_display,
                                self.labelled_trees.get(&id),
                                region.format.font_points,
                            )
                        }
                        CanvasContent::Text(text) => {
                            #[cfg(test)]
                            let _text_timing = crate::frame_profile::scope(
                                crate::frame_profile::Phase::AlignmentText,
                            );
                            origin.x += 5.;
                            let font = visual
                                .and_then(|v| v.document.as_ref())
                                .map(|d| FontId::proportional(d.style.font_size))
                                .unwrap_or_else(typography::body_font);
                            let galley = typography::layout_job(
                                child.painter(),
                                egui::text::LayoutJob::simple(
                                    text.clone(),
                                    font,
                                    Color32::BLACK,
                                    340.,
                                ),
                            );
                            -galley
                                .rows
                                .first()
                                .and_then(|row| row.glyphs.first().map(|g| row.pos.y + g.pos.y))
                                .unwrap_or(0.)
                        }
                        _ => 0.,
                    }
                };
                origin.y += offset;
                child = ui.new_child(
                    UiBuilder::new()
                        .id_salt(("region", id))
                        .max_rect(Rect::from_min_size(origin, max.size()))
                        .layout(egui::Layout::left_to_right(egui::Align::Min)),
                );
                child.set_clip_rect(paper.intersect(ui.clip_rect()));
                if result_pending {
                    child.set_opacity(0.45);
                }
            }
            let background_slot = visual
                .filter(|v| v.background.is_some() && rich_visual.is_none())
                .map(|_| child.painter().add(egui::Shape::Noop));
            #[cfg(test)]
            let content_timing = crate::frame_profile::scope(crate::frame_profile::Phase::Content);
            let response = if let CanvasContent::Opaque { xml, kind } = &mut region.content
                && kind == "Area"
            {
                let created = if let std::collections::hash_map::Entry::Vacant(entry) =
                    self.area_editors.entry(id)
                {
                    match area_ui::AreaEditor::new(xml) {
                        Ok(editor) => {
                            entry.insert(editor);
                            Ok(())
                        }
                        Err(error) => Err(error),
                    }
                } else {
                    Ok(())
                };
                match created {
                    Err(error) => child.colored_label(RED, error),
                    Ok(()) => {
                        let cache = self.area_presentations.entry(id).or_default();
                        let refreshed = cache.refresh(xml, region.format);
                        if refreshed.as_ref().is_ok_and(|changed| *changed)
                            || self
                                .area_prepared
                                .get(&id)
                                .is_none_or(|(epoch, _)| *epoch != self.area_epoch)
                        {
                            let empty_live = HashMap::new();
                            let empty_saved = HashMap::new();
                            let prepared = cache.prepare(
                                id,
                                self.area_results.get(&id).unwrap_or(&empty_live),
                                self.area_saved.get(&id).unwrap_or(&empty_saved),
                                &self.area_pending,
                            );
                            self.area_prepared.insert(id, (self.area_epoch, prepared));
                        }
                        let native = self.area_visuals.get(&id).map(Vec::as_slice).unwrap_or(&[]);
                        let mut views = self.area_prepared[&id].1.borrow_views(native);
                        if let Some(labelled) = self
                            .committed_input
                            .as_ref()
                            .and_then(|input| input.area_labelled.get(&id))
                        {
                            for (key, view) in &mut views {
                                view.labelled = labelled.get(key);
                            }
                        }
                        if self.uncommitted_math.contains(&id) {
                            for view in views.values_mut() {
                                view.pending = true;
                            }
                        }
                        let editor = self.area_editors.get_mut(&id).unwrap();
                        let _ = editor.set_input_grid(self.document.grid);
                        let first_child = editor
                            .math_sources()
                            .next()
                            .map(|source| area_ui::child_key(&source.region_path));
                        if self.focus_new == Some(id) && editor.active_child().is_none() {
                            if let Some(key) = first_child {
                                let _ = editor.activate(&key);
                            } else {
                                let _ = editor.activate_insertion();
                            }
                        }
                        let outcome =
                            editor.show(&mut child, xml, &views, region.format.font_points);
                        if let Some(error) = outcome.error {
                            self.status = error;
                        }
                        if outcome.document_changed {
                            self.dirty = true;
                            ui.ctx().request_repaint();
                        }
                        changed |= outcome.history_changed;
                        if outcome.calculation_changed {
                            math_edits.push(id);
                            area_commits.push(id);
                        }
                        if outcome.draft_changed || outcome.child_created.is_some() {
                            changed = true;
                            area_drafts.push(id);
                        }
                        if outcome.commit_requested {
                            area_commits.push(id);
                        }
                        evaluation_requested |= outcome.evaluation_requested.is_some();
                        if outcome.focused {
                            self.selected = Some(id);
                            self.editing_region = Some(id);
                            self.insertion = None;
                        }
                        outcome.response
                    }
                }
            } else if let (CanvasContent::Opaque { xml, kind }, Some(visual)) =
                (&mut region.content, rich_visual)
                && matches!(kind.as_str(), "spec-table" | "specTable")
            {
                if let std::collections::hash_map::Entry::Vacant(entry) =
                    self.table_editors.entry(id)
                {
                    match spec_table_ui::TableEditor::new(xml) {
                        Ok(editor) => {
                            entry.insert(editor);
                        }
                        Err(error) => {
                            self.status = error;
                        }
                    }
                }
                if let Some(editor) = self.table_editors.get_mut(&id) {
                    if self.selected != Some(id) && self.focus_new != Some(id) {
                        editor.clear_range_selection();
                    }
                    let outcome = editor.show(
                        &mut child,
                        xml,
                        visual,
                        if visual.size[0] > 0. {
                            visual.size[0]
                        } else {
                            max.width()
                        },
                        self.focus_new == Some(id),
                        region.format.font_points,
                    );
                    if outcome.committed {
                        changed = true;
                        math_edits.push(id);
                        table_commits.push(id);
                    }
                    if outcome.draft_changed {
                        self.dirty = true;
                    }
                    if outcome.focused {
                        self.selected = Some(id);
                        self.editing_region = Some(id);
                    }
                    outcome.response
                } else {
                    crate::native_presentation::draw(&mut child, visual, max.width())
                }
            } else if let Some(visual) = rich_visual {
                let width = if visual.size[0] > 0. {
                    visual.size[0]
                } else {
                    max.width()
                };
                let drawn = crate::native_presentation::draw(&mut child, visual, width);
                let response = child.interact(
                    drawn.rect,
                    egui::Id::new(("native-region", id)),
                    Sense::click(),
                );
                if response.clicked() && matches!(region.content, CanvasContent::Text(_)) {
                    context_action = Some((id, RegionAction::Edit));
                }
                response
            } else {
                match &mut region.content {
                    CanvasContent::Math(tree) => {
                        let editor = self
                            .editors
                            .entry(id)
                            .or_insert_with(|| Editor::from_portable(tree).unwrap_or_default());
                        let outcome = formula::formula_ui_styled(
                            &mut child,
                            editor,
                            formula_display,
                            self.focus_new == Some(id),
                            self.labelled_trees.get(&id),
                            region.format.font_points,
                        );
                        #[cfg(test)]
                        matrix_result_view_app_tests::observe_formula(id, &outcome.response);
                        evaluation_requested |= outcome.evaluation_requested;
                        if self
                            .matrix_view_drag
                            .as_ref()
                            .map(matrix_result_view::Drag::owner)
                            == Some(id)
                            || outcome.matrix_view_gesture.is_some()
                        {
                            matrix_painted = Some(id);
                        }
                        if let Some(gesture) = outcome.matrix_view_gesture {
                            hit_region = true;
                            matrix_gestures.push((
                                id,
                                if outcome.response.changed() {
                                    formula::MatrixViewGesture::Cancel
                                } else {
                                    gesture
                                },
                            ));
                        }
                        // Reuse only this frame's unchanged intrinsic layout.
                        // Native/labelled layouts require a fresh styled measurement.
                        if self.selected == Some(id)
                            && !native_source
                            && !outcome.response.changed()
                            && !self.labelled_trees.contains_key(&id)
                        {
                            current_formula_geometry = Some((
                                id,
                                editor.arena().revision(),
                                region.format,
                                matrix_window,
                                f64::from(outcome.layout_size.y),
                            ));
                        }
                        let response = outcome.response;
                        if response.changed() {
                            if let Ok(value) = editor.to_portable() {
                                *tree = value;
                            }
                            changed = true;
                            math_edits.push(id);
                        }
                        response
                    }
                    CanvasContent::Text(text) => {
                        let rich_error = self
                            .rich_text
                            .editors
                            .get(&id)
                            .and_then(|entry| entry.as_ref().err())
                            .or_else(|| {
                                self.rich_text
                                    .authored_editors
                                    .get(&id)
                                    .and_then(|entry| entry.as_ref().err())
                            });
                        if let Some(error) = rich_error {
                            child.label(text.as_str()).on_hover_text(error)
                        } else {
                            let output = plain_text_input::show(
                                &mut child,
                                id,
                                text,
                                &mut self.text_cursors,
                                &mut self.text_composition,
                                self.focus_new == Some(id),
                                plain_text_input::Presentation {
                                    style: visual
                                        .and_then(|v| v.document.as_ref())
                                        .map(|d| &d.style),
                                    rich: self
                                        .rich_text
                                        .editors
                                        .get_mut(&id)
                                        .and_then(|entry| entry.as_mut().ok())
                                        .map(authored_text_input::Editor::Native)
                                        .or_else(|| {
                                            self.rich_text
                                                .authored_editors
                                                .get_mut(&id)
                                                .and_then(|entry| entry.as_mut().ok())
                                                .map(authored_text_input::Editor::Authored)
                                        }),
                                    rich_wire: &mut region.rich_text,
                                    authored_wire: &mut region.authored_text,
                                    headroom: rich_headroom.as_ref(),
                                    suppress_paragraph_break: composing_text_owner == Some(id)
                                        || self.plot_input_queue.rich_ime_confirmation(),
                                },
                            );
                            if let Some(error) = output.error {
                                self.status = format!("텍스트 입력: {error}");
                            }
                            changed |= output.changed;
                            if output.changed {
                                text_edits.push(id);
                            }
                            output.response
                        }
                    }
                    CanvasContent::Plot(spec) => {
                        let pending = Err("그래프 계산 대기 중".into());
                        let old_size = [spec.width, spec.height];
                        let edit = self.inline_plots.entry(id).or_default();
                        edit.ensure_widget_generation(
                            self.plot_widget_generations.get(&id).copied().unwrap_or(0),
                        );
                        edit.ensure_palette_cursor(
                            self.plot_palette_cursors
                                .get(&id)
                                .copied()
                                .unwrap_or(0)
                                .max(spec.trace_count()),
                        );
                        edit.ensure_equation_id_floor(
                            self.plot_equation_floors.get(&id).copied().unwrap_or(0),
                        );
                        let drawn = crate::plots::draw(
                            &mut child,
                            spec,
                            edit,
                            self.plot_results.get(&id).unwrap_or(&pending),
                            self.focus_new == Some(id),
                        );
                        self.plot_palette_cursors.insert(id, edit.palette_cursor());
                        self.plot_widget_generations
                            .insert(id, edit.widget_generation());
                        self.plot_equation_floors
                            .insert(id, edit.equation_id_floor());
                        if drawn.expression_changed {
                            child.ctx().request_repaint();
                        }
                        if let Some(error) = drawn.input_error {
                            self.status = format!("도표 입력 오류: {error}");
                        }
                        plot_commits |= drawn.commit_requested;
                        if drawn.resize_started
                            && let Some(before) = drawn.resize_before
                        {
                            plot_resizes.push((id, before));
                        }
                        if old_size != [spec.width, spec.height] {
                            plot_resize_changes.push(id);
                        }
                        drawn.response
                    }
                    CanvasContent::Opaque { xml, kind } if kind == "solveblock" => {
                        let (method, guesses, constraints, output) = solve_block_summary(xml);
                        child
                            .group(|ui| {
                                ui.set_min_width(360.);
                                ui.horizontal(|ui| {
                                    ui.strong(format!("{method} 풀이 구간"));
                                    ui.label(
                                        egui::RichText::new(format!("등식 {constraints}개"))
                                            .color(Color32::from_gray(100)),
                                    );
                                });
                                if !guesses.is_empty() {
                                    ui.label(format!("초깃값  {}", guesses.join(", ")));
                                }
                                if let Some(output) = output {
                                    let message = row_result
                                        .and_then(|result| result.error.as_ref())
                                        .map(|error| format!("오류: {}", error.detail))
                                        .unwrap_or_else(|| format!("결과  {output}"));
                                    ui.label(message);
                                }
                                if let Some(display) = &display {
                                    formula::readonly_result_ui(
                                        ui,
                                        display,
                                        region.format.font_points,
                                    );
                                }
                            })
                            .response
                    }
                    CanvasContent::Opaque { kind, .. } if kind == "math" && display.is_some() => {
                        formula::readonly_result_ui(
                            &mut child,
                            display.as_ref().unwrap(),
                            region.format.font_points,
                        )
                    }
                    CanvasContent::Opaque { .. } => child.add(
                        egui::Label::new(
                            egui::RichText::new("가져온 영역 · 읽기 전용")
                                .color(Color32::from_gray(130)),
                        )
                        .sense(Sense::click()),
                    ),
                }
            };
            #[cfg(test)]
            drop(content_timing);
            #[cfg(test)]
            let _response_timing =
                crate::frame_profile::scope(crate::frame_profile::Phase::Response);
            if let (Some(slot), Some(visual)) = (background_slot, visual)
                && let Some([r, g, b, a]) = visual.background
            {
                let padding = if native_source {
                    match region.content {
                        CanvasContent::Math(_) => 2.,
                        CanvasContent::Text(_) => 5.,
                        _ => 0.,
                    }
                } else {
                    0.
                };
                let rect = response.rect.union(Rect::from_min_size(
                    origin - vec2(padding, 0.),
                    vec2(visual.size[0], visual.size[1]),
                ));
                child.painter().set(
                    slot,
                    egui::Shape::rect_filled(rect, 0., Color32::from_rgba_unmultiplied(r, g, b, a)),
                );
            }
            if self
                .region_widths
                .insert(id, response.rect.width())
                .is_none_or(|old| (old - response.rect.width()).abs() > 0.5)
            {
                ui.ctx().request_repaint();
            }
            if self
                .region_heights
                .insert(id, response.rect.height())
                .is_none_or(|old| (old - response.rect.height()).abs() > 0.5)
            {
                ui.ctx().request_repaint();
            }
            if response.rect.intersects(visible) && calculatable(region) {
                self.visible_regions.insert(id);
                let position = (region.top, region.left);
                if last_visible_position.is_none_or(|last| position >= last) {
                    last_visible_position = Some(position);
                    self.visible_calculation_end = Some(id);
                }
            }
            if self.focus_new == Some(id) {
                if !matches!(region.content, CanvasContent::Plot(_))
                    && !matches!(&region.content, CanvasContent::Opaque {kind, ..} if matches!(kind.as_str(), "spec-table" | "specTable" | "Area"))
                {
                    response.request_focus();
                }
                self.focus_new = None;
            }
            if response.clicked() || response.has_focus() {
                self.selected = Some(id);
                self.insertion = None;
            }
            if response.has_focus() {
                self.editing_region = Some(id);
            }
            hit_region |= response.hovered();
            if response.secondary_clicked() {
                context_selection = Some(id);
            }
            response.context_menu(|ui| {
                if region.supports_calculation_toggle() {
                    let mut disabled = region.calculation_disabled;
                    if ui.checkbox(&mut disabled, "영역 비활성화").changed() {
                        context_action = Some((id, RegionAction::SetCalculationDisabled(disabled)));
                        ui.close();
                    }
                    ui.separator();
                }
                for (label, action) in [
                    ("편집", RegionAction::Edit),
                    ("영역 복사", RegionAction::Copy),
                    ("영역 잘라내기", RegionAction::Cut),
                    ("복제", RegionAction::Duplicate),
                    ("삭제", RegionAction::Delete),
                ] {
                    if ui
                        .add_enabled(
                            !matches!(region.content, CanvasContent::Opaque { .. })
                                || (is_spec_table(region)
                                    && !matches!(action, RegionAction::Duplicate))
                                || (matches!(&region.content,CanvasContent::Opaque{kind,..}if kind=="Area") && matches!(action,RegionAction::Edit|RegionAction::Delete)),
                            egui::Button::new(label),
                        )
                        .clicked()
                    {
                        context_action = Some((id, action));
                        ui.close();
                    }
                }
            });
            if self.selected == Some(id) || response.hovered() {
                let border = response.rect.expand(1.);
                let stroke = Stroke::new(0.5, Color32::from_rgb(78, 126, 155));
                if response.has_focus() {
                    let points = [
                        border.left_top(),
                        border.right_top(),
                        border.right_bottom(),
                        border.left_bottom(),
                        border.left_top(),
                    ];
                    child
                        .painter()
                        .extend(egui::Shape::dashed_line(&points, stroke, 4., 2.));
                } else {
                    child
                        .painter()
                        .rect_stroke(border, 1., stroke, egui::StrokeKind::Outside);
                }
                let handle = Rect::from_min_size(
                    origin - vec2(14., 0.),
                    vec2(12., response.rect.height().max(20.)),
                );
                let drag = ui
                    .interact(handle, egui::Id::new(("move", id)), Sense::click_and_drag())
                    .on_hover_cursor(egui::CursorIcon::Grab)
                    .on_hover_text("드래그하여 이동 · 클릭 후 방향키로 미세 이동");
                hit_region |= drag.hovered();
                // egui selects its press hit using the latest position. If a
                // fast press+move is coalesced into one frame, that position
                // can already be outside this handle. Use the actual press
                // event to retain the intended target in that case.
                let transform = ui
                    .ctx()
                    .layer_transform_from_global(ui.layer_id())
                    .unwrap_or_default();
                let coalesced_origin = ui.input(|input| {
                    if !input.pointer.primary_down() || input.pointer.could_any_button_be_click() {
                        return None;
                    }
                    input.events.iter().find_map(|event| match event {
                        egui::Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed: true,
                            ..
                        } if handle.intersect(ui.clip_rect()).contains(transform * *pos) => {
                            Some(*pos)
                        }
                        _ => None,
                    })
                });
                if ui.input(|input| input.pointer.any_pressed())
                    && (drag.is_pointer_button_down_on() || coalesced_origin.is_some())
                {
                    self.drag_pointer_origin = ui.input(|input| input.pointer.press_origin());
                    self.pointer_history = Some(HistoryEntry {
                        text_affinity: self.selected.and_then(|id| {
                            self.rich_text.affinity(id).map(|affinity| (id, affinity))
                        }),
                        document: DocumentHistory::Region(Box::new(region.clone())),
                        selected: self.selected,
                        insertion: self.insertion,
                        editing_region: previous_editing,
                        active_editor: self.selected.and_then(|selected| {
                            self.editors
                                .get(&selected)
                                .map(|editor| (selected, editor.clone_without_history()))
                        }),
                        active_area_editor: self.selected.and_then(|selected| {
                            self.area_editors
                                .get(&selected)?
                                .active_snapshot()
                                .map(|(key, editor)| (selected, key, editor))
                        }),
                        text_cursor: self.selected.and_then(|selected| {
                            self.text_cursors
                                .get(&selected)
                                .map(|cursor| (selected, *cursor))
                        }),
                    });
                }
                if drag.clicked() {
                    self.selected = Some(id);
                    response.surrender_focus();
                    self.editing_region = None;
                }
                if drag.drag_started() || coalesced_origin.is_some() {
                    if self.pointer_history.is_none() {
                        self.pointer_history = Some(HistoryEntry {
                            text_affinity: self.selected.and_then(|id| {
                                self.rich_text.affinity(id).map(|affinity| (id, affinity))
                            }),
                            document: DocumentHistory::Region(Box::new(region.clone())),
                            selected: self.selected,
                            insertion: self.insertion,
                            editing_region: previous_editing,
                            active_editor: self.selected.and_then(|selected| {
                                self.editors
                                    .get(&selected)
                                    .map(|editor| (selected, editor.clone_without_history()))
                            }),
                            active_area_editor: self.selected.and_then(|selected| {
                                self.area_editors
                                    .get(&selected)?
                                    .active_snapshot()
                                    .map(|(key, editor)| (selected, key, editor))
                            }),
                            text_cursor: self.selected.and_then(|selected| {
                                self.text_cursors
                                    .get(&selected)
                                    .map(|cursor| (selected, *cursor))
                            }),
                        });
                    }
                    if self.drag_pointer_origin.is_none() {
                        self.drag_pointer_origin = coalesced_origin
                            .or_else(|| ui.input(|input| input.pointer.press_origin()));
                    }
                    self.selected = Some(id);
                    response.surrender_focus();
                    self.editing_region = None;
                    self.dragging = Some((id, region.left, region.top));
                    self.drag_preview = Some((id, region.left, region.top));
                }
                if let Some((moving, start_x, start_y)) = self.dragging
                    && moving == id
                    && let Some(delta) = ui.input(|input| {
                        self.drag_pointer_origin
                            .zip(input.pointer.latest_pos())
                            .map(|(origin, current)| (current - origin) * transform.scaling)
                    })
                {
                    let target = geometry.to_paper(start_x, start_y) + delta;
                    let (x, y) = geometry.to_body_clamped(target);
                    self.drag_preview = Some((id, x, y));
                    ui.ctx().request_repaint();
                }
                if drag.drag_stopped()
                    || (self.dragging.is_some_and(|(moving, _, _)| moving == id)
                        && ui.input(|input| {
                            input.pointer.button_released(egui::PointerButton::Primary)
                        }))
                {
                    let snap = |v: f64| {
                        let r = v % self.document.grid;
                        if r > self.document.grid / 2. {
                            v + self.document.grid - r
                        } else {
                            v - r
                        }
                    };
                    if let Some((_, x, y)) =
                        self.drag_preview.filter(|(moving, _, _)| *moving == id)
                    {
                        let position = (snap(x), snap(y));
                        geometry_changed |= position != (region.left, region.top);
                        (region.left, region.top) = position;
                    }
                    self.dragging = None;
                    self.drag_preview = None;
                    self.drag_pointer_origin = None;
                }
            }
            if let Some(error) = (!result_pending)
                .then_some(row_result)
                .flatten()
                .and_then(|r| r.error.as_ref())
                .map(|e| e.detail.as_str())
                .or_else(|| {
                    (!result_pending)
                        .then_some(saved_result)
                        .flatten()
                        .and_then(|r| r.error.as_deref())
                })
                .or_else(|| visual_error.map(String::as_str))
            {
                let indicator =
                    Rect::from_min_size(response.rect.right_top() + vec2(3., 4.), vec2(18., 18.));
                child
                    .painter()
                    .circle_filled(indicator.center(), 7., RED.gamma_multiply(0.12));
                child.painter().text(
                    indicator.center(),
                    egui::Align2::CENTER_CENTER,
                    "!",
                    FontId::proportional(13.),
                    RED,
                );
                child
                    .interact(
                        response.rect.union(indicator),
                        egui::Id::new(("error", id)),
                        Sense::hover(),
                    )
                    .on_hover_text(error);
            } else if self.show_warnings
                && !result_pending
                && (row_result.is_some_and(|result| result.redefinition_warning)
                    || saved_result.is_some_and(|result| !result.warnings.is_empty()))
            {
                let indicator =
                    Rect::from_min_size(response.rect.right_top() + vec2(3., 4.), vec2(18., 18.));
                child
                    .painter()
                    .circle_filled(indicator.center(), 7., AMBER.gamma_multiply(0.14));
                child.painter().text(
                    indicator.center(),
                    egui::Align2::CENTER_CENTER,
                    "!",
                    FontId::proportional(13.),
                    AMBER,
                );
                child
                    .interact(indicator, egui::Id::new(("warning", id)), Sense::hover())
                    .on_hover_text("이 이름은 워크시트의 앞선 정의를 다시 정의합니다.");
            }
        }
        #[cfg(test)]
        let commit_timing = crate::frame_profile::scope(crate::frame_profile::Phase::Commit);
        for (id, gesture) in matrix_gestures {
            self.matrix_view_gesture(id, gesture);
        }
        let history = if geometry_changed {
            self.pointer_history.take().or(before)
        } else {
            before
        };
        if (changed || geometry_changed)
            && let Some(before) = history
        {
            self.remember(before);
        }
        if !ui.input(|input| input.pointer.any_down()) {
            self.pointer_history = None;
        }
        if changed || geometry_changed {
            self.status.clear();
            self.dirty = true;
        }
        for id in text_edits {
            self.refresh_rich_text_visual(id);
        }
        for (id, size) in plot_resizes {
            self.plot_resize_start = Some((id, size));
        }
        for id in plot_resize_changes {
            if let Some((owner, before)) = self.plot_resize_start.take_if(|(owner, _)| *owner == id)
            {
                self.begin_plot_resize(owner, *before);
                geometry_changed = true;
            }
        }
        if !ui.input(|input| input.pointer.any_down()) {
            self.plot_resize_start = None;
        }
        if plot_commits {
            self.finish_plot_edits(None);
        }
        for id in math_edits {
            self.stage_math_edit(id, false);
        }
        for id in &area_drafts {
            self.stage_math_edit(*id, false);
        }
        if !area_commits.is_empty() && !evaluation_requested {
            self.commit_math_edits();
        }
        // A pointer may commit one child and start a sibling draft in one frame.
        for id in area_commits {
            if self
                .area_editors
                .get(&id)
                .is_some_and(|area| area.has_draft_changes())
            {
                self.stage_math_edit(id, false);
            }
        }
        if !table_commits.is_empty() {
            for id in table_commits {
                self.refresh_table_visual(id);
            }
            self.commit_math_edits();
        }
        if geometry_changed || evaluation_requested {
            self.calculation_changed(evaluation_requested);
        }
        if changed || geometry_changed || evaluation_requested {
            ui.ctx().request_repaint();
        }
        if let Some(id) = context_selection {
            self.selected = Some(id);
            self.editing_region = None;
            self.insertion = None;
            ui.memory_mut(|m| {
                if let Some(focus) = m.focused() {
                    m.surrender_focus(focus);
                }
            });
        }
        if let Some((id, action)) = context_action {
            self.selected = Some(id);
            match action {
                RegionAction::Edit => self.focus_new = Some(id),
                RegionAction::Copy => self.copy_region(ui.ctx(), false),
                RegionAction::Cut => self.copy_region(ui.ctx(), true),
                RegionAction::Duplicate => self.duplicate_selected(),
                RegionAction::Delete => self.remove_selected(),
                RegionAction::SetCalculationDisabled(disabled) => {
                    self.set_selected_calculation_disabled(disabled)
                }
            }
        }
        if let Some((x, y)) = self.insertion {
            let p = paper.min + geometry.to_paper(x, y).to_vec2();
            ui.painter().line_segment(
                [p - vec2(5.5, 0.), p + vec2(5.5, 0.)],
                Stroke::new(1., BLUE),
            );
            ui.painter().line_segment(
                [p - vec2(0., 5.5), p + vec2(0., 5.5)],
                Stroke::new(1., BLUE),
            );
        }
        if background.clicked()
            && !hit_region
            && let Some(pointer) = background.interact_pointer_pos()
        {
            let point = (pointer - paper.min).to_pos2();
            let body_position =
                if self.expand_right && point.x >= geometry.margins[0] && point.x < size.x {
                    let mut within_page = point;
                    within_page.x = geometry.margins[0];
                    geometry
                        .to_body(within_page)
                        .map(|(_, y)| (f64::from(point.x - geometry.margins[0]), y))
                } else {
                    geometry.to_body(point)
                };
            let Some((body_x, body_y)) = body_position else {
                return;
            };
            let snap = |v: f64| {
                let r = v % self.document.grid;
                if r > self.document.grid / 2. {
                    v + self.document.grid - r
                } else {
                    v - r
                }
            };
            self.insertion = Some((snap(body_x), snap(body_y)));
            self.selected = None;
            self.editing_region = None;
            ui.memory_mut(|m| {
                if let Some(id) = m.focused() {
                    m.surrender_focus(id);
                }
            });
            background.request_focus();
            self.commit_math_edits();
        }
        #[cfg(test)]
        drop(commit_timing);
        #[cfg(test)]
        let _keyboard_timing = crate::frame_profile::scope(crate::frame_profile::Phase::Keyboard);
        self.region_keyboard(ui, previous_editing);
        self.finish_matrix_view_frame(matrix_painted);
        match self.commit_table_edits(self.editing_region) {
            Ok(true) => self.commit_math_edits(),
            Err(error) => self.status = error,
            Ok(false) => (),
        }
        match self.commit_area_edits(self.editing_region) {
            Ok(true) => self.commit_math_edits(),
            Err(error) => self.status = error,
            Ok(false) => (),
        }
        if self.editing_region.is_none()
            && (self.selected.is_some() || self.insertion.is_some())
            && !ui.ctx().any_popup_open()
        {
            if !background.has_focus() {
                background.request_focus();
            }
            ui.memory_mut(|m| {
                m.set_focus_lock_filter(
                    background.id,
                    egui::EventFilter {
                        tab: true,
                        horizontal_arrows: true,
                        vertical_arrows: true,
                        ..Default::default()
                    },
                )
            });
        }
        // Enter commits the current formula and advances to a new insertion point.
        if let Some(id) = self.selected
            && self.editing_region == Some(id)
            && let Some(region) = self.document.regions.iter().find(|r| r.id == id)
            && matches!(region.content, CanvasContent::Math(_))
            && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter))
        {
            // Use exactly the source-bound result view drawn by the formula.
            // The legacy 12-row preview cannot measure an expanded matrix.
            let matrix_window = self
                .matrix_view_sources
                .window(id, self.requested_matrix_view(id));
            let matrix_indices_metadata = self.result_matrix_indices(id);
            let display = matrix_result_view::cached_formula_display(
                &mut self.display_cache,
                id,
                region.format,
                self.results.get(&id),
                self.saved_results.get(&id),
                matrix_window,
            );
            let display =
                match matrix_result_view::indexed_formula_display(display, matrix_indices_metadata)
                {
                    Ok(shown) => shown,
                    Err(error) => {
                        self.status = error;
                        display
                    }
                };
            let reused_height = current_formula_geometry
                .filter(|(rendered, revision, format, rendered_window, _)| {
                    *rendered == id
                        && *format == region.format
                        && *rendered_window == matrix_window
                        && self
                            .editors
                            .get(&id)
                            .is_some_and(|editor| editor.arena().revision() == *revision)
                })
                .map(|(_, _, _, _, height)| height);
            let measure_height = || {
                self.editors
                    .get(&id)
                    .map(|editor| {
                        formula::measured_size_styled(
                            ui,
                            editor,
                            display,
                            region.format.font_points,
                        )
                        .y as f64
                    })
                    .unwrap_or(self.document.grid * 3.)
            };
            #[cfg(test)]
            if crate::frame_profile::height_oracle_enabled()
                && let Some(height) = reused_height
            {
                crate::frame_profile::assert_height(height, measure_height(), self.document.grid);
            }
            let height = reused_height.unwrap_or_else(measure_height);
            let advance =
                ((height + self.document.grid) / self.document.grid).ceil() * self.document.grid;
            self.insertion = Some((
                region.left,
                region.top + advance.max(self.document.grid * 3.),
            ));
            self.reveal = self.insertion;
            self.selected = None;
            self.editing_region = None;
            ui.memory_mut(|m| {
                if let Some(focused) = m.focused() {
                    m.surrender_focus(focused)
                }
            });
            background.request_focus();
        }
        if let Some(position) = self.insertion {
            // Formula/program widgets consume their own Enter before reaching
            // this worksheet-level commit boundary.
            self.commit_math_edits();
            let ime_id = ui.id().with("insertion-preedit");
            let mut preedit = ui
                .data(|data| data.get_temp::<String>(ime_id))
                .unwrap_or_default();
            for event in ui.input(|i| i.events.clone()) {
                match event {
                    egui::Event::Ime(egui::ImeEvent::Preedit { text, .. }) => preedit = text,
                    egui::Event::Ime(egui::ImeEvent::Commit(_)) => preedit.clear(),
                    _ => (),
                }
            }
            let cursor =
                paper.min + geometry.to_paper(position.0, position.1).to_vec2() + vec2(2., 2.);
            if !preedit.is_empty() {
                ui.painter().text(
                    cursor,
                    egui::Align2::LEFT_TOP,
                    &preedit,
                    typography::MathStyle::Variable.font(typography::MATH_SIZE),
                    Color32::from_gray(50),
                );
            }
            ui.data_mut(|data| data.insert_temp(ime_id, preedit));
            if background.has_focus() {
                let to_global = ui
                    .ctx()
                    .layer_transform_to_global(ui.layer_id())
                    .unwrap_or_default();
                ui.output_mut(|o| {
                    o.ime = Some(egui::output::IMEOutput {
                        purpose: egui::IMEPurpose::Normal,
                        rect: to_global * Rect::from_min_size(cursor, vec2(120., 30.)),
                        cursor_rect: to_global * Rect::from_min_size(cursor, vec2(1., 30.)),
                        should_interrupt_composition: false,
                    })
                });
            }
            let text: String = ui.input(|i| {
                i.events
                    .iter()
                    .filter_map(|e| match e {
                        egui::Event::Text(s) | egui::Event::Paste(s) => Some(s.as_str()),
                        egui::Event::Ime(egui::ImeEvent::Commit(s)) => Some(s.as_str()),
                        _ => None,
                    })
                    .collect()
            });
            if !text.is_empty() {
                ui.data_mut(|data| data.remove::<String>(ime_id));
                let is_text = self.text_mode;
                if is_text {
                    if let Err(error) = self.insert_authored_text(position, &text) {
                        self.status = format!("텍스트 삽입: {error}");
                    }
                    ui.ctx().request_repaint();
                    return;
                }
                let id = self.insert(position, false);
                let mut editor = Editor::new();
                editor.set_bin_oct_hex_mode(self.numeric_input_mode().unwrap_or(false));
                let outcome = formula::input_text_with_actions(&mut editor, &text);
                if let Ok(tree) = editor.to_portable()
                    && let Some(region) = self.document.regions.iter_mut().find(|r| r.id == id)
                {
                    region.content = CanvasContent::Math(tree);
                }
                self.editors.insert(id, editor);
                self.stage_math_edit(id, outcome.evaluation_requested);
                ui.ctx().request_repaint();
            }
        }
        self.apply_page_break_request(ui);
        if self.menu_resume.is_none() && self.plot_draft.is_none() && !ui.ctx().any_popup_open() {
            self.finish_plot_edits(self.editing_region.or(self.focus_new));
        }
        if previous_editing.is_some()
            && previous_editing != self.editing_region
            && !previous_editing.is_some_and(|id| {
                self.document.regions.iter().any(|region| {
                    region.id == id && matches!(region.content, CanvasContent::Plot(_))
                })
            })
            && self.focus_new != previous_editing
            && self.menu_resume != previous_editing
            && !ui.ctx().any_popup_open()
        {
            self.commit_math_edits();
        }
    }
}

fn page_field_name(field: PageField) -> &'static str {
    match field {
        PageField::PageNumber(_) => "페이지 번호",
        PageField::DateSaved(_) => "저장 날짜",
        PageField::FileName => "파일 이름",
        PageField::FilePath => "파일 경로",
    }
}

fn push_page_band_region(band: &mut PageBand, next_id: &mut u64, content: PageBandContent) {
    if *next_id == u64::MAX {
        return;
    }
    band.regions.push(PageBandRegion {
        id: *next_id,
        left: 0.,
        top: 0.,
        content,
    });
    *next_id += 1;
}

fn page_band_ui(ui: &mut egui::Ui, title: &str, band: &mut PageBand, next_id: &mut u64) {
    ui.heading(title);
    let mut remove = None;
    for (index, region) in band.regions.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            ui.label(format!("{}.", index + 1));
            match &mut region.content {
                PageBandContent::Text(text) => {
                    ui.add(
                        egui::TextEdit::singleline(text)
                            .desired_width(280.)
                            .char_limit(16_384),
                    );
                }
                PageBandContent::Field(field) => {
                    egui::ComboBox::from_id_salt((title, region.id, "field"))
                        .selected_text(page_field_name(*field))
                        .show_ui(ui, |ui| {
                            for (name, value) in [
                                (
                                    "페이지 번호 / 전체",
                                    PageField::PageNumber(PageNumberFormat::PageNumberOfTotal),
                                ),
                                (
                                    "페이지 번호",
                                    PageField::PageNumber(PageNumberFormat::Number),
                                ),
                                (
                                    "저장 날짜",
                                    PageField::DateSaved(DateSavedFormat::SystemLong),
                                ),
                                ("파일 이름", PageField::FileName),
                                ("파일 경로", PageField::FilePath),
                            ] {
                                ui.selectable_value(field, value, name);
                            }
                        });
                }
            }
            ui.label("x");
            ui.add(
                egui::DragValue::new(&mut region.left)
                    .speed(1.)
                    .range(0. ..=5000.),
            );
            ui.label("y");
            ui.add(
                egui::DragValue::new(&mut region.top)
                    .speed(1.)
                    .range(0. ..=5000.),
            );
            if ui.small_button("삭제").clicked() {
                remove = Some(index);
            }
        });
    }
    if let Some(index) = remove {
        band.regions.remove(index);
    }
    ui.horizontal(|ui| {
        if ui.button("텍스트 추가").clicked() {
            push_page_band_region(band, next_id, PageBandContent::Text(String::new()));
        }
        ui.menu_button("필드 추가", |ui| {
            for (name, field) in [
                (
                    "페이지 번호 / 전체",
                    PageField::PageNumber(PageNumberFormat::PageNumberOfTotal),
                ),
                (
                    "페이지 번호",
                    PageField::PageNumber(PageNumberFormat::Number),
                ),
                (
                    "저장 날짜",
                    PageField::DateSaved(DateSavedFormat::SystemLong),
                ),
                ("파일 이름", PageField::FileName),
                ("파일 경로", PageField::FilePath),
            ] {
                if ui.button(name).clicked() {
                    push_page_band_region(band, next_id, PageBandContent::Field(field));
                    ui.close();
                }
            }
        });
    });
}

fn fault_statement(tree: &PortableTree) -> String {
    const ML: &str = "http://schemas.mathsoft.com/math50";
    if matches!(
        tree.kind,
        NodeKind::Binary(BinaryOp::Define | BinaryOp::GlobalDefine)
    ) && let Some(mut lhs) = tree.children.first()
    {
        while matches!(
            lhs.kind,
            NodeKind::Function { .. } | NodeKind::Index { .. } | NodeKind::Row | NodeKind::Column
        ) {
            let Some(name) = lhs.children.first() else {
                break;
            };
            lhs = name;
        }
        if matches!(
            lhs.kind,
            NodeKind::Identifier(_)
                | NodeKind::AnnotatedIdentifier { .. }
                | NodeKind::NativeIdentifier { .. }
        ) && let Ok(name) = worksheet_math::tree_to_mathml(lhs)
        {
            let tag = if tree.kind == NodeKind::Binary(BinaryOp::GlobalDefine) {
                "globalDefine"
            } else {
                "define"
            };
            return format!("<ml:{tag} xmlns:ml=\"{ML}\">{name}<ml:unsupported/></ml:{tag}>");
        }
    }
    format!(
        "<ml:eval xmlns:ml=\"{ML}\"><ml:unsupported/><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>"
    )
}

#[cfg(test)]
fn value_text(value: &Value) -> String {
    match value {
        Value::String { value } => format!("“{value}”"),
        Value::Symbolic { mathml } => mathml.clone(),
        Value::Scalar { quantity } => quantity.to_string(),
        Value::Matrix {
            rows,
            cols,
            elements,
        } => format!(
            "[{} × {}] {}",
            rows,
            cols,
            elements
                .iter()
                .map(|q| mathcraft_core::format_complex(q.value))
                .collect::<Vec<_>>()
                .join("; ")
        ),
        Value::Range {
            start, step, end, ..
        } => format!(
            "{}, {} .. {}",
            start,
            mathcraft_core::Quantity {
                value: start.value + step.value,
                ..*start
            },
            end
        ),
        Value::RangeValues { elements } => elements
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("; "),
    }
}

fn standalone(node: roxmltree::Node<'_, '_>, source: &str) -> String {
    let raw = &source[node.range()];
    let end = raw.find('>').unwrap_or(raw.len());
    let at = if raw[..end].ends_with('/') {
        end - 1
    } else {
        end
    };
    let mut declarations = String::new();
    for namespace in node.namespaces() {
        let name = namespace
            .name()
            .map_or("xmlns".to_owned(), |p| format!("xmlns:{p}"));
        if !raw[..end].contains(&format!("{name}=")) {
            declarations.push_str(&format!(" {name}=\"{}\"", namespace.uri()));
        }
    }
    format!("{}{}{}", &raw[..at], declarations, &raw[at..])
}
fn raw_fault_statement(source: &str) -> String {
    const ML: &str = "http://schemas.mathsoft.com/math50";
    if let Ok(document) = roxmltree::Document::parse(source) {
        let root = document.root_element();
        if matches!(root.tag_name().name(), "define" | "globalDefine")
            && let Some(mut lhs) = root.children().find(roxmltree::Node::is_element)
        {
            if lhs.has_tag_name((ML, "apply")) {
                let mut operands = lhs.children().filter(roxmltree::Node::is_element);
                if operands.next().is_some_and(|node| {
                    matches!(node.tag_name().namespace(), Some(ML))
                        && matches!(node.tag_name().name(), "indexer" | "matrow" | "matcol")
                }) && let Some(base) =
                    operands.next().filter(|node| node.has_tag_name((ML, "id")))
                {
                    lhs = base;
                }
            }
            if matches!(lhs.tag_name().name(), "id" | "function") {
                let name = standalone(lhs, source);
                let tag = root.tag_name().name();
                return format!("<ml:{tag} xmlns:ml=\"{ML}\">{name}<ml:unsupported/></ml:{tag}>");
            }
        }
    }
    format!(
        "<ml:eval xmlns:ml=\"{ML}\"><ml:unsupported/><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>"
    )
}

fn native_region_payload(source: &str, kind: &str) -> Result<String, String> {
    let wrapped = format!(
        "<context xmlns=\"http://schemas.mathsoft.com/worksheet50\" xmlns:ml=\"http://schemas.mathsoft.com/math50\">{source}</context>"
    );
    let document = roxmltree::Document::parse(&wrapped).map_err(|error| error.to_string())?;
    let root = document
        .root_element()
        .children()
        .find(roxmltree::Node::is_element)
        .ok_or("Empty native region")?;
    let payload = if root.tag_name().name() == kind {
        root
    } else {
        root.children()
            .find(|node| node.is_element() && node.tag_name().name() == kind)
            .ok_or("Missing native region payload")?
    };
    if kind == "math" {
        let expression = payload
            .children()
            .find(|node| {
                node.is_element()
                    && node.tag_name().namespace() == Some("http://schemas.mathsoft.com/math50")
            })
            .ok_or("Missing native expression")?;
        Ok(standalone(expression, &wrapped))
    } else {
        Ok(standalone(payload, &wrapped))
    }
}

fn import_native(path: &Path) -> Result<(CanvasDocument, NativePackage), String> {
    let size = std::fs::metadata(path).map_err(|e| e.to_string())?.len();
    if size > 16 * 1024 * 1024 {
        return Err("This worksheet exceeds the current 16 MiB import limit".into());
    }
    let source_bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    if source_bytes.len() > 16 * 1024 * 1024 {
        return Err("This worksheet exceeds the current 16 MiB import limit".into());
    }
    #[cfg(test)]
    let archive_timing = crate::frame_profile::scope(crate::frame_profile::Phase::ImportArchive);
    let package = NativePackage::from_bytes(&source_bytes)?;
    #[cfg(test)]
    drop(archive_timing);
    #[cfg(test)]
    let defaults_timing = crate::frame_profile::scope(crate::frame_profile::Phase::ImportDefaults);
    let default_format =
        mathcraft_document::formatting::read_worksheet_math_format_defaults(&package)?;
    #[cfg(test)]
    drop(defaults_timing);
    #[cfg(test)]
    let worksheet_timing =
        crate::frame_profile::scope(crate::frame_profile::Phase::ImportWorksheet);
    let source = package.worksheet_xml()?;
    let xml = roxmltree::Document::parse(&source).map_err(|e| e.to_string())?;
    let mut document = CanvasDocument {
        title: path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        native_archive: Some(source_bytes),
        ..Default::default()
    };
    if let Some(presentation) = package
        .original_xml_part("mathcad/settings/presentation.xml")
        .ok()
        .flatten()
        && let Ok(grid) = mathcraft_compat::PlacementGrid::from_presentation(
            presentation.trim_start_matches('\u{feff}'),
        )
    {
        document.grid = grid.spacing();
        if let Some((size, _)) = crate::page::source_settings(&presentation) {
            document.width = f64::from(size.x);
            document.height = f64::from(size.y);
        }
    }
    #[cfg(test)]
    drop(worksheet_timing);
    #[cfg(test)]
    let notes_timing = crate::frame_profile::scope(crate::frame_profile::Phase::ImportNotes);
    let notes: HashMap<_, _> = mathcraft_document::import_mcdx(path)
        .ok()
        .into_iter()
        .flat_map(|d| d.rows)
        .filter(|r| r.note)
        .filter_map(|r| r.imported.map(|i| (i.region_id, r.input)))
        .collect();
    #[cfg(test)]
    drop(notes_timing);
    #[cfg(test)]
    let _regions_timing = crate::frame_profile::scope(crate::frame_profile::Phase::ImportRegions);
    let regions = xml
        .root_element()
        .children()
        .find(|n| n.has_tag_name(("http://schemas.mathsoft.com/worksheet50", "regions")))
        .ok_or("Missing worksheet regions")?;
    for region in regions.children().filter(roxmltree::Node::is_element) {
        let source_id = region.attribute("region-id").unwrap_or("");
        let left: f64 = region
            .attribute("left")
            .unwrap_or("0")
            .parse()
            .map_err(|_| "Invalid region left")?;
        let top: f64 = region
            .attribute("top")
            .unwrap_or("0")
            .parse()
            .map_err(|_| "Invalid region top")?;
        let child = region.children().find(roxmltree::Node::is_element);
        let content = if let Some(child) = child {
            if child.has_tag_name(("http://schemas.mathsoft.com/worksheet50", "math")) {
                let tree = worksheet_math::tree_from_mathml(&standalone(child, &source));
                match tree {
                    Ok(tree) => CanvasContent::Math(tree),
                    Err(_) => CanvasContent::Opaque {
                        xml: standalone(region, &source),
                        kind: "math".into(),
                    },
                }
            } else if child.has_tag_name(("http://schemas.mathsoft.com/worksheet50", "plot")) {
                let raw = standalone(region, &source);
                mathcraft_document::native_plot::parse(&raw)
                    .map(CanvasContent::Plot)
                    .unwrap_or(CanvasContent::Opaque {
                        xml: raw,
                        kind: "plot".into(),
                    })
            } else if let Some(note) = notes.get(source_id) {
                CanvasContent::Text(note.clone())
            } else {
                CanvasContent::Opaque {
                    xml: standalone(region, &source),
                    kind: child.tag_name().name().into(),
                }
            }
        } else {
            CanvasContent::Opaque {
                xml: standalone(region, &source),
                kind: "empty region".into(),
            }
        };
        let id = document.next_id;
        document.next_id += 1;
        document.regions.push(CanvasRegion {
            id,
            left,
            top,
            content,
            source_region: Some(source_id.into()),
            rich_text: None,
            authored_text: None,
            format: child
                .filter(|n| n.has_tag_name(("http://schemas.mathsoft.com/worksheet50", "math")))
                .map(|math| {
                    mathcraft_document::formatting::read_math_format_with_defaults(
                        math,
                        default_format,
                    )
                })
                .unwrap_or(default_format),
            calculation_disabled: child.is_some_and(|node| {
                matches!(
                    node.tag_name().name(),
                    "math"
                        | "plot"
                        | "include"
                        | "mathBoard"
                        | "specTable"
                        | "spec-table"
                        | "solveblock"
                ) && matches!(node.attribute("disable-calc"), Some("true" | "1"))
            }),
            calculation_enabled_override: false,
        });
    }
    document.validate()?;
    Ok((document, package))
}

impl WorksheetApp {
    fn prepare_plot_input(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput) {
        self.release_retired_plot_widgets(ctx);
        let active = self
            .editing_region
            .or(self.focus_new)
            .or(self.selected)
            .is_some_and(|id| {
                self.document
                    .regions
                    .iter()
                    .any(|r| r.id == id && matches!(r.content, CanvasContent::Plot(_)))
            });
        let active_plain_text = self
            .editing_region
            .or(self.focus_new)
            .or(self.selected)
            .is_some_and(|id| {
                self.document.regions.iter().any(|region| {
                    region.id == id && matches!(region.content, CanvasContent::Text(_))
                })
            });
        if self.plot_input_queue.prepare_editor_input(
            raw,
            active,
            active_plain_text,
            active_plain_text
                && (self.document.native_archive.is_some()
                    || self.document.regions.iter().any(|r| {
                        Some(r.id) == self.editing_region.or(self.focus_new).or(self.selected)
                            && r.authored_text.is_some()
                    })),
        ) {
            ctx.request_repaint();
        }
    }
}

impl eframe::App for WorksheetApp {
    fn raw_input_hook(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput) {
        self.prepare_plot_input(ctx, raw);
    }
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.app_ui_frame(ui);
    }
}

impl WorksheetApp {
    fn app_ui_frame(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        self.retire_authored_select_all_before_commands(&ctx);
        self.poll_background_calculation();
        let inline_dirty = self
            .inline_plots
            .values()
            .any(crate::plots::InlinePlotEdit::has_changes);
        let title = format!(
            "{}{} — Mathcraft",
            self.path
                .as_ref()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| self.document.title.clone()),
            if self.dirty || inline_dirty { " *" } else { "" }
        );
        if title != self.last_title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.last_title = title;
        }
        if ctx.input(|i| i.viewport().close_requested())
            && (self.dirty || inline_dirty)
            && !self.closing
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.pending = Some(Pending::Close);
        }
        if self.closing {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if self.pending.is_none() {
            if ctx.input_mut(|i| {
                consume_exact_key(
                    i,
                    egui::Modifiers {
                        shift: true,
                        ..egui::Modifiers::COMMAND
                    },
                    egui::Key::S,
                )
            }) {
                self.save(true);
            } else if ctx
                .input_mut(|i| consume_exact_key(i, egui::Modifiers::COMMAND, egui::Key::S))
            {
                self.save(false);
            }
            if ctx.input_mut(|i| consume_exact_key(i, egui::Modifiers::COMMAND, egui::Key::N)) {
                self.request(Pending::New);
            }
            if ctx.input_mut(|i| consume_exact_key(i, egui::Modifiers::COMMAND, egui::Key::O))
                && let Some(path) = rfd::FileDialog::new()
                    .add_filter("Worksheets", &["mathcraft", "mcdx"])
                    .pick_file()
            {
                self.request(Pending::Open(path));
            }
            self.global_editing_shortcuts(&ctx);
            // Ctrl+Shift+Z belongs to the original infinity gesture, not redo.
            self.document_history_shortcuts(&ctx);
            self.region_clipboard_shortcuts(&ctx);
        }
        if self.pending.is_some() {
            ui.disable();
        }
        self.chrome_ui(ui);
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(Color32::from_rgb(192, 192, 192)))
            .show(ui, |ui| self.workspace_ui(ui));
        self.shortcuts_ui(&ctx);
        self.page_sections_ui(&ctx);
        self.printing_ui(&ctx);
        self.plot_settings_ui(&ctx);
        self.solve_block_ui(&ctx);
        if self.pending.is_some() {
            let modal = egui::Modal::new(egui::Id::new("unsaved-worksheet")).show(&ctx, |ui| {
                ui.heading("변경 사항을 저장할까요?");
                ui.label("저장하지 않은 변경 사항이 있습니다.");
                ui.horizontal(|ui| {
                    if ui.button("저장").clicked()
                        && self.save(false)
                        && let Some(action) = self.pending.take()
                    {
                        self.apply(action);
                    }
                    if ui.button("저장하지 않음").clicked()
                        && let Some(action) = self.pending.take()
                    {
                        self.apply(action);
                    }
                    if ui.button("취소").clicked() {
                        self.pending = None;
                    }
                });
            });
            if modal.should_close() {
                self.pending = None;
            }
        }
        self.frames += 1;
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
                    let _ = image.save(path);
                }
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            if self.frames >= 3 && !self.capture_sent {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
                self.capture_sent = true;
            }
            ctx.request_repaint();
        }
        if self.smoke {
            if self.frames >= 3 {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            } else {
                ctx.request_repaint();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    pub(super) fn assert_pending_curves_unchanged(
        actual: &Result<crate::plots::PlotData, String>,
        prior: &Result<crate::plots::PlotData, String>,
    ) {
        let actual = actual.as_ref().unwrap();
        let prior = prior.as_ref().unwrap();
        assert_eq!(actual.traces.len(), prior.traces.len());
        for (current, old) in actual.traces.iter().zip(&prior.traces) {
            assert!(matches!(
                current,
                crate::plots::TraceOutcome::Pending { previous: Some(_) }
            ));
            assert_eq!(current.curve(), old.curve());
        }
    }

    #[path = "native_open_tests.rs"]
    mod native_open_tests;
    use super::*;
    fn tree(kind: NodeKind, children: Vec<PortableTree>) -> PortableTree {
        PortableTree { kind, children }
    }
    fn number(value: &str) -> PortableTree {
        tree(NodeKind::Number(value.into()), vec![])
    }
    fn identifier(name: &str) -> PortableTree {
        tree(NodeKind::Identifier(name.into()), vec![])
    }
    fn annotated(name: &str, labels: &str) -> PortableTree {
        tree(
            NodeKind::AnnotatedIdentifier {
                name: name.into(),
                labels: labels.into(),
            },
            vec![],
        )
    }
    fn binary_tree(op: BinaryOp, left: PortableTree, right: PortableTree) -> PortableTree {
        tree(NodeKind::Binary(op), vec![left, right])
    }
    fn add_math_tree(
        document: &mut CanvasDocument,
        left: f64,
        top: f64,
        value: PortableTree,
    ) -> u64 {
        let id = document.add_math(left, top);
        document
            .regions
            .iter_mut()
            .find(|region| region.id == id)
            .unwrap()
            .content = CanvasContent::Math(value);
        id
    }

    fn technical_report_document() -> CanvasDocument {
        let mut document = CanvasDocument {
            title: "Mathcraft technical calculation report".into(),
            ..Default::default()
        };
        document.page_sections.explicit = true;
        document.add_header_text(0., 0., "Mathcraft · Technical calculation report");
        document.add_footer_field(
            0.,
            0.,
            PageField::PageNumber(PageNumberFormat::PageNumberOfTotal),
        );
        document.add_footer_field(420., 0., PageField::FileName);
        document.add_text(
            0.,
            20.,
            "구조 수식, 단위 선형계, 프로그램, 미적분, 기호식, 다중 곡선과 풀이 구간",
        );

        add_math_tree(
            &mut document,
            0.,
            80.,
            binary_tree(
                BinaryOp::Define,
                identifier("A"),
                tree(
                    NodeKind::Matrix {
                        rows: 2,
                        cols: 2,
                        display: None,
                    },
                    vec![number("3"), number("1"), number("1"), number("2")],
                ),
            ),
        );
        let force = |value| binary_tree(BinaryOp::Multiply, number(value), annotated("N", "UNIT"));
        add_math_tree(
            &mut document,
            0.,
            140.,
            binary_tree(
                BinaryOp::Define,
                identifier("b"),
                tree(
                    NodeKind::Matrix {
                        rows: 2,
                        cols: 1,
                        display: None,
                    },
                    vec![force("9"), force("8")],
                ),
            ),
        );
        add_math_tree(
            &mut document,
            320.,
            180.,
            binary_tree(
                BinaryOp::Evaluate,
                tree(
                    NodeKind::Call { argument_count: 2 },
                    vec![
                        annotated("lsolve", "FUNCTION"),
                        identifier("A"),
                        identifier("b"),
                    ],
                ),
                tree(NodeKind::Placeholder, vec![]),
            ),
        );
        add_math_tree(
            &mut document,
            0.,
            230.,
            binary_tree(
                BinaryOp::Define,
                identifier("program_value"),
                tree(
                    NodeKind::Program { statement_count: 2 },
                    vec![
                        tree(
                            NodeKind::LocalDefine,
                            vec![annotated("s", "VARIABLE"), number("2")],
                        ),
                        tree(
                            NodeKind::Return,
                            vec![binary_tree(
                                BinaryOp::Add,
                                annotated("s", "VARIABLE"),
                                number("3"),
                            )],
                        ),
                    ],
                ),
            ),
        );
        add_math_tree(
            &mut document,
            320.,
            230.,
            binary_tree(
                BinaryOp::Evaluate,
                identifier("program_value"),
                tree(NodeKind::Placeholder, vec![]),
            ),
        );
        add_math_tree(
            &mut document,
            0.,
            310.,
            binary_tree(
                BinaryOp::Evaluate,
                tree(
                    NodeKind::Integral,
                    vec![
                        tree(
                            NodeKind::Power,
                            vec![annotated("u", "VARIABLE"), number("2")],
                        ),
                        annotated("u", "VARIABLE"),
                        number("0"),
                        number("3"),
                    ],
                ),
                tree(NodeKind::Placeholder, vec![]),
            ),
        );
        add_math_tree(
            &mut document,
            320.,
            310.,
            tree(
                NodeKind::SymbolicEvaluation { command_count: 1 },
                vec![
                    binary_tree(BinaryOp::Add, annotated("z", "VARIABLE"), number("0")),
                    tree(
                        NodeKind::SymbolicCommand { argument_count: 0 },
                        vec![identifier("simplify")],
                    ),
                ],
            ),
        );
        add_math_tree(
            &mut document,
            0.,
            390.,
            binary_tree(
                BinaryOp::Define,
                identifier("x"),
                tree(
                    NodeKind::Range { has_step: true },
                    vec![number("-3"), number("-2.5"), number("3")],
                ),
            ),
        );
        let plot_id = document.add_plot(0., 450.);
        let CanvasContent::Plot(plot) = &mut document
            .regions
            .iter_mut()
            .find(|region| region.id == plot_id)
            .unwrap()
            .content
        else {
            unreachable!()
        };
        plot.promote_legacy_trace();
        plot.legacy_traces_mut().unwrap()[0].label = "sin".into();
        plot.legacy_traces_mut().unwrap()[0].style.color = [190, 35, 45, 255];
        let mut cosine = PlotTraceSpec {
            y_expression: "cos(x)".into(),
            label: "cos".into(),
            ..Default::default()
        };
        cosine.style.color = [35, 90, 180, 255];
        cosine.style.marker = PlotMarker::Circle;
        cosine.style.marker_frequency = 2;
        plot.add_trace(cosine).unwrap();
        plot.x_axis.label = "x".into();
        plot.x_axis.range = Some(PlotRange { min: -3., max: 3. });
        plot.y_axis.label = "f(x)".into();
        plot.y_axis.range = Some(PlotRange { min: -1., max: 1. });

        let mut solve = SolveBlockBuilder::new(SolveMethod::Find);
        solve.guess("p", number("0")).unwrap();
        solve.guess("q", number("0")).unwrap();
        solve
            .equality(
                binary_tree(
                    BinaryOp::Add,
                    annotated("p", "VARIABLE"),
                    annotated("q", "VARIABLE"),
                ),
                number("5"),
            )
            .unwrap();
        solve
            .equality(
                binary_tree(
                    BinaryOp::Subtract,
                    annotated("p", "VARIABLE"),
                    annotated("q", "VARIABLE"),
                ),
                number("1"),
            )
            .unwrap();
        solve.solve_for(["p", "q"]).unwrap();
        solve.export_as("answer").unwrap();
        let solve_xml = solve.build().unwrap().to_solveblock_xml().unwrap();
        document.add_opaque(500., 450., solve_xml, "solveblock");
        add_math_tree(
            &mut document,
            500.,
            720.,
            binary_tree(
                BinaryOp::Evaluate,
                identifier("answer"),
                tree(NodeKind::Placeholder, vec![]),
            ),
        );
        document
    }
    pub(super) fn frame(app: &mut WorksheetApp, ctx: &egui::Context, events: Vec<egui::Event>) {
        let _timing = crate::frame_profile::scope(crate::frame_profile::Phase::Egui);
        let mut events = events;
        loop {
            let mut raw = egui::RawInput {
                events,
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1200., 900.))),
                ..Default::default()
            };
            app.prepare_plot_input(ctx, &mut raw);
            let mut frame = ctx.run_ui(raw, |ui| app.page_ui(ui));
            crate::frame_profile::tessellate(ctx, &mut frame);
            frame.drop_without_applying_deltas();
            if app.plot_input_queue.is_empty() {
                break;
            }
            events = Vec::new();
        }
    }

    pub(super) fn enter() -> egui::Event {
        egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }
    }
    fn key(key: egui::Key, modifiers: egui::Modifiers) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }
    }

    fn editable_definition_with_dependent() -> (WorksheetApp, egui::Context, u64, u64) {
        let context = egui::Context::default();
        crate::configure(&context);
        let mut app = WorksheetApp {
            insertion: Some((38.4, 38.4)),
            ..Default::default()
        };
        frame(&mut app, &context, vec![egui::Event::Text("a:=2".into())]);
        let definition = app.selected.unwrap();
        frame(&mut app, &context, vec![enter()]);
        frame(&mut app, &context, vec![egui::Event::Text("a=".into())]);
        let dependent = app.selected.unwrap();
        frame(&mut app, &context, vec![enter()]);
        app.selected = Some(definition);
        app.focus_new = Some(definition);
        frame(&mut app, &context, vec![]);
        let editor = app.editors.get_mut(&definition).unwrap();
        let rhs = editor.arena().get(editor.root()).unwrap().children()[1];
        editor.place_text_caret(rhs, 1).unwrap();
        app.last_executed_regions.clear();
        (app, context, definition, dependent)
    }

    #[test]
    fn formula_typing_preserves_committed_results_until_enter() {
        let (mut app, context, definition, dependent) = editable_definition_with_dependent();
        for digit in ["3", "4"] {
            frame(&mut app, &context, vec![egui::Event::Text(digit.into())]);
            assert!(app.last_executed_regions.is_empty());
            assert!(app.uncommitted_math.contains(&definition));
            assert_eq!(
                app.results[&dependent]
                    .value
                    .as_ref()
                    .unwrap()
                    .numeric_json()["real"],
                2.
            );
        }
        frame(&mut app, &context, vec![enter()]);
        assert!(app.uncommitted_math.is_empty());
        assert_eq!(
            app.results[&dependent]
                .value
                .as_ref()
                .unwrap()
                .numeric_json()["real"],
            234.
        );
        assert!(app.last_executed_regions.contains(&dependent.to_string()));
    }

    #[test]
    fn leaving_a_formula_commits_but_manual_commit_keeps_previous_answers() {
        for manual in [false, true] {
            let (mut app, context, _, dependent) = editable_definition_with_dependent();
            app.automatic_calculation = !manual;
            frame(&mut app, &context, vec![egui::Event::Text("3".into())]);
            let point = pos2(600., 500.);
            for pressed in [true, false] {
                frame(
                    &mut app,
                    &context,
                    vec![
                        egui::Event::PointerMoved(point),
                        egui::Event::PointerButton {
                            pos: point,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                );
            }
            assert!(app.uncommitted_math.is_empty());
            assert_eq!(
                app.results[&dependent]
                    .value
                    .as_ref()
                    .unwrap()
                    .numeric_json()["real"],
                if manual { 2. } else { 23. }
            );
            if manual {
                assert!(app.pending_regions.contains(&dependent));
                assert!(app.last_executed_regions.is_empty());
                app.recalculate();
                assert_eq!(
                    app.results[&dependent]
                        .value
                        .as_ref()
                        .unwrap()
                        .numeric_json()["real"],
                    23.
                );
            }
        }
    }

    #[test]
    fn formula_undo_and_ime_commit_remain_drafts_until_explicit_evaluation() {
        let (mut app, context, definition, dependent) = editable_definition_with_dependent();
        frame(
            &mut app,
            &context,
            vec![egui::Event::Ime(egui::ImeEvent::Commit("3".into()))],
        );
        assert!(app.last_executed_regions.is_empty());
        app.undo();
        assert!(app.last_executed_regions.is_empty());
        assert!(app.uncommitted_math.contains(&definition));
        frame(&mut app, &context, vec![egui::Event::Text("4".into())]);
        app.selected = Some(definition);
        app.calculate_command(CalculationCommand::Worksheet);
        assert!(app.uncommitted_math.is_empty());
        assert_eq!(
            app.results[&dependent]
                .value
                .as_ref()
                .unwrap()
                .numeric_json()["real"],
            24.
        );
    }

    #[test]
    fn uncommitted_input_never_saves_old_dependent_results_as_synchronized() {
        let (mut app, context, _, dependent) = editable_definition_with_dependent();
        frame(&mut app, &context, vec![egui::Event::Text("3".into())]);
        app.persist_calculation_cache();
        let row = app
            .document
            .calculation_cache
            .as_ref()
            .unwrap()
            .rows
            .iter()
            .find(|r| r.region_id == dependent)
            .unwrap();
        assert_eq!(row.status, SavedCalculationStatus::Pending);
        assert_eq!(
            app.results[&dependent]
                .value
                .as_ref()
                .unwrap()
                .numeric_json()["real"],
            2.
        );
    }
    #[test]
    fn automatic_toggle_and_offscreen_f5_use_committed_input_without_losing_draft() {
        for has_bound in [false, true] {
            let (mut app, context, definition, dependent) = editable_definition_with_dependent();
            app.automatic_calculation = false;
            frame(&mut app, &context, vec![egui::Event::Text("3".into())]);
            app.ribbon_command(&context, RibbonAction::SetAutomaticCalculation(true));
            assert!(app.uncommitted_math.contains(&definition));
            assert_eq!(
                app.results[&dependent]
                    .value
                    .as_ref()
                    .unwrap()
                    .numeric_json()["real"],
                2.
            );
            app.visible_regions.clear();
            app.visible_calculation_end = has_bound.then_some(dependent);
            app.calculate_command(CalculationCommand::Now);
            assert!(app.uncommitted_math.contains(&definition));
            assert_eq!(
                app.results[&dependent]
                    .value
                    .as_ref()
                    .unwrap()
                    .numeric_json()["real"],
                2.
            );
            frame(&mut app, &context, vec![enter()]);
            assert!(app.uncommitted_math.is_empty());
            assert_eq!(
                app.results[&dependent]
                    .value
                    .as_ref()
                    .unwrap()
                    .numeric_json()["real"],
                23.
            );
        }
    }

    #[test]
    fn auto_on_during_a_new_definition_keeps_it_out_of_the_committed_engine() {
        let context = egui::Context::default();
        crate::configure(&context);
        let mut app = WorksheetApp {
            automatic_calculation: false,
            ..Default::default()
        };
        frame(&mut app, &context, vec![egui::Event::Text("a:=2".into())]);
        let id = app.selected.unwrap();
        app.ribbon_command(&context, RibbonAction::SetAutomaticCalculation(true));
        assert!(app.uncommitted_math.contains(&id));
        assert!(app.calculation_error.is_none());
        assert!(!app.results.contains_key(&id));
        frame(&mut app, &context, vec![enter()]);
        assert!(!app.uncommitted_math.contains(&id));
        assert!(app.last_executed_regions.contains(&id.to_string()));
    }

    #[test]
    fn undo_restores_the_editing_caret_and_typing_continues_in_the_same_region() {
        let context = egui::Context::default();
        crate::configure(&context);
        let mut app = WorksheetApp {
            insertion: Some((38.4, 38.4)),
            ..Default::default()
        };
        frame(&mut app, &context, vec![egui::Event::Text("a:=12".into())]);
        frame(&mut app, &context, vec![]);
        let id = app.selected.unwrap();
        let editor = app.editors.get_mut(&id).unwrap();
        let number = editor.arena().get(editor.root()).unwrap().children()[1];
        editor.place_text_caret(number, 1).unwrap();
        frame(&mut app, &context, vec![egui::Event::Text("3".into())]);
        app.undo();
        assert_eq!(app.selected, Some(id));
        assert_eq!(app.editors[&id].caret().value_range.unwrap().focus, 1);
        frame(&mut app, &context, vec![egui::Event::Text("9".into())]);
        assert_eq!(app.selected, Some(id));
        assert_eq!(
            app.editors[&id].arena().get(number).unwrap().kind(),
            &NodeKind::Number("192".into())
        );
        assert!(app.redo.is_empty());
    }

    #[test]
    fn typing_history_retains_only_the_active_region_and_preserves_other_state() {
        let context = egui::Context::default();
        crate::configure(&context);
        let mut app = WorksheetApp::default();
        frame(&mut app, &context, vec![egui::Event::Text("12".into())]);
        frame(&mut app, &context, vec![]);
        let active = app.selected.unwrap();
        for i in 0..3999 {
            app.document
                .add_text(200., 2000. + i as f64 * 40., "preserved text");
        }
        let unrelated = app.document.regions[1..].to_vec();
        let next_id = app.document.next_id;
        let before = app.document.regions[0].clone();
        frame(&mut app, &context, vec![egui::Event::Text("3".into())]);
        assert!(matches!(
            &app.undo.last().unwrap().document,
            DocumentHistory::Region(region) if region.id == active
        ));
        let after = app.document.regions[0].clone();
        assert_ne!(before.content, after.content);
        app.undo();
        assert_eq!(app.document.regions[0], before);
        assert_eq!(app.document.regions[1..], unrelated);
        assert_eq!(app.document.next_id, next_id);
        assert_eq!(app.editing_region, Some(active));
        app.redo();
        assert_eq!(app.document.regions[0], after);
        assert_eq!(app.document.regions[1..], unrelated);
        assert_eq!(app.document.next_id, next_id);
    }

    #[test]
    fn region_and_structural_history_can_be_undone_and_redone_in_order() {
        let mut app = WorksheetApp::default();
        let id = app.document.add_text(0., 0., "first");
        app.selected = Some(id);
        app.editing_region = Some(id);
        app.insertion = None;
        let before = app.input_snapshot();
        app.document.regions[0].content = CanvasContent::Text("edited".into());
        app.remember(before);
        let before = app.snapshot();
        let second = app.document.add_text(300., 0., "second");
        app.remember(before);
        app.undo();
        assert_eq!(app.document.regions.len(), 1);
        assert!(
            matches!(&app.document.regions[0].content, CanvasContent::Text(t) if t == "edited")
        );
        app.undo();
        assert!(matches!(&app.document.regions[0].content, CanvasContent::Text(t) if t == "first"));
        app.redo();
        assert!(
            matches!(&app.document.regions[0].content, CanvasContent::Text(t) if t == "edited")
        );
        app.redo();
        assert_eq!(app.document.regions.len(), 2);
        assert_eq!(app.document.regions[1].id, second);
        assert!(
            matches!(&app.document.regions[0].content, CanvasContent::Text(t) if t == "edited")
        );
    }

    #[test]
    #[ignore]
    fn benchmark_typing_history_full_document_versus_region() {
        for count in [100, 1000, 4000] {
            let mut app = WorksheetApp::default();
            for i in 0..count {
                app.document
                    .add_text(0., i as f64 * 40., "preserved worksheet content".repeat(16));
            }
            app.selected = Some(app.document.regions[0].id);
            app.editing_region = app.selected;
            app.insertion = None;
            for full in [true, false] {
                let started = std::time::Instant::now();
                for _ in 0..1000 {
                    std::hint::black_box(if full {
                        app.snapshot()
                    } else {
                        app.input_snapshot()
                    });
                }
                println!(
                    "PERF typing_history regions={count} snapshots=1000 full={full} elapsed_us={}",
                    started.elapsed().as_micros()
                );
            }
        }
    }
    #[test]
    fn inserting_a_fraction_from_the_menu_preserves_the_selected_operand() {
        let mut app = WorksheetApp::default();
        let id = app.insert((38.4, 38.4), false);
        let mut editor = Editor::new();
        formula::input_text(&mut editor, "1+23");
        let root = editor.root();
        let operand = editor.arena().get(root).unwrap().children()[1];
        editor.select_subtree(operand).unwrap();
        let before = editor.to_portable().unwrap();
        app.document.regions[0].content = CanvasContent::Math(before.clone());
        app.editors.insert(id, editor);
        let placeholder = || PortableTree {
            kind: NodeKind::Placeholder,
            children: vec![],
        };
        app.insert_template(PortableTree {
            kind: NodeKind::Fraction,
            children: vec![placeholder(), placeholder()],
        });
        let tree = app.editors[&id].to_portable().unwrap();
        assert_eq!(tree.children[1].kind, NodeKind::Fraction);
        assert_eq!(
            tree.children[1].children[0].kind,
            NodeKind::Number("23".into())
        );
        assert_eq!(app.editors[&id].root(), root);
        app.undo();
        assert_eq!(app.editors[&id].to_portable().unwrap(), before);
        assert_eq!(
            app.editors[&id].selection(),
            mathcraft_editor::Selection::Subtree(operand)
        );
    }

    #[test]
    fn document_undo_restores_text_selection_before_further_typing() {
        let context = egui::Context::default();
        crate::configure(&context);
        let mut app = WorksheetApp::default();
        app.begin_region(true);
        let id = app.selected.unwrap();
        frame(&mut app, &context, vec![]);
        frame(&mut app, &context, vec![egui::Event::Text("abcd".into())]);
        let text_id = egui::Id::new(("worksheet-text", id));
        let range = egui::text::CCursorRange::one(egui::text::CCursor::new(2));
        let mut state = egui::text_edit::TextEditState::load(&context, text_id).unwrap();
        state.cursor.set_char_range(Some(range));
        state.store(&context, text_id);
        app.text_cursors.insert(id, range);
        frame(&mut app, &context, vec![egui::Event::Text("X".into())]);
        assert!(matches!(&app.document.regions[0].content, CanvasContent::Text(s) if s == "abXcd"));
        app.undo();
        frame(&mut app, &context, vec![egui::Event::Text("Y".into())]);
        assert_eq!(app.editing_region, Some(id));
        assert!(matches!(&app.document.regions[0].content, CanvasContent::Text(s) if s == "abYcd"));
    }

    #[test]
    fn imported_rich_text_click_enters_editing_and_undo_restores_source_presentation() {
        use mathcraft_document::native_visual::{RegionVisual, TextStyle, read_flow_xml};
        let context = egui::Context::default();
        crate::configure(&context);
        let mut app = WorksheetApp::default();
        let id = app.document.add_text(0., 0., "Native text");
        app.document.regions[0].source_region = Some("text".into());
        let document = read_flow_xml(r##"<FlowDocument xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation" Background="#FF662D91" Foreground="#FFFFFFFF"><Paragraph><Run>Native text</Run></Paragraph></FlowDocument>"##, TextStyle::default()).unwrap();
        app.native_visuals = Arc::new(NativeVisuals {
            regions: HashMap::from([(
                "text".into(),
                Ok(RegionVisual {
                    size: [220., 35.],
                    background: Some([102, 45, 145, 255]),
                    document: Some(document),
                }),
            )]),
        });
        let output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1200., 900.))),
                ..Default::default()
            },
            |ui| app.page_ui(ui),
        );
        fn locate(shape: &egui::Shape) -> Option<egui::Pos2> {
            match shape {
                egui::Shape::Text(t) if t.galley.text() == "Native text" => {
                    Some(t.pos + t.galley.size() / 2.)
                }
                egui::Shape::Vec(shapes) => shapes.iter().find_map(locate),
                _ => None,
            }
        }
        let point = output.shapes.iter().find_map(|s| locate(&s.shape)).unwrap();
        output.drop_without_applying_deltas();
        for pressed in [true, false] {
            frame(
                &mut app,
                &context,
                vec![
                    egui::Event::PointerMoved(point),
                    egui::Event::PointerButton {
                        pos: point,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
        }
        frame(&mut app, &context, vec![]);
        assert_eq!(app.editing_region, Some(id));
        frame(&mut app, &context, vec![egui::Event::Text("!".into())]);
        assert!(
            matches!(&app.document.regions[0].content, CanvasContent::Text(t) if t.contains('!'))
        );
        assert!(
            app.native_visuals
                .region(&app.document.regions[0])
                .is_none()
        );
        app.undo();
        assert!(
            matches!(&app.document.regions[0].content, CanvasContent::Text(t) if t == "Native text")
        );
        assert!(
            app.native_visuals
                .region(&app.document.regions[0])
                .is_some()
        );
        assert!(app.last_executed_regions.is_empty());
    }

    #[test]
    fn escape_selects_region_arrows_move_it_and_enter_resumes_editing() {
        let context = egui::Context::default();
        crate::configure(&context);
        let mut app = WorksheetApp {
            insertion: Some((38.4, 38.4)),
            ..Default::default()
        };
        frame(&mut app, &context, vec![egui::Event::Text("23".into())]);
        frame(&mut app, &context, vec![]);
        let id = app.selected.unwrap();
        let original_x = app.document.regions[0].left;
        frame(
            &mut app,
            &context,
            vec![key(egui::Key::Escape, egui::Modifiers::NONE)],
        );
        assert_eq!(app.selected, Some(id));
        assert!(app.editing_region.is_none());
        frame(
            &mut app,
            &context,
            vec![key(egui::Key::ArrowRight, egui::Modifiers::NONE)],
        );
        assert!((app.document.regions[0].left - original_x - app.document.grid).abs() < 1e-10);
        app.undo();
        assert_eq!(app.selected, Some(id));
        assert_eq!(app.document.regions[0].left, original_x);
        frame(&mut app, &context, vec![enter()]);
        assert_eq!(app.focus_new, Some(id));
        assert_eq!(app.document.regions.len(), 1);
        frame(&mut app, &context, vec![egui::Event::Text("4".into())]);
        assert_eq!(app.editing_region, Some(id));
        assert_eq!(
            app.editors[&id].to_portable().unwrap().kind,
            NodeKind::Number("234".into())
        );
    }
    #[test]
    fn cancelled_blank_page_composition_does_not_create_an_empty_region() {
        let context = egui::Context::default();
        crate::configure(&context);
        let mut app = WorksheetApp {
            insertion: Some((38.4, 38.4)),
            ..Default::default()
        };
        frame(
            &mut app,
            &context,
            vec![egui::Event::Ime(egui::ImeEvent::Preedit {
                text: "한".into(),
                active_range_chars: None,
            })],
        );
        assert!(app.document.regions.is_empty());
        assert!(app.undo.is_empty());
        frame(
            &mut app,
            &context,
            vec![egui::Event::Ime(egui::ImeEvent::Commit(String::new()))],
        );
        assert!(app.document.regions.is_empty());
        frame(
            &mut app,
            &context,
            vec![egui::Event::Ime(egui::ImeEvent::Commit("한글".into()))],
        );
        assert_eq!(app.document.regions.len(), 1);
        let id = app.selected.unwrap();
        assert_eq!(
            app.editors[&id].to_portable().unwrap().kind,
            NodeKind::Identifier("한글".into())
        );
    }
    #[test]
    fn text_mode_accepts_first_committed_text_without_a_math_parser() {
        let context = egui::Context::default();
        crate::configure(&context);
        let mut app = WorksheetApp {
            insertion: Some((38.4, 38.4)),
            text_mode: true,
            ..Default::default()
        };
        frame(
            &mut app,
            &context,
            vec![egui::Event::Ime(egui::ImeEvent::Commit(
                "계산 메모: 2+3=5".into(),
            ))],
        );
        assert!(
            matches!(&app.document.regions[0].content, CanvasContent::Text(text) if text == "계산 메모: 2+3=5")
        );
        assert!(app.editors.is_empty());
    }
    fn workspace_frame(
        app: &mut WorksheetApp,
        ctx: &egui::Context,
        events: Vec<egui::Event>,
    ) -> egui::emath::TSTransform {
        let mut layer = None;
        let output = ctx.run_ui(
            egui::RawInput {
                events,
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1100., 900.))),
                ..Default::default()
            },
            |ui| {
                app.chrome_ui(ui);
                egui::CentralPanel::default().show(ui, |ui| {
                    layer = Some(egui::LayerId::new(
                        ui.layer_id().order,
                        egui::Id::new("worksheet-paper"),
                    ));
                    app.workspace_ui(ui);
                });
            },
        );
        fn find_text(shape: &egui::epaint::Shape) -> Option<Rect> {
            match shape {
                egui::epaint::Shape::Text(text) if text.galley.text() == "파일" => {
                    Some(Rect::from_min_size(text.pos, text.galley.size()))
                }
                egui::epaint::Shape::Vec(shapes) => shapes.iter().find_map(find_text),
                _ => None,
            }
        }
        if let Some(rect) = output.shapes.iter().find_map(|s| find_text(&s.shape)) {
            ctx.data_mut(|data| data.insert_temp(egui::Id::new("test-file-menu"), rect));
        }
        output.drop_without_applying_deltas();
        ctx.layer_transform_to_global(layer.unwrap()).unwrap()
    }
    #[test]
    fn closing_a_ribbon_menu_resumes_typing_without_a_second_escape_action() {
        let context = egui::Context::default();
        crate::configure(&context);
        let mut app = WorksheetApp::default();
        let id = app.insert((38.4, 38.4), false);
        let mut editor = Editor::new();
        formula::input_text(&mut editor, "12");
        app.document.regions[0].content = CanvasContent::Math(editor.to_portable().unwrap());
        app.editors.insert(id, editor);
        workspace_frame(&mut app, &context, vec![]);
        workspace_frame(&mut app, &context, vec![]);
        let point = context
            .data(|data| data.get_temp::<Rect>(egui::Id::new("test-file-menu")))
            .unwrap()
            .center();
        for pressed in [true, false] {
            workspace_frame(
                &mut app,
                &context,
                vec![
                    egui::Event::PointerMoved(point),
                    egui::Event::PointerButton {
                        pos: point,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
        }
        assert_eq!(app.menu_resume, Some(id));
        workspace_frame(
            &mut app,
            &context,
            vec![key(egui::Key::Escape, egui::Modifiers::NONE)],
        );
        workspace_frame(&mut app, &context, vec![egui::Event::Text("3".into())]);
        assert_eq!(app.editing_region, Some(id));
        assert_eq!(
            app.editors[&id].to_portable().unwrap().kind,
            NodeKind::Number("123".into())
        );
    }

    #[test]
    fn region_keyboard_navigation_keeps_focus_on_the_page_with_toolbar_present() {
        let context = egui::Context::default();
        crate::configure(&context);
        let mut app = WorksheetApp::default();
        for (top, text) in [(38.4, "1"), (115.2, "2")] {
            let id = app.insert((38.4, top), false);
            let mut editor = Editor::new();
            formula::input_text(&mut editor, text);
            app.document
                .regions
                .iter_mut()
                .find(|r| r.id == id)
                .unwrap()
                .content = CanvasContent::Math(editor.to_portable().unwrap());
            app.editors.insert(id, editor);
        }
        let first = app.document.regions[0].id;
        workspace_frame(&mut app, &context, vec![]);
        workspace_frame(
            &mut app,
            &context,
            vec![key(egui::Key::Escape, egui::Modifiers::NONE)],
        );
        workspace_frame(
            &mut app,
            &context,
            vec![key(egui::Key::Tab, egui::Modifiers::NONE)],
        );
        assert_eq!(app.selected, Some(first));
        assert!(app.editing_region.is_none());
        workspace_frame(&mut app, &context, vec![enter()]);
        workspace_frame(&mut app, &context, vec![egui::Event::Text("4".into())]);
        assert_eq!(app.editing_region, Some(first));
        assert_eq!(
            app.editors[&first].to_portable().unwrap().kind,
            NodeKind::Number("14".into())
        );
    }

    #[test]
    fn page_zoom_keeps_chrome_scale_and_pointer_document_coordinates() {
        for zoom in [0.5, 1., 2.] {
            let context = egui::Context::default();
            crate::configure(&context);
            let mut app = WorksheetApp {
                zoom,
                ..Default::default()
            };
            let transform = workspace_frame(&mut app, &context, vec![]);
            assert_eq!(context.zoom_factor(), 1.);
            assert_eq!(transform.scaling, zoom);
            let expected = (app.document.grid * 6., app.document.grid * 7.);
            let point = transform * app.page_geometry().to_paper(expected.0, expected.1);
            workspace_frame(
                &mut app,
                &context,
                vec![
                    egui::Event::PointerMoved(point),
                    egui::Event::PointerButton {
                        pos: point,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
            workspace_frame(
                &mut app,
                &context,
                vec![egui::Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                }],
            );
            let (x, y) = app
                .insertion
                .unwrap_or_else(|| panic!("insertion missing at zoom {zoom}"));
            assert!(
                (x - expected.0).abs() < 0.001 && (y - expected.1).abs() < 0.001,
                "zoom={zoom} position={x},{y}"
            );
            assert!(app.document.regions.is_empty());
        }
    }
    #[test]
    fn typing_on_page_defining_evaluating_and_reopening_is_one_workflow() {
        let context = egui::Context::default();
        crate::configure(&context);
        let mut app = WorksheetApp::default();
        assert!(app.document.regions.is_empty());
        app.insertion = Some((38.4, 38.4));
        frame(&mut app, &context, vec![egui::Event::Text("a:=2".into())]);
        frame(&mut app, &context, vec![enter()]);
        frame(&mut app, &context, vec![egui::Event::Text("a+3=".into())]);
        assert_eq!(app.document.regions.len(), 2);
        let id = app.document.regions[1].id;
        assert_eq!(
            app.results[&id].value.as_ref().unwrap().numeric_json()["real"],
            5.
        );
        assert!(app.document.regions[1].top > app.document.regions[0].top);
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "mathcraft-workflow-{}-{stamp}.mathcraft",
            std::process::id()
        ));
        app.sync_editors().unwrap();
        app.document.save(&path).unwrap();
        let doc = CanvasDocument::load(&path).unwrap();
        std::fs::remove_file(&path).unwrap();
        let mut reopened = WorksheetApp {
            document: doc,
            ..Default::default()
        };
        reopened.recalculate();
        assert_eq!(
            reopened.results[&id].value.as_ref().unwrap().numeric_json()["real"],
            5.
        );
    }
    #[test]
    fn moving_definition_changes_calculation_order_and_undo_restores_it() {
        let mut app = WorksheetApp::default();
        for (y, text) in [(38.4, "a:=2"), (115.2, "a+3=")] {
            let id = app.insert((38.4, y), false);
            let mut editor = Editor::new();
            formula::input_text(&mut editor, text);
            app.document
                .regions
                .iter_mut()
                .find(|r| r.id == id)
                .unwrap()
                .content = CanvasContent::Math(editor.to_portable().unwrap());
        }
        app.recalculate();
        let eval = app.document.regions[1].id;
        let definition = app.document.regions[0].id;
        assert!(app.results[&eval].error.is_none());
        let before = app.snapshot();
        app.document.move_region(definition, 38.4, 230.4).unwrap();
        app.remember(before);
        app.recalculate();
        assert_eq!(
            app.results[&eval].error.as_ref().unwrap().code,
            "bad%_variable"
        );
        app.undo();
        assert_eq!(
            app.results[&eval].value.as_ref().unwrap().numeric_json()["real"],
            5.
        );
    }
    #[test]
    fn typed_unit_and_output_override_keep_physical_and_display_values() {
        let mut app = WorksheetApp::default();
        for (y, text) in [(38.4, "d:=2m"), (115.2, "d=cm")] {
            let id = app.insert((38.4, y), false);
            let mut editor = Editor::new();
            formula::input_text(&mut editor, text);
            app.document
                .regions
                .iter_mut()
                .find(|r| r.id == id)
                .unwrap()
                .content = CanvasContent::Math(editor.to_portable().unwrap());
        }
        app.recalculate();
        let id = app.document.regions[1].id;
        let row = &app.results[&id];
        assert_eq!(row.value.as_ref().unwrap().numeric_json()["real"], 2.);
        assert_eq!(
            row.display_value.as_ref().unwrap().numeric_json()["real"],
            200.
        );
        assert_eq!(value_text(row.value.as_ref().unwrap()), "2 m");
    }
    #[test]
    fn clicking_empty_paper_and_typing_keeps_first_character() {
        let context = egui::Context::default();
        crate::configure(&context);
        let mut app = WorksheetApp::default();
        let point = pos2(100., 100.);
        frame(&mut app, &context, vec![]);
        frame(
            &mut app,
            &context,
            vec![
                egui::Event::PointerMoved(point),
                egui::Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        frame(
            &mut app,
            &context,
            vec![egui::Event::PointerButton {
                pos: point,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        assert!(app.insertion.is_some());
        frame(&mut app, &context, vec![egui::Event::Text("7=".into())]);
        assert_eq!(app.document.regions.len(), 1);
        let id = app.document.regions[0].id;
        assert_eq!(
            app.results[&id].value.as_ref().unwrap().numeric_json()["real"],
            7.
        );
    }
    #[test]
    fn each_new_formula_character_can_arrive_in_consecutive_frames() {
        let context = egui::Context::default();
        crate::configure(&context);
        let mut app = WorksheetApp {
            insertion: Some((38.4, 38.4)),
            ..Default::default()
        };
        for text in ["a", ":", "2"] {
            frame(&mut app, &context, vec![egui::Event::Text(text.into())]);
        }
        frame(&mut app, &context, vec![enter()]);
        for text in ["a", "+", "3", "="] {
            frame(&mut app, &context, vec![egui::Event::Text(text.into())]);
        }
        let id = app.document.regions[1].id;
        assert_eq!(
            app.results[&id].value.as_ref().unwrap().numeric_json()["real"],
            5.
        );
    }
    #[test]
    fn new_formula_accepts_all_text_and_paste_events_in_order() {
        let context = egui::Context::default();
        crate::configure(&context);
        let mut app = WorksheetApp {
            insertion: Some((38.4, 38.4)),
            ..Default::default()
        };
        frame(
            &mut app,
            &context,
            vec![
                egui::Event::Text("2".into()),
                egui::Event::Text("+3".into()),
                egui::Event::Paste("=".into()),
            ],
        );
        let id = app.document.regions[0].id;
        assert_eq!(
            app.results[&id].value.as_ref().unwrap().numeric_json()["real"],
            5.
        );
    }
    #[test]
    fn incomplete_redefinition_poison_is_local_and_does_not_show_stale_answer() {
        let mut app = WorksheetApp::default();
        let mut ids = Vec::new();
        for (i, text) in ["a:=2", "a:=3", "a=", "2+3="].iter().enumerate() {
            let id = app.insert((38.4, 38.4 + i as f64 * 96.), false);
            let mut editor = Editor::new();
            formula::input_text(&mut editor, text);
            let tree = editor.to_portable().unwrap();
            app.document
                .regions
                .iter_mut()
                .find(|r| r.id == id)
                .unwrap()
                .content = CanvasContent::Math(tree);
            app.editors.insert(id, editor);
            ids.push(id);
        }
        app.recalculate();
        assert_eq!(
            app.results[&ids[2]].value.as_ref().unwrap().numeric_json()["real"],
            3.
        );
        let editor = app.editors.get_mut(&ids[1]).unwrap();
        let rhs = editor.arena().get(editor.root()).unwrap().children()[1];
        editor.set_atom(rhs, NodeKind::Number(".".into())).unwrap();
        app.recalculate();
        assert!(app.results[&ids[1]].value.is_none());
        assert!(app.results[&ids[1]].error.is_some());
        assert!(app.results[&ids[2]].value.is_none());
        assert!(app.results[&ids[2]].error.is_some());
        assert_eq!(
            app.results[&ids[3]].value.as_ref().unwrap().numeric_json()["real"],
            5.
        );
        assert!(app.sync_editors().is_err());
    }
    #[test]
    fn region_and_formula_clipboard_keep_structure_and_use_new_ids() {
        let mut app = WorksheetApp::default();
        let mut editor = Editor::new();
        formula::input_text(&mut editor, "2+3=");
        let tree = editor.to_portable().unwrap();
        let text = format!("mathcraft-tree:{}", serde_json::to_string(&tree).unwrap());
        assert!(app.paste_region(&text));
        let first = app.document.regions[0].id;
        app.duplicate_selected();
        let second = app.document.regions[1].id;
        assert_ne!(first, second);
        app.recalculate();
        assert_eq!(
            app.results[&first].value.as_ref().unwrap().numeric_json()["real"],
            5.
        );
        assert_eq!(
            app.results[&second].value.as_ref().unwrap().numeric_json()["real"],
            5.
        );
        app.remove_selected();
        assert_eq!(app.document.regions.len(), 1);
        app.undo();
        assert_eq!(app.document.regions.len(), 2);
    }
    #[test]
    fn ribbon_unit_picker_keeps_unit_namespace_after_same_name_variable_definition() {
        let context = egui::Context::default();
        crate::configure(&context);
        let mut app = WorksheetApp::default();
        frame(&mut app, &context, vec![egui::Event::Text("m:=5".into())]);
        frame(&mut app, &context, vec![enter()]);
        frame(&mut app, &context, vec![egui::Event::Text("2".into())]);
        let id = app.selected.unwrap();
        app.ribbon_command(&context, crate::ribbon::unit_choice("m"));
        app.ribbon_command(&context, RibbonAction::Evaluate);
        let quantity = app.results[&id].value.as_ref().unwrap();
        assert_eq!(quantity.numeric_json()["real"], 2.);
        assert_eq!(value_text(quantity), "2 m");
        assert!(app.editors[&id].arena().nodes().any(|(_, n)| matches!(n.kind(), NodeKind::AnnotatedIdentifier {name, labels} if name == "m" && labels == "UNIT")));
    }

    #[test]
    fn ribbon_identifier_on_blank_page_is_one_document_command() {
        let context = egui::Context::default();
        crate::configure(&context);
        let mut app = WorksheetApp::default();
        app.ribbon_command(
            &context,
            RibbonAction::InsertIdentifier {
                name: "c",
                label: "CONSTANT",
            },
        );
        assert_eq!(app.document.regions.len(), 1);
        assert_eq!(app.undo.len(), 1);
        app.ribbon_command(&context, RibbonAction::Evaluate);
        let id = app.selected.unwrap();
        assert_eq!(
            app.results[&id].value.as_ref().unwrap().numeric_json()["real"],
            299792458.
        );
        app.undo();
        app.undo();
        assert!(app.document.regions.is_empty());
    }
    #[test]
    fn ribbon_inserts_program_calculus_and_symbolic_structures_as_document_commands() {
        let context = egui::Context::default();
        crate::configure(&context);
        for (action, expected) in [
            (
                RibbonAction::Program,
                NodeKind::Program { statement_count: 1 },
            ),
            (RibbonAction::Integral, NodeKind::Integral),
            (
                RibbonAction::SymbolicEvaluate,
                NodeKind::SymbolicEvaluation { command_count: 1 },
            ),
        ] {
            let mut app = WorksheetApp::default();
            app.ribbon_command(&context, action);
            let id = app.selected.unwrap();
            assert_eq!(app.undo.len(), 1);
            assert!(
                app.editors[&id]
                    .arena()
                    .nodes()
                    .any(|(_, node)| node.kind() == &expected)
            );
            app.document.validate().unwrap();
            app.undo();
            assert!(app.document.regions.is_empty());
        }
    }
    #[test]
    fn solve_block_authoring_builds_native_structure_calculates_and_exports_result() {
        let context = egui::Context::default();
        crate::configure(&context);
        let mut app = WorksheetApp::default();
        app.ribbon_command(&context, RibbonAction::InsertSolveBlock);
        assert!(app.commit_solve_draft(), "{}", app.status);
        let solve_id = app.selected.unwrap();
        assert!(matches!(
            &app.document.regions[0].content,
            CanvasContent::Opaque { kind, xml }
                if kind == "solveblock"
                    && xml.contains("solve-block-category=\"constraint\"")
                    && xml.contains("labels=\"KEYWORD\"")
                    && xml.contains(">Find</ml:id>")
        ));
        assert_eq!(
            value_text(app.results[&solve_id].value.as_ref().unwrap()),
            "[2 × 1] 3; 2"
        );

        let answer_id = app.document.add_math(0., app.document.grid * 24.);
        let mut editor = Editor::new();
        formula::input_text(&mut editor, "answer=");
        app.document
            .regions
            .iter_mut()
            .find(|region| region.id == answer_id)
            .unwrap()
            .content = CanvasContent::Math(editor.to_portable().unwrap());
        app.editors.insert(answer_id, editor);
        app.recalculate();
        assert_eq!(
            value_text(app.results[&answer_id].value.as_ref().unwrap()),
            "[2 × 1] 3; 2"
        );
        app.document.validate().unwrap();
        app.undo();
        assert!(app.document.regions.is_empty());
    }
    #[test]
    fn engineering_sheet_function_range_plot_format_save_reopen_and_undo() {
        let context = egui::Context::default();
        crate::configure(&context);
        let mut app = WorksheetApp::default();
        for text in ["a:=2", "f(x):=a*x^2", "x:=0,0.5..2"] {
            frame(&mut app, &context, vec![egui::Event::Text(text.into())]);
            frame(&mut app, &context, vec![enter()]);
        }
        let a_id = app.document.regions[0].id;
        let f_id = app.document.regions[1].id;
        app.selected = Some(f_id);
        app.ribbon_command(&context, RibbonAction::SetFontSize(18.));
        app.ribbon_command(&context, RibbonAction::SetPrecision(6));
        app.ribbon_command(&context, RibbonAction::SetResultFormat(2));
        app.selected = None;
        app.ribbon_command(&context, RibbonAction::InsertPlot);
        let plot_id = app.selected.unwrap();
        if let CanvasContent::Plot(plot) = &mut app.document.regions.last_mut().unwrap().content {
            plot.set_axis_expression_text(mathcraft_document::canvas::PlotAxis::X, 0, "x")
                .unwrap();
            plot.set_axis_expression_text(mathcraft_document::canvas::PlotAxis::Y, 0, "f(x)")
                .unwrap();
        }
        app.recalculate();
        let points = &app.plot_results[&plot_id]
            .as_ref()
            .unwrap()
            .first_curve()
            .unwrap()
            .points;
        assert_eq!(
            points,
            &vec![[0., 0.], [0.5, 0.5], [1., 2.], [1.5, 4.5], [2., 8.]]
        );
        app.sync_editors().unwrap();
        let path = std::env::temp_dir().join(format!(
            "mathcraft-engineering-{}-{}.mathcraft",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        app.persist_calculation_cache();
        app.document.save(&path).unwrap();
        app.open(&path);
        std::fs::remove_file(path).unwrap();
        assert_eq!(
            app.plot_results[&plot_id]
                .as_ref()
                .unwrap()
                .first_curve()
                .unwrap()
                .points
                .last(),
            Some(&[2., 8.])
        );
        let format = app
            .document
            .regions
            .iter()
            .find(|r| r.id == f_id)
            .unwrap()
            .format;
        assert_eq!(format.font_points, 18.);
        assert_eq!(format.precision, 6);
        assert_eq!(format.result_format, ResultFormat::Scientific);
        app.selected = Some(a_id);
        app.focus_new = Some(a_id);
        frame(&mut app, &context, vec![]);
        frame(
            &mut app,
            &context,
            vec![key(egui::Key::A, egui::Modifiers::COMMAND)],
        );
        frame(&mut app, &context, vec![egui::Event::Text("a:=3".into())]);
        assert_eq!(
            app.plot_results[&plot_id]
                .as_ref()
                .unwrap()
                .first_curve()
                .unwrap()
                .points
                .last(),
            Some(&[2., 8.])
        );
        frame(&mut app, &context, vec![enter()]);
        assert_eq!(
            app.plot_results[&plot_id]
                .as_ref()
                .unwrap()
                .first_curve()
                .unwrap()
                .points
                .last(),
            Some(&[2., 12.])
        );
        app.undo();
        app.commit_math_edits();
        assert_eq!(
            app.plot_results[&plot_id]
                .as_ref()
                .unwrap()
                .first_curve()
                .unwrap()
                .points
                .last(),
            Some(&[2., 8.])
        );
    }

    #[test]
    fn plot_axis_typing_stays_in_the_plot_and_escape_returns_to_region_selection() {
        let context = egui::Context::default();
        crate::configure(&context);
        let mut app = WorksheetApp::default();
        app.ribbon_command(&context, RibbonAction::InsertPlot);
        let id = app.selected.unwrap();
        frame(&mut app, &context, vec![]);
        frame(&mut app, &context, vec![]);
        frame(
            &mut app,
            &context,
            vec![key(egui::Key::A, egui::Modifiers::COMMAND)],
        );
        frame(&mut app, &context, vec![egui::Event::Text("cos(x)".into())]);
        assert_eq!(app.document.regions.len(), 1);
        assert!(
            matches!(&app.document.regions[0].content,CanvasContent::Plot(p) if p.primary_y_expression().is_exact_placeholder())
        );
        assert!(app.inline_plots[&id].has_changes());
        frame(&mut app, &context, vec![enter()]);
        assert!(
            matches!(&app.document.regions[0].content,CanvasContent::Plot(p) if p.primary_y_expression().as_tree().is_some_and(|tree| worksheet_math::tree_to_mathml(tree).unwrap().contains(">cos<")))
        );
        assert_eq!(app.editing_region, Some(id));
        frame(
            &mut app,
            &context,
            vec![key(egui::Key::Escape, egui::Modifiers::NONE)],
        );
        assert_eq!(app.selected, Some(id));
        assert!(app.editing_region.is_none());
        let before = app.document.regions[0].left;
        frame(
            &mut app,
            &context,
            vec![key(egui::Key::ArrowRight, egui::Modifiers::NONE)],
        );
        assert!((app.document.regions[0].left - before - app.document.grid).abs() < 1e-8);
    }

    #[test]
    fn legacy_plot_settings_commit_multiple_traces_axes_styles_save_reopen_and_undo() {
        let context = egui::Context::default();
        crate::configure(&context);
        let mut app = WorksheetApp::default();
        app.ribbon_command(&context, RibbonAction::InsertPlot);
        let id = app.selected.unwrap();
        assert!(
            matches!(&app.document.regions[0].content,CanvasContent::Plot(spec) if spec.primary_x_expression().is_exact_placeholder()&&spec.primary_y_expression().is_exact_placeholder())
        );
        // This regression exercises the existing Legacy-pair settings adapter.
        app.document.regions[0].content = CanvasContent::Plot(PlotSpec::default());
        app.recalculate();
        app.ribbon_command(&context, RibbonAction::EditPlot);
        let draft = app.plot_draft.as_mut().unwrap();
        draft.spec.legacy_traces_mut().unwrap()[0].label = "sin".into();
        draft.spec.legacy_traces_mut().unwrap()[0].style.line = PlotLineStyle::Dashed;
        draft.spec.legacy_traces_mut().unwrap()[0].style.color = [190, 35, 45, 255];
        let mut cosine = PlotTraceSpec {
            y_expression: "cos(x)".into(),
            label: "cos".into(),
            ..Default::default()
        };
        cosine.style.marker = PlotMarker::Circle;
        cosine.style.marker_frequency = 25;
        draft.spec.add_trace(cosine).unwrap();
        draft.spec.x_axis.label = "angle".into();
        draft.spec.x_axis.range = Some(PlotRange { min: -3., max: 3. });
        draft.spec.y_axis.label = "amplitude".into();
        draft.spec.y_axis.range = Some(PlotRange { min: -1., max: 1. });
        assert!(app.commit_plot_draft());
        assert_eq!(app.undo.len(), 2);
        let data = app.plot_results[&id].as_ref().unwrap();
        assert_eq!(data.traces.len(), 2);
        assert_eq!(data.traces[0].curve().unwrap().label, "sin");
        assert_eq!(data.traces[1].curve().unwrap().label, "cos");
        assert_eq!(data.traces[0].curve().unwrap().points.len(), 500);
        assert_eq!(data.traces[1].curve().unwrap().points.len(), 500);

        let path = std::env::temp_dir().join(format!(
            "mathcraft-multi-trace-{}-{}.mathcraft",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        app.persist_calculation_cache();
        app.document.save(&path).unwrap();
        app.open(&path);
        std::fs::remove_file(path).unwrap();
        let CanvasContent::Plot(spec) = &app.document.regions[0].content else {
            panic!("expected plot")
        };
        assert_eq!(spec.trace_count(), 2);
        assert_eq!(spec.x_axis.label, "angle");
        assert_eq!(spec.y_axis.range, Some(PlotRange { min: -1., max: 1. }));

        app.undo.clear();
        app.selected = Some(id);
        app.ribbon_command(&context, RibbonAction::EditPlot);
        app.plot_draft
            .as_mut()
            .unwrap()
            .spec
            .legacy_traces_mut()
            .unwrap()[0]
            .label = "changed".into();
        assert!(app.commit_plot_draft());
        app.undo();
        let CanvasContent::Plot(spec) = &app.document.regions[0].content else {
            panic!("expected plot")
        };
        assert_eq!(spec.resolved_traces().unwrap()[0].label, "sin");
    }

    #[test]
    fn region_format_changes_undo_and_clipboard_without_changing_calculation() {
        let context = egui::Context::default();
        crate::configure(&context);
        let mut app = WorksheetApp::default();
        frame(&mut app, &context, vec![egui::Event::Text("1/3=".into())]);
        let id = app.selected.unwrap();
        let value = app.results[&id].value.as_ref().unwrap().numeric_json();
        app.ribbon_command(&context, RibbonAction::SetPrecision(8));
        app.ribbon_command(&context, RibbonAction::SetFontSize(24.));
        assert_eq!(
            app.results[&id].value.as_ref().unwrap().numeric_json(),
            value
        );
        let region = &app.document.regions[0];
        let encoded = format!(
            "mathcraft-region:{}",
            serde_json::json!({"content":region.content,"format":region.format})
        );
        app.paste_region(&encoded);
        assert_eq!(
            app.document.regions[1].format,
            app.document.regions[0].format
        );
        app.undo();
        app.undo();
        assert_eq!(app.document.regions.len(), 1);
        assert_eq!(app.document.regions[0].format.font_points, 11.);
        assert_eq!(app.document.regions[0].format.precision, 8);
        app.undo();
        assert_eq!(app.document.regions[0].format.precision, 3);
    }
    #[test]
    fn shipped_engineering_example_calculates_force_matrix_and_graph_in_placement_order() {
        let mut app = WorksheetApp {
            document: serde_json::from_str(include_str!(
                "../../../examples/engineering-worksheet.mathcraft"
            ))
            .unwrap(),
            ..Default::default()
        };
        app.document.validate().unwrap();
        app.recalculate();
        assert!(app.calculation_error.is_none());
        assert!(app.results.values().all(|r| r.error.is_none()));
        for id in [6, 7] {
            assert_eq!(
                app.results[&id].value.as_ref().unwrap().numeric_json()["real"],
                19.6133
            );
        }
        assert_eq!(
            app.results[&10].value.as_ref().unwrap().numeric_json()["real"],
            5.
        );
        let plot = app.plot_results[&14].as_ref().unwrap();
        assert_eq!(plot.first_curve().unwrap().points.len(), 5);
        assert_eq!(plot.first_curve().unwrap().points.last(), Some(&[2., 8.]));
        assert!(app.paper_size().y < 2. * app.document.height as f32);
    }

    #[test]
    fn technical_report_fixture_save_reopen_and_recalculate_all_milestone_features() {
        let document = technical_report_document();
        document.validate().unwrap();
        if std::env::var_os("MATHCRAFT_PRINT_FIXTURE").is_some() {
            println!("{}", serde_json::to_string(&document).unwrap());
        }
        let fixture: CanvasDocument =
            serde_json::from_str(include_str!("../../../examples/technical-report.mathcraft"))
                .unwrap();
        assert_eq!(fixture, document);
        let path = std::env::temp_dir().join(format!(
            "mathcraft-technical-report-{}-{}.mathcraft",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        document.save(&path).unwrap();
        let reopened = CanvasDocument::load(&path).unwrap();
        std::fs::remove_file(path).unwrap();
        assert_eq!(reopened, document);
        let mut app = WorksheetApp {
            document: reopened,
            ..Default::default()
        };
        app.recalculate();
        assert!(
            app.calculation_error.is_none(),
            "{:?}",
            app.calculation_error
        );
        assert!(
            app.results.values().all(|row| row.error.is_none()),
            "{:?}",
            app.results
        );
        assert_eq!(
            app.plot_results
                .values()
                .next()
                .unwrap()
                .as_ref()
                .unwrap()
                .traces
                .len(),
            2
        );
        assert!(app.results.values().any(|row| {
            row.value
                .as_ref()
                .is_some_and(|value| value_text(value) == "[2 × 1] 2; 3")
        }));
        assert!(app.results.values().any(|row| {
            row.value
                .as_ref()
                .is_some_and(|value| value_text(value) == "[2 × 1] 3; 2")
        }));
        assert!(app.document.page_sections.explicit);
    }

    #[test]
    fn explicit_identifier_label_changes_calculation_and_undo_restores_contextual_meaning() {
        let context = egui::Context::default();
        crate::configure(&context);
        let mut app = WorksheetApp::default();
        frame(&mut app, &context, vec![egui::Event::Text("m=".into())]);
        let id = app.selected.unwrap();
        let root = app.editors[&id].root();
        let atom = app.editors[&id].arena().get(root).unwrap().children()[0];
        app.editors
            .get_mut(&id)
            .unwrap()
            .place_text_caret(atom, 1)
            .unwrap();
        assert_eq!(value_text(app.results[&id].value.as_ref().unwrap()), "1 m");
        app.ribbon_command(&context, RibbonAction::SetLabel("VARIABLE"));
        assert!(app.results[&id].error.is_none());
        frame(&mut app, &context, vec![enter()]);
        assert!(app.results[&id].error.is_some());
        assert!(app.results[&id].value.is_none());
        app.undo();
        app.commit_math_edits();
        assert_eq!(value_text(app.results[&id].value.as_ref().unwrap()), "1 m");
    }
    #[test]
    fn imaginary_gallery_inserts_a_literal_instead_of_an_undefined_constant_name() {
        let context = egui::Context::default();
        crate::configure(&context);
        let mut app = WorksheetApp::default();
        app.ribbon_command(&context, RibbonAction::InsertImaginary);
        app.ribbon_command(&context, RibbonAction::Evaluate);
        let id = app.selected.unwrap();
        let quantity = app.results[&id].value.as_ref().unwrap().numeric_json();
        assert_eq!(quantity["real"], 0.);
        assert_eq!(quantity["imag"], 1.);
    }
    #[test]
    fn tall_matrix_extends_the_scrollable_document_to_its_measured_height() {
        let context = egui::Context::default();
        crate::configure(&context);
        let mut app = WorksheetApp::default();
        let id = app.insert((0., app.document.grid), false);
        app.document.regions[0].content = CanvasContent::Math(PortableTree {
            kind: NodeKind::Matrix {
                rows: 60,
                cols: 1,
                display: None,
            },
            children: (0..60)
                .map(|i| PortableTree {
                    kind: NodeKind::Number(i.to_string()),
                    children: vec![],
                })
                .collect(),
        });
        frame(&mut app, &context, vec![]);
        assert!(app.region_heights[&id] > app.page_geometry().body_height());
        assert!(app.paper_size().y > app.document.height as f32);
    }

    #[test]
    fn calculation_error_navigation_follows_page_order_and_wraps() {
        let mut app = WorksheetApp::default();
        let mut ids = Vec::new();
        for (top, source) in [(120., "missing_b="), (40., "missing_a=")] {
            let id = app.document.add_math(20., top);
            let mut editor = Editor::new();
            formula::input_text(&mut editor, source);
            app.document
                .regions
                .iter_mut()
                .find(|region| region.id == id)
                .unwrap()
                .content = CanvasContent::Math(editor.to_portable().unwrap());
            app.editors.insert(id, editor);
            ids.push(id);
        }
        app.recalculate();
        assert_eq!(app.error_region_ids(), vec![ids[1], ids[0]]);

        app.selected = None;
        app.navigate_error(true);
        assert_eq!(app.selected, Some(ids[1]));
        assert!(app.status.contains("missing_a"));
        app.navigate_error(true);
        assert_eq!(app.selected, Some(ids[0]));
        app.navigate_error(true);
        assert_eq!(app.selected, Some(ids[1]));
        app.navigate_error(false);
        assert_eq!(app.selected, Some(ids[0]));
        assert!(app.insertion.is_none());
    }

    #[test]
    fn explicit_native_recalculation_with_an_unsupported_region_keeps_independent_results() {
        let mut document = CanvasDocument::default();
        let global = document.add_opaque(0., 20.,
            "<region><math><ml:globalDefine><ml:id labels=\"VARIABLE\">a</ml:id><ml:real>7</ml:real></ml:globalDefine></math></region>", "math");
        let unsupported =
            document.add_opaque(0., 60., "<region><component/></region>", "component");
        let answer = add_math_tree(
            &mut document,
            0.,
            100.,
            binary_tree(
                BinaryOp::Evaluate,
                identifier("a"),
                tree(NodeKind::Placeholder, vec![]),
            ),
        );
        let path = std::env::temp_dir().join(format!(
            "mathcraft-open-results-{}.mathcraft",
            std::process::id()
        ));
        document.save(&path).unwrap();
        let mut app = WorksheetApp::default();
        app.open(&path);
        assert!(app.results.is_empty());
        assert!(app.last_executed_regions.is_empty());
        app.recalculate();
        std::fs::remove_file(&path).unwrap();
        assert!(
            app.calculation_error.is_none(),
            "{:?}",
            app.calculation_error
        );
        assert!(app.results[&global].error.is_none());
        assert!(app.results[&unsupported].error.is_some());
        assert_eq!(
            app.results[&answer].value.as_ref().unwrap().numeric_json()["real"],
            7.
        );
    }

    #[test]
    fn right_expansion_includes_long_formula_without_changing_page_or_calculation() {
        let context = egui::Context::default();
        crate::configure(&context);
        let mut app = WorksheetApp::default();
        let source = format!("{}=", "1234567890".repeat(15));
        let mut editor = Editor::new();
        formula::input_text(&mut editor, &source);
        let id = add_math_tree(&mut app.document, 500., 50., editor.to_portable().unwrap());
        app.editors.insert(id, editor);
        app.recalculate();
        let value = app.results[&id].value.as_ref().unwrap().numeric_json();
        frame(&mut app, &context, vec![]);
        let page_width = app.document.width;
        assert!(app.right_content_edge() > page_width as f32);
        assert_eq!(app.paper_size().x, page_width as f32);
        app.expand_right = true;
        assert!(app.paper_size().x > page_width as f32 + 500.);
        frame(&mut app, &context, vec![]);
        assert_eq!(app.document.width, page_width);
        assert_eq!(
            app.results[&id].value.as_ref().unwrap().numeric_json(),
            value
        );
        assert!(app.undo.is_empty());
    }

    #[test]
    fn idle_large_worksheet_only_instantiates_visible_editors() {
        let context = egui::Context::default();
        crate::configure(&context);
        let mut app = WorksheetApp::default();
        for i in 0..1000 {
            add_math_tree(
                &mut app.document,
                0.,
                i as f64 * 100.,
                binary_tree(
                    BinaryOp::Evaluate,
                    number(&i.to_string()),
                    tree(NodeKind::Placeholder, vec![]),
                ),
            );
        }
        app.recalculate();
        frame(&mut app, &context, vec![]);
        let start = std::time::Instant::now();
        for _ in 0..120 {
            frame(&mut app, &context, vec![]);
        }
        println!(
            "1000 regions, 120 idle headless frames: {:?}; instantiated editors: {}",
            start.elapsed(),
            app.editors.len()
        );
        assert!(
            app.editors.len() < 20,
            "Offscreen regions should not be typeset"
        );
        assert_eq!(app.results.len(), 1000);
        assert!(!app.dirty);
        assert!(app.undo.is_empty());
    }

    #[test]
    fn horizontal_offscreen_math_is_not_instantiated_and_selected_math_is_kept_live() {
        let context = egui::Context::default();
        crate::configure(&context);
        let mut app = WorksheetApp::default();
        let visible = add_math_tree(&mut app.document, 0., 20., number("1"));
        let hidden = add_math_tree(&mut app.document, 2000., 20., number("2"));
        frame(&mut app, &context, vec![]);
        assert!(app.editors.contains_key(&visible));
        assert!(!app.editors.contains_key(&hidden));
        app.selected = Some(hidden);
        frame(&mut app, &context, vec![]);
        assert!(app.editors.contains_key(&hidden));
        assert!(!app.dirty);
    }

    #[test]
    fn jumping_to_a_distant_page_does_not_typeset_every_unvisited_preceding_formula() {
        let context = egui::Context::default();
        crate::configure(&context);
        let mut app = WorksheetApp::default();
        for i in 0..1000 {
            add_math_tree(
                &mut app.document,
                0.,
                i as f64 * 100.,
                binary_tree(
                    BinaryOp::Evaluate,
                    number(&i.to_string()),
                    tree(NodeKind::Placeholder, vec![]),
                ),
            );
        }
        app.recalculate();
        frame(&mut app, &context, vec![]);
        let scroll = app.page_geometry().to_paper(0., 80000.).y;
        let output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1200., 900.))),
                ..Default::default()
            },
            |ui| {
                let mut page = ui.new_child(UiBuilder::new().max_rect(Rect::from_min_size(
                    pos2(0., -scroll),
                    vec2(1200., scroll + 900.),
                )));
                app.page_ui(&mut page);
            },
        );
        output.drop_without_applying_deltas();
        assert!(
            app.editors.len() < 40,
            "Only potentially intersecting formulas should be instantiated, got {}",
            app.editors.len()
        );
        assert!(app.visible_regions.contains(&app.document.regions[800].id));
        assert!(app.undo.is_empty());
        assert!(!app.dirty);
    }

    #[test]
    fn retained_display_refreshes_after_formatting_and_recalculation() {
        let mut app = WorksheetApp::default();
        let mut editor = Editor::new();
        formula::input_text(&mut editor, "1/3=");
        let id = add_math_tree(&mut app.document, 0., 0., editor.to_portable().unwrap());
        app.recalculate();
        let format = app.document.regions[0].format;
        let original = app.display_result(id, format).unwrap().clone();
        app.document.regions[0].format.precision = 1;
        let format = app.document.regions[0].format;
        let rounded = app.display_result(id, format).unwrap().clone();
        assert_ne!(original, rounded);
        let mut edited = Editor::new();
        formula::input_text(&mut edited, "2/3=");
        app.document.regions[0].content = CanvasContent::Math(edited.to_portable().unwrap());
        app.recalculate();
        let updated = app.display_result(id, format).unwrap().clone();
        assert_ne!(rounded, updated);
    }

    /// Repeatable CPU-side workload; excludes GPU, printer and window-system time.
    /// Run explicitly with --ignored --nocapture before/after performance changes.
    #[test]
    #[ignore]
    fn benchmark_saved_worksheet_idle_and_pointer_hold() {
        for count in [100, 1000, 4000] {
            let context = egui::Context::default();
            crate::configure(&context);
            let mut app = WorksheetApp::default();
            let matrix = crate::result_display::FormattedResult {
                rows: 12,
                cols: 12,
                cells: (0..144)
                    .map(|i| {
                        crate::result_display::DisplayMath::Text(
                            format!("{}.123456789", i),
                            typography::MathStyle::Operator,
                        )
                    })
                    .collect(),
                brackets: true,
            }
            .to_saved();
            for i in 0..count {
                let id = add_math_tree(
                    &mut app.document,
                    0.,
                    i as f64 * 100.,
                    binary_tree(
                        BinaryOp::Evaluate,
                        identifier(&format!("A{i}")),
                        tree(NodeKind::Placeholder, vec![]),
                    ),
                );
                app.saved_results.insert(
                    id,
                    SavedCalculationRow {
                        region_id: id,
                        status: SavedCalculationStatus::Synchronized,
                        display: Some(matrix.clone()),
                        error: None,
                        warnings: vec![],
                        symbolic_mathml: None,
                        numeric_source: None,
                    },
                );
            }
            workspace_frame(&mut app, &context, vec![]);
            for held in [false, true] {
                if held {
                    workspace_frame(
                        &mut app,
                        &context,
                        vec![
                            egui::Event::PointerMoved(pos2(1050., 700.)),
                            egui::Event::PointerButton {
                                pos: pos2(1050., 700.),
                                button: egui::PointerButton::Primary,
                                pressed: true,
                                modifiers: egui::Modifiers::NONE,
                            },
                        ],
                    );
                }
                let started = std::time::Instant::now();
                for _ in 0..120 {
                    workspace_frame(&mut app, &context, vec![]);
                }
                println!(
                    "PERF regions={count} matrix=12x12 frames=120 pointer_held={held} elapsed_us={}",
                    started.elapsed().as_micros()
                );
            }
            assert!(app.undo.is_empty());
            assert!(!app.dirty);
            assert!(app.editors.len() < 30);
            assert!(app.last_executed_regions.is_empty());
        }
    }

    #[test]
    fn page_edge_button_switches_to_editable_draft_without_resizing_a4() {
        fn arrow(shape: &egui::Shape) -> Option<egui::Pos2> {
            match shape {
                egui::Shape::Text(text) if text.galley.text() == "▶" => {
                    Some(text.pos + text.galley.size() / 2.)
                }
                egui::Shape::Vec(shapes) => shapes.iter().find_map(arrow),
                _ => None,
            }
        }
        let context = egui::Context::default();
        crate::configure(&context);
        let mut app = WorksheetApp::default();
        let id = add_math_tree(
            &mut app.document,
            900.,
            50.,
            binary_tree(
                BinaryOp::Evaluate,
                number("7"),
                tree(NodeKind::Placeholder, vec![]),
            ),
        );
        app.recalculate();
        let before = app.document.clone();
        let output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1100., 900.))),
                ..Default::default()
            },
            |ui| app.workspace_ui(ui),
        );
        let button = output
            .shapes
            .iter()
            .find_map(|shape| arrow(&shape.shape))
            .unwrap();
        output.drop_without_applying_deltas();
        for pressed in [true, false] {
            let output = context.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1100., 900.))),
                    events: vec![
                        egui::Event::PointerMoved(button),
                        egui::Event::PointerButton {
                            pos: button,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                    ..Default::default()
                },
                |ui| app.workspace_ui(ui),
            );
            output.drop_without_applying_deltas();
        }
        assert!(app.expand_right);
        assert_eq!(app.page_geometry().margins, [0.; 4]);
        assert!(app.paper_size().x >= app.document.width as f32 * 2.);
        assert_eq!(app.document, before);
        assert_eq!(
            app.results[&id].value.as_ref().unwrap().numeric_json()["real"],
            7.
        );
        app.ribbon_command(&context, RibbonAction::SetDraftView(false));
        assert_eq!(app.paper_size().x, app.document.width as f32);
        assert_eq!(app.document, before);
        assert!(app.undo.is_empty());
    }

    #[test]
    fn unsupported_native_math_is_local_and_does_not_reuse_a_previous_definition() {
        let mut app = WorksheetApp::default();
        add_math_tree(
            &mut app.document,
            0.,
            20.,
            binary_tree(BinaryOp::Define, identifier("a"), number("2")),
        );
        let broken = app.document.add_opaque(0., 70.,
            "<region><math><ml:define unsupported-setting=\"true\"><ml:id labels=\"VARIABLE\">a</ml:id><ml:real>9</ml:real></ml:define></math></region>", "math");
        let dependent = add_math_tree(
            &mut app.document,
            0.,
            120.,
            binary_tree(
                BinaryOp::Evaluate,
                identifier("a"),
                tree(NodeKind::Placeholder, vec![]),
            ),
        );
        let independent = add_math_tree(
            &mut app.document,
            0.,
            170.,
            binary_tree(
                BinaryOp::Evaluate,
                number("8"),
                tree(NodeKind::Placeholder, vec![]),
            ),
        );
        app.recalculate();
        assert!(
            app.calculation_error.is_none(),
            "{:?}",
            app.calculation_error
        );
        assert!(app.results[&broken].error.is_some());
        assert!(app.results[&dependent].error.is_some());
        assert!(app.results[&dependent].value.is_none());
        assert_eq!(
            app.results[&independent]
                .value
                .as_ref()
                .unwrap()
                .numeric_json()["real"],
            8.
        );
    }

    #[test]
    fn manual_calculation_retains_pending_results_until_explicit_recalculation() {
        let context = egui::Context::default();
        crate::configure(&context);
        let mut app = WorksheetApp::default();
        let mut ids = Vec::new();
        for (top, source) in [(40., "a:=2"), (80., "a=")] {
            let id = app.document.add_math(20., top);
            let mut editor = Editor::new();
            formula::input_text(&mut editor, source);
            app.document
                .regions
                .iter_mut()
                .find(|region| region.id == id)
                .unwrap()
                .content = CanvasContent::Math(editor.to_portable().unwrap());
            app.editors.insert(id, editor);
            ids.push(id);
        }
        app.recalculate();
        assert_eq!(
            app.results[&ids[1]].value.as_ref().unwrap().numeric_json()["real"],
            2.
        );

        app.automatic_calculation = false;
        let editor = app.editors.get_mut(&ids[0]).unwrap();
        let rhs = editor.arena().get(editor.root()).unwrap().children()[1];
        editor.set_atom(rhs, NodeKind::Number("3".into())).unwrap();
        app.document.regions[0].content = CanvasContent::Math(editor.to_portable().unwrap());
        app.calculation_changed(false);
        assert!(app.calculation_pending);
        assert!(app.error_region_ids().is_empty());
        assert_eq!(
            app.results[&ids[1]].value.as_ref().unwrap().numeric_json()["real"],
            2.
        );

        app.ribbon_command(&context, RibbonAction::Recalculate);
        assert!(!app.calculation_pending);
        assert_eq!(
            app.results[&ids[1]].value.as_ref().unwrap().numeric_json()["real"],
            3.
        );

        app.automatic_calculation = false;
        let editor = app.editors.get_mut(&ids[0]).unwrap();
        editor.set_atom(rhs, NodeKind::Number("4".into())).unwrap();
        app.document.regions[0].content = CanvasContent::Math(editor.to_portable().unwrap());
        app.calculation_changed(false);
        app.ribbon_command(&context, RibbonAction::SetAutomaticCalculation(true));
        assert!(app.automatic_calculation);
        assert!(!app.calculation_pending);
        assert_eq!(
            app.results[&ids[1]].value.as_ref().unwrap().numeric_json()["real"],
            4.
        );
    }

    #[test]
    fn page_section_editor_commits_header_footer_as_one_document_undo() {
        let context = egui::Context::default();
        crate::configure(&context);
        let mut app = WorksheetApp::default();
        app.ribbon_command(&context, RibbonAction::EditPageSections);
        let draft = app.page_sections_draft.as_mut().unwrap();
        push_page_band_region(
            &mut draft.sections.header,
            &mut draft.next_id,
            PageBandContent::Text("Project Atlas".into()),
        );
        push_page_band_region(
            &mut draft.sections.footer,
            &mut draft.next_id,
            PageBandContent::Field(PageField::PageNumber(PageNumberFormat::PageNumberOfTotal)),
        );
        draft.sections.starting_page_number = 3;
        assert!(app.commit_page_sections_draft());
        assert!(app.document.page_sections.explicit);
        assert_eq!(app.document.page_sections.header.regions.len(), 1);
        assert_eq!(app.document.page_sections.footer.regions.len(), 1);
        assert_eq!(app.document.page_sections.starting_page_number, 3);
        assert_eq!(
            app.page_field_preview(
                PageField::PageNumber(PageNumberFormat::PageNumberOfTotal),
                1,
                3,
            ),
            "Page 4 of 5"
        );
        app.document.validate().unwrap();

        app.undo();
        assert!(!app.document.page_sections.explicit);
        assert!(app.document.page_sections.header.regions.is_empty());
        app.redo();
        assert_eq!(app.document.page_sections.header.regions.len(), 1);
        assert_eq!(app.document.page_sections.footer.regions.len(), 1);
    }

    #[test]
    fn redefinition_warning_is_separate_from_errors_and_can_be_hidden() {
        let context = egui::Context::default();
        crate::configure(&context);
        let mut app = WorksheetApp::default();
        let mut ids = Vec::new();
        for (top, source) in [(40., "a:=1"), (80., "a:=2")] {
            let id = app.document.add_math(20., top);
            let mut editor = Editor::new();
            formula::input_text(&mut editor, source);
            app.document
                .regions
                .iter_mut()
                .find(|region| region.id == id)
                .unwrap()
                .content = CanvasContent::Math(editor.to_portable().unwrap());
            ids.push(id);
        }
        app.recalculate();
        assert!(!app.results[&ids[0]].redefinition_warning);
        assert!(app.results[&ids[1]].redefinition_warning);
        assert!(app.results[&ids[1]].error.is_none());
        assert!(app.show_warnings);
        app.ribbon_command(&context, RibbonAction::ToggleWarnings);
        assert!(!app.show_warnings);
        assert!(app.results[&ids[1]].redefinition_warning);
    }

    fn calculation_command_fixture() -> (WorksheetApp, [u64; 3]) {
        let mut app = WorksheetApp::default();
        let mut ids = Vec::new();
        for (top, expression) in [(20., "a:=2"), (80., "a="), (2000., "a=")] {
            let mut editor = Editor::new();
            formula::input_text(&mut editor, expression);
            let id = app.document.add_math(20., top);
            app.document
                .regions
                .iter_mut()
                .find(|r| r.id == id)
                .unwrap()
                .content = CanvasContent::Math(editor.to_portable().unwrap());
            ids.push(id);
        }
        app.insertion = None;
        (app, ids.try_into().unwrap())
    }

    #[test]
    fn disabled_definition_is_excluded_from_all_calculation_scopes_and_can_be_reenabled() {
        let (mut app, ids) = calculation_command_fixture();
        app.recalculate();
        app.automatic_calculation = false;
        app.selected = Some(ids[0]);
        app.set_selected_calculation_disabled(true);
        assert!(app.document.regions[0].calculation_disabled);
        assert!(app.last_executed_regions.is_empty());
        assert!(app.pending_regions.contains(&ids[1]));
        assert!(!app.pending_regions.contains(&ids[0]));
        assert_eq!(
            app.saved_results[&ids[0]].status,
            SavedCalculationStatus::Pending
        );
        app.visible_regions.extend(ids);
        app.visible_calculation_end = Some(ids[2]);
        app.editing_region = Some(ids[0]);
        for command in [
            CalculationCommand::Expression(ids[0]),
            CalculationCommand::Now,
            CalculationCommand::Worksheet,
            CalculationCommand::Automatic,
        ] {
            app.calculate_command(command);
            assert!(!app.last_executed_regions.contains(&ids[0].to_string()));
            assert!(!app.results.contains_key(&ids[0]));
        }
        assert!(
            app.results[&ids[1]].error.is_some(),
            "the only disabled definition must not provide a binding"
        );
        assert!(app.results[&ids[2]].value.is_none());
        app.automatic_calculation = true;
        app.set_selected_calculation_disabled(false);
        assert_eq!(calculation_number(&app, ids[1]), 2.);
        assert_eq!(calculation_number(&app, ids[2]), 2.);
        assert!(!app.calculation_pending);
    }

    #[test]
    fn disabled_edits_after_reopening_keep_pending_saved_results_without_waking_a_cold_engine() {
        let (mut app, ids) = calculation_command_fixture();
        app.recalculate();
        app.selected = Some(ids[1]);
        app.set_selected_calculation_disabled(true);
        app.undo();
        assert!(!app.document.regions[1].calculation_disabled);
        assert_eq!(calculation_number(&app, ids[1]), 2.);
        app.redo();
        assert!(app.document.regions[1].calculation_disabled);
        app.persist_calculation_cache();
        let old_display = app.saved_results[&ids[1]].display.clone();
        let path = calculation_test_path("mathcraft");
        app.document.save(&path).unwrap();
        let mut opened = WorksheetApp::default();
        opened.open(&path);
        assert!(opened.document.regions[1].calculation_disabled);
        assert_eq!(
            opened.saved_results[&ids[1]].status,
            SavedCalculationStatus::Pending
        );
        assert!(!opened.pending_regions.contains(&ids[1]));
        assert!(!opened.calculation_pending);
        opened.selected = Some(ids[1]);
        opened.editing_region = Some(ids[1]);
        replace_calculation_definition(&mut opened, ids[1], "undefined_disabled_value=");
        opened.calculation_changed(true);
        assert!(opened.last_executed_regions.is_empty());
        assert!(
            opened.results.is_empty(),
            "editing a disabled item must not initialize the active engine"
        );
        assert_eq!(opened.saved_results[&ids[1]].display, old_display);
        opened.persist_calculation_cache();
        opened.document.save(&path).unwrap();
        opened.open(&path);
        std::fs::remove_file(path).unwrap();
        assert_eq!(opened.saved_results[&ids[1]].display, old_display);
        assert_eq!(
            opened.saved_results[&ids[1]].status,
            SavedCalculationStatus::Pending
        );
        opened.selected = Some(ids[1]);
        opened.set_selected_calculation_disabled(false);
        assert!(opened.results[&ids[1]].error.is_some());
        assert!(!opened.saved_results.contains_key(&ids[1]));
    }

    #[test]
    fn disabled_plot_preserves_points_but_never_runs_or_invalidates_other_regions_when_edited() {
        let (mut app, ids) = calculation_command_fixture();
        let plot = app.document.add_plot(0., 150.);
        app.recalculate();
        let old_plot = app.plot_results[&plot].clone();
        app.selected = Some(plot);
        app.set_selected_calculation_disabled(true);
        if let CanvasContent::Plot(spec) = &mut app
            .document
            .regions
            .iter_mut()
            .find(|r| r.id == plot)
            .unwrap()
            .content
        {
            *spec.primary_y_expression_mut().unwrap() = "unresolved_disabled_plot(x)".into();
        }
        app.calculation_changed(false);
        assert!(app.last_executed_regions.is_empty());
        assert_eq!(app.plot_results[&plot], old_plot);
        assert_eq!(calculation_number(&app, ids[2]), 2.);
        app.visible_calculation_end = Some(plot);
        app.visible_regions.insert(plot);
        app.editing_region = Some(plot);
        for command in [
            CalculationCommand::Now,
            CalculationCommand::Worksheet,
            CalculationCommand::Automatic,
            CalculationCommand::Expression(plot),
        ] {
            app.calculate_command(command);
            assert!(!app.last_executed_regions.contains(&plot.to_string()));
            assert_eq!(app.plot_results[&plot], old_plot);
        }
    }

    #[test]
    fn calculate_now_plot_before_math_uses_global_prerequisite_without_publishing_it() {
        let mut app = WorksheetApp::default();
        let plot = app.document.add_plot(0., 20.);
        let CanvasContent::Plot(spec) = &mut app.document.regions[0].content else {
            unreachable!()
        };
        *spec.primary_y_expression_mut().unwrap() = "g*x".into();
        let global = app.document.add_opaque(0., 2000.,
            "<region><math><ml:globalDefine><ml:id labels=\"VARIABLE\">g</ml:id><ml:real>3</ml:real></ml:globalDefine></math></region>", "math");
        app.restore_saved_calculations();
        app.visible_calculation_end = Some(plot);
        app.visible_regions.insert(plot);
        app.calculate_command(CalculationCommand::Now);
        assert!(app.calculation_error.is_none());
        let points = &app.plot_results[&plot]
            .as_ref()
            .unwrap()
            .first_curve()
            .unwrap()
            .points;
        assert_eq!(points.len(), 500);
        assert_eq!(points.first(), Some(&[-10., -30.]));
        assert_eq!(points.last(), Some(&[10., 30.]));
        assert!(!app.pending_regions.contains(&plot));
        assert!(app.pending_regions.contains(&global));
        assert!(app.results[&global].value.is_none());
        assert!(app.last_executed_regions.contains(&global.to_string()));
        app.calculate_command(CalculationCommand::Now);
        assert!(app.last_executed_regions.is_empty());
    }

    #[test]
    fn global_prerequisite_cache_is_retained_pending_then_removed_when_published() {
        for fail in [false, true] {
            let mut app = WorksheetApp::default();
            let mut editor = Editor::new();
            formula::input_text(&mut editor, "a=");
            let answer = app.document.add_math(0., 20.);
            app.document.regions[0].content = CanvasContent::Math(editor.to_portable().unwrap());
            let rhs = if fail {
                "<ml:unsupported/>"
            } else {
                "<ml:real>7</ml:real>"
            };
            let global = app.document.add_opaque(0., 2000., format!(
                "<region><math><ml:globalDefine><ml:id labels=\"VARIABLE\">a</ml:id>{rhs}</ml:globalDefine></math></region>"), "math");
            let display = crate::result_display::format(
                &Value::Scalar {
                    quantity: mathcraft_core::Quantity::scalar(7.),
                },
                &MathFormat::default(),
            )
            .to_saved();
            app.document.calculation_cache = Some(SavedCalculationCache {
                source_signature: app.document.calculation_signature(),
                rows: vec![
                    SavedCalculationRow {
                        region_id: answer,
                        status: SavedCalculationStatus::Pending,
                        display: None,
                        error: None,
                        warnings: vec![],
                        symbolic_mathml: None,
                        numeric_source: None,
                    },
                    SavedCalculationRow {
                        region_id: global,
                        status: SavedCalculationStatus::Synchronized,
                        display: Some(display.clone()),
                        error: None,
                        warnings: vec![],
                        symbolic_mathml: None,
                        numeric_source: None,
                    },
                ],
            });
            app.restore_saved_calculations();
            app.visible_calculation_end = Some(answer);
            app.visible_regions.insert(answer);
            app.calculate_command(CalculationCommand::Now);
            if !fail {
                assert_eq!(calculation_number(&app, answer), 7.);
            }
            assert!(app.last_executed_regions.contains(&global.to_string()));
            assert!(app.pending_regions.contains(&global));
            assert_eq!(
                app.saved_results[&global].display.as_deref(),
                Some(display.as_str())
            );
            assert!(
                app.results[&global].value.is_none(),
                "later global display was not published as fresh"
            );

            app.visible_calculation_end = Some(global);
            app.visible_regions.insert(global);
            app.calculate_command(CalculationCommand::Now);
            assert!(!app.pending_regions.contains(&global));
            assert!(
                !app.saved_results.contains_key(&global),
                "published global must replace the old presentation even when not reexecuted"
            );
            assert_eq!(app.results[&global].error.is_some(), fail);
        }
    }

    #[test]
    fn malformed_last_visible_native_item_cannot_abort_earlier_calculations() {
        for kind in ["math", "solveblock"] {
            let (mut app, ids) = calculation_command_fixture();
            app.automatic_calculation = false;
            let malformed = app.document.add_opaque(0., 120., "<region>", kind);
            app.visible_regions.extend([ids[0], ids[1], malformed]);
            app.visible_calculation_end = Some(malformed);
            app.calculate_command(CalculationCommand::Now);
            assert!(
                app.calculation_error.is_none(),
                "{:?}",
                app.calculation_error
            );
            assert_eq!(calculation_number(&app, ids[1]), 2.);
            assert!(app.results[&malformed].error.is_some());
            assert!(app.pending_regions.contains(&ids[2]));
        }
    }

    fn calculation_test_path(extension: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "mathcraft-calculation-{}-{}.{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            extension,
        ))
    }

    #[test]
    fn invalid_saved_presentation_cannot_silently_become_a_synchronized_blank() {
        let (mut app, ids) = calculation_command_fixture();
        app.recalculate();
        app.persist_calculation_cache();
        app.document.calculation_cache.as_mut().unwrap().rows[1].display =
            Some("not a supported saved presentation".into());
        app.restore_saved_calculations();
        assert!(app.results.is_empty());
        assert!(app.last_executed_regions.is_empty());
        assert!(app.pending_regions.contains(&ids[1]));
        assert_eq!(
            app.saved_results[&ids[1]].status,
            SavedCalculationStatus::Pending
        );
    }

    fn replace_calculation_definition(app: &mut WorksheetApp, id: u64, text: &str) {
        let mut editor = Editor::new();
        formula::input_text(&mut editor, text);
        app.editors.remove(&id);
        app.document
            .regions
            .iter_mut()
            .find(|r| r.id == id)
            .unwrap()
            .content = CanvasContent::Math(editor.to_portable().unwrap());
    }

    fn calculation_number(app: &WorksheetApp, id: u64) -> f64 {
        app.results[&id].value.as_ref().unwrap().numeric_json()["real"]
            .as_f64()
            .unwrap()
    }

    #[test]
    fn opening_saved_results_and_idle_frames_never_execute_worksheet_input() {
        let (mut app, ids) = calculation_command_fixture();
        app.recalculate();
        app.persist_calculation_cache();
        let expected = app
            .document
            .calculation_cache
            .as_ref()
            .unwrap()
            .rows
            .clone();
        let path = calculation_test_path("mathcraft");
        app.document.save(&path).unwrap();
        let mut opened = WorksheetApp::default();
        opened.open(&path);
        std::fs::remove_file(path).unwrap();
        assert!(
            opened.results.is_empty(),
            "saved values must not become live engine values"
        );
        assert!(opened.last_executed_regions.is_empty());
        assert!(!opened.calculation_pending);
        assert_eq!(opened.saved_results[&ids[1]], expected[1]);
        let context = egui::Context::default();
        crate::configure(&context);
        frame(&mut opened, &context, vec![]);
        assert!(opened.last_executed_regions.is_empty());
        assert!(opened.results.is_empty());
        assert!(
            crate::result_display::FormattedResult::from_saved(
                opened.saved_results[&ids[1]].display.as_deref().unwrap()
            )
            .is_some()
        );
        opened.editing_region = None;
        opened.calculate_command(CalculationCommand::Now);
        assert!(
            opened.last_executed_regions.is_empty(),
            "clean saved regions do not need Calculate Now"
        );
        assert!(!opened.calculation_pending);
    }

    #[test]
    fn saved_date_uses_imported_path_time_retains_it_without_file_io_and_updates_on_successful_save()
     {
        use std::time::{Duration, UNIX_EPOCH};
        let old = UNIX_EPOCH + Duration::from_secs(951_825_600);
        let expected = crate::page_fields::SavedDate::from_timestamp(old)
            .unwrap()
            .format(DateSavedFormat::YearMonthDay);
        for native in [false, true] {
            let path = if native {
                native_saved_result_fixture("<ml:real>2</ml:real>", "Synchronized", false)
            } else {
                let path = calculation_test_path("mathcraft");
                CanvasDocument::default().save(&path).unwrap();
                path
            };
            std::fs::OpenOptions::new()
                .write(true)
                .open(&path)
                .unwrap()
                .set_times(std::fs::FileTimes::new().set_modified(old))
                .unwrap();
            let mut app = WorksheetApp::default();
            app.open(&path);
            assert_eq!(app.path.as_deref(), Some(path.as_path()));
            assert_eq!(app.native_save_target.is_some(), native);
            let field = PageField::DateSaved(DateSavedFormat::YearMonthDay);
            assert_eq!(app.page_field_preview(field, 0, 1), expected);
            std::fs::remove_file(&path).unwrap();
            let context = egui::Context::default();
            crate::configure(&context);
            frame(&mut app, &context, vec![]);
            assert_eq!(app.page_field_preview(field, 0, 1), expected);
            if !native {
                assert!(app.save(false));
                assert_ne!(app.page_field_preview(field, 0, 1), expected);
                let current = crate::page_fields::SavedDate::from_path(&path)
                    .unwrap()
                    .format(DateSavedFormat::YearMonthDay);
                assert_eq!(app.page_field_preview(field, 0, 1), current);
                std::fs::remove_file(&path).unwrap();
            }
        }
    }

    #[test]
    fn missing_or_source_mismatched_cache_waits_for_explicit_calculation() {
        let (mut app, ids) = calculation_command_fixture();
        app.recalculate();
        app.persist_calculation_cache();
        replace_calculation_definition(&mut app, ids[0], "a:=7");
        let path = calculation_test_path("mathcraft");
        app.document.save(&path).unwrap();
        let mut opened = WorksheetApp::default();
        opened.open(&path);
        assert!(opened.results.is_empty());
        assert!(
            opened.saved_results.is_empty(),
            "mismatched cache must not be presented"
        );
        assert!(opened.calculation_pending);
        assert!(opened.last_executed_regions.is_empty());
        opened.recalculate();
        assert_eq!(calculation_number(&opened, ids[1]), 7.);
        opened.document.calculation_cache = None;
        opened.document.save(&path).unwrap();
        opened.open(&path);
        std::fs::remove_file(path).unwrap();
        assert!(opened.results.is_empty());
        assert!(opened.calculation_pending);
        assert!(opened.last_executed_regions.is_empty());
    }

    #[test]
    fn calculate_now_updates_dirty_visible_prefix_and_retains_pending_tail() {
        let (mut app, ids) = calculation_command_fixture();
        app.recalculate();
        app.automatic_calculation = false;
        replace_calculation_definition(&mut app, ids[0], "a:=3");
        app.calculation_changed(false);
        app.visible_calculation_end = Some(ids[1]);
        app.visible_regions.extend([ids[0], ids[1]]);
        app.calculate_command(CalculationCommand::Now);
        assert_eq!(calculation_number(&app, ids[1]), 3.);
        assert_eq!(calculation_number(&app, ids[2]), 2.);
        assert!(!app.pending_regions.contains(&ids[1]));
        assert!(app.pending_regions.contains(&ids[2]));
        assert!(app.calculation_pending);
        assert_eq!(
            app.last_executed_regions,
            ids[..2].iter().map(u64::to_string).collect::<Vec<_>>()
        );
        app.calculate_command(CalculationCommand::Now);
        assert!(
            app.last_executed_regions.is_empty(),
            "clean prefix was run twice"
        );
        assert!(app.pending_regions.contains(&ids[2]));
        assert!(!app.automatic_calculation);
        app.recalculate();
        assert_eq!(calculation_number(&app, ids[2]), 3.);
        assert!(!app.calculation_pending);
        assert_eq!(app.last_executed_regions.len(), 3);
        assert!(!app.automatic_calculation);
    }

    #[test]
    fn now_forces_visible_active_item_then_calculates_visible_dirty_prefix() {
        let (mut app, ids) = calculation_command_fixture();
        app.recalculate();
        app.visible_calculation_end = Some(ids[1]);
        app.visible_regions.extend([ids[0], ids[1]]);
        app.editing_region = Some(ids[0]);
        app.calculate_command(CalculationCommand::Now);
        assert!(app.last_executed_regions.contains(&ids[0].to_string()));
        assert!(app.last_executed_regions.contains(&ids[1].to_string()));
        assert!(!app.last_executed_regions.contains(&ids[2].to_string()));
        assert!(app.pending_regions.contains(&ids[2]));
        assert_eq!(
            app.last_executed_regions
                .iter()
                .filter(|id| *id == &ids[0].to_string())
                .count(),
            1
        );

        app.recalculate();
        app.editing_region = Some(ids[2]);
        app.calculate_command(CalculationCommand::Now);
        assert!(
            app.last_executed_regions.is_empty(),
            "offscreen active item must not be forced"
        );
    }

    #[test]
    fn calculate_now_without_visible_calculatable_is_a_noop() {
        let (mut app, ids) = calculation_command_fixture();
        app.calculation_changed(false);
        app.visible_calculation_end = None;
        app.last_executed_regions.clear();
        app.calculate_command(CalculationCommand::Now);
        assert!(app.last_executed_regions.is_empty());
        // A full worksheet request still reaches the last region without a view.
        app.recalculate();
        assert_eq!(calculation_number(&app, ids[2]), 2.);
    }

    #[test]
    fn pending_saved_results_survive_save_open_without_becoming_fresh() {
        let (mut app, ids) = calculation_command_fixture();
        app.recalculate();
        app.automatic_calculation = false;
        replace_calculation_definition(&mut app, ids[0], "a:=9");
        app.calculation_changed(false);
        app.persist_calculation_cache();
        let path = calculation_test_path("mathcraft");
        app.document.save(&path).unwrap();
        let mut opened = WorksheetApp::default();
        opened.open(&path);
        std::fs::remove_file(path).unwrap();
        assert!(opened.last_executed_regions.is_empty());
        assert_eq!(
            opened.saved_results[&ids[1]].status,
            SavedCalculationStatus::Pending
        );
        assert!(opened.pending_regions.contains(&ids[1]));
        let expected = crate::result_display::format(
            &Value::Scalar {
                quantity: mathcraft_core::Quantity::scalar(2.),
            },
            &MathFormat::default(),
        );
        assert_eq!(
            crate::result_display::FormattedResult::from_saved(
                opened.saved_results[&ids[1]].display.as_deref().unwrap()
            ),
            Some(expected)
        );
        opened.recalculate();
        assert_eq!(calculation_number(&opened, ids[1]), 9.);
        assert!(!opened.calculation_pending);
    }

    #[test]
    fn calculation_shortcuts_use_f5_and_control_f5_with_distinct_scopes() {
        let context = egui::Context::default();
        crate::configure(&context);
        let (mut app, ids) = calculation_command_fixture();
        app.recalculate();
        app.automatic_calculation = false;
        replace_calculation_definition(&mut app, ids[0], "a:=4");
        app.calculation_changed(false);
        app.visible_calculation_end = Some(ids[1]);
        app.visible_regions.extend([ids[0], ids[1]]);
        let mut output = context.run_ui(
            egui::RawInput {
                events: vec![key(egui::Key::F5, egui::Modifiers::NONE)],
                ..Default::default()
            },
            |ui| app.global_editing_shortcuts(ui.ctx()),
        );
        output.textures_delta.clear();
        assert_eq!(calculation_number(&app, ids[1]), 4.);
        assert_eq!(calculation_number(&app, ids[2]), 2.);
        let mut output = context.run_ui(
            egui::RawInput {
                events: vec![key(egui::Key::F5, egui::Modifiers::CTRL)],
                ..Default::default()
            },
            |ui| app.global_editing_shortcuts(ui.ctx()),
        );
        output.textures_delta.clear();
        assert_eq!(calculation_number(&app, ids[2]), 4.);
        assert!(!app.calculation_pending);
        assert!(!app.automatic_calculation);
    }

    #[test]
    fn headless_view_tracks_only_intersecting_calculatable_regions() {
        let context = egui::Context::default();
        crate::configure(&context);
        let (mut app, ids) = calculation_command_fixture();
        let mut editor = Editor::new();
        formula::input_text(&mut editor, "99=");
        let outside = app.document.add_math(app.document.width + 20., 200.);
        app.document
            .regions
            .iter_mut()
            .find(|r| r.id == outside)
            .unwrap()
            .content = CanvasContent::Math(editor.to_portable().unwrap());
        frame(&mut app, &context, vec![]);
        assert_eq!(app.visible_calculation_end, Some(ids[1]));
        assert!(app.visible_regions.contains(&ids[0]));
        assert!(app.visible_regions.contains(&ids[1]));
        assert!(!app.visible_regions.contains(&ids[2]));
        assert!(!app.visible_regions.contains(&outside));
    }

    fn native_saved_result_fixture(result: &str, status: &str, override_unit: bool) -> PathBuf {
        native_saved_result_fixture_with_disabled(result, status, override_unit, false)
    }

    fn native_saved_result_fixture_with_disabled(
        result: &str,
        status: &str,
        override_unit: bool,
        disabled: bool,
    ) -> PathBuf {
        use std::io::Write;
        let path = calculation_test_path("mcdx");
        let unit = if override_unit {
            "<ml:id labels=\"UNIT\">cm</ml:id>"
        } else {
            "<ml:placeholder/>"
        };
        // Independent fixture: source is undefined so evaluation would fail.
        let disabled_attr = if disabled {
            "disable-calc=\"true\" "
        } else {
            ""
        };
        let worksheet = format!(
            r#"<worksheet xmlns="http://schemas.mathsoft.com/worksheet50" xmlns:ml="http://schemas.mathsoft.com/math50"><regions><region region-id="42" left="0" top="20"><math {disabled_attr}resultRef="900"><ml:eval><ml:id labels="VARIABLE">unavailable</ml:id><ml:unitOverride>{unit}</ml:unitOverride></ml:eval></math></region></regions></worksheet>"#
        );
        let results = format!(
            r#"<resultsList xmlns="http://schemas.mathsoft.com/result10" xmlns:ml="http://schemas.mathsoft.com/math50" xmlns:u="http://schemas.mathsoft.com/units10"><resultData result-id="900" calculation-status="{status}">{result}</resultData></resultsList>"#
        );
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
        for (name, bytes) in [
            ("mathcad/worksheet.xml", worksheet),
            ("mathcad/results.xml", results),
        ] {
            zip.start_file(name, zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(bytes.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
        path
    }

    #[test]
    fn native_saved_numeric_and_symbolic_results_are_restored_without_running_input() {
        for result in [
            "<ml:result><ml:real>12.5</ml:real></ml:result>",
            "<ml:symResult><ml:apply><ml:pow/><ml:id labels=\"VARIABLE\">x</ml:id><ml:real>2</ml:real></ml:apply></ml:symResult>",
        ] {
            let path = native_saved_result_fixture(result, "Synchronized", false);
            let mut app = WorksheetApp::default();
            app.open(&path);
            std::fs::remove_file(path).unwrap();
            assert_eq!(app.document.regions.len(), 1, "{}", app.status);
            let id = app.document.regions[0].id;
            assert!(app.last_executed_regions.is_empty());
            assert!(app.results.is_empty());
            assert_eq!(
                app.saved_results[&id].status,
                SavedCalculationStatus::Synchronized
            );
            assert!(
                crate::result_display::FormattedResult::from_saved(
                    app.saved_results[&id].display.as_deref().unwrap()
                )
                .is_some()
            );
            app.recalculate();
            assert!(
                app.results[&id].error.is_some(),
                "undefined source must fail only after explicit calculation"
            );
            assert!(!app.saved_results.contains_key(&id));
        }
    }

    #[test]
    fn imported_native_disabled_math_keeps_its_saved_result_pending_and_never_executes() {
        let disabled = native_saved_result_fixture_with_disabled(
            "<ml:result><ml:real>12.5</ml:real></ml:result>",
            "Synchronized",
            false,
            true,
        );
        let mut app = WorksheetApp::default();
        app.open(&disabled);
        std::fs::remove_file(disabled).unwrap();
        assert_eq!(app.document.regions.len(), 1, "{}", app.status);
        let region = &app.document.regions[0];
        assert!(region.calculation_disabled);
        let id = region.id;
        assert_eq!(
            app.saved_results[&id].status,
            SavedCalculationStatus::Pending
        );
        assert!(app.saved_results[&id].display.is_some());
        assert!(!app.calculation_pending);
        app.recalculate();
        assert!(app.results.is_empty());
        assert!(app.last_executed_regions.is_empty());
        assert!(app.error_region_ids().is_empty());
        assert_eq!(
            app.saved_results[&id].status,
            SavedCalculationStatus::Pending
        );
    }

    #[test]
    fn native_pending_and_empty_results_do_not_turn_into_zero_or_fresh_answers() {
        for (payload, status) in [
            ("<ml:result><ml:real>6</ml:real></ml:result>", "Pending"),
            ("<ml:result/>", "Pending"),
            ("<ml:result/>", "Synchronized"),
        ] {
            let path = native_saved_result_fixture(payload, status, false);
            let mut app = WorksheetApp::default();
            app.open(&path);
            std::fs::remove_file(path).unwrap();
            let id = app.document.regions[0].id;
            assert!(app.results.is_empty());
            assert!(app.pending_regions.contains(&id));
            assert_eq!(
                app.saved_results[&id].status,
                SavedCalculationStatus::Pending
            );
            if payload == "<ml:result/>" {
                assert!(app.saved_results[&id].display.is_none());
            }
        }
    }

    #[test]
    fn native_explicit_unit_slot_does_not_repeat_the_cached_unit_glyph() {
        let path = native_saved_result_fixture(
            "<ml:result><ml:unitedValue><ml:real>200</ml:real><u:unitMonomial><u:unitReference unit=\"centimeter\" power-numerator=\"1\" power-denominator=\"1\"/></u:unitMonomial></ml:unitedValue></ml:result>",
            "Synchronized",
            true,
        );
        let mut app = WorksheetApp::default();
        app.open(&path);
        std::fs::remove_file(path).unwrap();
        let id = app.document.regions[0].id;
        let display = crate::result_display::FormattedResult::from_saved(
            app.saved_results[&id].display.as_deref().unwrap(),
        )
        .unwrap();
        let expected = crate::result_display::format(
            &Value::Scalar {
                quantity: mathcraft_core::Quantity::scalar(200.),
            },
            &MathFormat::default(),
        );
        assert_eq!(
            display, expected,
            "the editable cm RHS supplies the unit glyph"
        );
    }
}
#[path = "worksheet_ui/area_calculation_input.rs"]
mod area_calculation_input;
#[path = "worksheet_ui/area_presentation.rs"]
mod area_presentation;

#[path = "worksheet_ui/area_cache.rs"]
mod area_cache;
mod area_print;
#[path = "worksheet_ui/area_transactions.rs"]
mod area_transactions;

#[cfg(test)]
mod area_performance_audit;

#[cfg(test)]
mod area_native_transactions;
#[path = "worksheet_ui/area_ui.rs"]
mod area_ui;

mod area_authoring;

mod area_insertion;
mod background;
#[cfg(test)]
mod command_transactions;
#[cfg(test)]
mod drag_transactions;
mod edit_commit;
#[cfg(test)]
mod global_vector_transactions;

#[cfg(test)]
mod full_app_profile;
mod inert_string_result;
mod inline_plot_ui;
#[cfg(test)]
mod lcc_workspace_profile;
mod native_plot_source;
mod native_plot_warnings;
mod numeric_cache;
mod numeric_entry_mode;
mod original_shortcuts;
mod plot_input_queue;
mod plot_lifecycle;
mod plot_restore_context;
mod plot_slot_lifecycle;
mod print_ui;
#[cfg(test)]
mod render_quality;
#[cfg(test)]
mod result_formatting;
#[cfg(test)]
mod scalar_notation_transactions;
mod unit_input;

#[cfg(test)]
mod array_slice_transactions;
#[cfg(test)]
mod deletion_transactions;

#[cfg(test)]
#[path = "worksheet_ui/editing_repair_transactions.rs"]
mod editing_repair_transactions;
#[cfg(test)]
mod infix_entry_transactions;
mod input_intent;
#[cfg(test)]
mod label_cycle_transactions;
#[cfg(test)]
mod local_function_transactions;
#[cfg(test)]
mod matrix_authoring_transactions;
#[cfg(test)]
mod parenthesis_transactions;
#[cfg(test)]
mod program_control_transactions;
#[cfg(test)]
#[path = "worksheet_ui/replacement_transactions.rs"]
mod replacement_transactions;
#[cfg(test)]
mod scaling_transactions;
mod spec_table_ui;

#[cfg(test)]
mod native_saved_result_probe {
    use super::*;

    #[test]
    #[ignore = "Reads an explicitly selected local worksheet; no original runtime or GUI"]
    fn inspect_selected_native_saved_presentations_without_calculation() {
        let path =
            std::env::var_os("MATHCRAFT_NATIVE_TEST_FILE").expect("Set MATHCRAFT_NATIVE_TEST_FILE");
        let mut app = WorksheetApp::default();
        app.open(Path::new(&path));
        assert!(app.document.native_archive.is_some(), "{}", app.status);
        assert!(app.last_executed_regions.is_empty());
        assert!(app.results.is_empty());
        let mut restored = 0usize;
        let mut pending = Vec::new();
        for region in app.document.regions.clone() {
            if !expects_display_result(&region) {
                continue;
            }
            if app.display_result(region.id, region.format).is_some() {
                restored += 1;
            } else if app.pending_regions.contains(&region.id) {
                pending.push(region.source_region);
            }
        }
        println!(
            "{}",
            serde_json::json!({"regions":app.document.regions.len(),"restored_displays":restored,"pending_source_regions":pending,"executed":app.last_executed_regions.len()})
        );
    }
}

#[cfg(test)]
#[path = "worksheet_ui/area_lookup_cost.rs"]
mod area_lookup_cost;

#[path = "worksheet_ui/page_break_ui.rs"]
mod page_break_ui;

fn plot_trace_appearance_ui(
    ui: &mut egui::Ui,
    index: usize,
    label: &mut String,
    style: &mut PlotTraceStyle,
) {
    ui.label("범례");
    ui.text_edit_singleline(label);
    ui.end_row();
    ui.label("색");
    let mut color = Color32::from_rgba_unmultiplied(
        style.color[0],
        style.color[1],
        style.color[2],
        style.color[3],
    );
    if ui.color_edit_button_srgba(&mut color).changed() {
        style.color = color.to_array();
    }
    ui.end_row();
    ui.label("선");
    egui::ComboBox::from_id_salt(("plot-line", index))
        .selected_text(plot_line_name(style.line))
        .show_ui(ui, |ui| {
            for line in [
                PlotLineStyle::None,
                PlotLineStyle::Solid,
                PlotLineStyle::Dotted,
                PlotLineStyle::Dashed,
                PlotLineStyle::DashDot,
                PlotLineStyle::DashDotDot,
            ] {
                ui.selectable_value(&mut style.line, line, plot_line_name(line));
            }
        });
    ui.end_row();
    ui.label("선 굵기");
    ui.add(
        egui::DragValue::new(&mut style.line_width)
            .range(0.25..=9.)
            .speed(0.1),
    );
    ui.end_row();
    ui.label("표식");
    egui::ComboBox::from_id_salt(("plot-marker", index))
        .selected_text(plot_marker_name(style.marker))
        .show_ui(ui, |ui| {
            for marker in [
                PlotMarker::None,
                PlotMarker::Cross,
                PlotMarker::Plus,
                PlotMarker::Circle,
                PlotMarker::Square,
                PlotMarker::Diamond,
                PlotMarker::Point,
                PlotMarker::Star,
                PlotMarker::Triangle,
            ] {
                ui.selectable_value(&mut style.marker, marker, plot_marker_name(marker));
            }
        });
    ui.end_row();
    ui.label("표식 크기 / 간격");
    ui.horizontal(|ui| {
        ui.add(
            egui::DragValue::new(&mut style.marker_size)
                .range(0.25..=32.)
                .speed(0.25),
        );
        ui.add(egui::DragValue::new(&mut style.marker_frequency).range(1..=10_000));
        ui.checkbox(&mut style.marker_filled, "채움");
    });
    ui.end_row();
}
#[cfg(test)]
mod string_cache_tests;
#[cfg(test)]
#[path = "worksheet_ui/string_settings_tests.rs"]
mod string_settings_tests;

#[cfg(test)]
#[path = "worksheet_ui/plain_text_ime_tests.rs"]
mod plain_text_ime_tests;

mod authored_text_input;
#[cfg(test)]
mod authored_text_ui_tests;
mod native_save_ui;
mod paragraph_alignment_ui;
#[path = "worksheet_ui/plain_text_input.rs"]
mod plain_text_input;
mod rich_text_edit_map;
mod rich_text_input;
#[cfg(test)]
mod rich_text_integration_tests;
mod rich_text_layout;
mod text_creation_seed;

#[cfg(all(test, target_os = "windows"))]
mod offscreen_wgpu_probe;

#[cfg(test)]
mod matrix_native_units_tests;

mod matrix_result_view;
#[cfg(test)]
mod matrix_result_view_app_tests;

mod matrix_region_clipboard;

mod matrix_indices_input;

#[cfg(test)]
mod source_matrix_display_app_tests;
