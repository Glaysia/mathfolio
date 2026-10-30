//! Actual full-App shapes and lifecycle; extra Rust calculation is setup only.
use super::*;
use mathfolio_document::canvas::{MatrixResultViewport, MatrixViewOffset};
use std::sync::atomic::{AtomicU64, Ordering};

struct Paint {
    texts: Vec<String>,
    rect: Rect,
    focus: bool,
    ime_cursor: Option<Rect>,
}
fn formula_id(f: &Fixture) -> Option<egui::Id> {
    FORMULA_IDS.with_borrow(|slot| slot.as_ref().and_then(|ids| ids.get(&f.owner)).copied())
}
fn paint(f: &mut Fixture) -> Paint {
    f.tick += 1;
    let mut raw = egui::RawInput {
        time: Some(f.tick as f64 / 60.),
        screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(2400., 2200.))),
        ..Default::default()
    };
    eframe::App::raw_input_hook(&mut f.app, &f.ctx, &mut raw);
    // Context::read_response may still expose previous-pass rectangles on the
    // first geometry-changing frame. Collect against this real widget's own
    // returned Response; keep first-frame shape/clip and numeric goldens exact.
    FORMULA_RESPONSES.with_borrow_mut(HashMap::clear);
    let output = f.ctx.run_ui(raw, |ui| f.app.app_ui_frame(ui));
    let response = FORMULA_RESPONSES.with_borrow(|responses| responses.get(&f.owner).cloned());
    let rect = response.as_ref().map(|response| {
        f.ctx
            .layer_transform_to_global(response.layer_id)
            .unwrap_or_default()
            * response.rect
    });
    let mut texts = Vec::new();
    fn collect(shape: &egui::Shape, rect: Rect, clip: Rect, texts: &mut Vec<String>) {
        match shape {
            egui::Shape::Text(text) => {
                let bounds = Rect::from_min_size(text.pos, text.galley.size());
                if rect.expand(1.).contains(bounds.center()) && clip.intersects(bounds) {
                    texts.push(text.galley.text().to_owned());
                }
            }
            egui::Shape::Vec(children) => {
                for child in children {
                    collect(child, rect, clip, texts);
                }
            }
            _ => {}
        }
    }
    if let Some(rect) = rect {
        for clipped in &output.shapes {
            collect(&clipped.shape, rect, clipped.clip_rect, &mut texts);
        }
    }
    let focus = response.as_ref().is_some_and(egui::Response::has_focus);
    let ime_cursor = output
        .platform_output
        .ime
        .as_ref()
        .map(|ime| ime.cursor_rect);
    // No assertions/expect while FullOutput is alive.
    output.drop_without_applying_deltas();
    Paint {
        texts,
        rect: rect.expect("actual observed formula response missing"),
        focus,
        ime_cursor,
    }
}
fn count(paint: &Paint, text: &str) -> usize {
    paint
        .texts
        .iter()
        .filter(|value| value.as_str() == text)
        .count()
}
fn labels(paint: &Paint, origin: i32) {
    let mut expected: HashMap<String, usize> = HashMap::new();
    for offset in [2_i32, 3] {
        for index in offset..offset + 12 {
            *expected.entry((origin + index).to_string()).or_default() += 1;
        }
    }
    expected.insert(format!("{origin}\n⋮"), 2);
    expected.insert(format!(" ⋮\n{}", origin + 19), 2);
    for (text, expected) in expected {
        assert_eq!(
            count(paint, &text),
            expected,
            "label {text:?}: {:?}",
            paint.texts
        );
    }
}
fn no_labels(paint: &Paint, possible_origins: &[i32]) {
    for origin in possible_origins {
        assert_eq!(
            count(paint, &format!("{origin}\n⋮")),
            0,
            "stale before endpoint"
        );
        assert_eq!(
            count(paint, &format!(" ⋮\n{}", origin + 19)),
            0,
            "stale after endpoint"
        );
        // Use out-of-data-range origins. Matrix data in this fixture is1..400.
        if *origin >= 1000 {
            for index in 2..15 {
                assert_eq!(count(paint, &(origin + index).to_string()), 0);
            }
        }
    }
}
fn known(show: bool) -> Fixture {
    let mut f = Fixture::new(1.);
    let system = worksheet_math::tree_from_mathml(
        "<ml:define xmlns:ml=\"http://schemas.mathsoft.com/math50\"><ml:id labels=\"SYSTEM\">ORIGIN</ml:id><ml:real>1000</ml:real></ml:define>"
    ).unwrap();
    let definition = f.app.document.add_math(20., 0.);
    f.app
        .document
        .regions
        .iter_mut()
        .find(|region| region.id == definition)
        .unwrap()
        .content = CanvasContent::Math(system);
    // Actual SYSTEM binding in the executed Rust scope, not a fabricated row.
    // This setup execution is excluded from all subsequent zero-work checks.
    f.app.recalculate();
    assert_eq!(f.app.results[&f.owner].matrix_origin, Some(1000));
    assert!(f.app.results.values().all(|row| row.error.is_none()));
    f.app.persist_calculation_cache();
    f.app
        .set_matrix_result_view(
            f.owner,
            Some(MatrixResultViewport {
                offset: Some(MatrixViewOffset { row: 2, col: 3 }),
                show_indices: Some(show),
                ..Default::default()
            }),
        )
        .unwrap();
    f.app.selected = Some(f.owner);
    f.app.editing_region = None;
    f.app.focus_new = None;
    for _ in 0..3 {
        f.frame(vec![]);
    }
    f.app.last_executed_regions.clear();
    f.app.undo.clear();
    f.app.redo.clear();
    f.app.dirty = false;
    f
}
#[derive(PartialEq, Debug)]
struct Values {
    owner: u64,
    value: Option<serde_json::Value>,
    display: Option<serde_json::Value>,
}
fn values(app: &WorksheetApp) -> Vec<Values> {
    let mut values: Vec<_> = app
        .results
        .iter()
        .map(|(id, row)| Values {
            owner: *id,
            value: row.value.as_ref().map(Value::numeric_json),
            display: row.display_value.as_ref().map(Value::numeric_json),
        })
        .collect();
    values.sort_by_key(|row| row.owner);
    values
}
fn deliver(f: &mut Fixture, origin: Option<i32>, pending: bool) {
    let mut rows: Vec<_> = f.app.results.values().cloned().collect();
    rows.iter_mut()
        .find(|row| row.region_id == f.owner.to_string())
        .unwrap()
        .matrix_origin = origin;
    f.app.accept_calculation_update(CalculationUpdate {
        rows,
        pending_region_ids: if pending {
            vec![f.owner.to_string()]
        } else {
            vec![]
        },
        executed_region_ids: vec![],
        cancelled: false,
    });
}

#[test]
fn actual_app_live_origin_offset_labels_endpoints_and_source_caret_are_readonly() {
    let mut f = known(true);
    let id = formula_id(&f).unwrap();
    let editor = f.app.editors.get_mut(&f.owner).unwrap();
    let lhs = editor.arena().get(editor.root()).unwrap().children()[0];
    editor.place_text_caret(lhs, 1).unwrap();
    let caret = editor.caret();
    let selection = editor.selection();
    let source = editor.to_portable().unwrap();
    f.app.editing_region = Some(f.owner);
    f.ctx.memory_mut(|memory| memory.request_focus(id));
    let inert = Inert::new(&f.app);
    paint(&mut f); // establish IME/focus owner for the actual source widget
    let painted = paint(&mut f);
    assert!(painted.focus);
    labels(&painted, 1000);
    let cursor = painted
        .ime_cursor
        .expect("source caret has no actual IME rectangle");
    assert!(
        cursor.is_finite()
            && cursor.height() > 0.
            && painted.rect.expand(2.).contains(cursor.center())
    );
    assert_eq!(f.app.editors[&f.owner].caret(), caret);
    assert_eq!(f.app.editors[&f.owner].selection(), selection);
    assert_eq!(f.app.editors[&f.owner].to_portable().unwrap(), source);
    let navigator = id.with("matrix-result-navigator");
    for ordinal in 0_usize..4 {
        assert!(
            f.ctx
                .read_response(navigator.with(("marker", ordinal)))
                .is_some(),
            "missing real continuation hit {ordinal}"
        );
    }
    inert.check(&f.app);
    assert!(f.app.undo.is_empty() && f.app.redo.is_empty());
}

#[test]
fn focus_only_headings_are_transient_without_moving_source_caret_or_reserved_geometry() {
    let mut f = known(false);
    let inactive = paint(&mut f);
    assert!(!inactive.focus);
    no_labels(&inactive, &[1000]);
    let id = formula_id(&f).unwrap();
    let editor = f.app.editors.get_mut(&f.owner).unwrap();
    let lhs = editor.arena().get(editor.root()).unwrap().children()[0];
    editor.place_text_caret(lhs, 1).unwrap();
    let caret = editor.caret();
    let selection = editor.selection();
    let source = editor.to_portable().unwrap();
    let inert = Inert::new(&f.app);
    f.app.editing_region = Some(f.owner);
    f.ctx.memory_mut(|memory| memory.request_focus(id));
    paint(&mut f);
    let focused = paint(&mut f);
    assert!(focused.focus);
    labels(&focused, 1000);
    assert_eq!(focused.rect.size(), inactive.rect.size());
    assert_eq!(f.app.editors[&f.owner].caret(), caret);
    assert_eq!(f.app.editors[&f.owner].selection(), selection);
    f.ctx.memory_mut(|memory| memory.surrender_focus(id));
    f.app.editing_region = None;
    f.app.focus_new = None;
    let inactive_again = paint(&mut f);
    assert!(!inactive_again.focus);
    no_labels(&inactive_again, &[1000]);
    assert_eq!(inactive_again.rect.size(), focused.rect.size());
    assert_eq!(f.app.editors[&f.owner].caret(), caret);
    assert_eq!(f.app.editors[&f.owner].selection(), selection);
    assert_eq!(f.app.editors[&f.owner].to_portable().unwrap(), source);
    inert.check(&f.app);
    assert!(f.app.undo.is_empty() && f.app.redo.is_empty());
}

#[test]
fn equal_matrix_value_origin_only_callback_replaces_painted_labels_and_pending_hides_before_callback()
 {
    let mut f = known(true);
    labels(&paint(&mut f), 1000);
    let source = serde_json::to_vec(&f.app.document).unwrap();
    let value = values(&f.app);
    let generation = f.app.background.submitted_generation();
    let projection = f.app.display_cache[&f.owner]
        .matrix
        .value()
        .unwrap()
        .clone();
    deliver(&mut f, Some(2000), false);
    let next = paint(&mut f);
    labels(&next, 2000);
    no_labels(&next, &[1000]);
    assert_eq!(values(&f.app), value);
    assert_eq!(
        f.app.display_cache[&f.owner].matrix.value(),
        Some(&projection)
    );
    assert_eq!(serde_json::to_vec(&f.app.document).unwrap(), source);
    assert_eq!(f.app.background.submitted_generation(), generation);
    assert!(f.app.undo.is_empty() && f.app.redo.is_empty());
    f.app.pending_regions.insert(f.owner);
    f.app.calculation_pending = true;
    let pending = paint(&mut f);
    assert_eq!(
        f.app.results[&f.owner].matrix_origin,
        Some(2000),
        "do not conceal the gate bug by rewriting the old row"
    );
    no_labels(&pending, &[0, 1000, 2000]);
    assert!(f.app.matrix_indices_context().is_none());
    assert_eq!(values(&f.app), value);
    assert_eq!(
        f.app.display_cache[&f.owner].matrix.value(),
        Some(&projection)
    );
    deliver(&mut f, None, true);
    no_labels(&paint(&mut f), &[0, 1000, 2000]);
    deliver(&mut f, Some(2000), false);
    f.app.calculation_pending = false;
    labels(&paint(&mut f), 2000);
    assert_eq!(values(&f.app), value);
    assert_eq!(serde_json::to_vec(&f.app.document).unwrap(), source);
    assert_eq!(f.app.background.submitted_generation(), generation);
    assert!(f.app.last_executed_regions.is_empty() && !f.app.background.running);
    assert!(f.app.undo.is_empty() && f.app.redo.is_empty());
}

#[test]
fn indices_view_undo_leaves_private_math_draft_and_editor_undo_separate() {
    let mut f = known(false);
    f.app.editing_region = Some(f.owner);
    let editor = f.app.editors.get_mut(&f.owner).unwrap();
    let original = editor.to_portable().unwrap();
    let lhs = editor.arena().get(editor.root()).unwrap().children()[0];
    editor
        .place_caret(lhs, mathfolio_editor::Direction::Right)
        .unwrap();
    editor.insert_infix(BinaryOp::Add);
    let draft = editor.to_portable().unwrap();
    let caret = editor.caret();
    let selection = editor.selection();
    assert_ne!(draft, original);
    let inert = Inert::new(&f.app);
    f.app.ribbon_command(
        &f.ctx,
        RibbonAction::SetMatrixIndices {
            owner: f.owner,
            show: true,
        },
    );
    assert_eq!(f.app.undo.len(), 1);
    f.app.undo();
    assert!(!f.app.matrix_indices_context().unwrap().show_indices);
    assert_eq!(f.app.editors[&f.owner].to_portable().unwrap(), draft);
    assert_eq!(f.app.editors[&f.owner].caret(), caret);
    assert_eq!(f.app.editors[&f.owner].selection(), selection);
    f.app.redo();
    assert!(f.app.matrix_indices_context().unwrap().show_indices);
    assert_eq!(f.app.editors[&f.owner].to_portable().unwrap(), draft);
    assert_eq!(f.app.editors[&f.owner].caret(), caret);
    assert_eq!(f.app.editors[&f.owner].selection(), selection);
    assert!(f.app.editors.get_mut(&f.owner).unwrap().undo());
    assert_eq!(f.app.editors[&f.owner].to_portable().unwrap(), original);
    labels(&paint(&mut f), 1000);
    inert.check(&f.app);
    assert_eq!(f.app.undo.len(), 1);
    assert!(f.app.redo.is_empty());
}

static SERIAL: AtomicU64 = AtomicU64::new(0);
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        loop {
            let path = std::env::temp_dir().join(format!(
                "matrix-origin-lifecycle-{}-{}",
                std::process::id(),
                SERIAL.fetch_add(1, Ordering::Relaxed)
            ));
            match std::fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("{error}"),
            }
        }
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn query(app: &WorksheetApp) -> u64 {
    let mut queries = app.document.regions.iter().filter(|region| matches!(&region.content, CanvasContent::Math(tree) if matches!(tree.kind,NodeKind::Binary(BinaryOp::Evaluate))));
    let id = queries.next().unwrap().id;
    assert!(queries.next().is_none());
    id
}
fn cold(f: &mut Fixture, path: &Path) {
    f.app = WorksheetApp::default();
    f.app.open(path);
    f.owner = query(&f.app);
    f.app.selected = Some(f.owner);
    f.ctx = egui::Context::default();
    crate::configure(&f.ctx);
    f.tick = 0;
    FORMULA_IDS.with_borrow_mut(|slot| slot.as_mut().unwrap().clear());
}
#[test]
fn own_and_native_cold_keep_matrix_and_show_flag_without_reconstructing_origin_from_source() {
    let mut f = known(true);
    let scratch = Scratch::new();
    let own = scratch.0.join("known-live.mathfolio");
    let native = scratch.0.join("known-live.mcdx");
    f.app.persist_calculation_cache();
    f.app.document.save(&own).unwrap();
    f.app.prepare_native_export().unwrap();
    mathfolio_document::export::export_mcdx(&f.app.document, &native).unwrap();
    assert!(
        !String::from_utf8(std::fs::read(&own).unwrap())
            .unwrap()
            .contains("matrix_origin")
    );
    let package = NativePackage::open(&native).unwrap();
    assert!(
        !package
            .original_xml_part("mathcad/result.xml")
            .unwrap()
            .unwrap()
            .contains("matrix_origin")
    );
    for path in [&own, &native] {
        cold(&mut f, path);
        assert!(f.app.results.is_empty());
        assert!(f.app.pending_regions.is_empty(), "{}", f.app.status);
        // Own wire retains the requested override. Native cold reads the
        // source-local format instead; it need not manufacture a DTO entry.
        let effective = f
            .app
            .matrix_view_sources
            .effective(f.owner, f.app.requested_matrix_view(f.owner))
            .unwrap();
        assert_eq!(effective.show_indices, Some(true));
        if path == &own {
            assert_eq!(
                f.app
                    .document
                    .matrix_result_views
                    .get(&MatrixResultTarget::math(f.owner))
                    .unwrap()
                    .show_indices,
                Some(true)
            );
        }
        let before = Inert::new(&f.app);
        for _ in 0..4 {
            no_labels(&paint(&mut f), &[0, 1000, 2000]);
        }
        assert!(f.app.matrix_indices_context().is_none());
        assert_eq!(f.app.display_cache[&f.owner].matrix.shape(), Some((20, 20)));
        let view = f.app.display_cache[&f.owner].matrix.value().unwrap();
        assert_eq!(
            (view.rows, view.cols, view.row_offset, view.col_offset),
            (12, 12, 2, 3)
        );
        assert_eq!(view.cells[0].plain(), "44");
        assert_eq!(view.cells.last().unwrap().plain(), "275");
        before.check(&f.app);
        assert!(f.app.undo.is_empty() && f.app.redo.is_empty());
    }
}

mod callback_layout_tests;
