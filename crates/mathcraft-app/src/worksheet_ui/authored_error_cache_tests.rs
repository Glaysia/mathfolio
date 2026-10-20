use super::*;
use mathcraft_document::rich_text_source::{
    AuthoredStyleSnapshot, AuthoredTextSeed, AuthoredTextSeedSpec,
};

fn source(size: &str, text: &str) -> AuthoredRichText {
    let style = AuthoredStyleSnapshot {
        key: "Own".into(),
        font_family: "Tahoma".into(),
        size_dip: size.into(),
        weight: "Normal".into(),
        font_style: "Normal".into(),
        alignment: "Left".into(),
        foreground_rgba: [0, 0, 0, 255],
        background_rgba: [255, 255, 255, 0],
    };
    let empty = AuthoredRichText::empty(
        AuthoredTextSeed::new(AuthoredTextSeedSpec {
            format: style.clone(),
            proofing_language: "ko-KR".into(),
            catalog: vec![style],
            text_decorations: None,
        })
        .unwrap(),
    )
    .unwrap();
    if text.is_empty() {
        empty
    } else {
        empty.replace_in_run(0, 0, 0, text).unwrap().unwrap().source
    }
}
fn app(source: &AuthoredRichText) -> (WorksheetApp, u64) {
    let mut app = WorksheetApp::default();
    let id = app.document.add_text(0., 0., source.plain_text());
    app.document.regions[0].authored_text = Some(Box::new(source.wire().clone()));
    app.document.validate().unwrap();
    (app, id)
}
#[test]
fn source_valid_tiny_font_ui_error_is_retained_without_idle_readmission() {
    let source = source("0.1", "body");
    let (mut app, id) = app(&source);
    app.prepare_authored_text(id);
    assert!(app.rich_text.authored_editors[&id].is_err());
    assert!(app.rich_text.authored_failed.is_some());
    let count = app.rich_text.authored_admission_attempts;
    assert_eq!(count, 1);
    for _ in 0..10 {
        app.prepare_authored_text(id);
    }
    assert_eq!(app.rich_text.authored_admission_attempts, count);
    assert_eq!(
        app.document.regions[0].authored_text.as_deref(),
        Some(source.wire())
    );
}
#[test]
fn failed_binding_retries_changed_version_and_releases_on_native_owner_or_reset() {
    let first = source("0.1", "body");
    let (mut app, id) = app(&first);
    app.prepare_authored_text(id);
    let good = source("18.5", "body");
    app.document.regions[0].authored_text = Some(Box::new(good.wire().clone()));
    app.prepare_authored_text(id);
    assert!(app.rich_text.authored_editors[&id].is_ok());
    assert!(app.rich_text.authored_failed.is_none());
    assert_eq!(app.rich_text.authored_admission_attempts, 2);
    app.document.regions[0].authored_text = Some(Box::new(first.wire().clone()));
    app.prepare_authored_text(id);
    assert_eq!(app.rich_text.authored_admission_attempts, 3);
    app.rich_text.authored_editors.remove(&id); // Same removal performed by region Undo.
    app.prepare_authored_text(id);
    assert_eq!(app.rich_text.authored_admission_attempts, 4);
    let other = app.document.add_text(0., 40., "plain");
    app.prepare_authored_text(other);
    assert!(app.rich_text.authored_failed.is_none());
    app.rich_text = Default::default();
    assert!(app.rich_text.authored_failed.is_none());
}
#[test]
fn invalid_plain_and_cold_forgery_never_gain_failure_receipt_authority() {
    let value = source("0.1", "body");
    let (mut app, id) = app(&value);
    app.prepare_authored_text(id);
    app.document.regions[0].content = CanvasContent::Text("forged plain".into());
    app.prepare_authored_text(id);
    assert!(app.rich_text.authored_failed.is_none());
    let attempts = app.rich_text.authored_admission_attempts;
    app.prepare_authored_text(id);
    assert_eq!(app.rich_text.authored_admission_attempts, attempts + 1);
    let mut wire = serde_json::to_value(value.wire()).unwrap();
    wire["seed_sha256"][0] = ((wire["seed_sha256"][0].as_u64().unwrap() + 1) % 256).into();
    app.document.regions[0].content = CanvasContent::Text("body".into());
    app.document.regions[0].authored_text = Some(Box::new(serde_json::from_value(wire).unwrap()));
    assert!(app.document.validate().is_err());
    app.prepare_authored_text(id);
    assert!(app.rich_text.authored_failed.is_none());
}
#[test]
fn equal_wire_for_new_owner_does_not_reuse_old_ui_error_binding() {
    let source = source("0.1", "body");
    let (mut app, id) = app(&source);
    app.prepare_authored_text(id);
    let other = app.document.add_text(0., 40., "body");
    app.document.regions[1].authored_text = Some(Box::new(source.wire().clone()));
    app.prepare_authored_text(other);
    assert_eq!(app.rich_text.authored_admission_attempts, 2);
    assert!(!app.rich_text.authored_editors.contains_key(&id));
    assert!(app.rich_text.authored_editors[&other].is_err());
}
