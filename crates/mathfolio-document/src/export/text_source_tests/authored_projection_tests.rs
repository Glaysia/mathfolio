//! Child of existing text_source_tests; uses actual retained/cache fixture.
use super::*;
use crate::rich_text_source::{
    AuthoredRichText, AuthoredStyleSnapshot, AuthoredTextSeed, AuthoredTextSeedSpec,
    ValidatedRichText,
};

fn new_authored(key: &str) -> AuthoredRichText {
    let style = AuthoredStyleSnapshot {
        key: key.into(),
        font_family: "Tahoma".into(),
        size_dip: "16".into(),
        weight: "Normal".into(),
        font_style: "Normal".into(),
        alignment: "Left".into(),
        foreground_rgba: [0, 0, 0, 255],
        background_rgba: [255, 255, 255, 0],
    };
    let empty = AuthoredRichText::empty(
        AuthoredTextSeed::new(AuthoredTextSeedSpec {
            format: style.clone(),
            catalog: vec![style],
            proofing_language: "ko-KR".into(),
            text_decorations: None,
        })
        .unwrap(),
    )
    .unwrap();
    empty
        .replace_in_run(0, 0, 0, "새😀")
        .unwrap()
        .unwrap()
        .source
}

fn append_authored(canvas: &mut CanvasDocument, key: &str) -> AuthoredRichText {
    let source = new_authored(key);
    let id = canvas.add_text(20., 400., source.plain_text());
    canvas
        .regions
        .iter_mut()
        .find(|r| r.id == id)
        .unwrap()
        .authored_text = Some(Box::new(source.wire().clone()));
    source
}

/// Replace the deliberately partial test-only catalog with a real complete
/// native catalog and then bind all wires to those actual package bytes.
/// Copies share the original nested part intentionally: COW must isolate them.
fn complete_fixture(text_count: usize) -> (CanvasDocument, NativePackage) {
    let (mut canvas, mut package) = fixture();
    set_xml(
        &mut package,
        CATALOG,
        include_str!("../../../../../tests/reference/rich-text/lcc-standard-presentation.xml")
            .into(),
    )
    .unwrap();
    let xml = package.worksheet_xml().unwrap();
    let parsed = parse(&xml).unwrap();
    let regions = parsed
        .root_element()
        .children()
        .find(|n| n.has_tag_name((WS, "regions")))
        .unwrap();
    let first = regions
        .children()
        .find(|n| n.has_tag_name((WS, "region")))
        .unwrap();
    let original = &xml[first.range()];
    let mut extra = String::new();
    // Reparse each independent wrapper so byte ranges are wrapper-relative.
    for offset in 1..text_count {
        let doc = parse(original).unwrap();
        let owner = doc.root_element();
        let id = 4 + offset as u64;
        extra.push_str(
            &apply(
                original.into(),
                attrs(
                    owner,
                    original,
                    &[
                        ("region-id", Some(id.to_string())),
                        ("top", Some((200 + offset * 30).to_string())),
                    ],
                )
                .unwrap(),
            )
            .unwrap(),
        );
        let mut region = canvas.regions[0].clone();
        region.id = canvas.next_id;
        canvas.next_id += 1;
        region.source_region = Some(id.to_string());
        region.top = (200 + offset * 30) as f64;
        region.rich_text = None;
        canvas.regions.push(region);
    }
    let replacement = insert_child(&xml[regions.range()], &extra).unwrap();
    let changed = apply(xml.clone(), vec![(regions.range(), replacement)]).unwrap();
    let part = package.worksheet_part_name().to_owned();
    set_xml(&mut package, &part, changed).unwrap();
    package = NativePackage::from_bytes(&package.to_bytes().unwrap()).unwrap();
    canvas.native_archive = Some(package.to_bytes().unwrap());
    for region in canvas
        .regions
        .iter_mut()
        .filter(|r| matches!(r.content, CanvasContent::Text(_)))
    {
        let authority =
            read_native_text_authority(&package, region.source_region.as_deref().unwrap()).unwrap();
        let original = ValidatedRichText::from_original(authority, PLAIN).unwrap();
        let formatted = original
            .set_selection_format(
                TextSelection {
                    paragraph: 0,
                    anchor: 1,
                    focus: 3,
                },
                &TextFormat {
                    weight: Some(Weight::Bold),
                    ..Default::default()
                },
            )
            .unwrap()
            .unwrap()
            .source;
        region.rich_text = Some(Box::new(formatted.wire().clone()));
    }
    (canvas, package)
}

fn assert_old_payloads_pending(before: &NativePackage, after: &NativePackage) {
    let new = after.caches().unwrap();
    for old in before.caches().unwrap() {
        let row = new.iter().find(|r| r.result_id == old.result_id).unwrap();
        assert_eq!(row.calculation_status.as_deref(), Some("Pending"));
        assert_eq!(row.result_mathml, old.result_mathml);
        assert_eq!(row.tag, old.tag);
    }
}

#[test]
fn authored_catalog_change_has_old_order_red_oracle_and_full_prepare_green() {
    let (mut canvas, package) = complete_fixture(1);
    let source = append_authored(&mut canvas, "New authored style");
    // Actual old order, not a mirrored digest predicate: receipt admission,
    // real authored allocation/catalog mutation, then actual deferred writer.
    let edits = text_source::prepare(&canvas, &package).unwrap();
    let mut old_order = authored_export::append_region(&package, 99, 20., 400., &source).unwrap();
    let before = old_order.to_bytes().unwrap();
    assert!(
        text_source::apply_deferred(&mut old_order, edits.deferred)
            .unwrap_err()
            .contains("context changed")
    );
    assert_eq!(old_order.to_bytes().unwrap(), before);
    let original = package.to_bytes().unwrap();
    assert!(!source_context_matches(&canvas, &package).unwrap());
    let prepared = prepare_mcdx(&canvas).unwrap();
    let saved = NativePackage::from_bytes(prepared.bytes()).unwrap();
    assert_old_payloads_pending(&package, &saved);
    assert!(
        read_native_text_authority(&saved, "1")
            .unwrap()
            .original_xaml()
            .contains("Bold")
    );
    assert_eq!(
        read_native_text_authority(&saved, "5")
            .unwrap()
            .original_xaml(),
        source.xaml()
    );
    assert_eq!(package.to_bytes().unwrap(), original);
}

#[test]
fn two_imported_format_patches_ordinary_content_and_authored_share_original_proof_then_pending() {
    let (mut canvas, package) = complete_fixture(3);
    let third = canvas
        .regions
        .iter_mut()
        .find(|r| r.source_region.as_deref() == Some("6"))
        .unwrap();
    let original =
        ValidatedRichText::from_original(read_native_text_authority(&package, "6").unwrap(), PLAIN)
            .unwrap();
    let edited = original
        .replace_in_run(0, 0, 1, "Z")
        .unwrap()
        .unwrap()
        .source;
    third.content = CanvasContent::Text(edited.plain_text().into());
    third.rich_text = Some(Box::new(edited.wire().clone()));
    let source = append_authored(&mut canvas, "Fresh new style");
    let bytes = serde_json::to_vec(&canvas).unwrap();
    let cold: CanvasDocument = serde_json::from_slice(&bytes).unwrap();
    let saved = NativePackage::from_bytes(prepare_mcdx(&cold).unwrap().bytes()).unwrap();
    for owner in ["1", "5"] {
        let actual = read_native_text_authority(&saved, owner).unwrap();
        assert_eq!(actual.original_plain_text(), PLAIN);
        assert!(actual.original_xaml().contains("Bold"));
    }
    assert_eq!(
        read_native_text_authority(&saved, "6")
            .unwrap()
            .original_xaml(),
        edited.xaml()
    );
    assert_eq!(
        read_native_text_authority(&saved, "7")
            .unwrap()
            .original_xaml(),
        source.xaml()
    );
    assert_old_payloads_pending(&package, &saved);
    assert_eq!(serde_json::to_vec(&cold).unwrap(), bytes);
}

#[test]
fn catalog_conflict_still_fails_without_publishing_earlier_format_patches() {
    let (mut canvas, package) = complete_fixture(1);
    append_authored(&mut canvas, "Normal"); // Our 16 DIP conflicts with actual 14.666... DIP.
    let original = package.to_bytes().unwrap();
    let own = serde_json::to_vec(&canvas).unwrap();
    assert!(source_context_matches(&canvas, &package).is_err());
    assert!(prepare_mcdx(&canvas).is_err());
    // export_into is private and stages only in public callers. Do not assert
    // it is atomic; the actual public operations above cannot alter originals.
    assert_eq!(package.to_bytes().unwrap(), original);
    assert_eq!(serde_json::to_vec(&canvas).unwrap(), own);
}
