# Page-frame interaction cost

2026-10-19. This batch partitions actual headless open/input CPU, removes unnecessary input snapshots and Enter retypesetting, then independently removes pending-text galley mesh copies. No GUI, screenshot or proprietary runtime was used. All clocks and profiler state are `cfg(test)` only.

## Reference and scope

The pure r105 reference remains `artifacts/rea/try-control-open-input-after-r105-20261019.exe`, SHA256 `d7d2cbecd4a50af429e392ee6e70a5cac2afef7de84f14613a4407d29d6e077a`. The r105 archive remains `c6188ccd11d16f3a250c9b66e285985c23789da2dd85b000938bc098a9301c6d`. Both were preserved.

The existing `actual_open_first_input_profile` uses `Context::run_ui(|ui| app.page_ui(ui))`, rather than eframe's whole chrome/workspace/modal path. It excludes GPU/window/renderer work and, by default, tessellation. The first four fixtures save then open synthetic `.mathcraft` documents (4,000 scalar definitions and 100 matrices of 16×16, with/without their own saved results). The two originals directly open the existing 1015/1016 MCDX files. Thus `open_load` includes native import only for the originals. No original runtime is executed. Open creates zero editors; page frames create only viewport editors. Preparation/evaluation stay zero through focus, navigation and preedit. Dirty Text/Paste/IME Commit prepares once at Enter, with the same generation/error timing contracts.

The new profiler partitions open load/import, native visuals/settings, restore; page input/baseline/history, paper geometry/background, complete region scan, result display cache, native alignment, content, response decorations, post-loop commits, worksheet keyboard; formula initial layout, hit/pointer handling, input, relayout and painting; and egui wrapper work. Its nested scopes subtract child time from parents. Each sample's exclusive phases plus measured gap equal its total; column medians must not be added. The wrapper contains framework work not attributed to page phases, including begin/end-pass, fonts/atlas and output destruction. Later tessellation is an explicit separate phase. Returned JSON construction/printing is outside frame CPU. Instrumentation overhead still perturbs detailed runs.

## Initial distribution

Instrumented baseline `frame-interaction-instrumented-before-20261019.exe`, SHA256 `becd37978264c873d613af11054198c1bd3727182f798d38fcb72f7bb206d117`, includes concurrent development tests but no production interaction optimization. Five alternating recorder-off/on processes: 360 case contracts, 180 detailed opens and 720 detailed page frames, zero errors, all phase sums exact. Frozen r105 is a retained reference, not a third executable rerun in this observer experiment.

Unit ms; median of five ArrowRelease cases. These open and frame column medians are independent.

| input | open | load/import | visuals/settings | restore | first page | Arrow release | following Enter |
|---|---:|---:|---:|---:|---:|---:|---:|
| 4k saved cache | 26.891 | 16.223 | .067 | 10.442 | 9.976 | 1.023 | 2.619 |
| 4k no cache | 14.397 | 9.078 | .059 | 5.049 | 9.874 | 1.102 | 2.469 |
| matrix saved cache | 16.817 | 8.208 | .063 | 8.546 | 15.156 | .413 | 5.174 |
| matrix no cache | 12.567 | 7.874 | .070 | 4.381 | 19.448 | 2.801 | 7.385 |
| original 1015 | 18.799 | 5.989 | 4.218 | 8.499 | 22.649 | .276 | .432 |
| original 1016 | 18.389 | 5.703 | 4.106 | 8.417 | 21.623 | .263 | .470 |

For uncached matrix ArrowRelease, source preparation is zero, but the **following Enter** captures an entire raw committed-source baseline plus a history snapshot (2.842 ms) and retypesets selected formula height in worksheet keyboard handling (1.456 ms). Painting is 1.846 ms in the release frame and 2.181 ms in Enter, compared with .269/.273 ms in the cached fixture. The widget event interval is small; this is not a calculation bottleneck.

Cached 4k IME preedit spends 1.417 ms in baseline/history capture, then another 2.011 ms in following Enter capture, despite neither frame changing source. For original 1016 Text, open is 18.360 ms (load 5.982, visuals 4.062, restore 8.325), and the first page is 22.442 ms, dominated by native alignment/first layout 14.056 ms and egui wrapper 6.323 ms. Its warm event/painting work is much smaller. This locates the cold cost but does not imply those necessary first layouts can simply be removed.

Observer check: uncached matrix release+Enter median is 10.302 ms recorder off and 10.215 ms recorder on; 1016 Text+Enter is 3.889/3.718 ms. Recorder-on is not a fixed overhead percentage. Detailed phases diagnose structure, rather than proving speedup by themselves.

The earlier r101→r105 increases (uncached matrix release+Enter 9.407→10.567 ms and 1016 recorded open-through-Enter 44.792→46.631 ms) are preserved. Archive comparison proves `open`, `import_native`, `page_ui` and native retained-layout functions byte-identical across those two revisions. Formula UI differs in Try pointer/caret arms; these inputs do not exercise Try. This does **not** establish the cause of those old timing deltas, and they are not relabelled as fixed regressions or dismissed as noise. The work below addresses duplicate operations independently demonstrated by current phases and source.

## Input and Enter candidate

1. IME Preedit/Enabled/Disabled changes only composition/widget state and no source. Commit and DeleteSurrounding retain the conservative mutation path.
2. A single unmodified pressed Enter skips baseline/history capture only when an existing selected Math editor owned the previous editing state, has no program context, no new focus request/pointer press/popup, and there is no table draft. Mixed event batches, ShiftEnter, text/program/table Enter and uncertain focus paths retain the old conservative predicate. The actual formula commit and calculation boundary remain unchanged.
3. An unchanged formula can supply its already computed **intrinsic desired size**, before absolute-position float rounding. Enter reuses it only for that same frame/ID/editor revision/format, with no native or resolved-label rendering, and with result-cache equality to the legacy Enter result. Every failed guard falls back to the old measurer. A changed widget never reuses its stale initial rectangle. No geometry is retained across frames, and the app does not approximate height from node counts.

The actual six-fixture Enter oracle has 20 positive reuse cases with bit-identical height to old retypesetting and identical grid-ceiling insertion advance. Other native/labelled/result-mismatch cases fall back. Ordinary oracles cover font points 4/11/36, zoom 1/2 at DPI 2, fractions/powers and fractional region positions; mixed Text+Enter takes the changed/native fallback and Undo restores source. Composition tests preserve source, caret, selection, revision and both histories. Program Enter and matrix ShiftTab/Tab mutation paths remain covered.

Frozen candidate `frame-interaction-input-candidate-after-20261019.exe`, SHA256 `f43b638a9aec2b9fb11f5f64646d87b8cbd9591c07d98e0d1d8133d16720f7bb`, versus the instrumented baseline above. Five alternating recorder-off processes per side: all 360 contracts pass. No opacity optimization is present in this comparison. Unit ms; pair is summed per run before median.

| input/event | stimulus before→after | Enter before→after | stimulus+Enter before→after |
|---|---:|---:|---:|
| uncached matrix ArrowRelease | 2.813→3.428 | 7.488→2.874 | 10.271→6.302 |
| cached matrix ArrowRelease | .416→.402 | 5.146→.386 | 5.574→.795 |
| cached 4k IME preedit | 2.284→.770 | 2.680→.744 | 5.049→1.514 |
| uncached 4k IME preedit | 2.506→.834 | 3.173→.786 | 5.679→1.619 |
| original 1016 Text | .490→.492 | 3.367→3.435 | 3.857→3.954 |

The uncached matrix release alone worsens .615 ms in this sample, while its following Enter and pair improve. Original 1016 Text pair worsens .097 ms. These limits are retained rather than claiming every frame becomes faster.

## Pending text opacity candidate

The uncached matrix has pending result opacity .45. Egui 0.36.2 `Painter::multiply_opacity` calls `epaint::shape_transform::adjust_colors`; Text shapes use `Arc::make_mut` on the galley and each row, then adjust every mesh vertex. Thus cached formula galleys are copied and recolored again for each pending paint. Opacity 1 avoids that work, explaining the large **current** cached/uncached painting difference.

The bounded helper keeps the original painter for lines, fences, placeholders and caret. For a visible nonempty text galley with opacity between zero and one it clones painter state, preserves inherited tint and clipping, resets only painter opacity, and supplies the same factor through `TextShape::opacity_factor`. This preserves shared glyph meshes; opacity multiplication happens during tessellation instead. Zero/one/invisible and other cases use the ordinary path. Disabled tint can still require its own COW work, and no blanket benefit for that path is claimed.

Exact headless tessellation oracles compare vertices (position/UV/color), indices, texture IDs and clip rectangles across DPI 1/2, opacity 0/.45/.2/1, inherited disabled tint, explicit unit/constant/fallback colors, underline/background meshes and ordinary nontext fence shapes. They pass. A separate Arc oracle proves pending text retains the original shared galley. This is same-mesh evidence, not a screenshot comparison.

The isolated opacity baseline already includes the input/height improvements. Before `frame-interaction-opacity-before-20261019.exe` SHA256 `f12b5ed1b1a998643d86ed31ba977885105dabc8da24db76b6eaa2d20902afe2`; after `frame-interaction-opacity-after-20261019.exe` SHA256 `afea40f9e7c638fc842f2afdb17395e5c78d43ec79241da52fc04e5853264e33`. Both execute the same profiler including CPU tessellation after `run_ui`, moving shape buffers rather than cloning them. Five alternating processes per side: 360 contracts, 360 detailed opens and 1,440 page+tessellation frames, all exact sums. No GPU, renderer upload or actual window is measured.

Unit ms:

| fixture, ArrowRelease | page+tess release before→after | page+tess Enter before→after | paired total before→after | release painting before→after | release tessellation before→after |
|---|---:|---:|---:|---:|---:|
| matrix no cache | 3.857→1.605 | 3.529→1.634 | 7.546→3.274 | 1.839→.356 | 1.639→1.047 |
| matrix saved cache | 1.358→1.435 | 1.355→1.321 | 2.756→2.792 | .288→.296 | .921→.941 |

The uncached gain survives combined page+tessellation accounting; it is not merely work shifted out of painting. Reusing shared meshes also avoids destruction of the old per-paint copies. This does not measure RSS or complete peak memory. The unaffected opacity-1 case slightly worsens (.036 ms paired), and no universal acceleration claim is made.

| Text case | page+tess Text+Enter before→after |
|---|---:|
| cached 4k | 44.693→44.977 |
| uncached 4k | 45.434→45.243 |
| cached matrix | 38.801→38.836 |
| uncached matrix | 45.057→39.474 |
| 1015 | 4.198→4.297 |
| 1016 | 4.178→4.226 |

Independent candidates use different frozen harness pairs and sessions. Their median savings must not be added as a controlled final r105→r109 result. Final product integration and a separate frozen-product comparison belong to the parent run.

## Validation and artifacts

At source freeze the complete app suite passes 367 tests / 31 ignored, including existing draft/F5, saved result/error timing, table, program, Undo/Redo and print tests. All-target app Clippy with warnings denied passes. Product and test source are frozen; later formatting/archive/integration is owned by the parent. Old r101/r105/candidate artifacts are unchanged.

- `artifacts/rea/measure-frame-interaction-baseline-20261019.py`, `frame-interaction-baseline-alternating-20261019.{json,log}`, `frame-interaction-baseline-phase-summary-20261019.json`.
- `artifacts/rea/frame-interaction-r101-r105-source-comparison-20261019.{json,diff}`.
- `artifacts/rea/measure-frame-interaction-input-candidate-20261019.py`, `frame-interaction-input-candidate-alternating-20261019.{json,log}`.
- `artifacts/rea/measure-frame-interaction-opacity-20261019.py`, `frame-interaction-opacity-alternating-20261019.{json,log}`, `frame-interaction-opacity-paired-summary-20261019.json`.
- `artifacts/rea/frame-interaction-{input-intent-tests,height-oracle,height-font-tests,opacity-oracle,opacity-sharing,app-tests,clippy}-20261019.log`.
- Test-only `worksheet_ui/frame_profile.rs`; actual lifecycle wrappers in `open_input_latency.rs`; exact mesh tests in `typography/paint_opacity.rs`.

The cold original native first-layout and restore costs remain substantial. This batch identifies their location without claiming to eliminate them, and it does not infer a user-visible completion percentage from these CPU measurements.


## Final frozen r105 to r109 comparison

The subsequent product comparison uses pure r105 `d7d2cbec…6e077a` and final r109 `program-insertion-open-input-after-r109-20261019.exe`, SHA256 `b11903f67869d2a07d420c0aa0c0214a94d40e2e37c99de8962b035ebe080bd0`, bound to source archive `d537366ab6fa8760982709d99f82a0bc9f357ed7f243e0d7411866ffdc1057d3`. Five alternating processes per executable pass all 360 lifecycle/generation/error contracts and 360 preparation-record pairings. It uses the original recorder-off page workflow, without added CPU tessellation. This is separate from the instrumented candidate measurements above.

| actual event, paired stimulus+Enter | r105→r109 ms |
|---|---:|
| matrix no cache, ArrowRelease | 9.936→1.103 |
| matrix saved cache, ArrowRelease | 5.428→.824 |
| 4k saved cache, IME preedit | 4.876→1.567 |
| 4k no cache, IME preedit | 5.169→1.526 |

| Text fixture | first Text ms | Enter ms | paired Text+Enter ms | paired worksheet+display ms |
|---|---:|---:|---:|---:|
| 4k saved cache | 2.214→2.331 | 41.960→43.077 | 44.540→45.179 | 25.089→26.104 |
| 4k no cache | 2.141→2.126 | 41.921→42.763 | 44.149→44.920 | 25.331→25.747 |
| matrix saved cache | 5.054→5.063 | 30.324→30.948 | 35.490→36.085 | 12.530→12.705 |
| matrix no cache | 6.941→5.227 | 34.405→30.959 | 41.716→36.305 | 12.410→12.486 |
| 1015 | .457→.499 | 3.306→3.486 | 3.771→3.975 | 1.942→2.075 |
| 1016 | .468→.451 | 3.352→3.259 | 3.822→3.707 | 2.013→1.939 |

All four primary nonmutating pairs improve materially. Eleven of all 36 pair medians nevertheless increase, all dirty cases: cached4k Paste +1.767 ms / IME Commit +.871 / Text +.639; uncached4k Text +.771 / IME Commit +.617; cached matrix Text +.595 / IME Commit +.220 / Paste +.183; 1015 Text +.204 / Paste +.017; 1016 IME Commit +.043. Their expected Enter preparation count remains one. Preparation-heavy dirty cases have not universally improved. Original 1016 recorded open-through-Enter Text improves 45.385→44.057 ms, while 1015 worsens 45.282→47.371 ms. This is not a general startup-speed claim, and this new run does not retrospectively explain the old r101/r105 delta.

Final evidence, preserving every earlier artifact:

- `artifacts/rea/measure-program-insertion-open-input-20261019.py` and `summarize-program-insertion-open-input-20261019.py`.
- `artifacts/rea/program-insertion-open-input-alternating-20261019.{json,log}`.
- `artifacts/rea/program-insertion-open-input-phase-summary-20261019.{json,log}` contains all six exact phase tables, per-run combined medians, all 11 pair increases and source/binary identities.

No product or test source changed after the archived freeze. Whole workspace, cross-OS, native export, PDF and reference validation remain reported by the parent integration document.
