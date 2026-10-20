# Single-pass XML escape cost

2026-10-19. This helper change is accepted in the developing conditional-branches batch; the final integrated actual-open/Enter comparison is still pending. Source, timing boundaries and regressions below are kept distinct from the released r98 app measurements.

Production change is only `editor_bridge.rs::escape`: retain `String` return type and all call sites, replace five sequential `str::replace` calls with one byte scan that appends untouched UTF-8 spans and the same five entity spellings. The scan recognizes ASCII delimiters, so slice boundaries cannot split a UTF-8 code point. App/editor APIs, validation, label policy, signature, cache and worker protocols are unchanged. The new profile/test module and registration are `cfg(test)` only. No whole-bridge formatter was run.

Before is a frozen **current branch development** compat harness with the old escape and the new test-only module, not the released r98 app executable. Root's conditional parser/serializer work is separate; fixture source/error oracles match across both frozen binaries.

- Before SHA256: `79e55d98046b64c7236992e5eec193417ddacd649533a1bcd96abfa09fdb3009`.
- After SHA256: `6de5c01cc1cb3e709cc9b2b907570723a3fd8fee5c5aa78475c23b23a1f3ef8b`.
- Exact helper text identities and test-module SHA256 are in `artifacts/rea/escape-helper-identities-20261019.json`; helper text uses UTF-8/LF normalization and renames the test reference to `fn escape` for comparison.
- Five process alternations; no GUI/original runtime. Root avoided heavy CPU during measurement.
- Micro: 35 samples ×60,000 calls per helper; mixed ASCII/Unicode/entity corpus.
- Serializer: 35 samples per captured fixture, parsed PortableTrees prepared outside the timed interval. Tables/unsupported raw fragments are excluded from this **serializer-only** benchmark and their exact decode errors are compared too.
- Builder: same six exact app-captured fixture inputs as 0095/0097, 35 samples each cold/warm-same/warm-value-edit. The outcome column uses recorder-disabled measurements, not active phase totals.

## Results

All values ms, medians. Micro production 31.873→8.369; unchanged legacy reference control 29.777→30.012. This micro ratio is not an end-to-end app speedup.

| fixture | cold serializer before→after | cold builder before→after | warm same builder before→after | warm edit builder before→after |
|---|---:|---:|---:|---:|
| 4k cache | 7.833→5.342 | 23.642→22.508 | 5.711→6.174 | 5.658→6.550 |
| 4k no cache | 7.350→5.279 | 23.684→22.229 | 6.208→5.618 | 5.684→6.266 |
| 100×16 matrix cache | 13.322→6.467 | 8.144→8.144 | .441→.405 | .522→.509 |
| 100×16 matrix no cache | 13.719→6.515 | 7.882→8.211 | .437→.450 | .472→.557 |
| 1015 | .696→.484 | 2.107→1.826 | .524→.481 | .515→.473 |
| 1016 | .613→.417 | 1.938→1.864 | .502→.481 | .505→.504 |

Serializer-only improvement is clear in these fixtures, especially the 25,600 numeric matrix cells. Cold scalar builder also improves because its label patch serializer calls escape for 4,000 identifiers. Active phase evidence is label walk 10.250→8.038 ms (cache) /10.216→7.859 (no cache), while bulk wrap/parse is 4.845→4.853 /4.784→4.903. Phase medians are not additive.

Some builder outcomes worsen, notably warm scalar edit and uncached matrix cold. Warm-same hits do not invoke this helper, and warm-edit invokes it only for the changed fragment. Allocator/layout/process-order effects are possible but not proven; these observations are preserved and are not dismissed as noise. The helper has no added work on that hit path, but this experiment alone does not prove a universal non-regression or whole Enter speedup. An integrated actual-open/Enter run in the final source snapshot remains the proper end-to-end check. No repeat measurement was run.

## Exactness

Three ordinary regressions pass before and after: 1,000 adjacent-special/Unicode combinations plus corpus; full identifier label/context/subscript XML golden and round trip; existing numeric/identifier/label/arity serialization errors. The legacy escaping helper remains test-only as the independent byte oracle. Already-escaped-looking input remains double-escaped exactly as before.

Across five alternations there are 140 micro, 420 serializer, and 1,260 builder timing samples. Full serialized bytes for all supported captured trees and six full error strings match before/after; all excluded decode errors also match. Every builder sample checks exact source/metadata against the ordinary oracle. Input/output/retained payload, hit counts and exclusive phase sums also match. Example 4k cold input/output/cache payload: 529,781 /989,018 /1,790,452 bytes before and after.

Files:

- `escape-preparation-alternating-20261019.{json,log}` and `escape-preparation-summary-20261019.log`.
- `escape-preparation-exact-bytes-20261019.json`: baseline full serialized outputs/errors used for all comparisons.
- `measure-escape-preparation-20261019.py`.
- `escape-before-r98-20261019.exe`, `escape-after-candidate-20261019.exe`.
- New test module `crates/mathcraft-compat/src/editor_bridge/escape_profile.rs`.

Existing r98 evidence and all docs/spec are preserved. The separate read-only display/signature candidate notes remain in `post-r98-preparation-analysis-20261019.md`.

Final focused regressions pass 3/3; compat all-target Clippy `-D warnings` passes. Full compat verification is 284 pass /0 fail /6 ignored, recorded in `artifacts/rea/escape-compat-tests-20261019.log`; its count includes the concurrently developed conditional syntax tests. No further production helper change followed the measured candidate.

For reproduction, invoke `artifacts/rea/measure-escape-preparation-20261019.py` only when no other heavy local CPU work is running. It alternates the two frozen EXEs five times and runs both `profile_escape_and_actual_cold_serializer` and `actual_open_worksheet_build_phase_profile`. Each fixture has seven internal repeats per process. Exact input files remain `artifacts/rea/worksheet-preparation-inputs-20261019/*.json`. The script validates full outputs/errors, hashes and sample contracts before publishing the summary.
