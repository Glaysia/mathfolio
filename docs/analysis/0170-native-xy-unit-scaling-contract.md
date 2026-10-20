# Native XY axis units and scaling

2026-10-19. Static installed Prime 12.0.0.1 CIL, embedded schemas and installed help. No original engine, CLR, GUI or screenshots were used. No product Rust was changed. This contract separates source scaling expressions, engine result units and dimensionless display coordinates; the existing finite unitless curve codec does not establish any of those conversions.

## Identity and reproducible evidence

| Installed assembly | SHA256 | MVID |
|---|---|---|
| Ptc.Controls.Plots.dll | `4e5067e58e69c68e8152ce677d5ac77a6587e00f6295bda5cc8ddff18f6685eb` | `cecd333a-bf3e-4423-b74b-7ff799618db1` |
| Ptc.Calculation.Core.dll | `cf42d8116d697a560f84467c8c32eb8dc1a15172205772eaf4c95a1125eb01bf` | `f26e51c8-54b1-4d6f-8816-d7805b72d6c6` |

The [method ledger](../../artifacts/rea/native-xy-units-method-ledger-20261019.json) verifies 39 selected raw IL bodies against current installed PE bytes and records source metadata Evidence IDs, file offsets, sizes and hashes. Its [reproducer](../../artifacts/rea/native-xy-units-ledger-20261019.py) also verifies installed help hashes and extracts six help-embedded region XML examples without running the target. Method tokens below are build-local coordinates bound to these assembly identities, not stable public APIs.

Reused PlotEquation metadata Evidence: `ev_eda26b6104cbe44c78367474cd3fa207a9f33345a55c537a10ee42fd65b79acd`. Reused axis/trace metadata: `ev_1960b1fae2c5d467b6e1af9ef42c1c838180c712c96e94815909d70aa2ee3b25`. The Core metadata Evidence identity is retained in the ledger. Fresh exact PlotConverterHelper selection: `ev_0f16efed888fa70d3be78e6cd8b47cffae8d06893eb4afac2b3ff6a12df62b63`, complete with no unmatched type names.

Raw resolved CIL is retained under `artifacts/rea/feedback/native-xy-units-{source,equation,edit,trace,core,core-conversion,dissect,helper}-cil-20261019.txt`. The generic decoder labels its headers `plots` even for Core; assembly identity in the ledger is authoritative.

## Source: two distinct math controls per axis equation

`PlotEquation` owns expression `_exp` and scaling `_scaling`. `FormatPlotEquationSExpression`0327 builds `(@TRACE_EXPR expression (@TRACE_UNIT scaling))`. `GetRegionXmlNode`033d IL0023..0051 writes expression math first and `GetUserScalingExpression().GetRegionXmlNode()` second. It does not simply serialize the current auto-decorated `_scaling` control. Formatting overrides are separate optional children.

The worksheet50 schema's `plotEquation` sequence documents expression plus scaling and admits one or two `math` elements. The current managed writer positively emits two. Math nodes use the worksheet wrapper and math50 expression namespace. A blank source scaling is an actual math50 placeholder, not a missing unit silently equal to a particular unit name.

`SetScalingXmlNode`0339 IL0000..003a ignores a placeholder child; otherwise it imports the math into `_scaling`, captures its S-expression as `UserScalingExpression`, and shows the scaling field. `SetScalingSExpression`033a similarly ignores null/`@PLACEHOLDER`. On a user edit of the scaling control,030f IL0069..007a clears the preserved user source and sets `ScalingChangedByUser = !IsScalingEmpty`. Non-scaling source changes have a separate undo-aware DummyChangeUnitsCommand path; they must not be modeled as explicit scaling edits.

## Calculation input differs from automatic presentation

| Original method | Positive contract | Raw IL SHA256 |
|---|---|---|
| PlotEquation0328 `get_ScalingRl` | Return user scaling RL only when nonempty and changed by user; otherwise literal `(number 1)` | `36db89644536a3b7e29c2f9706fcca76c51c9b9c13ba5bac927182a0297d9627` |
| PlotEquation033e `GetUserScalingExpression` | If preserved user source exists, materialize an expression-only temporary control from it; otherwise return `_scaling` | `02b30e5441a63974659ed892caf0e6fff279bf2c98981f0af202f51ae0434eab` |
| TraceModel1286 `GetxyplotScalingRl` | Get axis equation ScalingRl and pass through GetReplacedRl with that scaling control for node/error mapping | `dc47b84a85e14fe472ada66db4455b3dea33d13cdc969984a0792e682ccf1d19` |
| TraceModel1281 `get_Rl` | Pass both axis expressions and both scaling RL operands into original `xyplot2`; manual sampling bounds use X scaling | `411108d88d7fd759568623dcb5663c5a5644c6edae6fe3492f846219ddbc60f2` |

The positive XY request shape is:

```text
(eval (numeric (xyplot2 X Y Xscale Yscale
    (op_scale lower Xscale) (op_scale upper Xscale) (numberOfPoints - 1))))
```

This is a managed request contract, not a recovered implementation of `xyplot2` or `op_scale`. Scaling is therefore part of calculation input and sampling-domain interpretation, not merely a renderer tick-label multiplier. AxisModel's separate `Scaling` enum/log-linear domain mode must not be confused with a PlotEquation scaling expression.

On successful result analysis,1297 IL0036..0093 copies RangeAxisInfo.Units and ResultAxisInfo.Units into trace analyzed state; IL00a7..00dc sends them to the corresponding axis equation's `set_Units`. Setter0352 calls MergeScalingExpression035d. That routine detaches expression-change handlers, temporarily removes the navigate-to-start binding, captures/restores the original user scaling source, merges result units, and restores the handlers in finally blocks. It runs inside a compound command scope. This is concrete separation between user source and calculated presentation; it is not proof that every presentation update is an independent user Undo step.

MergeUnitMonomial035e navigates to the beginning; when the current node differs from the placeholder constant, it inserts `MULTIPLICATION`, returns to the beginning, then calls InsertUnitMonomial. Its raw hash is `c5fb2e009580f20942ea19a395e5e303ee6fd4e42fe94ee7a6fd043f6cebcc25`. MergeScalingExpression035d hash is `dcc22612cec4b976ead22f35b80e6b5d87cf8b8a9d0d2f57a1ca7cd5e8305b6a`. The returned unit monomial may be empty; do not assume it always repeats the source unit or append a second source-unit glyph blindly.

## Installed help and literal native examples

The [help evidence](../../artifacts/rea/native-xy-units-help-20261019.json) records installed paths, byte hashes and text. The embedded XML is authored help content, not a newly captured running worksheet. It includes WebWorks title annotations and is not claimed to be a complete schema-clean MCDX package or a cache fixture.

| Installed help | SHA256 | Positive behavior |
|---|---|---|
| about_units_and_complex_numbers_in_plots.html | `5c198d67933509122a7aac54f51016d21fe001a415d466367c58bf6f89c3675c` | Units are accepted; corresponding base units appear in the unit placeholder; users can convert them or add scaling. Points with imaginary parts are not plotted; Re/Im are explicit alternatives. |
| to_change_the_scaling_of_an_axis_expression.html | `9b92567851998164b0da89882ef8623a36fffd6258ad062de8a5a0a260752b97` | Enter commits a real number/expression scaling; variable/constant labels must be chosen accordingly. Each expression can have its own scale. Physical value is coordinate times that expression's scale. |
| example_converting_units_in_plots.html | `15eb8ec76f807552a36fc8688bfb8132f0cbe14c9a1dc654885f2b1fe54e597f` | Same physical function displayed using m, in and 0.1 m; tick/callout values are unitless. |

The example defines `dist(x) = (x / 10^3) * km`. Extracted region `ID0ENFXJB` has Y expression `dist(x)` and scaling math `id(labels=UNIT,"m")`. `ID0E3HXJB` changes only that scaling source to `in` and its Y viewport end is200 instead of5. `ID0ENJXJB` has one X equation and two Y equations with the same `dist(x)` source but distinct scaling math: `in`, and `<apply><scale/><real>0.1</real><id labels="UNIT">m</id></apply>`. Their trace refs are14/15. The help's rounded plotted coordinates79 and20 represent approximately2 m under their respective scales; this is not exact rational proof that79 inches equals2 m. The later marker example uses unitless marker coordinates20 and79.

These are positive source shapes for arbitrary typed scaling expressions and per-equation scaling ownership. They do not justify duplicating one shared X source into independent per-trace authorities, or treating scaling as a global axis property.

## Trace result units and managed conversion boundary

Trace2dResult has separate RangeInfo and ResultInfo. Math50 AxisInfoType requires Min and Max doubles, and permits one units10 unitMonomial child. units10 defines a sequence of unitReference entries with required `unit` string, optional integer power-numerator/power-denominator defaults1, and optional monomial system string. The rational unit powers are not scalar coordinate factors and cannot be dropped from an otherwise valid curve.

Core0180 WriteAxisInfoNode writes Min/Max, then visits nonempty Units (`4c8edb52773d1c4a0e4c3e4b8d0a47a42a284f756cdaa1e0fb2e8cb98dd5d3cf`). Reader022b BuildAxisInfo parses invariant doubles and the optional namespaced unitMonomial, then constructs TraceAxisInfo (`c2229f9ed3a35e4a6eb7092c0a4473e8611d987b642f8f3a1bfdf379273965df`). See [0148](0148-native-xy-result-contract.md) for vector JSON and resultRef ownership. No new original unit-bearing cache was found in these help examples: resultRef attributes alone are not result payloads.

Core0297 DissectUnits accepts an IUnitedValueResultNode, separates its Units from its Value-as-matrix, and otherwise returns an empty unit list and the matrix itself (`fc0739c78fdaa16260f2479599bc919b6eafef5b656025e0a49dd3bedc5db1bb`).029b calls it for result matrix slots[0,0] and[1,0].0299 validates, builds numerical vectors, and constructs axis metadata from slots[2,0]/[3,0] and[4,0]/[5,0] with the separated unit lists. This representation is positive; it does not reveal the unit conversion performed upstream inside original engine operators.

Core0290 ValidateColumnsAndUnits checks nonempty range units against virtual RangeUnitsAllowed, then point-count and result-column limits. Its specific error resource is `PlotControl.Error.UnitsAreNotSupportedInRange`; this is not a general XY dimensional-mismatch code. PlotConverterHelper1221 returns `GetRangeAxisType()!=2`; GetRangeAxisType1298 reads the actual range axis Type. Do not apply this check indiscriminately to ordinary XY or invent the engine's other dimension errors. No source-level general compatible-dimension algorithm was recovered in this path.

Core01ff TryParsePlottableValue recursively unwraps united values; real nodes yield their double, complex/imaginary nodes yield NaN, and unknown node kinds return false. This agrees with the help's exclusion of imaginary points. It does not authorize silently taking the real part of a complex number.

## Implementation-ready boundaries and unknowns

The next implementation needs one authoritative expression plus an optional source scaling expression **per axis slot**. Automatic result unit presentation is separate transient/restorable state. Native serialization must preserve both math sources and labels; import must retain unit-bearing result metadata or explicitly leave its cache unsupported. Fresh calculation must produce dimensionless plot coordinates together with enough unit metadata to restore the displayed scaling field; bare f64 points cannot certify dimensionlessness. The existing unitless codec must continue rejecting unit-bearing payloads until a separate validated unit-aware codec exists.

The help supports a bounded physical-to-display conversion interpretation for ordinary linear unit factors. Exact original `xyplot2` dimension checking, zero/negative/complex scaling error behavior, automatic unit-system/base-unit selection, affine/offset units, rational-unit simplification, per-sample heterogeneous units, and scaling evaluation frequency remain unknown. No error IDs, cancellation rules or native once-only evaluation semantics are inferred. A future independent engine policy may support a tested subset, but must distinguish that policy from recovered original behavior.

No product completion percentage is implied by the39 inspected methods or6 source examples. The original investigation changed only this document and ignored evidence artifacts; the later independent codec is described below.

## Follow-up: canonical names and inert unit-aware codec

Core0185 `ResultNodeToXmcdTransform.Visit(Unit)` writes Unit.CanonicalName directly to units10 `unitReference@unit` (IL0020..002b), and always emits its numerator/denominator fields. Raw IL SHA256 is `1479fe0b35ad03b9be17229e9897f759eb8e7642f3249651317656deaa93ab5f`. Core017a writes the ordered monomial entries (`2b4146898b926608425c21e0cfefbf99cd8a9592dedf305fcb17f80257e5c2db`). Core0193 AddUnit stores its name argument as CanonicalName and obtains SymbolResultNode by DefaultUnitDescriptorTable[name] (`acab2fb234a23f90ef481526e25c64b143b2d84e9eb99ad05f3659fcf1a47eb4`). A user glyph is consequently not a safe native canonical name.

The default descriptor satellite resource supplies exact SI base mappings: meter→m, kilogram→kg, second→s, ampere→A, kelvin→K, mole→mol, candela→cd. Each comes from a single UnitDescriptor with explicit CanonicalName, Symbol and SI base membership. [Selected property blocks and resource identities](../../artifacts/rea/native-xy-units-si-canonical-names-20261019.json) retain exact BAML offsets (coordinates exclude its four-byte length prefix), satellite PE SHA256 `001e733202d0aaadcf9079df21eecfcfaab61903542213380419b369e3126073`, and BAML SHA256 `cdb8666e2258469f22fe3349d188d3dbec8578e4d5042f70db5e0b3596729ecb`. Core0010 loads `/Ptc.Calculation.Core;component/defaultunitdescriptortable.xaml`. Independently, authenticated SI Update0001 contains corresponding five-item unit arrays with canonical name, glyph, quantity and dimensional declaration; McdRun0017 passes those fields to Mcd.Unit construction. These positive maps do not establish a complete preferred-unit selection algorithm or license translating unknown names.

New [native_plot_units.rs](../../crates/mathcraft-document/src/native_plot_units.rs) adds NativeUnitReference, NativeUnitMonomial and NativePlotUnitResult plus read_result_with_units, read_mathml_with_units and finite_with_units_xml. Existing NativePlotResult and strict unitless APIs/goldens remain unchanged. The new reader removes only validated axis monomials from a bounded copy, restores in-scope namespace bindings, and delegates all numeric/vector/interval validation to the established unitless parser. No source math is evaluated or used as a unit definition. Fresh writer points are caller-supplied coordinates, and unit metadata is caller-supplied residual metadata; the codec does not infer the correct relationship between them.

Reference order, duplicate reference names, optional system (including explicit empty string), absent versus explicit power1, signed numerator/denominator, Unicode names and XML-sensitive text are preserved semantically. It does not reduce fractions, combine repeated units or resolve names. The shipped schema requires at least one reference: Some(empty monomial) is rejected and not collapsed to None. Denominator0 is rejected by policy even though xs:integer alone permits it. Other independent limits:32 references/axis,1024 UTF-8 bytes/name/system,16KiB total metadata per monomial (names plus bounded power overhead), integer powers within±1,000,000,4MiB XML,384 XML nodes, and the existing65,536-point finite vector bound. Comments/PI, unexpected attributes/namespaces/elements, mixed structural text and duplicate monomials are rejected. Inherited namespace bindings are capped separately. Unit identifiers remain uninterpreted strings.

Both metadata types support Serialize/Deserialize with strict unknown/duplicate field rejection, bounded reference decoding and validated conversion. JSON null on optional attributes means absence. Public validate methods and MAX_NATIVE_UNIT_METADATA_BYTES let the saved-presentation caller enforce the same policy; manually constructed invalid values must also be validated before being trusted.

Six focused tests cover optional/rational/system metadata and float bits including signed zero/subnormals, independent literal/wider intervals and inherited namespaces, malformed/injection/budget rejection, strict JSON, and a UTF-16 native result part with original bytes preserved through NativePackage and inert cache extraction. The existing five unitless tests remain passing. The ignored native_xy_units_schema_fixture produces two independent generated resultData fixtures; [offline validation](../../artifacts/rea/native-xy-units-schema-validation-20261019.json) confirms both against unchanged shipped result10/Math50/units10 schemas, including escaped supplementary Unicode and rational powers, and verifies empty monomials fail XSD. This does not claim original runtime acceptance or automatic dimensional conversion.

### Bounded canonical-name display map

The [selected display-map evidence](../../artifacts/rea/native-xy-unit-display-map-selected-20261019.json) retains31 descriptor objects covering30 distinct canonical names, reverified against the same installed Core satellite resource bytes. In addition to the SI7 mapping above, direct positive examples are `volt→V` (CanonicalName/Symbol BAML offsets9874/9922), `ohm→Ω` (19442/19479), and `henry→H` (14822/14863), each with explicit SI derived-system membership. Other selected records retain their complete property blocks and offsets; this is a bounded display mapping, not a conversion table or automatic preferred-unit algorithm.

`degree` occurs twice with symbols `deg` and `°`; the selected resource view does not establish dictionary-key/precedence resolution, so that ambiguous canonical name is excluded from a single automatic display map. `deltaC→Δ°C` and `deltaF→Δ°F` are distinct from `degC→°C` and `degF→°F`: the latter two descriptors explicitly set IsFunction=True. Their display glyphs do not authorize treating affine temperature functions as linear scaling factors.

The application may use only positively mapped SI/derived names for readonly presentation. Unknown native canonical strings remain literal preserved tokens, without evaluation or guessed conversion. Core0204 SymbolResultNode selects RL when IsSymbolResultNode is true, otherwise Symbol;020b Key uses CanonicalName only when no explicit key is supplied. These getters reinforce the separation between source identifiers, canonical cache names and display symbols, rather than defining a general normalization rule.
