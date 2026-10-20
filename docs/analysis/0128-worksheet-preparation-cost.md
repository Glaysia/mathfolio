# Worksheet preparation, direct assembly and bounded identifier traces

2026-10-19. r125 is preserved: source archive `0fa555837dead845b3291464d016434ff4d18933d661d228432cf9ad618a61ff`, product harness `bbb9766e65f0c90c451479fbe2fb182bedfdf8bdac7fc24e682709a90f4d03fc`. Read [0115](0115-fused-serialization-cost.md), [0123](0123-signature-emission-cost.md) and [0125](0125-structural-deletion-integration.md) before this investigation. No GUI, screenshots, original runtime, document-signature change, worker migration, eager-open warming or coverage reduction is involved. This is preparation CPU work, not new native semantics or a completion percentage.

## Baseline and complete cost

The unchanged frozen r125 app ran five alternating display-recorder off/on executions:360 actual save/open/input cases,90 recorded dirty-Enter displays, all counter/generation/error/zero-evaluation contracts passing. Source-capture I/O is disabled. Input includes4000 scalar definitions and10016×16 matrices, each with/without saved own cache, plus the two original1015/1016 packages. A separate compat binary adds only cfg(test) clocks around the previously unmeasured typed label adapter. It uses the six existing exact app-Enter captures, reconstructs only byte-matching immutable proofs and alternates cold/warm recorder off/on,120 intervals. Fixture construction, DOM reference checks and output inspection are outside timed regions. This captured builder profile is not another actual fresh-open frame.

The previous generic worksheet collector left typed plan, serialization, definition update, projection and validation inside its gap. They are now explicit phases, with no clocks or recorder branch in non-test builds. The remaining gap includes ordinary call/scope overhead, local destruction and uninstrumented work; it is not labeled all removable work. Separate median columns cannot be added into a median frame.

| Cold builder, ms | 4k/cache | 4k/no-cache | Matrix/cache | Matrix/no-cache | Original1015 | Original1016 |
|---|---:|---:|---:|---:|---:|---:|
| Recorder off whole |15.453|14.899|5.911|5.621|.748|.876|
| Recorder on whole |16.716|16.596|5.630|6.159|.766|.839|
| Typed labelled emission |3.237|3.106|1.473|1.631|.114|.116|
| Output assembly |2.254|2.157|.410|.388|.051|.050|
| Cache payload/store |1.993|1.979|.330|.337|.027|.030|
| Definition context update |1.432|1.304|.033|.035|.024|.024|
| Sparse label plan |1.413|1.350|.463|.491|.095|.105|
| Owned display projection |1.317|1.323|2.106|2.322|.069|.073|
| Projection validation |1.096|1.044|.629|.655|.045|.045|

Actual4k display preparation is approximately4.0–4.4ms with the recorder on: store1.68–1.70ms, geometry capture.65–.79ms, outer traversal.72–.74ms, String→Arc.40–.41ms. These include recorder overhead and are not subtracted from unobserved timings. Matrix projection is an eagerly required separately owned tree; its clones and validation are retained. Removing validation or delaying labels to the viewport would change the contract.

## Separate direct assembly and geometry candidate

The builder labels every fragment first, exactly as before, then reserves a bounded output hint and writes region headers directly into the output String. Coordinate ranges are recorded at emission boundaries instead of allocating a header String and searching its quote delimiters. Numeric formatting, namespace/attribute order, source region order, table branch, math resultRef and the8MiB check after each assembled region are unchanged. The hint is capped; it is not an earlier acceptance check. A test-only copy of the exact r125 builder from its frozen source archive is an independent source/range/error oracle, including the six captured worksheets.

Geometry capture now consumes the temporary raw statement Strings after the builder instead of cloning them. Region IDs retain original order. A new exact-capacity regression found and prevents Vec IntoIter allocation recycling from retaining the larger old tuple allocation: the baseline tuple Vec uses the original len-sized allocation, and excess XML String capacity is removed as the old clone did. Archive/settings/context identity, reordering, source invalidation, coordinate rewriting and old immutable input versions retain their exact guards. The first measured consume prototype predates that capacity correction and is preserved separately; the final product keeps the correction.

The independently measured assembly phase changed4k/cache2.254→1.527ms,4k/no-cache2.157→1.730ms, matrix/cache.410→.222ms, matrix/no-cache.388→.202ms. Those are separate profile runs, not a substitute for the alternating actual-frame comparison below.

| Actual Text, r125→assembly/consume candidate, ms | Enter | Per-run Text+Enter |
|---|---:|---:|
| 4k/cache |34.701→33.813|37.005→36.329|
| 4k/no-cache |34.538→32.328|37.031→34.963|
| Matrix/cache |20.105→19.125|25.809→24.644|
| Matrix/no-cache |20.210→19.619|26.154→25.241|
| Original1015 |2.286→2.262|2.823→2.864|
| Original1016 |2.302→2.172|2.756→2.724|

Five executable pairs preserve360 contracts. Original1015's pair regresses despite a slightly lower Enter; other event increases are retained in the raw comparison. Whole-frame improvement is not claimed for every case. The source-only profiles do not explain original native open/cold-font costs.

## Operation-local identifier-range trace

For a new immutable proof, the same raw single-buffer serializer can record identifier AST child paths and their exact emitted byte ranges. Matrix ranges use its actual `row*cols+col` traversal, not an AST/XML preorder zip. Context-dependent labels still use the same sparse Plan and shared resolver/global prepass. Label emission copies unchanged raw spans and writes only changed identifiers through the existing borrowed identifier writer. Projection and validation run unchanged. The raw source's structural/context/XML-normalization validation order is unchanged; full writer errors remain authoritative.

An opaque strong Arc owner ties the trace to the immutable proof whose raw bytes are also compared against the supplied statement. Another Arc with equal source bytes does not authorize reuse. This is additional operation metadata, not a replacement for any exact source guard. Call/Index or any subtree delegated to the fallible legacy serializer invalidates the entire optional trace; partial traces are never emitted. Unsupported forms, unavailable/budget-exhausted metadata and all cache-hit/fallback paths retain the full writer. The builder's output-budget error still occurs after all fragment labeling/cache effects.

The sorted operation-local Vec, span Vec capacities, boxed paths and Cursor/Pool scratch are accounted within1MiB. The path scratch is a fixed depth64 stack array; the lookup Vec is allocated only when a useful eligible proof is created. Source-cache hits and small/unsupported fragments allocate no trace index. This is owned payload capacity accounting, not allocator overhead, process peak memory or RSS. Existing raw/proof/labelled output storage is not counted as new trace metadata. Traces are consumed and dropped inside this one builder call. TypedMathml, fragment cache and app label cache fields retain no trace, historical worksheet or duplicate tree. Existing source/label cache budgets and retained payload remain unchanged.

## Ungated experiments and rejection boundary

The first same-executable120-sample comparison includes raw validation/serialization, immutable tree clone, proof-map creation and the entire cold builder. It excludes signature, app geometry/display adoption, UI layout and calculator execution. Timers are outside the complete interval; phase recorder is disabled. Every output matches the independent full-DOM source/projection reference.

| Initial trace experiment, ms | Full writer proof+builder | Trace proof+builder | Additional trace payload B |
|---|---:|---:|---:|
| 4k/cache |21.862|19.501|352,512|
| 4k/no-cache |19.690|19.358|352,512|
| Matrix/cache |10.938|9.762|9,312|
| Matrix/no-cache |11.187|10.103|9,312|
| Original1015 |1.169|1.109|20,400|
| Original1016 |1.319|1.357|20,400|

The next frozen experiment uses stack scratch/lazy indexing. Its same-executable120 samples show inconsistency on scalar workloads:

| Stack/lazy experiment, ms | Full writer proof+builder | Trace proof+builder |
|---|---:|---:|
| 4k/cache |20.558|22.374|
| 4k/no-cache |22.669|20.319|
| Matrix/cache |10.496|9.169|
| Matrix/no-cache |10.386|9.437|
| Original1015 |.999|.970|
| Original1016 |.984|.967|

Do not replace the scalar/cache regression with the favorable scalar/no-cache number or attribute it to a known cause. The initial ungated trace's actual-frame comparison against the preserved assembly candidate also fails to establish a consistent scalar gain:

| Actual Text, assembly→ungated trace candidate, ms | Enter | Per-run Text+Enter |
|---|---:|---:|
| 4k/cache |32.775→33.056|35.398→35.687|
| 4k/no-cache |32.202→32.062|34.684→34.617|
| Matrix/cache |20.429→18.149|25.899→23.749|
| Matrix/no-cache |20.393→18.582|26.575→24.240|
| Original1015 |2.379→2.237|2.983→2.800|
| Original1016 |2.162→2.165|2.673→2.737|

All360 counter/generation/error contracts pass in that comparison. Per-run worksheet+display changes are4k/cache16.362→16.219ms, no-cache17.480→15.864ms, matrices6.409→4.914/6.396→4.864ms. Some whole-Enter/pair values still regress. Concurrent root editor/input work exists in candidate executables; they are not pure feature-free product attribution. The first r125 product reference remains preserved.

**Final selection:** small-fragment/scalar traces are excluded. Only validated plain bodies containing a Matrix of at least256cells may produce trace metadata. This is a conservative cost gate justified by the repeatedly favorable16×16 samples, not a native rule or universally optimal cutoff. All other bodies use the original full writer, with no trace index allocation. Normal255/256-cell and nested Group oracles check source/ranges/projection/cache equality. Larger matrices, root/global/Eval labels, ownership mismatch, first errors and metadata exhaustion retain complete fallbacks. The final gate was correctness-tested after the experiments; no additional final-product timing is inferred from the earlier ungated scalar measurements. Final r125→r129 whole actual-open/Enter comparison is owned by integration.

Fragment retained payload in all experiments is exactly1,790,452B for either4k capture,1,152,672B for either matrix capture,155,864/155,866B for the originals. The selected matrix trace adds operation-local metadata only; its preceding stack/lazy prototype accounts9,400B for the captured matrix workload. Trace and cache/proof source ownership does not increase persistent payload.

## Verification and additive evidence

The existing eight typed oracles now exercise the trace-capable API too, comparing source bytes, canonical eager display trees, identifier labels, global/geometry order, XAML/Unicode/subscripts, units/None, unsupported forms, stale proof, depth/normalization rejection, output budget, failures and exact fragment payload against the full-DOM path. Six original captured worksheets pass the expanded full-DOM oracle. Five dedicated trace tests cover strong-owner mismatch, complete Call/Index fallback,1MiB exhaustion with partial-trace discard, zero metadata/index, invalid raw/first-error/output-limit cache effects, and255/256 dense selection. The independent frozen r125 assembly has two source/range/error tests, including the six captured inputs. App edit_commit35 normal tests pass with12 ignored explicit profiles/probes, including F5/draft isolation, open execution0, disabled/manual behavior, geometry invalidation and exact consuming geometry capacity. Compat+app all-target Clippy `-D warnings` passes after a test-only array lint repair. Final OS-wide/native/PDF/print and product comparison are integration gates, not inferred from these counts.

All files under `artifacts/rea/` are additive, preserving previous batches:

- `worksheet-preparation-r129-identities-20261019.json`: final owned-source and frozen executable SHA256 identities.
- `worksheet-preparation-r129-baseline-alternating-20261019.{json,log}` and baseline summary: preserved r125 off/on actual frames.
- `worksheet-preparation-instrumented-before-r129-20261019.exe` (`246a5f5b650136539644016e973c0cacb94be8e2d9cdb6510b81611e399138f0`), `worksheet-preparation-r129-typed-baseline-20261019.log`: cfg(test) typed phase breakdown.
- Assembly compat candidate `5e118d4878de4bb3400315434da9676e09bd2c04edbea8de9f3575c2484924c5`, app `cb6a301ca96dca964450a9830e5c01b887f1d47d7ea2e790353c9b984b0b20f6`; `worksheet-preparation-assembly-open-input-alternating-r129-20261019.{json,log}` and `worksheet-preparation-r129-typed-assembly-20261019.log`.
- Ungated trace compat `373650478707eee9fe2eb84eb512c303325a474fbc265141714a0f997500634c`, app `2f023397abb53f3022b683786f42fa29b9600dbd40c739d32c7948ff7ba5387a`; `worksheet-preparation-r129-trace-complete-profile-20261019.log`, `worksheet-preparation-trace-open-input-alternating-r129-20261019.{json,log}`.
- Stack/lazy ungated compat `0e0b00f84869f7ae55eeec54bb39a5a221679ad4829c1104a3da0478f26345fc`, `worksheet-preparation-r129-final-trace-complete-profile-20261019.log`. Despite its filename this is the ungated prototype; final source gate is recorded by the final source identities.
- `worksheet-preparation-r129-final-assembly-oracles-20261019.log`, `worksheet-preparation-r129-traced-corpus-oracle-20261019.log`, `worksheet-preparation-r129-final-compat-oracles-20261019.log`, `worksheet-preparation-r129-dense-trace-oracles-20261019.log`, `worksheet-preparation-r129-traced-app-oracles-20261019.log`, `worksheet-preparation-r129-final-clippy-pass-20261019.log`.

No16ms whole-frame target, resolved native open/glyph jank, universal speedup or extra feature completion is established by this investigation. Dense trace benefits, scalar rejection and whole-frame adverse cases remain distinct. No preparation work has been hidden in another frame or worker.
