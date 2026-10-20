//! Opt-in test-only signature phase timings. No production clocks.
use super::*;
use std::{cell::RefCell, time::Instant};
#[derive(Clone, Copy)]
pub(super) enum Phase {
    Manage,
    Json,
    Hash,
    Payload,
    Clone,
    Exact,
    Settings,
    Archive,
}
const NAMES: [&str; 8] = [
    "manage",
    "json",
    "hash_feed",
    "payload_scan",
    "clone_entry",
    "exact_guard",
    "settings",
    "archive_finalize",
];
struct Running {
    phase: Phase,
    start: Instant,
    nested: u128,
}
struct Stats {
    ns: [u128; 8],
    calls: [usize; 8],
    stack: Vec<Running>,
}
thread_local! {static ACTIVE:RefCell<Option<Stats>>=const{RefCell::new(None)};}
pub(super) struct Scope(bool);
pub(super) fn scope(phase: Phase) -> Scope {
    Scope(ACTIVE.with_borrow_mut(|stats| {
        if let Some(stats) = stats {
            stats.stack.push(Running {
                phase,
                start: Instant::now(),
                nested: 0,
            });
            true
        } else {
            false
        }
    }))
}
impl Drop for Scope {
    fn drop(&mut self) {
        if self.0 {
            ACTIVE.with_borrow_mut(|stats| {
                let stats = stats.as_mut().unwrap();
                let run = stats.stack.pop().unwrap();
                let elapsed = run.start.elapsed().as_nanos();
                stats.ns[run.phase as usize] += elapsed - run.nested;
                stats.calls[run.phase as usize] += 1;
                if let Some(parent) = stats.stack.last_mut() {
                    parent.nested += elapsed;
                }
            });
        }
    }
}
fn measured(
    run: impl FnOnce() -> String,
    record: bool,
) -> (String, u128, Option<serde_json::Value>) {
    if record {
        ACTIVE.with_borrow_mut(|stats| {
            assert!(stats.is_none());
            *stats = Some(Stats {
                ns: [0; 8],
                calls: [0; 8],
                stack: Vec::new(),
            });
        });
    }
    let start = Instant::now();
    let signature = run();
    let total = start.elapsed().as_nanos();
    let phases = if record {
        Some(ACTIVE.with_borrow_mut(|stats|{let stats=stats.take().unwrap();assert!(stats.stack.is_empty());let accounted:u128=stats.ns.iter().sum();assert!(accounted<=total);let phases:serde_json::Map<_,_>=NAMES.iter().enumerate().map(|(i,n)|(n.to_string(),serde_json::json!({"ns":stats.ns[i],"calls":stats.calls[i]}))).collect();serde_json::json!({"total_ns":total,"accounted_ns":accounted,"gap_ns":total-accounted,"phases":phases})}))
    } else {
        None
    };
    (signature, total, phases)
}
fn fixture(count: usize, matrix: usize) -> CanvasDocument {
    let mut doc = CanvasDocument::default();
    for i in 0..count {
        doc.add_math(0., i as f64 * 35.);
        let rhs = if matrix == 0 {
            PortableTree {
                kind: NodeKind::Number(i.to_string()),
                children: vec![],
            }
        } else {
            PortableTree {
                kind: NodeKind::Matrix {
                    rows: matrix,
                    cols: matrix,
                    display: None,
                },
                children: (0..matrix * matrix)
                    .map(|i| PortableTree {
                        kind: NodeKind::Number(i.to_string()),
                        children: vec![],
                    })
                    .collect(),
            }
        };
        doc.regions.last_mut().unwrap().content = CanvasContent::Math(PortableTree {
            kind: NodeKind::Binary(mathcraft_editor::BinaryOp::Define),
            children: vec![
                PortableTree {
                    kind: NodeKind::Identifier(format!("v{i}")),
                    children: vec![],
                },
                rhs,
            ],
        });
    }
    doc
}
fn documents() -> Vec<(String, CanvasDocument)> {
    let mut docs = vec![
        ("4k-scalars".into(), fixture(4000, 0)),
        ("100-matrices16".into(), fixture(100, 16)),
    ];
    for name in ["1015_LCC_S", "1016_LCC_S"] {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../artifacts/rea/unit-selected-sources")
            .join(format!("{name}.mcdx"));
        docs.push((
            name.into(),
            crate::worksheet_ui::import_native(&path).unwrap().0,
        ));
    }
    docs
}
#[test]
#[ignore = "CPU-only independent signature cold/warm/edited phase partition"]
fn source_signature_phase_profile() {
    for (name, mut doc) in documents() {
        let original_left = doc.regions[0].left;
        for round in 0..5 {
            for record in if round % 2 == 0 {
                [false, true]
            } else {
                [true, false]
            } {
                doc.regions[0].left = original_left;
                let mut cache = SignatureCache::default();
                for state in ["cold", "warm", "edited"] {
                    if state == "edited" {
                        doc.regions[0].left += doc.grid;
                    }
                    let (signature, total, phases) = measured(|| cache.signature(&doc), record);
                    assert_eq!(signature, doc.active_calculation_signature());
                    println!(
                        "SIGNATURE_PROFILE {}",
                        serde_json::json!({"fixture":name,"round":round,"record":record,"state":state,"total_ns":total,"retained_bytes":cache.retained_bytes(),"serializations":cache.serializations,"phases":phases})
                    );
                }
            }
        }
    }
}

#[test]
#[ignore = "CPU-only derived JSON versus fixed framing candidate; independent bytes"]
fn signature_json_framing_profile() {
    for (name, doc) in documents() {
        for round in 0..10 {
            for candidate in if round % 2 == 0 {
                [false, true]
            } else {
                [true, false]
            } {
                let start = Instant::now();
                let mut total_bytes = 0;
                let mut capacity = 0;
                for region in doc.regions.iter().filter(|r| active(r)) {
                    let json = if candidate {
                        let mut out = Vec::with_capacity(128);
                        mathcraft_document::canvas::write_region_signature_json(region, &mut out)
                            .unwrap();
                        out
                    } else {
                        serde_json::to_vec(region).unwrap()
                    };
                    total_bytes += json.len();
                    capacity += json.capacity();
                    std::hint::black_box(json);
                }
                let ns = start.elapsed().as_nanos();
                println!(
                    "SIGNATURE_JSON_PROFILE {}",
                    serde_json::json!({"fixture":name,"round":round,"candidate":candidate,"ns":ns,"bytes":total_bytes,"json_capacity":capacity})
                );
            }
        }
        for region in &doc.regions {
            let mut bytes = Vec::new();
            mathcraft_document::canvas::write_region_signature_json(region, &mut bytes).unwrap();
            assert_eq!(bytes, serde_json::to_vec(region).unwrap());
        }
        for active_only in [false, true] {
            let mut bytes = Vec::new();
            mathcraft_document::canvas::write_calculation_signature_json(
                &doc,
                active_only,
                &mut bytes,
            )
            .unwrap();
            let regions: Vec<_> = doc
                .regions
                .iter()
                .filter(|r| {
                    !matches!(r.content, CanvasContent::Text(_))
                        && (!active_only || !r.calculation_disabled)
                })
                .collect();
            assert_eq!(
                bytes,
                serde_json::to_vec(&(regions, doc.width, doc.height, doc.grid)).unwrap()
            );
        }
    }
}

#[test]
#[ignore = "CPU-only full document Vec versus buffered SHA emission candidates"]
fn signature_document_emission_profile() {
    struct Sink(Sha256);
    impl Write for Sink {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            self.0.update(b);
            Ok(b.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    for (name, doc) in documents() {
        let regions: Vec<_> = doc.regions.iter().filter(|r| active(r)).collect();
        let mut oracle = Sha256::new();
        oracle.update(b"mathcraft-calculation-presentation-v1\0");
        oracle.update(serde_json::to_vec(&(&regions, doc.width, doc.height, doc.grid)).unwrap());
        if let Some(archive) = &doc.native_archive {
            oracle.update(b"\0native\0");
            oracle.update(archive);
        }
        let expected = format!("{:x}", oracle.finalize());
        for round in 0..10 {
            for mode in if round % 2 == 0 {
                [0, 1, 2, 3]
            } else {
                [3, 2, 1, 0]
            } {
                let start = Instant::now();
                let mut sink = Sink(Sha256::new());
                sink.0.update(b"mathcraft-calculation-presentation-v1\0");
                let allocated_capacity;
                match mode {
                    0 => {
                        let regions: Vec<_> = doc.regions.iter().filter(|r| active(r)).collect();
                        let json = serde_json::to_vec(&(regions, doc.width, doc.height, doc.grid))
                            .unwrap();
                        allocated_capacity = json.capacity();
                        sink.0.update(&json);
                    }
                    1 => {
                        let mut json = Vec::with_capacity(128);
                        mathcraft_document::canvas::write_calculation_signature_json(
                            &doc, true, &mut json,
                        )
                        .unwrap();
                        allocated_capacity = json.capacity();
                        sink.0.update(&json);
                    }
                    2 => {
                        let mut output = std::io::BufWriter::with_capacity(64 * 1024, &mut sink);
                        allocated_capacity = output.capacity();
                        mathcraft_document::canvas::write_calculation_signature_json(
                            &doc,
                            true,
                            &mut output,
                        )
                        .unwrap();
                        output.flush().unwrap();
                    }
                    3 => {
                        let regions: Vec<_> = doc.regions.iter().filter(|r| active(r)).collect();
                        let mut output = std::io::BufWriter::with_capacity(64 * 1024, &mut sink);
                        allocated_capacity = output.capacity();
                        serde_json::to_writer(
                            &mut output,
                            &(regions, doc.width, doc.height, doc.grid),
                        )
                        .unwrap();
                        output.flush().unwrap();
                    }
                    _ => unreachable!(),
                }
                if let Some(archive) = &doc.native_archive {
                    sink.0.update(b"\0native\0");
                    sink.0.update(archive);
                }
                let signature = format!("{:x}", sink.0.finalize());
                let ns = start.elapsed().as_nanos();
                assert_eq!(signature, expected);
                println!(
                    "SIGNATURE_DOCUMENT_PROFILE {}",
                    serde_json::json!({"fixture":name,"round":round,"mode":mode,"ns":ns,"json_capacity":allocated_capacity})
                );
            }
        }
    }
}
