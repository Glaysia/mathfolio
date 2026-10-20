use super::*;

fn regions(raw: &[&str]) -> Vec<(u64, f64, f64, String)> {
    raw.iter()
        .enumerate()
        .map(|(i, raw)| (i as u64 + 1, -0., i as f64 * 40., (*raw).into()))
        .collect()
}
fn assert_projected(source: &[(u64, f64, f64, String)]) -> (usize, usize) {
    let mut ordinary = WorksheetXmlCache::default();
    let mut projected = WorksheetXmlCache::default();
    let plain = ordinary.build_prepared(source).unwrap();
    let built = projected.build_prepared_with_trees(source).unwrap();
    assert_eq!(built.source, plain.source);
    assert!(plain.decoded_trees.is_empty());
    assert_eq!(projected.bytes, ordinary.bytes, "no retained tree payload");
    assert_eq!(projected.entries.len(), ordinary.entries.len());
    let dom = roxmltree::Document::parse(&built.source).unwrap();
    for (meta, projection) in built.regions.into_iter().zip(built.decoded_trees) {
        let Some(range) = meta.math else {
            assert!(projection.is_none());
            continue;
        };
        let node = dom
            .descendants()
            .find(|n| n.range() == range && n.has_tag_name((WS, "math")))
            .unwrap();
        assert_eq!(
            projection,
            Some(tree_from_mathml_node(node)),
            "id {}",
            meta.id
        );
    }
    let warm = projected.build_prepared_with_trees(source).unwrap();
    assert!(warm.decoded_trees.iter().all(Option::is_none));
    assert_eq!(warm.source, plain.source);
    (projected.bytes, projected.entries.len())
}

#[test]
fn eager_projection_preserves_units_globals_explicit_contextual_none_and_rich_identifiers() {
    let mut source = regions(&[
        "<ml:eval><ml:id>m</ml:id><ml:unitOverride><ml:id>m</ml:id></ml:unitOverride></ml:eval>",
        "<ml:define><ml:id labels='*'>m</ml:id><ml:real>-0</ml:real></ml:define>",
        "<ml:globalDefine><ml:id>cm</ml:id><ml:real>8</ml:real></ml:globalDefine>",
        "<ml:eval><ml:apply><ml:id label-is-contextual='true' labels='UNIT'>sin</ml:id><ml:id labels='CONSTANT'>π</ml:id></ml:apply><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>",
        "<ml:define><ml:id xmlns:p='http://schemas.microsoft.com/winfx/2006/xaml/presentation' xmlns:t='clr-namespace:Ptc.Wpf;assembly=Ptc.Core'><p:Span>𝑥µ한글<t:Subscript>r😀</t:Subscript></p:Span></ml:id><ml:id labels='VARIABLE' label-is-contextual='false'>cm</ml:id></ml:define>",
    ]);
    for round in 0..3 {
        if round == 1 {
            source[1].2 = -100.;
        }
        if round == 2 {
            source[0].3 = source[0].3.replace(">m<", ">cm<");
        }
        assert_projected(&source);
    }
}

#[test]
fn eager_projection_uses_node_identity_across_column_major_matrix_and_program_body_normalization() {
    let source = regions(&[
        "<ml:define><ml:id>m</ml:id><ml:real>3</ml:real></ml:define>",
        "<ml:define><ml:id>A</ml:id><ml:matrix rows='2' cols='2'><ml:id>m</ml:id><ml:id>s</ml:id><ml:id>kg</ml:id><ml:id>cm</ml:id></ml:matrix></ml:define>",
        "<ml:eval><ml:program><ml:if><ml:test><ml:id>m</ml:id></ml:test><ml:then><ml:id>s</ml:id></ml:then><ml:elseif><ml:test><ml:real>0</ml:real></ml:test><ml:then><ml:program><ml:id>kg</ml:id></ml:program></ml:then></ml:elseif><ml:else><ml:id>cm</ml:id></ml:else></ml:if><ml:tryCatch><ml:id>m</ml:id><ml:program><ml:id>s</ml:id></ml:program></ml:tryCatch></ml:program><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>",
    ]);
    assert_projected(&source);
    let built = WorksheetXmlCache::default()
        .build_prepared_with_trees(&source)
        .unwrap();
    assert!(
        built
            .decoded_trees
            .iter()
            .all(|r| r.as_ref().is_some_and(Result::is_ok))
    );
    let array = built.decoded_trees[1].as_ref().unwrap().as_ref().unwrap();
    let names: Vec<_> = array.children[1]
        .children
        .iter()
        .map(|n| match &n.kind {
            NodeKind::AnnotatedIdentifier { name, .. } => name.as_str(),
            _ => panic!("expected canonical labelled matrix identifier"),
        })
        .collect();
    assert_eq!(names, ["m", "kg", "s", "cm"]);
    let control = built.decoded_trees[2].as_ref().unwrap().as_ref().unwrap();
    assert!(matches!(
        control.children[0].children[1].kind,
        NodeKind::Try
    ));
}

#[test]
fn eager_projection_errors_are_display_results_and_do_not_change_source_acceptance() {
    assert_projected(&regions(&[
        "<ml:define><ml:id>a</ml:id><ml:unsupported/></ml:define>",
        "<ml:eval><ml:real>NaN</ml:real><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>",
        "<ml:eval unsupported='kept'><ml:real>2</ml:real><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>",
        "<ml:eval><ml:id labels='VARIABLE'></ml:id><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>",
        "<spec-table><math><ml:define><ml:id>m</ml:id><ml:apply><ml:scale/><ml:matrix rows='1' cols='1'><ml:real>7</ml:real></ml:matrix><ml:placeholder/></ml:apply></ml:define></math></spec-table>",
    ]));
    let invalid = regions(&["<ml:eval><ml:id unsupported='x'>m</ml:id></ml:eval>"]);
    assert_eq!(
        WorksheetXmlCache::default()
            .build_prepared_with_trees(&invalid)
            .err(),
        worksheet_xml(&invalid).err()
    );
}

#[test]
fn bulk_projection_and_warm_miss_metadata_never_retain_duplicate_trees() {
    let mut source: Vec<_> = (0..4000)
        .map(|i| {
            (
                i + 1,
                0.,
                i as f64 * 40.,
                format!("<ml:define><ml:id>v{i}</ml:id><ml:real>{i}</ml:real></ml:define>"),
            )
        })
        .collect();
    assert_projected(&source);
    let mut cache = WorksheetXmlCache::default();
    cache.build_prepared_with_trees(&source).unwrap();
    source[3].3 = source[3].3.replace("<ml:real>3<", "<ml:real>123<");
    let edited = cache.build_prepared_with_trees(&source).unwrap();
    assert_eq!(
        edited.decoded_trees.iter().filter(|r| r.is_some()).count(),
        1
    );
    assert_eq!(edited.regions[3].id, 4);
    let direct = tree_from_mathml(&edited.source[edited.regions[3].math.clone().unwrap()]);
    assert_eq!(edited.decoded_trees[3], Some(direct));
}

#[test]
fn projection_preserves_ordered_failure_and_cache_side_effects() {
    let valid = "<ml:define><ml:id>x</ml:id><ml:real>2</ml:real></ml:define>";
    let oversized = format!(
        "<ml:define><ml:id>{}</ml:id><ml:real>1</ml:real></ml:define>",
        "x".repeat(MAX_XML / 2)
    );
    let cases = [
        regions(&[valid, "<ml:eval><ml:id bad='1'>x</ml:id></ml:eval>"]),
        regions(&[valid, "<ml:eval>"]),
        vec![(1, 0., 0., valid.into()), (1, 1., 1., valid.into())],
        vec![(1, 0., 0., valid.into()), (2, f64::NAN, 1., valid.into())],
        vec![(1, 0., 0., oversized.clone()), (2, 1., 1., oversized)],
    ];
    for source in cases {
        let mut ordinary = WorksheetXmlCache::default();
        let mut projected = WorksheetXmlCache::default();
        let plain = ordinary.build_prepared(&source).err();
        assert!(plain.is_some());
        assert_eq!(projected.build_prepared_with_trees(&source).err(), plain);
        assert_eq!(projected.bytes, ordinary.bytes);
        assert_eq!(projected.entries.len(), ordinary.entries.len());
        assert_eq!(
            projected.build_prepared_with_trees(&source).err(),
            ordinary.build_prepared(&source).err()
        );
        assert_eq!(projected.bytes, ordinary.bytes);
    }
}

#[test]
#[ignore = "read-only actual Enter captures, not shipped fixtures"]
fn captured_open_enter_projection_matches_every_full_dom_tree_and_retention() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../artifacts/rea/worksheet-preparation-inputs-20261019");
    let mut names: Vec<_> = std::fs::read_dir(root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect();
    names.sort();
    assert_eq!(names.len(), 6);
    for path in names {
        let source: Vec<(u64, f64, f64, String)> =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        let (bytes, entries) = assert_projected(&source);
        println!(
            "PROJECTION_ORACLE {} {} regions {} retained_payload_bytes {} retained_entries",
            path.file_name().unwrap().to_string_lossy(),
            source.len(),
            bytes,
            entries
        );
    }
}
