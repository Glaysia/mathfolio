# Inert saved Area child presentation

2026-10-19. This is a Mathcraft canvas JSON model for reopening displayed Area results without executing worksheet input. It extends the source ownership and calculation contracts in [0132](0132-area-contract.md) and [0133](0133-area-calculation.md). It does not claim that the original Mathcad result cache uses this JSON layout. No original application, screenshots or managed runtime was used.

## Format and API

`canvas::SavedNumericSource::AreaPresentation(String)` stores one JSON-encoded `SavedAreaPresentation { rows: Vec<SavedAreaChildResult> }`. Each child contains `region_path: Vec<usize>` and an existing `SavedCalculationRow`. The row's `region_id` is the owning Canvas Area ID; the element-child path identifies a direct math child region relative to the authoritative namespace-complete owning WS region fragment. For a simple fragment the first child path is `[0,0,0]` (Area, regions, region). Native child IDs and result references remain in the original source XML; they are not allocated as new Canvas regions.

The existing parent row stores only the bundle as `numeric_source`, with no direct display/symbolic payload. Child rows retain the existing Pending/Synchronized status and optional text, warnings, error, symbolic and numeric presentation fields. Subsets and empty bundles are allowed. The same row/cache structs are reused: no required fields were added to old JSON, and old optional numeric-source omissions serialize identically.

The public types are re-exported from `mathcraft_document::canvas`. `SavedAreaPresentation::parse_for_region(data, owner)` validates a bundle against its Opaque `kind == "Area"` source. It does not establish signature freshness or validate the entire CanvasDocument; the caller still uses the ordinary document validation and source-signature guard. `numeric_cache::display` returns None for the parent bundle. Child display restoration belongs to the app's Area normalization path.

## Validation and byte accounting

Document validation retains the existing cache signature/row-ID checks, result/error conflict, warning, per-source and aggregate budget order for ordinary rows. The new Area-specific validator runs after the existing parent numeric-source budget checks. Its guards are:

- Opaque Area owner and a namespace-correct owning WS region fragment containing one Area.
- One direct `regions` collection; source-bound, unique direct math-region paths and a matching owning Canvas ID for every child row.
- Strict serde `deny_unknown_fields` on the envelope, child wrapper and reused row/numeric source. JSON uses the normal bounded serde nesting parser. A child AreaPresentation variant is rejected without parsing its inner string; recursive bundles are never interpreted.
- At most4096 saved child rows, at most48 path elements, at most1024 warnings per row, a4 MiB parent source string and the existing8 MiB total saved-presentation budget. Native XML is limited to8 MiB and250,000 DOM nodes; structural expression scanning uses an explicit stack and48-level depth guard.
- The existing error/result conflict applies to child rows too. Source expressions containing a placeholder cannot carry a Synchronized display, symbolic or numeric result. Pending rows may retain their previous presentation payload, matching the existing manual-calculation contract in0032/0042; they may also be empty or retain an error. Pending payload is inert display, not an engine binding. The usual placeholder inside an evaluation's `unitOverride` is excluded from this incomplete-input check.
- Nested Area source and invalid/ambiguous source ownership cannot be used to bind cached children. Unsupported math is not decoded or evaluated by this validator: it remains inert/read-only app content.

All embedded child strings already appear in the JSON envelope string. Only that outer string is added to the document's aggregate presentation-byte count; child text/numeric strings are not counted again. Per-child source length remains guarded. The calculation/source-signature implementation is unchanged, and result-cache content does not affect either active or full calculation signatures.

## Freshness and integration

A well-formed cache with a stale source signature is not an instruction to calculate. The existing `saved_calculation_cache()` guard returns None; the app restores Pending state without executing expressions. Structural corruption is different: absent paths, a wrong owner, recursive variants or a Synchronized result on a newly blank placeholder are invalid document cache data. A source-edit/save transaction must mark that child Pending (preserving any previous display) or retain its error before saving the bundle. This module does not weaken validation merely because the outer signature is stale.

The bundle contains presentation only. It does not hold a binding checkpoint, calculation context, cancellation token or executable source. Source XML, child location/resultRef ownership, pending calculation scheduling, formatting, print display and app restore/generation behavior remain with their existing owners.

## Validation evidence

Ten focused document tests cover byte-stable old rows and empty bundles, source-bound paths/IDs, rejected duplicate/non-math paths and JSON unknown fields, nested variants, PH/Eval(PH)/definition-placeholder fresh-result guards and previous Pending payload preservation, result/error/warning precedence, wrong owners, malformed/nested/deep XML, row counts,4 MiB/8 MiB budgets without double-counting, unchanged signatures, stale-cache rejection by signature and a real `.mathcraft` save/load roundtrip. They also demonstrate that the document layer accepts ordinary numeric presentation strings as inert bytes rather than executing or decoding them into engine values. Final workspace and app/native restore evidence belongs to the integrating batch.
