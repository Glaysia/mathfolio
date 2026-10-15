# Expression scope, insertion direction, and command undo

Date: 2026-10-15. Static analysis only; no original executable, GUI, or screenshots were used for this pass.

Mathcad's scope is an editing state with a direction and history. Treating Space as a generic parent selection loses both the side on which the next operator is inserted and the route back to the original text caret. The Rust editor now represents these separately from document mutations.

## Source identity and evidence

The source is the installed Mathcad Prime 12.0.0.1, `Ptc.EquationEditor.dll`, SHA-256 `d419032c0e99a62eab2f278e571a338d8c4b9662655af8ab462d482a028c0223`, MVID `d8627dca-3258-470c-b4d1-c00bbe2f2441`. REA managed-member evidence: `ev_419ab24312df266f2e58ff6b91d32e0261a9a2bba9105ba09ebecb7517b43fdf`.

Decoded CIL is retained locally in ignored `artifacts/rea/editor/equation-il.json`; focused extracts are `scope-deep-il.txt`, `scope-transitions-il.txt`, `scope-insert-il.txt`, and `insertion-operation-il.txt`. The supplemental decoder retains the original REA body hash and file offset for each method. It does not recover original source names for obfuscated types. The public [editor contract](../spec/mathcad12-editor-contract.json) records descriptor facts, rather than proprietary implementation bodies.

| Question | Method and location | Static observation |
| --- | --- | --- |
| Initial text scope direction | `ak.a`, token `0x060003e9`, body file offset 69212, IL `003e–00c0` | Compare caret position against value length divided by 2; strictly before midpoint selects Left, otherwise Right. |
| Expansion history | `ak.l`, `0x060003d9`, offset 67660 | Push the previous linear segment before expanding. Expansion has branches for continuous n-ary children, adjacent subexpressions, container slots, and scopable parents. |
| Contraction | `ak.k`, `0x060003db`, offset 68432 | Pop a previous segment when available; a caret-origin scope can clear its segment when history is exhausted. |
| Terminal expansion returns to entry caret | `m.ao`, `0x06000294`, offset 50704, IL `0000–0021` | Save `ak.q` before expansion; if expansion fails, restore that saved navigator state and leave scope mode. `ak.l` clears terminal scope through `ak.i` (`0x060003de`). |
| Contraction returns to normal mode | `m.a8`, `0x06000295`, offset 50766 | Contract, check whether a scope remains, and restore the saved navigator state when it does not. |
| Selected operand insertion side | `bl.bq`, `0x060005d9`, offset 127168, IL `0394–03de` | For a normal operator lacking a primal-child override, Right fills the first placeholder operand; Left fills the last placeholder operand. |

Original raw CIL SHA-256 values, in table order: `c34c9a116d93d7d4426aebc8c5b824ed5d29ef9c9bd22231e866a76d5b5b03af`, `65bdf4579b402ff9bba4b3a2288b338513b082e93b81bbc4a1243790fe12e014`, `4882141b962901cd5fee3bf16f564a94a5abd44b1cd73bf50b6e7c4333d283b5`, `705925c9fb98d53996d51455f1ed1de70441e68ec6ab401d21db48b0d420b627`, `d1f5612453bbaad60e7da4f71146af34bbd4a1fb69630a3c1e235e04032ec172`, and `f148d85443e73a00d367e1c62e214985432ced2e469f475b955181ba75379dbe`.

## Independent Rust behavior

`Editor::scope_direction`, `expand_scope`, and `contract_scope` preserve a scope's initial direction, original caret (including UTF-16 position and insertion mode), and expansion history. Navigation does not add a document undo entry. Once a mutation starts, its undo snapshot includes the preceding caret, selection, and scope history. Undo therefore restores the editing context as well as the expression.

For example, selecting `b*c` from its left side and inserting `/` produces an empty numerator over `b*c`. Selecting the same expression from its right side produces `b*c` over an empty denominator. Power and infix insertion follow the same side rule within the represented binary operators. Sibling nodes retain their IDs; undo and redo retain the resulting tree IDs as well.

The represented selection is still a whole subtree. Expansion follows parents within that subset, skips empty placeholders, and cycles from the expression root to the exact entry caret. This parent traversal is an explicit bounded reconstruction: the original also has partial/adjacent operand segments, n-ary ranges, matrix selections, hidden logical containers, and descriptor restrictions that are not represented by `Selection::Subtree`. The observed terminal-cycle and direction rules do not establish parity for those omitted states.

`contract_scope` is a core command API. It is not bound to Shift+Space: shipped `Doc/en-US/keyboard_shortcuts.pdf`, page 21 (SHA-256 `db4cdf1f913cfad20075b3f6354085183a22bde588900709c12ca06c065b3a4f`), assigns Shift+Space to matrix column insertion. No alternative keyboard binding is inferred in this pass.

## One user command, one undo step

The original control assembly's `VisualMathRegionNode.BuildAndExecPasteCmd`, token `0x06000a6b`, body offset 150576, creates a `Ptc.Undo.CompoundCommand` and executes the named `Paste` command. This is static evidence that paste has a command boundary spanning its component edits. It does not establish the exact undo granularity of every ordinary typing sequence.

Control source identity: `Ptc.Controls.EquationControl.dll`, SHA-256 `0b6fd580eb6cc183f17b5b5e0967226bbafc5e12efc6da809f44cd16416bf9d1`, MVID `cf1d7cb5-c73f-4618-8e6b-b83959d13b98`; REA evidence `ev_389338a9845c7ea83dfd310922316cf92ca1637bdbac6602a01c4690e52b8349`. Paste method raw CIL SHA-256: `98c381298b7843425de14fd41c21c31980e5559918614d8ce0487dc582bd9b3a`.

Rust's `Editor::edit_group` supplies an independent command boundary for callers such as plain-text paste: nested primitive edits create one checkpoint on the first mutation. A navigation-only or empty group preserves redo. Undo/redo is unavailable while a group is executing. Panic unwinding resets group bookkeeping and leaves any partial edit as one undoable command; it is not an automatic rollback. This API's nesting and panic policy are project choices, not claims about Mathcad's internal command implementation.

## Verification and remaining questions

Headless `cargo test -p mathcraft-editor --locked` covers direction at and around the midpoint, UTF-16 surrogate boundaries, left and right fraction insertion, left-side power/infix insertion, scope contraction and terminal cycle, undo restoration of scope/caret, stable sibling and abandoned-branch IDs, nested edit grouping, navigation-only groups preserving redo, and panic recovery. These tests execute the Rust implementation only.

Partial operand ranges and native precedence-sensitive scope transitions remain open. Exact command coalescing for ordinary typing and a keyboard route for contraction are also unresolved. Static evidence identifies intended state transitions; runtime appearance, highlight geometry, IME behavior, and timing have not been verified.
