//! Source-bound Area import/edit/cache/export oracle; original files are never changed.
use super::*;
use std::io::{Cursor, Read, Write};

fn build_native(path: &Path) {
    let mut doc = CanvasDocument::default();
    for (top, text) in [
        (
            30.,
            "<ml:define><ml:id>x</ml:id><ml:real>3</ml:real></ml:define>",
        ),
        (
            80.,
            "<ml:eval><ml:id>x</ml:id><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>",
        ),
        (
            220.,
            "<ml:eval><ml:id>x</ml:id><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>",
        ),
    ] {
        let id = doc.add_math(20., top);
        doc.regions.iter_mut().find(|r| r.id == id).unwrap().content = CanvasContent::Math(
            worksheet_math::tree_from_mathml(&format!(
                "<math xmlns:ml=\"http://schemas.mathsoft.com/math50\">{text}</math>"
            ))
            .unwrap(),
        );
    }
    mathfolio_document::export::export_mcdx(&doc, path).unwrap();
    let bytes = std::fs::read(path).unwrap();
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut writer = zip::ZipWriter::new(Cursor::new(vec![]));
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).unwrap();
        let mut bytes = vec![];
        entry.read_to_end(&mut bytes).unwrap();
        if entry.name().unwrap() == "mathcad/worksheet.xml" {
            let xml = String::from_utf8(bytes).unwrap();
            let dom = roxmltree::Document::parse(&xml).unwrap();
            let regions = dom
                .descendants()
                .find(|n| n.is_element() && n.tag_name().name() == "regions")
                .unwrap();
            let children: Vec<_> = regions.children().filter(|n| n.is_element()).collect();
            let area = format!(
                "<region region-id=\"100\" left=\"20\" top=\"20\"><Area IsCollapsed=\"false\" IsBorderVisible=\"true\" AreaHeight=\"150\"><regions>{}{}</regions></Area></region>{}",
                &xml[children[0].range()],
                &xml[children[1].range()],
                &xml[children[2].range()]
            );
            let mut changed = xml.clone();
            changed.replace_range(children[0].range().start..children[2].range().end, &area);
            bytes = changed.into_bytes();
        }
        writer
            .start_file(
                entry.name().unwrap(),
                zip::write::SimpleFileOptions::default().compression_method(entry.compression()),
            )
            .unwrap();
        writer.write_all(&bytes).unwrap();
    }
    std::fs::write(path, writer.finish().unwrap().into_inner()).unwrap();
}
fn run(output: &Path) {
    std::fs::create_dir_all(output).unwrap();
    let native = output.join("area-source.mcdx");
    build_native(&native);
    let mut app = WorksheetApp::default();
    app.open(&native);
    assert!(app.last_executed_regions.is_empty(), "{}", app.status);
    assert_eq!(app.document.regions.len(), 2);
    let owner = app
        .document
        .regions
        .iter()
        .find(|r| matches!(&r.content,CanvasContent::Opaque{kind,..}if kind=="Area"))
        .unwrap()
        .id;
    app.recalculate();
    assert!(
        app.calculation_error.is_none(),
        "{:?}",
        app.calculation_error
    );
    assert!(
        app.area_results[&owner]
            .values()
            .all(|row| row.error.is_none())
    );
    let xml = match &app
        .document
        .regions
        .iter()
        .find(|r| r.id == owner)
        .unwrap()
        .content
    {
        CanvasContent::Opaque { xml, .. } => xml.clone(),
        _ => unreachable!(),
    };
    let mut area = area_ui::AreaEditor::new(&xml).unwrap();
    area.activate("[0,0,0]").unwrap();
    let editor = area.active_editor_mut().unwrap();
    let rhs = editor.arena().get(editor.root()).unwrap().children()[1];
    editor.set_atom(rhs, NodeKind::Number("7".into())).unwrap();
    area.mark_active_changed();
    app.area_editors.insert(owner, area);
    app.selected = Some(owner);
    app.editing_region = Some(owner);
    app.stage_math_edit(owner, false);
    app.calculate_command(CalculationCommand::Worksheet);
    for row in app.area_results[&owner]
        .values()
        .chain(app.results.values())
    {
        assert!(row.error.is_none(), "{row:?}");
        if let Some(Value::Scalar { quantity }) = &row.value {
            assert_eq!(quantity.value.re, 7.);
        }
    }
    app.persist_calculation_cache();
    app.document.validate().unwrap();
    let saved = output.join("area-edited.mathfolio");
    app.document.save(&saved).unwrap();
    let mut reopened = WorksheetApp::default();
    reopened.open(&saved);
    assert!(
        reopened.last_executed_regions.is_empty() && reopened.pending_regions.is_empty(),
        "{} {:?}",
        reopened.status,
        reopened.pending_regions
    );
    assert_eq!(reopened.document.regions.len(), 2);
    assert_eq!(reopened.area_saved[&owner].len(), 2);
    let display = crate::result_display::FormattedResult::from_saved(
        reopened.area_saved[&owner]["[0,0,1]"]
            .display
            .as_deref()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(display.cells[0].plain(), "7");
    let exported = output.join("area-edited.mcdx");
    mathfolio_document::export::export_mcdx(&app.document, &exported).unwrap();
    let mut imported = WorksheetApp::default();
    imported.open(&exported);
    assert!(imported.last_executed_regions.is_empty());
    imported.recalculate();
    assert_eq!(imported.document.regions.len(), 2);
    assert!(
        imported.area_results[&owner]
            .values()
            .all(|row| row.error.is_none())
    );
    assert!(
        imported.area_results[&owner]
            .values()
            .any(|row| matches!(&row.value,Some(Value::Scalar{quantity})if quantity.value.re==7.))
    );
    println!(
        "AREA_NATIVE regions=2 child_rows=2 saved_reopen_execution=0 saved_reopen_pending=0 native_recalculate_errors=0"
    );
}
#[test]
fn source_bound_area_edit_commit_cache_and_native_export_keep_nested_ownership() {
    let output = std::env::temp_dir().join(format!(
        "mathfolio-area-native-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    run(&output);
    for name in [
        "area-source.mcdx",
        "area-edited.mcdx",
        "area-edited.mathfolio",
    ] {
        std::fs::remove_file(output.join(name)).unwrap();
    }
    std::fs::remove_dir(output).unwrap();
}
#[test]
#[ignore = "Explicit source-bound Area MCDX/cache/PDF integration fixture; no GUI or original runtime"]
fn area_native_edit_save_export_probe() {
    run(&PathBuf::from(
        std::env::var_os("MATHFOLIO_AREA_NATIVE_OUTPUT").expect("MATHFOLIO_AREA_NATIVE_OUTPUT"),
    ));
}
