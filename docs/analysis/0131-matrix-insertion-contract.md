# Matrix creation and mode-sensitive insertion (Prime12)

2026-10-19. Static installed-artifact REA and independent Rust headless verification. No original GUI, screenshots, application runtime, CLR or COM execution. This extends array selection/expansion0065–0068 and the common analysis-list insertion contracts0114/0118. A new command is not a claim that all matrix/table workflows are complete.

## Source identity

E = `C:\Program Files\PTC\Mathcad Prime 12.0.0.1\Ptc.EquationEditor.dll`, SHA256 `d419032c0e99a62eab2f278e571a338d8c4b9662655af8ab462d482a028c0223`, MVID `d8627dca-3258-470c-b4d1-c00bbe2f2441`.

REA selected-members parent `ev_211c301062f29891c48f48694d7d720f8951f26ca66454be1d3a4c8fae8f5530`, summary `ev_83ece60994ba63a15f3603b86ffd6e26ba2d043d8496eb063dcc46cfb0b4b71a`: r and MatrixNodeInfo selected, 78 methods, no unmatched names, metadata coverage complete. Selected #US terminal flags are noncanonical; readable UTF16 is retained and runtime acceptance is not assessed. Receipt: ignored `artifacts/rea/matrix-authoring-rea-summary-20261019.json`. General insertion bodies reuse the artifact-bound complete E metadata from0114/0118; tokens are build-local.

| Installed Help | SHA256 | Relevant contract |
|---|---|---|
| `matrix_operator.html` (H107) | `edf7afe704eae1ff8435d8826d66d87af53e3b15ef0e362b2fc9fcb6794b5399` | Matrix operator creates an empty placeholder; Ctrl+M / `[`; expand with rows and columns afterwards. |
| `keyboard_shortcuts_for_editing_arrays_and_tables.html` (H097) | `077a67fee05e6ff532ec7a7176ef1a73fb91dbed2a2a53e33eab8d9a9ec54783` | Shift+Space column insertion, Shift+Enter row insertion, Tab row-major and final-cell row growth. |
| `to_edit_arrays_and_tables.html` (H169) | `85cc675cc698b30fc4fd20655032f63743cbf94bc196323c148a6a3ac311beb0` | Expand, select, remove rows/columns and clear contents without changing size. |

## Native default and descriptor

MATRIX initializer E0x06000bbb IL3731..3801 registers Outfix4, precedence200, nary childCount1, navigable parts[0,1], containerSlots[0] plus AllChildSlotsAreContainerSlots=true, left/right edges enabled, XML `matrix`, MatrixNodeInfo and custom factory r.e002eb. There is no GROUPING-style prefix override in this descriptor. Matrix is not the Group Family1 fence.

The custom factory002eb initializes rows=1,cols=1 and zero supplied values. Optional factory arguments can supply explicit dimensions followed by elements; those arguments are not evidence of an interactive dimensions dialog. 02f1→02f3 creates the matrix and uses02fa to fill missing cells. MatrixNodeInfo default ctor0eac independently uses full/viewport size1×1. Larger matrices can have a12×12 viewport cap in002eb; preserving all matrix viewport formatting is a separate import/export concern.

This corrects the product's inconsistent commands: the old ribbon inserted an arbitrary2×2 matrix, Ctrl+M inserted1×1 and all cases wrapped the current expression. The native-default command now has one common1×1 implementation. The explicit-dimensional `Editor::insert_matrix(rows,cols)` API remains an independent legacy construction/wrapping API and is not a model of Normal keyboard insertion.

The actual Ctrl+M entry is independently traced: `Ptc.Controls.EquationCommands..cctor` C0025 IL064b..065f adds Gestures.CtrlM to InsertMatrix.InputGestures; C019d OnInsertMatrix converts the routed command parameter to nullable TableSize and raises the model event; E0184 InsertMatrix with no size calls InsertOperator("MATRIX") directly. E0184's explicit-size branch uses a different undo command. No dimensions chooser is invoked on that captured keyboard path. The original ribbon can supply a TableSize parameter and its picker behavior has not been recovered here. The product's plain Matrix action now invokes the unparameterized command; this does not reproduce or claim a complete original size picker.

The additional static dispatcher receipts are `artifacts/rea/feedback/matrix-command-{entry-control,fire-control,model,static-bindings}-cil-20261019.txt`. C is `Ptc.Controls.EquationControl.dll`, artifact identity is retained in the source metadata for0065/0068. C0025 raw IL SHA256 `54d4a80893e8a2de79dadeecd117ff166e44afd38b6462e67429de882ce6124a`; C019d `d87ffb2ee5ebc751f620a0d647f2d47e48eeb9f1104158cf2013a94e33cbeb33`; E0184 `a017454a2224c44198983817c96583bdf304869d6e194456927d14d575ebed5a`.

The null-parameter conversion is also traced rather than assumed: Core TableSizeConverter01b8 delegates to Discrete2DCoordinateConverter01bc, whose IL00f6 returns null for the absent command parameter;01b8 IL0040 preserves null. The model therefore takes the HasValue=false branch. Raw bodies: `matrix-command-size-converter-cil-20261019.txt` SHA `93aab2b9e70bf1ee9ab8a3479603e0d5bac2e46dca081afcb874b34cbe9ed598`, `matrix-command-coordinate-converter-cil-20261019.txt` SHA `c50c63ee1dd11e568b4a71cb8839415cd045ba2adc0b1c369c142614ec50f498`. The Core artifact identity remains in the matching retained metadata receipt.

## Normal, full value selection and Scope

The following are CIL-derived shapes, not captured original runtime outcomes. MATRIX is an actual Outfix descriptor;04a3 converts it to an effective operand handle. This differs from the opening GROUPING key, whose explicit effective-prefix override consumes the following operand. The existing analysis-list normalization/parse is reused, including implicit Scale142 and native container boundaries.

| Command context | Represented result | Caret |
|---|---|---|
| Empty placeholder | Matrix1×1[PH] | New cell PH Center |
| Normal complete atom right edge | Scale(atom,Matrix1×1[PH]) | New cell PH Center |
| Normal complete atom left edge / ordinary Center without range | Scale(Matrix1×1[PH],atom) | New cell PH Center |
| Explicit complete value selection `[0,len]` | Matrix1×1[old atom] | Old atom first native entry, text0 |
| Normal1234 middle `[2,2]` | Scale(Scale(12,Matrix1×1[PH]),34) | New cell PH Center |
| Normal1234 selection `[1,3]` | Scale(Scale(1,Matrix1×1[PH]),4) | New cell PH Center |
| Scope selected expression | Matrix1×1[selected expression] | Matrix outer Left/Right from Scope direction |

Blank/full-value insertion follows replacement handlers:053a's Outfix path inserts the new node as a parent and places entry at its first child; an existing placeholder becomes its cell. Boundary insertion05c4 creates an actual Outfix handle. 05c6 IL00fa..010a constructs its Center part0 and moves Right once, entering the first placeholder cell. Text splitting05d4 uses the existing factory for retained fragments and prioritizes the new operator's placeholder. Scope05d9 wraps the selected segment and returns the new Outfix's outer side. A Scope subtree is not a full ValueRange; the command preserves that distinction.

All matrix cells are native container slots. Entering a matrix inside a Power exponent therefore reparses that exponent, without escaping to the base expression. Existing Scale chains are reparsed at Scale142, left associatively. An explicit completed PowerRight boundary and a caret inside its exponent remain different states.

Own IR retains Unicode scalar strings and a single optional literal suffix. Partial edits crossing a UTF16 surrogate boundary, splits inside a formatted literal suffix, unrepresented parent repair such as computed APPLY callee changes, MatrixCells mode and separate Typeover mode are atomic unsupported in this command. These are implementation bounds, not original prohibitions. Whole-node metadata/IDs are preserved where the native mode preserves the operand. No temporary analysis handle is persisted as an AST node.

## Transaction and workflow verification

`Editor::insert_native_matrix()->bool` returns documentChanged. It builds a candidate, validates node/depth/text bounds, then creates one checkpoint. Failure retains document, caret, Scope, selection, revision and redo. New source uses the existing Matrix AST and native row/column storage bridge. Existing row/column and range-selection commands are unchanged.

The application routes Ctrl+M, blank `[` and the Matrix ribbon action through this command. A populated `[` keeps its separate index command. Creation starts with one cell; existing Shift+Space / Tab commands grow and fill a2×2 array in the integration fixture. Draft edits retain calculated results until Enter. Headless tests verify source trees, exact cell order/value oracles, Undo/Redo and native export/import.

Local source checks: eight new core regressions, full editor228 pass; five new normal app checks, full app427 pass/42 ignored. New `matrix_authoring_save_native_export_reopen_probe`, env `MATHCRAFT_MATRIX_AUTHORING_OUTPUT`, basename `matrix-authoring`: seven regions comprising three definitions and four queries, matrixA `[1,2;3,4]`, matrixB `[12]` from complete value selection, matrixC `2 Scale [3]`, and scalar queryA[0,1]=2. Independent source/value expectations are asserted after actual CPU Key/Text editing. Saved reopen has four query displays, zero execution and zero Pending; exported MCDX reimport/recalculation has zero errors. Own/native row-major source mapping remains covered by exact expected values.

Ignored local output `artifacts/rea/matrix-authoring-preflight-20261019-v1` contains own save, MCDX, reimported save, PDF and text-only verification. The original vector-PDF headless probe passed with seven regions/four restored displays, two pages. Independent PDF object inspection found zero image XObjects,196 vector paths and two embedded font resource names; no page screenshots were produced. PDF SHA256 `b89209252c0123f4cfd1cc12ebf01e280c833ad0921cf18c7f466a80fb77e62c`. These local results do not claim the final both-OS frozen-source gate; the integration owner records that separately.

Core/app all-target Clippy passed before the last test-only additions. The later full app Clippy check reached an independently owned Area API type-complexity diagnostic in compat2119; it is reported to that owner and is not hidden by this module. Final integration must rerun the complete gate after all active owners freeze. Raw16-method SHA verification is retained in ignored `matrix-authoring-method-ledger-20261019.json`, with extracted CIL in `matrix-authoring-cil-20261019.txt`; target execution is false.

## Raw CIL identities

| Token | Method | Raw IL SHA256 |
|---|---|---|
| `0x060002eb` | r.e MATRIX factory | `232c255413bea559ba2706cfcbf0a45796fe7242f977fe37c3550f820646918a` |
| `0x060002f1` | r.a matrix construction | `894b329130789909e8ec5104195e8a3925df4582b1e3f536d161cec1878447f2` |
| `0x060002f3` | r.a matrix allocation/info | `8c1c98f1ccbe2d92c9529d5257a78b3e431a1862178c3591871d29ccdd4d6182` |
| `0x06000eac` | MatrixNodeInfo default ctor | `eb0bb8de5f6debe9ccd71e9c8ebad02858926494ae59538d476bc20dc70e4e1c` |
| `0x06000eaf` | MatrixNodeInfo full ctor | `43b123ee5c8ba71d5227b39efeeb77c486390f169f0d256da93831f87406b1cc` |
| `0x0600053a` | Outfix InsertAsParent | `06cacd01672daf0a2a86ea0bf1ec35747c422a484a4c6a1ee83fbafb78365aed` |
| `0x060005c6` | Insertion cursor | `3bc6ac5566ace12d0ee48c1c7b8795bbaffb44737548ba64e75b270f85ef6a69` |

## Remaining original workflow gaps

This block does not complete all matrix/table authoring, the original lexical parser, formatted partial split, original ribbon dimension-picker behavior, matrix viewport/ellipsis editing, or the unimplemented empty-cell Ctrl+Left/Right bypass (EA-064). The keyboard dispatcher is traced independently of factory defaults; no original dimensions dialog has been inferred or introduced. Spec Table's header/units/data model remains distinct from mathematical Matrix.
