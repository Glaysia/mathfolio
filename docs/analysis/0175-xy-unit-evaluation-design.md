# XY unit evaluation: proposed bounded design

2026-10-19. Read-only review of current Mathcraft sources during r172 validation. This is an implementation proposal, not a report of implemented behavior. No product files, builds, original runtime or GUI were used. [0170](0170-native-xy-unit-scaling-contract.md) supplies the original static/help contract; this note specifies independent Rust policy where that contract is incomplete. The [source audit](../../artifacts/rea/xy-unit-evaluation-source-audit-20261019.json) records hashes of15 reviewed source files.

## Existing path and the unit-loss points

| Source | Current behavior and consequence |
|---|---|
| app `plots.rs::expression_tree/evaluation` | Plain core Expr becomes an editor tree; an optional named sample replacement is only f64 and emits Number. Unitful X would lose its dimension during pointwise Y evaluation. |
| `plots.rs::query` | CalculationContext queries computed bindings without reexecuting prerequisite worksheet definitions; returns physical Value. Keep that environment boundary. |
| `plots.rs::numbers/real_number` | Accept Scalar/Matrix/RangeValues/Range, select the first matrix column, then immediately reject every nonzero dimension and reduce Quantity to f64. This is the primary extension point. |
| `plots.rs::evaluate_trace` | Parses X/Y, evaluates both initially, supplies a dimensionless −10..10/500 fallback for an undefined bare X, and pointwise substitutes X into Y. Scalar broadcasting/min-length pairing are independent existing policies. |
| `plots.rs::evaluate_in_context` | Resolves shared axis slots into per-trace pairs, retains ordered Ready/Failed/Incomplete states, and deliberately does not memoize equal/shared expressions. Preserve this until original evaluation ownership is proven. |
| core `Quantity` | Canonical complex numeric value plus seven f64 SI dimension exponents; same_dimension uses1e−9 tolerance. multiply(divide=true) subtracts dimensions and divides canonical numeric values. No authored unit spelling/system or affine offset is retained. |
| compat `Value` | Scalar, Matrix, Range and RangeValues retain Quantity throughout. numeric_json deliberately omits dimensions and therefore cannot serve as the plot unit codec. |
| compat `CalculationContext::evaluate_mathml` | Labels a fragment using stored worksheet labels, evaluates read-only, validates display-unit metadata, then returns the physical Value, not the converted display Value. Adding unitOverride alone does not convert the returned axis coordinates. |
| compat editor_bridge | Plain Identifier is contextually resolved; explicit UNIT/VARIABLE annotations survive serialization. Stored bindings take priority for contextual names; explicit UNIT and VARIABLE are different Symbol identities. |
| document `PlotAxisEquation` | One authoritative id/source String per slot, no scaling source. resolved_traces returns plain X/Y strings and appearances, losing slot identity in its temporary pair view. |
| `native_plot` | Requires scaling math to remain PH; plain-source roundtrip guards reject explicit label/format structures it cannot preserve. Unitful native plots remain raw/Opaque rather than silently losing labels. |
| `PlotTraceData`, plot_snapshot, trace_outcome | Persist points, implicit_domain, label/style only; v1 all-ready and v2 ordered outcomes have no unit semantics. |
| native_plot_result / export plot_results | Finite unitless codec deliberately rejects unitMonomial. Export currently sends curve points to that codec, relying on the current evaluator's dimensionless-only admission. This assumption must become explicit before unitful support. |

There is no public Rust UnitSystem type or selected worksheet unit-system model in these sources. `core::unit`/`units::lookup` provide SI quantity facts. `result_display::quantity` has a separate derived-unit presentation table based on previously extracted static SI metadata; that table is not proof of original XY automatic unit selection. In particular its liter candidate is scaled, so reusing its string formatting as an identity scale would change coordinates.

## Proposed source model and identity

Add optional **typed authored scaling** to PlotAxisEquation, rather than a global axis option or independent field on each resolved trace. Missing/PH scaling means automatic; an explicit source contains a validated PortableTree (or an equivalent single authoritative typed MathML wrapper). Keep one representation authoritative: do not persist both editable text and typed tree as competing sources. Prefer PortableTree for the existing CPU editor/bridge; document already consumes the editor/compat dependency chain.

An additive optional field can preserve legacy JSON. Existing Legacy traces have no scale; explicitly promoting to AxisLists allocates the existing stable identities and supplies automatic scaling. Slot insertion/removal must carry the full equation object, including scaling, and preserve shared X/Y identity. A source change to scaling must affect calculation/source signatures, retained-source comparison, Pending invalidation, Undo and the native second math. An auto **result** unit must never be written back into this authored field.

For an initial typed scale editor, `m`, `in`, and `0.1*m` use explicit UNIT identifiers. Variables/constants require explicit labels or ordinary contextual resolution chosen by the editor, as0170's help requires. A blanket “all names in scale are UNIT” rule is wrong: a user-labelled VARIABLE m must remain a variable. Existing plain axis expressions can consume unitful worksheet bindings; general native expressions with explicit label/suffix metadata stay Opaque until the main expression source can preserve them too. Supporting scaling is not permission to loosen that roundtrip guard.

`resolved_traces()` should remain a compatibility presentation API; introduce a borrowed evaluation-pair view carrying references to the actual axis equations/IDs and their optional scaling trees. It must use the proven min-index pairing and not duplicate a shared slot into new authored owners.

## Proposed pure/runtime APIs

Names below are proposals, not existing public APIs:

```text
AxisSamples { values: Vec<Quantity>, shape: Scalar | Column | Range }
extract_axis_samples(&Value) -> Result<AxisSamples, PlotUnitFault>
ResolvedAxisScale { quantity: Quantity, presentation: AxisUnitPresentation }
resolve_axis_scale(authored, samples_dimension, context) -> Result<ResolvedAxisScale, ...>
display_coordinate(physical: Quantity, scale: &ResolvedAxisScale) -> Result<f64, ...>
evaluation_with_quantity(expression, replacement: Option<(&str, Quantity)>) -> Result<MathML, ...>
PlotTraceData { points, ..., axis_units: [AxisUnitPresentation; 2] }
```

`AxisUnitPresentation` needs mode (dimensionless/explicit-linear/automatic-SI), physical dimension, evaluated physical-per-coordinate scale, and a typed effective scale display tree. It is inert calculation presentation metadata. Authored scaling remains in PlotSpec; result presentation may reference the source slot ID/signature, but does not supply executable source. Avoid reusing Quantity as the entire presentation: it lacks unit spelling, mode and native residual units.

`extract_axis_samples` preserves Quantity and the current first-column/matrix, Range and RangeValues rules. Validate bounded lengths, consistent actual matrix dimensions, real finite values and finite dimensions; require homogeneous dimensions on every selected sample. Range start/step/end must have compatible dimensions, and each generated physical value must remain finite. A malformed mixed-dimension collection must fail, not inherit the first element's unit. Existing unrelated pairing behavior stays unchanged in this first change.

For explicit scaling, evaluate its typed expression as a separate ordinary read-only context eval. Require a scalar, real, finite, nonzero Quantity with finite dimensions. Require every physical sample to have the same dimension as that scale; then calculate q.multiply(scale,true), check dimensionlessness, realness and finite result, and take its real coordinate. Nonzero negative linear scales can be accepted as an explicit independent policy; original error behavior for them is unknown. Do not claim any original engine error ID for local mismatch/zero/complex/unsupported errors. Never silently divide incompatible quantities or take real parts.

This quotient realizes the proven help relation `physical value = displayed coordinate × scaling`, for ordinary linear factors. It does not recover original xyplot2 internals. Offset/affine conversions are outside this scale operation; Quantity does not represent an affine origin. A temperature-valued quantity divided by K is an ordinary linear case, but Celsius/Fahrenheit display offsets must not be invented from dimension equality.

Pointwise f(x) must receive the **physical** X sample, not its normalized coordinate. Replace the current f64 substitution with a typed physical literal: canonical coefficient multiplied by explicit UNIT SI-base factors (bounded exact integer powers initially). Serialize with the bridge and evaluate in the same context, so a variable named m cannot capture the inserted UNIT m. This avoids adding a new worksheet binding or mutating global scope. A future scoped scalar-binding query API could avoid tree expansion, but would need its own labelled Symbol identity, cancellation and ownership contract; it is unnecessary for the first subset.

Do not substitute names by raw string replacement. Existing Expr name matching/structure is the starting point; native typed identifiers/suffixes require exact labelled identity before expanding that input subset. Do not memoize evaluations by identical text or assert one evaluation per shared slot. Parsing a typed scale once is separate from evaluating it once; the initial implementation can preserve current per-trace evaluation scheduling, with that independent policy documented.

## Automatic units: a conservative SI subset

For a dimensionless homogeneous axis, automatic scale is1 and no unit is displayed. For a homogeneous unitful axis, a first automatic policy may choose canonical SI-base product `m^L kg^M s^T A^I K^Θ mol^N cd^J`, factor1. Require exact bounded integral exponents and known base identifiers. This is a safe representable subset of the help's base-unit behavior and current canonical quantities, not a recovered worksheet-system selector or preferred derived-unit algorithm.

Do not round arbitrary f64 dimensions to rational native powers merely because same_dimension accepts tolerance. If an exponent cannot be represented exactly within the chosen integer subset, return explicit unsupported. Supporting rational powers later needs a deliberate rational representation/proof. Retain mode/provenance in saved metadata so a later improved automatic policy can invalidate old calculated presentation rather than reinterpret its coordinates.

An automatic dimensional sample's coordinate is its canonical value divided by the chosen base product of factor1. The shown effective unit tree is separate from blank authored scaling. Angle dimensions are zero in Quantity; automatic selection cannot infer rad/degree from that vector alone. An explicit rad/degree scale retains the authored choice; do not auto-infer it from a bare dimensionless value.

## Domain boundary

Keep the existing implicit quickplot only for demonstrably dimensionless X domain, with absent or dimensionless scaling. Do not silently reinterpret fallback −10..10 as meters/seconds merely because a unitful X scaling is present. Unitful implicit-domain generation, original default unit-system selection and physical-domain min/max coupling are not fully defined by the current app model; reject that route explicitly for this first subset.

Supported initial route: finite stored scalar/vector/range physical X and Y, or pointwise f(x) over a finite known physical X, with compatible explicit linear scales or the bounded SI automatic policy. If a bare undefined X requires fallback and either X scaling is unitful or the expression requires physical X, report unsupported. A unitful Y result over an otherwise dimensionless known/fallback X can use its own proven-compatible scale; do not impose global equal dimensions between X and Y.

## Saved presentation and native cache separation

Introduce a version3 saved plot snapshot for any curve carrying unit metadata; retain v1/v2 readers unchanged. Do not add optional unit fields to the old wire types under the same version: their deny_unknown_fields guards currently protect semantics. v3 preserves the same ordered per-trace states and Pending previous curves, plus both axis unit presentations, bounded finite evaluated factors/dimensions, and typed effective display trees. Include/source-bind scaling identities in the existing document signature machinery. Never evaluate saved trees, factors or native units on reopen.

Legacy v1/v2 are inert coordinates produced by the previous unitless evaluator. They can continue to restore under exactly matching legacy source/context, but cannot certify units for a newly scaled or unitful source. Do not upgrade them to unit-aware freshness by assigning guessed SI units; source mismatch must make them Pending/unavailable or require fresh calculation. Disabling/reenabling a plot and Pending historical curves must retain the same unit metadata with their previous presentation.

Keep three data domains explicit:

1. Authored source: axis expression and optional user scaling tree, including labels.
2. Own calculation presentation: dimensionless curve coordinates plus physical scale/dimension/effective unit metadata in v3.
3. Native cache: literal RangeInfo/ResultInfo units10 monomials and interval metadata, retained from original XML or written by an independently validated unit-aware adapter.

Native result unitMonomial describes original result metadata, not simply the user's scale spelling. The original managed path can merge residual returned units into a preserved user scale. Therefore do not manufacture result Units=in from authored scale=in, or use physical dimension as an automatic proof of the correct residual unit monomial. Keep original unit monomial structure/system/rational exponents inert and lossless when preserving unchanged native source/context. Until a unit-aware adapter has exact compatible semantics, original unit-bearing caches remain unsupported rather than decoded through finite_unitless_xml.

For fresh explicit-linear results whose coordinates are proven dimensionless, an independent bounded export adapter could emit unitless sample metadata while preserving authored scaling source and v3 unit presentation. This is a schema-representable proposal, **not verified native reload behavior**. Gate it on focused golden/XSD/source-cache tests and label it independent policy; otherwise write source-only Pending and preserve own v3 display. Automatic-SI native results need explicit native unit names and residual-unit mapping before Synchronized export; do not derive names from presentation glyphs such as m/Ω. Retain current finite_unitless codec as a strict unitless API rather than weakening its guards.

`export/plot_results::actions` must switch on metadata proof/version: never route every Ready curve's points to finite_unitless_xml after unitful evaluation is enabled. Native import CacheIndex likewise needs either a validated unit-aware decoder that preserves native metadata or an honest unsupported result. Failed/Incomplete curves must retain the established ClearCurveToPending behavior, preventing stale curves from reappearing; no native engine error text/code is synthesized.

## Suggested implementation order and acceptance checks

First add source scale ownership/serialization/signatures and pure Quantity extraction/coordinate conversion. Next add physical point substitution and ordered per-trace runtime results. Then add v3 persistence/display and explicit source-native second-math preservation. Only after those work should unit-bearing native cache writing be enabled. UI scaling editing can follow the already separate expression/scaling source model, without duplicating authority per trace.

Meaningful cases: X=[1m,2m], scaling in→coordinates[1/0.0254,2/0.0254]; scale0.1m→[10,20]; Y=x^2 over physical X with scaling m^2; explicit UNIT m versus VARIABLE m shadowing; same X shared by Y slots with independent scaling; mixed dimensions fail one trace while other traces survive; zero/complex/nonfinite scales fail; Range start/step dimensions preserved; unitful implicit-domain unsupported; automatic exact SI product roundtrip; no rational/affine guess; v1/v2 unchanged restoration; v3 Pending retains former units; scale edit invalidates old signature; source/cache export never drops unit fields. Existing sample/memory limits and cancellation checks remain in force. No original once-evaluation or original error-ID claim should be inferred from these tests.

현재 후속 구현·검증은 [0176](0176-xy-physical-units-implementation.md)과 [0178 r180](0178-xy-units-integration.md)에 기록한다. 이 문서의 당시 설계/미구현 진술과 최신 지원 범위를 구분한다.
