# Native XY saved-curve contract

2026-10-19. Installed Mathcad Prime 12.0.0.1 metadata/CIL, embedded XSD and Mathcraft headless codec tests. No CLR assembly loading, GUI, screenshots or original runtime execution. Source/axis/style reconstruction is separately recorded in 0146; this note concerns cached curve samples. Cache data never restores an evaluator binding.

## Artifact identities and raw proof

| Installed artifact | SHA256 | MVID |
|---|---|---|
| Ptc.Calculation.Core.dll | `cf42d8116d697a560f84467c8c32eb8dc1a15172205772eaf4c95a1125eb01bf` | `f26e51c8-54b1-4d6f-8816-d7805b72d6c6` |
| Ptc.Controls.Plots.dll | `4e5067e58e69c68e8152ce677d5ac77a6587e00f6295bda5cc8ddff18f6685eb` | `cecd333a-bf3e-4423-b74b-7ff799618db1` |
| Ptc.Controls.Core.dll | `654a0252f699c9c9763aa4f4102488279500ed89cb8ce285a38476c2dd24a977` | `b5e8e4fd-c134-4713-b1b1-eea8cddcdd9f` |

Fresh REA Core member Evidence is `ev_e967082b7f1a13161c82b5c2c4d6ab9d566b093733a94a37f0185b2ebb7b6473`, with complete metadata coverage and 828 returned methods. A selected projection retains original method/field/ref/string records, linked to that complete Evidence. Plots source evidence is `ev_1960b1fae2c5d467b6e1af9ef42c1c838180c712c96e94815909d70aa2ee3b25`. The [ledger](../../artifacts/rea/native-xy-result-method-ledger-20261019.json) independently rechecks 26 Core/Plots raw IL bodies plus the existing Controls wrapper body against current installed bytes. It records offsets, raw IL SHA256, build-local tokens, enum constants, resources and output hashes. These counts measure evidence coverage, not product completion.

Embedded schemas were rechecked against their original resource byte ranges in installed Ptc.Core.dll SHA256 `ace9265338ef2aecf73b3e62540e026c5f0966df61a00127a2083110f4a2a4ab`: Math50.xsd `a10afb4dd442a4f5dd77b6f1df903c1d2e5b6f4d6a701aae38b0cefee21f6ca3`, result10.xsd `f2d598e3cb62fecd5dd986b781856f4d5ffdb1ee530cc872ac4969984d208bcf`, units10.xsd `d905470fd889df389521eb556f24071fb5ccdcaf6835b4a41336451a913eb6b5`. Resource extraction Evidence/offsets remain in the [schema manifest](../../artifacts/rea/document-deep/schemas/manifest.json).

## Direct result root and binding

`result10:resultsList/resultData` can directly contain `ml:Trace2dResult`, as an alternative to ordinary `ml:result` or symbolic `ml:symResult`. It is not a scalar result wrapped around an arbitrary plot expression.

Controls `ResultSerializer.AddResultToElement` BB6, raw IL SHA256 `30002522b38b6be75da2d067f91431b7276b46836d767c80242b6e5b56377d83`, positively skips the ordinary numeric wrapper for TypeRef 27c(ISymbolicResultNode) or 27d(ITrace2DResultNode). Those TypeRef names were resolved from the current installed table. The direct root agrees with XSD; the helper does not invent an additional ml:result wrapper.

TraceModel `get_RegionXmlNode`1277 ends with its result manager’s `CreateResultRef`; setter 1278 ends with `UpdateResultRef`. Their raw IL hashes are respectively `a0e908aa71be44a1b54e01bebc12505b488592759a83fd81510a1da116eba050` and `f4124047cfc45f607019b3fc030411720a597af1be1dade1e0006445053a138c`. The trace’s own resultRef joins the resultData result-id, rather than being an axis equation ordinal or a region ID. Core15e writes the attribute when a result ref is specified;161 reads it and binds the owner via 160. With no attribute, 161 returns false. Source-only export without cached points is therefore a separate supported shape, not proof that an exported curve has cached samples.

Synchronization status and ID allocation are owned by the caller. Synchronized/Pending semantics, historical Pending payloads and context/source proof are the existing [0143 contract](0143-native-result-persistence.md). This codec does not certify freshness, execute source math or reconstruct native engine binary state.

## Sample representation

The child order is exact:

```xml
<ml:Trace2dResult xmlns:ml="http://schemas.mathsoft.com/math50" TraceType="Vector">
  <ml:RangeInfo Min="-1" Max="1"/>
  <ml:ResultInfo Min="0" Max="2"/>
  <ml:Data>
    <ml:RangePoints>
      <ml:DataVectors VectorsType="Columns" Count="1" VectorLength="3" ValuesType="Numeric">[-1,0,1]</ml:DataVectors>
    </ml:RangePoints>
    <ml:ResultPoints>
      <ml:DataVectors VectorsType="Columns" Count="1" VectorLength="3" ValuesType="Numeric">[2,0,2]</ml:DataVectors>
    </ml:ResultPoints>
  </ml:Data>
</ml:Trace2dResult>
```

RangePoints is X and ResultPoints is Y. Corresponding samples are paired in original order; no sort or duplicate-point removal is implied. Count is the number of vectors, not the number of samples. VectorLength is the length of each vector. `Columns` and `Rows` are serialized vector orientations; Numeric and String are distinct value kinds.

| Contract | Installed code | Raw IL SHA256 |
|---|---|---|
| Root/TraceType/axis ordering | Core17f Visit(ITrace2DResultNode) | `1dec2567de75292b5b10ddba5b2cc2ffc78215268211292ce60ed58a0da99982` |
| Min, Max then optional units | Core180 WriteAxisInfoNode | `4c8edb52773d1c4a0e4c3e4b8d0a47a42a284f756cdaa1e0fb2e8cb98dd5d3cf` |
| RangePoints then ResultPoints | Core181 WritePlotDataPoints | `1bc636f521b5b4912baa812a529f2020594b6fc3d2d2a3ee9f821f0cc371867d` |
| DataVectors fields and numeric writer dispatch | Core182 Visit(IVectorListResultNode) | `a1c330a9b6b3e8795cdda3f7bdbbb3d939feb6791d6ae3117ea9296fd41ba34d` |
| Flatten actual vectors before serialization | Core23c Write | `f6007521105b0a417cc5de45656cef79da845353d5855d86539ff368711325d7` |
| Chunk parsed values by VectorLength | Core23b Read | `d37b2783b7846c90697b63ce53242484dae0448e56c07f7817685d579861dc25` |
| Actual vector count must equal Count | Core230 BuildDataVectors | `59fe556d1fae9c7b731e56a7423a2310cefe9858bc5fbc56ec0cdb1ee4a4f2b7` |

**The current writer uses flat JSON.** Math50’s documentation example shows nested arrays, but current 23c flattens the vectors and serializes T[]. TypeSpec1b00004b raw bytes `1d1300` mean SZARRAY of generic variable 0, and 23f constructs DataContractJsonSerializer for that type. 23d calls WriteObject, 23e calls ReadObject, and 23b divides the flat list into VectorLength chunks. The schema’s DataVectors simple content is only xs:string and cannot resolve this discrepancy; accepting the comment’s nested array as current numeric data would be wrong.

Current enum constants are Trace2DResultType Vector 0/Parametric 1/Real 2/Range 3, VectorsType Columns 0/Rows 1, ResultValueType Numeric 1/String 2/Mixed 3/NaNs 4. Native trace style is a different enum: Lines 0, Columns 1, Effects 2, Boxes 3, Bars 4, Errors 5, Stems 6, Waterfall 7. Do not confuse the style `Columns` with a DataVectors orientation.

Column flags have a separate positive producer path. Core29c `get_ReadAsColumns` is `CurrentType != Boxes(3)`. 29a BuildTraceData constructs X vectors as Columns and chooses Columns for ordinary line Y data, Rows for the Boxes path. The constructor 245 stores that orientation and vector length. For one vector on each axis, TraceModel1292 selects the first X/Y vector for ordinary Lines and Core2b8 pairs their elements. That establishes the narrow finite Columns pair; it does not establish support for all vector/style combinations.

## Units, gaps and unsupported forms

RangeInfo and ResultInfo may contain units10:unitMonomial. Core180 omits empty units;22b reconstructs an axis’s united minimum and unit information. Min/Max are axis result metadata, not necessarily the exact sampled extrema or the user’s manual viewport. The reader preserves finite metadata intervals separately from points; wider intervals are not discarded. App PlotData does not itself carry every original interval/unit field, so this stage does not claim full original viewport/pixel identity. Original XML must remain available for unchanged context/source reuse.

There is positive evidence of NaN break markers in the original point representation: Core2b8 AddOneDataSegment copies paired points and can append Point(NaN,NaN) at segment boundaries;2b9 compresses repeated NaN gaps for a fixed X, and2be builds tuple segments. This does not prove every special floating-point JSON lexeme. The current bounded codec deliberately rejects nonfinite points instead of inventing a gap convention or silently connecting separated segments.

The original writer can emit an empty Data for empty TraceDataPoints; the XSD sequence requires RangePoints and ResultPoints. That native empty/cache-image path is outside this finite nonempty subset. Parametric, Real/Range trace reductions, Rows, multiple vectors, strings, units, images and special point representations remain unsupported by this codec. They must retain original source/cache or follow caller Pending fallback, not be reinterpreted as unitless vectors.

## Bounded independent implementation

[new native_plot_result.rs](../../crates/mathfolio-document/src/native_plot_result.rs) provides:

- `finite_unitless_xml(&[[f64;2]])`: a namespace-bound direct Trace2dResult for nonempty finite already-dimensionless points, with one Columns/Numeric vector per axis. It derives Min/Max from those fresh points. Empty input returns None; invalid/budgeted data returns an error. Bare f64 cannot establish dimensionlessness, so that proof belongs to the caller.
- `read_result(roxmltree::Node)`: accepts only direct Vector/Columns/Numeric Count 1 pairs with equal positive lengths, finite values, finite ordered axis metadata containing the samples, exact attribute/child structure and no units. Unknown supported-native shapes return None; malformed dimensions/JSON/structure return an error.
- `NativePlotResult`: original-order points plus separate RangeInfo/ResultInfo `[min,max]` fields.

Limits are our policy: 65,536 points, 4MiB source/output, 64 XML nodes. They are not Prime maxima. The app may impose a smaller sample limit. A bounded serde visitor reserves only the validated vector length and rejects an oversized JSON sequence during parsing; it does not first deserialize an unbounded JSON tree. Numerical f64 bits including signed zero/subnormals are preserved; decimal spelling is not claimed byte-identical to the CLR serializer.

The module owns no source parser, engine, calculation status, result-ID allocation or style conversion. Callers must prove current plot source/package context, dimensionlessness and result status before writing fresh Synchronized rows. Pending historical presentation can be displayed inertly, but cannot establish bindings or certify current source. Root application/export integration remains separate.

## Headless tests and XSD validation

Five normal tests pass: independent literal golden/read path, float bits/signed zero/subnormal roundtrip, unsupported units/Rows/multiple-vector/type fallback, malformed count/length/nested/null/NaN JSON/unknown metadata/budget rejection, axis bounds and duplicate-child guards. Document all-target Clippy with `-D warnings` passes.

The ignored `native_xy_result_schema_fixture`, with env `MATHCRAFT_NATIVE_XY_RESULT_OUTPUT`, independently generated three resultData fixtures: ordinary points, difficult floating-point bits and constant axes. The probe passes and all three pass unchanged embedded result10/Math50 XSD. Networking/entities are disabled. An aggregator imports the existing local standard xml.xsd namespace declaration before the original result10.xsd, because embedded Math50 has a location-less XML namespace import; proprietary source schema bytes are not patched. The standard supplement SHA256 is `4e3111f5687d47cd3652d7f0b1d7f3249be65cb7e06a752f5cfa79a505f90e43`.

Unknown TraceType fails XSD as expected. Nested JSON still passes XSD because its content is xs:string, while the independent Rust reader rejects it. XSD validity alone is not a native runtime acceptance test. No original runtime was executed.

Evidence artifacts: [current serializer CIL](../../artifacts/rea/feedback/native-xy-result-current-serializer-cil-20261019.txt), [column flags](../../artifacts/rea/feedback/native-xy-result-column-flags-cil-20261019.txt), [TraceModel binding/analysis](../../artifacts/rea/feedback/native-xy-result-trace-binding-cil-20261019.txt), [raw method ledger](../../artifacts/rea/native-xy-result-method-ledger-20261019.json), [schema validation](../../artifacts/rea/native-xy-result-schema-validation-20261019.json), [offline validator](../../artifacts/rea/validate-native-xy-result-20261019.py), [probe log](../../artifacts/rea/native-xy-result-fixture-20261019.log), [ordinary fixture](../../artifacts/rea/native-xy-result-generated-20261019/ordinary.xml), [float fixture](../../artifacts/rea/native-xy-result-generated-20261019/float-bits.xml), [constant fixture](../../artifacts/rea/native-xy-result-generated-20261019/constant.xml). Complete own-source hashes and source-projection relationships are retained in the ledger; later integration identities are recorded by root separately.
