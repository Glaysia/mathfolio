# 0142 — Large Area child lookup: r140 instrumented baseline → bounded index candidate

Date: 2026-10-19. No GUI, screenshot, original runtime, or calculation engine execution. Product baseline r140 remains preserved: source archive SHA256 `8063abfd045c0144aa01920bb43b50b463510c36389d4672590014396653eff7`, 11274240 bytes; [source manifest](../../artifacts/rea/area-authoring-integration-20261019/source-manifest.json). The profile baseline is that production code plus the new test-only module, not the already frozen r140 product executable.

## Change and contracts

`AreaEditor` still owns every child in its original vector order. `install` builds a path→vector-index map only after the new XML metadata/child parse succeeds and replaces it with that vector. The only two production reads changed are the native-height math-membership check and inert-child lookup. No child creation, ordering, geometry, cache IDs, eager coverage, label resolution, source, calculation signature, results or history behavior changed. Unsupported/read-only math children remain indexed; repeated region IDs are harmless because complete element paths are the keys.

## Method

Direct `AreaEditor::new` + `AreaEditor::show` in headless egui, with tessellation included. This is not an actual app save/open or GPU benchmark. Synthetic 16/256/1024/2048-child Areas contain all math or 80% math + 20% native text, plus a 1024 collapsed control. Child Y positions exceed the 900px viewport; no culling or lazy processing was added. Supplied values/native visuals, varied font sizes and all child views exercise both former linear searches.

Five alternating recorder-OFF process runs per binary; warm value is each run’s median of three frames, then median across five processes. Cold pair = construction + first UI+tessellation measured and added per run before median. Component medians need not sum to the pair median. One additional observer-ON run per binary checks phase/counter behavior independently; its overhead is not primary timing evidence. 108 total case records: 90 OFF and 18 ON. All pass unchanged-source/all-child/no-events contracts.

## Initial baseline (separate, unpaired)

The initial 54-case baseline established the large-frame problem before the index patch. Its five recorder-OFF runs and one recorder-ON run used the same nine fixtures. These timings are preserved rather than substituted for the alternating before samples below.

| Children | Kind | Construct ms | Cold UI+tess ms | Construct+cold pair ms | Warm UI+tess ms |
|---:|---|---:|---:|---:|---:|
| 16 | math | 0.5829 | 9.8685 | 10.4514 | 0.0708 |
| 16 | mixed | 0.3644 | 9.0436 | 9.3811 | 0.0559 |
| 256 | math | 5.7927 | 14.8890 | 20.7925 | 0.7140 |
| 256 | mixed | 4.9201 | 14.7894 | 19.6733 | 0.7252 |
| 1024 | math | 29.8313 | 30.7119 | 60.5179 | 5.5440 |
| 1024 | mixed | 27.9218 | 27.8787 | 55.7597 | 5.2973 |
| 1024 | collapsed math | 30.0083 | 8.0205 | 38.0986 | 0.1293 |
| 2048 | math | 69.9575 | 60.3637 | 130.3212 | 14.4328 |
| 2048 | mixed | 76.1146 | 52.3056 | 129.2224 | 12.6798 |

## Alternating recorder-OFF milliseconds

| Children | Kind | Construct before→after | Cold UI+tess before→after | Construct+cold pair before→after | Warm UI+tess before→after |
|---:|---|---:|---:|---:|---:|
| 16 | math | 0.5425→0.5730 | 9.8551→9.8965 | 10.3989→10.4695 | 0.0596→0.0578 |
| 16 | mixed | 0.3681→0.3598 | 9.2093→9.1604 | 9.5628→9.5202 | 0.0590→0.0639 |
| 256 | math | 6.2950→5.9662 | 14.9917→15.2337 | 21.0202→21.2530 | 0.7369→0.6492 |
| 256 | mixed | 5.0171→5.0892 | 14.4635→14.4221 | 19.8588→19.9539 | 0.7461→0.6106 |
| 1024 | math | 29.7140→29.8683 | 31.1920→29.6977 | 60.9060→59.8871 | 5.4813→2.7702 |
| 1024 | mixed | 27.9645→28.1313 | 27.6898→25.1513 | 55.2054→53.5664 | 5.4310→2.8903 |
| 1024 | collapsed math | 29.5952→29.7890 | 7.8407→7.6773 | 37.7296→37.2473 | 0.0947→0.1146 |
| 2048 | math | 70.5249→70.5405 | 60.4912→53.6082 | 132.0768→123.9212 | 14.2242→6.4880 |
| 2048 | mixed | 76.7817→77.1236 | 55.3576→45.1345 | 132.1393→122.4953 | 13.5538→6.1485 |

The 2048 math warm pair improves 54.4% (14.2242→6.4880ms); mixed improves 54.6% (13.5538→6.1485ms). This establishes a cost from the two repeated O(N) lookups inside per-child loops. It does not attribute the entire uninstrumented frame gap to lookup. Mandatory font/layout/paint and child-editor construction remain. Construction for 2048 math remains 70.5249→70.5405ms; no constructor win is claimed.

Adverse samples are retained: 16 mixed warm 0.0590→0.0639ms; 1024 collapsed warm 0.0947→0.1146ms; 256 math cold UI+tess 14.9917→15.2337ms and construct+cold 21.0202→21.2530ms. Small timings are noisy; this benchmark does not establish a universal improvement or cutoff.

## Observer evidence, final warm frame (single sample, milliseconds)

| Children/kind | Total before→after | Accounted before→after | Uninstrumented gap before→after | Warm typeset before/after | Layout calls before/after |
|---|---:|---:|---:|---:|---:|
| 1024/math | 5.6696→2.9681 | 1.6845→1.6054 | 3.9851→1.3627 | 0/0 | 1024/1024 |
| 1024/mixed | 5.4718→2.7141 | 1.4376→1.3393 | 4.0342→1.3748 | 0/0 | 820/820 |
| 2048/math | 16.7476→7.2924 | 5.8888→4.1018 | 10.8588→3.1906 | 0/0 | 2048/2048 |
| 2048/mixed | 14.7464→8.0105 | 4.5600→4.3707 | 10.1864→3.6398 | 0/0 | 1639/1639 |

Warm typeset is zero, but every expected math child still enters layout. Cold typeset count equals all math children (except collapsed control, expected 0); collapsed editor still retains all 1024 math children. The single observer sample includes clock overhead; do not subtract it from the OFF medians. Residual gap includes Area loops, egui/framework and other uninstrumented work.

## Retained memory tradeoff

The persistent map adds one owned source-path string and one usize index per math child. On this 64-bit build: 2048 math map capacity 3584, entry-capacity model 3584×32=114688B, key allocations 19370 B, total model 134058 B (~131 KiB); 1024 math 66474 B; 2048 mixed 72846 B. This is O(N) additional retained payload, not RSS. Hash-table control bytes, spare buckets beyond the reported usable capacity, allocator overhead and the map object itself are excluded, and capacity is implementation-dependent. Source install creates/replaces this map; ordinary frames perform no map rebuild. No global unbounded cache was introduced.

## Correctness and scope

- New three normal tests compare the index against an independent linear search for all math/source/absent paths, native-text holes, unsupported/read-only math and duplicate native IDs; source replacement/remove/reorder/restore, failed installation, collapse/expand, live child insertion and active editor tree/caret/selection preservation.
- Existing and new Area filter: 49 passed, 0 failed, 4 ignored. App all-target Clippy with `-D warnings` passes.
- Across 45 paired OFF fixtures, source/count/position metadata match; all 180 cold+warm frame pairs have equal shape/vertex/index counts. This is not a full mesh/colour hash or GPU verification. Exact linear reference equality plus unchanged paint/geometry branches is the main semantic guard.
- r140 archives/frozen product executable are preserved. The earlier unpaired 54-case baseline is separate evidence; only the new alternating 108-case run is used for candidate deltas.
- `install` still reconciles existing children via linear position lookups, and construction reparses XML/creates all editors. These are remaining cold/source-change costs, not removed by this patch. No claims about original runtime internal algorithms are made.

## Identities and artifacts

- before: [area-lookup-before-r140-instrumented-20261019.exe](../../artifacts/rea/area-lookup-before-r140-instrumented-20261019.exe), SHA256 `377d0f576a318dfed1e7714ee6d9f36b276e48647d8c89125407419ee332c100`, 23584256 bytes.
- after: [area-lookup-index-after-20261019.exe](../../artifacts/rea/area-lookup-index-after-20261019.exe), SHA256 `7e25303a4868824f6e701ecb418d057629e4012ff5f6549ec47133a6d2474a96`, 23626240 bytes.
- [area-lookup-baseline-20261019.json](../../artifacts/rea/area-lookup-baseline-20261019.json) SHA256 `c71b287539a86b7943582e8d4e461592590469b430689205527616cfced54cf3`.
- [area-lookup-baseline-20261019.log](../../artifacts/rea/area-lookup-baseline-20261019.log) SHA256 `ff532290e51956c34a77b62c97f2efa2909c33eeab0e888c003c35ce8f0a8b32`.
- [area-lookup-index-alternating-20261019.json](../../artifacts/rea/area-lookup-index-alternating-20261019.json) SHA256 `96acf7a7ad19f5a491f8db8a4c97a00bd1fe8492d22ad0a0d627011c8b2a2df2`.
- [area-lookup-index-alternating-20261019.log](../../artifacts/rea/area-lookup-index-alternating-20261019.log) SHA256 `c1bfd135c01a82d2f4f452ca204b972f576aa4b7d62fa04c9e80a79835e7b55e`.
- [area-lookup-index-differential-20261019.json](../../artifacts/rea/area-lookup-index-differential-20261019.json) SHA256 `396d73029731de63e933b9c9ec7a0b7be243e5610042a73fe0e28a75a5855fb3`.
- [area-lookup-baseline-identities-20261019.json](../../artifacts/rea/area-lookup-baseline-identities-20261019.json) SHA256 `88c6985ae5393ef56fd02f04759ffdd399608e7f8fa8ebe24cc25de3a5805974`.
- [area-lookup-index-identities-20261019.json](../../artifacts/rea/area-lookup-index-identities-20261019.json) SHA256 `f6923e7a88d8abbc5beb097040e4b27212f8f5412d3e3e21057a53d15d8f1847`.
- [measure-area-lookup-cost-20261019.py](../../artifacts/rea/measure-area-lookup-cost-20261019.py) SHA256 `4a7450409a83807a6c447afa4d75909e41dbe06905ec5b08d78ea8de2a154848`.

The first unpaired baseline runner attempt had a marker-prefix extraction error after a passing product test; `area-lookup-baseline-first-prefix-attempt-20261019.log` is retained. Marker extraction was corrected before both successful measurements. No product source was changed for that runner fix.

Source scope: [worksheet_ui/area_ui.rs](../../crates/mathcraft-app/src/worksheet_ui/area_ui.rs) only the field/install/two lookups and cfg(test) index snapshot; new [worksheet_ui/area_lookup_cost.rs](../../crates/mathcraft-app/src/worksheet_ui/area_lookup_cost.rs); one cfg(test) module registration in [worksheet_ui.rs](../../crates/mathcraft-app/src/worksheet_ui.rs). Formula production bytes remain r140 SHA256 `9b1ae6a6984cd9233b0403e681d1c8493ea2a9797e622e74b3a1f6b7b67b45d0`. No calculation/parser/document changes.

Relevant source: [AreaEditor](../../crates/mathcraft-app/src/worksheet_ui/area_ui.rs), [profile and independent lookup tests](../../crates/mathcraft-app/src/worksheet_ui/area_lookup_cost.rs). The baseline and candidate source SHA256 values are recorded separately in their identity artifacts; only the candidate adds the index and its test-only accounting. Later integration source/harness identities must be recorded separately.
