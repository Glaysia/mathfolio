# Specification-table structural commands

This follows [0053](0053-spec-table-editing.md). Evidence comes from native
worksheet XML, installed help and static REA of the shipped assemblies. No
original application, GUI, screenshots, COM or CLR execution was used.

Clipboard correction: [0061](0061-spec-table-clipboard-contract.md) inspects
the actual Copy/Paste callers. Header/unit selections create a table object;
only body selections copy a matrix. The earlier application grid-distribution
Paste interpretation was unsupported and has been removed. The `paste_matrix`
model helper below is an internal editing utility, not an original UI claim.

## Original contracts

`Ptc.EquationEditor.dll` SHA
`d419032c0e99a62eab2f278e571a338d8c4b9662655af8ab462d482a028c0223`:

| Method | Native fact |
| --- | --- |
| `SpecTableModel..ctor`, `0x06000a96` | Calls the dimensioned constructor with one data row and one column. Header and unit are two additional rows. |
| `SpecTableColumn` native XML reader, `0x06000a52` | Requires a definition, a scale factor and a column matrix with `cols=1`, `rows>0`. |
| Native XML writer, `0x06000a53` | Always writes `define(header, apply(scale, matrix(rows=DataCellCount, cols=1, cells...), unit))`, including a 1×1 matrix. |
| `AddRow`, `0x06000adb`; command `c.Do`, `0x06000b0c` | Acts only on data rows. Explicit Up inserts before the current row; Down and indeterminate direction insert after it. Indeterminate direction checks the equation's caret edges first. |
| `AddColumn`, `0x06000ada`; command `b.Do`, `0x06000b07` | Explicit Left inserts before; Right and indeterminate direction insert after the current column. |
| Row removal helper, `0x06000ace`; `f.Do`, `0x06000b1b` | Ordinary row deletion removes one row across all columns. With one remaining data row, clear its cells instead; already empty is a no-op. |
| Column removal helper, `0x06000acf`; `g.Do`, `0x06000b20` | Ordinary deletion removes a column. With one remaining column, clear its header, unit and data instead; already empty is a no-op. |
| `HandleRemoveOrClearSelection`, `0x06000ae7`; `i.Do`, `0x06000b2a`; `j.Do`, `0x06000b2f` | Selected full-row/full-column Delete has separate guards. Full columns take priority; deletion stops at one original column. Full data rows stop at one original data row and refuse header/unit deletion. Selecting all leaves the original last slot unchanged, rather than clearing it. Partial selection clears content. This differs from explicit deletion commands above. |
| `CreateMatrixFromTableCells`, `0x06000af6` | Clones each selected cell's mathematical root in row-major rectangular order, including header/unit when selected. |

The XML writer's matrix storage must not be confused with the calculation
S-expression lowering in `get_GetAllValuesScaledSExpression`: one calculated
value becomes a scalar, while the native saved table still contains a matrix.
The writer IL SHA is
`461241340955651807a7a064230cc53cdcfe59086d221c0a5b07b61eca84db38`.
The default constructor IL SHA is
`59183f527280c8bb41097efe6dd6a1cea937d968da4a158e88be81068388c766`.

The worksheet factory uses that default when no explicit dimension is supplied;
a ribbon dimension selection supplies its requested row and column counts.
See `Ptc.Controls.Worksheet.ControlFactory.CreateSpecTableControl`,
`0x0600007c`, and `WorksheetViewModel.CreateSpecTableControl`, `0x06000a34`.

`SpecTableView.OnPreviewMouseDown` (`0x06000392`) clears the old selection and
records the pointer start. `OnPreviewMouseMove` (`0x06000395`) constructs a
rectangular cell selection while that start exists. `OnPreviewMouseUp`
(`0x06000391`) clears the drag state. There is no Shift-key condition in this
selection path; Shift-click extension is not established by these methods.

Installed help documents Shift+Enter row insertion, Shift+Space column
insertion, Tab forward with final-row creation, Shift+Tab backward and arrows
within the array/table. Caret position affects before/after insertion. Ordinary
Enter is not documented as an implicit downward cell-navigation command.
Help identities and navigation details are retained in 0053.

Selected original method identities and supplemental static instructions are
retained under `artifacts/rea/spec-table-structural-20261016/` in
`equation-contract.json` and `pointer-selection-contract.json`. These retain
their parent REA evidence IDs, artifact hashes, MVIDs and IL digests. Factory
instructions are in the retained `native-visuals-20261016/anchor-contract-cil.json`.

## Independent document implementation

The existing opaque XML content and version-2 canvas format remain authoritative.
The document model now provides:

- `SpecTable::new(data_rows, columns)`: native-shaped blank table; the standard
  default is `new(1,1)` with separate empty header, unit and data slots.
- `insert_row(index)` / `insert_column(index)`: insert before an explicit index;
  using the count appends. The application maps native caret/direction rules to
  those indices.
- `delete_row(index)` / `delete_column(index)`: explicit native deletion rules,
  including clearing the final row or column. Returned `bool` means changed;
  it is not a request to remove the whole worksheet region.
- `clear_cells`: atomic selected-slot clearing, with duplicate slots handled once.
- `remove_or_clear_selection(display_rows, columns)`: native Delete/Backspace
  selection rules with whole-column priority, header/unit guards and the final
  original row/column retained. This is separate from the Clear Cells command.
- `copy_matrix`: data-only rectangular copy; `copy_display_matrix` additionally
  addresses header row 0 and unit row 1. Both produce independent structural trees.
- `paste_matrix`: atomic mathematical matrix replacement within an existing
  data rectangle. It preserves blanks and never substitutes zero. Invalid or
  out-of-bounds input leaves the original unchanged.

Source edits preserve untouched expressions, header/unit labels, literal
spelling, per-column formatting, unknown metadata and comments. Matrix row
counts update without flattening their attributes. Structural changes remove
stale `actualWidth`/`actualHeight` measurement caches so the renderer measures
the new geometry; user width and position attributes remain intact. Dimensions,
cell counts and source size keep the existing document limits.

Changes are constructed as nonoverlapping byte-range patches and applied in one
forward pass. Multi-cell clear/paste does not serialize, reparse or shift the
whole source separately for each edited cell.

New tables no longer require a preserved native source region to render or
export. `NativeVisuals` uses an internal NUL-prefixed `canvas:<id>` key for these
entries. XML attributes cannot contain NUL, so local entries cannot collide
with native source IDs. Consumers still use `region(&CanvasRegion)`; no persisted
model field or existing native cache key changes. MCDX export permits this
specific new opaque table type, writes its column matrices and assigns a fresh
Pending result reference to every new column.

## Verification and limits

The application now creates the native 1-data-row × 1-column table from the
table command, inherits preserved worksheet math-format defaults and edits its
header, unit and body with the existing structural formula editor. Selected
table ribbon row/column insertion and deletion use the document operations.
Indeterminate insertion chooses before at the whole cell's left boundary,
including a first nested operand, and after otherwise. Header/unit cells do
not insert or remove data rows. Shift+Enter and Shift+Space use the same
commands. Tab beyond the final cell appends an entire padded data row.

Plain pointer dragging across cells selects a rectangle. The current Copy
caller uses a matrix for body-only selections and a new table object when
headers/units are selected. Cut/Backspace/Delete follow the structural removal
guards, while Clear Cells preserves dimensions and formatting. Equation Paste
uses the active structural math editor and does not distribute a matrix over
table cells. See 0061 for the caller evidence and current application checks.
Ribbon Copy preserves the rectangle. No additional native operating-system
clipboard format is claimed.

An explicit structural ribbon command commits its prior cell draft and the
structural mutation as one region undo entry, refreshes the native visual and
commits calculation once. Keystrokes remain drafts. Moving by pointer to a
different cell commits the old cell once. Cell undo/redo consumes the command
only if the structural editor actually changed; otherwise it falls through
to worksheet history, so a newly focused cell cannot swallow a structural
undo. Its dirty flag compares the actual draft tree with the committed cell.

The focused application suite initially passed 12 tests with one selected-file
probe ignored. It covers draft Enter commit, native input-only cache status,
explicit calculation with disabled-table preservation, Tab growth, caret-edge
insertion and header guard, Shift insertion, minimum explicit deletion versus
range deletion, actual pointer-drag matrix Copy, range Delete with one history
entry/calculation, bounded matrix Paste, ribbon draft-plus-structure undo and
redo while still editing, and actual pointer click to another cell followed
by one undo restoring the original XML. Final app clippy with
`--all-targets --no-deps -- -D warnings` passed. These are headless checks;
workspace and both-OS results belong to the parent integration snapshot.

Two final review corrections cover real worksheet coordinates and native names.
Table pointer coordinates are transformed from global into the table's layer
before cell hit-testing. An added headless test translates the layer by
`(237,119)` and scales it by 2, performs actual press/move/release events, then
issues Copy with the Ribbon focus request; the 4×1 header/unit/body selection
survives. The explicit range-clear API clears both selection and drag origin
when the parent selects another region. Right-edge atom navigation reads the
editor's active text field, including a native identifier's literal suffix,
rather than treating that node kind as an empty string.
The final focused run passed 14 tests with one selected-file probe ignored;
both the native name and suffix endpoint navigation cases passed.

Focused dev-profile document tests passed: 13 tests, one ignored selected-file
probe. They cover minimum table creation and native matrix storage, middle
row/column insertion and deletion, minimum-row/column clear semantics, metadata
and blank preservation, header/unit/data selected copying, matrix paste,
atomic failures, native selected-range deletion priorities/minimum guards,
cache-key collision refusal and native export invalidation. The complete
document suite then passed 67 tests with two explicitly ignored file probes.

The new-table persistence test creates and populates a table, inserts a row and
column, pastes a matrix, saves/reopens `.mathcraft`, exports a new `.mcdx`, then
reads the package again. It asserts geometry, unit expressions, both column
matrices, data values and the separate Pending result references. This verifies
our package roundtrip, not an original Prime reopen.

The explicit fixture probe passed for copied 1015 and 1016 sources: each has
6 columns, 10 data rows and 55 nonempty data values. After a temporary cell edit
and restore, an inserted middle row and column are removed again; all six
definition MathML strings match their pre-command bytes. Both source files
remain byte-identical. Logs are `document-tests.log` and `actual-fixtures.log`
under the retained evidence directory. The final focused document clippy check
with `--no-deps --all-targets -- -D warnings` also passed. Workspace/OS checks
belong to the final integration snapshot.

Exact visual fidelity, rectangle Cut, plain-text/Excel clipboard
interoperability, automatic paste expansion, header/unit matrix paste, every
native selected-range Delete edge case and all original clipboard formats
remain incomplete. Unsupported operations must remain explicit rather than
silently truncating source or fabricating values.
