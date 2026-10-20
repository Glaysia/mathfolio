# Native open and first-page CPU cost

2026-10-19. r117 is preserved: archive `fea698f24e4af0ba087d2399574a554064df6c895aa3c0693a683afad42ab34d`, actual-open harness `df7f803114021833686cd39c7754f2b0644d3a3339bddda672b579f9a256e7b6`. No GUI, screenshots, original runtime, viewport coverage reduction or eager open calculation is used.

## Baseline, production unchanged

The first instrumented harness `native-open-instrumented-before-20261019.exe`, SHA `ecf7dd2833e29ca74760fd5b5f7ff4aec6d049ae54a35e847c552c41b9a8478a`, adds only cfg(test) phase hooks. Root infix work is concurrent; this instrumented binary is not described as the unchanged r117 product binary. The preserved r117 product remains the final comparison reference.

Five alternating recorder-off/on runs cover six fixtures and six first-event modes each:360 cases,180 recorded opens and720 recorded page frames. All preparation/draft/generation/error/zero-evaluation contracts and exclusive phase sums pass. Separate column medians below cannot be added to reconstruct a median frame.

| Exclusive phase, ms | Original1015 | Original1016 |
|---|---:|---:|
| Whole open |18.676|18.428|
| Visual decode |3.086|3.078|
| Legacy notes importer |2.507|2.443|
| Native saved-display conversion |2.135|2.065|
| Native regions inspection |1.964|1.913|
| Import canvas regions |1.666|1.614|
| Import archive validation/decompression |1.312|.993|
| Refresh committed tables |1.187|1.175|
| Visual archive validation/decompression |.953|.962|
| Restore archive validation/decompression |.924|.914|
| Native caches inspection |.846|.862|
| Import worksheet XML/settings |.517|.508|
| Restore worksheet DOM |.473|.465|
| Whole first page |24.114|23.674|
| Ordinary equation alignment |9.063|8.764|
| Egui wrapper |6.773|6.951|
| Native rich paragraph layout |3.724|3.575|
| Native table layout, exclusive children |1.635|1.619|
| Region content |1.261|1.218|
| Rich inline-math layout |.034|.030|

Repeated parsing is real but is not the dominant first-page explanation. The initially suspected rich inline-math duplicate is tiny in this workload. Ordinary equation alignment's9 ms requires further split between editor creation, layout cache lookup, glyph work and non-glyph typesetting before claiming removable work. Cold font work must not be called duplicate merely because its first use occurs inside alignment. Native paragraph/table work remains eagerly and completely represented.

Raw baseline and summaries: `native-open-baseline-alternating-20261019.{json,log}`, `native-open-baseline-summary-20261019.log`. Scope is optimized headless page-frame CPU, excluding process/font configuration/fixture creation, GPU/tessellation and UI-frame gaps; source import and preparation are not calculation. Recorder overhead is reported separately rather than subtracted from products.

## Bounded open-package candidate design

Direct native import already constructs a bounded, validated `NativePackage`. The same archive is reconstructed for visuals/settings and saved native presentations. Keep that package only as a local open operation owner and borrow it through these consumers, leaving the exact original archive bytes on the document. For saved .mathcraft files create the package in the original visual stage and reuse it for restore. No persistent package/DOM cache, weak pointer identity, changed error surface or moved work to another frame is needed. Existing general restore APIs keep their independent fallback; invalid archives retain the original silent visual/presentation failure and pending results. This removes two successful archive validation/decompression passes in direct native open and one in saved native open; the baseline repeated passes are about1.9 ms for the originals, not a promised total-open saving.

A separate possible optimization shares XML inspection between `regions()` and `caches()`, but must preserve both independent Results/error order, source ordering, UTF-16 decode, all XML-part validation, nested/header/footer coverage and exact raw/cache/source fragments. Existing public methods must remain independent differential oracles. Legacy notes import is deliberately unchanged initially: its1000-region and whole-import validation/error policy affects whether all text imports are discarded, so using rich-text output directly could silently change content on fallback.

All product candidates require exact document/native bytes, saved rows/errors/status/pending state, complete visual/table/XAML/color data, page settings/margins/grid, first-input zero evaluation, source coordinate and print snapshot oracles. Saved and absent/stale own caches, imported/current edited tables, malformed/unsupported presentation and package errors must remain covered. Final whole open plus first-page CPU and recorded open-through-Enter must be measured with retained r117 and candidate binaries; a shifted phase or cache warming is not counted as a saving.

## Fine split: first-page work is cold glyph layout

A second test-only split (`native-open-fine-instrumented-before-20261019.exe`, SHA `ddc607fa9a5e78ae909c7925f3bfb23d4d5fbee6e8926badee48e88096ae96da`) isolates ordinary editor creation, retained formula lookup/labels/typesetting/payload, font key and glyph layout. Its single diagnostic run is not a five-sample estimate. Original1015/1016 first pages spend13.546/13.825 ms exclusively in glyph layout,497 calls each. Non-glyph formula typesetting is.212/.213 ms; editor creation.087/.107 ms; font-key work.085/.084 ms. The next focus frame has zero glyph-layout calls because retained layouts are reused. This supports a cold text-layout explanation, rather than a duplicate9 ms editor/AST construction explanation. It does not prove all glyph work is irreducible or isolate rasterization from shaping/font lookup; the measurement boundary is the existing typography layout call. No font warming, delayed label/visual construction or skipped offscreen decode is added.

Fine raw log: `native-open-fine-baseline-20261019.log`. All36 cases and each exclusive phase sum pass. Additional hooks remain cfg(test), with no production clocks.

## Implemented package and table reuse

`import_native` now returns its already validated package beside the canvas document. `open` retains that owner only for the duration of the open operation, borrowing it for visual/settings decoding and native saved presentations. A .mathcraft source creates a package in the original visual stage and reuses it for restore. General restore still has its independent package-reading fallback, including a failed/absent archive. File size, ZIP preflight/member/XML limits, UTF-16 decode, original archive bytes, source ordering, settings/notes logic, cache status/errors and print/display contents are unchanged. Every package part is still validated. No persistent package/DOM cache is introduced; the local decompressed package can overlap signature/restore temporary allocations for longer, so unchanged RSS/peak memory is **not** claimed.

The only table reuse is a successfully decoded table from this same freshly imported native package. The provenance is local to that operation; it is not a persisted flag or weak pointer guard. `import_native` sets each table's default format from `read_worksheet_math_format_defaults(package)`. `NativeVisuals::read_spec_table` reads the same defaults and calls `read_spec_table_with_defaults`. Existing `refresh_table_visual`→`parse_table_visual` uses the same size-attribute precedence and the same `read_spec_table_with_defaults` with the imported region's defaults. Thus a successful initial table already contains the complete final column math, labels, styles/fonts, widths, colors and geometry. It is reused instead of decoding it again. The standalone XML8 MiB cap is checked before reuse. All .mathcraft/current edited/local tables, `specTable` aliases, missing entries and original table errors keep the ordinary refresh path. Old visual-warning accounting remains from the initial complete visual pass, and region-size caches are still cleared on open.

Shared `regions()`/`caches()` parsing and the legacy notes importer were not changed. The former is a separate validation/error-order refactor, while the latter's whole-import failure policy can drop all notes. Keeping them preserves a small independently reviewable patch rather than weakening those contracts for a larger estimated gain.

## Candidate CPU comparison and limits

The candidate frozen executable `native-open-package-candidate-after-20261019.exe` SHA is `489bca951a66254a05403c070449635fc7b3093b9eb9d42675b91e0994648404`. It and the fine-instrumented before executable were alternated five times with recorders disabled,360 total fixture/event contracts. A separate36-case candidate diagnostic checks72 combined before/after open records and288 page partitions against exact phase sums. Candidate diagnostics confirm one import archive construction, no restore construction and near-zero visual archive handling for direct native open; table refresh is.0006/.0012 ms for the two originals. Glyph content remains497 calls on each original first page. These are concurrent infix-development harnesses, not the final r117/r121 product comparison.

Text-case medians, ms. Combined columns sum each sample's intervals before taking its median; independent columns cannot be added.

| Fixture | Open before→candidate | First page before→candidate | Open+first page before→candidate | Recorded open→Enter before→candidate | Text+Enter before→candidate |
|---|---:|---:|---:|---:|---:|
| 4k/cache |27.278→27.702|10.262→10.497|37.494→38.445|72.691→76.307|34.404→35.405|
| 4k/no cache |14.761→14.111|10.742→10.293|25.503→24.380|61.000→59.737|34.677→34.639|
| 100 matrices/cache |17.401→17.627|16.309→16.253|33.053→33.876|58.920→60.097|25.391→25.438|
| 100 matrices/no cache |12.432→12.592|16.158→16.236|28.283→28.828|55.139→56.283|25.851→26.693|
| Original1015 |19.405→15.848|24.969→23.078|44.117→39.047|47.609→41.985|3.267→2.601|
| Original1016 |18.967→15.551|22.727→22.428|41.679→37.979|44.983→40.818|2.613→2.469|

The explicit repeated package/table phases were removed; first-page glyph variation is not attributed to that change. Synthetic fixtures without native archives show regressions, including saved4k and both matrix open+first-page medians.18 of36 first-event+Enter medians and11 of36 open+first-page medians increase. Every increased sample is retained in `native-open-package-candidate-analysis-20261019.json`, alongside source/binary identities and the six rows above. No universal startup/input improvement is claimed. Recorder-off raw runs are `native-open-package-candidate-alternating-20261019.{json,log}`; diagnostic output is `native-open-package-candidate-phases-20261019.log`. The final root-owned r117→r121 comparison remains the authoritative whole-product gate.

After the measured executable, a no-archive branch uses the existing general restore wrapper to keep that production API live under Clippy, and test initialization/formatting was repaired. The direct-native Some(package) path and parsed data are unchanged. `native-open-source-identities-20261019.json` records pre-final-formatter sources; the final pipeline archives and rebuilds its own source. Process setup, fixture creation, font configuration, tessellation/GPU and inter-frame gaps are outside these recorded intervals.

## Semantic and error oracles

A test-only independent pre-change `legacy_open` is retained. Comparisons cover the complete serialized canvas document/original bytes, every native visual Result (including XAML/color/table/column math), saved rows/status/errors/numeric sources, pending regions, page margins/grid/settings, source coordinates, plot identity, zero editor creation on open and zero calculation. Original source files are read-only and checked unchanged. Both originals have107 regions/107 visuals/80 saved rows; direct import and own-save combinations with absent/valid/stale caches and current edited tables pass. Each original contains56 `ml:real` entries in its specification table, so the edited-table oracle modifies actual content.

Normal independent cases cover saved numeric results, disabled rows, same-ID own saved caches, malformed preserved archive/native files and per-column17-point style with19-point worksheet defaults, width181.5, ARGB background, UTF-16/BOM with a different MathML prefix, a malformed matrix/table error and the `specTable` alias fallback. A stale-cache fixture is intentionally serialized directly: the ordinary save API rejects its invalid signature, and that rejection is not bypassed in production. Normal native tests pass; the ignored two-original complete oracle passes. Existing native/print/anchor tests selected with `native_` report50 passed/20 ignored. Root's candidate full app run reports397 passed/0 failed/35 ignored. App all-target Clippy with `-D warnings` passes. Final workspace/OS/reference/PDF validation is reported by the integration owner.
