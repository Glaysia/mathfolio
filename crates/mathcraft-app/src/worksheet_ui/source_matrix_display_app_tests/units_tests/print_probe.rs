//! Actual typed source viewport and full numeric result through readonly PDF.
//! Rust calculation setup once; no original runtime or original print parity.
use super::*;
use sha2::{Digest, Sha256};

fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn values(value: &Value) -> Vec<f64> {
    let Value::Matrix { elements, .. } = value else {
        panic!("missing full matrix")
    };
    elements.iter().map(|q| q.value.re).collect()
}

fn source_focus(app: &WorksheetApp, ctx: &egui::Context) -> serde_json::Value {
    let focused = ctx.memory(|memory| memory.focused());
    let mut sources = Vec::new();
    for region in &app.document.regions {
        let response =
            super::super::super::matrix_result_view_app_tests::observed_formula(region.id)
                .expect("actual source response must be observed");
        let preedit = ctx
            .data(|data| data.get_temp::<String>(response.id.with("ime-preedit")))
            .unwrap_or_default();
        assert!(!response.has_focus() && focused != Some(response.id));
        assert!(preedit.is_empty());
        sources.push(serde_json::json!({
            "owner":region.id,"response_id":format!("{:?}",response.id),
            "has_focus":response.has_focus(),"ime_preedit_chars":preedit.chars().count()
        }));
    }
    assert!(app.editing_region.is_none() && app.focus_new.is_none());
    assert!(app.text_composition.is_none());
    serde_json::json!({"sources":sources,"global_focused_id":focused.map(|id|format!("{id:?}"))})
}

fn case(output: &Path, unit: bool) -> serde_json::Value {
    let scratch = Scratch::new();
    let path = if unit {
        unit_fixture(&scratch.0, None)
    } else {
        fixture(&scratch.0, None)
    };
    let original = std::fs::read(&path).unwrap();
    let original_package = NativePackage::open(&path).unwrap();
    let source_xml = source_matrix(&original_package);
    let mut app = manual_open(&path);
    let def = definition(&app);
    let owner = query(&app);
    let regions = app.document.regions.clone();
    let source_tops: Vec<_> = regions.iter().map(|r| r.top).collect();
    let worksheet = original_package.worksheet_xml().unwrap();
    let parsed = roxmltree::Document::parse(&worksheet).unwrap();
    let native_tops: Vec<f64> = parsed
        .descendants()
        .filter(|n| n.has_tag_name((WS, "region")))
        .map(|n| n.attribute("top").unwrap().parse().unwrap())
        .collect();
    // The native fixture preserves add_math's normal grid snapping, so these
    // are the actual exported/imported coordinates, not the unsnapped seeds.
    assert_eq!(source_tops, native_tops);
    assert_eq!(source_tops.len(), 2);
    assert!(source_tops[0] < 100. && source_tops[1] > 170.);
    let source_signature = app.document.calculation_signature();
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    app.ribbon_command(&ctx, RibbonAction::Recalculate);
    assert!(app.results[&def].error.is_none(), "{:?}", app.results[&def]);
    let row = &app.results[&owner];
    assert!(row.error.is_none(), "{row:?}");
    if unit {
        matrix_value(row.value.as_ref().unwrap(), 1., LENGTH);
        matrix_value(row.display_value.as_ref().unwrap(), 100., [0.; 7]);
    } else {
        assert_value(row.value.as_ref().unwrap());
    }
    let physical = values(row.value.as_ref().unwrap());
    let display = values(row.display_value.as_ref().or(row.value.as_ref()).unwrap());
    let setup_executed = app.last_executed_regions.clone();
    assert!(
        setup_executed.contains(&def.to_string()) && setup_executed.contains(&owner.to_string())
    );
    app.last_executed_regions.clear();
    app.persist_calculation_cache();
    let source = app.document.regions.iter().find(|r| r.id == def).unwrap();
    let CanvasContent::Math(tree) = &source.content else {
        panic!("typed source required")
    };
    let source_tree = serde_json::to_value(tree).unwrap();
    assert_eq!(tree.children[1].children.len(), 6);
    let NodeKind::Matrix {
        rows: 2,
        cols: 3,
        display: Some(view),
    } = tree.children[1].kind
    else {
        panic!("source matrix display missing")
    };
    assert_eq!(view.window([2, 3]).unwrap(), ([0, 1], [1, 2]));
    assert!(app.document.matrix_result_views.is_empty());
    assert_eq!(app.document.regions, regions);
    assert_eq!(app.document.calculation_signature(), source_signature);
    assert!(app.pending_regions.is_empty() && !app.calculation_pending && !app.background.running);

    // Warm actual UI once with no source focus. No blanket global focus claim:
    // ribbon/scroll can own focus independently of the two source responses.
    app.editing_region = None;
    app.focus_new = None;
    if let Some(id) = ctx.memory(|memory| memory.focused()) {
        ctx.memory_mut(|memory| memory.surrender_focus(id));
    }
    frame(&mut app, &ctx, 10);
    let focus = source_focus(&app, &ctx);
    assert!(app.last_executed_regions.is_empty());
    let document = app.document.clone();
    let saved = app.saved_results.clone();
    let results = serde_json::to_value(&app.results).unwrap();
    let pending = app.pending_regions.clone();
    let generation = app.background.submitted_generation();
    let history = (app.undo.len(), app.redo.len());
    let dirty = app.dirty;
    let name = if unit { "source-unit" } else { "source-plain" };
    let pdf = output.join(format!("{name}.pdf"));
    app.export_vector_pdf_for_test(&pdf).unwrap();
    assert_eq!(app.document, document);
    assert_eq!(app.saved_results, saved);
    assert_eq!(serde_json::to_value(&app.results).unwrap(), results);
    assert_eq!(app.pending_regions, pending);
    assert_eq!(app.background.submitted_generation(), generation);
    assert_eq!((app.undo.len(), app.redo.len()), history);
    assert_eq!(app.dirty, dirty);
    assert!(app.last_executed_regions.is_empty());
    assert!(!app.background.running && !app.calculation_pending);
    assert_eq!(std::fs::read(&path).unwrap(), original);
    let bytes = std::fs::read(&pdf).unwrap();
    serde_json::json!({
        "name":name,"unit_source":unit,"full_shape":[2,3],"physical_row_major":physical,
        "physical_dimension":if unit {LENGTH}else{[0.;7]},"display_row_major":display,
        "visible_source_row_major":[2,3],"source_display":{"offset":[0,1],"size":[1,2]},
        "definition_owner":def,"query_owner":owner,"source_tops":source_tops,
        "source_tree":source_tree,"source_matrix_xml":source_xml,
        "source_signature":source_signature,"input_sha256":sha(&original),
        "setup_calculation_requests":1,"setup_executed_region_ids":setup_executed,
        "executions_after_setup":app.last_executed_regions,"generation_before_print":generation,
        "generation_after_print":app.background.submitted_generation(),
        "history_before_print":[history.0,history.1],"history_after_print":[app.undo.len(),app.redo.len()],
        "source_focus":focus,"print_snapshot":"existing readonly snapshot/vector writer",
        "file":{"name":format!("{name}.pdf"),"bytes":bytes.len(),"sha256":sha(&bytes)}
    })
}

#[test]
#[ignore = "Explicit typed source viewport/full result readonly vector PDF packet"]
fn source_matrix_viewport_print_probe() {
    let _observing = super::super::super::matrix_result_view_app_tests::Observe::start();
    let output = PathBuf::from(
        std::env::var_os("MATHCRAFT_SOURCE_MATRIX_PRINT_OUTPUT")
            .expect("MATHCRAFT_SOURCE_MATRIX_PRINT_OUTPUT"),
    );
    assert!(!output.exists(), "use a fresh output directory");
    std::fs::create_dir_all(&output).unwrap();
    let cases = [case(&output, false), case(&output, true)];
    std::fs::write(
        output.join("observation.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"version":1,"original_runtime":false,"pdfs":2,"cases":cases}),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(std::fs::read_dir(&output).unwrap().count(), 3);
    println!("SOURCE_MATRIX_PRINT_PACKET cases=2 pdf=2 observations=1");
}
