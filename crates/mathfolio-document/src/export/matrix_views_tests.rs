use super::super::{export_into, minimal_package, source_context_matches};
use super::*;
use crate::canvas::{
    MatrixResultTarget, MatrixViewOffset, MatrixViewSize, SavedCalculationCache,
    SavedCalculationRow, SavedCalculationStatus, SavedNumericSource,
};

struct Fixture {
    document: CanvasDocument,
    package: NativePackage,
    owner: u64,
    reference: i32,
}

fn request() -> MatrixResultViewport {
    MatrixResultViewport {
        size: Some(MatrixViewSize { rows: 3, cols: 7 }),
        offset: Some(MatrixViewOffset { row: 5, col: 2 }),
        show_indices: Some(true),
    }
}

fn fixture(matrix: &str) -> Fixture {
    let mut document = CanvasDocument::default();
    let definition = document.add_math(0., 0.);
    document.regions[0].content = CanvasContent::Math(
        mathfolio_compat::editor_bridge::tree_from_mathml(&format!(
            "<ml:define xmlns:ml='{ML}'><ml:id labels='VARIABLE'>upstream</ml:id><ml:real>3</ml:real></ml:define>"
        )).unwrap());
    let owner = document.add_math(0., 50.);
    let source_cells = (1..=12)
        .map(|i| format!("<ml:real>{i}.125</ml:real>"))
        .collect::<String>();
    document.regions[1].content = CanvasContent::Math(
        mathfolio_compat::editor_bridge::tree_from_mathml(&format!(
            "<ml:eval xmlns:ml='{ML}'><ml:matrix rows='3' cols='4'>{source_cells}</ml:matrix><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>"
        )).unwrap());
    let mut package = minimal_package(&document).unwrap();
    export_into(&document, &mut package).unwrap();
    let source = package.worksheet_xml().unwrap();
    let doc = parse(&source).unwrap();
    let native = doc
        .root_element()
        .children()
        .find(|n| n.has_tag_name((WS, "regions")))
        .unwrap()
        .children()
        .filter(|n| n.has_tag_name((WS, "region")))
        .collect::<Vec<_>>();
    assert_eq!(native.len(), 2);
    for (region, old) in document.regions.iter_mut().zip(&native) {
        region.source_region = old.attribute("region-id").map(str::to_owned);
    }
    assert_eq!(document.regions[0].id, definition);
    let math = native[1]
        .children()
        .find(|n| n.has_tag_name((WS, "math")))
        .unwrap();
    let reference = crate::native::parse_result_id(math.attribute("resultRef").unwrap()).unwrap();
    let def_ref = native[0]
        .children()
        .find(|n| n.has_tag_name((WS, "math")))
        .unwrap()
        .attribute("resultRef")
        .unwrap()
        .to_owned();
    let addition = format!(
        "<resultFormat><general precision='3'/><!--keep number metadata-->{matrix}</resultFormat>"
    );
    let updated = insert_child(&source[math.range()], &addition).unwrap();
    let source = apply(source.clone(), vec![(math.range(), updated)]).unwrap();
    set_xml(&mut package, "mathcad/worksheet.xml", source).unwrap();
    let cells = (1..=12)
        .map(|i| format!("<ml:real>{i}.125</ml:real>"))
        .collect::<String>();
    let result = format!(
        "<resultsList xmlns='http://schemas.mathsoft.com/result10' xmlns:ml='{ML}'><resultData result-id='{def_ref}' calculation-status='Pending'/><resultData result-id='{reference}' calculation-status='Synchronized' tag='original'><ml:result><ml:matrix rows='3' cols='4'>{cells}</ml:matrix></ml:result><!--exact payload marker--></resultData></resultsList>"
    );
    package
        .set_part("mathcad/result.xml", result.into_bytes())
        .unwrap();
    document.native_archive = Some(package.to_bytes().unwrap());
    Fixture {
        document,
        package,
        owner,
        reference,
    }
}

fn with_view(f: &mut Fixture) {
    f.document
        .matrix_result_views
        .set(MatrixResultTarget::math(f.owner), request())
        .unwrap();
}
fn mapping(f: &Fixture) -> HashMap<String, u64> {
    f.document
        .regions
        .iter()
        .map(|r| (r.source_region.clone().unwrap(), r.id))
        .collect()
}
fn raw_result(package: &NativePackage) -> Vec<u8> {
    package
        .original_part("mathcad/result.xml")
        .unwrap()
        .to_vec()
}

#[test]
fn imported_view_only_export_keeps_entire_numeric_result_bytes_status_and_unrelated_source() {
    let mut f = fixture(
        "<matrix size=\"12,12\" offset=\"0,0\" show-indices=\"false\" expand-nested-arrays=\"false\"/>",
    );
    let original_source = f.package.worksheet_xml().unwrap();
    let original_result = raw_result(&f.package);
    let archive = f.document.native_archive.clone();
    let signature = f.document.calculation_signature();
    with_view(&mut f);
    assert_eq!(f.document.calculation_signature(), signature);
    assert!(source_context_matches(&f.document, &f.package).unwrap());
    let mut out = f.package.clone();
    export_into(&f.document, &mut out).unwrap();
    assert_eq!(raw_result(&out), original_result);
    let expected = original_source
        .replace("size=\"12,12\"", "size=\"3,7\"")
        .replace("offset=\"0,0\"", "offset=\"5,2\"")
        .replace("show-indices=\"false\"", "show-indices=\"true\"");
    assert_eq!(out.worksheet_xml().unwrap(), expected);
    assert_eq!(f.document.native_archive, archive);
    let cache = out
        .caches()
        .unwrap()
        .into_iter()
        .find(|r| {
            r.result_id
                .as_deref()
                .and_then(|id| crate::native::parse_result_id(id).ok())
                == Some(f.reference)
        })
        .unwrap();
    assert_eq!(cache.calculation_status.as_deref(), Some("Synchronized"));
    let cold = NativePackage::from_bytes(&out.to_bytes().unwrap()).unwrap();
    assert_eq!(raw_result(&cold), original_result);
    assert_eq!(cold.worksheet_xml().unwrap(), expected);
}

#[test]
fn own_roundtrip_and_repeated_export_stay_bound_to_original_retained_archive() {
    let mut f = fixture("<matrix size='12,12' offset='0,0' expand-nested-arrays='false'/>");
    with_view(&mut f);
    let native = f
        .package
        .caches()
        .unwrap()
        .into_iter()
        .find(|row| {
            row.result_id
                .as_deref()
                .and_then(|id| crate::native::parse_result_id(id).ok())
                == Some(f.reference)
        })
        .unwrap();
    f.document.calculation_cache = Some(SavedCalculationCache {
        source_signature: f.document.calculation_signature(),
        rows: vec![SavedCalculationRow {
            region_id: f.owner,
            status: SavedCalculationStatus::Synchronized,
            display: None,
            error: None,
            warnings: vec![],
            symbolic_mathml: None,
            numeric_source: Some(SavedNumericSource::NativeMathml(
                native.result_mathml.unwrap(),
            )),
        }],
    });
    let cache_before = serde_json::to_vec(&f.document.calculation_cache).unwrap();
    let own = serde_json::to_vec(&f.document).unwrap();
    let cold: CanvasDocument = serde_json::from_slice(&own).unwrap();
    cold.validate().unwrap();
    let mut a = NativePackage::from_bytes(cold.native_archive.as_ref().unwrap()).unwrap();
    let mut b = a.clone();
    export_into(&cold, &mut a).unwrap();
    export_into(&cold, &mut b).unwrap();
    assert_eq!(a.worksheet_xml().unwrap(), b.worksheet_xml().unwrap());
    assert_eq!(raw_result(&a), raw_result(&f.package));
    assert_eq!(raw_result(&b), raw_result(&f.package));
    assert_eq!(serde_json::to_vec(&cold).unwrap(), own);
    assert_eq!(
        serde_json::to_vec(&cold.calculation_cache).unwrap(),
        cache_before
    );
}

#[test]
fn existing_equal_typed_values_preserve_native_lexemes_and_absent_properties() {
    let mut f = fixture("<matrix size='3,7' show-indices='false' expand-nested-arrays='false'/>");
    f.document
        .matrix_result_views
        .set(
            MatrixResultTarget::math(f.owner),
            MatrixResultViewport {
                size: Some(MatrixViewSize { rows: 3, cols: 7 }),
                show_indices: Some(false),
                ..Default::default()
            },
        )
        .unwrap();
    let before = f.package.worksheet_xml().unwrap();
    export_into(&f.document, &mut f.package).unwrap();
    assert_eq!(f.package.worksheet_xml().unwrap(), before);
    assert!(!before.contains("offset="));
}

#[test]
fn fresh_math_uses_existing_owner_and_result_allocator_without_execution_or_origin() {
    let mut document = CanvasDocument::default();
    let id = document.add_math(0., 0.);
    document.regions[0].content = CanvasContent::Math(
        mathfolio_compat::editor_bridge::tree_from_mathml(&format!(
            "<ml:real xmlns:ml='{ML}'>7</ml:real>"
        ))
        .unwrap(),
    );
    document
        .matrix_result_views
        .set(MatrixResultTarget::math(id), request())
        .unwrap();
    let mut out = minimal_package(&document).unwrap();
    export_into(&document, &mut out).unwrap();
    let xml = out.worksheet_xml().unwrap();
    let doc = parse(&xml).unwrap();
    let math = doc
        .descendants()
        .find(|n| n.has_tag_name((WS, "math")))
        .unwrap();
    assert_eq!(
        crate::matrix_result_format::read_local(math).unwrap(),
        request()
    );
    assert!(
        out.caches()
            .unwrap()
            .iter()
            .all(|r| r.calculation_status.as_deref() == Some("Pending"))
    );
    assert!(!xml.contains("origin="));
    assert!(document.native_archive.is_none());
}

#[test]
fn changed_upstream_source_still_marks_old_matrix_pending_and_keeps_full_payload() {
    let mut f = fixture("<matrix size='12,12' offset='0,0'/>");
    with_view(&mut f);
    f.document.regions[0].content=CanvasContent::Math(
        mathfolio_compat::editor_bridge::tree_from_mathml(&format!("<ml:define xmlns:ml='{ML}'><ml:id labels='VARIABLE'>upstream</ml:id><ml:real>99</ml:real></ml:define>")).unwrap());
    assert!(!source_context_matches(&f.document, &f.package).unwrap());
    let original = f
        .package
        .original_xml_part("mathcad/result.xml")
        .unwrap()
        .unwrap();
    let original_doc = parse(&original).unwrap();
    let matrix = original_doc
        .descendants()
        .find(|n| n.has_tag_name((ML, "matrix")))
        .unwrap();
    let payload = original[matrix.range()].to_owned();
    export_into(&f.document, &mut f.package).unwrap();
    let result = f
        .package
        .original_xml_part("mathcad/result.xml")
        .unwrap()
        .unwrap();
    assert!(result.contains(&payload));
    let cache = f
        .package
        .caches()
        .unwrap()
        .into_iter()
        .find(|r| {
            r.result_id
                .as_deref()
                .and_then(|id| crate::native::parse_result_id(id).ok())
                == Some(f.reference)
        })
        .unwrap();
    assert_eq!(cache.calculation_status.as_deref(), Some("Pending"));
}

#[test]
fn unknown_nested_and_ambiguous_bindings_fail_before_helper_mutates_any_part() {
    for matrix in [
        "<matrix future='raw'/>",
        "<matrix><nestedMatrixFormats/></matrix>",
        "<matrix/><matrix/>",
        "<matrix size='03,07' show-indices='0'/>",
    ] {
        let mut f = fixture(matrix);
        let mut unchanged = f.package.clone();
        export_into(&f.document, &mut unchanged).unwrap();
        assert_eq!(
            unchanged.worksheet_xml().unwrap(),
            f.package.worksheet_xml().unwrap()
        );
        assert_eq!(raw_result(&unchanged), raw_result(&f.package));
        with_view(&mut f);
        let before = f.package.worksheet_xml().unwrap();
        let raw = raw_result(&f.package);
        assert!(
            prepare(
                &f.document,
                &f.package,
                &mapping(&f),
                &f.package.result_reference_counts().unwrap()
            )
            .is_err()
        );
        assert_eq!(f.package.worksheet_xml().unwrap(), before);
        assert_eq!(raw_result(&f.package), raw);
    }
    let mut f = fixture("<matrix size='12,12'/>");
    with_view(&mut f);
    let mut owners = mapping(&f);
    owners.insert("other-owner".into(), f.owner);
    assert!(
        prepare(
            &f.document,
            &f.package,
            &owners,
            &f.package.result_reference_counts().unwrap()
        )
        .is_err()
    );
    assert!(
        prepare(
            &f.document,
            &f.package,
            &HashMap::new(),
            &f.package.result_reference_counts().unwrap()
        )
        .is_err()
    );
    assert!(prepare(&f.document, &f.package, &mapping(&f), &HashMap::new()).is_err());
    let mut references = f.package.result_reference_counts().unwrap();
    references.insert(f.reference, 2);
    assert!(prepare(&f.document, &f.package, &mapping(&f), &references).is_err());
    let mut wrong_owner = f.document.clone();
    wrong_owner.regions[1].content = CanvasContent::Math(
        mathfolio_compat::editor_bridge::tree_from_mathml(&format!(
            "<ml:real xmlns:ml='{ML}'>999</ml:real>"
        ))
        .unwrap(),
    );
    assert!(
        prepare(
            &wrong_owner,
            &f.package,
            &mapping(&f),
            &f.package.result_reference_counts().unwrap()
        )
        .is_err()
    );
    let source = f.package.worksheet_xml().unwrap();
    let doc = parse(&source).unwrap();
    let id = f.document.regions[1].source_region.as_ref().unwrap();
    let region = doc
        .descendants()
        .find(|n| n.has_tag_name((WS, "region")) && n.attribute("region-id") == Some(id.as_str()))
        .unwrap();
    let extra = source[region.range()].replace(
        &format!("region-id=\"{id}\""),
        &format!("region-id=\"0{id}\""),
    );
    let updated = source.replace("</regions>", &format!("{extra}</regions>"));
    set_xml(&mut f.package, "mathcad/worksheet.xml", updated).unwrap();
    assert!(
        prepare(
            &f.document,
            &f.package,
            &mapping(&f),
            &f.package.result_reference_counts().unwrap()
        )
        .is_err()
    );
}

#[test]
fn stale_receipt_cannot_change_another_source_and_area_export_fails_explicitly() {
    let mut f = fixture("<matrix size='12,12'/>");
    with_view(&mut f);
    let edits = prepare(
        &f.document,
        &f.package,
        &mapping(&f),
        &f.package.result_reference_counts().unwrap(),
    )
    .unwrap();
    let changed = f
        .package
        .worksheet_xml()
        .unwrap()
        .replace("keep number metadata", "changed after receipt");
    set_xml(&mut f.package, "mathcad/worksheet.xml", changed.clone()).unwrap();
    let raw = raw_result(&f.package);
    assert!(apply_deferred(&mut f.package, edits).is_err());
    assert_eq!(f.package.worksheet_xml().unwrap(), changed);
    assert_eq!(raw_result(&f.package), raw);
    let mut area = CanvasDocument::default();
    let owner=area.add_opaque(0.,0.,format!("<region xmlns='{WS}' xmlns:ml='{ML}'><Area><regions><region left='0' top='0'><math><ml:real>1</ml:real></math></region></regions></Area></region>"),"Area");
    area.matrix_result_views
        .set(
            MatrixResultTarget::area_child(owner, vec![0, 0, 0]),
            request(),
        )
        .unwrap();
    let error = prepare(&area, &f.package, &HashMap::new(), &HashMap::new())
        .err()
        .unwrap();
    assert!(error.contains("Area matrix view"));
}

#[test]
fn actual_format_only_text_cow_and_matrix_view_compose_without_changing_any_result_byte() {
    use super::super::{FLOW_REL, text_source, zip_bytes};
    use crate::rich_text_source::{TextFormat, TextSelection, ValidatedRichText, Weight};
    let mut f = fixture("<matrix size='12,12' offset='0,0' show-indices='false'/>");
    let original_result = raw_result(&f.package);
    let text_id = f.document.add_text(0., 100., "A😀BC");
    // Build one complete static original package before taking source authority.
    // These setup operations never invoke a calculator or execute this source.
    export_into(&f.document, &mut f.package).unwrap();
    f.package
        .set_part("mathcad/result.xml", original_result.clone())
        .unwrap();
    let source = f.package.worksheet_xml().unwrap();
    let doc = parse(&source).unwrap();
    let text = doc
        .descendants()
        .find(|n| n.has_tag_name((WS, "text")))
        .unwrap();
    let text_owner = text
        .parent_element()
        .unwrap()
        .attribute("region-id")
        .unwrap()
        .to_owned();
    let item = text.attribute("item-idref").unwrap().to_owned();
    let target = crate::native_visual::resolve_relationship(
        &f.package,
        f.package.worksheet_part_name(),
        &item,
        FLOW_REL,
    )
    .unwrap();
    let x = "http://schemas.microsoft.com/winfx/2006/xaml/presentation";
    let xaml = format!(
        "<Section xmlns='{x}' FontWeight='Normal' FontStyle='Normal' FontSize='16' Foreground='#FF000000'><Paragraph><Run style='Normal'>A😀BC</Run></Paragraph></Section>"
    );
    let nested = zip_bytes(&[
        ("Xaml/Document.xaml".into(), xaml.into_bytes()),
        ("Resources/exact.bin".into(), vec![0, 255, 3, 9]),
    ])
    .unwrap();
    f.package.set_part(&target, nested.clone()).unwrap();
    let catalog = "mathcad/settings/presentation.xml";
    set_xml(&mut f.package,catalog,
        "<presentation xmlns='http://schemas.ptc.com/mathcad/settings/presentation10'><textRendering><textStyles><textStyle style-key='Normal'/></textStyles></textRendering></presentation>".into()).unwrap();
    f.package = NativePackage::from_bytes(&f.package.to_bytes().unwrap()).unwrap();
    f.document
        .regions
        .iter_mut()
        .find(|r| r.id == text_id)
        .unwrap()
        .source_region = Some(text_owner.clone());
    f.document.native_archive = Some(f.package.to_bytes().unwrap());
    let authority = text_source::read_native_authority(&f.package, &text_owner, catalog).unwrap();
    let original = ValidatedRichText::from_original(authority, "A😀BC").unwrap();
    let changed = original
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
        .unwrap();
    changed.source.prove_format_only().unwrap();
    let expected_xaml = changed.source.xaml().to_owned();
    f.document
        .regions
        .iter_mut()
        .find(|r| r.id == text_id)
        .unwrap()
        .rich_text = Some(Box::new(changed.source.wire().clone()));
    with_view(&mut f);
    // A valid CONTENT COW is still a context change; the view receipt is not
    // permission to preserve stale synchronized status for that other edit.
    let content = original.replace_in_run(0, 0, 1, "Z").unwrap().unwrap();
    let mut changed_context = f.document.clone();
    let text_region = changed_context
        .regions
        .iter_mut()
        .find(|r| r.id == text_id)
        .unwrap();
    text_region.content = CanvasContent::Text(content.source.plain_text().into());
    text_region.rich_text = Some(Box::new(content.source.wire().clone()));
    assert!(!source_context_matches(&changed_context, &f.package).unwrap());
    let mut pending_package = f.package.clone();
    export_into(&changed_context, &mut pending_package).unwrap();
    let pending_row = pending_package
        .caches()
        .unwrap()
        .into_iter()
        .find(|r| {
            r.result_id
                .as_deref()
                .and_then(|id| crate::native::parse_result_id(id).ok())
                == Some(f.reference)
        })
        .unwrap();
    assert_eq!(pending_row.calculation_status.as_deref(), Some("Pending"));
    let old_row = f
        .package
        .caches()
        .unwrap()
        .into_iter()
        .find(|r| {
            r.result_id
                .as_deref()
                .and_then(|id| crate::native::parse_result_id(id).ok())
                == Some(f.reference)
        })
        .unwrap();
    assert_eq!(pending_row.result_mathml, old_row.result_mathml);
    assert!(source_context_matches(&f.document, &f.package).unwrap());
    export_into(&f.document, &mut f.package).unwrap();
    assert_eq!(raw_result(&f.package), original_result);
    assert_eq!(f.package.original_part(&target).unwrap(), nested);
    let reopened = text_source::read_native_authority(&f.package, &text_owner, catalog).unwrap();
    assert_eq!(reopened.original_xaml(), expected_xaml);
    let source = f.package.worksheet_xml().unwrap();
    let doc = parse(&source).unwrap();
    let owner = f.document.regions[1].source_region.as_deref().unwrap();
    let math = doc
        .descendants()
        .find(|n| n.has_tag_name((WS, "region")) && n.attribute("region-id") == Some(owner))
        .unwrap()
        .children()
        .find(|n| n.has_tag_name((WS, "math")))
        .unwrap();
    assert_eq!(
        crate::matrix_result_format::read_local(math).unwrap(),
        request()
    );
    let text = doc
        .descendants()
        .find(|n| n.has_tag_name((WS, "text")))
        .unwrap();
    assert_ne!(text.attribute("item-idref"), Some(item.as_str()));
}

#[test]
fn prefixed_math_gets_namespace_bound_matrix_without_replacing_number_metadata() {
    let mut f = fixture("");
    let mut source = f.package.worksheet_xml().unwrap();
    for name in [
        "worksheet",
        "regions",
        "region",
        "math",
        "resultFormat",
        "general",
    ] {
        source = source
            .replace(&format!("<{name}"), &format!("<w:{name}"))
            .replace(&format!("</{name}>"), &format!("</w:{name}>"));
    }
    source = source.replacen("<w:worksheet ", &format!("<w:worksheet xmlns:w='{WS}' "), 1);
    set_xml(&mut f.package, "mathcad/worksheet.xml", source).unwrap();
    f.document.native_archive = Some(f.package.to_bytes().unwrap());
    with_view(&mut f);
    let raw = raw_result(&f.package);
    export_into(&f.document, &mut f.package).unwrap();
    let source = f.package.worksheet_xml().unwrap();
    assert!(source.contains("<w:general precision='3'/><!--keep number metadata-->"));
    assert!(source.contains(&format!("<matrix xmlns=\"{WS}\"")));
    let doc = parse(&source).unwrap();
    let matrix = doc
        .descendants()
        .find(|n| n.has_tag_name((WS, "matrix")))
        .unwrap();
    assert!(
        matrix
            .parent_element()
            .unwrap()
            .has_tag_name((WS, "resultFormat"))
    );
    assert_eq!(matrix.attribute("size"), Some("3,7"));
    assert_eq!(raw_result(&f.package), raw);
}

#[test]
fn utf16_worksheet_encoding_and_unrelated_bytes_survive_deferred_patch() {
    let mut f = fixture("<matrix size='12,12' expand-nested-arrays='false'/>");
    let source = format!(
        "<?xml version='1.0' encoding='utf-16'?>{}",
        f.package.worksheet_xml().unwrap()
    );
    let bytes = [0xff, 0xfe]
        .into_iter()
        .chain(source.encode_utf16().flat_map(u16::to_le_bytes))
        .collect::<Vec<_>>();
    f.package.set_part("mathcad/worksheet.xml", bytes).unwrap();
    f.document.native_archive = Some(f.package.to_bytes().unwrap());
    with_view(&mut f);
    let raw = raw_result(&f.package);
    export_into(&f.document, &mut f.package).unwrap();
    assert!(
        f.package
            .original_part("mathcad/worksheet.xml")
            .unwrap()
            .starts_with(&[0xff, 0xfe])
    );
    let source = f.package.worksheet_xml().unwrap();
    assert!(source.contains("encoding='utf-16'"));
    assert!(source.contains("keep number metadata"));
    assert!(source.contains("expand-nested-arrays='false'"));
    assert_eq!(raw_result(&f.package), raw);
}
