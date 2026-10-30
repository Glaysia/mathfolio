//! Version 2 native Mathfolio canvas documents.
//!
//! This is currently a flat editable canvas. Nested Mathcad regions, sections,
//! relationships and unsupported content remain available in `native_archive`;
//! saving this format does not export edits back to Mathcad's package format.
//! Portable trees contain semantic content, never editor caret or undo history.
use mathfolio_editor::{NodeKind, PortableTree};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
#[cfg(not(target_arch = "wasm32"))]
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

#[path = "canvas_source_bytes.rs"]
mod source_bytes;

#[path = "canvas_authored.rs"]
mod authored;

#[path = "canvas_area_cache.rs"]
mod area_cache;
pub use area_cache::{SavedAreaChildResult, SavedAreaPresentation};

#[path = "matrix_result_views.rs"]
mod matrix_result_views;
pub use matrix_result_views::{
    MatrixResultTarget, MatrixResultView, MatrixResultViewport, MatrixResultViews,
    MatrixViewOffset, MatrixViewSize,
};

#[path = "canvas_signature.rs"]
mod signature_json;
pub use signature_json::{write_calculation_signature_json, write_region_signature_json};

pub const CANVAS_VERSION: u32 = 2;
pub const MAX_NATIVE_ARCHIVE_BYTES: usize = 16 * 1024 * 1024;
const MAX_DOCUMENT_BYTES: u64 = 128 * 1024 * 1024;
const MAX_REGIONS: usize = 4096;
const MAX_TEXT_BYTES: usize = 8 * 1024 * 1024;
const MAX_TREE_NODES_TOTAL: usize = 65_536;
// Keeps the serialized tree within serde_json's bounded nesting parser.
const MAX_CANVAS_TREE_DEPTH: usize = 48;
const MAX_DIMENSION: f64 = 1_000_000_000.0;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanvasDocument {
    pub version: u32,
    pub title: String,
    pub width: f64,
    pub height: f64,
    pub grid: f64,
    pub next_id: u64,
    pub regions: Vec<CanvasRegion>,
    #[serde(default)]
    pub page_sections: PageSections,
    /// Explicit paper settings. Legacy/imported documents retain their source
    /// settings until the user changes page setup.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_setup: Option<PageSetup>,
    /// Saved presentation only: opening a document must not execute formulas.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calculation_cache: Option<SavedCalculationCache>,
    /// An explicit NUMERIC authoring-mode override; None retains native settings.
    /// This changes future input recognition, not the meaning of existing trees.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bin_oct_hex_mode: Option<bool>,
    /// Requested matrix-result presentation only; not calculation source or values.
    #[serde(default, skip_serializing_if = "MatrixResultViews::is_empty")]
    pub matrix_result_views: MatrixResultViews,
    pub native_archive: Option<Vec<u8>>,
}

/// A snapshot of displayed results, never an evaluator environment. The source
/// signature prevents a cache from a different worksheet being reused as fresh.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SavedCalculationCache {
    pub source_signature: String,
    pub rows: Vec<SavedCalculationRow>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SavedCalculationRow {
    pub region_id: u64,
    pub status: SavedCalculationStatus,
    pub display: Option<String>,
    pub error: Option<String>,
    #[serde(default)]
    pub warnings: Vec<String>,
    pub symbolic_mathml: Option<String>,
    /// Inert precision-preserving presentation input, never worksheet code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub numeric_source: Option<SavedNumericSource>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "data",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum SavedNumericSource {
    NativeMathml(String),
    EvaluatedValue(String),
    /// JSON bundle of source-bound direct Area child presentation rows.
    AreaPresentation(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SavedCalculationStatus {
    Synchronized,
    Pending,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaperSize {
    A1,
    A2,
    A3,
    A4,
    A5,
    Letter,
    Legal,
    Ledger,
}

impl PaperSize {
    pub const ALL: [Self; 8] = [
        Self::A1,
        Self::A2,
        Self::A3,
        Self::A4,
        Self::A5,
        Self::Letter,
        Self::Legal,
        Self::Ledger,
    ];

    pub fn code(self) -> &'static str {
        match self {
            Self::A1 => "A1",
            Self::A2 => "A2",
            Self::A3 => "A3",
            Self::A4 => "A4",
            Self::A5 => "A5",
            Self::Letter => "Letter",
            Self::Legal => "Legal",
            Self::Ledger => "Ledger",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|paper| paper.code() == code)
    }

    pub fn size(self, landscape: bool) -> [f64; 2] {
        let size = match self {
            Self::A1 => [594., 841.],
            Self::A2 => [420., 594.],
            Self::A3 => [297., 420.],
            Self::A4 => [210., 297.],
            Self::A5 => [148., 210.],
            Self::Letter => [215.9, 279.4],
            Self::Legal => [215.9, 355.6],
            Self::Ledger => [279.4, 431.8],
        }
        .map(|mm| mm * 96. / 25.4);
        if landscape { [size[1], size[0]] } else { size }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PageSetup {
    pub paper: PaperSize,
    pub landscape: bool,
    /// Left, top, right, bottom in 96-DPI document coordinates.
    pub margins: [f64; 4],
}

impl PageSetup {
    pub fn validate(&self) -> Result<(), String> {
        let size = self.paper.size(self.landscape);
        if self.margins.iter().any(|m| !m.is_finite() || *m < 0.)
            || self.margins[0] + self.margins[2] >= size[0] - 1.
            || self.margins[1] + self.margins[3] >= size[1] - 1.
        {
            return Err("Page margins must leave a positive printable body".into());
        }
        Ok(())
    }
}

/// Editable content in the regular page header and footer. Prime stores these
/// as independent whiteboards, so their regions keep positions instead of
/// being reduced to left/center/right strings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PageSections {
    /// False means an imported package's page sections remain source-owned.
    /// Setting this to true makes Mathfolio serialize the fields below.
    #[serde(default)]
    pub explicit: bool,
    #[serde(default)]
    pub header: PageBand,
    #[serde(default)]
    pub footer: PageBand,
    #[serde(default = "default_starting_page")]
    pub starting_page_number: u32,
    #[serde(default)]
    pub total_pages_offset: u32,
}

impl Default for PageSections {
    fn default() -> Self {
        Self {
            explicit: false,
            header: PageBand::default(),
            footer: PageBand::default(),
            starting_page_number: default_starting_page(),
            total_pages_offset: 0,
        }
    }
}

const fn default_starting_page() -> u32 {
    1
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PageBand {
    pub regions: Vec<PageBandRegion>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PageBandRegion {
    pub id: u64,
    pub left: f64,
    pub top: f64,
    pub content: PageBandContent,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum PageBandContent {
    Text(String),
    Field(PageField),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PageField {
    PageNumber(PageNumberFormat),
    DateSaved(DateSavedFormat),
    FileName,
    FilePath,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PageNumberFormat {
    #[default]
    Number,
    PageNumber,
    AbbreviatedPageNumber,
    NumberOfTotal,
    PageNumberOfTotal,
    DashedNumber,
    ParenthesizedNumber,
    NumberParenthesizedTotal,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DateSavedFormat {
    #[default]
    MonthDayYear,
    MonthDayShortYear,
    DayMonthYear,
    DayMonthShortYear,
    YearMonthDay,
    SystemShort,
    SystemLong,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanvasRegion {
    pub id: u64,
    pub left: f64,
    pub top: f64,
    pub content: CanvasContent,
    pub source_region: Option<String>,
    /// Exact inert native XAML source, admitted against the retained archive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rich_text: Option<Box<crate::rich_text_source::RichTextWire>>,
    /// Own-authored provenance, separate from imported native authority.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authored_text: Option<Box<crate::rich_text_source::AuthoredTextWire>>,
    #[serde(default)]
    pub format: MathFormat,
    /// Prime's per-calculation-item disable-calc flag. Disabled source remains
    /// editable and printable but does not supply bindings or execute.
    #[serde(default, skip_serializing_if = "is_false")]
    pub calculation_disabled: bool,
    /// Explicitly re-enabled source whose preserved native archive may still
    /// carry disable-calc=true. Distinguishes user intent from older documents
    /// written before Mathfolio understood the original disable flag.
    #[serde(default, skip_serializing_if = "is_false")]
    pub calculation_enabled_override: bool,
}

fn is_false(value: &bool) -> bool {
    !*value
}

impl CanvasRegion {
    pub fn supports_calculation_toggle(&self) -> bool {
        matches!(
            &self.content,
            CanvasContent::Math(_) | CanvasContent::Plot(_)
        ) || matches!(&self.content, CanvasContent::Opaque { kind, .. }
                if matches!(kind.as_str(), "math" | "plot" | "include" | "mathBoard" | "specTable" | "spec-table" | "solveblock"))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MathFormat {
    #[serde(default)]
    pub explicit: bool,
    pub font_points: f32,
    pub result_format: ResultFormat,
    pub precision: u8,
    #[serde(default, skip_serializing_if = "is_false")]
    pub show_trailing_zeros: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub use_e_notation: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub use_engineering_format: bool,
    #[serde(
        default = "default_exponential_threshold",
        skip_serializing_if = "is_default_exponential_threshold"
    )]
    pub exponential_threshold: u8,
    #[serde(
        default = "default_zero_threshold",
        skip_serializing_if = "is_default_zero_threshold"
    )]
    pub zero_threshold: u16,
    #[serde(
        default = "default_complex_threshold",
        skip_serializing_if = "is_default_complex_threshold"
    )]
    pub complex_threshold: u16,
    #[serde(default, skip_serializing_if = "is_default_complex_format")]
    pub complex_format: ComplexFormat,
}

const fn default_exponential_threshold() -> u8 {
    3
}
const fn default_zero_threshold() -> u16 {
    15
}
const fn default_complex_threshold() -> u16 {
    10
}
fn is_default_exponential_threshold(value: &u8) -> bool {
    *value == default_exponential_threshold()
}
fn is_default_zero_threshold(value: &u16) -> bool {
    *value == default_zero_threshold()
}
fn is_default_complex_threshold(value: &u16) -> bool {
    *value == default_complex_threshold()
}
fn is_default_complex_format(value: &ComplexFormat) -> bool {
    *value == ComplexFormat::default()
}

impl Default for MathFormat {
    fn default() -> Self {
        Self {
            explicit: false,
            font_points: 11.,
            result_format: ResultFormat::General,
            precision: 3,
            show_trailing_zeros: false,
            use_e_notation: false,
            use_engineering_format: false,
            exponential_threshold: default_exponential_threshold(),
            zero_threshold: default_zero_threshold(),
            complex_threshold: default_complex_threshold(),
            complex_format: ComplexFormat::I,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResultFormat {
    #[default]
    General,
    Decimal,
    Scientific,
    Engineering,
    Percent,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComplexFormat {
    #[default]
    I,
    J,
    PolarRadians,
    PolarPiRadians,
    PolarDegrees,
}

#[path = "plot_expression.rs"]
mod plot_expression;
pub use plot_expression::{
    MAX_EXPRESSION_BYTES, MAX_EXPRESSION_NODES, MAX_PLOT_EXPRESSION_BYTES,
    MAX_PLOT_EXPRESSION_NODES, PlotExpression, PlotExpressionRef, PlotExpressionTree,
};

#[path = "plot_source_model.rs"]
mod plot_source_model;
pub use plot_source_model::{PlotAxisEquation, PlotAxisLists, PlotSource, PlotTraceAppearance};
#[path = "plot_scaling.rs"]
mod plot_scaling;
pub use plot_scaling::{PlotEquationRef, PlotPairRef, PlotScaling};
#[path = "plot_axis_authoring.rs"]
mod plot_axis_authoring;
pub use plot_axis_authoring::{
    PlotAxis, PlotAxisEditOutcome, PlotAxisEquationChange, PlotAxisTraceChange,
};

#[path = "plot_geometry.rs"]
mod plot_geometry;
pub use plot_geometry::{PlotGraphSize, PlotNativeGeometry};

#[derive(Clone, Debug, PartialEq)]
pub struct PlotSpec {
    pub source: PlotSource,
    pub native_geometry: Option<Box<PlotNativeGeometry>>,
    pub width: f32,
    pub height: f32,
    pub x_axis: PlotAxisSpec,
    pub y_axis: PlotAxisSpec,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlotTraceSpec {
    pub x_expression: String,
    pub y_expression: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub style: PlotTraceStyle,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "plot_scaling::present_scaling"
    )]
    pub x_scaling: Option<PlotScaling>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "plot_scaling::present_scaling"
    )]
    pub y_scaling: Option<PlotScaling>,
}

impl Default for PlotTraceSpec {
    fn default() -> Self {
        Self {
            x_expression: "x".into(),
            y_expression: "sin(x)".into(),
            x_scaling: None,
            y_scaling: None,
            label: String::new(),
            style: PlotTraceStyle::default(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlotAxisSpec {
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub range: Option<PlotRange>,
    /// None retains absent native source; effective X/Y defaults differ.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legend_position: Option<PlotLegendPosition>,
    /// Native axisLine legendWidth, never converted to a renderer pixel width.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legend_width: Option<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlotLegendPosition {
    Bottom,
    Top,
    Left,
    Right,
}

#[path = "plot_legend.rs"]
mod plot_legend;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlotRange {
    pub min: f64,
    pub max: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlotTraceStyle {
    pub color: [u8; 4],
    pub line: PlotLineStyle,
    pub line_width: f32,
    pub marker: PlotMarker,
    #[serde(default)]
    pub marker_filled: bool,
    pub marker_size: f32,
    pub marker_frequency: u32,
}

impl Default for PlotTraceStyle {
    fn default() -> Self {
        Self {
            color: [0, 70, 150, 255],
            line: PlotLineStyle::Solid,
            line_width: 1.5,
            marker: PlotMarker::None,
            marker_filled: false,
            marker_size: 3.,
            marker_frequency: 1,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlotLineStyle {
    None,
    #[default]
    Solid,
    Dotted,
    Dashed,
    DashDot,
    DashDotDot,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlotMarker {
    #[default]
    None,
    Cross,
    Plus,
    Circle,
    Square,
    Diamond,
    Point,
    Star,
    Triangle,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum CanvasContent {
    Math(PortableTree),
    Text(String),
    Plot(PlotSpec),
    Opaque { xml: String, kind: String },
}

impl Default for CanvasDocument {
    fn default() -> Self {
        Self {
            version: CANVAS_VERSION,
            title: "Untitled worksheet".into(),
            width: 21. * 96. / 2.54,
            height: 29.7 * 96. / 2.54,
            grid: 0.5 * 96. / 2.54,
            next_id: 1,
            regions: Vec::new(),
            page_sections: PageSections::default(),
            page_setup: None,
            calculation_cache: None,
            bin_oct_hex_mode: None,
            matrix_result_views: MatrixResultViews::default(),
            native_archive: None,
        }
    }
}

impl CanvasDocument {
    /// Deterministic content identity, independent of cached results, title and
    /// page headers. Includes layout because worksheet definition order is
    /// geometric, and the preserved source because native settings affect math.
    pub fn calculation_signature(&self) -> String {
        self.calculation_source_signature(false)
    }

    /// Disabled expression edits must not invalidate downstream calculation.
    /// The saved presentation signature above still includes their source so a
    /// cache cannot be mistaken for a synchronized result of the edited input.
    pub fn active_calculation_signature(&self) -> String {
        self.calculation_source_signature(true)
    }

    fn calculation_source_signature(&self, active_only: bool) -> String {
        signature_json::source_signature(self, active_only)
    }

    /// A missing or stale saved presentation is not an instruction to calculate.
    pub fn saved_calculation_cache(&self) -> Option<&SavedCalculationCache> {
        self.calculation_cache
            .as_ref()
            .filter(|cache| cache.source_signature == self.calculation_signature())
    }

    /// Adds a blank structural expression. Invalid caller-supplied coordinates
    /// remain invalid until corrected; validate/save will reject them.
    pub fn add_math(&mut self, left: f64, top: f64) -> u64 {
        self.add_region(
            left,
            top,
            CanvasContent::Math(PortableTree {
                kind: NodeKind::Placeholder,
                children: Vec::new(),
            }),
        )
    }

    pub fn add_text(&mut self, left: f64, top: f64, text: impl Into<String>) -> u64 {
        self.add_region(left, top, CanvasContent::Text(text.into()))
    }

    pub fn add_plot(&mut self, left: f64, top: f64) -> u64 {
        self.add_region(left, top, CanvasContent::Plot(PlotSpec::default()))
    }

    pub fn add_opaque(
        &mut self,
        left: f64,
        top: f64,
        xml: impl Into<String>,
        kind: impl Into<String>,
    ) -> u64 {
        self.add_region(
            left,
            top,
            CanvasContent::Opaque {
                xml: xml.into(),
                kind: kind.into(),
            },
        )
    }

    pub fn add_header_text(&mut self, left: f64, top: f64, text: impl Into<String>) -> u64 {
        self.add_page_band_region(true, left, top, PageBandContent::Text(text.into()))
    }

    pub fn add_footer_text(&mut self, left: f64, top: f64, text: impl Into<String>) -> u64 {
        self.add_page_band_region(false, left, top, PageBandContent::Text(text.into()))
    }

    pub fn add_header_field(&mut self, left: f64, top: f64, field: PageField) -> u64 {
        self.add_page_band_region(true, left, top, PageBandContent::Field(field))
    }

    pub fn add_footer_field(&mut self, left: f64, top: f64, field: PageField) -> u64 {
        self.add_page_band_region(false, left, top, PageBandContent::Field(field))
    }

    fn add_page_band_region(
        &mut self,
        header: bool,
        left: f64,
        top: f64,
        content: PageBandContent,
    ) -> u64 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        let region = PageBandRegion {
            id,
            left: snap_coordinate(left, self.grid).unwrap_or(left),
            top: snap_coordinate(top, self.grid).unwrap_or(top),
            content,
        };
        if header {
            self.page_sections.header.regions.push(region);
        } else {
            self.page_sections.footer.regions.push(region);
        }
        self.page_sections.explicit = true;
        id
    }

    fn add_region(&mut self, left: f64, top: f64, content: CanvasContent) -> u64 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        self.regions.push(CanvasRegion {
            id,
            left: snap_coordinate(left, self.grid).unwrap_or(left),
            top: snap_coordinate(top, self.grid).unwrap_or(top),
            content,
            source_region: None,
            rich_text: None,
            authored_text: None,
            format: MathFormat::default(),
            calculation_disabled: false,
            calculation_enabled_override: false,
        });
        id
    }

    /// Snaps both coordinates before mutating the region, so an invalid move
    /// never leaves half an update behind. Vector order is not evaluation order.
    pub fn move_region(&mut self, id: u64, left: f64, top: f64) -> Result<(), String> {
        let left = snap_coordinate(left, self.grid)?;
        let top = snap_coordinate(top, self.grid)?;
        let region = self
            .regions
            .iter_mut()
            .find(|r| r.id == id)
            .ok_or_else(|| format!("Unknown canvas region {id}"))?;
        region.left = left;
        region.top = top;
        Ok(())
    }

    /// Loads only Mathfolio v2. Mathcad import is a separate source-preserving
    /// conversion path; this method never opens external package relationships.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn load(path: &Path) -> Result<Self, String> {
        check_extension(path)?;
        let mut file = File::open(path).map_err(|e| e.to_string())?;
        if file.metadata().map_err(|e| e.to_string())?.len() > MAX_DOCUMENT_BYTES {
            return Err("Canvas document exceeds byte limit".into());
        }
        let mut bytes = Vec::new();
        Read::by_ref(&mut file)
            .take(MAX_DOCUMENT_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() as u64 > MAX_DOCUMENT_BYTES {
            return Err("Canvas document exceeds byte limit".into());
        }
        Self::from_bytes(&bytes)
    }

    /// Same validation and legacy restoration as file loading; no OS IO.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() as u64 > MAX_DOCUMENT_BYTES {
            return Err("Canvas document exceeds byte limit".into());
        }
        let mut doc: Self = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        doc.validate()?;
        doc.restore_legacy_native_disabled_regions();
        Ok(doc)
    }

    fn restore_legacy_native_disabled_regions(&mut self) {
        if !self.regions.iter().any(|r| {
            !r.calculation_disabled
                && !r.calculation_enabled_override
                && r.source_region.is_some()
                && r.supports_calculation_toggle()
        }) {
            return;
        }
        let Some(bytes) = &self.native_archive else {
            return;
        };
        let Ok(package) = crate::native::NativePackage::from_bytes(bytes) else {
            return;
        };
        let Ok(source) = package.worksheet_xml() else {
            return;
        };
        let Ok(xml) = roxmltree::Document::parse(&source) else {
            return;
        };
        const WS: &str = "http://schemas.mathsoft.com/worksheet50";
        let Some(regions) = xml
            .root_element()
            .children()
            .find(|n| n.has_tag_name((WS, "regions")))
        else {
            return;
        };
        let disabled: HashSet<_> = regions
            .children()
            .filter(|n| n.has_tag_name((WS, "region")))
            .filter(|region| {
                region.children().any(|n| {
                    n.tag_name().namespace() == Some(WS)
                        && matches!(
                            n.tag_name().name(),
                            "math"
                                | "plot"
                                | "include"
                                | "mathBoard"
                                | "specTable"
                                | "spec-table"
                                | "solveblock"
                        )
                        && matches!(n.attribute("disable-calc"), Some("true" | "1"))
                })
            })
            .filter_map(|n| n.attribute("region-id"))
            .collect();
        let cache_was_valid = self.saved_calculation_cache().is_some();
        let mut changed = false;
        for region in &mut self.regions {
            if !region.calculation_disabled
                && !region.calculation_enabled_override
                && region.supports_calculation_toggle()
                && region
                    .source_region
                    .as_deref()
                    .is_some_and(|id| disabled.contains(id))
            {
                region.calculation_disabled = true;
                changed = true;
            }
        }
        if changed && cache_was_valid {
            let signature = self.calculation_signature();
            if let Some(cache) = &mut self.calculation_cache {
                cache.source_signature = signature;
                for row in &mut cache.rows {
                    row.status = SavedCalculationStatus::Pending;
                }
            }
        }
    }

    /// Validates and writes a new sibling temporary file, then atomically
    /// replaces the destination. Failed validation/write/rename preserves the
    /// old destination; an incomplete temporary file is removed on failure.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn save(&self, path: &Path) -> Result<(), String> {
        check_extension(path)?;
        let bytes = self.to_bytes()?;
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let name = path
            .file_name()
            .ok_or("Canvas output file name is missing")?;
        static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos();
        let mut temp_name = name.to_os_string();
        temp_name.push(format!(
            ".{}-{stamp}-{}.tmp",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        let temp = parent.join(temp_name);
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|e| e.to_string())?;
        let written = file.write_all(&bytes).and_then(|_| file.sync_all());
        drop(file);
        if let Err(error) = written {
            let _ = std::fs::remove_file(&temp);
            return Err(error.to_string());
        }
        if let Err(error) = std::fs::rename(&temp, path) {
            let _ = std::fs::remove_file(&temp);
            return Err(error.to_string());
        }
        Ok(())
    }

    /// Compact own-format bytes, with the same bounds as native Save.
    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        // Compact JSON bounds the overhead of the preserved byte array.
        let bytes = serde_json::to_vec(self).map_err(|e| e.to_string())?;
        if bytes.len() as u64 > MAX_DOCUMENT_BYTES {
            return Err("Serialized canvas document exceeds byte limit".into());
        }
        Ok(bytes)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.version != CANVAS_VERSION {
            return Err(format!(
                "Unsupported canvas document version {}",
                self.version
            ));
        }
        if !self.width.is_finite()
            || !self.height.is_finite()
            || !(1.0..=MAX_DIMENSION).contains(&self.width)
            || !(1.0..=MAX_DIMENSION).contains(&self.height)
        {
            return Err("Canvas dimensions must be finite positive sizes within limits".into());
        }
        if !self.grid.is_finite() || self.grid <= 0.0 || self.grid > MAX_DIMENSION {
            return Err("Canvas grid must be finite and positive within limits".into());
        }
        if let Some(setup) = &self.page_setup {
            setup.validate()?;
            let size = setup.paper.size(setup.landscape);
            if (self.width - size[0]).abs() > 0.01 || (self.height - size[1]).abs() > 0.01 {
                return Err("Page setup and document paper dimensions disagree".into());
            }
        }
        let page_region_count = self
            .page_sections
            .header
            .regions
            .len()
            .saturating_add(self.page_sections.footer.regions.len());
        if self.title.len() > 1024
            || self.regions.len().saturating_add(page_region_count) > MAX_REGIONS
        {
            return Err("Canvas title or region count exceeds limits".into());
        }
        if self
            .native_archive
            .as_ref()
            .is_some_and(|b| b.len() > MAX_NATIVE_ARCHIVE_BYTES)
        {
            return Err("Preserved native package exceeds 16 MiB".into());
        }
        let mut ids = HashSet::new();
        let mut text_bytes = self.title.len();
        let mut tree_nodes = 0usize;
        for region in &self.regions {
            if region.id == 0 || region.id >= self.next_id || !ids.insert(region.id) {
                return Err("Canvas region IDs must be unique, nonzero and below next_id".into());
            }
            check_coordinate(region.left)?;
            check_coordinate(region.top)?;
            if !region.format.font_points.is_finite()
                || !(1. ..=200.).contains(&region.format.font_points)
                || region.format.precision > 15
                || region.format.exponential_threshold > 17
                || region.format.zero_threshold > 307
                || region.format.complex_threshold > 307
            {
                return Err("Region font must be 1–200 points, precision at most 15, exponential threshold at most 17 and zero/complex thresholds at most 307".into());
            }
            if let Some(source) = &region.source_region {
                if source.len() > 1024 {
                    return Err("Source region reference exceeds byte limit".into());
                }
                text_bytes = text_bytes.saturating_add(source.len());
            }
            if let Some(source) = region.validated_authored_text()? {
                // Charge the complete serialized own wire once PER owner,
                // including seed/catalog and JSON escapes, plus plain below.
                text_bytes = text_bytes.saturating_add(source.budget().serialized_wire_bytes());
            }
            if let Some(source) = &region.rich_text {
                if !matches!(region.content, CanvasContent::Text(_)) {
                    return Err("Rich Text source requires a Text region".into());
                }
                if source.source_bytes() > crate::rich_text_source::MAX_SOURCE_BYTES {
                    return Err("Rich Text source exceeds 1 MiB".into());
                }
                if self.native_archive.is_none() {
                    return Err("Rich Text source requires its preserved native package".into());
                }
                text_bytes = text_bytes.saturating_add(source.source_bytes());
            }
            if region.calculation_disabled && !region.supports_calculation_toggle() {
                return Err("Calculation can only be disabled for calculation regions".into());
            }
            if region.calculation_enabled_override
                && (region.calculation_disabled || !region.supports_calculation_toggle())
            {
                return Err(
                    "Explicit calculation re-enable state is invalid for this region".into(),
                );
            }
            match &region.content {
                CanvasContent::Math(tree) => {
                    tree.validate()
                        .map_err(|e| format!("Region {}: {e}", region.id))?;
                    let mut stack = vec![(tree, 1)];
                    while let Some((node, depth)) = stack.pop() {
                        tree_nodes += 1;
                        if tree_nodes > MAX_TREE_NODES_TOTAL || depth > MAX_CANVAS_TREE_DEPTH {
                            return Err("Canvas expression complexity exceeds limits".into());
                        }
                        text_bytes =
                            text_bytes.saturating_add(source_bytes::node_text_bytes(&node.kind));
                        stack.extend(node.children.iter().map(|n| (n, depth + 1)));
                    }
                }
                CanvasContent::Text(text) => {
                    if text.len() > 1024 * 1024 {
                        return Err("Canvas text region exceeds 1 MiB".into());
                    }
                    text_bytes = text_bytes.saturating_add(text.len());
                }
                CanvasContent::Plot(spec) => {
                    spec.validate()?;
                    text_bytes = text_bytes
                        .saturating_add(spec.source_text_bytes())
                        .saturating_add(spec.x_axis.label.len())
                        .saturating_add(spec.y_axis.label.len());
                }
                CanvasContent::Opaque { xml, kind } => {
                    if kind.is_empty() || kind.len() > 128 || xml.len() > MAX_TEXT_BYTES {
                        return Err("Opaque region type or XML exceeds limits".into());
                    }
                    text_bytes = text_bytes
                        .saturating_add(xml.len())
                        .saturating_add(kind.len());
                }
            }
            if text_bytes > MAX_TEXT_BYTES {
                return Err("Canvas text exceeds total 8 MiB budget".into());
            }
        }
        if self.page_sections.starting_page_number == 0 {
            return Err("Starting page number must be positive".into());
        }
        for region in self
            .page_sections
            .header
            .regions
            .iter()
            .chain(self.page_sections.footer.regions.iter())
        {
            if region.id == 0 || region.id >= self.next_id || !ids.insert(region.id) {
                return Err("Canvas region IDs must be unique, nonzero and below next_id".into());
            }
            check_coordinate(region.left)?;
            check_coordinate(region.top)?;
            if let PageBandContent::Text(text) = &region.content {
                if text.len() > 1024 * 1024 {
                    return Err("Header or footer text region exceeds 1 MiB".into());
                }
                text_bytes = text_bytes.saturating_add(text.len());
                if text_bytes > MAX_TEXT_BYTES {
                    return Err("Canvas text exceeds total 8 MiB budget".into());
                }
            }
        }
        if self.next_id == 0 {
            return Err("Canvas next_id must be nonzero".into());
        }
        if self.regions.iter().any(|region| region.rich_text.is_some()) {
            let package = crate::native::NativePackage::from_bytes(
                self.native_archive
                    .as_deref()
                    .ok_or("Missing rich Text native source")?,
            )?;
            // XAML bytes were counted once in the ordinary source budget above.
            // This shared pure admission validates actual retained origin/plain.
            crate::export::validate_imported_rich_text_carriers(self, &package)?;
        }
        if let Some(cache) = &self.calculation_cache {
            if cache.source_signature.len() != 64
                || !cache
                    .source_signature
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit())
                || cache.rows.len() > MAX_REGIONS
            {
                return Err("Saved calculation cache signature or row count is invalid".into());
            }
            let mut cached_ids = HashSet::new();
            let mut cached_bytes = 0usize;
            for row in &cache.rows {
                if !self.regions.iter().any(|region| region.id == row.region_id)
                    || !cached_ids.insert(row.region_id)
                {
                    return Err(
                        "Saved calculation rows must refer to unique existing regions".into(),
                    );
                }
                if row.error.is_some()
                    && (row.display.is_some()
                        || row.symbolic_mathml.is_some()
                        || row.numeric_source.is_some())
                {
                    return Err(
                        "Saved calculation row cannot contain both a result and an error".into(),
                    );
                }
                if row.warnings.len() > 1024 {
                    return Err("Saved calculation warnings exceed limits".into());
                }
                if let Some(source) = &row.numeric_source {
                    let data = match source {
                        SavedNumericSource::NativeMathml(data)
                        | SavedNumericSource::EvaluatedValue(data)
                        | SavedNumericSource::AreaPresentation(data) => data,
                    };
                    if data.len() > 4 * 1024 * 1024 {
                        return Err("Saved numeric source exceeds 4 MiB".into());
                    }
                    cached_bytes = cached_bytes.saturating_add(data.len());
                    if cached_bytes > MAX_TEXT_BYTES {
                        return Err("Saved calculation presentation exceeds 8 MiB".into());
                    }
                }
                if let Some(SavedNumericSource::AreaPresentation(data)) = &row.numeric_source {
                    let owner = self
                        .regions
                        .iter()
                        .find(|region| region.id == row.region_id)
                        .expect("Saved row owner was checked above");
                    area_cache::validate(owner, row, data)?;
                }
                for text in row
                    .display
                    .iter()
                    .chain(row.error.iter())
                    .chain(row.symbolic_mathml.iter())
                    .chain(row.warnings.iter())
                {
                    cached_bytes = cached_bytes.saturating_add(text.len());
                    if cached_bytes > MAX_TEXT_BYTES {
                        return Err("Saved calculation presentation exceeds 8 MiB".into());
                    }
                }
            }
        }
        self.matrix_result_views
            .validate_for_regions(&self.regions)?;
        Ok(())
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn check_extension(path: &Path) -> Result<(), String> {
    if path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(crate::is_own_extension)
    {
        Ok(())
    } else {
        Err("Canvas documents require the .mathfolio extension".into())
    }
}

fn check_coordinate(value: f64) -> Result<(), String> {
    if value.is_finite() && value.abs() <= MAX_DIMENSION {
        Ok(())
    } else {
        Err("Canvas coordinates must be finite and within limits".into())
    }
}

/// Project canvas policy: nearest grid point, midpoint ties towards the smaller
/// coordinate. Negative coordinates use that same policy; this does not claim
/// to reproduce Mathcad's negative-coordinate clipping or restricted areas.
pub fn snap_coordinate(value: f64, grid: f64) -> Result<f64, String> {
    check_coordinate(value)?;
    if !grid.is_finite() || grid <= 0.0 || grid > MAX_DIMENSION {
        return Err("Canvas grid must be finite and positive within limits".into());
    }
    let remainder = value.rem_euclid(grid);
    let offset = if remainder > grid / 2.0 {
        grid - remainder
    } else {
        -remainder
    };
    let snapped = value + offset;
    check_coordinate(snapped)?;
    Ok(snapped)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestDir(std::path::PathBuf);
    impl TestDir {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "mathfolio-canvas-{}-{stamp}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn path(&self, name: &str) -> std::path::PathBuf {
            self.0.join(name)
        }
    }
    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn atom(kind: NodeKind) -> PortableTree {
        PortableTree {
            kind,
            children: Vec::new(),
        }
    }

    fn cached_row(region_id: u64) -> SavedCalculationRow {
        SavedCalculationRow {
            region_id,
            status: SavedCalculationStatus::Synchronized,
            display: Some("5 cm".into()),
            error: None,
            warnings: vec!["saved redefinition warning".into()],
            symbolic_mathml: None,
            numeric_source: None,
        }
    }

    #[test]
    fn extended_math_format_defaults_preserve_old_serialized_shape() {
        let old =
            r#"{"explicit":false,"font_points":11.0,"result_format":"General","precision":3}"#;
        let restored: MathFormat = serde_json::from_str(old).unwrap();
        assert_eq!(restored, MathFormat::default());
        assert_eq!(serde_json::to_string(&restored).unwrap(), old);
        let nondefault = MathFormat {
            show_trailing_zeros: true,
            use_e_notation: true,
            use_engineering_format: true,
            exponential_threshold: 5,
            zero_threshold: 12,
            complex_threshold: 7,
            complex_format: ComplexFormat::PolarDegrees,
            ..restored
        };
        assert_eq!(
            serde_json::from_str::<MathFormat>(&serde_json::to_string(&nondefault).unwrap())
                .unwrap(),
            nondefault
        );
    }

    #[test]
    fn saved_numeric_sources_are_optional_inert_bounded_presentation_inputs() {
        let mut document = CanvasDocument::default();
        let region_id = document.add_math(0., 0.);
        let mut row = cached_row(region_id);
        let old = serde_json::to_value(&row).unwrap();
        assert!(old.get("numeric_source").is_none());
        assert_eq!(
            serde_json::from_value::<SavedCalculationRow>(old).unwrap(),
            row
        );
        row.numeric_source = Some(SavedNumericSource::NativeMathml(
            "<ml:real>0.94359</ml:real>".into(),
        ));
        assert_eq!(
            serde_json::from_str::<SavedCalculationRow>(&serde_json::to_string(&row).unwrap())
                .unwrap(),
            row
        );
        document.calculation_cache = Some(SavedCalculationCache {
            source_signature: document.calculation_signature(),
            rows: vec![row],
        });
        document.validate().unwrap();
        let cache = document.calculation_cache.as_mut().unwrap();
        cache.rows[0].numeric_source = Some(SavedNumericSource::EvaluatedValue(
            "x".repeat(4 * 1024 * 1024),
        ));
        document.validate().unwrap();
        let cache = document.calculation_cache.as_mut().unwrap();
        cache.rows[0].numeric_source = Some(SavedNumericSource::EvaluatedValue(
            "x".repeat(4 * 1024 * 1024 + 1),
        ));
        assert!(document.validate().unwrap_err().contains("4 MiB"));
    }

    #[test]
    fn saved_cache_and_native_fractional_coordinates_survive_json_file_roundtrip_exactly() {
        let dir = TestDir::new();
        let path = dir.path("native-fractional-cache.mathfolio");
        let mut document = CanvasDocument::default();
        // Actual native worksheet coordinates whose default JSON parsing was
        // one ULP away from the serialized float, invalidating our own cache.
        let positions = [
            907.0866141732283,
            1285.0393700787401,
            1530.7086614173227,
            1662.9921259842517,
            1965.3543307086613,
        ];
        let mut rows = Vec::new();
        for coordinate in positions {
            let id = document.add_math(0., 0.);
            let region = document.regions.last_mut().unwrap();
            region.left = coordinate;
            region.top = coordinate;
            rows.push(cached_row(id));
        }
        let signature = document.calculation_signature();
        let active_signature = document.active_calculation_signature();
        document.calculation_cache = Some(SavedCalculationCache {
            source_signature: signature.clone(),
            rows,
        });
        document.save(&path).unwrap();
        let reopened = CanvasDocument::load(&path).unwrap();
        for (before, after) in document.regions.iter().zip(&reopened.regions) {
            assert_eq!(before.left.to_bits(), after.left.to_bits());
            assert_eq!(before.top.to_bits(), after.top.to_bits());
        }
        assert_eq!(reopened.calculation_signature(), signature);
        assert_eq!(reopened.active_calculation_signature(), active_signature);
        assert_eq!(
            reopened.saved_calculation_cache(),
            document.saved_calculation_cache()
        );
        assert_eq!(reopened, document);
    }

    #[test]
    fn saved_calculation_presentation_roundtrips_without_evaluation() {
        let dir = TestDir::new();
        let path = dir.path("saved-results.mathfolio");
        let mut document = CanvasDocument::default();
        // An intentionally blank formula cannot independently evaluate to 5 cm.
        let id = document.add_math(0., 0.);
        let error_id = document.add_math(0., 30.);
        let mut error = cached_row(error_id);
        error.display = None;
        error.error = Some("saved error message".into());
        error.status = SavedCalculationStatus::Pending;
        document.calculation_cache = Some(SavedCalculationCache {
            source_signature: document.calculation_signature(),
            rows: vec![cached_row(id), error],
        });
        document.save(&path).unwrap();
        let loaded = CanvasDocument::load(&path).unwrap();
        let cache = loaded.saved_calculation_cache().unwrap();
        assert_eq!(cache.rows[0].display.as_deref(), Some("5 cm"));
        assert_eq!(cache.rows[0].warnings, ["saved redefinition warning"]);
        assert_eq!(cache.rows[1].error.as_deref(), Some("saved error message"));
        assert_eq!(cache.rows[1].status, SavedCalculationStatus::Pending);
        assert_eq!(loaded, document);
    }

    #[test]
    fn page_setup_roundtrip_keeps_paper_orientation_and_validated_margins() {
        for paper in PaperSize::ALL {
            for landscape in [false, true] {
                let [width, height] = paper.size(landscape);
                let setup = PageSetup {
                    paper,
                    landscape,
                    margins: [24., 36., 48., 60.],
                };
                let doc = CanvasDocument {
                    width,
                    height,
                    page_setup: Some(setup.clone()),
                    ..Default::default()
                };
                doc.validate().unwrap();
                let restored: CanvasDocument =
                    serde_json::from_str(&serde_json::to_string(&doc).unwrap()).unwrap();
                assert_eq!(restored.page_setup, Some(setup));
                assert_eq!(PaperSize::from_code(paper.code()), Some(paper));
            }
        }
        let mut doc = CanvasDocument::default();
        let setup = PageSetup {
            paper: PaperSize::A4,
            landscape: false,
            margins: [f64::NAN, 0., 0., 0.],
        };
        assert!(setup.validate().is_err());
        doc.page_setup = Some(PageSetup {
            margins: [500.; 4],
            ..setup.clone()
        });
        assert!(doc.validate().is_err());
        doc.page_setup = Some(PageSetup {
            margins: [30.; 4],
            landscape: true,
            ..setup
        });
        assert!(
            doc.validate().is_err(),
            "dimensions and orientation cannot disagree"
        );
    }

    #[test]
    fn saved_cache_signature_tracks_math_layout_and_source_but_not_presentation_cache() {
        let mut document = CanvasDocument::default();
        let id = document.add_math(0., 0.);
        let signature = document.calculation_signature();
        document.calculation_cache = Some(SavedCalculationCache {
            source_signature: signature.clone(),
            rows: vec![cached_row(id)],
        });
        document.title = "Renamed".into();
        document.add_header_text(0., 0., "Page header");
        document.add_text(20., 30., "Comment without calculation semantics");
        assert_eq!(document.calculation_signature(), signature);
        assert!(document.saved_calculation_cache().is_some());
        for changed in [
            {
                let mut copy = document.clone();
                copy.regions[0].top += 10.;
                copy
            },
            {
                let mut copy = document.clone();
                copy.regions[0].content = CanvasContent::Math(atom(NodeKind::Number("8".into())));
                copy
            },
            {
                let mut copy = document.clone();
                copy.regions[0].format.precision = 8;
                copy
            },
            {
                let mut copy = document.clone();
                copy.native_archive = Some(vec![1, 2]);
                copy
            },
        ] {
            assert!(changed.saved_calculation_cache().is_none());
            // A stale cache is harmless preserved data, not a corrupt document.
            changed.validate().unwrap();
        }
    }

    #[test]
    fn earlier_v2_documents_have_no_implicit_saved_results() {
        let mut json = serde_json::to_value(CanvasDocument::default()).unwrap();
        json.as_object_mut().unwrap().remove("calculation_cache");
        let loaded: CanvasDocument = serde_json::from_value(json).unwrap();
        assert!(loaded.saved_calculation_cache().is_none());
        loaded.validate().unwrap();
    }

    #[test]
    fn saved_presentation_rejects_conflicting_or_unbound_rows_and_bounded_payloads() {
        let mut document = CanvasDocument::default();
        let id = document.add_math(0., 0.);
        document.calculation_cache = Some(SavedCalculationCache {
            source_signature: document.calculation_signature(),
            rows: vec![cached_row(id)],
        });
        document.validate().unwrap();
        let valid = document.clone();
        document
            .calculation_cache
            .as_mut()
            .unwrap()
            .rows
            .push(cached_row(id));
        assert!(document.validate().unwrap_err().contains("unique existing"));
        document = valid.clone();
        document.calculation_cache.as_mut().unwrap().rows[0].region_id = id + 1;
        assert!(document.validate().unwrap_err().contains("unique existing"));
        document = valid.clone();
        document.calculation_cache.as_mut().unwrap().rows[0].error = Some("bad".into());
        assert!(
            document
                .validate()
                .unwrap_err()
                .contains("both a result and an error")
        );
        document = valid.clone();
        document
            .calculation_cache
            .as_mut()
            .unwrap()
            .source_signature = "invalid".into();
        assert!(document.validate().unwrap_err().contains("signature"));
        document = valid;
        document.calculation_cache.as_mut().unwrap().rows[0].display =
            Some("x".repeat(MAX_TEXT_BYTES + 1));
        assert!(document.validate().unwrap_err().contains("8 MiB"));
    }

    #[test]
    fn geometry_tree_opaque_content_and_native_bytes_survive_save_load() {
        let dir = TestDir::new();
        let path = dir.path("canvas.mathfolio");
        let mut doc = CanvasDocument::default();
        assert!(doc.regions.is_empty());
        let math = doc.add_math(21.0, 58.0);
        let text = doc.add_text(300.0, 97.0, "한글 메모");
        doc.regions[0].content = CanvasContent::Math(PortableTree {
            kind: NodeKind::Fraction,
            children: vec![
                atom(NodeKind::AnnotatedIdentifier {
                    name: "ωᵣ".into(),
                    labels: "VARIABLE".into(),
                }),
                atom(NodeKind::Number("2".into())),
            ],
        });
        doc.regions[0].source_region = Some("/mathcad/worksheet.xml#region=42".into());
        doc.regions.push(CanvasRegion {
            id: doc.next_id,
            left: 59.25,
            top: 100.125,
            content: CanvasContent::Opaque {
                xml: "<future xmlns='urn:unknown' flag=' untouched '/>".into(),
                kind: "plot3d".into(),
            },
            source_region: Some("native:88".into()),
            rich_text: None,
            authored_text: None,
            format: MathFormat::default(),
            calculation_disabled: false,
            calculation_enabled_override: false,
        });
        doc.next_id += 1;
        doc.native_archive = Some(vec![b'P', b'K', 3, 4, 0, 255, 17, 0]);
        doc.move_region(math, 61.0, 139.0).unwrap();
        doc.move_region(text, 253.0, 178.0).unwrap();
        doc.save(&path).unwrap();
        let saved = std::fs::read_to_string(&path).unwrap();
        assert!(!saved.contains("undo"));
        assert!(!saved.contains("caret"));
        assert!(!saved.contains("arena"));
        assert_eq!(CanvasDocument::load(&path).unwrap(), doc);
        let next = doc.add_math(0.0, 0.0);
        assert!(next > math && next > text);
        doc.title = "Updated".into();
        doc.save(&path).unwrap();
        assert_eq!(CanvasDocument::load(&path).unwrap(), doc);
    }

    #[test]
    fn midpoint_snaps_down_and_failed_move_is_atomic() {
        assert_eq!(snap_coordinate(10.0, 4.0).unwrap(), 8.0);
        assert_eq!(snap_coordinate(10.1, 4.0).unwrap(), 12.0);
        assert_eq!(snap_coordinate(-10.0, 4.0).unwrap(), -12.0);
        let mut doc = CanvasDocument::default();
        let id = doc.add_math(19.2, 38.4);
        let before = doc.clone();
        assert!(doc.move_region(id, 200.0, f64::NAN).is_err());
        assert_eq!(doc, before);
        assert!(doc.move_region(999, 20.0, 20.0).is_err());
        assert_eq!(doc, before);
    }

    #[test]
    fn positioned_page_sections_persist_and_old_v2_files_get_source_owned_defaults() {
        let dir = TestDir::new();
        let path = dir.path("page-sections.mathfolio");
        let mut doc = CanvasDocument::default();
        let header = doc.add_header_text(20.0, 10.0, "Project Atlas");
        let footer = doc.add_footer_field(
            500.0,
            10.0,
            PageField::PageNumber(PageNumberFormat::PageNumberOfTotal),
        );
        doc.page_sections.starting_page_number = 7;
        doc.page_sections.total_pages_offset = 2;
        doc.save(&path).unwrap();
        let loaded = CanvasDocument::load(&path).unwrap();
        assert_eq!(loaded, doc);
        assert!(loaded.page_sections.explicit);
        assert!(footer > header);

        let mut value = serde_json::to_value(&doc).unwrap();
        value.as_object_mut().unwrap().remove("page_sections");
        std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        let legacy = CanvasDocument::load(&path).unwrap();
        assert_eq!(legacy.page_sections, PageSections::default());
        assert!(!legacy.page_sections.explicit);
    }

    #[test]
    fn page_section_ids_coordinates_numbering_and_text_share_document_limits() {
        let mut doc = CanvasDocument::default();
        let main = doc.add_text(0.0, 0.0, "body");
        let header = doc.add_header_text(0.0, 0.0, "header");
        assert!(header > main);
        assert!(doc.validate().is_ok());

        doc.page_sections.starting_page_number = 0;
        assert!(doc.validate().is_err());
        doc.page_sections.starting_page_number = 1;
        doc.page_sections.header.regions[0].id = main;
        assert!(doc.validate().is_err());
        doc.page_sections.header.regions[0].id = header;
        doc.page_sections.header.regions[0].left = f64::NAN;
        assert!(doc.validate().is_err());
    }

    #[test]
    fn invalid_save_does_not_replace_previous_document() {
        let dir = TestDir::new();
        let path = dir.path("safe.mathfolio");
        let mut doc = CanvasDocument::default();
        doc.add_text(0.0, 0.0, "original");
        doc.save(&path).unwrap();
        let before = std::fs::read(&path).unwrap();
        doc.regions[0].top = f64::INFINITY;
        assert!(doc.save(&path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert_eq!(std::fs::read_dir(&dir.0).unwrap().count(), 1);
        assert!(doc.save(&dir.path("original.mcdx")).is_err());
    }

    #[test]
    fn rename_failure_removes_only_temporary_output() {
        let dir = TestDir::new();
        let destination = dir.path("occupied.mathfolio");
        std::fs::create_dir(&destination).unwrap();
        std::fs::write(destination.join("keep"), b"unchanged").unwrap();
        assert!(CanvasDocument::default().save(&destination).is_err());
        assert_eq!(
            std::fs::read(destination.join("keep")).unwrap(),
            b"unchanged"
        );
        assert_eq!(std::fs::read_dir(&dir.0).unwrap().count(), 1);
    }

    #[test]
    fn rejects_wrong_version_duplicate_ids_invalid_trees_and_oversized_payloads() {
        let mut doc = CanvasDocument::default();
        doc.add_math(0.0, 0.0);
        doc.regions.push(doc.regions[0].clone());
        assert!(doc.validate().is_err());
        doc.regions.pop();
        doc.regions[0].content = CanvasContent::Math(PortableTree {
            kind: NodeKind::Fraction,
            children: Vec::new(),
        });
        assert!(doc.validate().is_err());
        doc.regions[0].content = CanvasContent::Text("x".repeat(1024 * 1024 + 1));
        assert!(doc.validate().is_err());
        doc.regions.clear();
        doc.version = 1;
        assert!(doc.validate().is_err());
        doc.version = 2;
        doc.native_archive = Some(vec![0; MAX_NATIVE_ARCHIVE_BYTES + 1]);
        assert!(doc.validate().is_err());
        doc.native_archive = None;
        doc.grid = 0.0;
        assert!(doc.validate().is_err());
    }

    #[test]
    fn supported_deep_tree_roundtrips_and_unknown_serialized_state_is_rejected() {
        let dir = TestDir::new();
        let path = dir.path("tree.mathfolio");
        let mut tree = atom(NodeKind::Placeholder);
        for _ in 1..MAX_CANVAS_TREE_DEPTH {
            tree = PortableTree {
                kind: NodeKind::Group,
                children: vec![tree],
            };
        }
        let mut doc = CanvasDocument::default();
        doc.add_math(0.0, 0.0);
        doc.regions[0].content = CanvasContent::Math(tree);
        doc.save(&path).unwrap();
        assert_eq!(CanvasDocument::load(&path).unwrap(), doc);
        let mut json = serde_json::to_value(&doc).unwrap();
        json["undo"] = serde_json::json!([]);
        std::fs::write(&path, serde_json::to_vec(&json).unwrap()).unwrap();
        assert!(CanvasDocument::load(&path).is_err());
    }

    #[test]
    fn plot_and_region_format_persist_and_old_v2_regions_get_defaults() {
        let mut doc = CanvasDocument::default();
        doc.add_plot(10., 20.);
        doc.regions[0].format = MathFormat {
            explicit: true,
            font_points: 18.,
            result_format: ResultFormat::Engineering,
            precision: 7,
            ..Default::default()
        };
        let mut value = serde_json::to_value(&doc).unwrap();
        let restored: CanvasDocument = serde_json::from_value(value.clone()).unwrap();
        restored.validate().unwrap();
        assert_eq!(restored, doc);
        value["regions"][0]
            .as_object_mut()
            .unwrap()
            .remove("format");
        let plot = value["regions"][0]["content"]["Plot"]
            .as_object_mut()
            .unwrap();
        plot.remove("traces");
        plot.remove("x_axis");
        plot.remove("y_axis");
        let old: CanvasDocument = serde_json::from_value(value).unwrap();
        assert_eq!(old.regions[0].format, MathFormat::default());
        let CanvasContent::Plot(old_plot) = &old.regions[0].content else {
            unreachable!()
        };
        assert!(old_plot.legacy_traces().unwrap().is_empty());
        assert_eq!(old_plot.x_axis, PlotAxisSpec::default());
        old.validate().unwrap();
        let dir = TestDir::new();
        let path = dir.path("plot.mathfolio");
        doc.save(&path).unwrap();
        assert_eq!(CanvasDocument::load(&path).unwrap(), doc);
    }

    #[test]
    fn plot_and_format_limits_are_checked_before_saving() {
        let mut doc = CanvasDocument::default();
        doc.add_plot(0., 0.);
        for points in [0., 201., f32::NAN] {
            doc.regions[0].format.font_points = points;
            assert!(doc.validate().is_err());
        }
        doc.regions[0].format = MathFormat::default();
        doc.regions[0].format.precision = 16;
        assert!(doc.validate().is_err());
        doc.regions[0].format.precision = 15;
        doc.validate().unwrap();
        doc.regions[0].format.exponential_threshold = 18;
        assert!(doc.validate().is_err());
        doc.regions[0].format.exponential_threshold = 17;
        doc.regions[0].format.zero_threshold = 308;
        assert!(doc.validate().is_err());
        doc.regions[0].format.zero_threshold = 307;
        doc.regions[0].format.complex_threshold = 308;
        assert!(doc.validate().is_err());
        doc.regions[0].format.complex_threshold = 307;
        doc.validate().unwrap();
        let CanvasContent::Plot(spec) = &mut doc.regions[0].content else {
            unreachable!()
        };
        spec.width = 119.;
        assert!(doc.validate().is_err());
        let CanvasContent::Plot(spec) = &mut doc.regions[0].content else {
            unreachable!()
        };
        spec.width = 420.;
        *spec.primary_y_expression_mut().unwrap() = "x".repeat(4097);
        assert!(doc.validate().is_err());
    }

    #[test]
    fn multi_trace_axis_ranges_and_styles_validate_and_roundtrip() {
        let mut doc = CanvasDocument::default();
        doc.add_plot(0., 0.);
        let CanvasContent::Plot(spec) = &mut doc.regions[0].content else {
            unreachable!()
        };
        spec.promote_legacy_trace();
        spec.legacy_traces_mut().unwrap()[0].label = "sine".into();
        spec.legacy_traces_mut().unwrap()[0].style = PlotTraceStyle {
            color: [210, 40, 60, 255],
            line: PlotLineStyle::Dashed,
            marker: PlotMarker::Circle,
            marker_filled: true,
            marker_frequency: 2,
            ..Default::default()
        };
        spec.add_trace(PlotTraceSpec {
            y_expression: "cos(x)".into(),
            label: "cosine".into(),
            ..Default::default()
        })
        .unwrap();
        spec.x_axis = PlotAxisSpec {
            label: "time".into(),
            range: Some(PlotRange { min: -2., max: 2. }),
            ..Default::default()
        };
        spec.y_axis.label = "amplitude".into();
        doc.validate().unwrap();
        let encoded = serde_json::to_vec(&doc).unwrap();
        let decoded: CanvasDocument = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(decoded, doc);
        let mut without_fill = serde_json::to_value(&doc).unwrap();
        without_fill["regions"][0]["content"]["Plot"]["traces"][0]["style"]
            .as_object_mut()
            .unwrap()
            .remove("marker_filled");
        let without_fill: CanvasDocument = serde_json::from_value(without_fill).unwrap();
        let CanvasContent::Plot(without_fill) = &without_fill.regions[0].content else {
            unreachable!()
        };
        assert!(!without_fill.legacy_traces().unwrap()[0].style.marker_filled);

        let CanvasContent::Plot(spec) = &mut doc.regions[0].content else {
            unreachable!()
        };
        let removed = spec.remove_trace(1).unwrap();
        assert_eq!(removed.label, "cosine");
        assert_eq!(spec.trace_count(), 1);
        assert!(spec.remove_trace(0).is_err());

        let CanvasContent::Plot(spec) = &mut doc.regions[0].content else {
            unreachable!()
        };
        spec.x_axis.range = Some(PlotRange { min: 1., max: 1. });
        assert!(doc.validate().is_err());
        let CanvasContent::Plot(spec) = &mut doc.regions[0].content else {
            unreachable!()
        };
        spec.x_axis.range = None;
        spec.legacy_traces_mut().unwrap()[0].style.marker_frequency = 0;
        assert!(doc.validate().is_err());
    }
    #[test]
    fn calculation_disable_is_backward_compatible_and_only_active_source_affects_dependencies() {
        let mut document = CanvasDocument::default();
        let id = document.add_math(0., 0.);
        let encoded = serde_json::to_string(&document).unwrap();
        assert!(!encoded.contains("calculation_disabled"));
        let older: CanvasDocument = serde_json::from_str(&encoded).unwrap();
        assert!(!older.regions[0].calculation_disabled);
        document.regions[0].calculation_disabled = true;
        let active = document.active_calculation_signature();
        let presentation = document.calculation_signature();
        document.regions[0].content = CanvasContent::Math(PortableTree {
            kind: NodeKind::Number("99".into()),
            children: vec![],
        });
        assert_eq!(document.active_calculation_signature(), active);
        assert_ne!(document.calculation_signature(), presentation);
        let encoded = serde_json::to_string(&document).unwrap();
        let reopened: CanvasDocument = serde_json::from_str(&encoded).unwrap();
        assert!(reopened.regions[0].calculation_disabled);
        assert_eq!(reopened.regions[0].id, id);
        document.regions[0].calculation_disabled = false;
        assert_ne!(document.active_calculation_signature(), active);
        document.regions[0].content = CanvasContent::Text("not a calculation".into());
        document.regions[0].calculation_disabled = true;
        assert!(document.validate().is_err());
    }

    #[test]
    fn legacy_native_disable_migration_retains_pending_presentations_and_honors_explicit_reenable()
    {
        let dir = TestDir::new();
        let native_path = dir.path("original-disabled.mcdx");
        let mut original = CanvasDocument::default();
        original.add_math(0., 0.);
        original.regions[0].content = CanvasContent::Math(PortableTree {
            kind: NodeKind::Number("2".into()),
            children: vec![],
        });
        original.regions[0].calculation_disabled = true;
        crate::export::export_mcdx(&original, &native_path).unwrap();
        let package = crate::native::NativePackage::open(&native_path).unwrap();
        let source_id = package.regions().unwrap()[0].region_id.clone().unwrap();
        let mut legacy = original;
        legacy.native_archive = Some(std::fs::read(native_path).unwrap());
        legacy.regions[0].source_region = Some(source_id);
        legacy.regions[0].calculation_disabled = false;
        legacy.calculation_cache = Some(SavedCalculationCache {
            source_signature: legacy.calculation_signature(),
            rows: vec![SavedCalculationRow {
                region_id: legacy.regions[0].id,
                status: SavedCalculationStatus::Synchronized,
                display: Some("previous saved presentation".into()),
                error: None,
                warnings: vec![],
                symbolic_mathml: None,
                numeric_source: None,
            }],
        });
        let path = dir.path("legacy.mathfolio");
        legacy.save(&path).unwrap();
        let mut migrated = CanvasDocument::load(&path).unwrap();
        assert!(migrated.regions[0].calculation_disabled);
        let cache = migrated.saved_calculation_cache().unwrap();
        assert_eq!(
            cache.rows[0].display.as_deref(),
            Some("previous saved presentation")
        );
        assert_eq!(cache.rows[0].status, SavedCalculationStatus::Pending);
        migrated.regions[0].calculation_disabled = false;
        migrated.regions[0].calculation_enabled_override = true;
        migrated
            .calculation_cache
            .as_mut()
            .unwrap()
            .source_signature = migrated.calculation_signature();
        migrated.calculation_cache.as_mut().unwrap().rows[0].status =
            SavedCalculationStatus::Synchronized;
        migrated.save(&path).unwrap();
        let reopened = CanvasDocument::load(&path).unwrap();
        assert!(!reopened.regions[0].calculation_disabled);
        assert!(reopened.regions[0].calculation_enabled_override);
        assert_eq!(
            reopened.saved_calculation_cache().unwrap().rows[0].status,
            SavedCalculationStatus::Synchronized
        );
        legacy.calculation_cache.as_mut().unwrap().source_signature = "0".repeat(64);
        legacy.save(&path).unwrap();
        let mismatched = CanvasDocument::load(&path).unwrap();
        assert!(mismatched.regions[0].calculation_disabled);
        assert!(
            mismatched.saved_calculation_cache().is_none(),
            "migration must not revive an already invalid cache"
        );
    }
}

#[cfg(test)]
#[path = "canvas_signature_tests.rs"]
mod signature_tests;

#[cfg(test)]
mod string_source_budget_tests {
    use super::*;
    #[test]
    fn many_valid_string_atoms_share_the_existing_document_text_budget() {
        let mut document = CanvasDocument::default();
        let id = document.add_math(0., 0.);
        let source = PortableTree {
            kind: NodeKind::Matrix {
                rows: 17,
                cols: 32,
                display: None,
            },
            children: (0..17 * 32)
                .map(|_| PortableTree {
                    kind: NodeKind::StringLiteral("s".repeat(mathfolio_editor::MAX_ATOM_BYTES)),
                    children: vec![],
                })
                .collect(),
        };
        source.validate().unwrap();
        document
            .regions
            .iter_mut()
            .find(|region| region.id == id)
            .unwrap()
            .content = CanvasContent::Math(source);
        assert_eq!(
            document.validate().unwrap_err(),
            "Canvas text exceeds total 8 MiB budget"
        );
    }
}
