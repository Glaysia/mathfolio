//! Opt-in headless page-frame CPU partition. No clock exists in production.
use std::{
    cell::{Cell, RefCell},
    time::Instant,
};
#[derive(Clone, Copy)]
pub(crate) enum Phase {
    Egui,
    OpenLoad,
    OpenVisuals,
    OpenRestore,
    Input,
    Geometry,
    Background,
    RegionScan,
    Display,
    Alignment,
    Content,
    Response,
    Commit,
    Keyboard,
    FormulaLayout,
    FormulaPointer,
    FormulaEvent,
    FormulaRelayout,
    FormulaPaint,
    Tessellation,
    ImportArchive,
    ImportDefaults,
    ImportWorksheet,
    ImportNotes,
    ImportRegions,
    VisualArchive,
    VisualDecode,
    VisualSettings,
    OpenTables,
    RestoreSignature,
    RestoreSaved,
    RestoreArchive,
    RestoreRegions,
    RestoreCaches,
    RestoreDom,
    RestoreDisplays,
    AlignmentRich,
    AlignmentMath,
    AlignmentText,
    NativeLookup,
    NativeBuild,
    NativeParagraph,
    NativeTable,
    NativeMath,
    NativePayload,
    AlignmentEditor,
    FormulaLookup,
    FormulaLabels,
    FormulaTypeset,
    FormulaPayload,
    GlyphLayout,
    FontKey,
}
const NAMES: &[&str] = &[
    "egui_wrapper",
    "open_load",
    "open_visuals",
    "open_restore",
    "input_snapshot",
    "geometry",
    "background",
    "region_scan",
    "display_cache",
    "native_alignment",
    "region_content",
    "region_response",
    "commit",
    "worksheet_keyboard",
    "formula_initial_layout",
    "formula_pointer_hits",
    "formula_event",
    "formula_relayout_hits",
    "formula_paint",
    "tessellation",
    "import_archive",
    "import_defaults",
    "import_worksheet",
    "import_notes",
    "import_regions",
    "visual_archive",
    "visual_decode",
    "visual_settings",
    "open_tables",
    "restore_signature",
    "restore_saved_cache",
    "restore_archive",
    "restore_regions",
    "restore_caches",
    "restore_dom",
    "restore_displays",
    "alignment_rich",
    "alignment_math",
    "alignment_text",
    "native_layout_lookup",
    "native_layout_build",
    "native_paragraph",
    "native_table",
    "native_math",
    "native_layout_payload",
    "alignment_editor",
    "formula_cache_lookup",
    "formula_labels",
    "formula_typeset",
    "formula_payload",
    "glyph_layout",
    "font_key",
];
const COUNT: usize = NAMES.len();
struct Running {
    phase: Phase,
    start: Instant,
    nested: u128,
}
struct Stats {
    nanos: [u128; COUNT],
    inclusive: [u128; COUNT],
    calls: [u64; COUNT],
    stack: Vec<Running>,
}
thread_local! {
    static ENABLED: Cell<bool> = const { Cell::new(false) };
    static ACTIVE: RefCell<Option<Stats>> = const { RefCell::new(None) };
}
pub(crate) struct Frame(Option<Instant>);
impl Frame {
    pub(crate) fn start() -> Self {
        if !ENABLED.get() {
            return Self(None);
        }
        ACTIVE.with_borrow_mut(|active| {
            assert!(active.is_none());
            *active = Some(Stats {
                nanos: [0; COUNT],
                inclusive: [0; COUNT],
                calls: [0; COUNT],
                stack: Vec::new(),
            });
        });
        Self(Some(Instant::now()))
    }
    pub(crate) fn finish(self) -> Option<serde_json::Value> {
        self.0.map(|start| {
            let total = start.elapsed().as_nanos();
            let stats = ACTIVE.with_borrow_mut(|active| active.take().unwrap());
            assert!(stats.stack.is_empty());
            let accounted: u128 = stats.nanos.iter().sum();
            assert!(accounted <= total);
            let phases: serde_json::Map<_,_> = NAMES.iter().enumerate().map(|(i,name)| (name.to_string(),serde_json::json!({"ns":stats.nanos[i], "inclusive_ns":stats.inclusive[i], "calls":stats.calls[i]}))).collect();
            serde_json::json!({"total_ns":total,"accounted_ns":accounted,"gap_ns":total-accounted,"phases":phases})
        })
    }
}
pub(crate) struct Scope(bool);
pub(crate) fn scope(phase: Phase) -> Scope {
    let active = ACTIVE.with_borrow_mut(|active| {
        if let Some(stats) = active {
            stats.stack.push(Running {
                phase,
                start: Instant::now(),
                nested: 0,
            });
            true
        } else {
            false
        }
    });
    Scope(active)
}
impl Drop for Scope {
    fn drop(&mut self) {
        if !self.0 {
            return;
        }
        ACTIVE.with_borrow_mut(|active| {
            let stats = active.as_mut().unwrap();
            let scope = stats.stack.pop().unwrap();
            let elapsed = scope.start.elapsed().as_nanos();
            let i = scope.phase as usize;
            stats.nanos[i] += elapsed - scope.nested;
            stats.inclusive[i] += elapsed;
            stats.calls[i] += 1;
            if let Some(parent) = stats.stack.last_mut() {
                parent.nested += elapsed;
            }
        });
    }
}
pub(crate) fn record<T>(run: impl FnOnce() -> T) -> T {
    assert!(!ENABLED.replace(true));
    let result = run();
    ENABLED.set(false);
    assert!(ACTIVE.with_borrow(|a| a.is_none()));
    result
}
#[test]
fn nested_frame_phases_are_exclusive_and_opt_in() {
    assert!(Frame::start().finish().is_none());
    let result = record(|| {
        let frame = Frame::start();
        {
            let _outer = scope(Phase::Egui);
            {
                let _inner = scope(Phase::Geometry);
            }
        }
        frame.finish().unwrap()
    });
    assert_eq!(result["phases"]["egui_wrapper"]["calls"], 1);
    assert_eq!(result["phases"]["geometry"]["calls"], 1);
    assert_eq!(
        result["accounted_ns"].as_u64().unwrap() + result["gap_ns"].as_u64().unwrap(),
        result["total_ns"].as_u64().unwrap()
    );
}

thread_local! {
    static HEIGHT_ORACLE: Cell<bool> = const { Cell::new(false) };
    static HEIGHT_CHECKS: Cell<usize> = const { Cell::new(0) };
}
pub(crate) fn height_oracle_enabled() -> bool {
    HEIGHT_ORACLE.get()
}
pub(crate) fn assert_height(current: f64, original: f64, grid: f64) {
    assert_eq!(
        current.to_bits(),
        original.to_bits(),
        "intrinsic height must equal old typesetting"
    );
    assert_eq!(
        ((current + grid) / grid).ceil(),
        ((original + grid) / grid).ceil(),
        "identical insertion advance"
    );
    HEIGHT_CHECKS.set(HEIGHT_CHECKS.get() + 1);
}
pub(crate) fn with_height_oracle(run: impl FnOnce()) -> usize {
    assert!(!HEIGHT_ORACLE.replace(true));
    HEIGHT_CHECKS.set(0);
    run();
    HEIGHT_ORACLE.set(false);
    HEIGHT_CHECKS.get()
}

thread_local! { static TESSELLATE: Cell<bool> = const { Cell::new(false) }; }
pub(crate) fn tessellate(ctx: &eframe::egui::Context, frame: &mut eframe::egui::FullOutput) {
    if TESSELLATE.get() {
        let _timing = scope(Phase::Tessellation);
        std::hint::black_box(
            ctx.tessellate(std::mem::take(&mut frame.shapes), frame.pixels_per_point),
        );
    }
}
pub(crate) fn with_tessellation(run: impl FnOnce()) {
    assert!(!TESSELLATE.replace(true));
    record(run);
    TESSELLATE.set(false);
}
