use super::*;
use crate::rich_text_source::{
    AuthoredStyleSnapshot, AuthoredTextSeed, AuthoredTextSeedSpec, TextFormat, TextSelection,
    ValidatedRichText, Weight,
};

fn authored() -> AuthoredRichText {
    let style = AuthoredStyleSnapshot {
        key: "Heading 1".into(),
        font_family: "Tahoma".into(),
        size_dip: "16".into(),
        weight: "Normal".into(),
        font_style: "Italic".into(),
        alignment: "Left".into(),
        foreground_rgba: [12, 34, 56, 255],
        background_rgba: [240, 230, 220, 128],
    };
    let mut unused = style.clone();
    unused.key = "Unused snapshot only".into();
    AuthoredRichText::empty(
        AuthoredTextSeed::new(AuthoredTextSeedSpec {
            format: style.clone(),
            proofing_language: "ko-KR".into(),
            catalog: vec![style, unused],
            text_decorations: None,
        })
        .unwrap(),
    )
    .unwrap()
}
fn minimal() -> NativePackage {
    minimal_package(&CanvasDocument::default()).unwrap()
}
fn parts(package: &NativePackage) -> Vec<(String, Vec<u8>)> {
    package
        .parts()
        .iter()
        .map(|p| (p.name.clone(), p.bytes().to_vec()))
        .collect()
}

#[test]
fn empty_and_committed_own_source_generate_actual_native_authority_and_first_typing() {
    let original = minimal();
    let a = authored();
    let candidate = append_region(&original, 1, 12.5, 34.25, &a).unwrap();
    let catalog = candidate.original_xml_part(CATALOG).unwrap().unwrap();
    assert!(catalog.contains("style-key=\"Heading 1\""));
    assert!(!catalog.contains("Unused snapshot only"));
    assert!(catalog.contains("ProofingLanguage=\"ko-KR\""));
    let authority = read_native_text_authority(&candidate, "1").unwrap();
    let imported = ValidatedRichText::from_original(authority.clone(), "").unwrap();
    assert_eq!(imported.xaml(), a.xaml());
    let typed = imported
        .replace_in_run(0, 0, 0, "한😀")
        .unwrap()
        .unwrap()
        .source;
    let mut saved = candidate.clone();
    saved
        .set_part(
            &authority.anchor().identity().package_part,
            typed.export_package().unwrap().to_vec(),
        )
        .unwrap();
    let copy = NativePackage::from_bytes(&saved.to_bytes().unwrap()).unwrap();
    let reopened =
        ValidatedRichText::from_original(read_native_text_authority(&copy, "1").unwrap(), "한😀")
            .unwrap();
    assert_eq!(reopened.xaml(), typed.xaml());
    assert!(reopened.xaml().contains("xml:lang=\"ko-KR\""));
    assert!(reopened.xaml().contains("FontStyle=\"Italic\""));
    let own = a.replace_in_run(0, 0, 0, "직접😀").unwrap().unwrap().source;
    let bytes = serde_json::to_vec(own.wire()).unwrap();
    let own = AuthoredRichText::from_json(&bytes, "직접😀").unwrap();
    let second = append_region(&copy, 2, 22., 70., &own).unwrap();
    assert_eq!(
        ValidatedRichText::from_original(
            read_native_text_authority(&second, "2").unwrap(),
            "직접😀"
        )
        .unwrap()
        .xaml(),
        own.xaml()
    );
    assert_eq!(
        original.worksheet_xml().unwrap(),
        minimal().worksheet_xml().unwrap()
    );
}
#[test]
fn exact_matching_definition_reuses_catalog_and_unknown_original_parts_stay_exact() {
    let mut original = append_region(&minimal(), 1, 0., 0., &authored()).unwrap();
    original
        .set_part("custom/resource.bin", vec![0, 255, 42])
        .unwrap();
    let catalog = original
        .original_xml_part(CATALOG)
        .unwrap()
        .unwrap()
        .replace("<pageModel", "<pageModel retained=\"unknown\"");
    original
        .set_part(CATALOG, catalog.clone().into_bytes())
        .unwrap();
    let before = parts(&original);
    let a = authored()
        .replace_in_run(0, 0, 0, "body")
        .unwrap()
        .unwrap()
        .source;
    let result = append_region(&original, 2, 10., 20., &a).unwrap();
    assert_eq!(
        result.original_part(CATALOG),
        original.original_part(CATALOG)
    );
    assert_eq!(
        result.original_part("custom/resource.bin"),
        Some([0, 255, 42].as_slice())
    );
    for (name, bytes) in &before {
        if name.ends_with("XamlPackage") {
            assert_eq!(result.original_part(name), Some(bytes.as_slice()));
        }
    }
    assert_eq!(parts(&original), before);
}
#[test]
fn conflict_collision_missing_style_key_and_unknown_target_attrs_fail_atomically() {
    let original = append_region(&minimal(), 1, 0., 0., &authored()).unwrap();
    let before = parts(&original);
    assert!(append_region(&original, 1, 0., 0., &authored()).is_err());
    assert_eq!(parts(&original), before);
    let catalog = original.original_xml_part(CATALOG).unwrap().unwrap();
    for bad in [
        catalog.replace("font-size=\"16\"", "font-size=\"17\""),
        catalog.replace("style-key=\"Heading 1\"", ""),
        catalog.replace("font-size=\"16\"", "font-size=\"16\" future=\"x\""),
    ] {
        let mut changed = original.clone();
        changed.set_part(CATALOG, bad.into_bytes()).unwrap();
        let before = parts(&changed);
        assert!(append_region(&changed, 2, 0., 0., &authored()).is_err());
        assert_eq!(parts(&changed), before);
    }
    assert!(append_region(&original, 2, f64::NAN, 0., &authored()).is_err());
}
#[test]
fn actual_relationship_and_contenttype_reservations_are_not_reused() {
    let mut original = minimal();
    let worksheet = original.worksheet_part_name().to_owned();
    let rel = relationship_part(&worksheet).unwrap();
    let existing = original
        .original_xml_part(&rel)
        .unwrap()
        .unwrap_or_else(|| format!("<Relationships xmlns=\"{REL}\"/>"));
    set_xml(&mut original,&rel,insert_child(&existing,&format!("<Relationship xmlns=\"{REL}\" Id=\"MathcraftText7_0\" Type=\"urn:unknown\" Target=\"/custom.bin\"/>" )).unwrap()).unwrap();
    let types = original
        .original_xml_part("[Content_Types].xml")
        .unwrap()
        .unwrap();
    set_xml(&mut original,"[Content_Types].xml",insert_child(&types,&format!("<Override xmlns=\"{CONTENT}\" PartName=\"/mathcad/xaml/MathcraftText7_1.XamlPackage\" ContentType=\"application/zip\"/>" )).unwrap()).unwrap();
    let result = append_region(&original, 7, 0., 0., &authored()).unwrap();
    let authority = read_native_text_authority(&result, "7").unwrap();
    assert_eq!(
        authority.anchor().identity().package_part,
        "mathcad/xaml/MathcraftText7_2.XamlPackage"
    );
    assert!(
        result
            .original_part("mathcad/xaml/MathcraftText7_1.XamlPackage")
            .is_none()
    );
}
#[test]
fn plain_none_owner_nested_package_is_unchanged_and_authored_local_override_is_not_catalog() {
    let mut original = minimal();
    let worksheet = original.worksheet_part_name().to_owned();
    let item = add_text_part(&mut original, &worksheet, "1", "old plain\nbody").unwrap();
    let old_name = "mathcad/xaml/MathcraftText1_0.XamlPackage";
    let old = original.original_part(old_name).unwrap().to_vec();
    assert_eq!(item, "MathcraftText1_0");
    let base = authored()
        .replace_in_run(0, 0, 0, "styled")
        .unwrap()
        .unwrap()
        .source;
    let styled = base
        .set_selection_format(
            TextSelection {
                paragraph: 0,
                anchor: 0,
                focus: 6,
            },
            &TextFormat {
                weight: Some(Weight::Bold),
                ..Default::default()
            },
        )
        .unwrap()
        .unwrap()
        .source;
    let result = append_region(&original, 2, 0., 0., &styled).unwrap();
    assert_eq!(result.original_part(old_name), Some(old.as_slice()));
    let catalog = result.original_xml_part(CATALOG).unwrap().unwrap();
    assert!(catalog.contains("font-weight=\"Normal\""));
    let current = ValidatedRichText::from_original(
        read_native_text_authority(&result, "2").unwrap(),
        "styled",
    )
    .unwrap();
    assert!(current.xaml().contains("FontWeight=\"Bold\""));
}

#[test]
fn actual_new_region_hook_installs_parts_but_returns_exactly_one_unappended_owner() {
    let source = authored()
        .replace_in_run(0, 0, 0, "새😀")
        .unwrap()
        .unwrap()
        .source;
    let mut canvas = CanvasDocument::default();
    canvas.add_text(12.5, 35., source.plain_text());
    canvas.regions[0].authored_text = Some(Box::new(source.wire().clone()));
    let mut package = minimal();
    let part = package.worksheet_part_name().to_owned();
    let before = package.original_part(&part).unwrap().to_vec();
    let mut ids = ResultIds {
        next: 1,
        pending: Vec::new(),
        next_region: 100,
        owners: HashMap::new(),
        plots: HashMap::new(),
    };
    let mut texts = text_source::prepare(&CanvasDocument::default(), &package).unwrap();
    let fragment = new_region(
        &canvas.regions[0],
        "1",
        &mut package,
        &part,
        &mut ids,
        &mut texts.derived,
    )
    .unwrap();
    assert_eq!(package.original_part(&part), Some(before.as_slice()));
    assert!(ids.pending.is_empty());
    append_owner(&mut package, &fragment).unwrap();
    validate_installed(&package, 1, &source).unwrap();
    let xml = package.worksheet_xml().unwrap();
    let doc = parse(&xml).unwrap();
    assert_eq!(
        doc.descendants()
            .filter(|n| n.has_tag_name((WS, "region")) && n.attribute("region-id") == Some("1"))
            .count(),
        1
    );
}
#[test]
fn actual_default_name_only_layout_normalization_only_adds_exact_keys_preserving_point_lexemes() {
    let xml = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/reference/rich-text/authored-default-presentation.xml"
    ));
    let normalized = normalize_layout_keys(xml).unwrap();
    let mut restored = normalized.clone();
    for name in ["areaLayoutStyle", "headerLayoutStyle", "footerLayoutStyle"] {
        restored = restored.replace(&format!(" style-key=\"{name}\""), "");
    }
    assert_eq!(restored, xml);
    assert_eq!(
        normalized.matches("font-size=\"11\"").count(),
        xml.matches("font-size=\"11\"").count()
    );
    let mut package = minimal();
    package.set_part(CATALOG, xml.as_bytes().to_vec()).unwrap();
    let source = authored();
    let candidate = append_region(&package, 1, 0., 0., &source).unwrap();
    validate_installed(&candidate, 1, &source).unwrap();
    let output = candidate.original_xml_part(CATALOG).unwrap().unwrap();
    assert!(output.contains("font-size=\"11\""));
    assert!(output.contains("style-key=\"Heading 1\""));
    assert_eq!(package.original_part(CATALOG), Some(xml.as_bytes()));
}
#[test]
fn reserved_layout_source_and_duplicate_or_unknown_name_only_catalogs_reject_atomically() {
    for key in ["areaLayoutStyle", "headerLayoutStyle", "footerLayoutStyle"] {
        let mut spec = authored().seed().spec().clone();
        spec.format.key = key.into();
        spec.catalog[0].key = key.into();
        let source = AuthoredRichText::empty(AuthoredTextSeed::new(spec).unwrap()).unwrap();
        let package = minimal();
        let before = parts(&package);
        assert!(append_region(&package, 1, 0., 0., &source).is_err());
        assert_eq!(parts(&package), before);
    }
    let xml = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/reference/rich-text/authored-default-presentation.xml"
    ));
    for bad in [
        xml.replace("name=\"headerLayoutStyle\"", "name=\"areaLayoutStyle\""),
        xml.replace(
            "name=\"headerLayoutStyle\"",
            "name=\"headerLayoutStyle\" style-key=\"areaLayoutStyle\"",
        ),
        xml.replace("name=\"headerLayoutStyle\"", "name=\"UnknownStyle\""),
    ] {
        let mut package = minimal();
        package.set_part(CATALOG, bad.into_bytes()).unwrap();
        let before = parts(&package);
        assert!(append_region(&package, 1, 0., 0., &authored()).is_err());
        assert_eq!(parts(&package), before);
    }
}

#[test]
fn actual_lcc_normal_reuses_original_default_attributes_and_exact_dip() {
    let mut original = minimal();
    let catalog =
        include_str!("../../../../tests/reference/rich-text/lcc-standard-presentation.xml");
    set_xml(&mut original, CATALOG, catalog.into()).unwrap();
    let style = AuthoredStyleSnapshot {
        key: "Normal".into(),
        font_family: "Tahoma".into(),
        size_dip: "14.666666666666668".into(),
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
            proofing_language: "ko-kr".into(),
            text_decorations: None,
        })
        .unwrap(),
    )
    .unwrap();
    let source = empty
        .replace_in_run(0, 0, 0, "한😀")
        .unwrap()
        .unwrap()
        .source;
    let native = append_region(&original, 1, 0., 0., &source).unwrap();
    assert_eq!(
        native.original_part(CATALOG),
        original.original_part(CATALOG)
    );
    let cold =
        ValidatedRichText::from_original(read_native_text_authority(&native, "1").unwrap(), "한😀")
            .unwrap();
    assert_eq!(cold.xaml(), source.xaml());
    for (from, to) in [
        ("based-on-key=\"\"", "based-on-key=\"Other\""),
        ("marker-style=\"None\"", "marker-style=\"Bullet\""),
    ] {
        let mut invalid = original.clone();
        set_xml(&mut invalid, CATALOG, catalog.replace(from, to)).unwrap();
        assert!(append_region(&invalid, 1, 0., 0., &source).is_err());
        assert_eq!(
            native.original_part(CATALOG),
            original.original_part(CATALOG)
        );
    }
}
