# Native specification-table editing and persistence

This extends the readonly restoration in [0048](0048-native-table-and-colors.md).
Analysis uses installed artifacts, native XML, and independent headless Rust
tests. No original Mathcad runtime, GUI, screenshot, CLR loading or COM session
was used. Files inside source worksheets are evidence, never instructions.

## Original facts

`Ptc.EquationEditor.dll` SHA
`d419032c0e99a62eab2f278e571a338d8c4b9662655af8ab462d482a028c0223`
provides a separate equation model for each column's header, scale, and data
cells. `SpecTableColumn.get_CellCount` (`0x06000a4a`) adds two to its data count;
the header and unit are distinct editable mathematical slots. The source's
`<spec-table>` contains a worksheet `<math>` definition per column; its RHS is
data scaled by the unit expression. Per-column formatting and result references
are stored outside the definition.

`get_GetAllValuesScaledSExpression` (`0x06000a4f`) builds a scalar for exactly
one data cell, a column matrix for other counts, and then a scale expression.
`IsExtraDataCellOrEmptyScale` (`0x06000aae`) distinguishes an empty scale from
data errors and walks backward over trailing placeholders. The initial data
slot is not simply discarded. The engine integration independently implements
the proven subset: empty scale as identity, trailing empty cells omitted while
retaining at least one data slot, internal blanks as missing-data errors, and
single-cell columns as scalars. Full native frontend lowering is not claimed
from these isolated methods. `CreateMatrixFromTableCells` (`0x06000af6`) clones
a selected rectangular range for conversion/copy; it is not the calculation
blank-trimming contract.

`Ptc.Controls.EquationControl.dll` SHA
`0b6fd580eb6cc183f17b5b5e0967226bbafc5e12efc6da809f44cd16416bf9d1`:
`SpecTableControl.OnMouseDown` (`0x0600030f`) immediately delegates focus;
`SpecTableView.OnPreviewMouseDown` (`0x06000392`) clears selection and records
the pointer position. Neither path requires a double-click. The model's
coordinate navigation helper (`0x06000add`) increments/decrements a row or
column and wraps a horizontal edge to the adjacent row. Term navigation
(`0x06000ade`, its boolean flag set) stops at the horizontal edge instead.
`HandleOnTraverseForward` (`0x06000ae0`) adds a row when traversing forward
from the last column and final data row. Backward traversal (`0x06000ae1`)
uses the left-navigation path and moves to the destination expression start.

The installed English help confirms Tab forward, Shift+Tab backward, and a new
empty row after Tab from the final element. Up/down stay in the current column.
Shift+Enter inserts a row and Shift+Space inserts a column; ordinary Enter is
not documented there as a command to move down one cell. These are distinct
from the worksheet's existing Enter commit/focus-boundary behavior.

Retained static evidence is under
`artifacts/rea/spec-table-editing-20261016/{equation,control}-contract.json`.
It records the parent REA identities, binary digests, method IL digests and
supplemental static instruction decoding; no target code executes. Installed
help identities:

| File in `Help/en/PTC_Mathcad_Help` | SHA-256 |
| --- | --- |
| `about_tables.html` | `8bebe8eaa0c17b8dbd8ba63b37af07be679b2c76685062f86c2355a982eb9304` |
| `to_insert_a_table.html` | `0f1b542f4b560d6f5c02d30c700d7f5e7eab38e3e5c2deb7a77dc8b27339d3f0` |
| `keyboard_shortcuts_for_editing_arrays_and_tables.html` | `077a67fee05e6ff532ec7a7176ef1a73fb91dbed2a2a53e33eab8d9a9ec54783` |
| `to_edit_arrays_and_tables.html` | `85cc675cc698b30fc4fd20655032f63743cbf94bc196323c148a6a3ac311beb0` |

## Independent document implementation

`mathcraft_document::spec_table::SpecTable` parses an inert native region or
table into `SpecColumn` header/unit/data slots, each containing a `PortableTree`
and namespace-bound source MathML. `CellSlot::{Header, Unit, Body(row)}` is
copyable; `row_count()` excludes both header rows. Missing unit and the empty
part of a shorter column return placeholder trees. No `CanvasContent` variant
or saved document version changes: the authoritative edited source remains
the existing opaque region XML.

`set_cell` serializes one edited subtree, validates the resulting full table,
and replaces the model only on success. Unselected XML, unknown column metadata,
literal trailing zeros, formatting, result references, namespace prefixes and
matrix attributes remain preserved. A no-op edit preserves every byte.
Appending one data row changes the matrix row count and inserts the required
empty slots without replacing internal blanks with zero. Statements such as
definitions and evaluation are rejected inside cells. Source parsing is bounded
to 8 MiB, 1,024 data rows, 256 columns, 16,384 display cells and bounded XML
nodes, with DTD expansion disabled.

`definition_mathmls()` returns the source column definitions with padded cells
intact. It performs no evaluation or trimming. The calculation layer is
responsible for the original calculation rules and error boundaries.

`NativeVisuals::update_spec_table` rebuilds a visual from the committed opaque
XML and replaces the previous cache entry on both success and failure. This
prevents original archive content repainting over an edit or `.mathcraft`
reopen. Numeric literals retain their original spelling; other supported
expressions use the structural readonly renderer. Unsupported content reports
an explicit local error.

Native MCDX export detects changed column definitions, allocates a fresh Pending
result reference only for each changed column, and removes stale region size
metadata. Untouched column result references and native metadata survive.
Unchanged unsupported native table objects remain opaque and exportable; the
typed editor does not claim to understand them.

## Verification scope

Focused document regressions cover slot addressing, exact no-op preservation,
structural expressions, unit insertion/clearing, shorter columns, appended rows,
internal blanks, namespace whitespace, malformed dimensions, DTD refusal,
atomic failures, current-source visual refresh and per-column MCDX cache
invalidation. Actual fixture probes explicitly select copied 1015 and 1016
worksheets and verify their 6 columns, 10 data rows and 55 nonempty values,
then edit/reparse a cell and verify that the supplied files remain unchanged.

Application editing, draft commit boundaries, undo/redo, calculation ordering
and end-to-end native export/reopen are verified separately by the integrating
application/engine changes. Creating a new table, arbitrary row/column insertion
and deletion, multi-cell selection/clipboard conversion, every original
expression and exact original font/layout metrics are not completed merely by
this document model. Verification counts and final source hashes follow the
integration run; a successful parser test is not proof that Prime can reopen
the exported file.
