//! Child of existing export/text_source_tests; actual Canvas/native pipeline.
use super::*;
use crate::rich_text_source::{
    NativeFullSelectionTypingContext, TypingFlowDirection, ValidatedRichText,
};

fn full(selection_len: usize) -> TextSelection {
    TextSelection {
        paragraph: 0,
        anchor: 0,
        focus: selection_len,
    }
}
fn profile() -> NativeFullSelectionTypingContext<'static> {
    NativeFullSelectionTypingContext {
        input_language: "en-US",
        input_flow_direction: TypingFlowDirection::LeftToRight,
        has_retained_springload: false,
    }
}
fn native_fixture() -> (CanvasDocument, NativePackage, ValidatedRichText) {
    let (mut canvas, mut package) = fixture();
    let owner = canvas.regions[0].source_region.as_deref().unwrap();
    let target = canvas.regions[0]
        .rich_text
        .as_deref()
        .unwrap()
        .origin()
        .identity()
        .package_part
        .clone();
    let source = format!(
        "<Section xmlns='{X}' xml:space='preserve' xml:lang='en-US' FlowDirection='LeftToRight' FontFamily='Tahoma' FontWeight='Normal' FontStyle='Normal' FontSize='16' Foreground='#FF123456'><Paragraph><Run style='Normal' FontStyle='Italic'></Run><Run style='Normal' FontWeight='Bold' Background='#FF111111'>A&#x1F600;</Run><Run style='Normal' Background='#FF222222'>BC</Run></Paragraph></Section>"
    );
    let nested = zip_bytes(&[
        ("Xaml/Document.xaml".into(), source.into_bytes()),
        ("Resources/exact.bin".into(), vec![0, 255, 3, 9]),
    ])
    .unwrap();
    package.set_part(&target, nested).unwrap();
    package = NativePackage::from_bytes(&package.to_bytes().unwrap()).unwrap();
    let authority = text_source::read_native_authority(&package, owner, CATALOG).unwrap();
    let original = ValidatedRichText::from_original(authority, PLAIN).unwrap();
    canvas.native_archive = Some(package.to_bytes().unwrap());
    canvas.regions[0].rich_text = Some(Box::new(original.wire().clone()));
    (canvas, package, original)
}

#[test]
fn native_same_plain_unknown_underline_body_witness_exports_and_cold_reopens_pending_exact_source()
{
    let (mut canvas, package, original) = native_fixture();
    assert!(
        original
            .selection_format(TextSelection {
                paragraph: 0,
                anchor: 0,
                focus: 1
            })
            .unwrap()
            .text_decorations
            .is_none()
    );
    let patch = original
        .replace_select_all_text(full(4), PLAIN, profile())
        .unwrap()
        .unwrap();
    let expected = original
        .xaml()
        .replace("A&#x1F600;", "A😀BC")
        .replace(">BC</Run>", "></Run>");
    assert_eq!(patch.source.xaml(), expected);
    assert_eq!(patch.source.plain_text(), PLAIN);
    assert!(patch.source.has_original_raw_body_only_delta());
    assert!(patch.source.prove_format_only().is_err());
    canvas.regions[0].rich_text = Some(Box::new(patch.source.wire().clone()));
    // Actual own DTO cold admission precedes public full native preparation.
    let own = serde_json::to_vec(&canvas).unwrap();
    let cold: CanvasDocument = serde_json::from_slice(&own).unwrap();
    cold.validate().unwrap();
    assert!(!source_context_matches(&cold, &package).unwrap());
    let old_results = package.caches().unwrap();
    let old_archive = package.to_bytes().unwrap();
    let output = prepare_mcdx(&cold).unwrap();
    assert_eq!(package.to_bytes().unwrap(), old_archive);
    let reopened = NativePackage::from_bytes(output.bytes()).unwrap();
    let owner = canvas.regions[0].source_region.as_deref().unwrap();
    let authority = text_source::read_native_authority(&reopened, owner, CATALOG).unwrap();
    let source = ValidatedRichText::from_original(authority.clone(), PLAIN).unwrap();
    assert_eq!(source.xaml(), expected);
    assert!(!source.xaml().contains("TextDecorations"));
    assert_eq!(
        reopened.original_part(CATALOG),
        package.original_part(CATALOG)
    );
    let mut nested = ZipArchive::new(Cursor::new(authority.original_package())).unwrap();
    let mut kept = vec![];
    nested
        .by_name("Resources/exact.bin")
        .unwrap()
        .read_to_end(&mut kept)
        .unwrap();
    assert_eq!(kept, [0, 255, 3, 9]);
    let actual = reopened.caches().unwrap();
    for old in old_results {
        let result = actual
            .iter()
            .find(|r| r.result_id == old.result_id)
            .unwrap();
        assert_eq!(result.calculation_status.as_deref(), Some("Pending"));
        assert_eq!(result.result_mathml, old.result_mathml);
        assert_eq!(result.tag, old.tag);
    }
}

#[test]
fn native_same_plain_explicit_property_removal_has_no_witness_and_still_rejects_export() {
    let (mut canvas, package, original) = native_fixture();
    let mut wire_json = serde_json::to_value(original.wire()).unwrap();
    wire_json["current_xaml"] = original.xaml().replace(" FontWeight='Bold'", "").into();
    let wire = serde_json::from_value(wire_json).unwrap();
    let authority = text_source::read_native_authority(
        &package,
        canvas.regions[0].source_region.as_deref().unwrap(),
        CATALOG,
    )
    .unwrap();
    let removed = ValidatedRichText::from_wire(wire, authority, PLAIN).unwrap();
    assert!(!removed.has_original_raw_body_only_delta());
    assert!(removed.prove_format_only().is_err());
    canvas.regions[0].rich_text = Some(Box::new(removed.wire().clone()));
    let before = package.to_bytes().unwrap();
    assert!(prepare_mcdx(&canvas).is_err());
    assert_eq!(package.to_bytes().unwrap(), before);
    // A legitimate previous formatting delta keeps its existing format proof.
    let formatted = original
        .set_selection_format(
            TextSelection {
                paragraph: 0,
                anchor: 0,
                focus: 1,
            },
            &TextFormat {
                weight: Some(Weight::Normal),
                ..Default::default()
            },
        )
        .unwrap()
        .unwrap()
        .source;
    assert!(!formatted.has_original_raw_body_only_delta());
    assert!(formatted.prove_format_only().is_ok());
    canvas.regions[0].rich_text = Some(Box::new(formatted.wire().clone()));
    assert!(prepare_mcdx(&canvas).is_ok());
}
