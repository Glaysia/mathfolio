//! Independent differential scenarios for the calculation-only label memo.
//! These compare complete XML and observable calculations, not cache internals.
use mathfolio_compat::{
    editor_bridge::{WorksheetXmlCache, worksheet_xml},
    evaluate_worksheet,
};

type Region = (u64, f64, f64, String);

fn id(name: &str) -> String {
    format!("<ml:id>{name}</ml:id>")
}

fn eval(expression: &str) -> String {
    format!("<ml:eval>{expression}<ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>")
}

fn define(name: &str, expression: &str) -> String {
    format!("<ml:define>{name}{expression}</ml:define>")
}

fn exact(cache: &mut WorksheetXmlCache, regions: &[Region]) -> String {
    let actual = cache.build(regions);
    let expected = worksheet_xml(regions);
    assert_eq!(actual, expected);
    actual.unwrap()
}

fn calculated(cache: &mut WorksheetXmlCache, regions: &[Region]) -> String {
    let actual = exact(cache, regions);
    let expected = worksheet_xml(regions).unwrap();
    assert_eq!(
        serde_json::to_value(evaluate_worksheet(&actual).unwrap()).unwrap(),
        serde_json::to_value(evaluate_worksheet(&expected).unwrap()).unwrap()
    );
    actual
}

fn label(xml: &str, region_id: u64, name: &str) -> String {
    let document = roxmltree::Document::parse(xml).unwrap();
    let region_id = region_id.to_string();
    document
        .descendants()
        .find(|node| node.attribute("region-id") == Some(region_id.as_str()))
        .unwrap()
        .descendants()
        .find(|node| node.tag_name().name() == "id" && node.text() == Some(name))
        .unwrap()
        .attribute("labels")
        .unwrap()
        .to_owned()
}

#[test]
fn mixed_shadowing_and_label_edits_keep_call_and_literal_suffix_identities_distinct() {
    let suffix = "<ml:id><p:Span xmlns:p='http://schemas.microsoft.com/winfx/2006/xaml/presentation'>m<ptc:Subscript xmlns:ptc='clr-namespace:Ptc.Wpf;assembly=Ptc.Core'>r</ptc:Subscript></p:Span></ml:id>";
    let mut regions = vec![
        (1, 0., 0., define(&id("m"), "<ml:real>7</ml:real>")),
        (2, 0., 10., define(&id("sin"), "<ml:real>3</ml:real>")),
        (3, 0., 20., define(suffix, "<ml:real>11</ml:real>")),
        (4, 0., 30., define(&id("mr"), "<ml:real>13</ml:real>")),
        (
            5,
            0.,
            40.,
            eval("<ml:apply><ml:id>sin</ml:id><ml:real>0</ml:real></ml:apply>"),
        ),
        (
            6,
            0.,
            50.,
            eval("<ml:id labels='UNIT' label-is-contextual='true'>m</ml:id>"),
        ),
        (
            7,
            0.,
            60.,
            eval("<ml:id labels='UNIT' label-is-contextual='false'>m</ml:id>"),
        ),
        (8, 0., 70., eval(suffix)),
        (9, 0., 80., eval(&id("mr"))),
    ];
    let mut cache = WorksheetXmlCache::default();
    let before = calculated(&mut cache, &regions);
    assert_eq!(label(&before, 5, "sin"), "VARIABLE");
    assert_eq!(label(&before, 6, "m"), "VARIABLE");
    assert_eq!(label(&before, 7, "m"), "UNIT");

    // The same query bytes acquire builtin identities when declarations move
    // below them; the explicit UNIT and both suffix identities stay intact.
    regions[0].2 = 100.;
    regions[1].2 = 110.;
    let moved = calculated(&mut cache, &regions);
    assert_eq!(label(&moved, 5, "sin"), "FUNCTION");
    assert_eq!(label(&moved, 6, "m"), "UNIT");
    assert_eq!(label(&moved, 7, "m"), "UNIT");

    // Reuse an existing region ID for a function whose bound m must shadow
    // the worksheet/unit context, then change only a declaration's label.
    regions[1].2 = 10.;
    regions[1].3 = "<ml:define><ml:function><ml:id>sin</ml:id><ml:boundVars><ml:id labels='UNIT' label-is-contextual='true'>m</ml:id></ml:boundVars></ml:function><ml:apply><ml:plus/><ml:id>m</ml:id><ml:real>1</ml:real></ml:apply></ml:define>".into();
    calculated(&mut cache, &regions);
    regions[0].2 = 0.;
    for declaration_label in ["UNIT", "CONSTANT", "VARIABLE", "FUNCTION"] {
        regions[0].3 = define(
            &format!("<ml:id labels='{declaration_label}'>m</ml:id>"),
            "<ml:real>7</ml:real>",
        );
        let changed = calculated(&mut cache, &regions);
        assert_eq!(label(&changed, 6, "m"), declaration_label);
        assert_eq!(label(&changed, 7, "m"), "UNIT");
    }
    regions.swap(2, 3);
    regions[2].3 = define(&id("mr"), "<ml:real>17</ml:real>");
    calculated(&mut cache, &regions);
}

#[test]
fn global_poison_and_table_disable_transitions_do_not_reuse_a_previous_binding() {
    let table = "<spec-table><math resultRef='41'><ml:define><ml:id>m</ml:id><ml:apply><ml:scale/><ml:matrix rows='1' cols='1'><ml:real>7</ml:real></ml:matrix><ml:placeholder/></ml:apply></ml:define></math></spec-table>";
    let mut regions = vec![
        (10, 0., 0., eval(&id("gvar"))),
        (11, 0., 10., table.into()),
        (12, 0., 20., eval(&id("m"))),
        (13, 0., 30., "<ml:globalDefine><ml:id>gvar</ml:id><ml:real>5</ml:real></ml:globalDefine>".into()),
        // Unknown command metadata is deliberately outside name resolution.
        (14, 0., 40., "<ml:symEval><ml:real>2</ml:real><ml:command><ml:id unsupported='kept'>globalDefine</ml:id></ml:command></ml:symEval>".into()),
    ];
    let mut cache = WorksheetXmlCache::default();
    calculated(&mut cache, &regions);
    regions[3].3 = "<ml:globalDefine><ml:id>gvar</ml:id><ml:unknown/></ml:globalDefine>".into();
    let poisoned = calculated(&mut cache, &regions);
    let rows = evaluate_worksheet(&poisoned).unwrap();
    let query = rows.iter().find(|row| row.region_id == "10").unwrap();
    assert!(query.value.is_none());
    assert_eq!(
        query.error.as_ref().unwrap().previous_region.as_deref(),
        Some("13")
    );
    regions.push((15, 0., -100., regions[3].3.clone()));
    calculated(&mut cache, &regions);
    regions.pop();
    regions[3].3 =
        "<ml:globalDefine><ml:id>gvar</ml:id><ml:real>9</ml:real></ml:globalDefine>".into();
    for flag in ["true", "1", "false", "0"] {
        regions[1].3 = table.replace(
            "<spec-table>",
            &format!("<spec-table disable-calc='{flag}'>"),
        );
        let changed = calculated(&mut cache, &regions);
        assert_eq!(
            label(&changed, 12, "m"),
            if flag == "true" || flag == "1" {
                "UNIT"
            } else {
                "VARIABLE"
            }
        );
    }
    regions.remove(3);
    let no_global = calculated(&mut cache, &regions);
    assert!(evaluate_worksheet(&no_global).unwrap()[0].error.is_some());
}

#[test]
fn rejected_builds_and_cache_budget_resets_preserve_validation_and_later_recovery() {
    let mut cache = WorksheetXmlCache::default();
    let valid = vec![
        (1, 0., 0., define(&id("m"), "<ml:real>7</ml:real>")),
        (2, 0., 10., eval(&id("m"))),
    ];
    let baseline = calculated(&mut cache, &valid);
    for invalid in [
        vec![(1, 0., 0., "<ml:eval>".into())],
        vec![(1, f64::NAN, 0., valid[0].3.clone())],
        vec![
            (1, 0., 0., valid[0].3.clone()),
            (1, 0., 10., valid[1].3.clone()),
        ],
        vec![(
            1,
            0.,
            0.,
            "<ml:eval><ml:id labels='UNIT' label-is-contextual='maybe'>m</ml:id></ml:eval>".into(),
        )],
        vec![(1, 0., 0., " ".repeat(8 * 1024 * 1024 + 1))],
    ] {
        assert!(worksheet_xml(&invalid).is_err());
        assert_eq!(cache.build(&invalid), worksheet_xml(&invalid));
        assert_eq!(calculated(&mut cache, &valid), baseline);
    }
    // Each fragment is below the 8 MiB limit. Nine distinct 2 MiB source and
    // labelled strings require more than the memo's 32 MiB retained payload
    // budget, before the total worksheet budget rejects the complete build.
    let payload = "x".repeat(2 * 1024 * 1024);
    let overflowing: Vec<_> = (20..29)
        .map(|region_id| {
            (
                region_id,
                0.,
                region_id as f64,
                eval(&format!("<ml:str>{payload}</ml:str>")),
            )
        })
        .collect();
    let expected_error = worksheet_xml(&overflowing).unwrap_err();
    assert_eq!(expected_error, "Worksheet exceeds XML budget");
    assert_eq!(cache.build(&overflowing), Err(expected_error));
    assert_eq!(calculated(&mut cache, &valid), baseline);
    assert_eq!(exact(&mut cache, &[]), worksheet_xml(&[]).unwrap());
    assert_eq!(calculated(&mut cache, &valid), baseline);
}
