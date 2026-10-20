# Rendering and calculation costs

2026-10-16. Headless source-level measurements and regression tests; no GUI,
screenshots, Mathcad process, COM or CLR execution. The original calculation
command contract remains the static REA contract documented in
[0032](0032-calculation-commands-and-saved-results.md).

The renderer previously reconstructed every visible numeric presentation and
decoded every visible saved-result JSON payload on each frame. It also rebuilt
the formula geometry and glyph runs even when only the caret moved. Holding a
mouse button cloned the entire document into a potential Undo snapshot on every
frame. A jump to a previously unvisited distant page could instantiate all
earlier unmeasured formulas, despite the existing vertical viewport check.

The changes retain formatted results until calculation or formatting changes,
and retain mathematical geometry while the arena, labels, result, font size,
pixel density and font-atlas dimensions stay the same. Caret, selection, hit
testing and input still run on every frame. A source/Undo change includes node
identities in cache comparison, so hits cannot point into a retired arena.
Pointer gestures retain one pre-press Undo state. Known horizontal offscreen
regions are culled; page painting visits only intersecting pages. For unmeasured
source formulas with supported numeric results, conservative structural height
bounds avoid typesetting every preceding formula on a distant-page jump.
Arbitrary symbolic/imported presentations and text still use exact measurement
when no safe height bound is available.

The calculator also contained avoidable copying. A byte-identical worksheet with
the same placement grid now reuses its parsed statements; an updated worksheet
moves retained result payloads and clean binding deltas instead of deep-copying
them. The global prepass clones global statements only. Pending plot retention
uses one ID set, replacing a nested scan through all document regions. Changing
the placement grid invalidates calculations even when the XML itself is
unchanged, since snapped ordering/coordinates may change.

## Measurements

Windows release, CPU-side egui layout/shape construction. These are single
sequential before/after samples, including normal runtime noise; they do not
measure GPU presentation, printer performance or end-to-end perceived latency.
No timing threshold is used as a correctness test.

`benchmark_saved_worksheet_idle_and_pointer_hold` creates 100, 1,000 and 4,000
formula regions with saved 12×12 matrix results, displays the same viewport and
runs 120 full workspace frames. Source and saved values do not execute.

| Regions / workload | Before, ms / 120 frames | After retained layout, ms / 120 frames |
| --- | ---: | ---: |
| 1,000, idle | 205.371 | 122.472 |
| 1,000, pointer held | 201.716 | 105.596 |
| 4,000, idle | 215.696 | 136.718 |
| 4,000, pointer held | 434.245 | 131.548 |

`benchmark_large_matrix_session_update_and_context` defines independent 8×8
matrices, calculates them, updates the same source and snapshots ten calculation
contexts. This isolates calculator costs from rendering.

| Matrix definitions | Operation | Before, ms | After, ms |
| --- | --- | ---: | ---: |
| 100 | full calculation | 1.220 | 0.812 |
| 100 | unchanged source update | 2.760 | 0.039 |
| 1,000 | full calculation | 11.554 | 7.493 |
| 1,000 | unchanged source update | 28.378 | 0.493 |
| 1,000 | ten context snapshots | 15.690 | 14.373 |

Initial parsing is still necessary and remained approximately 24–26 ms for the
1,000-matrix source. Context snapshots still clone computed bindings; this was
not removed or described as a recovered Prime dependency graph.

Local ignored evidence files under `artifacts/rea/`:

- `performance-baseline-20261016.log`
- `performance-after-20261016.log` (presentation/pointer changes, before geometry retention)
- `performance-retained-layout-20261016.log`
- `calculation-performance-baseline-20261016.log`
- `calculation-performance-after-20261016.log`

Reproduce the benchmarks with release Cargo tests and the exact filters above,
followed by `-- --ignored --nocapture`. Correctness regressions cover changing
source/result/font geometry, cached numeric precision/recalculation, horizontal
culling with selected editors, distant-page visibility, placement-grid changes,
and recovery after invalid XML. Workspace validation is recorded separately in
[CONFORMANCE](../CONFORMANCE.md).
