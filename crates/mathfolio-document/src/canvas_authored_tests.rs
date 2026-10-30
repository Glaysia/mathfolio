use super::*;
use crate::rich_text_source::{AuthoredStyleSnapshot, AuthoredTextSeed, AuthoredTextSeedSpec};

fn authored(text: &str) -> AuthoredRichText {
    let style = AuthoredStyleSnapshot {
        key: "Normal".into(),
        font_family: "Noto Sans CJK KR".into(),
        size_dip: "16".into(),
        weight: "Normal".into(),
        font_style: "Normal".into(),
        alignment: "Left".into(),
        foreground_rgba: [0, 0, 0, 255],
        background_rgba: [255, 255, 255, 255],
    };
    let seed = AuthoredTextSeed::new(AuthoredTextSeedSpec {
        format: style.clone(),
        proofing_language: "ko-KR".into(),
        catalog: vec![style],
        text_decorations: None,
    })
    .unwrap();
    let empty = AuthoredRichText::empty(seed).unwrap();
    if text.is_empty() {
        empty
    } else {
        empty.replace_in_run(0, 0, 0, text).unwrap().unwrap().source
    }
}
fn document(source: &AuthoredRichText) -> CanvasDocument {
    let mut doc = CanvasDocument::default();
    let id = doc.add_text(1., 2., source.plain_text());
    doc.regions
        .iter_mut()
        .find(|r| r.id == id)
        .unwrap()
        .authored_text = Some(Box::new(source.wire().clone()));
    doc
}

#[test]
fn old_none_wire_and_signature_are_exact_and_math_framing_stays_serde() {
    let mut doc = CanvasDocument::default();
    doc.add_text(0., 0., "old plain");
    doc.add_math(0., 40.);
    let wire = serde_json::to_vec(&doc).unwrap();
    assert!(!String::from_utf8_lossy(&wire).contains("authored_text"));
    let parsed: CanvasDocument = serde_json::from_slice(&wire).unwrap();
    assert_eq!(serde_json::to_vec(&parsed).unwrap(), wire);
    let mut old: serde_json::Value = serde_json::from_slice(&wire).unwrap();
    old["regions"][0]
        .as_object_mut()
        .unwrap()
        .insert("authored_text".into(), serde_json::Value::Null);
    let explicit_none: CanvasDocument = serde_json::from_value(old).unwrap();
    assert_eq!(serde_json::to_vec(&explicit_none).unwrap(), wire);
    for region in &doc.regions {
        let mut encoded = Vec::new();
        crate::canvas::write_region_signature_json(region, &mut encoded).unwrap();
        assert_eq!(encoded, serde_json::to_vec(region).unwrap());
    }
    let signature = doc.calculation_signature();
    doc.regions[0].authored_text = Some(Box::new(authored("old plain").wire().clone()));
    assert_eq!(
        doc.calculation_signature(),
        signature,
        "inert Text does not change computation identity"
    );
}

#[test]
fn own_roundtrip_cold_admission_warms_only_derived_facts_and_preserves_bytes() {
    let source = authored("한😀<&");
    let doc = document(&source);
    doc.validate().unwrap();
    let before = serde_json::to_vec(&doc).unwrap();
    let cold: CanvasDocument = serde_json::from_slice(&before).unwrap();
    let wire = cold.regions[0].authored_text.as_deref().unwrap();
    assert_eq!(wire.cached_serialized_wire_bytes(), None);
    assert_eq!(wire.cached_visual_fingerprint(), None);
    assert!(
        cold.source_text_bytes().is_err(),
        "raw cold wire must be admitted before pure budget access"
    );
    cold.validate().unwrap();
    assert_eq!(cold, doc, "derived cache is excluded from equality");
    assert_eq!(serde_json::to_vec(&cold).unwrap(), before);
    assert_eq!(
        wire.cached_serialized_wire_bytes(),
        Some(serde_json::to_vec(wire).unwrap().len())
    );
    assert_eq!(
        wire.cached_visual_fingerprint(),
        source.wire().cached_visual_fingerprint()
    );
    assert_eq!(
        cold.source_text_bytes().unwrap(),
        doc.source_text_bytes().unwrap()
    );
}

#[test]
fn mutually_exclusive_wrong_type_and_source_owner_fail_without_changing_source() {
    let source = authored("body");
    let doc = document(&source);
    let original = serde_json::to_vec(&doc).unwrap();
    let mut bad = doc.clone();
    bad.regions[0].source_region = Some("1".into());
    assert!(bad.validate().is_err());
    let mut bad = doc.clone();
    bad.regions[0].content = CanvasContent::Math(PortableTree {
        kind: NodeKind::Number("7".into()),
        children: vec![],
    });
    assert!(bad.validate().is_err());
    let mut bad = doc.clone();
    bad.regions[0].content = CanvasContent::Opaque {
        kind: "Area".into(),
        xml: "<Area/>".into(),
    };
    assert!(bad.validate().is_err());
    // Deliberately inert/unadmitted imported metadata is enough to prove that
    // carrier ambiguity is rejected before attempting either authority path.
    let native = serde_json::json!({"version":1,"current_xaml":"unused","origin":{"identity":{
        "owner_region_id":"1","item_idref":"r1","package_part":"mathcad/xaml/source.XamlPackage",
        "catalog_part":"mathcad/settings/presentation.xml","owner_xml_sha256":vec![0u8;32]},
        "package_sha256":vec![0u8;32],"original_xaml_sha256":vec![0u8;32],"catalog_sha256":vec![0u8;32]}});
    let mut bad = doc.clone();
    bad.regions[0].rich_text = Some(Box::new(serde_json::from_value(native).unwrap()));
    assert!(bad.validate().unwrap_err().contains("both"));
    assert_eq!(serde_json::to_vec(&doc).unwrap(), original);
}

#[test]
fn forged_plain_source_seed_and_unknown_map_reject_on_document_admission() {
    let doc = document(&authored("body"));
    for change in [0, 1, 2, 3] {
        let mut value = serde_json::to_value(&doc).unwrap();
        let wire = &mut value["regions"][0]["authored_text"];
        match change {
            0 => {
                wire["current_xaml"] = wire["current_xaml"]
                    .as_str()
                    .unwrap()
                    .replace("<Paragraph>", "<Paragraph FontWeight=\"Bold\">")
                    .into()
            }
            1 => wire["seed"]["format"]["font_family"] = "forged".into(),
            2 => {
                wire["seed_sha256"][0] =
                    ((wire["seed_sha256"][0].as_u64().unwrap() + 1) % 256).into()
            }
            _ => {
                value["regions"][0]["content"]["Text"] = "different".into();
            }
        }
        let bad: CanvasDocument = serde_json::from_value(value).unwrap();
        assert!(bad.validate().is_err());
    }
    let mut value = serde_json::to_value(&doc).unwrap();
    value["regions"][0]["authored_text"]["paragraph_origins"] = serde_json::json!([false]);
    assert!(serde_json::from_value::<CanvasDocument>(value).is_err());
}

#[test]
fn aggregate_charges_each_owner_and_plain_mirror_even_for_shared_wire() {
    let source = authored(&"x".repeat(400_000));
    let mut doc = CanvasDocument::default();
    for _ in 0..10 {
        let id = doc.add_text(0., 0., source.plain_text());
        doc.regions
            .iter_mut()
            .find(|r| r.id == id)
            .unwrap()
            .authored_text = Some(Box::new(source.wire().clone()));
    }
    doc.validate().unwrap();
    let exact = doc.title.len()
        + 10 * (source.plain_text().len() + serde_json::to_vec(source.wire()).unwrap().len());
    assert_eq!(doc.source_text_bytes().unwrap(), exact);
    let candidate = doc.regions[0]
        .authored_text_copy(doc.next_id, 0., 0.)
        .unwrap();
    assert!(doc.validate_text_append_budget(&candidate).is_err());
    doc.regions.push(candidate);
    doc.next_id += 1;
    assert!(doc.validate().is_err());
}

#[test]
fn borrowed_append_replace_copy_and_headroom_do_not_clone_archive_or_lose_source() {
    let source = authored("body");
    let mut doc = document(&source);
    let retained = crate::export::prepare_mcdx(&CanvasDocument::default()).unwrap();
    doc.native_archive = Some(retained.bytes().to_vec());
    let id = doc.regions[0].id;
    let old = serde_json::to_vec(&doc).unwrap();
    let replacement = source
        .replace_in_run(0, 0, 4, "한😀")
        .unwrap()
        .unwrap()
        .source;
    doc.validate_authored_text_edit_budget(id, &replacement)
        .unwrap();
    assert!(
        doc.validate_text_edit_budget(id, "plain", None).is_err(),
        "old plain/native editor must not flatten authored source"
    );
    let mut candidate = doc.regions[0].clone();
    candidate.content = CanvasContent::Text(replacement.plain_text().into());
    candidate.authored_text = Some(Box::new(replacement.wire().clone()));
    doc.validate_text_replace_budget(&candidate).unwrap();
    assert_eq!(serde_json::to_vec(&doc).unwrap(), old);
    assert_eq!(doc.native_archive.as_deref(), Some(retained.bytes()));
    let copy = doc.regions[0]
        .authored_text_copy(doc.next_id, 3., 4.)
        .unwrap();
    doc.validate_text_append_budget(&copy).unwrap();
    assert_eq!(copy.authored_text, doc.regions[0].authored_text);
    doc.regions.push(copy);
    doc.next_id += 1;
    doc.validate().unwrap();
    assert_eq!(
        doc.regions[0]
            .authored_text
            .as_deref()
            .unwrap()
            .cached_visual_fingerprint(),
        doc.regions[1]
            .authored_text
            .as_deref()
            .unwrap()
            .cached_visual_fingerprint()
    );
    doc.regions[0] = candidate;
    assert_ne!(doc.regions[0].authored_text, doc.regions[1].authored_text);
    doc.validate().unwrap();
}

#[test]
fn immutable_version_identity_is_constant_work_and_never_admits_equal_cold_data() {
    let source = authored("body");
    let clone = source.wire().clone();
    assert!(source.wire().same_source_version(&clone));
    let cold: crate::rich_text_source::AuthoredTextWire =
        serde_json::from_slice(&serde_json::to_vec(source.wire()).unwrap()).unwrap();
    assert!(!source.wire().same_source_version(&cold));
    assert_eq!(&cold, source.wire());
    let changed = source.replace_in_run(0, 0, 1, "B").unwrap().unwrap().source;
    assert!(!source.wire().same_source_version(changed.wire()));
    assert_ne!(
        source.wire().cached_visual_fingerprint(),
        changed.wire().cached_visual_fingerprint()
    );
}

#[test]
fn native_facades_prove_authored_and_native_cold_full_prepare_without_fake_identity() {
    let source = authored("한😀body");
    let doc = document(&source);
    let own_json = serde_json::to_vec(&doc).unwrap();
    let own: CanvasDocument = serde_json::from_slice(&own_json).unwrap();
    let empty = crate::export::prepare_mcdx(&CanvasDocument::default()).unwrap();
    let package = crate::native::NativePackage::from_bytes(empty.bytes()).unwrap();
    crate::export::validate_rich_text_carriers(&own, &package).unwrap();
    assert!(!crate::export::source_context_matches(&own, &package).unwrap());
    let prepared = crate::export::prepare_mcdx(&own).unwrap();
    let native = crate::native::NativePackage::from_bytes(prepared.bytes()).unwrap();
    let imported = crate::rich_text_source::ValidatedRichText::from_original(
        crate::export::read_native_text_authority(&native, "1").unwrap(),
        source.plain_text(),
    )
    .unwrap();
    assert_eq!(imported.xaml(), source.xaml());
    let mut cold = own.clone();
    cold.native_archive = Some(prepared.bytes().to_vec());
    cold.regions[0].authored_text = None;
    cold.regions[0].source_region = Some("1".into());
    cold.regions[0].rich_text = Some(Box::new(imported.wire().clone()));
    cold.validate().unwrap();
    crate::export::validate_rich_text_carriers(&cold, &native).unwrap();
    assert!(crate::export::source_context_matches(&cold, &native).unwrap());
    let saved = crate::export::prepare_mcdx(&cold).unwrap();
    let again = crate::native::NativePackage::from_bytes(saved.bytes()).unwrap();
    assert_eq!(
        crate::export::read_native_text_authority(&again, "1")
            .unwrap()
            .original_xaml(),
        imported.xaml()
    );
    assert_eq!(serde_json::to_vec(&own).unwrap(), own_json);
    for wrong_type in [true, false] {
        let mut bad = own.clone();
        if wrong_type {
            bad.regions[0].content = CanvasContent::Math(PortableTree {
                kind: NodeKind::Number("7".into()),
                children: vec![],
            });
        } else {
            bad.regions[0].rich_text = Some(Box::new(imported.wire().clone()));
        }
        assert!(crate::export::prepare_mcdx(&bad).is_err());
        assert!(crate::export::source_context_matches(&bad, &package).is_err());
        assert!(crate::export::validate_rich_text_carriers(&bad, &package).is_err());
        assert!(
            crate::export::validate_rich_text_append(
                &CanvasDocument::default(),
                &package,
                &bad.regions[0]
            )
            .is_err()
        );
        assert!(
            crate::export::validate_rich_text_replacement(&own, &package, &bad.regions[0]).is_err()
        );
    }
}

#[test]
fn own_file_roundtrip_preserves_carrier_and_copy_independence() {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let path = std::env::temp_dir().join(format!(
        "mathfolio-authored-own-{}-{}.mathfolio",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let source = authored("한😀<&");
    let mut doc = document(&source);
    let copy = doc.regions[0]
        .authored_text_copy(doc.next_id, 40., 50.)
        .unwrap();
    doc.regions.push(copy);
    doc.next_id += 1;
    doc.save(&path).unwrap();
    let reopened = CanvasDocument::load(&path).unwrap();
    std::fs::remove_file(&path).unwrap();
    assert_eq!(reopened, doc);
    for region in &reopened.regions {
        let admitted = region.validated_authored_text().unwrap().unwrap();
        assert_eq!(admitted.xaml(), source.xaml());
        assert_eq!(admitted.plain_text(), source.plain_text());
        assert_eq!(
            serde_json::to_vec(admitted.wire()).unwrap(),
            serde_json::to_vec(source.wire()).unwrap()
        );
    }
}
