//! Shared native/Web egui chrome; existing source-admitted command groups remain available.
#[cfg(test)]
#[path = "matrix_indices_ribbon_tests.rs"]
mod matrix_indices_ribbon_tests;
mod modern;
#[cfg(test)]
#[path = "text_format_tests.rs"]
mod text_format_tests;
use eframe::egui::{
    self, Align, Color32, FontId, Layout, Pos2, Rect, Response, Sense, Stroke, Ui, Vec2,
};
use mathcraft_document::canvas::{ComplexFormat, MathFormat};

const MUTED: Color32 = Color32::from_rgb(143, 148, 153);
const PANEL: Color32 = Color32::from_rgb(240, 244, 247);
const GROUP_HEIGHT: f32 = 85.;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum RibbonTab {
    #[default]
    Math,
    InputOutput,
    Functions,
    Matrices,
    Plots,
    MathFormatting,
    TextFormatting,
    Calculation,
    Document,
    Resources,
}

impl RibbonTab {
    const ALL: [Self; 10] = [
        Self::Math,
        Self::InputOutput,
        Self::Functions,
        Self::Matrices,
        Self::Plots,
        Self::MathFormatting,
        Self::TextFormatting,
        Self::Calculation,
        Self::Document,
        Self::Resources,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Math => "수학",
            Self::InputOutput => "입력/출력",
            Self::Functions => "함수",
            Self::Matrices => "행렬/표",
            Self::Plots => "도표",
            Self::MathFormatting => "계산 서식 지정",
            Self::TextFormatting => "텍스트 서식 지정",
            Self::Calculation => "계산",
            Self::Document => "문서",
            Self::Resources => "리소스",
        }
    }
}

#[derive(Default)]
pub(crate) struct RibbonState {
    pub tab: RibbonTab,
    pub minimized: bool,
    text_format: TextFormatDraft,
    pub tools_open: bool,
    pub inspector_open: bool,
    pub help_requested: bool,
    pub help_focus: bool,
    pub help_query: String,
    command_states: Vec<Option<&'static str>>,
    pub input_ids: Vec<egui::Id>,
}

/// Source-admitted nonempty selection only. This is not a default/typing format.
#[derive(Clone, Copy, Debug)]
pub(crate) struct TextFormatContext {
    pub enabled: bool,
    /// Controller source/selection epoch, checked again before applying actions.
    pub selection_key: u64,
    pub retained_bold: bool,
    pub retained_italic: bool,
    pub retained_underline: bool,
    /// Keyboard toggle requires known current None/Underline on every Run.
    pub underline_enabled: bool,
    /// Explicit toolbar setter may assign an admitted nonempty selection even
    /// when its prior decoration is absent/unknown. Not a current-value query.
    pub underline_set_enabled: bool,
    pub font_points: Option<f64>,
    pub color_rgba: Option<[u8; 4]>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct MatrixIndicesContext {
    pub owner: u64,
    pub show_indices: bool,
}

#[derive(Default)]
struct TextFormatDraft {
    selection_key: Option<u64>,
    source_points: Option<f64>,
    source_color: Option<[u8; 4]>,
    size: String,
    last_valid_size: String,
    color: [u8; 4],
    last_used_color: Option<[u8; 4]>,
    color_dirty: bool,
}

fn font_points_text(value: Option<f64>) -> String {
    value
        .filter(|value| value.is_finite())
        .map_or_else(String::new, |value| {
            if value.fract() == 0. {
                format!("{value:.0}")
            } else {
                format!("{value:.1}")
            }
        })
}

/// Invariant decimal first slice; original culture-specific input is separate.
fn parse_text_font_points(value: &str) -> Option<f64> {
    let (whole, fraction) = value
        .split_once('.')
        .map_or((value, None), |(a, b)| (a, Some(b)));
    if whole.is_empty()
        || !whole.bytes().all(|byte| byte.is_ascii_digit())
        || fraction
            .is_some_and(|part| part.len() > 1 || !part.bytes().all(|byte| byte.is_ascii_digit()))
    {
        return None;
    }
    value
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite() && (1. ..=1638.).contains(value))
}

impl TextFormatDraft {
    fn sync(&mut self, context: Option<TextFormatContext>) {
        let Some(context) = context.filter(|context| context.enabled) else {
            self.selection_key = None;
            self.size.clear();
            self.last_valid_size.clear();
            self.color_dirty = false;
            return;
        };
        if self.selection_key != Some(context.selection_key)
            || self.source_points != context.font_points
            || self.source_color != context.color_rgba
        {
            self.selection_key = Some(context.selection_key);
            self.source_points = context.font_points;
            self.source_color = context.color_rgba;
            self.size = font_points_text(context.font_points);
            self.last_valid_size = self.size.clone();
            self.color = context
                .color_rgba
                .or(self.last_used_color)
                .unwrap_or([0, 0, 0, 255]);
            self.color_dirty = false;
        }
    }

    fn commit_size(&mut self) -> Option<RibbonAction> {
        let selection_key = self.selection_key?;
        if self.size == self.last_valid_size {
            return None;
        }
        let Some(points) = parse_text_font_points(&self.size) else {
            self.size = self.last_valid_size.clone();
            return None;
        };
        self.last_valid_size = self.size.clone();
        (self.source_points != Some(points)).then_some(RibbonAction::SetTextFontSize {
            points,
            selection_key,
        })
    }

    fn commit_color(&mut self) -> Option<RibbonAction> {
        let selection_key = self.selection_key?;
        if !self.color_dirty {
            return None;
        }
        self.color_dirty = false;
        self.last_used_color = Some(self.color);
        (self.source_color != Some(self.color)).then_some(RibbonAction::SetTextColor {
            rgba: self.color,
            selection_key,
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ParagraphAlignmentContext {
    pub selection_key: u64,
    pub current: Option<mathcraft_document::rich_text_source::ParagraphAlignment>,
}

pub(crate) struct RibbonContext<'a> {
    pub document_name: &'a str,
    pub can_undo: bool,
    pub can_redo: bool,
    pub has_selection: bool,
    pub can_insert_area: bool,
    pub can_page_break: bool,
    pub can_remove_page_break_above: bool,
    pub can_remove_page_break_below: bool,
    pub zoom: f32,
    pub grid: bool,
    pub font_points: f32,
    pub precision: u8,
    pub result_format: u8,
    pub number_format: MathFormat,
    pub matrix_indices: Option<MatrixIndicesContext>,
    pub has_math: bool,
    pub has_plot: bool,
    pub draft_view: bool,
    pub in_matrix: bool,
    pub in_table: bool,
    pub can_label: bool,
    pub has_errors: bool,
    pub automatic_calculation: bool,
    pub bin_oct_hex_mode: Option<bool>,
    pub calculation_running: bool,
    pub region_calculation_disabled: Option<bool>,
    pub show_warnings: bool,
    pub has_warnings: bool,
    pub text_format: Option<TextFormatContext>,
    pub paragraph_alignment: Option<ParagraphAlignmentContext>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum RibbonAction {
    New,
    Open,
    Save,
    SaveAs,
    ExportMcdx,
    Print,
    ExportPdf,
    PageSetup,
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    Delete,
    InsertMath,
    InsertText,
    InsertTable,
    InsertArea,
    InsertPageBreak,
    RemovePageBreakAbove,
    RemovePageBreakBelow,
    InsertPlot,
    EditPlot,
    InsertSolveBlock,
    Fraction,
    Power,
    Root,
    Absolute,
    Factorial,
    Percent,
    Degree,
    Polar,
    Transpose,
    Conjugate,
    Norm,
    RowOperator,
    ColumnOperator,
    Vectorize,
    CrossProduct,
    GlobalDefine,
    Matrix,
    Index,
    LiteralSubscript,
    Range,
    Function,
    Program,
    LocalDefine,
    If,
    ElseIf,
    AlsoIf,
    Else,
    ForLoop,
    WhileLoop,
    Return,
    Break,
    Continue,
    Try,
    Integral,
    Derivative,
    PartialDerivative,
    Summation,
    Product,
    SymbolicEvaluate,
    Define,
    Evaluate,
    InsertSymbol(&'static str),
    InsertUnit(&'static str),
    InsertImaginary,
    InsertIdentifier {
        name: &'static str,
        label: &'static str,
    },
    InsertFunction(&'static str),
    Recalculate,
    CalculateNow,
    StopAllCalculations,
    SetAutomaticCalculation(bool),
    SetBinOctHexMode(bool),
    SetRegionCalculationDisabled(bool),
    PreviousError,
    NextError,
    ToggleWarnings,
    Zoom(f32),
    ToggleGrid,
    SetDraftView(bool),
    EditPageSections,
    SetFontSize(f32),
    SetTextBold {
        value: bool,
        selection_key: u64,
    },
    SetParagraphAlignment {
        value: mathcraft_document::rich_text_source::ParagraphAlignment,
        selection_key: u64,
    },
    SetTextUnderline {
        value: bool,
        selection_key: u64,
    },
    SetTextItalic {
        value: bool,
        selection_key: u64,
    },
    SetTextFontSize {
        points: f64,
        selection_key: u64,
    },
    SetTextColor {
        rgba: [u8; 4],
        selection_key: u64,
    },
    SetPrecision(u8),
    SetResultFormat(u8),
    SetMatrixIndices {
        owner: u64,
        show: bool,
    },
    SetComplexFormat(ComplexFormat),
    SetTrailingZeros(bool),
    SetZeroThreshold(u16),
    SetComplexThreshold(u16),
    SetExponentialThreshold(u8),
    SetLabel(&'static str),
    InsertMatrixRow,
    InsertMatrixColumn,
    DeleteMatrixRow,
    DeleteMatrixColumn,
    ClearCells,
}

#[derive(Clone, Copy)]
enum Icon {
    Open,
    Save,
    Undo,
    Redo,
    Math,
    Text,
    Plot,
    Operators,
    Symbols,
    Program,
    Constants,
    Symbolics,
    Label,
    Subscript,
    Units,
    Clipboard,
    Cut,
    Copy,
    Delete,
    Matrix,
    Rows,
    Recalculate,
    Page,
    Zoom,
    Function,
    Align,
    Link,
    Help,
}

/// Renders only chrome; actions are dispatched by the worksheet after painting.
pub(crate) fn show(
    parent_ui: &mut Ui,
    state: &mut RibbonState,
    current: &RibbonContext<'_>,
) -> Vec<RibbonAction> {
    modern::show(parent_ui, state, current)
}

pub(crate) fn show_help(
    ctx: &egui::Context,
    state: &mut RibbonState,
    open: &mut bool,
) -> Vec<RibbonAction> {
    modern::help(ctx, state, open)
}

fn tab_button(ui: &mut Ui, label: &str, selected: bool, _file: bool) -> Response {
    ui.add(
        egui::Button::new(label)
            .selected(selected)
            .min_size(egui::vec2(0., 28.)),
    )
}

fn group(ui: &mut Ui, title: &str, width: f32, content: impl FnOnce(&mut Ui)) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, GROUP_HEIGHT), Sense::hover());
    let body = Rect::from_min_max(
        rect.min + egui::vec2(2., 3.),
        rect.max - egui::vec2(3., 19.),
    );
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(body)
            .layout(Layout::left_to_right(Align::Min)),
    );
    child.spacing_mut().item_spacing = egui::vec2(1., 0.);
    content(&mut child);
    ui.painter().text(
        egui::pos2(rect.center().x, rect.max.y - 9.),
        egui::Align2::CENTER_CENTER,
        title,
        FontId::proportional(11.),
        ui.visuals().weak_text_color(),
    );
    ui.painter().vline(
        rect.max.x,
        rect.min.y + 4. ..=rect.max.y - 4.,
        Stroke::new(1., crate::theme::palette(ui.ctx()).edge),
    );
}

fn icon_button(
    ui: &mut Ui,
    icon: Icon,
    label: &str,
    tooltip: &str,
    enabled: bool,
    large: bool,
    dropdown: bool,
) -> Response {
    let width = if large {
        if label.chars().count() > 5 { 76. } else { 58. }
    } else if label.is_empty() {
        25.
    } else {
        101.
    };
    let size = egui::vec2(width, if large { 61. } else { 21. });
    let response = ui
        .add_enabled_ui(enabled, |ui| ui.allocate_response(size, Sense::click()))
        .inner;
    let p = ui.painter();
    if response.hovered() && enabled {
        p.rect_filled(response.rect, 2., ui.visuals().widgets.hovered.weak_bg_fill);
        p.rect_stroke(
            response.rect,
            2.,
            ui.visuals().widgets.hovered.bg_stroke,
            egui::StrokeKind::Inside,
        );
    }
    let color = if enabled {
        ui.visuals().text_color()
    } else {
        MUTED
    };
    let icon_rect = if large {
        Rect::from_center_size(
            response.rect.center_top() + egui::vec2(0., 21.),
            Vec2::splat(32.),
        )
    } else {
        Rect::from_center_size(
            response.rect.left_center() + egui::vec2(11., 0.),
            Vec2::splat(16.),
        )
    };
    paint_icon(
        p,
        icon_rect,
        icon,
        if enabled {
            crate::theme::palette(ui.ctx()).accent
        } else {
            MUTED
        },
    );
    if !label.is_empty() {
        p.text(
            if large {
                response.rect.center_bottom() - egui::vec2(0., 12.)
            } else {
                response.rect.left_center() + egui::vec2(25., 0.)
            },
            if large {
                egui::Align2::CENTER_CENTER
            } else {
                egui::Align2::LEFT_CENTER
            },
            label,
            FontId::proportional(11.),
            color,
        );
    }
    if dropdown {
        let c = if large {
            response.rect.center_bottom() - egui::vec2(0., 2.)
        } else {
            response.rect.right_center() - egui::vec2(7., 0.)
        };
        p.add(egui::Shape::convex_polygon(
            vec![
                c + egui::vec2(-2.5, -2.),
                c + egui::vec2(2.5, -2.),
                c + egui::vec2(0., 1.),
            ],
            color,
            Stroke::NONE,
        ));
    }
    response
        .on_hover_text(tooltip)
        .on_disabled_hover_text(tooltip)
}

fn command(
    ui: &mut Ui,
    icon: Icon,
    label: &str,
    action: RibbonAction,
    enabled: bool,
    large: bool,
    actions: &mut Vec<RibbonAction>,
) {
    let mut tooltip = crate::command_catalog::tooltip(action, label);
    if !enabled {
        tooltip.push_str("\n선택한 영역 또는 현재 편집 위치에서 사용할 수 없습니다.");
    }
    if icon_button(ui, icon, label, &tooltip, enabled, large, false).clicked() {
        actions.push(action);
    }
}

fn unavailable(ui: &mut Ui, icon: Icon, label: &str, large: bool) {
    icon_button(
        ui,
        icon,
        label,
        &format!("{label}\n아직 지원하지 않는 명령입니다."),
        false,
        large,
        false,
    );
}

fn menu(ui: &mut Ui, icon: Icon, label: &str, large: bool, content: impl FnOnce(&mut Ui)) {
    menu_enabled(ui, icon, label, true, large, content);
}

fn menu_enabled(
    ui: &mut Ui,
    icon: Icon,
    label: &str,
    enabled: bool,
    large: bool,
    content: impl FnOnce(&mut Ui),
) {
    let response = icon_button(ui, icon, label, label, enabled, large, true);
    egui::Popup::menu(&response).show(|ui| {
        ui.set_min_width(180.);
        content(ui);
    });
}

fn menu_action(ui: &mut Ui, label: &str, action: RibbonAction, actions: &mut Vec<RibbonAction>) {
    if ui
        .button(label)
        .on_hover_text(crate::command_catalog::tooltip(action, label))
        .clicked()
    {
        actions.push(action);
        ui.close();
    }
}

fn clipboard(ui: &mut Ui, current: &RibbonContext<'_>, actions: &mut Vec<RibbonAction>) {
    group(ui, "클립보드", 109., |ui| {
        ui.vertical(|ui| {
            command(
                ui,
                Icon::Cut,
                "잘라내기",
                RibbonAction::Cut,
                current.has_selection,
                false,
                actions,
            );
            command(
                ui,
                Icon::Copy,
                "복사",
                RibbonAction::Copy,
                current.has_selection,
                false,
                actions,
            );
            command(
                ui,
                Icon::Clipboard,
                "붙여넣기",
                RibbonAction::Paste,
                true,
                false,
                actions,
            );
        });
    });
}

fn math_tab(ui: &mut Ui, current: &RibbonContext<'_>, actions: &mut Vec<RibbonAction>) {
    group(ui, "영역", 305., |ui| {
        command(
            ui,
            Icon::Math,
            "수학",
            RibbonAction::InsertMath,
            true,
            true,
            actions,
        );
        command(
            ui,
            Icon::Math,
            "풀이 구간",
            RibbonAction::InsertSolveBlock,
            true,
            true,
            actions,
        );
        unavailable(ui, Icon::Plot, "차트 컴포넌트", true);
        ui.vertical(|ui| {
            unavailable(ui, Icon::Text, "텍스트 블록", false);
            command(
                ui,
                Icon::Text,
                "텍스트 상자",
                RibbonAction::InsertText,
                true,
                false,
                actions,
            );
            unavailable(ui, Icon::Page, "이미지", false);
        });
    });
    group(ui, "연산자 및 기호", 303., |ui| {
        menu(ui, Icon::Operators, "연산자", true, |ui| {
            egui::Grid::new("operators_gallery")
                .num_columns(3)
                .spacing(egui::vec2(4., 4.))
                .show(ui, |ui| {
                    let ops = [
                        ("분수", RibbonAction::Fraction),
                        ("거듭제곱", RibbonAction::Power),
                        ("근호", RibbonAction::Root),
                        ("절댓값/행렬식", RibbonAction::Absolute),
                        ("팩토리얼", RibbonAction::Factorial),
                        ("백분율 %", RibbonAction::Percent),
                        ("도 °", RibbonAction::Degree),
                        ("극형식 ∠", RibbonAction::Polar),
                        ("전치", RibbonAction::Transpose),
                        ("복소수 켤레", RibbonAction::Conjugate),
                        ("노름", RibbonAction::Norm),
                        ("행 추출", RibbonAction::RowOperator),
                        ("열 추출", RibbonAction::ColumnOperator),
                        ("벡터화", RibbonAction::Vectorize),
                        ("벡터 외적", RibbonAction::CrossProduct),
                        ("정의 :=", RibbonAction::Define),
                        ("전역 정의", RibbonAction::GlobalDefine),
                        ("계산 =", RibbonAction::Evaluate),
                        ("범위", RibbonAction::Range),
                        ("인덱스", RibbonAction::Index),
                        ("함수", RibbonAction::Function),
                        ("정적분", RibbonAction::Integral),
                        ("미분", RibbonAction::Derivative),
                        ("편미분", RibbonAction::PartialDerivative),
                        ("합", RibbonAction::Summation),
                        ("곱", RibbonAction::Product),
                    ];
                    for (i, (label, action)) in ops.into_iter().enumerate() {
                        menu_action(ui, label, action, actions);
                        if i % 3 == 2 {
                            ui.end_row();
                        }
                    }
                });
        });
        menu(ui, Icon::Symbols, "기호", true, |ui| {
            egui::Grid::new("symbols_gallery")
                .num_columns(6)
                .show(ui, |ui| {
                    for (i, s) in ["α", "β", "γ", "δ", "ε", "θ", "λ", "μ", "π", "ρ", "σ", "ω"]
                        .into_iter()
                        .enumerate()
                    {
                        menu_action(ui, s, RibbonAction::InsertSymbol(s), actions);
                        if i % 6 == 5 {
                            ui.end_row();
                        }
                    }
                });
        });
        menu(ui, Icon::Program, "프로그래밍", true, |ui| {
            for (label, action) in [
                ("프로그램 블록", RibbonAction::Program),
                ("로컬 정의", RibbonAction::LocalDefine),
                ("if", RibbonAction::If),
                ("else if", RibbonAction::ElseIf),
                ("also if", RibbonAction::AlsoIf),
                ("else", RibbonAction::Else),
                ("for", RibbonAction::ForLoop),
                ("while", RibbonAction::WhileLoop),
                ("return", RibbonAction::Return),
                ("break", RibbonAction::Break),
                ("continue", RibbonAction::Continue),
                ("try / on error", RibbonAction::Try),
            ] {
                menu_action(ui, label, action, actions);
            }
        });
        menu(ui, Icon::Constants, "상수", true, |ui| {
            for (label, s) in [
                ("π — 원주율", "π"),
                ("e — 자연상수", "e"),
                ("i — 허수 단위", "i"),
                ("g — 표준 중력", "g"),
                ("c — 빛의 속력", "c"),
            ] {
                if s == "i" {
                    menu_action(ui, label, RibbonAction::InsertImaginary, actions);
                } else {
                    menu_action(
                        ui,
                        label,
                        RibbonAction::InsertIdentifier {
                            name: s,
                            label: if s == "g" { "UNIT" } else { "CONSTANT" },
                        },
                        actions,
                    );
                }
            }
        });
        menu(ui, Icon::Symbolics, "기호 연산", true, |ui| {
            menu_action(ui, "기호식 계산 →", RibbonAction::SymbolicEvaluate, actions);
        });
    });
    group(ui, "스타일", 111., |ui| {
        ui.vertical(|ui| {
            label_menu(ui, current, actions, false);
            command(
                ui,
                Icon::Subscript,
                "아래 첨자",
                RibbonAction::LiteralSubscript,
                current.has_math,
                false,
                actions,
            );
            unavailable(ui, Icon::Math, "방정식 행 분할", false);
        });
    });
    group(ui, "단위", 127., |ui| {
        menu(ui, Icon::Units, "단위", true, |ui| {
            for (label, s) in [
                ("길이  m", "m"),
                ("질량  kg", "kg"),
                ("시간  s", "s"),
                ("전류  A", "A"),
                ("온도  K", "K"),
                ("온도  °C", "°C"),
                ("온도  °F", "°F"),
                ("온도차  Δ°C", "Δ°C"),
                ("온도차  Δ°F", "Δ°F"),
                ("힘  N", "N"),
                ("압력  Pa", "Pa"),
                ("에너지  J", "J"),
                ("전력  W", "W"),
                ("주파수  Hz", "Hz"),
            ] {
                menu_action(ui, label, unit_choice(s), actions);
            }
        });
        unavailable(ui, Icon::Units, "단위계", true);
    });
    clipboard(ui, current, actions);
}

/// Shared command path for the actual Units menu and its worksheet regression.
pub(crate) fn unit_choice(name: &'static str) -> RibbonAction {
    RibbonAction::InsertUnit(name)
}

fn io_tab(ui: &mut Ui, actions: &mut Vec<RibbonAction>) {
    group(ui, "워크시트", 150., |ui| {
        command(
            ui,
            Icon::Open,
            "열기",
            RibbonAction::Open,
            true,
            true,
            actions,
        );
        unavailable(ui, Icon::Link, "포함", true);
    });
    group(ui, "파일 액세스", 200., |ui| {
        ui.vertical(|ui| {
            unavailable(ui, Icon::Open, "데이터 읽기", false);
            unavailable(ui, Icon::Save, "데이터 쓰기", false);
            unavailable(ui, Icon::Matrix, "Excel", false);
        });
    });
    group(ui, "개체", 130., |ui| {
        unavailable(ui, Icon::Page, "개체 삽입", true);
    });
    group(ui, "입력 컨트롤", 200., |ui| {
        unavailable(ui, Icon::Page, "입력 컨트롤", true);
    });
}

fn functions_tab(ui: &mut Ui, actions: &mut Vec<RibbonAction>) {
    group(ui, "함수", 650., |ui| {
        menu(ui, Icon::Function, "모든 함수", true, |ui| {
            for name in [
                "sin", "cos", "tan", "exp", "ln", "log", "sqrt", "abs", "mean", "min", "max",
                "det", "inverse", "rows", "cols",
            ] {
                menu_action(ui, name, RibbonAction::InsertFunction(name), actions);
            }
        });
        ui.vertical(|ui| {
            menu(ui, Icon::Function, "삼각 함수", false, |ui| {
                for s in ["sin", "cos", "tan", "asin", "acos", "atan", "atan2"] {
                    menu_action(ui, s, RibbonAction::InsertFunction(s), actions);
                }
            });
            menu(ui, Icon::Function, "통계", false, |ui| {
                for s in ["mean", "median", "stdev", "var", "min", "max"] {
                    menu_action(ui, s, RibbonAction::InsertFunction(s), actions);
                }
            });
            menu(ui, Icon::Function, "벡터 및 행렬", false, |ui| {
                for s in ["det", "inverse", "rows", "cols", "length", "transpose"] {
                    menu_action(ui, s, RibbonAction::InsertFunction(s), actions);
                }
            });
        });
        ui.vertical(|ui| {
            unavailable(ui, Icon::Function, "곡선 적합", false);
            unavailable(ui, Icon::Function, "데이터 분석", false);
            unavailable(ui, Icon::Function, "미분 방정식", false);
        });
        ui.vertical(|ui| {
            unavailable(ui, Icon::Function, "신호 처리", false);
            unavailable(ui, Icon::Function, "확률 분포", false);
            unavailable(ui, Icon::Function, "이미지 처리", false);
        });
    });
}

fn matrices_tab(ui: &mut Ui, current: &RibbonContext<'_>, actions: &mut Vec<RibbonAction>) {
    group(ui, "행렬 및 표", 195., |ui| {
        command(
            ui,
            Icon::Matrix,
            "행렬 삽입",
            RibbonAction::Matrix,
            true,
            true,
            actions,
        );
        command(
            ui,
            Icon::Matrix,
            "표 삽입",
            RibbonAction::InsertTable,
            true,
            true,
            actions,
        );
        command(
            ui,
            Icon::Subscript,
            "인덱스",
            RibbonAction::Index,
            true,
            true,
            actions,
        );
    });
    group(ui, "행 및 열", 180., |ui| {
        ui.vertical(|ui| {
            command(
                ui,
                Icon::Rows,
                "행 삽입",
                RibbonAction::InsertMatrixRow,
                current.in_matrix || current.in_table,
                false,
                actions,
            );
            command(
                ui,
                Icon::Rows,
                "열 삽입",
                RibbonAction::InsertMatrixColumn,
                current.in_matrix || current.in_table,
                false,
                actions,
            );
            menu_enabled(
                ui,
                Icon::Delete,
                "행 / 열 삭제",
                current.in_matrix || current.in_table,
                false,
                |ui| {
                    menu_action(ui, "행 삭제", RibbonAction::DeleteMatrixRow, actions);
                    menu_action(ui, "열 삭제", RibbonAction::DeleteMatrixColumn, actions);
                    if ui
                        .add_enabled(
                            current.in_matrix || current.in_table,
                            egui::Button::new("셀 내용 지우기"),
                        )
                        .clicked()
                    {
                        actions.push(RibbonAction::ClearCells);
                        ui.close();
                    }
                },
            );
        });
    });
    group(ui, "결과 형식", 140., |ui| {
        matrix_indices_button(ui, current.matrix_indices, actions);
        unavailable(ui, Icon::Matrix, "중첩 행렬 축소", false);
    });
    clipboard(ui, current, actions);
}

fn plots_tab(ui: &mut Ui, current: &RibbonContext<'_>, actions: &mut Vec<RibbonAction>) {
    group(ui, "도표", 255., |ui| {
        command(
            ui,
            Icon::Plot,
            "XY 도표",
            RibbonAction::InsertPlot,
            true,
            true,
            actions,
        );
        unavailable(ui, Icon::Plot, "극좌표 도표", true);
        unavailable(ui, Icon::Plot, "3D 도표", true);
    });
    group(ui, "편집", 150., |ui| {
        command(
            ui,
            Icon::Plot,
            "도표 설정",
            RibbonAction::EditPlot,
            current.has_plot,
            true,
            actions,
        );
    });
    group(ui, "도표 값 서식", 125., |ui| {
        unavailable(ui, Icon::Plot, "표시 형식", true)
    });
}

fn label_menu(
    ui: &mut Ui,
    current: &RibbonContext<'_>,
    actions: &mut Vec<RibbonAction>,
    large: bool,
) {
    menu_enabled(
        ui,
        Icon::Label,
        "레이블",
        current.can_label,
        large,
        |ui| {
            for (title, label) in [
                ("자동", "*"),
                ("변수", "VARIABLE"),
                ("단위", "UNIT"),
                ("상수", "CONSTANT"),
                ("함수", "FUNCTION"),
                ("시스템", "SYSTEM"),
                ("키워드", "KEYWORD"),
            ] {
                menu_action(ui, title, RibbonAction::SetLabel(label), actions);
            }
        },
    );
}

fn math_format_tab(ui: &mut Ui, current: &RibbonContext<'_>, actions: &mut Vec<RibbonAction>) {
    group(ui, "결과", 410., |ui| {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.add_enabled_ui(current.has_math, |ui| {
                    let mut format = current.result_format.min(4);
                    egui::ComboBox::from_id_salt("math_result_format")
                        .width(130.)
                        .selected_text(
                            ["일반", "소수", "과학", "공학", "백분율"][usize::from(format)],
                        )
                        .show_ui(ui, |ui| {
                            for (value, title) in [
                                (0, "일반"),
                                (1, "소수"),
                                (2, "과학"),
                                (3, "공학"),
                                (4, "백분율"),
                            ] {
                                if ui.selectable_value(&mut format, value, title).changed() {
                                    actions.push(RibbonAction::SetResultFormat(value));
                                }
                            }
                        });
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("소수 자릿수").size(11.));
                        egui::ComboBox::from_id_salt("result_precision")
                            .width(42.)
                            .selected_text(current.precision.to_string())
                            .show_ui(ui, |ui| {
                                for precision in 0..=15 {
                                    if ui
                                        .selectable_label(
                                            current.precision == precision,
                                            precision.to_string(),
                                        )
                                        .clicked()
                                    {
                                        actions.push(RibbonAction::SetPrecision(precision));
                                        ui.close();
                                    }
                                }
                            });
                    });
                });
                ui.add_enabled_ui(current.has_math, |ui| {
                    let mut zeros = current.number_format.show_trailing_zeros;
                    if ui.checkbox(&mut zeros, "후행 0 표시").changed() {
                        actions.push(RibbonAction::SetTrailingZeros(zeros));
                    }
                });
            });
            ui.vertical(|ui| {
                ui.add_enabled_ui(current.has_math, |ui| {
                    let choices = [
                        (ComplexFormat::I, "i"),
                        (ComplexFormat::J, "j"),
                        (ComplexFormat::PolarRadians, "극형식 · rad"),
                        (ComplexFormat::PolarPiRadians, "극형식 · π rad"),
                        (ComplexFormat::PolarDegrees, "극형식 · °"),
                    ];
                    let mut complex = current.number_format.complex_format;
                    egui::ComboBox::from_id_salt("complex_result_format")
                        .width(140.)
                        .selected_text(
                            choices
                                .iter()
                                .find(|(value, _)| *value == complex)
                                .unwrap()
                                .1,
                        )
                        .show_ui(ui, |ui| {
                            for (value, title) in choices {
                                if ui.selectable_value(&mut complex, value, title).changed() {
                                    actions.push(RibbonAction::SetComplexFormat(value));
                                }
                            }
                        });
                    ui.menu_button("임계값", |ui| {
                        let mut zero = current.number_format.zero_threshold;
                        let mut complex = current.number_format.complex_threshold;
                        let mut exponential = current.number_format.exponential_threshold;
                        ui.horizontal(|ui| {
                            ui.label("0");
                            if ui
                                .add(egui::DragValue::new(&mut zero).range(0..=307))
                                .changed()
                            {
                                actions.push(RibbonAction::SetZeroThreshold(zero));
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.label("복소수");
                            if ui
                                .add(egui::DragValue::new(&mut complex).range(0..=307))
                                .changed()
                            {
                                actions.push(RibbonAction::SetComplexThreshold(complex));
                            }
                        });
                        ui.add_enabled_ui(current.result_format == 0, |ui| {
                            ui.horizontal(|ui| {
                                ui.label("지수");
                                if ui
                                    .add(egui::DragValue::new(&mut exponential).range(0..=15))
                                    .changed()
                                {
                                    actions
                                        .push(RibbonAction::SetExponentialThreshold(exponential));
                                }
                            });
                        });
                    });
                });
            });
        });
    });
    group(ui, "계산 서식", 240., |ui| {
        ui.vertical(|ui| {
            ui.add_enabled_ui(current.has_math, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("글꼴 크기").size(11.));
                    egui::ComboBox::from_id_salt("math_font_points")
                        .width(58.)
                        .selected_text(format!("{} pt", current.font_points))
                        .show_ui(ui, |ui| {
                            for points in 4..=72 {
                                if ui
                                    .selectable_label(
                                        (current.font_points - points as f32).abs() < 0.01,
                                        format!("{points} pt"),
                                    )
                                    .clicked()
                                {
                                    actions.push(RibbonAction::SetFontSize(points as f32));
                                    ui.close();
                                }
                            }
                        });
                });
            });
            label_menu(ui, current, actions, false);
            unavailable(ui, Icon::Math, "서식 지우기", false);
        });
    });
}

fn text_format_tab(
    ui: &mut Ui,
    current: &RibbonContext<'_>,
    draft: &mut TextFormatDraft,
    actions: &mut Vec<RibbonAction>,
) {
    let context = current.text_format.filter(|context| context.enabled);
    group(ui, "글꼴", 310., |ui| {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.add_enabled(false, egui::Button::new("Tahoma           ▾"));
                ui.add_enabled_ui(context.is_some(), |ui| {
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut draft.size)
                            .id(egui::Id::new("ribbon-rich-text-font-points"))
                            .desired_width(40.)
                            .hint_text("크기"),
                    );
                    let commit = response.lost_focus();
                    let mut selected = None;
                    egui::ComboBox::from_id_salt("rich_text_font_points_presets")
                        .width(18.)
                        .selected_text("▾")
                        .show_ui(ui, |ui| {
                            for points in [8, 9, 10, 11, 12, 14, 16, 20, 24] {
                                if ui
                                    .selectable_label(
                                        draft.source_points == Some(f64::from(points)),
                                        points.to_string(),
                                    )
                                    .clicked()
                                {
                                    selected = Some(points);
                                    ui.close();
                                }
                            }
                        });
                    if let Some(points) = selected {
                        draft.size = points.to_string();
                    }
                    if (commit || selected.is_some())
                        && let Some(action) = draft.commit_size()
                    {
                        actions.push(action);
                    }
                });
            });
            ui.horizontal(|ui| {
                ui.add_enabled_ui(context.is_some(), |ui| {
                    let bold = context.is_some_and(|context| context.retained_bold);
                    let italic = context.is_some_and(|context| context.retained_italic);
                    if ui
                        .add(egui::Button::new(egui::RichText::new("B").strong()).selected(bold))
                        .on_hover_text("굵게  Ctrl+B")
                        .clicked()
                        && let Some(context) = context
                    {
                        actions.push(RibbonAction::SetTextBold {
                            value: !bold,
                            selection_key: context.selection_key,
                        });
                    }
                    if ui
                        .add(egui::Button::new(egui::RichText::new("I").italics()).selected(italic))
                        .on_hover_text("기울임꼴  Ctrl+I")
                        .clicked()
                        && let Some(context) = context
                    {
                        actions.push(RibbonAction::SetTextItalic {
                            value: !italic,
                            selection_key: context.selection_key,
                        });
                    }
                });
                let underline = context.is_some_and(|c| c.retained_underline);
                if ui
                    .add_enabled(
                        context.is_some_and(|c| c.underline_set_enabled),
                        egui::Button::new(egui::RichText::new("U").underline()).selected(underline),
                    )
                    .on_hover_text("밑줄  Ctrl+U")
                    .clicked()
                    && let Some(context) = context.filter(|c| c.underline_set_enabled)
                {
                    actions.push(RibbonAction::SetTextUnderline {
                        value: !underline,
                        selection_key: context.selection_key,
                    });
                }
                ui.add_enabled_ui(context.is_some(), |ui| {
                    if ui
                        .button("글꼴 색")
                        .on_hover_text("마지막으로 선택한 글꼴 색 적용")
                        .clicked()
                        && let Some(context) = context
                    {
                        actions.push(RibbonAction::SetTextColor {
                            rgba: draft.last_used_color.unwrap_or([0, 0, 0, 255]),
                            selection_key: context.selection_key,
                        });
                    }
                    let response = ui.color_edit_button_srgba_unmultiplied(&mut draft.color);
                    if response.changed() {
                        draft.color_dirty = true;
                    }
                    // No source action while dragging/typing preview. Release,
                    // popup close or Enter is an independent commit boundary.
                    let commit = !ui.ctx().any_popup_open()
                        || ui.input(|input| {
                            input.pointer.any_released() || input.key_pressed(egui::Key::Enter)
                        });
                    if commit && let Some(action) = draft.commit_color() {
                        actions.push(action);
                    }
                });
            });
        });
    });
    group(ui, "텍스트 및 영역", 180., |ui| {
        use mathcraft_document::rich_text_source::ParagraphAlignment as A;
        ui.horizontal_wrapped(|ui| {
            for (value, label, shortcut) in [
                (A::Left, "왼쪽", "Ctrl+L"),
                (A::Center, "가운데", "Ctrl+E"),
                (A::Right, "오른쪽", "Ctrl+R"),
                (A::Justify, "양쪽", "Ctrl+J"),
            ] {
                if ui
                    .add_enabled(
                        current.paragraph_alignment.is_some(),
                        egui::Button::new(label).selected(
                            current
                                .paragraph_alignment
                                .is_some_and(|c| c.current == Some(value)),
                        ),
                    )
                    .on_hover_text(format!("{label} 맞춤  {shortcut}"))
                    .clicked()
                    && let Some(context) = current.paragraph_alignment
                {
                    actions.push(RibbonAction::SetParagraphAlignment {
                        value,
                        selection_key: context.selection_key,
                    });
                }
            }
        });
        unavailable(ui, Icon::Text, "텍스트 서식", true);
    });
    group(ui, "스타일", 120., |ui| {
        unavailable(ui, Icon::Text, "일반", true)
    });
    group(ui, "링크", 110., |ui| {
        unavailable(ui, Icon::Link, "링크", true)
    });
    group(ui, "맞춤법", 110., |ui| {
        unavailable(ui, Icon::Text, "맞춤법 검사", true)
    });
}

fn calculation_tab(ui: &mut Ui, current: &RibbonContext<'_>, actions: &mut Vec<RibbonAction>) {
    group(ui, "컨트롤", 355., |ui| {
        ui.vertical(|ui| {
            let mut automatic = current.automatic_calculation;
            if ui.checkbox(&mut automatic, "자동 계산").changed() {
                actions.push(RibbonAction::SetAutomaticCalculation(automatic));
            }
            command(
                ui,
                Icon::Delete,
                "모든 계산 중지",
                RibbonAction::StopAllCalculations,
                current.calculation_running,
                false,
                actions,
            );
            let mut disabled = current.region_calculation_disabled.unwrap_or(false);
            if ui
                .add_enabled(
                    current.region_calculation_disabled.is_some(),
                    egui::Checkbox::new(&mut disabled, "영역 비활성화"),
                )
                .changed()
            {
                actions.push(RibbonAction::SetRegionCalculationDisabled(disabled));
            }
        });
        command(
            ui,
            Icon::Recalculate,
            "워크시트 계산",
            RibbonAction::Recalculate,
            true,
            true,
            actions,
        );
        command(
            ui,
            Icon::Recalculate,
            "지금 계산",
            RibbonAction::CalculateNow,
            true,
            true,
            actions,
        );
    });
    group(ui, "오류 추적", 170., |ui| {
        command(
            ui,
            Icon::Undo,
            "이전 오류",
            RibbonAction::PreviousError,
            current.has_errors,
            true,
            actions,
        );
        command(
            ui,
            Icon::Redo,
            "다음 오류",
            RibbonAction::NextError,
            current.has_errors,
            true,
            actions,
        );
    });
    group(ui, "경고", 110., |ui| {
        let label = if current.show_warnings {
            "경고 숨기기"
        } else {
            "경고 표시"
        };
        command(
            ui,
            Icon::Math,
            label,
            RibbonAction::ToggleWarnings,
            current.has_warnings || current.show_warnings,
            true,
            actions,
        )
    });
    group(ui, "옵션", 175., |ui| {
        ui.vertical(|ui| {
            unavailable(ui, Icon::Math, "수렴 허용 오차", false);
            unavailable(ui, Icon::Math, "배열 원점", false);
            ui.menu_button("계산 옵션", |ui| {
                let mut radix_mode = current.bin_oct_hex_mode.unwrap_or(false);
                if ui
                    .add_enabled(
                        current.bin_oct_hex_mode.is_some(),
                        egui::Checkbox::new(&mut radix_mode, "Bin/Oct/Hex 모드"),
                    )
                    .changed()
                {
                    actions.push(RibbonAction::SetBinOctHexMode(radix_mode));
                    ui.close();
                }
            });
        });
    });
}

fn document_tab(ui: &mut Ui, current: &RibbonContext<'_>, actions: &mut Vec<RibbonAction>) {
    group(ui, "영역", 160., |ui| {
        command(
            ui,
            Icon::Rows,
            "영역",
            RibbonAction::InsertArea,
            current.can_insert_area,
            true,
            actions,
        );
        ui.vertical(|ui| {
            unavailable(ui, Icon::Math, "모두 선택", false);
            command(
                ui,
                Icon::Delete,
                "영역 삭제",
                RibbonAction::Delete,
                current.has_selection,
                false,
                actions,
            );
            unavailable(ui, Icon::Align, "영역 맞춤", false);
        });
    });
    group(ui, "간격", 190., |ui| {
        ui.vertical(|ui| {
            command(
                ui,
                Icon::Page,
                "페이지 나누기  Ctrl+Enter",
                RibbonAction::InsertPageBreak,
                current.can_page_break,
                false,
                actions,
            );
            menu(
                ui,
                Icon::Delete,
                "공백 제거",
                current.can_page_break,
                |ui| {
                    for (label, action, enabled) in [
                        (
                            "위쪽 공간",
                            RibbonAction::RemovePageBreakAbove,
                            current.can_remove_page_break_above,
                        ),
                        (
                            "아래쪽 공간",
                            RibbonAction::RemovePageBreakBelow,
                            current.can_remove_page_break_below,
                        ),
                    ] {
                        if ui
                            .add_enabled(enabled, egui::Button::new(label))
                            .on_hover_text(
                                "인접한 페이지 나누기가 있는 단순 배치에서 사용할 수 있습니다.",
                            )
                            .clicked()
                        {
                            actions.push(action);
                            ui.close();
                        }
                    }
                },
            );
        });
    });
    group(ui, "페이지", 170., |ui| {
        command(
            ui,
            Icon::Page,
            "크기",
            RibbonAction::PageSetup,
            true,
            true,
            actions,
        );
        command(
            ui,
            Icon::Page,
            "여백",
            RibbonAction::PageSetup,
            true,
            true,
            actions,
        );
    });
    group(ui, "보기", 260., |ui| {
        ui.vertical(|ui| {
            if ui.selectable_label(!current.draft_view, "페이지").clicked() {
                actions.push(RibbonAction::SetDraftView(false));
            }
            if ui.selectable_label(current.draft_view, "초안").clicked() {
                actions.push(RibbonAction::SetDraftView(true));
            }
        });
        menu(ui, Icon::Zoom, "확대/축소", true, |ui| {
            for pct in [50, 75, 100, 125, 150, 200] {
                menu_action(
                    ui,
                    &format!("{pct}%"),
                    RibbonAction::Zoom(pct as f32 / 100.),
                    actions,
                );
            }
        });
        ui.vertical(|ui| {
            if ui.selectable_label(current.grid, "눈금선 표시").clicked() {
                actions.push(RibbonAction::ToggleGrid);
            }
            if ui.button("100%").clicked() {
                actions.push(RibbonAction::Zoom(1.));
            }
            ui.label(egui::RichText::new(format!("현재 {:.0}%", current.zoom * 100.)).size(11.));
        });
    });
    group(ui, "머리글 및 바닥글", 135., |ui| {
        command(
            ui,
            Icon::Page,
            "머리글 / 바닥글",
            RibbonAction::EditPageSections,
            true,
            true,
            actions,
        )
    });
    clipboard(ui, current, actions);
}

fn resources_tab(ui: &mut Ui) {
    for (title, label) in [
        ("도움말", "도움말"),
        ("커뮤니티", "커뮤니티"),
        ("기술 지원", "기술 지원"),
        ("학습", "자습서"),
    ] {
        group(ui, title, 130., |ui| {
            unavailable(ui, Icon::Help, label, true)
        });
    }
}

fn paint_icon(p: &egui::Painter, rect: Rect, icon: Icon, color: Color32) {
    let unit = rect.width() / 32.;
    let q = |x: f32, y: f32| Pos2::new(rect.min.x + x * unit, rect.min.y + y * unit);
    let stroke = Stroke::new(1.25 * unit, color);
    let line = |a: (f32, f32), b: (f32, f32)| {
        p.line_segment([q(a.0, a.1), q(b.0, b.1)], stroke);
    };
    let text = |s: &str, size: f32| {
        p.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            s,
            FontId::proportional(size * unit),
            color,
        );
    };
    match icon {
        Icon::Page | Icon::Text | Icon::Math => {
            p.add(egui::Shape::convex_polygon(
                vec![q(6., 2.), q(22., 2.), q(27., 7.), q(27., 30.), q(6., 30.)],
                Color32::WHITE,
                stroke,
            ));
            line((22., 2.), (22., 7.));
            line((22., 7.), (27., 7.));
            match icon {
                Icon::Math => text("x", 23.),
                Icon::Text => {
                    for y in [12., 17., 22.] {
                        line((10., y), (23., y));
                    }
                }
                _ => {}
            }
        }
        Icon::Open => {
            p.add(egui::Shape::convex_polygon(
                vec![
                    q(2., 8.),
                    q(12., 8.),
                    q(15., 11.),
                    q(29., 11.),
                    q(27., 28.),
                    q(2., 28.),
                ],
                Color32::from_rgb(250, 218, 137),
                stroke,
            ));
            line((5., 16.), (30., 16.));
            line((5., 16.), (2., 28.));
        }
        Icon::Save => {
            p.rect_filled(Rect::from_min_max(q(3., 3.), q(29., 29.)), 1., color);
            p.rect_filled(
                Rect::from_min_max(q(8., 4.), q(23., 13.)),
                0.,
                Color32::from_rgb(235, 241, 245),
            );
            p.rect_filled(
                Rect::from_min_max(q(8., 18.), q(24., 28.)),
                0.,
                Color32::WHITE,
            );
            line((21., 6.), (21., 11.));
        }
        Icon::Undo | Icon::Redo => {
            let flip = matches!(icon, Icon::Redo);
            let qq = |x: f32, y: f32| q(if flip { 32. - x } else { x }, y);
            p.add(egui::Shape::line(
                vec![
                    qq(5., 11.),
                    qq(18., 8.),
                    qq(26., 13.),
                    qq(26., 21.),
                    qq(20., 26.),
                ],
                stroke,
            ));
            p.add(egui::Shape::line(
                vec![qq(7., 3.), qq(4., 12.), qq(13., 15.)],
                stroke,
            ));
        }
        Icon::Plot => {
            line((4., 3.), (4., 27.));
            line((4., 27.), (30., 27.));
            p.add(egui::Shape::line(
                vec![q(6., 23.), q(11., 11.), q(16., 18.), q(21., 8.), q(28., 5.)],
                Stroke::new(2. * unit, color),
            ));
        }
        Icon::Operators => {
            text("±", 29.);
            line((3., 28.), (29., 28.));
        }
        Icon::Symbols => text("α", 30.),
        Icon::Constants => text("π", 29.),
        Icon::Symbolics => text("→", 30.),
        Icon::Program => {
            line((9., 3.), (5., 3.));
            line((5., 3.), (5., 29.));
            line((5., 29.), (9., 29.));
            for y in [8., 16., 24.] {
                line((11., y), (28., y));
            }
        }
        Icon::Label => text("Aa", 22.),
        Icon::Subscript => text("xₙ", 25.),
        Icon::Units => text("m", 29.),
        Icon::Clipboard => {
            p.rect_stroke(
                Rect::from_min_max(q(5., 5.), q(27., 30.)),
                1.,
                stroke,
                egui::StrokeKind::Inside,
            );
            p.rect_filled(Rect::from_min_max(q(11., 2.), q(22., 8.)), 1., color);
            for y in [14., 20., 25.] {
                line((10., y), (23., y));
            }
        }
        Icon::Copy => {
            p.rect_stroke(
                Rect::from_min_max(q(3., 3.), q(23., 25.)),
                0.,
                stroke,
                egui::StrokeKind::Inside,
            );
            p.rect_filled(
                Rect::from_min_max(q(10., 9.), q(29., 30.)),
                0.,
                Color32::WHITE,
            );
            p.rect_stroke(
                Rect::from_min_max(q(10., 9.), q(29., 30.)),
                0.,
                stroke,
                egui::StrokeKind::Inside,
            );
        }
        Icon::Cut => {
            p.circle_stroke(q(8., 24.), 5. * unit, stroke);
            p.circle_stroke(q(23., 24.), 5. * unit, stroke);
            line((10., 20.), (24., 3.));
            line((21., 20.), (7., 3.));
        }
        Icon::Delete => {
            line((7., 7.), (25., 25.));
            line((25., 7.), (7., 25.));
        }
        Icon::Matrix | Icon::Rows => {
            p.rect_stroke(
                Rect::from_min_max(q(3., 4.), q(29., 28.)),
                0.,
                stroke,
                egui::StrokeKind::Inside,
            );
            for x in [12., 21.] {
                line((x, 4.), (x, 28.));
            }
            for y in [12., 20.] {
                line((3., y), (29., y));
            }
        }
        Icon::Recalculate => {
            p.circle_stroke(q(16., 16.), 11. * unit, stroke);
            p.rect_filled(Rect::from_min_max(q(17., 1.), q(31., 13.)), 0., PANEL);
            line((25., 5.), (27., 14.));
            line((27., 14.), (18., 12.));
        }
        Icon::Zoom => {
            p.circle_stroke(q(13., 13.), 9. * unit, stroke);
            line((19., 20.), (29., 30.));
            line((8., 13.), (18., 13.));
            line((13., 8.), (13., 18.));
        }
        Icon::Function => text("f(x)", 21.),
        Icon::Align => {
            for (y, w) in [(6., 24.), (13., 18.), (20., 24.), (27., 18.)] {
                line((4., y), (w, y));
            }
        }
        Icon::Link => {
            p.circle_stroke(q(12., 13.), 7. * unit, stroke);
            p.circle_stroke(q(21., 21.), 7. * unit, stroke);
        }
        Icon::Help => {
            p.circle_stroke(q(16., 16.), 13. * unit, stroke);
            text("?", 24.);
        }
    }
}

fn matrix_indices_button(
    ui: &mut Ui,
    current: Option<MatrixIndicesContext>,
    actions: &mut Vec<RibbonAction>,
) -> egui::Response {
    let response = ui.add_enabled(
        current.is_some(),
        egui::Button::new("인덱스 표시").selected(current.is_some_and(|c| c.show_indices)),
    );
    if response.clicked()
        && let Some(current) = current
    {
        actions.push(RibbonAction::SetMatrixIndices {
            owner: current.owner,
            show: !current.show_indices,
        });
    }
    response
}
