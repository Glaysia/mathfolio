//! ARTIFACT ONLY: child of canvas::source_bytes after integration.
//! Proposed tests, not compiled/executed by this artifact task.
use super::*;

// Independent old accounting, intentionally does not call node_text_bytes or
// source_text_bytes. Keep its explicit match when production gains node kinds.
fn reference_bytes(doc: &CanvasDocument) -> usize {
    let mut total = doc.title.len();
    for region in &doc.regions {
        total += region.source_region.as_ref().map_or(0, String::len);
        total += region
            .rich_text
            .as_ref()
            .map_or(0, |wire| wire.source_bytes());
        match &region.content {
            CanvasContent::Text(s) => total += s.len(),
            CanvasContent::Opaque { xml, kind } => total += xml.len() + kind.len(),
            CanvasContent::Plot(spec) => {
                total +=
                    spec.source_text_bytes() + spec.x_axis.label.len() + spec.y_axis.label.len()
            }
            CanvasContent::Math(tree) => {
                let mut stack = vec![tree];
                while let Some(n) = stack.pop() {
                    total += match &n.kind {
                        NodeKind::Identifier(s)
                        | NodeKind::Number(s)
                        | NodeKind::StringLiteral(s)
                        | NodeKind::BasedNumber { literal: s, .. }
                        | NodeKind::Imaginary(s) => s.len(),
                        NodeKind::AnnotatedIdentifier { name, labels } => name.len() + labels.len(),
                        NodeKind::NativeIdentifier {
                            name,
                            labels,
                            subscript,
                            ..
                        } => name.len() + labels.len() + subscript.as_ref().map_or(0, String::len),
                        NodeKind::ImaginarySymbol { value, symbol } => value.len() + symbol.len(),
                        _ => 0,
                    };
                    stack.extend(n.children.iter());
                }
            }
        }
    }
    for region in doc
        .page_sections
        .header
        .regions
        .iter()
        .chain(doc.page_sections.footer.regions.iter())
    {
        if let PageBandContent::Text(s) = &region.content {
            total += s.len();
        }
    }
    total
}

#[test]
fn byte_walk_matches_independent_all_source_fields_without_package_access() {
    let mut d = CanvasDocument {
        title: "제목😀".into(),
        ..Default::default()
    };
    let id = d.add_text(0., 0., "표시😀");
    d.regions
        .iter_mut()
        .find(|r| r.id == id)
        .unwrap()
        .source_region = Some("0042".into());
    let id = d.add_math(0., 10.);
    let CanvasContent::Math(t) = &mut d.regions.iter_mut().find(|r| r.id == id).unwrap().content
    else {
        panic!()
    };
    t.kind = NodeKind::StringLiteral("source <&>\"😀".into());
    t.children.clear();
    d.add_opaque(0., 20., "<foreign xmlns='urn:kept'>字</foreign>", "Unknown");
    let id = d.add_plot(0., 30.);
    let CanvasContent::Plot(p) = &mut d.regions.iter_mut().find(|r| r.id == id).unwrap().content
    else {
        panic!()
    };
    p.x_axis.label = "横軸".into();
    p.y_axis.label = "세로😀".into();
    d.add_header_text(0., 0., "머리");
    d.add_footer_text(0., 0., "꼬리😀");
    // Deliberately NOT a ZIP. Byte accounting must neither parse nor clone it.
    d.native_archive = Some(vec![0xa5; 16 * 1024 * 1024]);
    assert_eq!(d.source_text_bytes().unwrap(), reference_bytes(&d));
}

#[test]
fn exact_utf8_headroom_rejects_before_mutation() {
    let mut d = CanvasDocument::default();
    d.title.clear();
    for _ in 0..8 {
        d.add_text(0., 0., "x".repeat(1024 * 1024));
    }
    let id = d.add_text(0., 0., "");
    assert_eq!(d.text_source_headroom(id).unwrap(), 0);
    assert!(d.validate_text_edit_budget(id, "", None).is_ok());
    assert!(d.validate_text_edit_budget(id, "é", None).is_err());
    let CanvasContent::Text(first) = &mut d.regions[0].content else {
        panic!()
    };
    first.truncate(first.len() - 2);
    assert_eq!(d.text_source_headroom(id).unwrap(), 2);
    assert!(d.validate_text_edit_budget(id, "é", None).is_ok());
    assert!(d.validate_text_edit_budget(id, "éa", None).is_err());
    assert_eq!(
        d.regions.last().unwrap().content,
        CanvasContent::Text(String::new())
    );
}

#[test]
fn prospective_commands_keep_ids_and_document_unchanged_on_errors() {
    let mut d = CanvasDocument::default();
    let id = d.add_text(0., 0., "old");
    let before = serde_json::to_vec(&d).unwrap();
    let mut candidate = d.regions[0].clone();
    candidate.id = d.next_id;
    candidate.source_region = None;
    candidate.content = CanvasContent::Text("new".into());
    d.validate_text_append_budget(&candidate).unwrap();
    candidate.id = id;
    assert!(d.validate_text_append_budget(&candidate).is_err());
    candidate.source_region = Some("foreign".into());
    assert!(d.validate_text_replace_budget(&candidate).is_err());
    assert_eq!(serde_json::to_vec(&d).unwrap(), before);
}

#[test]
fn identifier_and_literal_lexemes_match_the_old_counter_exactly() {
    let mut d = CanvasDocument::default();
    d.title.clear();
    let id = d.add_math(0., 0.);
    let kinds = [
        NodeKind::AnnotatedIdentifier {
            name: "a😀".into(),
            labels: "VARIABLE".into(),
        },
        NodeKind::NativeIdentifier {
            name: "x".into(),
            labels: "VARIABLE".into(),
            contextual: Some(false),
            subscript: Some("dc😀".into()),
        },
        NodeKind::StringLiteral("<&>\"😀".into()),
        NodeKind::ImaginarySymbol {
            value: "-1.25".into(),
            symbol: "j".into(),
        },
    ];
    for kind in kinds {
        let CanvasContent::Math(t) =
            &mut d.regions.iter_mut().find(|r| r.id == id).unwrap().content
        else {
            panic!()
        };
        t.kind = kind;
        t.children.clear();
        assert_eq!(d.source_text_bytes().unwrap(), reference_bytes(&d));
    }
}
