//! Artifact-only actual full App pointer regressions; not yet compiled/executed.
//! Production touches proposed separately are cfg(test) registration + ID tap.
use super::*;
use mathcraft_document::canvas::MatrixResultTarget;
use std::cell::RefCell;

thread_local! {
    static FORMULA_IDS: RefCell<Option<HashMap<u64, egui::Id>>> = const { RefCell::new(None) };
    // Exact current Response returned by the real widget, before Ui/Context
    // end-pass bookkeeping. Diagnostic only; no geometry recalculation.
    static FORMULA_RESPONSES: RefCell<HashMap<u64, egui::Response>> = RefCell::new(HashMap::new());
}
/// Called immediately after the actual formula widget; no geometry recalculation.
pub(super) fn observe_formula(owner: u64, response: &egui::Response) {
    if FORMULA_IDS.with_borrow(|slot| slot.is_some()) {
        FORMULA_RESPONSES.with_borrow_mut(|responses| {
            responses.insert(owner, response.clone());
        });
    }
    FORMULA_IDS.with_borrow_mut(|slot| {
        if let Some(ids) = slot {
            ids.insert(owner, response.id);
        }
    });
}
pub(super) struct Observe;
impl Observe {
    pub(super) fn start() -> Self {
        FORMULA_IDS.with_borrow_mut(|slot| {
            assert!(slot.is_none());
            *slot = Some(HashMap::new());
        });
        FORMULA_RESPONSES.with_borrow_mut(HashMap::clear);
        Self
    }
}
impl Drop for Observe {
    fn drop(&mut self) {
        FORMULA_IDS.with_borrow_mut(|slot| *slot = None);
        FORMULA_RESPONSES.with_borrow_mut(HashMap::clear);
    }
}
pub(super) fn observed_formula(owner: u64) -> Option<egui::Response> {
    FORMULA_RESPONSES.with_borrow(|responses| responses.get(&owner).cloned())
}
fn handle(ctx: &egui::Context, owner: u64) -> egui::Pos2 {
    let id = FORMULA_IDS
        .with_borrow(|slot| slot.as_ref().and_then(|ids| ids.get(&owner)).copied())
        .expect("actual worksheet Math branch was not painted");
    let response = ctx
        .read_response(id.with("matrix-result-bracket-drag").with("hit"))
        .expect("prepared matrix had no actual bracket interaction widget");
    ctx.layer_transform_to_global(response.layer_id)
        .unwrap_or_default()
        * response.rect.center()
}
fn node(kind: NodeKind, children: Vec<PortableTree>) -> PortableTree {
    PortableTree { kind, children }
}
fn number(n: usize) -> PortableTree {
    node(NodeKind::Number(n.to_string()), vec![])
}
fn identifier() -> PortableTree {
    node(NodeKind::Identifier("M".into()), vec![])
}

struct Fixture {
    app: WorksheetApp,
    ctx: egui::Context,
    owner: u64,
    tick: u64,
    _observe: Observe,
}
impl Fixture {
    fn new(zoom: f32) -> Self {
        let observe = Observe::start();
        let ctx = egui::Context::default();
        crate::configure(&ctx);
        let mut app = WorksheetApp {
            automatic_calculation: false,
            zoom,
            ..Default::default()
        };
        let definition = app.document.add_math(20., 20.);
        app.document
            .regions
            .iter_mut()
            .find(|r| r.id == definition)
            .unwrap()
            .content = CanvasContent::Math(node(
            NodeKind::Binary(BinaryOp::Define),
            vec![
                identifier(),
                node(
                    NodeKind::Matrix {
                        rows: 20,
                        cols: 20,
                        display: None,
                    },
                    (1..=400).map(number).collect(),
                ),
            ],
        ));
        let owner = app.document.add_math(20., 500.);
        app.document
            .regions
            .iter_mut()
            .find(|r| r.id == owner)
            .unwrap()
            .content = CanvasContent::Math(node(
            NodeKind::Binary(BinaryOp::Evaluate),
            vec![identifier(), node(NodeKind::Placeholder, vec![])],
        ));
        // Independent Rust calculator setup, excluded from zero-execution UI scope.
        app.recalculate();
        let row = app
            .results
            .get(&owner)
            .expect("Rust matrix fixture setup failed");
        let Some(mathcraft_compat::Value::Matrix {
            rows,
            cols,
            elements,
        }) = &row.value
        else {
            panic!("{row:?}")
        };
        assert_eq!((*rows, *cols, elements.len()), (20, 20, 400));
        assert!(row.error.is_none());
        app.persist_calculation_cache();
        let numeric = app
            .document
            .calculation_cache
            .as_ref()
            .unwrap()
            .rows
            .iter()
            .find(|r| r.region_id == owner)
            .unwrap();
        assert_eq!(numeric.status, SavedCalculationStatus::Synchronized);
        assert!(matches!(
            numeric.numeric_source.as_ref(),
            Some(SavedNumericSource::EvaluatedValue(_))
        ));
        app.last_executed_regions.clear();
        app.undo.clear();
        app.redo.clear();
        app.dirty = false;
        app.selected = Some(owner);
        app.insertion = None;
        app.editing_region = None;
        app.focus_new = None;
        let mut f = Self {
            app,
            ctx,
            owner,
            tick: 0,
            _observe: observe,
        };
        for _ in 0..8 {
            f.frame(vec![]);
        }
        assert!(f.app.display_cache[&owner].matrix.shape() == Some((20, 20)));
        assert_eq!(
            f.app.display_cache[&owner]
                .matrix
                .value()
                .map(|p| (p.rows, p.cols)),
            Some((12, 12))
        );
        f.app.last_executed_regions.clear();
        f.app.undo.clear();
        f.app.redo.clear();
        f.app.dirty = false;
        f
    }
    fn frame(&mut self, events: Vec<egui::Event>) {
        self.tick += 1;
        let mut raw = egui::RawInput {
            events,
            time: Some(self.tick as f64 / 60.),
            screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(2400., 2200.))),
            ..Default::default()
        };
        eframe::App::raw_input_hook(&mut self.app, &self.ctx, &mut raw);
        self.ctx
            .run_ui(raw, |ui| self.app.app_ui_frame(ui))
            .drop_without_applying_deltas();
    }
    fn begin(&mut self) -> egui::Pos2 {
        let start = handle(&self.ctx, self.owner);
        self.frame(vec![egui::Event::PointerMoved(start)]);
        self.frame(button(start, true));
        assert!(
            self.app.matrix_view_drag.is_some(),
            "actual bracket press failed to Begin"
        );
        start
    }
}
fn button(pos: egui::Pos2, pressed: bool) -> Vec<egui::Event> {
    vec![
        egui::Event::PointerMoved(pos),
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        },
    ]
}
fn command(key: egui::Key) -> Vec<egui::Event> {
    vec![egui::Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::COMMAND,
    }]
}
fn rows(app: &WorksheetApp) -> Vec<(u64, serde_json::Value)> {
    let mut rows: Vec<_> = app
        .results
        .iter()
        .map(|(id, r)| (*id, serde_json::to_value(r).unwrap()))
        .collect();
    rows.sort_by_key(|(id, _)| *id);
    rows
}
struct Inert {
    regions: Vec<CanvasRegion>,
    cache: Option<SavedCalculationCache>,
    saved: HashMap<u64, SavedCalculationRow>,
    rows: Vec<(u64, serde_json::Value)>,
    signature: String,
    active: String,
    input: Option<String>,
    generation: u64,
    plot_signature: String,
    pending: HashSet<u64>,
}
impl Inert {
    fn new(app: &WorksheetApp) -> Self {
        Self {
            regions: app.document.regions.clone(),
            cache: app.document.calculation_cache.clone(),
            saved: app.saved_results.clone(),
            rows: rows(app),
            signature: app.document.calculation_signature(),
            active: app.document.active_calculation_signature(),
            input: app.committed_input.as_ref().map(|i| i.signature.clone()),
            generation: app.background.submitted_generation(),
            plot_signature: app.plot_signature.clone(),
            pending: app.pending_regions.clone(),
        }
    }
    fn check(&self, app: &WorksheetApp) {
        assert_eq!(app.document.regions, self.regions);
        assert_eq!(app.document.calculation_cache, self.cache);
        assert_eq!(app.saved_results, self.saved);
        assert_eq!(rows(app), self.rows);
        assert_eq!(app.document.calculation_signature(), self.signature);
        assert_eq!(app.document.active_calculation_signature(), self.active);
        assert_eq!(
            app.committed_input.as_ref().map(|i| i.signature.clone()),
            self.input
        );
        assert_eq!(app.background.submitted_generation(), self.generation);
        assert_eq!(app.plot_signature, self.plot_signature);
        assert_eq!(app.pending_regions, self.pending);
        assert!(app.last_executed_regions.is_empty());
        assert!(!app.calculation_pending && !app.background.running);
    }
}

#[test]
fn actual_app_bracket_preview_commit_undo_redo_preserve_full_value_cache_and_generation() {
    for zoom in [1., 1.5] {
        let mut f = Fixture::new(zoom);
        let inert = Inert::new(&f.app);
        let target = MatrixResultTarget::math(f.owner);
        assert!(f.app.document.matrix_result_views.get(&target).is_none());
        let start = f.begin();
        inert.check(&f.app);
        assert!(f.app.undo.is_empty() && !f.app.dirty);
        let moved = start + vec2(120. * zoom, 80. * zoom);
        f.frame(vec![egui::Event::PointerMoved(moved)]);
        inert.check(&f.app);
        assert!(f.app.document.matrix_result_views.get(&target).is_none());
        assert!(f.app.undo.is_empty() && !f.app.dirty);
        let preview = f
            .app
            .requested_matrix_view(f.owner)
            .expect("Preview was not published");
        let size = preview
            .size
            .expect("actual pointer movement did not change size");
        assert!(size.rows > 12 && size.cols > 12);
        f.frame(vec![]); // Render the ephemeral projection while pointer remains held.
        inert.check(&f.app);
        let projected = f.app.display_cache[&f.owner].matrix.value().unwrap();
        assert_eq!(
            (projected.rows, projected.cols),
            (size.rows as usize, size.cols as usize)
        );
        assert_eq!(projected.cells.len(), projected.rows * projected.cols);
        assert!(f.app.undo.is_empty() && !f.app.dirty);
        f.frame(button(moved, false));
        inert.check(&f.app);
        assert!(f.app.matrix_view_drag.is_none());
        let committed = f.app.document.matrix_result_views.get(&target).copied();
        assert_eq!(committed, Some(preview));
        assert_eq!(f.app.undo.len(), 1);
        assert!(f.app.redo.is_empty() && f.app.dirty);
        f.frame(command(egui::Key::Z));
        inert.check(&f.app);
        assert!(f.app.document.matrix_result_views.get(&target).is_none());
        assert!(f.app.undo.is_empty());
        assert_eq!(f.app.redo.len(), 1);
        f.frame(command(egui::Key::Y));
        inert.check(&f.app);
        assert_eq!(
            f.app.document.matrix_result_views.get(&target).copied(),
            committed
        );
        assert_eq!(f.app.undo.len(), 1);
        assert!(f.app.redo.is_empty());
    }
}

#[test]
fn same_shape_result_delivery_cancels_transient_app_drag_without_late_release_commit() {
    let mut f = Fixture::new(1.);
    let target = MatrixResultTarget::math(f.owner);
    let start = f.begin();
    let moved = start + vec2(80., 80.);
    f.frame(vec![egui::Event::PointerMoved(moved)]);
    assert!(f.app.matrix_view_drag.is_some());
    assert!(f.app.document.matrix_result_views.get(&target).is_none());
    let source = f.app.document.regions.clone();
    let signature = f.app.document.active_calculation_signature();
    let previous_first = f.app.display_cache[&f.owner].matrix.value().unwrap().cells[0].plain();
    // Simulate one accepted worker delivery, not a source execution or API resize.
    let mut delivered: Vec<_> = f.app.results.values().cloned().collect();
    let row = delivered
        .iter_mut()
        .find(|r| r.region_id == f.owner.to_string())
        .unwrap();
    let Some(mathcraft_compat::Value::Matrix {
        rows: matrix_rows,
        cols: matrix_cols,
        elements,
    }) = row.value.as_mut()
    else {
        panic!()
    };
    assert_eq!((*matrix_rows, *matrix_cols), (20, 20));
    for q in elements {
        q.value.re += 1000.;
    }
    if let Some(mathcraft_compat::Value::Matrix { elements, .. }) = row.display_value.as_mut() {
        for q in elements {
            q.value.re += 1000.;
        }
    }
    f.app.accept_calculation_update(CalculationUpdate {
        rows: delivered,
        pending_region_ids: vec![],
        executed_region_ids: vec![],
        cancelled: false,
    });
    assert!(f.app.matrix_view_drag.is_none());
    let after = rows(&f.app);
    let generation = f.app.background.submitted_generation();
    f.frame(vec![]);
    f.frame(button(moved, false));
    f.frame(vec![]);
    assert!(f.app.matrix_view_drag.is_none());
    assert!(f.app.document.matrix_result_views.get(&target).is_none());
    assert!(f.app.undo.is_empty() && f.app.redo.is_empty() && !f.app.dirty);
    assert_eq!(f.app.document.regions, source);
    assert_eq!(f.app.document.active_calculation_signature(), signature);
    assert_eq!(rows(&f.app), after);
    assert_eq!(f.app.background.submitted_generation(), generation);
    assert!(f.app.last_executed_regions.is_empty());
    let projected = f.app.display_cache[&f.owner].matrix.value().unwrap();
    assert_eq!((projected.rows, projected.cols), (12, 12));
    assert_ne!(projected.cells[0].plain(), previous_first);
    let Some(mathcraft_compat::Value::Matrix { elements, .. }) = &f.app.results[&f.owner].value
    else {
        panic!()
    };
    assert_eq!(elements[0].value.re, 1001.);
}

mod source_event_tests;

mod continuation_tests;

mod navigator_app_tests;

mod clipboard_tests;

mod duplicate_classification_tests;

mod indices_tests;

mod origin_indices_tests;

mod indices_context_tests;

mod first_local_clone_tests;

#[cfg(test)]
mod navigator_resize_app_tests;
