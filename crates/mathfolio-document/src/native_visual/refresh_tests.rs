//! Copy to native_visual/refresh_tests.rs and register in native_visual.rs.
use super::*;
use crate::{
    canvas::CanvasDocument,
    rich_text_source::{
        NativeTextIdentity, OriginalTextAuthority, TextFormat, TextSelection, ValidatedRichText,
        Weight,
    },
};
use std::io::Write;
use zip::{ZipWriter, write::SimpleFileOptions};

fn fixture() -> (NativeVisuals, CanvasRegion, ValidatedRichText) {
    let xml = r#"<Section xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation" FontSize="16" FontWeight="Normal"><Paragraph><Run style="Normal">한😀AB</Run></Paragraph></Section>"#;
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    zip.start_file("Xaml/Document.xaml", SimpleFileOptions::default())
        .unwrap();
    zip.write_all(xml.as_bytes()).unwrap();
    let bytes = zip.finish().unwrap().into_inner();
    let identity = NativeTextIdentity {
        owner_region_id: "17".into(),
        item_idref: "FlowDocument0".into(),
        package_part: "mathcad/xaml/FlowDocument0.XamlPackage".into(),
        catalog_part: "mathcad/settings/presentation.xml".into(),
        owner_xml_sha256: [4; 32],
    };
    let authority = OriginalTextAuthority::from_package(
        identity,
        bytes,
        include_bytes!("../../../../tests/reference/rich-text/lcc-standard-presentation.xml")
            .to_vec(),
    )
    .unwrap();
    let source = ValidatedRichText::from_original(authority, "한😀AB").unwrap();
    let mut canvas = CanvasDocument::default();
    canvas.add_text(0., 0., "한😀AB");
    let mut region = canvas.regions.remove(0);
    region.source_region = Some("17".into());
    let base = TextStyle {
        font_family: "Tahoma".into(),
        background: Some([1, 2, 3, 255]),
        ..Default::default()
    };
    let visual = RegionVisual {
        size: [220., 38.],
        background: base.background,
        document: Some(read_flow_xml(xml, base).unwrap()),
    };
    let visuals = NativeVisuals {
        regions: HashMap::from([("17".into(), Ok(visual))]),
    };
    (visuals, region, source)
}
fn bold(source: &ValidatedRichText) -> ValidatedRichText {
    source
        .set_selection_format(
            TextSelection {
                paragraph: 0,
                anchor: 0,
                focus: 2,
            },
            &TextFormat {
                weight: Some(Weight::Bold),
                ..Default::default()
            },
        )
        .unwrap()
        .unwrap()
        .source
}

#[test]
fn same_plain_new_style_never_returns_stale_import_and_undo_restores_original() {
    let (mut visuals, mut region, original) = fixture();
    let old = visuals.region(&region).unwrap().as_ref().unwrap().clone();
    let _ = original.wire().visual_source_fingerprint();
    let edited = bold(&original);
    assert_ne!(
        original.wire().visual_source_fingerprint(),
        edited.wire().visual_source_fingerprint()
    );
    region.rich_text = Some(Box::new(edited.wire().clone()));
    assert!(visuals.region(&region).is_none());
    visuals.update_rich_text(&region, &edited).unwrap();
    let current = visuals.region(&region).unwrap().as_ref().unwrap();
    assert_eq!(current.size, old.size);
    assert_eq!(current.background, old.background);
    let Block::Paragraph(p) = &current.document.as_ref().unwrap().blocks[0] else {
        unreachable!()
    };
    assert!(p.runs[0].style.bold);
    assert_eq!(
        current.document.as_ref().unwrap().imported_plain_text,
        "한😀AB"
    );
    region.rich_text = None;
    assert_eq!(visuals.region(&region).unwrap().as_ref().unwrap(), &old);
}

#[test]
fn unchanged_serialization_equality_cache_reset_and_overlay_growth() {
    let (mut visuals, mut region, source) = fixture();
    let before = serde_json::to_vec(source.wire()).unwrap();
    let decoded: crate::rich_text_source::RichTextWire = serde_json::from_slice(&before).unwrap();
    let fingerprint = source.wire().visual_source_fingerprint();
    assert_eq!(&decoded, source.wire());
    assert_eq!(before, serde_json::to_vec(source.wire()).unwrap());
    assert_eq!(fingerprint, decoded.visual_source_fingerprint());
    for current in [source.clone(), bold(&source), source.clone()] {
        region.rich_text = Some(Box::new(current.wire().clone()));
        visuals.update_rich_text(&region, &current).unwrap();
        assert_eq!(
            visuals
                .regions
                .keys()
                .filter(|key| key.starts_with(&rich_overlay_prefix(region.id)))
                .count(),
            1
        );
    }
    visuals.remove_local_region(region.id);
    assert!(visuals.regions.contains_key("17"));
    assert!(
        !visuals
            .regions
            .keys()
            .any(|key| key.starts_with(&rich_overlay_prefix(region.id)))
    );
}

#[test]
fn mismatched_authority_wire_and_owner_do_not_reuse_same_plain_overlay() {
    let (mut visuals, mut region, source) = fixture();
    let edited = bold(&source);
    region.rich_text = Some(Box::new(edited.wire().clone()));
    visuals.update_rich_text(&region, &edited).unwrap();
    region.rich_text = Some(Box::new(source.wire().clone()));
    assert!(visuals.region(&region).is_none());
    assert!(visuals.update_rich_text(&region, &edited).is_err());
    region.source_region = Some("different-owner".into());
    assert!(visuals.update_rich_text(&region, &source).is_err());
    assert!(visuals.region(&region).is_none());
}

#[test]
fn local_id_alias_uses_original_authority_owner_not_missing_source_region() {
    let (mut visuals, mut region, source) = fixture();
    region.source_region = None;
    region.rich_text = Some(Box::new(source.wire().clone()));
    visuals.update_rich_text(&region, &source).unwrap();
    region.rich_text = None;
    assert_eq!(visuals.region(&region), visuals.regions.get("17"));
}
