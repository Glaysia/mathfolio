# Native saved-result persistence contract

2026-10-19. Static installed artifacts, embedded schemas and original package contents. No original GUI, screenshots, runtime or CLR assembly loading. A serialized result is presentation data; it is never passed to the evaluator or used to restore bindings.

## Evidence identity

| Artifact | SHA256 | MVID |
|---|---|---|
| Ptc.Controls.Core.dll | `654a0252f699c9c9763aa4f4102488279500ed89cb8ce285a38476c2dd24a977` | `b5e8e4fd-c134-4713-b1b1-eea8cddcdd9f` |
| Ptc.Calculation.Core.dll | `cf42d8116d697a560f84467c8c32eb8dc1a15172205772eaf4c95a1125eb01bf` | `f26e51c8-54b1-4d6f-8816-d7805b72d6c6` |

Fresh REA member evidence: `ev_745e901a62591e76401ee5f5d1a1f8989f0e4fb55caca2987c65c0e9fb536934` (33 serializer methods) and `ev_55745f3f074cd4c4d3155e8e4a34fa0bbf159974b4abc2575e2e8bc7d630f806` (71 result transform/EngineResult methods). Installed bytes and 24 selected raw IL hashes were independently checked in `artifacts/rea/native-result-persistence-method-ledger-20261019.json`; decoded instructions are in `feedback/native-result-persistence-cil-20261019.txt`. The extraction script and original package observation manifest use the `native-result-persistence` prefix. These counts describe evidence coverage, not feature completion.

Embedded result10.xsd version1.8.1 SHA256 `f2d598e3cb62fecd5dd986b781856f4d5ffdb1ee530cc872ac4969984d208bcf`; Math50.xsd `a10afb4dd442a4f5dd77b6f1df903c1d2e5b6f4d6a701aae38b0cefee21f6ca3`; units10.xsd `d905470fd889df389521eb556f24071fb5ccdcaf6835b4a41336451a913eb6b5`; worksheet50.xsd `b4a73084e11db4dbcba52c603e2791a3026b088e150cb482114b9a315f355ba3`. Resource extraction identity is recorded in `artifacts/rea/document-deep/schemas/manifest.json`.

## Original storage and status

`resultsList/resultData` has required signed-int `result-id`, optional `tag` and optional `calculation-status`. The only schema statuses are **Synchronized** and **Pending**. `UpToDate` is not the stored enumeration. The child sequence is an optional choice of `ml:result`, `ml:symResult` or `ml:Trace2dResult`, followed by optional `engineErrors` and `Warnings`. Worksheet math `resultRef` is an optional xs:int joining this ID. Requiring a fresh ref for newly exported supported math is our allocation policy, not an XSD requirement that every math element already has one.

`ResultDataSerializer.Write` `0x06000ba7` SHA `5e43c0f27fe430fb2ab521620275ab970b0fc37559cb632c2b1d9632ef717693` writes `/mathcad/result.xml` with the original result content type. `ResultSerializer.WriteXml` `0x06000bb5` SHA `9a3e5e9d791c82f99e321338c093a8b9fdd90771ffdffa4ccb826bcf56dace39` enumerates the result manager and writes ID, tag, synchronization status, result, errors and warnings. It does not discard the value merely because its status is Pending.

`EngineResult` status-copy constructor `0x06000051` SHA `ebc2cd0bae2e7a2948553c3a95ecc8b0f0247de79f1713e5d3a250f071535d2e` copies the result node, errors, warnings and serialization tag, then changes synchronization status. Historical values with Pending are therefore legitimate original data. Preservation of that payload does not certify it as a newly calculated value.

Original en-US templates and user-provided detune.mcdx show synchronized empty definition rows and query rows containing `ml:result`. Eight package/result-part observations with source/part hashes are retained. The inspected sample contained real, complex, united-value and redefinition-warning payloads; it did not contain an original matrix cache. Matrix layout below is established by original writer and reader instructions.

## Numeric representation

`AddResultToElement` `0x06000bb6` SHA `30002522b38b6be75da2d067f91431b7276b46836d767c80242b6e5b56377d83` wraps ordinary numeric results in `ml:result`.

| Value | Original representation and proof |
|---|---|
| Real | `ml:real`; `WriteRealXml` `0x0600016f`, SHA `cbaf09648f12f44f7f464f5f99c08503b7f00e3e907ad849463d1435b2656899`, invariant `Double.ToString("R")` |
| Complex | `ml:complex` with real then imag; Visit `0x06000176`. Imaginary writer `0x06000170`, SHA `b9536e86ba993eb17f77f16029f53868cbf330514e3cb1c50179771c69657dd7` uses the same round-trip formatting |
| Matrix | `ml:matrix rows/cols`, **column-major** child sequence; `WriteMatrixXml` `0x06000171`, SHA `e1c8e256df788927547684099c524c1cbfd71361339d8e3cebd2aef1f1404155`, outer column loop, inner row loop, `get_Item(row,column)` |
| Unitful | `ml:unitedValue` with numeric child then `u:unitMonomial`; Visit `0x06000179`, SHA `b6433239af64fb4ef468303939391ed7ae7e52d2d4499ab401ee01c7505d04b4` |

Unit references carry a unit descriptor name and rational exponent. Choosing descriptors depends on the original unit-system/result conversion path; it cannot be reconstructed by guessing labels for a dimension vector.

Explicit output units have a separate important observation: detune regions9/21/31 with ns/pF/μH overrides store already-converted bare numbers (`2777.777777777778`, `1334.6294794966027`, `146.65827163074007`) without a cached unitMonomial. Ordinary and Area own-cache saving already persists `display_value.or(value)`, so explicit overrides arrive as dimensionless converted display values. Substituting canonical SI values would change the shown number; adding another unit would duplicate the source's output unit.

## Whole-program preservation proof

An unchanged row is insufficient when an upstream definition changes. `native_result::program::same_calculation_program` compares the complete ordered source, without calculating it. Direct siblings are stably ordered by local Y then X, following the existing bounded compat order. Areas contribute their local child sequence as a whole block at the parent's position, rather than globally sorting parent+child absolute coordinates. Region identities and ownership, MathML labels/values, disable and other semantic attributes remain bound. Reordering, changing a source or a disable flag prevents preservation of the old program's Sync states.

Known direct math `resultRef` and `error`, supported leaf numeric result formatting and basic formatting overrides are excluded from this computation proof. Unknown attributes, children, collection metadata and significant text remain in canonical source hashes. Prefix spelling and attribute order are normalized, while unknown nested whitespace remains conservatively exact. Pure nonembedded text and bare page breaks do not add calculation records. An ordinary empty Area adds no calculation record; unknown flags/children prevent this omission. IDs must be unique decimal nonnegative source IDs within the bounded proof; draft marker IDs, missing/duplicate IDs and malformed coordinates reject the proof.

The caller must additionally require `ContextSnapshot::matches`. This captures sorted part name, length and SHA256 for **all** nonworksheet parts except parse-confirmed result10 `resultsList` roots. It includes calculation/presentation settings, relationships, external flow documents, binaries and unknown parts. A file called result.xml with another root remains hash-bound; an actual result root under another filename is classified by its XML. UTF16 result parts are recognized. Binary bytes are hashed by reference, not cloned into the snapshot.

This additional context check is necessary because a text region may reference a flow document with embedded math and components can reference source outside worksheet.xml. Changing even a plain text part or page resource conservatively rejects reuse for now. No partial row-by-row Sync proof is returned when either full-program or package-context proof fails. Pending historical presentation and separately proven fresh results remain different fallback paths owned by export integration.

## Bounded fresh numeric writer

`native_result::numeric_result_xml(&SavedNumericSource)` returns a complete namespace-bound `ml:result` only for supported `EvaluatedValue` data: finite exactly-dimensionless scalar/complex values and nonempty rectangular matrices. Input values are internal row-major; the writer emits native column-major. Rust round-trip decimal output preserves the f64 numerical bits, including signed real zero; this is not a claim that every decimal lexeme is byte-identical to .NET's spelling.

Unitful values, ranges/range outputs, symbolic results, raw NativeMathml and Area bundles return `None`. Malformed JSON, unknown scalar/matrix/quantity fields, invalid dimensions and budget violations return an error. Raw MathML is neither parsed as executable input nor passed to the evaluator. JSON is validated and then moved into the typed value without duplicating the whole JSON tree.

The caller owns current source-signature equality, per-row Synchronized state, owner/Area path identity and final export result-ID mapping. The literal writer does not certify freshness. These checks must complete before inserting a Synchronized resultData; unsafe or unsupported data requires Pending fallback.

Limits are our implementation bounds: 4MiB input/output, 65,536 matrix cells, JSON depth16/1.5M nodes, XML250K nodes. Source proof uses 8MiB XML, 250K parser nodes and depth64; NativePackage's existing part/archive bounds also apply. These are not Prime maxima.

SavedCalculationRow retains human error/warning strings without original native error ID/code/node handle/previous-error relationship or warning nodeId/oldType. Fresh structured diagnostics cannot be faithfully recreated from that carrier. Original raw diagnostics can be preserved under the source/context proof; inventing native codes from translated strings is outside this implementation. Unit-system selection, rational dimensions, lazy/display matrices, ranges, symbolic and plot output writers remain separate work.

Validation: fourteen focused literal/program/context tests pass, including exact float and signed-zero roundtrip, non-square column-major matrix, unsupported inert payloads, malformed metadata, size/depth bounds, upstream changes, Area ordering, unknown metadata, resource changes, UTF16 and filename-independent result classification. The document crate all-target Clippy check passes. Parent export/application/OS integration is recorded separately; these checks do not establish original runtime acceptance.
