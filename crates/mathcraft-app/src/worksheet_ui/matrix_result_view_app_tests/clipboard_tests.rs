use super::*;
use mathcraft_document::canvas::MatrixResultViewport;
fn expanded(f: &mut Fixture) -> MatrixResultViewport {
    let start = f.begin();
    let moved = start + vec2(100., 100.);
    f.frame(vec![egui::Event::PointerMoved(moved)]);
    f.frame(button(moved, false));
    f.frame(vec![]);
    *f.app
        .document
        .matrix_result_views
        .get(&MatrixResultTarget::math(f.owner))
        .unwrap()
}
fn copy_event(f: &mut Fixture, cut: bool) -> Option<String> {
    f.app.editing_region = None;
    f.app.selected = Some(f.owner);
    f.tick += 1;
    let mut raw = egui::RawInput {
        events: vec![if cut {
            egui::Event::Cut
        } else {
            egui::Event::Copy
        }],
        time: Some(f.tick as f64 / 60.),
        screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(2400., 2200.))),
        ..Default::default()
    };
    eframe::App::raw_input_hook(&mut f.app, &f.ctx, &mut raw);
    let output = f.ctx.run_ui(raw, |ui| f.app.app_ui_frame(ui));
    let copied = output.platform_output.commands.iter().find_map(|c| {
        if let egui::OutputCommand::CopyText(s) = c {
            Some(s.clone())
        } else {
            None
        }
    });
    output.drop_without_applying_deltas();
    copied
}
#[test]
fn actual_region_copy_paste_keeps_view_new_target_without_copying_result_authority() {
    let mut f = Fixture::new(1.);
    let requested = expanded(&mut f);
    let source = Inert::new(&f.app);
    let payload = copy_event(&mut f, false).expect("actual Copy did not emit region payload");
    source.check(&f.app);
    let raw: serde_json::Value =
        serde_json::from_str(payload.strip_prefix("mathcraft-region:").unwrap()).unwrap();
    assert_eq!(
        serde_json::from_value::<MatrixResultViewport>(raw["matrix_result_view"].clone()).unwrap(),
        requested
    );
    for forbidden in [
        "numeric_source",
        "result",
        "value",
        "calculation_cache",
        "native_archive",
    ] {
        assert!(raw.get(forbidden).is_none());
    }
    f.app.undo.clear();
    f.app.redo.clear();
    f.app.editing_region = None;
    f.frame(vec![egui::Event::Paste(payload)]);
    let pasted = f.app.selected.unwrap();
    assert_ne!(pasted, f.owner);
    assert_eq!(
        f.app
            .document
            .matrix_result_views
            .get(&MatrixResultTarget::math(pasted)),
        Some(&requested)
    );
    assert_eq!(
        f.app
            .document
            .matrix_result_views
            .get(&MatrixResultTarget::math(f.owner)),
        Some(&requested)
    );
    assert!(f.app.last_executed_regions.is_empty());
    assert!(
        f.app
            .saved_results
            .get(&pasted)
            .is_none_or(|r| r.numeric_source.is_none())
    );
    assert!(
        f.app
            .results
            .get(&pasted)
            .is_none_or(|r| r.value.is_none() && r.display_value.is_none())
    );
    f.app.undo();
    assert!(
        f.app
            .document
            .matrix_result_views
            .get(&MatrixResultTarget::math(pasted))
            .is_none()
    );
    assert!(f.app.document.regions.iter().all(|r| r.id != pasted));
    f.app.redo();
    assert_eq!(
        f.app
            .document
            .matrix_result_views
            .get(&MatrixResultTarget::math(pasted)),
        Some(&requested)
    );
}
#[test]
fn actual_cut_prepares_metadata_before_removal_then_paste_and_undo_keep_new_ids() {
    let mut f = Fixture::new(1.);
    let requested = expanded(&mut f);
    let source_tree = f
        .app
        .document
        .regions
        .iter()
        .find(|r| r.id == f.owner)
        .unwrap()
        .content
        .clone();
    f.app.undo.clear();
    f.app.redo.clear();
    let payload = copy_event(&mut f, true).unwrap();
    assert!(f.app.document.regions.iter().all(|r| r.id != f.owner));
    assert!(
        f.app
            .document
            .matrix_result_views
            .get(&MatrixResultTarget::math(f.owner))
            .is_none()
    );
    f.frame(vec![egui::Event::Paste(payload)]);
    let pasted = f.app.selected.unwrap();
    assert_ne!(pasted, f.owner);
    assert_eq!(
        f.app
            .document
            .regions
            .iter()
            .find(|r| r.id == pasted)
            .unwrap()
            .content,
        source_tree
    );
    assert_eq!(
        f.app
            .document
            .matrix_result_views
            .get(&MatrixResultTarget::math(pasted)),
        Some(&requested)
    );
    f.app.undo();
    assert!(f.app.document.regions.iter().all(|r| r.id != pasted));
    f.app.undo();
    assert_eq!(
        f.app
            .document
            .regions
            .iter()
            .find(|r| r.id == f.owner)
            .unwrap()
            .content,
        source_tree
    );
    assert_eq!(
        f.app
            .document
            .matrix_result_views
            .get(&MatrixResultTarget::math(f.owner)),
        Some(&requested)
    );
    assert!(f.app.last_executed_regions.is_empty());
}
#[test]
fn forged_wrong_type_and_overbudget_paste_refuse_before_document_history_mutation() {
    let mut f = Fixture::new(1.);
    let requested = expanded(&mut f);
    let source = serde_json::json!({"content":CanvasContent::Text("wrong target".into()),"matrix_result_view":requested});
    let before = serde_json::to_vec(&f.app.document).unwrap();
    let undo = f.app.undo.len();
    assert!(f.app.paste_region(&format!("mathcraft-region:{source}")));
    assert!(
        serde_json::to_vec(&f.app.document).unwrap() == before,
        "Paste changed document on refusal"
    );
    assert_eq!(f.app.undo.len(), undo);
    let payload = copy_event(&mut f, false).unwrap();
    let one = MatrixResultViewport {
        size: Some(mathcraft_document::canvas::MatrixViewSize { rows: 1, cols: 1 }),
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
    let before = serde_json::to_vec(&f.app.document).unwrap();
    let next = f.app.document.next_id;
    let undo = f.app.undo.len();
    assert!(f.app.paste_region(&payload));
    assert!(
        serde_json::to_vec(&f.app.document).unwrap() == before,
        "Paste changed document on refusal"
    );
    assert_eq!(f.app.document.next_id, next);
    assert_eq!(f.app.undo.len(), undo);
    assert!(f.app.status.contains("limit"));
}
