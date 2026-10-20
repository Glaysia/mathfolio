# Exact source signature emission cost

2026-10-19. r121 remains intact: archive `faa6743cf5904e0acabdd604ff3a3b5ecfa5dabae1f6701ecf2a2c8f6aee1fe6`, product actual-open harness `956afee2c8c456d78dccb0e3497724480ddb81e30e31f12198c8b854a81ac495`. This work changes signature emission only. It does not change native import, calculation threading, original behavior, document validation, source coverage, eager display, or first-event preparation. No GUI, screenshot or original runtime was used.

## Cost and scope

The existing app memo already retains an exact region snapshot and its derived JSON. A cold miss performs JSON serialization, SHA feed, payload accounting and a snapshot clone; a hit compares the full source and hashes retained JSON. Directly streaming tiny serde writes into SHA would bypass this useful memo and increase hash-call overhead. The signature must include every original source byte, including native archive replacements of the same length, rather than a weaker pointer/hash identity.

The cfg(test)-only exclusive collector measures management, JSON, hash feed, payload scan, snapshot clone, exact guard, settings and archive/finalization. Disabled recording does not read clocks inside the production path; test scope/TLS checks still exist in the instrumented test binary. Production builds contain no collector clocks. Default event behavior is unchanged. Fixture import/construction, expected signature checks and reporting occur outside measured regions. Two fixtures reproduce 4,000 scalar definitions and 100 16×16 matrices; the other two import the existing 1015/1016 original packages into complete canvas documents. This is source-signature CPU measurement, not a fresh-open frame, GUI, GPU or whole-Enter benchmark.

Initial five alternating off/on rounds showed the cold dominant cost:

| Source | Whole cold, recorder off ms | Whole cold, recorder on ms | JSON ms | Snapshot clone ms | Hash feed ms |
|---|---:|---:|---:|---:|---:|
| 4k scalars |7.370|8.667|5.208|1.552|.870|
| 100 matrices |6.821|6.583|4.059|1.921|.483|
| Original1015 |.334|.363|.220|.062|.036|
| Original1016 |.352|.365|.212|.061|.037|

These are separate medians; their sum is not a median whole run. Recorder overhead, particularly 4k per-region scopes, is material and is not subtracted from product timing. The collector tracks exclusive nested sums and an unaccounted gap for each individual run. It does not establish that all clone cost is removable without weakening the exact guard.

## Bounded emitter

`canvas_signature.rs` emits only the fixed CanvasRegion/PortableTree field names, delimiters and child order. NodeKind, metadata, MathFormat, Option, strings, floats and enums retain serde_json's own serializer and the workspace's `float_roundtrip` feature. Math regions use this path; Plot/Opaque/Text retain the complete old derived serializer. Exhaustive destructuring rejects newly added CanvasRegion/PortableTree fields at compile time. Future serde rename/skip attribute changes still require updating the fixed framing and independent byte oracles; this synchronization risk is explicit.

On an app memo miss, the emitter writes into the same initially128-byte Vec. Existing exact guards, disabled/text pruning, region-order lookup, NaN miss behavior, Plot non-retention, 32MiB payload budget, >10,000-region and allocation-budget fallback, eviction and clone policy remain intact. Settings and fallback serialization stay on the old serde path. No additional source snapshot is retained.

For CanvasDocument's uncached presentation and active signatures, the same emitter writes through a64KiB BufWriter into SHA-256, avoiding both the collected reference Vec and a whole-document JSON Vec. Prefix `mathcraft-calculation-presentation-v1\0`, filtered source order, tuple dimensions/grid and `\0native\0` plus every archive byte are identical. The buffer is transient with64KiB capacity, not retained RSS; for tiny documents it can be larger than the old JSON allocation. The explicit flush targets an infallible SHA writer. Document persistence itself still uses its original derived JSON serializer.

## Isolated same-binary candidates

Ten alternating legacy/candidate region-emission rounds,80 samples, preserve exact JSON bytes and aggregate Vec capacity:

| Source | Derived region JSON ms | Fixed framing ms | JSON length B | Vec capacity B, both |
|---|---:|---:|---:|---:|
| 4k scalars |4.370|2.903|1,206,010|2,048,000|
| 100 matrices |3.817|2.081|1,044,222|1,638,400|
| Original1015 |.235|.193|62,966|93,696|
| Original1016 |.199|.141|62,967|93,696|

A separate160-sample, ten-round alternating/reversed four-candidate probe measures the complete uncached active hash, including SHA prefix, source JSON, optional archive and hexadecimal finalization. Every result matches an independent old Vec/derived-Serialize hash:

| Source | Old derived Vec ms | Fixed framing Vec ms | Fixed framing buffered SHA ms | Derived serde buffered SHA ms |
|---|---:|---:|---:|---:|
| 4k scalars |5.146|3.350|2.083|2.737|
| 100 matrices |4.444|2.677|1.655|2.371|
| Original1015 |.248|.185|.138|.159|
| Original1016 |.246|.185|.134|.159|

The whole-source JSON Vec capacity is2,097,152B for4k,1,048,576B for matrices and65,536B for either original. The selected buffered path uses65,536B for each. This reports transient JSON capacity, not total allocation traffic, peak process memory or RSS. The chosen combination is supported by both serialization and hashing measurements, not a claim that streaming alone eliminates all JSON work.

## Frozen cache before/after

Five alternating executable pairs produce1,200 source-signature samples: each side has five internal off/on rounds × four fixtures × cold/warm/edited states per execution. The table uses25 recorder-off observations per cell. The frozen pre-change instrumented binary preserves old production serialization. Concurrent root feature work exists in these instrumented binaries, so neither is labeled the pure r121 product. Same-binary raw-byte comparisons above isolate the framing change; the final r121→r125 actual-open integration comparison is separate.

| Source | Cold before→after ms | Warm before→after ms | One edited region before→after ms | Retained payload B, both |
|---|---:|---:|---:|---:|
| 4k scalars |7.794→5.999|1.613→1.640|1.549→1.533|4,065,780|
| 100 matrices |6.711→5.129|.762→.815|.796→.828|4,416,490|
| Original1015 |.342→.295|.072→.072|.069→.070|186,381|
| Original1016 |.339→.284|.071→.072|.069→.071|186,382|

Warm and some edited cases regress; their cause is not established. The hit path deliberately stays unchanged. These data do not support a universal faster-input claim. The original fixtures' first region is text, so their edited state does not alter active calculation source. Retained payload accounting is exact equality between the two tested caches, including source snapshots and JSON capacity; it is not RSS.

Recorder-on cold medians identify the changed component:4k JSON5.369→3.841ms, clone1.652→1.649ms, hash feed.909→.892ms; matrix JSON4.202→2.522ms, clone2.014→2.054ms, hash feed.475→.479ms. Separate median columns must not be summed. The remaining exact clone and warm comparison cost is not removed in this batch.

## Correctness and evidence

Five document oracles preserve independent derived bytes for every current NodeKind/BinaryOp, all tree field/child framing, invalid arity serialization, Unicode/JSON control escaping, contextual labels/subscripts, all metadata fields, float-roundtrip/-0/nonfinite null encoding, disabled/explicit-enabled flags, order/deletion/new IDs, settings, Text/Plot/Opaque/spec-table fallback and native archive replacements. A limited failing writer checks identical emitted prefix/error text/IO error kind. Independent legacy Vec+SHA code checks both active and presentation signatures. An inline old persisted-cache JSON golden checks the original literal hashes and unchanged saved cache acceptance/re-serialization; no expected hash is generated by the new emitter.

The app's five existing cache oracles now compare against an explicit independent legacy Vec+SHA recipe. They include exact source and same-length archive changes, -0/NaN/Inf, nested Plot floats, unchanged snapshot reuse, disabled/text pruning, table/raw XML, bounded eviction and fallback/recovery. Eight frozen-baseline/F5 tests protect pending draft isolation, source/table columns and discard-without-old-preparation behavior. Local verification: document75 pass/2 ignored; focused cache5 pass/3 ignored profiles; lazy baseline8 pass; document+app all-targets Clippy `-D warnings` passes. Final OS-wide and actual-open/Enter results belong to the r125 integration record.

Artifacts under `artifacts/rea/` are additive; r121 and previous batch artifacts are untouched:

- `signature-emission-identities-20261019.json`: exact owned-source and frozen-harness SHA256 records.
- `signature-emission-instrumented-before-20261019.exe`: `3eb2c6dd30ad957cc451cd8b2c4e82d0c4bcaa55237a6bc90baa0283bc80d008`.
- `signature-emission-json-prototype-20261019.exe`: `40d03267193eaf3e4ec1c96358e877f19a248b2dac998483fc49671be7ca0034`.
- `signature-emission-connected-framing-20261019.exe`: `9a74a5a6b792d95573c7e99502214025f2003e563ed195eb5da68022145977e1` (cache candidate and four document candidates; public document signature still legacy in this measured executable).
- `signature-emission-baseline-profile-20261019.log`, `signature-emission-json-prototype-20261019.log`, `signature-emission-document-candidates-20261019.log`.
- `measure-signature-emission-cache-20261019.py`, `signature-emission-cache-alternating-20261019.{json,log}`.
- `signature-emission-final-five-oracles-20261019.log`, `signature-emission-document-all-oracles-20261019.log`, `signature-emission-final-cache-oracles-20261019.log`, `signature-emission-final-lazy-baseline-oracles-20261019.log`, `signature-emission-clippy-20261019.log`.

All recorded source and function-only measurements exclude original runtime execution. No work was moved into another UI frame or hidden worker. Whole-Enter savings, non-edit regressions and final product source identity must be judged from the final integration's retained product executables, not by adding these isolated phase medians.
