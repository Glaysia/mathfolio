//! Unit-test-only, opt-in CPU instrumentation of the ordinary builder.
//! No module, clock, TLS access, or runtime branch exists in non-test builds.
use super::*;
use std::{cell::RefCell, time::Instant};

#[derive(Clone, Copy)]
pub(in crate::editor_bridge) enum Phase {
    Retain,
    Order,
    Globals,
    Lookup,
    Parse,
    Label,
    Patch,
    Payload,
    Assemble,
    TypedPlan,
    TypedSerialize,
    TypedDefinition,
    TypedProject,
    TypedValidate,
}
const NAMES: [&str; 14] = [
    "retain",
    "order",
    "globals",
    "lookup",
    "parse",
    "label_walk",
    "label_patch",
    "payload",
    "assemble",
    "typed_plan",
    "typed_serialize",
    "typed_definition",
    "typed_project",
    "typed_validate",
];
#[derive(Default)]
struct Profile {
    nanos: [u128; 14],
    calls: [u64; 14],
}
thread_local! {
    static ACTIVE: RefCell<Option<Profile>> = const { RefCell::new(None) };
}
pub(in crate::editor_bridge) struct Scope {
    phase: Phase,
    start: Option<Instant>,
}
pub(in crate::editor_bridge) fn scope(phase: Phase) -> Scope {
    Scope {
        phase,
        start: ACTIVE.with_borrow(|active| active.as_ref().map(|_| Instant::now())),
    }
}
impl Drop for Scope {
    fn drop(&mut self) {
        if let Some(start) = self.start {
            let nanos = start.elapsed().as_nanos();
            ACTIVE.with_borrow_mut(|active| {
                let profile = active.as_mut().expect("timing scope outlived recorder");
                profile.nanos[self.phase as usize] += nanos;
                profile.calls[self.phase as usize] += 1;
            });
        }
    }
}
fn record<T>(run: impl FnOnce() -> T) -> (T, Profile, u128) {
    ACTIVE.with_borrow_mut(|active| {
        assert!(active.is_none(), "nested recorders would mix measurements");
        *active = Some(Profile::default());
    });
    let start = Instant::now();
    let result = run();
    let total = start.elapsed().as_nanos();
    let mut profile = ACTIVE.with_borrow_mut(|active| active.take().unwrap());
    // The label call includes the patch scope. Report exclusive phases so
    // their sum is meaningful; table labelling stays in label_walk as a whole.
    profile.nanos[Phase::Label as usize] -= profile.nanos[Phase::Patch as usize];
    (result, profile, total)
}
type Regions = Vec<(u64, f64, f64, String)>;

fn verify(actual: GeneratedWorksheet, expected: &str) {
    let (source, regions) = actual.into_parts();
    assert_eq!(source, expected);
    let document = roxmltree::Document::parse(&source).unwrap();
    let nodes: Vec<_> = document
        .root_element()
        .first_element_child()
        .unwrap()
        .children()
        .filter(Node::is_element)
        .collect();
    assert_eq!(nodes.len(), regions.len());
    for (node, region) in nodes.into_iter().zip(regions) {
        assert_eq!(
            node.attribute("region-id"),
            Some(region.id.to_string().as_str())
        );
        assert_eq!(
            node.attribute_node("left").unwrap().range_value(),
            region.left
        );
        assert_eq!(
            node.attribute_node("top").unwrap().range_value(),
            region.top
        );
        assert_eq!(
            node.children()
                .find(|n| n.has_tag_name((WS, "math")))
                .map(|n| n.range()),
            region.math
        );
    }
}

#[test]
fn timing_scopes_preserve_cold_warm_reordered_utf8_and_table_bytes_and_ranges() {
    let mut regions: Regions = vec![
        (0, -0., 40., "<ml:eval><ml:id>m</ml:id><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>".into()),
        (u64::MAX, 0., 20., "<ml:globalDefine><ml:id>m</ml:id><ml:real>7</ml:real></ml:globalDefine>".into()),
        (3, 1e100, 60., "<ml:define><ml:id>한글µ</ml:id><ml:real>9</ml:real></ml:define>".into()),
        (4, 2., 80., "<spec-table disable-calc='true'><math><ml:define><ml:id>s</ml:id><ml:apply><ml:scale/><ml:matrix rows='1' cols='1'><ml:real>7</ml:real></ml:matrix><ml:placeholder/></ml:apply></ml:define></math></spec-table>".into()),
    ];
    let mut cache = WorksheetXmlCache::default();
    for round in 0..4 {
        if round == 2 {
            regions[2].2 = -10.;
        }
        if round == 3 {
            regions[1].3 = regions[1].3.replace("<ml:real>7", "<ml:real>8");
        }
        let expected = worksheet_xml(&regions).unwrap();
        let (actual, profile, total) = record(|| cache.build_prepared(&regions));
        verify(actual.unwrap(), &expected);
        assert_eq!(profile.calls[Phase::Order as usize], 1);
        assert_eq!(profile.calls[Phase::Globals as usize], 1);
        assert_eq!(profile.calls[Phase::Assemble as usize], 1);
        assert!(profile.nanos.iter().sum::<u128>() <= total);
        verify(cache.build_prepared(&regions).unwrap(), &expected);
    }
}

#[test]
fn timing_scopes_preserve_failure_and_reset_for_following_build() {
    for regions in [
        vec![(1, f64::NAN, 0., "<ml:eval/>".into())],
        vec![
            (1, 0., 0., "<ml:eval/>".into()),
            (1, 0., 0., "<ml:eval/>".into()),
        ],
        vec![(1, 0., 0., "<ml:eval><ml:id>m</ml:eval>".into())],
        vec![(
            1,
            0.,
            0.,
            "<ml:eval><ml:id labels='VARIABLE' label-is-contextual='invalid'>m</ml:id></ml:eval>"
                .into(),
        )],
    ] {
        let mut cache = WorksheetXmlCache::default();
        let expected = worksheet_xml(&regions).err();
        let (actual, _, _) = record(|| cache.build_prepared(&regions));
        assert_eq!(actual.err(), expected);
        let (actual, _, _) = record(|| cache.build_prepared(&[]));
        verify(actual.unwrap(), &worksheet_xml(&[]).unwrap());
    }
}

fn change_value(regions: &mut Regions) {
    for (_, _, _, xml) in regions {
        if let Some(at) = xml.find("<ml:real>") {
            let at = at + "<ml:real>".len();
            if let Some(end) = xml[at..].find('<') {
                xml.replace_range(at..at + end, "123");
                return;
            }
        }
    }
    panic!("fixture needs a numeric value for warm edit");
}

#[test]
#[ignore = "CPU-only exact app-captured worksheet builder phase profile"]
fn actual_open_worksheet_build_phase_profile() {
    let directory = std::env::var_os("MATHCRAFT_WORKSHEET_CAPTURE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../artifacts/rea/worksheet-preparation-inputs-20261019")
        });
    for name in [
        "4k-scalars-with-cache",
        "4k-scalars-without-cache",
        "100-matrices16-with-cache",
        "100-matrices16-without-cache",
        "1015_LCC_S",
        "1016_LCC_S",
    ] {
        let bytes = std::fs::read(directory.join(format!("{name}.json")))
            .expect("run explicit app capture first");
        let source: Regions = serde_json::from_slice(&bytes).unwrap();
        for mode in ["cold", "warm-same", "warm-value-edit"] {
            let mut input = source.clone();
            if mode == "warm-value-edit" {
                change_value(&mut input);
            }
            let expected = worksheet_xml(&input).unwrap();
            for round in 0..7 {
                let mut plain = WorksheetXmlCache::default();
                let mut timed = WorksheetXmlCache::default();
                if mode != "cold" {
                    plain.build_prepared(&source).unwrap();
                    timed.build_prepared(&source).unwrap();
                }
                let hits_before = timed.hits;
                let unprofiled = |cache: &mut WorksheetXmlCache| {
                    let start = Instant::now();
                    let result = cache.build_prepared(&input);
                    (result, start.elapsed().as_nanos())
                };
                let ((observed, profile, total), (ordinary, ordinary_ns)) = if round % 2 == 0 {
                    let ordinary = unprofiled(&mut plain);
                    (record(|| timed.build_prepared(&input)), ordinary)
                } else {
                    let observed = record(|| timed.build_prepared(&input));
                    (observed, unprofiled(&mut plain))
                };
                let output_bytes = observed.as_ref().unwrap().source.len();
                verify(observed.unwrap(), &expected);
                verify(ordinary.unwrap(), &expected);
                let accounted: u128 = profile.nanos.iter().sum();
                assert!(accounted <= total);
                let phases: serde_json::Map<String, serde_json::Value> = NAMES
                    .iter()
                    .enumerate()
                    .map(|(i, name)| {
                        (
                            (*name).into(),
                            serde_json::json!({"ns":profile.nanos[i], "calls":profile.calls[i]}),
                        )
                    })
                    .collect();
                println!(
                    "WORKSHEET_BUILD {}",
                    serde_json::json!({"name":name,"mode":mode,"round":round,
                    "regions":input.len(),"input_bytes":input.iter().map(|r|r.3.len()).sum::<usize>(),
                    "output_bytes":output_bytes,"total_ns":total,"unprofiled_ns":ordinary_ns,
                    "accounted_ns":accounted,"unaccounted_ns":total-accounted,"phases":phases,
                    "cache_hits":timed.hits-hits_before,"cache_entries":timed.entries.len(),"retained_payload_bytes":timed.bytes})
                );
            }
        }
    }
}

#[test]
#[ignore = "CPU-only complete typed worksheet cold/warm phase profile"]
fn actual_open_typed_worksheet_phase_profile() {
    for (name, source, proofs) in super::typed_tests::captures() {
        let expected = worksheet_xml(&source).unwrap();
        for round in 0..5 {
            for observed in if round % 2 == 0 {
                [false, true]
            } else {
                [true, false]
            } {
                let mut cache = WorksheetXmlCache::default();
                for mode in ["cold", "warm"] {
                    let (result, profile, total) = if observed {
                        let (r, p, t) =
                            record(|| cache.build_prepared_with_typed_trees(&source, &proofs));
                        (r, Some(p), t)
                    } else {
                        let start = Instant::now();
                        let result = cache.build_prepared_with_typed_trees(&source, &proofs);
                        (result, None, start.elapsed().as_nanos())
                    };
                    let result = result.unwrap();
                    assert_eq!(result.source, expected);
                    let phases = profile.as_ref().map(|p| {
                        NAMES
                            .iter()
                            .enumerate()
                            .map(|(i, n)| {
                                (
                                    n.to_string(),
                                    serde_json::json!({"ns":p.nanos[i],"calls":p.calls[i]}),
                                )
                            })
                            .collect::<serde_json::Map<_, _>>()
                    });
                    let accounted = profile.as_ref().map(|p| p.nanos.iter().sum::<u128>());
                    if let Some(ns) = accounted {
                        assert!(ns <= total);
                    }
                    println!(
                        "TYPED_WORKSHEET_PHASE {}",
                        serde_json::json!({"fixture":name,"round":round,"observed":observed,"mode":mode,"total_ns":total,"accounted_ns":accounted,"phases":phases,"retained_bytes":cache.bytes,"regions":source.len(),"source_bytes":result.source.len()})
                    );
                }
            }
        }
    }
}
