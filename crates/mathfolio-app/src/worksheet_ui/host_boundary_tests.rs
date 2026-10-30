//! Native regression of the exact bytes path used by the web host.
use super::*;

#[test]
fn bytes_import_keeps_native_notes_without_opening_the_label_as_a_file() {
    let mut document = CanvasDocument::default();
    document.add_text(20., 20., "한국어 메모와 원본 색상");
    let math = document.add_math(20., 80.);
    let tree = PortableTree {
        kind: NodeKind::Binary(BinaryOp::Evaluate),
        children: vec![
            PortableTree {
                kind: NodeKind::Number("42".into()),
                children: vec![],
            },
            PortableTree {
                kind: NodeKind::Placeholder,
                children: vec![],
            },
        ],
    };
    document
        .regions
        .iter_mut()
        .find(|r| r.id == math)
        .unwrap()
        .content = CanvasContent::Math(tree);
    let exported = mathfolio_document::export::prepare_mcdx(&document).unwrap();
    let (imported, package) = import_native_bytes(
        Path::new("never-created-label.mcdx"),
        exported.bytes().to_vec(),
    )
    .unwrap();
    assert_eq!(imported.regions.len(), 2);
    assert!(
        imported.regions.iter().any(
            |r| matches!(&r.content, CanvasContent::Text(text) if text.contains("한국어 메모"))
        )
    );
    let source = imported.clone();
    let mut app = WorksheetApp::default();
    app.install_document(
        imported,
        Some(package),
        None,
        Path::new("never-created-label.mcdx"),
        Default::default(),
    );
    assert!(!app.dirty);
    assert!(
        app.results.is_empty(),
        "opening byte input must never run formulas"
    );
    assert!(app.undo.is_empty());
    assert_eq!(app.document.regions, source.regions);
    app.calculate_command(CalculationCommand::Worksheet);
    let row = app.results.get(&math).unwrap();
    assert!(matches!(&row.value, Some(Value::Scalar { quantity }) if quantity.value.re == 42.));
    app.prepare_native_export().unwrap();
    let own = app.document.to_bytes().unwrap();
    let restored = CanvasDocument::from_bytes(&own).unwrap();
    assert_eq!(restored, app.document);
    assert!(restored.saved_calculation_cache().is_some());
}

#[test]
fn bytes_validation_rejects_wrong_version_and_invalid_math_before_installation() {
    let mut app = WorksheetApp::default();
    app.document.add_text(20., 20., "existing document");
    let before = app.document.clone();
    let invalid = CanvasDocument {
        version: 999,
        ..Default::default()
    };
    let bytes = serde_json::to_vec(&invalid).unwrap();
    assert!(CanvasDocument::from_bytes(&bytes).is_err());
    assert!(CanvasDocument::from_bytes(b"not JSON").is_err());
    assert!(import_native_bytes(Path::new("invalid.mcdx"), vec![1, 2, 3]).is_err());
    assert_eq!(app.document, before);
}
