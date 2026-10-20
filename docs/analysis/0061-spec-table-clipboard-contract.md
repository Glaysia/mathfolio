# Specification-table clipboard and clearing contract

This corrects the clipboard interpretation in [0059](0059-spec-table-structural-commands.md).
`CreateMatrixFromTableCells` supports arbitrary table rows, but its native Copy
caller dispatches header/unit selections to a different clipboard path. A helper's
capabilities alone do not establish the application's behavior.

## Original static evidence

The REA skill was used with retained managed metadata and supplemental static CIL
decoding. No original executable, GUI, screenshots, COM or CLR execution was used.
Selected evidence, call identities, MVIDs and IL hashes are retained under
`artifacts/rea/spec-table-clipboard-20261016/`. The decoder reads DLL bytes without
loading the assemblies. It reconstructs method instructions; it does not recover
original source code or establish runtime observations.

| Assembly | SHA-256 | REA evidence |
| --- | --- | --- |
| `Ptc.EquationEditor.dll` | `d419032c0e99a62eab2f278e571a338d8c4b9662655af8ab462d482a028c0223` | `ev_419ab24312df266f2e58ff6b91d32e0261a9a2bba9105ba09ebecb7517b43fdf` |
| `Ptc.Controls.EquationControl.dll` | `0b6fd580eb6cc183f17b5b5e0967226bbafc5e12efc6da809f44cd16416bf9d1` | `ev_389338a9845c7ea83dfd310922316cf92ca1637bdbac6602a01c4690e52b8349` |
| `Ptc.Controls.Worksheet.dll` | `c1a70ec8007228ff119d4a31b36b9062132be3a10c771459c1dbe3fd0a7baddd` | `ev_ca6ee361e44f5b877e72b99111bddb37dcf24e4dd6163ce516a16f3ad2b2611b` |

The relevant MVIDs are `d8627dca-3258-470c-b4d1-c00bbe2f2441`,
`cf1d7cb5-c73f-4618-8e6b-b83959d13b98` and
`44f31187-dcfc-4afd-a7d6-50fbee8dc361`, respectively.

### Copy has two payload types

`SpecTableControl.HandleCopySelection` (`0x0600031f`) converts selected corner
indices to an inclusive rectangle, then branches on `Top > 1`:

- Body-only selection: `CreateMatrixFromTableCells` (`0x06000af6`) clones the
  mathematical roots in row-major order, then `IClipboard.SetSExpression`
  receives that matrix expression.
- Header/unit selection: constructs a separate `SpecTableControl` with
  `max(Bottom - 1, 1)` data rows and `Right - Left + 1` columns. It clones the
  header only when `Top == 0`, the unit when `Bottom >= 1`, and data rows
  `2..=Bottom` when `Bottom >= 2`. It raises `CopySingleItemNoImageEvent` with
  the new table. It does not pass those selected header/unit roots through
  `SetSExpression` as matrix rows.

Thus header-only copy creates a table with copied headers, blank units and one
blank body row. Unit-only copy creates blank headers, copied units and one blank
body row. A selection beginning at the unit row and extending through data
copies those units and data while leaving the new headers blank. A table copy
uses new column/region objects rather than cloning source persistent metadata,
result references or column formatting. The worksheet paste formatting context
is separate from the copied mathematical expressions.

### Cut, Delete, Backspace and Clear Cells differ

`SpecTableView.OnCut` (`0x06000358`) calls Copy, then
`HandleDeleteSelection(Left=0)`. Backspace (`0x06000367`) passes the same direction;
Delete (`0x06000366`) passes Right=1. The direction controls focus placement.
Both enter `SpecTableModel.HandleRemoveOrClearSelection` (`0x06000ae7`), so their
content removal obeys the same existing full-column/full-row guards and minimum
retention rules documented in 0059. Cutting an entire sole column can copy
successfully while changing no source content.

`OnClearCells` (`0x06000364`) passes Indeterminate=4 for a selected rectangle.
That direction skips both structural-deletion branches in `0x06000ae7` and
executes the clear-content command (`h.Do`, `0x06000b25`) for every selected cell.
The table dimensions remain unchanged, including its minimum row and column.
The command then invokes `RequestClearSelection` (`0x06000aee`, field `i`), fires
table-changed and requests focus for its prior active equation. Range selection
does not remain active after Clear Cells.

Without a rectangle, Clear Cells calls the active equation's `ClearContent`
through `0x06000365` and `0x06000ae6`. `EquationModel.ClearContent` (`0x060001ac`)
dispatches helper `0x060001f1`, which replaces the equation root with a default
empty root and resets navigation. This is separate from Clear Math Format;
there is no format-reset call in the inspected content-clear path.

The table's editable command list (`0x06000350`) binds Clear Cells alongside
row/column commands. Installed help
`Help/en/PTC_Mathcad_Help/to_edit_arrays_and_tables.html`, SHA-256
`85cc675cc698b30fc4fd20655032f63743cbf94bc196323c148a6a3ac311beb0`,
places editing options on Matrices/Tables > Rows and Columns and describes
Clear Cells as clearing selected contents without changing matrix size.
An exact original context-menu placement is not established here.

### No evidence for automatic grid-distribution Paste

The inspected table editable command list does not bind Paste. An active
equation's `EquationControl.OnPaste` (`0x06000184`) uses the general visual math
paste command, then `EquationModel.Paste` (`0x0600014f`). Its normal editing mode
`l.aw` (`0x060000a7`) inserts a clipboard mathematical node at the active
navigation location; selection mode `m.aw` (`0x06000285`) replaces a selected
mathematical segment. These are equation-expression operations, not traversal
over table cells. `WorksheetPasteHandler.ProcessClipboardContents`
(`0x06000937`) separately processes whole Prime objects and S-expressions, and
`WorksheetViewModel.Paste` (`0x06000aa8`) adds resulting items to the whiteboard.

Automatic distribution over a body rectangle, growth to fit a matrix, and
header/unit range Paste were not established. Earlier `paste_matrix` is retained
as an explicit bounded document transformation helper; it is not evidence of
an original UI operation. The application integration removes automatic use of
that helper for ordinary formula Paste. Behavior when a whole table rectangle
has focus is kept distinct from selecting mathematical nodes within one cell;
no unobserved range-replacement policy is fabricated.

## Independent implementation

Only `mathcraft-document/src/spec_table.rs` is changed in this document-model
step. The versioned worksheet representation remains opaque native XML.

- `SpecTableClipboard::{Matrix(PortableTree), TableXml(String)}` is our inert
  tagged transport representation. It is not Prime's binary clipboard format.
- `copy_selection(display_rows, columns)` implements the caller's two branches
  and exact bounds above. New table payloads use native-shaped column matrices,
  preserve selected expression spelling and blanks, and have no copied source
  result references, measurement caches or column formatting overrides.
- `cut_selection` validates and constructs the copy before mutating source,
  then uses the existing native remove-or-clear policy. It returns the payload
  and a separate changed flag. Caret direction/focus is an application concern.
- `clear_selection` always clears selected roots; it never removes dimensions.
  Existing column settings, unknown metadata, comments and unselected bytes
  stay intact. Repeating it on blank cells is a no-op.

Selections retain existing row, column, cell-count and 8 MiB XML limits. Invalid
ranges leave source unchanged. Clipboard callers must still validate incoming
transport size and parse an incoming table; serde deserialization alone is not
a validation or execution boundary.

## Verification and limits

Focused document tests passed: 16 tests, one explicit-file probe ignored. Added
regressions cover header-only, unit-only and combined Copy bounds, minimum blank
data slots, body matrix dispatch, typed transport serialization, native saved
matrix shape, fresh copied metadata, Cut/minimum no-op, Clear-versus-Delete
structural differences, byte preservation and atomic invalid ranges.

The explicit selected-file probe also passed on copied `1015_LCC_S.mcdx` and
`1016_LCC_S.mcdx`: each full Copy has six columns, ten body rows, all 55 values,
five blank cells and matching header/unit trees. Clear preserves the 10×6 shape;
Cut keeps the original last column under the native minimum guard. Subsequent
edit/row/column roundtrips pass and both input files remain byte-identical.
No user source was overwritten and no original Prime clipboard interoperability
or reopen is claimed. Application/OS integration checks are separate.
