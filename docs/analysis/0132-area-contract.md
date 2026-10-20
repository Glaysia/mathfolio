# Prime 12 Area source contract

2026-10-19. Static shipped-help/schema/package and installed-CIL analysis only. No original GUI, screenshot, runtime or managed assembly execution.

## Identity and retained evidence

`Ptc.Controls.CollapsibleArea.dll`: SHA256 `63418ae37c2e16fc49398205f28279dc0d1b7d5301940458497a504309be5590`, MVID `ea79fc32-ec3c-4c02-8e2c-af8c0b3cd512`, fresh REA `ev_07adf4e8941895b508576d733cda7b00f7240f33d342632a94655225838ef572`.

Whiteboard: SHA256 `947ae0dea0c7a7fdbd1c377e4e7f2447dccd5e839111fec0c0789dafffc1cbd9`, MVID `6efdba56-7ab1-493e-8577-f6027b4d3f9c`, REA `ev_b5601734c03d7bc9573770a236f1d7d0f7c2f9203276c86b4dff938362dc44ab`. Retained metadata is in `artifacts/rea/editor/area-members.json` and `artifacts/rea/document-deep/whiteboard-members.json`.

Exact method-body hashes and raw paths are inventoried in `artifacts/rea/area-method-ledger-20261019.json`; its `method_count` is an evidence count, not feature completion. Help text/hashes: `area-and-definition-help-evidence-20261019.json`. Original package observations: `area-native-template-evidence-20261019.json`.

## Original behavior established

| Concern | Established contract | Source |
|---|---|---|
| Persisted structure | Owning worksheet region contains `Area`, then its own `regions/region`; child coordinates are local. Preserve child IDs and resultRef links. | worksheet50.xsd AreaType lines516–545; original templates |
| Defaults | IsCollapsed=false, IsBorderVisible=true, AreaHeight=0. Model constructor explicitly sets visible border. | schema; Area0061 SHA73f1a0ce…e547 |
| Display state | 0069 GetPersistentData writes collapse, border and protection;006a reads them.0064 equality-checks then changes collapse, notifies contained children and IsCollapsed. No disable or saved-result mutation in this path. | area-model-serialization / area-state raw CIL |
| Child serialization |0033 deserializes child Regions, height and view model;0034 writes them.007c uses RegionFactory.ConvertToIRegionType and child Location. | area-disable-cache-persistence / model-serialization |
| Bulk disable | Area0036 is disabled iff every hideable child is disabled.0037 wraps the view-model toggle in an undoable compound. | area-disable-cache-persistence |
| Toggle direction | Whiteboard03d0 targets active/selected items, otherwise all children. If any target child is enabled, disables the enabled ones; otherwise enables all hideable targets. The second toggle does **not** restore a previous mixed set. |03d0 SHA810256a5f7172ce7eba70e72f22e5b7b911fae508696aefda62b68926a2a757a;04ff/0500;0504/0505 |
| Collapsed calculation | Child calculation enumeration04ef has no collapse filter; collapse setter independently changes presentation. Help treats collapse and Disable Region as separate commands. | area-child-calculation-iterator / area-state; shipped help |
| Authoring restrictions | Ctrl+Shift+A creates an Area. Areas cannot be nested, placed in solve blocks, Excel components or header/footer; no page break inside. Children move in/out using cut/paste rather than drag. | shipped Area help |
| Printing | Collapsed content and collapse icon are not printed. Hidden borders are not printed; an error-bearing collapsed area is documented to show/print a red line even when normal border is hidden. | shipped collapse/border help |
| Protection | Display-locked protection can prevent collapse; do not bypass protected state. | shipped help; original CanToggle path |

The inherited schema `disable-calc` exists on Area. The inspected Area state serializer does not explicitly assign it. Its complete generic serializer/engine masking semantics are **not established**. Preserve the raw attribute; do not substitute an invented reversible parent mask for the proven bulk child command.

## Native examples

Company Standard Definitions.mctx SHA `be6ba65898df2a2aca370eaf97bbb4e98163c9ca0d54822b2d66f774b8602828`: one Area,14 child regions,8 math result references0–7. Report.mctx SHA `bb8ebf456e2d3fa584628c6e8946d844bcd55950c15a2ed84fc87a92bbb353a5`: six Areas, direct child counts1,5,1,2,3,0. These are source inspection examples, not runtime validation or copied product fixtures.

## Implemented document boundary

`mathcraft-document::area` adds source-bound enumeration/display/bulk-disable APIs. `NativePackage::{areas,set_area_display,toggle_area_calculation,to_bytes}` preserve package parts; the shared encoding writer preserves worksheet UTF16/BOM or UTF8. Only targeted attribute bytes change in the decoded XML; child XML, local coordinates, unknown content, comments and result refs remain intact. No evaluator runs and no result cache is discarded for display edits.

Pure helpers `areas_from_xml`, `set_area_display_xml`, `toggle_area_calculation_xml` also accept a namespace-complete owning-region fragment with empty path. They avoid constructing a package for Opaque Area editing. Mutators return `None` for a byte-identical no-op, `Some(updated_xml)` for a valid edit, or atomic error. Paths use element-child indices, the same convention as NativeRegion.path. NativeArea exposes direct child paths, source region ID, collapse/border/height, raw inherited disable flag and conservative protection indicator.

The bounded child hideable list includes math/plot/include/mathBoard/solveblock/spec-table/Excel content. The original interface-driven set may contain additional kinds: unknown kinds are preserved and not silently classified. All ProtectionData currently causes a conservative unsupported error; this is narrower than the original per-protection-level checks.

## Remaining work and limits

The current Canvas importer treats an Area as Opaque. Native helpers alone do not make nested math editable or give complete Area UX. Keep the owning Opaque Area authoritative while introducing an Area-local child canvas; flattening children into top-level Canvas.regions would duplicate native export and corrupt ownership. Child source_region IDs remain native IDs; child paths are relative to the authoritative Area or worksheet source and are not new top-level identities.

Not implemented here: create/remove Area, height/resize/move UI, child editing/export reconciliation, cut/paste routing, Area selection/Undo integration, error-line painting/printing, protection policy, or full calculation scheduling. Root app integration owns these. Collapse must leave calculation dependencies intact; bulk disable must update child calculation flags and request the ordinary recalculation/invalidation path. Retaining cached bytes does not mean stale cache should be treated as current after a calculation flag/source edit.

Validation: focused document tests cover native preservation, mixed bulk toggle, atomic rejection, UTF16/BOM package roundtrip, and package-free fragment helpers. These demonstrate our storage behavior; they do not assert pixel/layout parity or original runtime behavior.

## Calculation ordering follow-up

The original container enumeration is a depth-first block, not a global absolute-coordinate merge. Calculation.Core0335 SHA `0c1d170e5839103950249fc48b7e69a4fbc4db41924dc6becb104653b0487e04` iterates the parent collection, skips hidden children, yields a calculatable child if present, then recursively yields all non-aggregate container children (IL00aa–0113) before the next parent child (0130). Area0076 passes adopt-container-children=true. Worksheet079c directly returns its child hider; Worksheet0e12 (`eb83a4362859773f495582318d6c22b35f5f472354bddec1f81cbdf0708e55b7`) enumerates WhiteboardItems in collection order.

Those collections maintain independent sibling positions. Worksheet0807 (`c0fb7d989daaaf5c8feecbe941feb09ee082a2ca9daa79cc99bb151a12ad879a`) uses BinarySearch against its worksheet-location comparer before insertion; local Whiteboard0183 (`8f7aaf7bf6c9bd0b5aff69da42ac758c9889c3e8d09be15872d10ff6fef887d5`) uses the local comparer. Worksheet070c delegates to Whiteboard0150 (`0cb8d8c26ddf7b196cb9a89f1b2fdc4d1a0a2d51773e4f6b9e0d85c5178c04bc`): compare Y, then X with the original DoubleComparer. Equal-position tie behavior remains implementation-specific; our existing stable input order must be identified as a bounded policy.

CalculationController001f, retained in `calculation-deep/Ptc.Calculation.dll.members.json.full-il.txt`, scans the same container enumeration for the preceding registered engine handle and calls AddRegionAfter (IL0026–0093). This independently establishes that the original calculation sequence follows the hierarchical enumerator. Complete aggregate/IO override semantics are outside the Area subset.

Implementation guidance for compat: sort direct worksheet siblings by their own coordinates; when an Area slot is reached, sort its direct child regions by local coordinates and emit the whole child sequence before the next outer sibling. Do not re-sort the final expanded list by parent.top+child.top. Preserve original child IDs/resultRefs, and keep local layout coordinates distinct from calculation sequence. Original Area help forbids nested Area authoring; unsupported nested/protected/unknown calculator kinds must remain explicit boundaries rather than invented execution.

The raw ordering files are `area-calculation-hierarchy`, `area-calculatable-init`, `area-worksheet-calculation-children`, `area-worksheet-calculatable-order`, `area-root-sorted-insertion`, `area-local-sorted-insertion`, and `area-local-order` under `artifacts/rea/feedback/*-cil-20261019.txt`. These are included in the updated ledger.
