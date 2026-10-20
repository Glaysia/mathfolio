//! Native rich Text source, ownership, admission and result-cache contracts.
use super::*;
use crate::canvas::{
    SavedCalculationCache, SavedCalculationRow, SavedCalculationStatus, SavedNumericSource,
};
use crate::rich_text_source::{TextFormat, TextSelection, Weight};

const CATALOG: &str = "mathcad/settings/presentation.xml";
const P: &str = "http://schemas.ptc.com/mathcad/settings/presentation10";
const X: &str = "http://schemas.microsoft.com/winfx/2006/xaml/presentation";
const PLAIN: &str = "A😀BC";

fn fixture() -> (CanvasDocument, NativePackage) {
    let mut canvas = CanvasDocument::default();
    canvas.add_text(0., 0., PLAIN);
    for (y, value) in [(50., "7"), (100., "8")] {
        let id = canvas.add_math(0., y);
        canvas.regions.iter_mut().find(|r| r.id == id).unwrap().content = CanvasContent::Math(
            tree_from_mathml(&format!("<ml:eval xmlns:ml='{ML}'><ml:real>{value}</ml:real><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>")).unwrap(),
        );
    }
    canvas.add_plot(0., 150.);
    let mut package = minimal_package(&canvas).unwrap();
    export_into(&canvas, &mut package).unwrap();
    let regions = package.regions().unwrap();
    for (canvas_region, native) in canvas.regions.iter_mut().zip(&regions) {
        canvas_region.source_region = native.region_id.clone();
    }
    let text_owner = canvas.regions[0].source_region.as_deref().unwrap();
    let target = crate::native_visual::resolve_relationship(
        &package,
        package.worksheet_part_name(),
        "MathcraftText1_0",
        FLOW_REL,
    )
    .unwrap();
    let source = format!(
        "<Section xmlns='{X}' FontWeight='Normal' FontStyle='Normal' FontSize='16' Foreground='#FF000000'><Paragraph><Run style='Normal'>{PLAIN}</Run></Paragraph></Section>"
    );
    let nested = zip_bytes(&[
        ("Xaml/Document.xaml".into(), source.into_bytes()),
        ("Resources/exact.bin".into(), vec![0, 255, 3, 9]),
    ])
    .unwrap();
    package.set_part(&target, nested).unwrap();
    set_xml(&mut package, CATALOG, format!("<presentation xmlns='{P}'><textRendering><textStyles><textStyle style-key='Normal'/></textStyles></textRendering></presentation>")).unwrap();
    let math_refs = [
        regions[1]
            .math
            .as_ref()
            .unwrap()
            .result_ref
            .clone()
            .unwrap(),
        regions[2]
            .math
            .as_ref()
            .unwrap()
            .result_ref
            .clone()
            .unwrap(),
    ];
    let plot_ref = crate::native_plot::trace_result_refs(&regions[3].raw_xml).unwrap()[0].unwrap();
    let curve = crate::native_plot_result::finite_unitless_xml(&[[0., 0.], [1., 1.]])
        .unwrap()
        .unwrap();
    let numeric = format!("<ml:result xmlns:ml='{ML}'><ml:real>7</ml:real></ml:result>");
    let old_pending = format!("<ml:result xmlns:ml='{ML}'><ml:real>8</ml:real></ml:result>");
    set_xml(&mut package, "mathcad/result.xml", format!("<resultsList xmlns='{RESULTS}'><resultData result-id='{}' calculation-status='Synchronized' tag='exact scalar'>{numeric}</resultData><resultData result-id='{}' calculation-status='Pending' tag='exact previous'>{old_pending}</resultData><resultData result-id='{plot_ref}' calculation-status='Synchronized' tag='exact plot'>{curve}</resultData></resultsList>",math_refs[0],math_refs[1])).unwrap();
    // Reconstruct from the actual ZIP so the source anchor is the native bytes.
    package = NativePackage::from_bytes(&package.to_bytes().unwrap()).unwrap();
    canvas.native_archive = Some(package.to_bytes().unwrap());
    let authority = text_source::read_native_authority(&package, text_owner, CATALOG).unwrap();
    let original =
        crate::rich_text_source::ValidatedRichText::from_original(authority, PLAIN).unwrap();
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
    canvas.regions[0].rich_text = Some(Box::new(changed.source.wire().clone()));
    let rows = [
        (1usize, SavedCalculationStatus::Synchronized, numeric),
        (2, SavedCalculationStatus::Pending, old_pending),
        (3, SavedCalculationStatus::Synchronized, curve),
    ]
    .into_iter()
    .map(|(index, status, source)| SavedCalculationRow {
        region_id: canvas.regions[index].id,
        status,
        display: None,
        error: None,
        warnings: vec![],
        symbolic_mathml: None,
        numeric_source: Some(SavedNumericSource::NativeMathml(source)),
    })
    .collect();
    canvas.calculation_cache = Some(SavedCalculationCache {
        source_signature: canvas.calculation_signature(),
        rows,
    });
    (canvas, package)
}

#[test]
fn inert_format_native_origin_keeps_entire_numeric_and_plot_result_part_exact() {
    let (canvas, mut package) = fixture();
    let before = package
        .original_part("mathcad/result.xml")
        .unwrap()
        .to_vec();
    let old_xaml = package
        .parts()
        .iter()
        .filter(|p| p.name.ends_with(".XamlPackage"))
        .map(|p| (p.name.clone(), p.bytes().to_vec()))
        .collect::<Vec<_>>();
    assert!(source_context_matches(&canvas, &package).unwrap());
    export_into(&canvas, &mut package).unwrap();
    assert_eq!(
        package.original_part("mathcad/result.xml"),
        Some(before.as_slice())
    );
    for (name, bytes) in old_xaml {
        assert_eq!(package.original_part(&name), Some(bytes.as_slice()));
    }
    let reopened = NativePackage::from_bytes(&package.to_bytes().unwrap()).unwrap();
    assert_eq!(
        reopened.original_part("mathcad/result.xml"),
        Some(before.as_slice())
    );
    assert!(
        reopened
            .parts()
            .iter()
            .filter(|p| p.name.ends_with(".XamlPackage"))
            .count()
            >= 2
    );
}

#[test]
fn inert_format_no_own_cache_keeps_same_whole_context_proof_and_native_bytes() {
    let (mut canvas, mut package) = fixture();
    canvas.calculation_cache = None;
    let before = package
        .original_part("mathcad/result.xml")
        .unwrap()
        .to_vec();
    assert!(source_context_matches(&canvas, &package).unwrap());
    export_into(&canvas, &mut package).unwrap();
    assert_eq!(
        package.original_part("mathcad/result.xml"),
        Some(before.as_slice())
    );
}

#[test]
fn rich_content_edit_keeps_full_styles_and_old_payload_but_demotes_native_status() {
    let (mut canvas, mut package) = fixture();
    let owner = canvas.regions[0].source_region.as_deref().unwrap();
    let authority = text_source::read_native_authority(&package, owner, CATALOG).unwrap();
    let original =
        crate::rich_text_source::ValidatedRichText::from_original(authority, PLAIN).unwrap();
    let edit = original.replace_in_run(0, 0, 1, "Z").unwrap().unwrap();
    canvas.regions[0].content = CanvasContent::Text(edit.source.plain_text().into());
    canvas.regions[0].rich_text = Some(Box::new(edit.source.wire().clone()));
    assert!(!source_context_matches(&canvas, &package).unwrap());
    let old = package.caches().unwrap();
    export_into(&canvas, &mut package).unwrap();
    let actual = package.caches().unwrap();
    for before in old {
        let after = actual
            .iter()
            .find(|n| n.result_id == before.result_id)
            .unwrap();
        assert_eq!(after.calculation_status.as_deref(), Some("Pending"));
        assert_eq!(after.result_mathml, before.result_mathml);
        assert_eq!(after.tag, before.tag);
    }
}

#[test]
fn paragraph_split_is_content_cow_and_retains_old_numeric_and_plot_payloads_pending() {
    let (mut canvas, mut package) = fixture();
    let owner = canvas.regions[0].source_region.clone().unwrap();
    let authority = text_source::read_native_authority(&package, &owner, CATALOG).unwrap();
    let current = crate::rich_text_source::ValidatedRichText::from_wire(
        canvas.regions[0].rich_text.as_deref().unwrap().clone(),
        authority,
        PLAIN,
    )
    .unwrap();
    let split = current
        .split_paragraph_at_caret(TextSelection {
            paragraph: 0,
            anchor: 2,
            focus: 2,
        })
        .unwrap()
        .patch
        .source;
    assert_eq!(split.plain_text(), "A😀\nBC");
    assert!(split.prove_format_only().is_err());
    canvas.regions[0].content = CanvasContent::Text(split.plain_text().into());
    canvas.regions[0].rich_text = Some(Box::new(split.wire().clone()));
    canvas.validate().unwrap();
    let own: CanvasDocument =
        serde_json::from_slice(&serde_json::to_vec(&canvas).unwrap()).unwrap();
    own.validate().unwrap();
    assert!(!source_context_matches(&own, &package).unwrap());
    let before = package.caches().unwrap();
    export_into(&own, &mut package).unwrap();
    let actual = package.caches().unwrap();
    assert_eq!(before.len(), 3);
    for before in before {
        let after = actual
            .iter()
            .find(|row| row.result_id == before.result_id)
            .unwrap();
        assert_eq!(after.calculation_status.as_deref(), Some("Pending"));
        assert_eq!(after.result_mathml, before.result_mathml);
        assert_eq!(after.tag, before.tag);
    }
    let reopened = text_source::read_native_authority(&package, &owner, CATALOG).unwrap();
    assert_eq!(reopened.original_xaml(), split.xaml());
    assert_eq!(reopened.original_plain_text(), "A😀\nBC");
}

#[test]
fn upstream_math_change_cannot_use_inert_text_receipt_to_promote_stale_results() {
    let (mut canvas, mut package) = fixture();
    canvas.regions[1].content = CanvasContent::Math(tree_from_mathml(&format!("<ml:eval xmlns:ml='{ML}'><ml:real>77</ml:real><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>")).unwrap());
    assert!(!source_context_matches(&canvas, &package).unwrap());
    export_into(&canvas, &mut package).unwrap();
    assert!(
        package
            .caches()
            .unwrap()
            .iter()
            .all(|row| row.calculation_status.as_deref() != Some("Synchronized"))
    );
}

#[test]
fn same_archive_derived_copy_gets_fresh_native_owner_and_preserves_rich_resources() {
    let (mut canvas, mut package) = fixture();
    let mut copy = canvas.regions[0].clone();
    copy.id = canvas.next_id;
    canvas.next_id += 1;
    copy.source_region = None;
    copy.left = 40.;
    copy.top = 40.;
    canvas.regions.push(copy);
    let old = package.caches().unwrap();
    export_into(&canvas, &mut package).unwrap();
    let owners = package.regions().unwrap();
    let text = owners
        .iter()
        .filter(|r| r.kind == "text")
        .collect::<Vec<_>>();
    assert_eq!(text.len(), 2);
    assert_ne!(text[0].region_id, text[1].region_id);
    assert_eq!(text[1].left, Some(40.));
    assert_eq!(text[1].top, Some(40.));
    let source = package.worksheet_xml().unwrap();
    let doc = parse(&source).unwrap();
    let refs = doc
        .descendants()
        .filter(|n| n.has_tag_name((WS, "text")))
        .map(|n| n.attribute("item-idref").unwrap())
        .collect::<Vec<_>>();
    assert_ne!(refs[0], refs[1]);
    for reference in refs {
        let target = crate::native_visual::resolve_relationship(
            &package,
            package.worksheet_part_name(),
            reference,
            FLOW_REL,
        )
        .unwrap();
        let mut inner = zip::ZipArchive::new(std::io::Cursor::new(
            package.original_part(&target).unwrap(),
        ))
        .unwrap();
        let mut source = String::new();
        inner
            .by_name("Xaml/Document.xaml")
            .unwrap()
            .read_to_string(&mut source)
            .unwrap();
        assert!(source.contains("FontWeight"));
        let mut resource = Vec::new();
        inner
            .by_name("Resources/exact.bin")
            .unwrap()
            .read_to_end(&mut resource)
            .unwrap();
        assert_eq!(resource, [0, 255, 3, 9]);
    }
    // Fresh derived COW happens before context proof. Historical values remain,
    // but this v2 does not invent an inert-copy cache preservation exception.
    let actual = package.caches().unwrap();
    for before in old {
        let after = actual
            .iter()
            .find(|n| n.result_id == before.result_id)
            .unwrap();
        assert_eq!(after.calculation_status.as_deref(), Some("Pending"));
        assert_eq!(after.result_mathml, before.result_mathml);
    }
}

#[test]
fn changed_origin_text_target_rejects_derivative_without_mutating_package() {
    let (mut canvas, mut package) = fixture();
    let mut copy = canvas.regions[0].clone();
    copy.id = canvas.next_id;
    canvas.next_id += 1;
    copy.source_region = None;
    canvas.regions.push(copy);
    let worksheet = package.worksheet_xml().unwrap();
    let doc = parse(&worksheet).unwrap();
    let id = doc
        .descendants()
        .find(|n| n.has_tag_name((WS, "text")))
        .unwrap()
        .attribute("item-idref")
        .unwrap();
    let target = crate::native_visual::resolve_relationship(
        &package,
        package.worksheet_part_name(),
        id,
        FLOW_REL,
    )
    .unwrap();
    package.set_part(&target, vec![1, 2, 3]).unwrap();
    let before = package
        .parts()
        .iter()
        .map(|p| (p.name.clone(), p.bytes().to_vec()))
        .collect::<Vec<_>>();
    assert!(export_into(&canvas, &mut package).is_err());
    assert_eq!(
        before,
        package
            .parts()
            .iter()
            .map(|p| (p.name.clone(), p.bytes().to_vec()))
            .collect::<Vec<_>>()
    );
}

#[test]
fn identical_text_parts_with_different_owner_wrapper_are_not_the_same_authority() {
    let (mut canvas, mut package) = fixture();
    let mut copy = canvas.regions[0].clone();
    copy.id = canvas.next_id;
    canvas.next_id += 1;
    copy.source_region = None;
    canvas.regions.push(copy);
    let source = package.worksheet_xml().unwrap();
    let doc = parse(&source).unwrap();
    let text_owner = doc
        .descendants()
        .find(|n| {
            n.has_tag_name((WS, "region")) && n.children().any(|c| c.has_tag_name((WS, "text")))
        })
        .unwrap();
    let edits = attrs(text_owner, &source, &[("width", Some("333".into()))]).unwrap();
    let part = package.worksheet_part_name().to_owned();
    set_xml(&mut package, &part, apply(source, edits).unwrap()).unwrap();
    let before = package
        .parts()
        .iter()
        .map(|p| (p.name.clone(), p.bytes().to_vec()))
        .collect::<Vec<_>>();
    assert!(export_into(&canvas, &mut package).is_err());
    assert_eq!(
        before,
        package
            .parts()
            .iter()
            .map(|p| (p.name.clone(), p.bytes().to_vec()))
            .collect::<Vec<_>>()
    );
}

#[test]
fn equivalent_text_authority_is_not_rejected_merely_for_different_workbook_math() {
    let (mut canvas, mut package) = fixture();
    let mut copy = canvas.regions[0].clone();
    copy.id = canvas.next_id;
    canvas.next_id += 1;
    copy.source_region = None;
    canvas.regions.push(copy);
    let source = package.worksheet_xml().unwrap();
    let doc = parse(&source).unwrap();
    let value = doc
        .descendants()
        .find(|n| n.has_tag_name((ML, "real")))
        .unwrap();
    let edits = vec![(
        value.range(),
        format!("<ml:real xmlns:ml='{ML}'>99</ml:real>"),
    )];
    let part = package.worksheet_part_name().to_owned();
    set_xml(&mut package, &part, apply(source, edits).unwrap()).unwrap();
    // Text owner/context namespaces and all actual Text authority bytes remain
    // exact. Different unrelated equations do not authorize or forbid copying
    // inert Text; their cache proof remains independently conservative.
    export_into(&canvas, &mut package).unwrap();
    assert_eq!(
        package
            .regions()
            .unwrap()
            .iter()
            .filter(|r| r.kind == "text")
            .count(),
        2
    );
    assert!(
        package
            .caches()
            .unwrap()
            .iter()
            .all(|r| r.calculation_status.as_deref() != Some("Synchronized"))
    );
}

#[test]
fn admission_facade_is_readonly_nonrecursive_and_not_general_document_validation() {
    let (mut canvas, package) = fixture();
    let before = package
        .parts()
        .iter()
        .map(|p| (p.name.clone(), p.bytes().to_vec()))
        .collect::<Vec<_>>();
    // Deliberately invalid unrelated document/cache fields prove this facade
    // does not call CanvasDocument::validate or project/serialize result caches.
    // CanvasDocument's existing caller still rejects these fields separately.
    canvas.width = -1.;
    canvas.calculation_cache.as_mut().unwrap().source_signature = "invalid".into();
    let stats = validate_rich_text_carriers(&canvas, &package).unwrap();
    assert_eq!(stats.carrier_count, 1);
    assert_eq!(stats.plain_text_bytes, PLAIN.len());
    assert!(stats.source_bytes > PLAIN.len());
    assert!(stats.authority_bytes > 0);
    assert_eq!(
        before,
        package
            .parts()
            .iter()
            .map(|p| (p.name.clone(), p.bytes().to_vec()))
            .collect::<Vec<_>>()
    );
}

#[test]
fn admission_accounts_original_noop_carrier_source_instead_of_only_export_changes() {
    let (mut canvas, package) = fixture();
    let owner = canvas.regions[0].source_region.as_deref().unwrap();
    let authority = read_native_text_authority(&package, owner).unwrap();
    let original =
        crate::rich_text_source::ValidatedRichText::from_original(authority, PLAIN).unwrap();
    let bytes = original.xaml().len();
    canvas.regions[0].rich_text = Some(Box::new(original.wire().clone()));
    let stats = validate_rich_text_carriers(&canvas, &package).unwrap();
    assert_eq!(stats.source_bytes, bytes);
    assert_eq!(stats.carrier_count, 1);
    assert_eq!(stats.plain_text_bytes, PLAIN.len());
}

#[test]
fn admission_rejects_missing_authority_nontext_owner_and_region_budget() {
    let (mut canvas, package) = fixture();
    canvas.native_archive = None;
    assert!(validate_rich_text_carriers(&canvas, &package).is_err());
    let (mut canvas, _) = fixture();
    canvas.regions[1].rich_text = canvas.regions[0].rich_text.clone();
    assert!(validate_rich_text_carriers(&canvas, &package).is_err());
    let (mut canvas, _) = fixture();
    let plain = canvas.regions[1].clone();
    while canvas.regions.len() <= 4096 {
        canvas.regions.push(plain.clone());
    }
    assert!(validate_rich_text_carriers(&canvas, &package).is_err());
}

#[test]
fn absent_carrier_admission_is_exact_legacy_fastpath_even_without_native_source() {
    let canvas = CanvasDocument::default();
    let package = minimal_package(&canvas).unwrap();
    assert_eq!(
        validate_rich_text_carriers(&canvas, &package).unwrap(),
        RichTextAdmissionStats::default()
    );
}

// ARTIFACT ONLY: append INSIDE existing export::text_source_tests module,
// reusing its private fixture(). Not a separate sibling module. NOT RUN.
#[test]
fn borrowed_append_matches_materialized_canvas_admission_without_mutation() {
    let (canvas, package) = fixture();
    let before = serde_json::to_vec(&canvas).unwrap();
    let mut candidate = canvas.regions[0].clone();
    candidate.id = canvas.next_id;
    candidate.source_region = None;
    candidate.left += 30.;
    candidate.top += 30.;
    let stats = validate_rich_text_append(&canvas, &package, &candidate).unwrap();
    let mut materialized = canvas.clone(); // ORACLE ONLY, never application path.
    materialized.next_id += 1;
    materialized.regions.push(candidate.clone());
    assert_eq!(
        stats,
        validate_rich_text_carriers(&materialized, &package).unwrap()
    );
    let mut forged = candidate;
    forged.content = CanvasContent::Text("not the validated XAML projection".into());
    assert!(validate_rich_text_append(&canvas, &package, &forged).is_err());
    assert_eq!(serde_json::to_vec(&canvas).unwrap(), before);
}
#[test]
fn borrowed_replacement_matches_full_admission_and_rejects_route_changes() {
    let (canvas, package) = fixture();
    let mut replacement = canvas.regions[0].clone();
    let stats = validate_rich_text_replacement(&canvas, &package, &replacement).unwrap();
    assert_eq!(
        stats,
        validate_rich_text_carriers(&canvas, &package).unwrap()
    );
    replacement.source_region = None;
    assert!(validate_rich_text_replacement(&canvas, &package, &replacement).is_err());
}
#[test]
fn rich_source_headroom_counts_plain_and_xaml_once_each() {
    let (canvas, _) = fixture();
    let region = &canvas.regions[0];
    let CanvasContent::Text(plain) = &region.content else {
        panic!()
    };
    let wire = region.rich_text.as_deref().unwrap();
    let expected =
        8 * 1024 * 1024 - (canvas.source_text_bytes().unwrap() - plain.len() - wire.source_bytes());
    assert_eq!(canvas.text_source_headroom(region.id).unwrap(), expected);
    canvas
        .validate_text_edit_budget(region.id, plain, Some(wire))
        .unwrap();
}

#[path = "text_source_tests/authored_projection_tests.rs"]
mod authored_projection_tests;

#[path = "text_source_tests/native_full_selection_export_tests.rs"]
mod native_full_selection_export_tests;
