//! Actual command + real calculated Fixture; generated native metadata is
//! parsed through NativePackage/SourceFormats, not injected into private fields.
use super::*;
use mathfolio_document::canvas::{MatrixResultViewport, MatrixViewOffset, MatrixViewSize};
fn formats(
    f: &Fixture,
    size: &str,
    offset: Option<&str>,
    local: Option<&str>,
) -> matrix_result_view::SourceFormats {
    use std::io::{Cursor, Write};
    let ws = "http://schemas.mathsoft.com/worksheet50";
    let ml = "http://schemas.mathsoft.com/math50";
    let local = local
        .map(|a| format!("<ws:resultFormat><ws:matrix {a}/></ws:resultFormat>"))
        .unwrap_or_default();
    let source = format!(
        "<ws:worksheet xmlns:ws='{ws}' xmlns:ml='{ml}'><ws:regions><ws:region region-id='42' left='0' top='0'><ws:math><ml:eval><ml:id labels='VARIABLE'>M</ml:id><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>{local}</ws:math></ws:region></ws:regions></ws:worksheet>"
    );
    let offset = offset.map(|n| format!("offset='{n}'")).unwrap_or_default();
    let settings = format!(
        "<presentation xmlns='http://schemas.ptc.com/mathcad/settings/presentation10' xmlns:ws='{ws}'><mathRendering><ws:results><ws:matrix size='{size}' {offset} show-indices='false' expand-nested-arrays='false'/></ws:results></mathRendering></presentation>"
    );
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, text) in [
        ("mathcad/worksheet.xml", source),
        ("mathcad/settings/presentation.xml", settings),
    ] {
        zip.start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(text.as_bytes()).unwrap();
    }
    let bytes = zip.finish().unwrap().into_inner();
    let package = NativePackage::from_bytes(&bytes).unwrap();
    let mut doc = f.app.document.clone();
    doc.native_archive = Some(bytes);
    if !local.is_empty() {
        doc.regions
            .iter_mut()
            .find(|r| r.id == f.owner)
            .unwrap()
            .source_region = Some("42".into());
    }
    matrix_result_view::SourceFormats::from_document(&doc, Some(&package))
}
#[test]
fn first_show_change_clones_requested_not_actual_size_and_undo_restores_inheritance() {
    let mut f = Fixture::new(1.);
    f.app.matrix_view_sources = formats(&f, "30,30", Some("7,8"), None);
    let inert = Inert::new(&f.app);
    assert!(!f.app.matrix_indices_context().unwrap().show_indices);
    f.app.ribbon_command(
        &f.ctx,
        RibbonAction::SetMatrixIndices {
            owner: f.owner,
            show: true,
        },
    );
    let target = MatrixResultTarget::math(f.owner);
    let frozen = *f.app.document.matrix_result_views.get(&target).unwrap();
    assert_eq!(frozen.size, Some(MatrixViewSize { rows: 30, cols: 30 }));
    assert_eq!(frozen.offset, Some(MatrixViewOffset { row: 7, col: 8 }));
    assert_eq!(frozen.show_indices, Some(true));
    let actual = f.app.display_cache[&f.owner].matrix.value().unwrap();
    assert_eq!(
        (
            actual.rows,
            actual.cols,
            actual.row_offset,
            actual.col_offset
        ),
        (20, 20, 0, 0)
    );
    inert.check(&f.app);
    // A later positively parsed inherited setting must not change the local clone.
    f.app.matrix_view_sources = formats(&f, "4,5", Some("2,3"), None);
    let effective = f
        .app
        .matrix_view_sources
        .effective(f.owner, Some(frozen))
        .unwrap();
    assert_eq!(effective, frozen);
    f.app.undo();
    assert!(f.app.document.matrix_result_views.get(&target).is_none());
    let inherited = f.app.matrix_view_sources.effective(f.owner, None).unwrap();
    assert_eq!(inherited.size, Some(MatrixViewSize { rows: 4, cols: 5 }));
    assert_eq!(inherited.offset, Some(MatrixViewOffset { row: 2, col: 3 }));
    assert_eq!(inherited.show_indices, Some(false));
    inert.check(&f.app);
}
#[test]
fn known_native_local_result_format_becomes_explicit_command_clone() {
    let mut f = Fixture::new(1.);
    f.app.matrix_view_sources = formats(
        &f,
        "30,30",
        Some("7,8"),
        Some("size='13,17' offset='2,3' show-indices='false' expand-nested-arrays='false'"),
    );
    let inert = Inert::new(&f.app);
    assert!(f.app.set_matrix_indices(f.owner, true).unwrap());
    let view = f
        .app
        .document
        .matrix_result_views
        .get(&MatrixResultTarget::math(f.owner))
        .unwrap();
    assert_eq!(view.size, Some(MatrixViewSize { rows: 13, cols: 17 }));
    assert_eq!(view.offset, Some(MatrixViewOffset { row: 2, col: 3 }));
    assert_eq!(view.show_indices, Some(true));
    inert.check(&f.app);
}
#[test]
fn incomplete_inherited_offset_cannot_be_invented_by_first_local_clone() {
    let mut f = Fixture::new(1.);
    f.app.matrix_view_sources = formats(&f, "30,30", None, None);
    let before = serde_json::to_vec(&f.app.document).unwrap();
    let undo = f.app.undo.len();
    assert!(f.app.set_matrix_indices(f.owner, true).is_err());
    assert!(serde_json::to_vec(&f.app.document).unwrap() == before);
    assert_eq!(f.app.undo.len(), undo);
    assert!(f.app.last_executed_regions.is_empty());
}
#[test]
fn existing_partial_request_keeps_its_import_semantics_outside_first_local_command() {
    let mut f = Fixture::new(1.);
    let target = MatrixResultTarget::math(f.owner);
    f.app
        .document
        .matrix_result_views
        .set(
            target.clone(),
            MatrixResultViewport {
                size: Some(MatrixViewSize { rows: 10, cols: 11 }),
                ..Default::default()
            },
        )
        .unwrap();
    f.app.matrix_view_sources = formats(&f, "30,30", Some("7,8"), None);
    assert!(f.app.set_matrix_indices(f.owner, true).unwrap());
    let view = f.app.document.matrix_result_views.get(&target).unwrap();
    assert_eq!(view.size, Some(MatrixViewSize { rows: 10, cols: 11 }));
    assert_eq!(view.offset, None);
    assert_eq!(view.show_indices, Some(true));
}
