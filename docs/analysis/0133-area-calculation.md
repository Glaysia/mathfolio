# Area calculation blocks and ordering

2026-10-19. This adds bounded calculation support to the Area storage contract in [0132](0132-area-contract.md). Original behavior is based on retained installed CIL, schema and shipped help. No original GUI, screenshots, managed assembly execution or original calculation runtime was used.

## Evidence and ordering

The method identities and raw CIL hashes are recorded in `artifacts/rea/area-method-ledger-20261019.json`. Relevant retained bodies are in `artifacts/rea/feedback/`:

- `area-root-sorted-insertion-cil-20261019.txt`: Worksheet0807 inserts using BinarySearch070c and its location comparer.
- `area-local-sorted-insertion-cil-20261019.txt` and `area-local-order-cil-20261019.txt`: local Whiteboard0183 uses the local comparer;0150 compares Y, then X.
- `area-worksheet-calculatable-order-cil-20261019.txt`: Worksheet0e12 enumerates the ordered WhiteboardItems.
- `area-worksheet-calculation-children-cil-20261019.txt`: Worksheet079c obtains calculation children through childHider.
- `area-calculation-hierarchy-cil-20261019.txt`: calculation-core0335 yields a container's children to exhaustion (IL00aa–0113) before advancing to the next sibling (IL0130); it does not emit the aggregate as a formula.
- `area-calculatable-init-cil-20261019.txt`: Area0076 initializes `adoptContainerChildren=true`.

These establish sibling-local Y→X order followed by expansion at the owning parent's slot. They do not establish a global `(parent.top + child.top)` sort. Exact overlap ordering remains our stable input-order policy; complete original overlap tie behavior has not been recovered.

For example, an Area at worksheet Y=20 containing a definition at local Y=1000 executes that definition before a normal worksheet query at Y=30. A global-coordinate flattening would incorrectly execute the query first.

## Implementation boundary

`mathcraft-compat/src/area_calculation.rs` collects bounded Area child blocks for `parse_worksheet`. A supported container has one WS `Area`, one `regions` collection, source region children, recognized display attributes, and an absent or explicitly false inherited parent calculation flag. Children are sorted within that collection, then expanded at the parent's sibling slot. Empty Areas produce no calculation row. Child region IDs, resultRef values, source fragments, and local coordinates remain authoritative. The existing optional grid snaps those local coordinates; coordinates are never added to parent coordinates.

Collapse, border visibility and height are presentation metadata. Changing those fields alone does not alter child calculation signatures or make an already computed child pending. The ordinary per-child disabled flag excludes that child; re-enabling restores its original identity/resultRef and invalidates the affected downstream prefix. General top-level math execution metadata handling remains unchanged.

The existing checkpoint, global prerequisite, ThroughRegion and ActiveRegion mechanisms consume the resulting sequence. Global definitions inside Areas remain global prerequisites. Parsing and updating still execute zero expressions and retain previous results until an explicit calculation replaces them.

`context_before(left, top)` previously assumed every result row used worksheet coordinates. It now retains a separate top-level slot for each calculation row. This lets a worksheet plot/query after an Area obtain all computed child bindings even when their local Y exceeds the plot's worksheet Y. Published child result geometry stays local. The extra retained positions cost two f64 values per calculation row; this is not another native XML or result-cache copy. This coordinate-based API describes worksheet-level queries; an Area-local plot context API is not introduced here.

## Conservative unsupported cases and budgets

Nested Areas are forbidden by the original authoring help. A nested Area, unknown execution attributes, mixed Area/math content, malformed display metadata, protected/unknown container content, or an enabled inherited parent `disable-calc` flag produces one Unsupported row for the owning container. No child from that container is partially executed. The inherited parent flag's full engine masking semantics remain unestablished; this implementation does not invent a reversible parent mask.

A malformed child coordinate fails parsing atomically and leaves the previous calculator session intact. Existing XML size, coordinate, namespace, statement and evaluation guards remain. The independent 10,000-region traversal budget counts Area containers as well as their traversed children, including empty containers; the existing 8 MiB worksheet limit also applies. This is a bounded implementation policy, not a claim about the original maximum.

Opaque Area authoring, native export reconciliation, print/error-line behavior and app child-result presentation are separate app/document responsibilities. The plain `editor_bridge::worksheet_xml` statement builder does not accept an Area as an ordinary MathML statement. Integration must preserve the actual WS region/Area/regions hierarchy rather than wrap it inside WS math or duplicate children at worksheet level.

## Compatibility and validation

The non-Area branch retains the r132 parser's XML-order parsing/table-label collection followed by stable geometric execution sorting. It does not reinterpret old table label context in the new Area ordering scheme. `src/area_legacy_tests.rs` contains a copied independent r132 parser and compares parsed statements, exact signature fragments, result rows, executed order and error strings for non-Area input. Cases include unsorted table/unit-label definitions, unknown metadata, skipped disabled tables/text/images, signed-zero ties, malformed coordinates, grid errors, size errors and the 10,001st-region budget/error precedence. Baseline source/archive hashes are recorded in `artifacts/rea/area-calculation-legacy-oracle-20261019.json`.

Twelve integration tests in `tests/area_calculation.rs` cover parent-slot versus global order, local Y/X order, multiple Areas, original child geometry/IDs/resultRef, presentation-only cache reuse, child disable transitions, atomic nested/protected/unknown handling, global prerequisite publication, old payload preservation, Through/Active calculation prefixes, external contexts, empty Areas, grid snapping and bounded atomic failure. Two independent non-Area oracle tests pass. Compat all-target Clippy with `-D warnings` passes. The first broader compat pass also passed; final workspace/OS/native/PDF/performance integration is recorded by the owning integration batch, not inferred from these focused tests.

These tests establish our source, ordering and calculation contracts. They do not establish complete Area UX, pixel parity, original runtime timing or complete protected/nested execution support.


## Area-aware raw worksheet label preparation

`editor_bridge::worksheet_xml_with_areas(raw_math, native_regions)` prepares a calculation copy for Area documents. Plain inputs are raw `(id,left,top,MathML)` tuples; native inputs are complete namespace-complete WS region fragments, including Area/solve-block wrappers. It must run before outside identifier inference. Building outside math first can permanently annotate `m` as UNIT before an Area defines the variable `m`, causing an incorrect result even if the combined worksheet is labelled again.

The helper bounds the whole source, records exact generated tuple/context ranges, parses the composed XML, and reuses the evaluator's Area sibling-slot order. It first gathers active direct global declarations, including those in supported Areas. It then applies the existing `label_parsed_fragment`/table label policy in that hierarchical order. Raw tuple contexts become the ordinary math wrapper only after labelling; native math wrappers and unknown source siblings remain intact while their expression ranges are patched. Global names from disabled children and unsupported Area containers do not enter the environment. Unknown Area wrappers remain one raw container fault; children are not partially labelled into active bindings.

Native math uses a namespace-complete context for the existing labeller. Namespace aliases, resultRef, local coordinates, region IDs, comments outside structural math, formatting siblings and XAML literal subscripts are retained. A conflicting `ml` binding is rejected conservatively rather than allowing generated `ml:id` patches to change namespace meaning. Exact complete-region/context ranges, top-level and active calculation ID uniqueness, nonoverlapping patches and both pre/post-label8 MiB guards prevent fragment escape, partial output or unbounded assembly.

The new helper has no cached or viewport-lazy mode. Solve-block source remains under the existing solve parser and is not newly rewritten or given additional implicit exports. The ordinary cached builder is unchanged; calling this helper without native regions delegates to it byte-for-byte. This helper prepares source/labels only and runs no expression.

Nine additional tests cover an independent explicit-label whole-result oracle (`Area m:=7` followed by an unlabelled outer `m=` must yield dimensionless7), source/slot ordering, active late globals and disabled/unsupported exclusion, namespace aliases and descendant `ml` conflicts, explicit UNIT/literal-subscript preservation, table labels, solve source guards, IDs, tuple-boundary injection and source budgets. The app's actual authoring/commit/native restore tests remain part of the integrating batch.
