# Area child math editing and native save

2026-10-19. This implements the source-preserving document boundary needed by an Area-local formula editor. Original Area identity, schema, child-local coordinates, calculation ordering and known limits are in0132. No original GUI/runtime or CLR assembly execution was used.

## Ownership and API

`crates/mathcraft-document/src/area.rs` exports:

- `area_math_children_xml(xml, area_path) -> Result<Vec<NativeAreaMath>,String>`: direct math children only. Owning namespace-complete region fragments use `area_path=[]`; worksheet input uses the original NativeRegion.path convention.
- `NativeAreaMath` exposes child `region_path`, original optional `region_id`, local finite `left/top`, optional `result_ref`, child `calculation_disabled`, namespace-complete expression `mathml`, `math_xml`, and owning child `region_xml`.
- `replace_area_math_xml(xml, area_path, child_path, mathml) -> Result<Option<String>,String>`: validates a supported editor-bridge tree, replaces only that expression, and returns None for a semantic no-op. Unsupported/malformed input, wrong owner/path or protected Area is an atomic error.

The authoritative Opaque Area XML owns its children. Do not also append them to top-level Canvas.regions. Root app owns child selection, CPU formula editing, whole-document Undo, draft/Enter and result invalidation. This module does not evaluate cached MathML, insert regions, resize/reorder children or fabricate unsupported values.

## Edit and save boundary

A changed expression clears its old math `resultRef` and `error`, and its owning child region's `actualWidth/actualHeight`. This is an intermediate editor representation, intentionally without the schema-required resultRef. It prevents an edited child from resolving the old saved result. The helper preserves child IDs/local coordinates, formattingOverride/resultFormat and unknown siblings, other child XML/result refs and package cache parts. Source labels/units remain in the supported PortableTree serialization. Same-tree edits return the exact original bytes rather than rewriting numeric spelling/metadata.

`export.rs::edit_region` now handles Opaque kind `Area`. It matches direct children by original region-id, checks expression structural/portable equality against the original source, and uses the **existing shared ResultIds allocator** for every changed child. Thus exported math has a fresh integer resultRef and matching Pending resultData; no private marker, fake attribute or second allocator is introduced. The shared allocator starts beyond all original cache IDs and native region refs. Missing/different refs on an unchanged expression are restored from the original source so a raw Opaque edit cannot attach unrelated cached output.

Adding/removing children, changing a child kind, missing/duplicate/foreign child IDs, malformed MathML and protected Area edits are unsupported. Unknown non-math children survive. Changed child measurement attrs and outer Area-region actual dimensions are cleared so old text geometry cannot be reused. AreaHeight, collapse/border state, local coordinates and IDs remain source-owned.

The existing global export contract remains: `export_into` invalidates cached resultData values/statuses after worksheet export, then appends new Pending rows. Consequently package XML helpers preserve untouched cache bytes, whereas full MCDX export still applies the repository's broader stale-cache policy. This patch does not claim that dependency-related caches can remain current after an Area expression edit, and does not silently weaken that policy. Existing global allocator/content-type handling is reused.

## Native evidence and interpretation

Original Area0033/0034 serialize nested Regions, AreaHeight and persistent state;007c delegates each child to RegionFactory and preserves its local Location (0132 raw ledger). Worksheet50 AreaType contains `regions/region`, while math carries a resultRef used by the global result part. Those facts justify preserving hierarchy/IDs and using ordinary math result allocation for edited children. Clearing stale error/measurement metadata and the intermediate no-ref representation are explicit reconstruction policies shared with existing normal math export, not a claim that the original GUI serializes the same transient bytes.

## Verification

Document tests cover source-identity/local-coordinate retention, semantic no-op, wrong-path/malformed rejection, modified-only ref invalidation, preservation of a neighboring saved reference, fresh native result allocation, exactly one Area owner with two nested children (no flattening/duplication), unknown formatting payload preservation, custom binary part retention and UTF16/BOM output. Existing Area tests cover collapse/disable and package-free fragment mutation. Validation completed: full document suite82 passed/2 ignored, all-target Clippy with -Dwarnings and formatting passed; these tests establish our file behavior, not original runtime reopening or pixel parity.

## Readonly child visual addendum

`native_visual::area_child_visuals(package, area_region_path)` returns `Result<AreaChildVisuals,String>`, where AreaChildVisuals is a map from JSON element-child path to `Result<RegionVisual,String>`. The API validates the requested Area through NativePackage::areas, traverses only its direct child regions, and removes the owning region path prefix from each key. Thus a global Area owner path `[0,1]` and global child path `[0,1,0,0,2]` produce key `[0,0,2]`, exactly the child path used when editing the namespace-complete owning-region fragment. Source region IDs do not participate and cannot collide with top-level IDs or other Area children.

Existing `read_region` supplies math background/dimensions, rich text/FlowDocument tables and specification tables. Relationship resolution remains worksheet-owned and inert: no WPF/XAML execution or external source loading. Unknown or empty child payloads have explicit Err entries instead of disappearing or becoming invented output. This map is intended to be populated once on open; an edited child math uses its current source tree rather than treating old package presentation as authoritative.

`NativeVisuals::from_package` is unchanged and remains root-only. The regression checks repeated source IDs across root/Area children, owner-relative keys, rich text table/style/color preservation, math background, specification table reconstruction, explicit unknown errors, unchanged root visual results and byte-identical worksheet source. Native visual tests5 passed/1 ignored, all-target document Clippy -Dwarnings and fmtcheck passed. No new original-package visual oracle count or original runtime validation is claimed.

The companion `area_child_visual_entries(package, area_region_path)` returns entries with relative `region_path`, JSON `key`, finite source-local `left/top`, source content `kind`, and the same per-child `visual` Result. This supplies AreaUI placement without reparsing XML or flattening the document. The map convenience API delegates to this collector; callers should select one API per open. Missing/nonfinite local coordinates are explicit errors, never fabricated zero positions. The regression additionally checks text local9.6/9.6 and unknown kind retention. Focused entry regression and all-target Clippy passed after this addition.
