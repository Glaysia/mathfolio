# Display preparation cost

2026-10-19. Initial `cfg(test)` phase instrumentation identified duplicate full-worksheet XML parse and display-tree decode. The accepted candidate reuses the builder's parsed contexts for eager display trees. Geometry capture and label-source String storage remain unchanged. GUI and the original runtime were not used.

The retained product reference is r101 archive `c8c15d671a9728e3f0dfa162c1c386c67c8d7096b22109e6f85662cc66b9e078` and app harness `0808f7bf04d0891a976a583d8b4972875b6071b2eaf533e97fcf3761bdc53cf7`. The instrumented baseline below is a later development harness including concurrent Try support; it is **not** the pure r101 binary. Its preparation production behavior is unchanged and these latency fixtures contain no Try statements.

## Instrumentation and baseline

The frozen instrumented harness is `artifacts/rea/display-preparation-instrumented-before-20261019.exe`, SHA256 `84415e04cd261f95273198e5481c12bbfc00ec85149bde6ca8c8a76a5d5daca8`. The ordinary actual-open profile runs with the recorder off. An ignored wrapper enables detailed clocks while calling the very same save/fresh-open/first-input/Enter profile. Fixture generation is outside detailed recording; only preparation in dirty Enter frames is captured. Viewport-only editors, saved-cache/no-cache fixtures, originals 1015/1016, draft/generation and zero-calculation-before-Enter contracts are retained.

Five alternating recorder-off/on processes each produce 36 cases. All 360 case contracts passed; all 90 detailed display frames have exact per-sample exclusive phase sums plus gap equal to total. The independent cold/warm/geometry label-tree oracle also passed. The clocks/TLS/module are absent from non-test builds.

Phase boundaries:

- `arc`: owned source String to Arc<str>.
- `geometry`: discard old geometry, collect label IDs container, capture native archive, clone raw math Strings, revalidate/sort order, retain coordinate ranges/source.
- `dom`: full generated worksheet XML parse only.
- `traversal`: DOM or emitted-range traversal and ID bookkeeping, excluding nested check/decode/store.
- `check`: exact previous labelled-statement bytes and existing label-tree lookup.
- `decode`: `tree_from_mathml[_node]`, including PortableTree parsing and validation.
- `store`: labelled tree/map insert or remove and owned statement String storage.
- `prune`: remove obsolete/disabled/failed entries.
- gap: remaining work, DOM destruction and the observer's own bookkeeping. Detailed total stops before its JSON/log printing; the broad PREP/Enter frame does include that print overhead in recorder-on runs.

## Measured phases

Unit ms, five Text-case samples per fixture. Column medians must not be added. Recorder-on timings describe the cost distribution; recorder-off frames remain the baseline for later production comparisons.

| input | display total | Arc | geometry | DOM | decode | store | traversal | check | prune | gap |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 4k saved cache | 14.444 | .348 | .761 | 5.371 | 4.139 | 1.707 | 1.101 | .210 | .155 | .280 |
| 4k no cache | 14.948 | .315 | .757 | 5.500 | 4.344 | 1.783 | 1.106 | .220 | .219 | .345 |
| 100×16 matrix saved cache | 9.872 | .079 | .057 | 4.241 | 4.721 | .201 | .326 | .006 | .004 | .244 |
| 100×16 matrix no cache | 10.049 | .182 | .035 | 4.243 | 4.773 | .202 | .331 | .006 | .004 | .200 |
| original 1015 | .805 | .021 | .020 | .385 | .268 | .045 | .035 | .004 | .003 | .007 |
| original 1016 | .763 | .019 | .018 | .381 | .281 | .029 | .035 | .004 | .003 | .008 |

| input | Enter recorder off | Enter recorder on |
|---|---:|---:|
| 4k saved cache | 47.898 | 49.933 |
| 4k no cache | 49.867 | 50.617 |
| matrix saved cache | 37.682 | 35.971 |
| matrix no cache | 40.527 | 39.960 |
| 1015 | 3.908 | 3.973 |
| 1016 | 3.747 | 4.083 |

Recorder-on is not consistently slower for small/noisy samples, and its many per-region clocks and JSON output perturb execution. This off/on table is an observer check, not an optimization result. Neither total app startup nor GPU/window latency is measured. This development harness is not compared to another session's r101 time as a controlled speedup.

## Implication before a production patch

The dominant 4k display work is full DOM parse plus decode/validate, about 9.5 ms in this observer. Matrix has about 9 ms in the same two stages. Geometry copying is only about .76 ms in 4k and labelled retention about 1.75 ms; those intervals also contain necessary sorting/map insertion, so eliminating String copies cannot remove either entire interval or all 14 ms.

Two bounded candidates remain structurally valid but their likely saving is roughly millisecond scale, not 14 ms:

1. Compute final region IDs before capture, then consume the already-owned math Strings into GeometryPreparation. Keep the byte-for-byte archive/settings/order/statement guards and solveblock fallback. Measure retained capacities, since moving an opaque String can retain different spare capacity from cloning it.
2. Store each exact labelled statement as a source Arc plus range. Rebase **every successful entry** to the current source on each full rebuild and prune obsolete/disabled/failure entries. Leaving unchanged entries on arbitrary old Arcs can retain thousands of whole worksheet versions. Preserve sequential nested duplicate-ID behavior for solveblocks, compare against the full DOM oracle, and retain full-document labels for canvas and print.

No such patch has been applied at this baseline stage. A direct-child region traversal using trusted generated IDs may also remove part of the ~1.1 ms traversal in ordinary worksheets, with existing nested DOM fallback for solveblocks; it needs measurement. Removing validation, using viewport-only labels or moving all preparation to a worker is not justified by these measurements alone.

## Evidence

- `artifacts/rea/measure-display-preparation-baseline-20261019.py`.
- `artifacts/rea/display-preparation-baseline-alternating-20261019.{json,log}`.
- `artifacts/rea/display-preparation-baseline-summary-20261019.log`.
- Test-only `worksheet_ui/display_profile.rs` and existing input-latency full DOM/uncached source oracles.

Existing r101 archives, source manifests, frozen harnesses and measurements were preserved.


## Accepted parsed-context projection

`build_prepared_with_trees` requests optional per-region display results from the existing fragment labelling pass. An explicit private `Parser` uses a map keyed by the raw document's `NodeId` for precisely the identifiers that the labeller serializes. Its canonical kinds match decoding the generated XML: an unsubscripted patched identifier is `AnnotatedIdentifier`, and a subscripted one is `NativeIdentifier` with no contextual flag. Matrix XML column order, program BODY normalization and recursive scope are handled by the parser, without matching nodes by traversal position. Parsing and `PortableTree::validate` still run eagerly for every cache miss; they run against the already parsed fragment, rather than a second whole-worksheet DOM.

The app moves each successful tree into its existing labelled-tree map. A projected error remains a display decode result: it removes the display entry and does not become a calculation fault. The serialized source, exact labelled-source guard, error order, global prepass, coordinate handling, archive/settings signatures and worker submission/cancellation contracts retain their previous paths. The `Fragment` cache stores no trees. Cache hits provide no projection and use the existing exact app source cache or ordinary decoder fallback. A missing app display cache with all worksheet hits still takes the full DOM path. Solveblocks keep the metadata-only builder and complete post-insertion DOM path, including nested IDs. Spec-table fragments do not project an ordinary math tree. Labels remain eager for the complete worksheet and print; no viewport-dependent label policy was introduced.

Legacy `build`, `build_prepared`, `worksheet_xml` and `into_parts` keep their bytes and behavior. The optional vector is separate from `GeneratedRegion`, so existing metadata elements do not become larger and old callers allocate no tree vector or identifier override map. Existing acceptance budgets bound source/region/depth as before; this introduces no new retained tree cache. The temporary projection vector coexists with XML assembly and is consumed by the app. Its payload is additional transient ownership at that stage, not a demonstrated RSS reduction or a complete peak-allocation measurement.

## Candidate comparison, recorder off

Frozen candidate harness `artifacts/rea/display-preparation-projection-candidate-20261019.exe`, SHA256 `dd1450beadf7e33af841db6aa07d5f9eb82e9c1c543da1b241c36d37b14ae45f`, versus the instrumented development baseline `84415e04…aca8` above. Five alternating runs of each execute the ordinary profile with detailed display recording off. Five candidate-only recorder-on runs collect the moved-work distribution. These are the same six real save/fresh-open inputs and six event modes per process. All 540 case contracts passed, with zero global errors; all 90 detailed frame sums are exact. Nondirty modes still prepare zero times and dirty modes prepare once only at Enter. This is a CPU headless development comparison, not the final r101-to-r105 product comparison.

Unit ms, median of five Text cases per side. Text+Enter and worksheet+display are summed **within each run before taking the median**, not by adding column medians.

| input | first Text before→after | Enter before→after | Text+Enter before→after | worksheet+display before→after |
|---|---:|---:|---:|---:|
| 4k saved cache | 2.543→2.394 | 49.181→42.007 | 51.655→44.401 | 32.157→25.723 |
| 4k no cache | 2.376→2.286 | 47.046→42.835 | 49.545→45.121 | 30.022→25.394 |
| matrix saved cache | 4.968→5.076 | 36.021→31.682 | 41.226→36.586 | 17.294→12.557 |
| matrix no cache | 7.529→7.009 | 39.209→33.693 | 46.738→40.702 | 16.848→12.241 |
| 1015 | .468→.529 | 3.826→3.513 | 4.313→3.952 | 2.398→2.071 |
| 1016 | .462→.438 | 3.603→3.275 | 4.131→3.713 | 2.235→1.925 |

| input | worksheet before→after | display before→after | preparation total before→after |
|---|---:|---:|---:|
| 4k saved cache | 18.526→22.340 | 13.282→3.135 | 47.052→40.056 |
| 4k no cache | 17.085→22.199 | 12.937→3.214 | 45.163→40.705 |
| matrix saved cache | 7.593→12.261 | 9.985→.286 | 32.683→28.856 |
| matrix no cache | 7.220→12.058 | 9.553→.183 | 32.642→27.643 |
| 1015 | 1.647→2.002 | .746→.076 | 3.391→3.107 |
| 1016 | 1.535→1.873 | .667→.056 | 3.185→2.890 |

The decode work moved into worksheet preparation, so the display-only decrease overstates the saving. The paired worksheet+display reduction is 4.628–6.434 ms for 4k, 4.607–4.737 ms for the matrix fixtures and .310–.327 ms for originals. Whole Enter falls in all six Text rows, while first Text rises .108 ms for cached matrix and .061 ms for 1015. Ten of all 36 stimulus+Enter pair medians rise: cached matrix Arrow press/release +.687/+.154 ms; uncached matrix Arrow press +.681 ms; 1015 Arrow press/release and IME preedit +.018/+.085/+.099 ms; 1016 Arrow release +.029 ms; cached 4k IME preedit +.701 ms; uncached 4k Arrow press/release +.044/+.163 ms. All ten are nondirty, with no preparation or calculation. Their cause is not established; this result does not show every event or overall startup becoming faster.

Candidate recorder-on Text display totals are 3.964/4.132 ms for 4k, .457/.324 ms for matrix and .118/.104 ms for originals. DOM calls/time are zero. The decode interval now measures moving the prepared result (.121/.121 ms for 4k), not the actual parse/validation moved into worksheet. Tree payload accounting is recorder-only and contributes to the detailed gap and broad display/Enter time; old baseline lacked this accounting, so recorder-on is diagnostic rather than the primary speedup evidence.

## Exactness and ownership evidence

| actual input | projected trees | transient projected owned bytes | retained worksheet payload before = after | retained entries |
|---|---:|---:|---:|---:|
| 4k, either saved-cache state | 4,000 | 2,145,781 | 1,790,452 | 4,000 |
| matrix, either saved-cache state | 100 | 2,781,291 | 1,152,672 | 100 |
| 1015 | 79 | 122,462 | 155,864 | 79 |
| 1016 | 79 | 122,463 | 155,866 | 79 |

Transient owned bytes count projection slots, child-vector capacities, string capacities and error strings. They exclude allocator overhead, temporary override maps/DOM/XML buffers and any other app state; these are neither peak bytes nor RSS. Successful trees become the app's existing display-tree ownership rather than an additional retained cache. The one table in each original has no ordinary math projection. The 32 MiB worksheet cache policy and exact retained payload are unchanged.

The new projection oracles compare every `Result<PortableTree, String>` to `tree_from_mathml_node` on the final generated full DOM, along with exact XML bytes, warm output and retained cache bytes. They cover the six captured actual Enter inputs, UTF-8/non-BMP names, explicit/contextual/None labels, units/globals and geometric reorder, rich subscripts, column-major matrix decoding, conditional and Try BODY normalization, unsupported structures, invalid structural trees and attributes. Failure tests preserve malformed XML, duplicate ID, NaN coordinate, output-budget error text and cache side effects. App oracles cover edited/moved/disabled/failed/pruned rows, old-source F5, saved results/errors, table/solveblock fallback and missing display-cache fallback. No proprietary runtime was executed.

Validation at candidate source freeze: full compat package 300 passed / 7 ignored; app edit_commit 31 passed / 6 ignored; captured-input projection oracle 1 passed; app+compat all-target Clippy with warnings denied passed. After the measured frozen candidate only the corpus/error tests and Clippy-equivalent `let`/`size_of_val` cleanup were added; final archived product integration and r101 comparison are separate parent-run evidence.

Additional artifacts:

- `artifacts/rea/measure-display-projection-candidate-20261019.py`.
- `artifacts/rea/display-projection-candidate-alternating-20261019.{json,log}` and `display-projection-candidate-summary-20261019.log`.
- `artifacts/rea/display-projection-{compat-full-tests,captured-oracle,app-final-tests,clippy}-20261019.log`.
- `editor_bridge/worksheet_projection_tests.rs`, `worksheet_ui/display_profile.rs` and existing complete-DOM/lazy-baseline tests.
- Prepared final runner `artifacts/rea/measure-try-control-open-input-20261019.py` keeps pure r101 harness `0808f7bf…53cf7` as its separate baseline; it is not the instrumented baseline used here.
