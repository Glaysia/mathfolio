use super::*;

const NATIVE_CATALOG: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/reference/rich-text/lcc-standard-presentation.xml"
));
const INSTALLED_DEFAULT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/reference/rich-text/installed-default-presentation.xml"
));

fn style(key: &str) -> AuthoredStyleSnapshot {
    AuthoredStyleSnapshot {
        key: key.into(),
        font_family: "SeoulNamsan CB".into(),
        size_dip: "17.25".into(),
        weight: "Bold".into(),
        font_style: "Italic".into(),
        alignment: "Left".into(),
        foreground_rgba: [10, 20, 30, 255],
        background_rgba: [40, 50, 60, 128],
    }
}
fn snapshot(current: AuthoredStyleSnapshot) -> CreationSnapshot {
    CreationSnapshot {
        catalog: vec![style("Normal"), style("Heading 1"), style("Normal~custom")],
        current,
        proofing_language: "ko-kr".into(),
        unsupported: UnsupportedCreationProperties::default(),
    }
}

#[test]
fn layout_key_normalization_keeps_custom_current_appearance_and_language() {
    for key in ["areaLayoutStyle", "headerLayoutStyle", "footerLayoutStyle"] {
        let mut current = style(key);
        current.size_dip = "29.125".into();
        current.font_family = "Custom Explicit Family".into();
        let mut expected = current.clone();
        expected.key = "Normal".into();
        let seed = prepare_creation_seed(snapshot(current)).unwrap();
        assert_eq!(seed.spec().format, expected);
        assert_eq!(seed.spec().proofing_language, "ko-kr");
        assert_eq!(seed.spec().catalog[0].size_dip, "17.25");
    }
}

#[test]
fn ordinary_custom_key_and_local_overrides_are_not_reset_to_catalog_values() {
    for key in ["Heading 1", "Normal~custom"] {
        let mut current = style(key);
        current.size_dip = "37.3333333333333".into();
        current.weight = "Normal".into();
        current.foreground_rgba = [255, 254, 253, 255];
        let seed = prepare_creation_seed(snapshot(current.clone())).unwrap();
        assert_eq!(seed.spec().format, current);
        assert_ne!(
            seed.spec().format,
            seed.spec()
                .catalog
                .iter()
                .find(|s| s.key == key)
                .unwrap()
                .clone()
        );
    }
}

#[test]
fn unresolved_or_missing_catalog_and_unsupported_current_properties_do_not_default() {
    let mut missing = snapshot(style("areaLayoutStyle"));
    missing.catalog.retain(|s| s.key != "Normal");
    assert!(prepare_creation_seed(missing).is_err());
    let mut empty_language = snapshot(style("Normal"));
    empty_language.proofing_language.clear();
    assert!(prepare_creation_seed(empty_language).is_err());
    for property in 0..5 {
        let mut value = snapshot(style("Normal"));
        match property {
            0 => value.unsupported.underline_or_other_decorations = true,
            1 => value.unsupported.nonzero_indent = true,
            2 => value.unsupported.custom_paragraph_or_line_geometry = true,
            3 => value.unsupported.right_to_left = true,
            _ => value.unsupported.unresolved_script_or_font_properties = true,
        }
        assert!(prepare_creation_seed(value).is_err());
    }
}

#[test]
fn actual_explicit_ordinary_style_uses_source_key_dip_lexeme_and_catalog_language() {
    let bytes_before = NATIVE_CATALOG.as_bytes().to_vec();
    let custom =
        seed_for_declared_native_style(NATIVE_CATALOG, "Normal~639058311443748180").unwrap();
    assert_eq!(custom.spec().format.font_family, "SeoulNamsan CB");
    assert_eq!(custom.spec().format.size_dip, "13.3333333333333");
    assert_eq!(custom.spec().format.background_rgba, [49, 173, 194, 255]);
    assert_eq!(custom.spec().proofing_language, "ko-kr");
    let heading = seed_for_declared_native_style(NATIVE_CATALOG, "Heading 1").unwrap();
    assert_eq!(heading.spec().format.key, "Heading 1");
    assert_eq!(heading.spec().format.size_dip, "21.3333333333333");
    assert_eq!(heading.spec().format.foreground_rgba, [46, 49, 146, 255]);
    assert_eq!(NATIVE_CATALOG.as_bytes(), bytes_before);
    for key in [
        "areaLayoutStyle",
        "headerLayoutStyle",
        "footerLayoutStyle",
        "머리글 1",
    ] {
        assert!(seed_for_declared_native_style(NATIVE_CATALOG, key).is_err());
    }
}

#[test]
fn native_catalog_missing_ambiguous_inherited_or_unsupported_selected_style_rejects() {
    for xml in [
        NATIVE_CATALOG.replace("ProofingLanguage=\"ko-kr\"", ""),
        NATIVE_CATALOG.replace("style-key=\"Heading 1\"", "style-key=\"Normal\""),
        NATIVE_CATALOG.replacen(
            "style-key=\"Heading 1\" based-on-key=\"\"",
            "style-key=\"Heading 1\" based-on-key=\"Normal\"",
            1,
        ),
        NATIVE_CATALOG.replacen(
            "style-key=\"Heading 1\"",
            "style-key=\"Heading 1\" unknown=\"value\"",
            1,
        ),
        NATIVE_CATALOG.replace("style-key=\"Heading 1\"", ""),
        NATIVE_CATALOG.replace("<textRendering ", "<textRendering xmlns=\"urn:wrong\" "),
    ] {
        assert!(seed_for_declared_native_style(&xml, "Heading 1").is_err());
    }
    assert!(seed_for_declared_native_style(&"x".repeat(1024 * 1024 + 1), "Normal").is_err());
}

#[test]
fn native_layout_current_uses_points_operation_order_and_preserves_actual_normal_definition() {
    for key in ["areaLayoutStyle", "headerLayoutStyle", "footerLayoutStyle"] {
        let seed = seed_for_native_current_style(NATIVE_CATALOG, key).unwrap();
        assert_eq!(seed.spec().format.key, "Normal");
        assert_eq!(seed.spec().format.size_dip, "14.666666666666668");
        assert_eq!(seed.spec().format.font_family, "Tahoma");
        assert_eq!(seed.spec().catalog[0].key, "Normal");
        assert_eq!(seed.spec().catalog[0].size_dip, "14.666666666666668");
    }
    let changed = NATIVE_CATALOG.replace(
        "name=\"areaLayoutStyle\" font-family=\"Tahoma\" font-size=\"11\"",
        "name=\"areaLayoutStyle\" font-family=\"Explicit Current Family\" font-size=\"27\"",
    );
    assert_ne!(changed, NATIVE_CATALOG);
    let seed = seed_for_native_current_style(&changed, "areaLayoutStyle").unwrap();
    assert_eq!(seed.spec().format.size_dip, "36");
    assert_eq!(seed.spec().format.font_family, "Explicit Current Family");
    assert_eq!(seed.spec().catalog[0].size_dip, "14.666666666666668");
    assert_eq!(seed.spec().catalog[0].font_family, "Tahoma");
    let no_normal = NATIVE_CATALOG.replace("style-key=\"Normal\"", "style-key=\"RemovedNormal\"");
    assert!(seed_for_native_current_style(&no_normal, "areaLayoutStyle").is_err());
    for invalid in ["0", "-1", "NaN", "Infinity", "4096", "1e-6"] {
        assert!(layout_points_to_dip(invalid).is_err());
    }
}

#[test]
fn initial_body_uses_observed_name_only_layout_and_verified_embedded_normal_overlay() {
    let seed = seed_for_native_body(INSTALLED_DEFAULT).unwrap();
    assert_eq!(seed.spec().format.key, "Normal");
    assert_eq!(seed.spec().format.font_family, "Tahoma");
    assert_eq!(seed.spec().format.size_dip, "14.666666666666668");
    assert_eq!(seed.spec().format.foreground_rgba, [0, 0, 0, 255]);
    assert_eq!(seed.spec().format.background_rgba, [255, 255, 255, 0]);
    assert_eq!(seed.spec().proofing_language, "ko-kr");
    assert_eq!(seed.spec().catalog, [seed.spec().format.clone()]);
    assert!(!INSTALLED_DEFAULT.contains("style-key=\"Normal\""));
    let changed = INSTALLED_DEFAULT.replace(
        "font-family=\"Tahoma\" font-size=\"11\"",
        "font-family=\"Body Current Family\" font-size=\"27\"",
    );
    let seed = seed_for_native_body(&changed).unwrap();
    assert_eq!(seed.spec().format.font_family, "Body Current Family");
    assert_eq!(seed.spec().format.size_dip, "36");
    assert_eq!(seed.spec().catalog[0].font_family, "Body Current Family");
    let existing = seed_for_native_body(NATIVE_CATALOG).unwrap();
    assert_eq!(existing.spec().catalog[0].size_dip, "14.666666666666668");
    for bad in [
        INSTALLED_DEFAULT.replace("font-family=\"Tahoma\"", ""),
        INSTALLED_DEFAULT.replace(
            "name=\"areaLayoutStyle\"",
            "name=\"areaLayoutStyle\" text-align=\"Center\"",
        ),
        INSTALLED_DEFAULT.replace(
            "name=\"areaLayoutStyle\"",
            "name=\"areaLayoutStyle\" underline=\"true\"",
        ),
    ] {
        assert!(seed_for_native_body(&bad).is_err());
    }
}
