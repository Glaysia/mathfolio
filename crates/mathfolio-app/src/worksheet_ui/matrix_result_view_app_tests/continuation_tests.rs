//! Actual full-App frame tests; generated Rust fixture only, no original runtime.
//! Registered as child of existing matrix_result_view_app_tests to reuse private
//! actual frame/pointer fixture without widening production/helper visibility.
use super::*;
use mathfolio_document::canvas::{MatrixResultViewport, MatrixViewSize};
fn expanded(f: &mut Fixture) {
    let start = f.begin();
    let moved = start + vec2(0., 600.);
    f.frame(vec![egui::Event::PointerMoved(moved)]);
    f.frame(button(moved, false));
    f.frame(vec![]);
    let view = f.app.display_cache[&f.owner].matrix.value().unwrap();
    assert_eq!(view.rows, 20);
    assert_eq!(view.cols, 12);
}
#[test]
fn actual_expanded_matrix_enter_uses_complete_rendered_height_without_execution() {
    let mut f = Fixture::new(1.);
    expanded(&mut f);
    f.app.selected = Some(f.owner);
    f.app.editing_region = Some(f.owner);
    f.app.focus_new = Some(f.owner);
    let editor = f.app.editors.get_mut(&f.owner).unwrap();
    editor
        .place_caret(editor.root(), mathfolio_editor::Direction::Right)
        .unwrap();
    f.frame(vec![]);
    let source = Inert::new(&f.app);
    let owner = f
        .app
        .document
        .regions
        .iter()
        .find(|r| r.id == f.owner)
        .unwrap();
    let top = owner.top;
    let actual_height = f64::from(f.app.region_heights[&f.owner]);
    let grid = f.app.document.grid;
    let undo = f.app.undo.len();
    let views = f.app.document.matrix_result_views.clone();
    f.frame(vec![egui::Event::Key {
        key: egui::Key::Enter,
        physical_key: Some(egui::Key::Enter),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }]);
    let insertion = f
        .app
        .insertion
        .expect("actual Enter failed to leave a matrix formula");
    assert!(
        insertion.1 >= top + actual_height,
        "insertion{} overlaps rendered matrix bottom{}",
        insertion.1,
        top + actual_height
    );
    assert!(
        insertion.1 <= top + actual_height + grid * 3.,
        "unexpected excessive gap"
    );
    assert_eq!(f.app.undo.len(), undo);
    assert_eq!(f.app.document.matrix_result_views, views);
    source.check(&f.app);
}
#[test]
fn own_duplicate_copies_requested_view_independently_and_undo_restores_source_status() {
    let mut f = Fixture::new(1.);
    expanded(&mut f);
    let original = f.app.document.regions.clone();
    let signature = f.app.document.calculation_signature();
    let source_rows = rows(&f.app);
    let source_saved = f.app.saved_results.clone();
    let requested = *f
        .app
        .document
        .matrix_result_views
        .get(&MatrixResultTarget::math(f.owner))
        .unwrap();
    f.app.undo.clear();
    f.app.redo.clear();
    f.app.last_executed_regions.clear();
    f.app.selected = Some(f.owner);
    f.app.duplicate_selected();
    let copied = f.app.selected.unwrap();
    assert_ne!(copied, f.owner);
    assert_eq!(
        f.app
            .document
            .matrix_result_views
            .get(&MatrixResultTarget::math(copied)),
        Some(&requested)
    );
    assert_eq!(
        f.app
            .document
            .matrix_result_views
            .get(&MatrixResultTarget::math(f.owner)),
        Some(&requested)
    );
    for old in &original {
        assert_eq!(
            f.app.document.regions.iter().find(|r| r.id == old.id),
            Some(old)
        );
    }
    assert!(f.app.last_executed_regions.is_empty());
    assert_eq!(f.app.undo.len(), 1);
    let changed = MatrixResultViewport {
        size: Some(MatrixViewSize { rows: 10, cols: 11 }),
        ..requested
    };
    f.app.set_matrix_result_view(copied, Some(changed)).unwrap();
    assert_eq!(
        f.app
            .document
            .matrix_result_views
            .get(&MatrixResultTarget::math(f.owner)),
        Some(&requested)
    );
    f.app.undo();
    assert_eq!(f.app.document.regions, original);
    assert_eq!(f.app.document.calculation_signature(), signature);
    assert!(
        f.app
            .document
            .matrix_result_views
            .get(&MatrixResultTarget::math(copied))
            .is_none()
    );
    assert_eq!(rows(&f.app), source_rows);
    assert_eq!(f.app.saved_results, source_saved);
    assert!(f.app.last_executed_regions.is_empty());
    f.app.redo();
    assert_eq!(
        f.app
            .document
            .matrix_result_views
            .get(&MatrixResultTarget::math(copied)),
        Some(&changed)
    );
    assert!(f.app.last_executed_regions.is_empty());
}
#[test]
fn duplicate_map_budget_failure_is_atomic_before_owner_id_history_or_source_change() {
    let mut f = Fixture::new(1.);
    let one = MatrixResultViewport {
        size: Some(MatrixViewSize { rows: 1, cols: 1 }),
        ..Default::default()
    };
    while f.app.document.regions.len() < 4096 {
        f.app.document.add_math(0., 0.);
    }
    for r in &f.app.document.regions {
        f.app
            .document
            .matrix_result_views
            .set(MatrixResultTarget::math(r.id), one)
            .unwrap();
    }
    f.app.selected = Some(f.owner);
    let document = serde_json::to_vec(&f.app.document).unwrap();
    let next = f.app.document.next_id;
    let undo = f.app.undo.len();
    let dirty = f.app.dirty;
    f.app.duplicate_selected();
    assert_eq!(serde_json::to_vec(&f.app.document).unwrap(), document);
    assert_eq!(f.app.document.next_id, next);
    assert_eq!(f.app.undo.len(), undo);
    assert_eq!(f.app.dirty, dirty);
    assert!(f.app.status.contains("limit"));
    assert!(f.app.last_executed_regions.is_empty());
}
#[test]
fn duplicate_native_local_view_survives_source_owner_clear() {
    use std::io::{Cursor, Write};
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    struct Scratch(PathBuf);
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let scratch = loop {
        let p = std::env::temp_dir().join(format!(
            "matrix-duplicate-{}-{}",
            std::process::id(),
            SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        match std::fs::create_dir(&p) {
            Ok(()) => break Scratch(p),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => panic!("{e}"),
        }
    };
    let ws = "http://schemas.mathsoft.com/worksheet50";
    let ml = "http://schemas.mathsoft.com/math50";
    let source = format!(
        "<ws:worksheet xmlns:ws='{ws}' xmlns:ml='{ml}'><ws:regions><ws:region region-id='42' left='20' top='100'><ws:math resultRef='900'><ml:eval><ml:id labels='VARIABLE'>M</ml:id><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval><ws:resultFormat><ws:matrix size='13,14' offset='2,3' show-indices='false' expand-nested-arrays='false'/></ws:resultFormat></ws:math></ws:region></ws:regions></ws:worksheet>"
    );
    let cells = (1..=400)
        .map(|n| format!("<ml:real>{n}</ml:real>"))
        .collect::<String>();
    let result = format!(
        "<resultsList xmlns='http://schemas.mathsoft.com/result10' xmlns:ml='{ml}'><resultData result-id='900' calculation-status='Synchronized'><ml:result><ml:matrix rows='20' cols='20'>{cells}</ml:matrix></ml:result></resultData></resultsList>"
    );
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, text) in [
        ("mathcad/worksheet.xml", source),
        ("mathcad/result.xml", result),
    ] {
        zip.start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(text.as_bytes()).unwrap();
    }
    let native = scratch.0.join("source.mcdx");
    std::fs::write(&native, zip.finish().unwrap().into_inner()).unwrap();
    let mut f = Fixture::new(1.);
    f.app.open(&native);
    f.owner = f.app.document.regions[0].id;
    f.app.selected = Some(f.owner);
    f.app.insertion = None;
    for _ in 0..4 {
        f.frame(vec![]);
    }
    assert!(f.app.last_executed_regions.is_empty());
    assert!(f.app.document.regions[0].source_region.is_some());
    let old = f.app.display_cache[&f.owner].matrix.value().unwrap();
    assert_eq!(
        (old.rows, old.cols, old.row_offset, old.col_offset),
        (13, 14, 2, 3)
    );
    assert!(f.app.document.matrix_result_views.is_empty());
    f.app.duplicate_selected();
    let copied = f.app.selected.unwrap();
    assert_ne!(copied, f.owner);
    assert!(
        f.app
            .document
            .regions
            .iter()
            .find(|r| r.id == copied)
            .unwrap()
            .source_region
            .is_none()
    );
    let view = f
        .app
        .document
        .matrix_result_views
        .get(&MatrixResultTarget::math(copied))
        .unwrap();
    assert_eq!(view.size, Some(MatrixViewSize { rows: 13, cols: 14 }));
    assert_eq!(
        view.offset,
        Some(mathfolio_document::canvas::MatrixViewOffset { row: 2, col: 3 })
    );
    assert_eq!(view.show_indices, Some(false));
    assert!(
        f.app
            .document
            .matrix_result_views
            .get(&MatrixResultTarget::math(f.owner))
            .is_none()
    );
    assert!(f.app.last_executed_regions.is_empty());
}
