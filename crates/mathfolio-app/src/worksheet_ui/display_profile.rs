//! Opt-in unit-test clocks; this module is absent from production builds.
use super::*;
use std::cell::Cell;

#[derive(Clone, Copy)]
pub(in crate::worksheet_ui::edit_commit) enum Phase {
    Arc,
    Geometry,
    Dom,
    Traversal,
    Check,
    Decode,
    Store,
    Prune,
}
const NAMES: [&str; 8] = [
    "arc",
    "geometry",
    "dom",
    "traversal",
    "check",
    "decode",
    "store",
    "prune",
];
struct Stats {
    nanos: [u128; 8],
    calls: [u64; 8],
    regions: usize,
    source_bytes: usize,
    labels: usize,
    sources: usize,
    projections: usize,
    projection_owned_bytes: usize,
}
thread_local! {
    static ENABLED: Cell<bool> = const { Cell::new(false) };
    static ACTIVE: RefCell<Option<Stats>> = const { RefCell::new(None) };
    static FINISHED: RefCell<Vec<serde_json::Value>> = const { RefCell::new(Vec::new()) };
}
pub(in crate::worksheet_ui::edit_commit) struct Frame(Option<Instant>);
impl Frame {
    pub(in crate::worksheet_ui::edit_commit) fn start(regions: usize, source_bytes: usize) -> Self {
        if !ENABLED.get() || !RECORDS.with_borrow(|r| r.is_some()) {
            return Self(None);
        }
        ACTIVE.with_borrow_mut(|active| {
            assert!(active.is_none());
            *active = Some(Stats {
                nanos: [0; 8],
                calls: [0; 8],
                regions,
                source_bytes,
                labels: 0,
                sources: 0,
                projections: 0,
                projection_owned_bytes: 0,
            });
        });
        Self(Some(Instant::now()))
    }
    pub(in crate::worksheet_ui::edit_commit) fn projections(
        &self,
        generated: &[worksheet_math::DisplayProjection],
    ) {
        if self.0.is_none() {
            return;
        }
        fn heap(tree: &PortableTree) -> usize {
            let strings = match &tree.kind {
                NodeKind::Identifier(s) | NodeKind::Number(s) | NodeKind::Imaginary(s) => {
                    s.capacity()
                }
                NodeKind::AnnotatedIdentifier { name, labels } => {
                    name.capacity() + labels.capacity()
                }
                NodeKind::NativeIdentifier {
                    name,
                    labels,
                    subscript,
                    ..
                } => {
                    name.capacity()
                        + labels.capacity()
                        + subscript.as_ref().map_or(0, String::capacity)
                }
                NodeKind::ImaginarySymbol { value, symbol } => value.capacity() + symbol.capacity(),
                _ => 0,
            };
            strings
                + tree.children.capacity() * std::mem::size_of::<PortableTree>()
                + tree.children.iter().map(heap).sum::<usize>()
        }
        let trees: Vec<_> = generated
            .iter()
            .filter_map(Option::as_ref)
            .filter_map(|r| r.as_ref().ok())
            .collect();
        let bytes: usize = trees.iter().map(|t| heap(t)).sum();
        let bytes: usize = bytes
            + std::mem::size_of_val(generated)
            + generated
                .iter()
                .filter_map(Option::as_ref)
                .filter_map(|r| r.as_ref().err())
                .map(String::capacity)
                .sum::<usize>();
        ACTIVE.with_borrow_mut(|active| {
            let active = active.as_mut().unwrap();
            active.projections = trees.len();
            active.projection_owned_bytes = bytes;
        });
    }
    pub(in crate::worksheet_ui::edit_commit) fn finish(self, labels: usize, sources: usize) {
        if self.0.is_some() {
            ACTIVE.with_borrow_mut(|active| {
                let active = active.as_mut().unwrap();
                active.labels = labels;
                active.sources = sources;
            });
        }
    }
}
impl Drop for Frame {
    fn drop(&mut self) {
        let Some(start) = self.0 else {
            return;
        };
        let total = start.elapsed().as_nanos();
        let mut stats = ACTIVE.with_borrow_mut(|active| active.take().unwrap());
        // Traversal encloses check/decode/store. Keep the reported phases exclusive.
        stats.nanos[Phase::Traversal as usize] -= stats.nanos[Phase::Check as usize]
            + stats.nanos[Phase::Decode as usize]
            + stats.nanos[Phase::Store as usize];
        let accounted: u128 = stats.nanos.iter().sum();
        assert!(accounted <= total);
        let phases: serde_json::Map<String, serde_json::Value> = NAMES
            .iter()
            .enumerate()
            .map(|(i, n)| {
                (
                    (*n).into(),
                    serde_json::json!({"ns":stats.nanos[i],"calls":stats.calls[i]}),
                )
            })
            .collect();
        let result = serde_json::json!({"total_ns":total,"accounted_ns":accounted,"unaccounted_ns":total-accounted,"regions":stats.regions,"source_bytes":stats.source_bytes,"labels":stats.labels,"sources":stats.sources,"projected_trees":stats.projections,"projection_owned_bytes":stats.projection_owned_bytes,"phases":phases});
        println!("DISPLAY_PHASE {result}");
        FINISHED.with_borrow_mut(|done| done.push(result));
    }
}
pub(in crate::worksheet_ui::edit_commit) struct Scope(Phase, Option<Instant>);
pub(in crate::worksheet_ui::edit_commit) fn scope(phase: Phase) -> Scope {
    Scope(
        phase,
        ACTIVE.with_borrow(|active| active.as_ref().map(|_| Instant::now())),
    )
}
impl Drop for Scope {
    fn drop(&mut self) {
        if let Some(start) = self.1 {
            let ns = start.elapsed().as_nanos();
            ACTIVE.with_borrow_mut(|active| {
                let active = active.as_mut().unwrap();
                active.nanos[self.0 as usize] += ns;
                active.calls[self.0 as usize] += 1;
            });
        }
    }
}
fn record<T>(run: impl FnOnce() -> T) -> (T, Vec<serde_json::Value>) {
    assert!(!ENABLED.replace(true));
    FINISHED.with_borrow_mut(Vec::clear);
    let result = run();
    ENABLED.set(false);
    assert!(ACTIVE.with_borrow(|active| active.is_none()));
    (result, FINISHED.with_borrow_mut(std::mem::take))
}

#[test]
#[ignore = "CPU-only actual saved/open first-input display preparation phases"]
fn actual_open_display_phase_profile() {
    let (_, frames) = record(open_input_latency::actual_open_first_input_profile);
    assert_eq!(frames.len(), 18, "only dirty Enter has display preparation");
}

#[test]
fn display_recording_keeps_cold_warm_and_geometry_labels_exact() {
    RECORDS.with_borrow_mut(|r| *r = Some(Vec::new()));
    let mut app = fixture(4, 0);
    let (_, frames) = record(|| {
        let cold = app.prepare_calculation_input().unwrap();
        assert_matches_complete_dom(&app, &cold.source);
        app.document.regions[0].left = 12345.6789;
        let moved = app.prepare_calculation_input().unwrap();
        assert_matches_complete_dom(&app, &moved.source);
        if let CanvasContent::Math(tree) = &mut app.document.regions[0].content {
            tree.children[1].kind = NodeKind::Number("9".into());
        }
        let edited = app.prepare_calculation_input().unwrap();
        assert_matches_complete_dom(&app, &edited.source);
    });
    assert_eq!(frames.len(), 2);
    assert_eq!(frames[0]["phases"]["decode"]["calls"], 4);
    assert_eq!(frames[1]["phases"]["decode"]["calls"], 1);
    assert_eq!(frames[1]["phases"]["dom"]["calls"], 0);
    RECORDS.with_borrow_mut(|r| *r = None);
}

#[test]
fn missing_display_cache_with_worksheet_hits_keeps_the_complete_dom_fallback() {
    let mut app = fixture(4, 0);
    app.prepare_calculation_input().unwrap();
    assert_eq!(app.background.preparation.full_dom_parses, 0);
    app.labelled_trees.clear();
    app.background.preparation.labelled_sources.clear();
    app.background.preparation.geometry = None;
    app.background.preparation.input = None;
    let input = app.prepare_calculation_input().unwrap();
    assert_eq!(app.background.preparation.full_dom_parses, 1);
    assert_matches_complete_dom(&app, &input.source);
}
