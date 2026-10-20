//! Test-only CPU phase timings; no counters, clocks or environment checks are
//! compiled into the desktop calculation preparation path.
use super::*;
use std::{cell::RefCell, time::Instant};

#[path = "open_input_latency.rs"]
mod open_input_latency;

#[path = "display_profile.rs"]
pub(super) mod display_profile;

thread_local! {
    static RECORDS: RefCell<Option<Vec<PreparationTiming>>> = const { RefCell::new(None) };
    static WORKSHEET_CAPTURE: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

// Explicitly armed by the ignored open-lifecycle probe only. This file I/O
// belongs to a capture run, never a performance sample or production build.
pub(super) fn capture_worksheet_input(regions: &[(u64, f64, f64, String)]) {
    WORKSHEET_CAPTURE.with_borrow_mut(|target| {
        if let Some(path) = target.take() {
            std::fs::write(path, serde_json::to_vec(regions).unwrap()).unwrap();
        }
    });
}

pub(super) struct PreparationTiming {
    start: Instant,
    last: Instant,
    signature: u128,
    validation: u128,
    serialization: u128,
    worksheet: u128,
    display: u128,
}

impl PreparationTiming {
    pub(super) fn start() -> Self {
        let start = Instant::now();
        Self {
            start,
            last: start,
            signature: 0,
            validation: 0,
            serialization: 0,
            worksheet: 0,
            display: 0,
        }
    }
    fn phase(&mut self) -> u128 {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last).as_micros();
        self.last = now;
        elapsed
    }
    pub(super) fn signature(&mut self) {
        self.signature = self.phase();
    }
    pub(super) fn validation(&mut self) {
        self.validation = self.phase();
    }
    pub(super) fn serialization(&mut self) {
        self.serialization = self.phase();
    }
    pub(super) fn worksheet(&mut self) {
        self.worksheet = self.phase();
    }
    pub(super) fn display(&mut self) {
        self.display = self.phase();
    }
    pub(super) fn finish(self, cached: bool) {
        RECORDS.with_borrow_mut(|records| if let Some(records) = records {
            println!("PREP_PHASE cached={cached} signature_us={} validation_us={} serialize_us={} worksheet_us={} display_us={} total_us={}", self.signature, self.validation, self.serialization, self.worksheet, self.display, self.start.elapsed().as_micros());
            records.push(self);
        });
    }
}

fn tree(kind: NodeKind, children: Vec<PortableTree>) -> PortableTree {
    PortableTree { kind, children }
}

fn fixture(count: usize, matrix: usize) -> WorksheetApp {
    let mut app = WorksheetApp::default();
    for n in 0..count {
        let id = app.document.add_math(0., n as f64 * 35.);
        let value = if matrix == 0 {
            tree(NodeKind::Number(n.to_string()), vec![])
        } else {
            tree(
                NodeKind::Matrix {
                    rows: matrix,
                    cols: matrix,
                    display: None,
                },
                (0..matrix * matrix)
                    .map(|i| tree(NodeKind::Number(i.to_string()), vec![]))
                    .collect(),
            )
        };
        let value = tree(
            NodeKind::Binary(BinaryOp::Define),
            vec![tree(NodeKind::Identifier(format!("v{n}")), vec![]), value],
        );
        app.editors
            .insert(id, Editor::from_portable(&value).unwrap());
        app.document.regions.last_mut().unwrap().content = CanvasContent::Math(value);
    }
    app
}

#[test]
#[ignore = "CPU-only per-phase input preparation profile"]
fn preparation_phase_profile() {
    RECORDS.with_borrow_mut(|r| *r = Some(Vec::new()));
    for (count, matrix) in [(4000, 0), (100, 16)] {
        println!("PREP_CASE count={count} matrix={matrix}");
        let mut app = fixture(count, matrix);
        app.prepare_calculation_input().unwrap();
        app.prepare_calculation_input().unwrap();
        app.document.regions[0].left += app.document.grid;
        app.prepare_calculation_input().unwrap();
        if let CanvasContent::Math(tree) = &mut app.document.regions[0].content {
            tree.children[1].kind = NodeKind::Number("9".into());
        }
        app.prepare_calculation_input().unwrap();
        println!(
            "SIGNATURE_CACHE count={count} retained_bytes={} region_serializations={}",
            app.background.preparation.signature.stats().0,
            app.background.preparation.signature.stats().1
        );
    }
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../artifacts/rea/unit-selected-sources/1015_LCC_S.mcdx");
    if path.is_file() {
        println!("PREP_CASE actual=1015_LCC_S");
        let mut app = WorksheetApp::default();
        app.open(&path);
        app.prepare_calculation_input().unwrap();
        app.document
            .regions
            .iter_mut()
            .find(|r| matches!(r.content, CanvasContent::Math(_)))
            .unwrap()
            .left += app.document.grid;
        app.prepare_calculation_input().unwrap();
    }
    let records = RECORDS.with_borrow_mut(Option::take).unwrap();
    assert!(records.len() >= 6);
}

fn assert_matches_uncached(app: &mut WorksheetApp, cached: &CalculationInput) {
    let labels = app.labelled_trees.clone();
    assert_matches_complete_dom(app, &cached.source);
    app.background.preparation = PreparationCache::default();
    let fresh = app.prepare_calculation_input().unwrap();
    assert_eq!(fresh.source, cached.source);
    assert_eq!(fresh.faults, cached.faults);
    assert_eq!(fresh.region_ids, cached.region_ids);
    assert_eq!(fresh.signature, cached.signature);
    assert_eq!(fresh.signature, app.document.active_calculation_signature());
    assert_eq!(app.labelled_trees, labels);
}

#[test]
fn prepared_signature_keeps_document_identity_even_when_math_xml_is_unchanged() {
    let mut app = fixture(3, 0);
    let mut before = app.prepare_calculation_input().unwrap();
    assert_eq!(
        before.signature,
        app.document.active_calculation_signature()
    );
    assert_eq!(app.background.preparation.signature.stats().1, 3);
    for change in 0..6 {
        match change {
            0 => app.document.regions[0].format.precision = 8,
            1 => app.document.regions[0].source_region = Some("µ한글".into()),
            2 => app.document.width += 1.,
            3 => app.document.native_archive = Some(vec![1, 2, 3]),
            4 => app.document.native_archive.as_mut().unwrap()[0] = 9,
            5 => app.document.regions[0].calculation_enabled_override = true,
            _ => unreachable!(),
        }
        let after = app.prepare_calculation_input().unwrap();
        assert_ne!(after.signature, before.signature);
        assert_eq!(after.signature, app.document.active_calculation_signature());
        assert_eq!(after.source, before.source);
        before = after;
    }
    // Only changed region metadata caused additional serialization. Page and
    // archive changes were independently included without recoding any region.
    assert_eq!(app.background.preparation.signature.stats().1, 6);
    assert_matches_uncached(&mut app, &before);
}

fn assert_matches_complete_dom(app: &WorksheetApp, source: &str) {
    // Independent oracle for the former display path. It sees inherited
    // namespaces, nested regions and exact range boundaries in one full DOM.
    let parsed = roxmltree::Document::parse(source).unwrap();
    let expected: HashMap<_, _> = parsed
        .descendants()
        .filter(|n| n.has_tag_name(("http://schemas.mathsoft.com/worksheet50", "region")))
        .filter_map(|region| {
            let id = region.attribute("region-id")?.parse::<u64>().ok()?;
            let math = region
                .children()
                .find(|n| n.has_tag_name(("http://schemas.mathsoft.com/worksheet50", "math")))?;
            worksheet_math::tree_from_mathml_node(math)
                .ok()
                .map(|tree| (id, tree))
        })
        .collect();
    assert_eq!(app.labelled_trees, expected);
}

#[test]
fn emitted_ranges_preserve_display_trees_through_utf8_edit_move_disable_and_failure() {
    let mut app = fixture(3, 0);
    for (index, region) in app.document.regions.iter_mut().enumerate() {
        if let CanvasContent::Math(tree) = &mut region.content {
            tree.children[0].kind = NodeKind::Identifier(format!("幅µ{index}"));
            app.editors
                .insert(region.id, Editor::from_portable(tree).unwrap());
        }
    }
    for change in 0..5 {
        match change {
            1 => {
                if let CanvasContent::Math(tree) = &mut app.document.regions[0].content {
                    tree.children[1].kind = NodeKind::Number("123456789".into());
                }
            }
            2 => app.document.regions[1].left = 1e100,
            3 => app.document.regions[2].calculation_disabled = true,
            4 => {
                let id = app.document.regions[0].id;
                let editor = app.editors.get_mut(&id).unwrap();
                let rhs = editor.arena().get(editor.root()).unwrap().children()[1];
                editor
                    .set_atom(rhs, NodeKind::Number("NaN".into()))
                    .unwrap();
            }
            _ => (),
        }
        let prepared = app.prepare_calculation_input().unwrap();
        assert_eq!(app.background.preparation.full_dom_parses, 0);
        assert_matches_complete_dom(&app, &prepared.source);
        assert_matches_uncached(&mut app, &prepared);
    }
}

#[test]
fn exact_display_xml_cache_rebuilds_only_changed_math_and_prunes_removed_disabled_or_invalid_rows()
{
    let mut app = fixture(4, 0);
    app.prepare_calculation_input().unwrap();
    assert_eq!(app.background.preparation.labelled_decodes, 4);
    if let CanvasContent::Math(tree) = &mut app.document.regions[0].content {
        tree.children[1].kind = NodeKind::Number("9".into());
    }
    let changed = app.prepare_calculation_input().unwrap();
    assert_eq!(app.background.preparation.labelled_decodes, 5);
    assert_matches_uncached(&mut app, &changed);
    let removed = app.document.regions.pop().unwrap().id;
    let disabled = app.document.regions[1].id;
    app.document.regions[1].calculation_disabled = true;
    let invalid = app.document.regions[2].id;
    let editor = app.editors.get_mut(&invalid).unwrap();
    let rhs = editor.arena().get(editor.root()).unwrap().children()[1];
    editor
        .set_atom(rhs, NodeKind::Number("NaN".into()))
        .unwrap();
    let prepared = app.prepare_calculation_input().unwrap();
    assert!(prepared.faults.contains_key(&invalid));
    for id in [removed, disabled, invalid] {
        assert!(!app.labelled_trees.contains_key(&id));
        assert!(
            !app.background
                .preparation
                .labelled_sources
                .contains_key(&id)
        );
    }
    assert_matches_uncached(&mut app, &prepared);
}

#[test]
fn unchanged_query_source_is_relabelled_when_a_global_binding_changes_its_context() {
    let mut app = WorksheetApp::default();
    for (top, text) in [(0., "m="), (100., "v:=3")] {
        let mut editor = Editor::new();
        crate::formula::input_text(&mut editor, text);
        let id = app.document.add_math(0., top);
        app.document.regions.last_mut().unwrap().content =
            CanvasContent::Math(editor.to_portable().unwrap());
        app.editors.insert(id, editor);
    }
    app.prepare_calculation_input().unwrap();
    let query = app.document.regions[0].id;
    let old = app.labelled_trees[&query].clone();
    if let CanvasContent::Math(tree) = &mut app.document.regions[1].content {
        tree.kind = NodeKind::Binary(BinaryOp::GlobalDefine);
        tree.children[0].kind = NodeKind::Identifier("m".into());
    }
    let prepared = app.prepare_calculation_input().unwrap();
    assert_ne!(app.labelled_trees[&query], old);
    assert_eq!(app.background.preparation.labelled_decodes, 4);
    assert_matches_uncached(&mut app, &prepared);
}

#[test]
fn geometry_rewrite_matches_uncached_xml_for_repeated_moves_and_new_float_widths() {
    let mut app = fixture(4, 0);
    app.prepare_calculation_input().unwrap();
    let baseline = app
        .background
        .preparation
        .geometry
        .as_ref()
        .unwrap()
        .source
        .clone();
    for x in [1., 12345.6789, 1e100, -0.] {
        app.document.regions[0].left = x;
        let moved = app.prepare_calculation_input().unwrap();
        assert!(Arc::ptr_eq(
            &baseline,
            &app.background.preparation.geometry.as_ref().unwrap().source
        ));
        let xml = roxmltree::Document::parse(&moved.source).unwrap();
        let region = xml
            .descendants()
            .find(|n| n.is_element() && n.attribute("region-id") == Some("1"))
            .unwrap();
        assert_eq!(region.attribute("left").unwrap(), x.to_string());
    }
    let moved = app.prepare_calculation_input().unwrap();
    assert_matches_uncached(&mut app, &moved);
}

#[test]
fn geometry_cache_rejects_equal_length_content_and_order_changes() {
    let mut app = fixture(4, 0);
    app.prepare_calculation_input().unwrap();
    let baseline = app
        .background
        .preparation
        .geometry
        .as_ref()
        .unwrap()
        .source
        .clone();
    if let CanvasContent::Math(tree) = &mut app.document.regions[0].content {
        tree.children[1].kind = NodeKind::Number("9".into());
    }
    let changed = app.prepare_calculation_input().unwrap();
    assert!(!Arc::ptr_eq(
        &baseline,
        &app.background.preparation.geometry.as_ref().unwrap().source
    ));
    assert_matches_uncached(&mut app, &changed);
    let baseline = app
        .background
        .preparation
        .geometry
        .as_ref()
        .unwrap()
        .source
        .clone();
    app.document.regions[0].top = 999.;
    let reordered = app.prepare_calculation_input().unwrap();
    assert!(!Arc::ptr_eq(
        &baseline,
        &app.background.preparation.geometry.as_ref().unwrap().source
    ));
    assert_matches_uncached(&mut app, &reordered);
}

#[test]
fn geometry_cache_rejects_grid_page_settings_and_equal_size_archive_changes() {
    let mut app = fixture(3, 0);
    for setting in 0..5 {
        app.prepare_calculation_input().unwrap();
        let baseline = app
            .background
            .preparation
            .geometry
            .as_ref()
            .unwrap()
            .source
            .clone();
        match setting {
            0 => app.document.width += 1.,
            1 => app.document.height += 1.,
            2 => app.document.grid *= 0.5,
            3 => app.document.native_archive = Some(vec![1, 2, 3]),
            _ => app.document.native_archive.as_mut().unwrap()[0] = 9,
        }
        let changed = app.prepare_calculation_input().unwrap();
        assert!(
            !Arc::ptr_eq(
                &baseline,
                &app.background.preparation.geometry.as_ref().unwrap().source
            ),
            "setting {setting}"
        );
        assert_matches_uncached(&mut app, &changed);
    }
}

#[test]
fn opaque_solveblock_disables_geometry_reuse_and_keeps_new_outer_position() {
    let mut app = fixture(2, 0);
    let id = app
        .document
        .add_opaque(0., 200., "<solveblock/>", "solveblock");
    app.prepare_calculation_input().unwrap();
    assert_eq!(app.background.preparation.full_dom_parses, 1);
    assert!(app.background.preparation.geometry.is_none());
    app.document
        .regions
        .iter_mut()
        .find(|r| r.id == id)
        .unwrap()
        .top = 333.;
    let moved = app.prepare_calculation_input().unwrap();
    assert!(app.background.preparation.geometry.is_none());
    let xml = roxmltree::Document::parse(&moved.source).unwrap();
    assert!(xml.descendants().any(|n| n.is_element()
        && n.attribute("region-id") == Some(id.to_string().as_str())
        && n.attribute("top") == Some("333")));
    assert_matches_uncached(&mut app, &moved);
}

#[test]
fn reordered_definition_switches_unit_to_variable_and_origin_order_is_forwarded() {
    let mut app = WorksheetApp::default();
    for (top, text) in [
        (0., "m="),
        (100., "m:=3"),
        (200., "ORIGIN:=1"),
        (300., "ORIGIN="),
    ] {
        let mut editor = Editor::new();
        crate::formula::input_text(&mut editor, text);
        let id = app.document.add_math(0., top);
        app.document.regions.last_mut().unwrap().content =
            CanvasContent::Math(editor.to_portable().unwrap());
        app.editors.insert(id, editor);
    }
    let first = app.prepare_calculation_input().unwrap();
    let evaluated = app.document.regions[0].id;
    let before = app.labelled_trees[&evaluated].clone();
    app.document.regions[1].top = -1.;
    app.document.regions[2].top = 400.;
    let reordered = app.prepare_calculation_input().unwrap();
    assert_ne!(app.labelled_trees[&evaluated], before);
    assert_ne!(reordered.source, first.source);
    assert_matches_uncached(&mut app, &reordered);
    let mut engine = WorksheetCalculator::default();
    engine.update(&reordered.source, None).unwrap();
    let update = engine.calculate(CalculationRequest::Worksheet).unwrap();
    let origin = update
        .rows
        .iter()
        .find(|r| r.region_id == app.document.regions[3].id.to_string())
        .unwrap();
    assert_eq!(origin.value.as_ref().unwrap().numeric_json()["real"], 0.);
}

#[test]
#[ignore = "CPU-only real pointer path for a geometry-cache hit"]
fn benchmark_4k_drag_preserving_definition_order() {
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    let mut app = fixture(4000, 0);
    app.automatic_calculation = false;
    app.selected = Some(app.document.regions[0].id);
    app.enable_background_calculation(ctx.clone());
    app.calculation_changed(false);
    let settle = |app: &mut WorksheetApp| {
        let deadline = Instant::now() + std::time::Duration::from_secs(10);
        while app.background.running {
            app.poll_background_calculation();
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
    };
    settle(&mut app);
    app.dirty = false;
    let id = app.document.regions[0].id;
    let frame = |app: &mut WorksheetApp, events: Vec<egui::Event>| {
        super::super::tests::frame(app, &ctx, events)
    };
    frame(&mut app, vec![]);
    frame(&mut app, vec![]);
    let handle = ctx.read_response(egui::Id::new(("move", id))).unwrap();
    let point = ctx
        .layer_transform_to_global(handle.layer_id)
        .unwrap_or_default()
        * handle.rect.center();
    let button = |pos, pressed| {
        vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            },
        ]
    };
    let baseline = app
        .background
        .preparation
        .geometry
        .as_ref()
        .unwrap()
        .source
        .clone();
    let signature = app.document.active_calculation_signature();
    let generation = app.background.submitted_generation();
    let started = Instant::now();
    frame(&mut app, button(point, true));
    let press = started.elapsed().as_micros();
    let started = Instant::now();
    for i in 0..60 {
        frame(
            &mut app,
            vec![egui::Event::PointerMoved(
                point + vec2(20. + (i % 20) as f32, 0.),
            )],
        );
    }
    let deltas = started.elapsed().as_micros();
    assert_eq!(app.document.active_calculation_signature(), signature);
    assert_eq!(app.background.submitted_generation(), generation);
    assert!(!app.dirty);
    assert!(app.undo.is_empty());
    let started = Instant::now();
    frame(&mut app, button(point + vec2(39., 0.), false));
    let drop_time = started.elapsed().as_micros();
    assert!(
        Arc::ptr_eq(
            &baseline,
            &app.background.preparation.geometry.as_ref().unwrap().source
        ),
        "real move must reuse the exact old context/template"
    );
    assert_ne!(app.document.active_calculation_signature(), signature);
    assert_eq!(app.background.submitted_generation(), generation + 1);
    assert_eq!(app.undo.len(), 1);
    settle(&mut app);
    println!(
        "DRAG_SAME_ORDER regions=4000 press_us={press} frames=60 delta_us={deltas} drop_us={drop_time} intermediate_submissions=0 drop_submissions=1 exact_geometry_cache_hit=true"
    );
}
