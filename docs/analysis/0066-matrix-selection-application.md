# Matrix cell selection in the formula widget

This integrates the independent editor model described in
[0065](0065-matrix-selection-contract.md). It concerns ordinary mathematical
matrices, distinct from the native specification-table clipboard contract in
[0061](0061-spec-table-clipboard-contract.md).

The REA skill's parallel static investigation established the original mode
contract; application behavior was implemented independently and verified with
headless egui input. No original application execution, GUI, screenshots, COM or
CLR was used. Original CIL identity, digests and limitations belong to 0065 and
its retained evidence; the tests below do not replace that original evidence.

## Observed contract and integration

The recovered matrix-cell typeover mode `n` uses
`TypeoverState.AreMatrixCellsInTypeover`. Its Copy handler `n.a6`
(`0x060002d5`) copies a rectangular cloned matrix. Its deletion helper `n.b`
(`0x060002bb`) distinguishes a full matrix, whole rows, whole columns and an
interior rectangle; Clear handler `n.bj` (`0x060002d0`) preserves dimensions.
The editor owns those mutations and their undo grouping.

The original pointer `SelectTo` path uses a shared matrix ancestor. In the
application, dragging across different cells resolves the nearest shared matrix
and the two immediate child-cell indices. Dragging within one atom retains text
selection; dragging across non-matrix mathematical operands retains the existing
subtree selection. When a cell contains a nested matrix, dragging across its
inner cells selects that inner matrix. Crossing to another outer cell selects
cells of the outer matrix rather than replacing a nested operand accidentally.

`formula.rs` dispatches those indices through `Editor::select_matrix_cells` and
paints the selected source cells. Selection does not change AST contents or
produce an undo entry. Painting filters the retained hit list with a set of
selected IDs; it does not search every hit separately for every selected cell.
Layout, glyph and hit caches retain their existing revision and coordinate keys.
Selection painting uses our substitute-font geometry, not recovered original
pixel measurements.

Copy uses `copy_matrix_selection` and the existing inert `mathcraft-tree:`
transport. Cut copies first and then invokes the same core deletion operation
as Backspace. Delete and Backspace call `delete_matrix_selection`; the core
removes whole selected axes, clears interior selections, and turns a fully
selected matrix into its native minimum blank matrix. The surrounding equation
and unselected cell subtrees are preserved. Clear Cells remains a separate
application/ribbon command calling `clear_matrix_cells`, without a new keyboard
shortcut or automatic calculation request in the formula widget.

The original range mode does not override Paste; its inherited `b.aw`
(`0x0600024b`) returns false. Ordinary expression input is likewise guarded in
the recovered mode. The application therefore leaves a range unchanged for
Paste and ordinary mathematical text/operators. It does not tile a scalar,
distribute a matrix, grow the destination, or replace the entire matrix with the
clipboard expression. Typed transport still passes normal validation before the
core's native no-op. Editing one cell follows the existing expression editor.

No Shift+Arrow rectangle shortcut was added. Original SelectLeft/Right in this
mode inherit empty handlers; the application leaves an established rectangle
unchanged for Shift+Left/Right while preserving ordinary atom text selection.
Shift+Up/Down's full key-to-command routing was not established in this step;
the prior geometric navigation remains outside the new rectangle contract.

Core operations can now intentionally reject a formula command in range mode.
The formula widget consequently checks the actual arena revision before marking
its response changed, preventing a rejected command from dirtying a worksheet
or scheduling computation. Explicit evaluation requests remain independent of
document changes. Normal Enter and Escape remain available to the parent
worksheet; matrix-cell input remains a draft until that parent commits it.

## Headless verification

`formula/matrix_selection_tests.rs` adds five focused tests:

- Actual pointer drag across a 3×3 matrix inside an addition, structural 2×2
  Copy, interior Delete, preserved remaining cells and outside operand, followed
  by one undo restoring the exact tree.
- Actual pointer drag through a translated `(201,117)` layer at scale 2 into a
  nested 2×2 matrix. Cut copies that nested matrix, resets only its native minimum
  and preserves its outer matrix shape and sibling cells; one undo restores it.
- Backspace removes selected whole columns, whereas Clear Cells preserves the
  3×3 dimensions. Enter remains in the parent's event queue and does not request
  evaluation or another formula edit.
- Structural Paste, ordinary text Paste, text input, Shift+Left/Right and guarded
  operator/matrix commands remain no-ops in range mode, without dirty responses,
  undo entries or calculation requests.
- Shared-matrix resolution chooses the deepest matching matrix and handles an
  endpoint crossing to an outer cell without conflating the two ranges.

The final focused run passed all five tests. The broader formula run passed
57 tests with one explicitly ignored CPU benchmark. Application clippy with
`--all-targets --no-deps -- -D warnings` passed. Parent worksheet save/reopen,
native export, calculation-commit and both-OS validation are recorded separately
by integration; these widget checks alone do not establish complete Mathcad
editing parity or original clipboard interoperability.
