# Equals, text replacement, and undo command boundaries

2026-10-15. Static REA inspection; no target code, original application, GUI, COM, or screenshots were used in this pass. The implementation is independently written Rust. Source bodies remain under ignored artifacts.

## Evidence identity

`Ptc.EquationEditor.dll`, Prime 12.0.0.1: SHA-256 `d419032c0e99a62eab2f278e571a338d8c4b9662655af8ab462d482a028c0223`, MVID `d8627dca-3258-470c-b4d1-c00bbe2f2441`.

- Full saved managed metadata: `ev_419ab24312df266f2e58ff6b91d32e0261a9a2bba9105ba09ebecb7517b43fdf`.
- Focused EquationModel evidence: `ev_fd77c469a4e79dcea831eee774b27cd5a793ad0c324dea2b4b314892b5ec0dee`, retained in `artifacts/rea/ux/equation-model.json`.
- Reboot recovery selected `bc`, `ba`, `bd`: `ev_fbeadd7d7dfb2620c9b2dd073abe2f8a6d30864ad6ace861cc62b5b72805e8df`, summary `ev_6d87e1e60ff0ba60f6fe5570820f017b03ad7c6001a41a96243f4ccb39906a98`. Member coverage is complete within these types. REA reports one readable #US string with a nonstandard terminal flag; this is not a claim about runtime acceptance.

Supplemental `artifacts/rea/editor/equation-il.json` resolves CIL operands and retains the original raw CIL hashes. Token identities require the exact DLL hash and MVID above.

## `=` is a contextual command

`EquationModel.ProcessEqualsKey` (`0x060001a0`, method body file offset 36692; raw CIL SHA-256 `7abca7d0ea2633803c1656f6e107afbc3ea53bca4252afaeb9bb3b388abfde4a`) first attempts a local range-variable definition and observes the undo-stack depth. It then consults `get_EvaluationRoot`. If an ordinary evaluation is already present, it does not insert another. The allowed non-expression-only path can execute the named `Insert Evaluation` command. An Action callback also permits calculation without another tree mutation. Callback counts across the entire insertion path were not established.

`get_EvaluationRoot` (`0x060000f9`, offset 25348; raw CIL SHA-256 `31a66c2472ea35b934f5213b5ac45699be8474aed4a4074a9a11cccd28bddd0b`) recognizes either a root evaluation or an evaluation on the RHS of a definition/global definition. The EVALUATION descriptor in `0x06000bbb`, IL `2072–20a6`, restricts its parent to ROOT or slot 1 of a definition/global definition.

Rust now returns `InputOutcome { document_changed, evaluation_requested }`. Repeated `=` retains the tree, caret, selection and undo stack while requesting worksheet recalculation. On a definition, insertion wraps its RHS and preserves the assignment. Dirty state and document history depend on actual source/geometry changes, while calculation requests can run without them. The output unit slot remains editable independently of the physical value.

Local range-variable definition and expression-only modes are not represented by this editor. Ignoring the `=` in an empty `:=` RHS is the prototype's text shortcut policy. It is not the complete original equals state machine.

## Selected expression replacement

`EquationModel.InsertValue` (`0x06000198`, offset 36308) feeds a formatted value to `0x06000199` (offset 36376), which executes the named `insert value` undo command. Raw CIL SHA-256 of the latter is `27befcd6c6b4687cf9c72a47aebeba75bf721a0e2b6c829904bee50d5668bedf`. Its closure (`x.a`, `0x06000b99`) dispatches to the current editing mode.

Typeover mode `n.x` (`0x060002c0`, offset 53128) rejects matrix-cell typeover in this path and invokes mutation builder `ax.a` (`0x060004f6`, offset 101328; raw CIL SHA-256 `55ff0c4fb825228e5fe976afee3915fd064acd08c9bb26cecbeff243af7b713a`). That builder supplies the selected navigator segment, inclusion policy and entry state to `bc`. `bc.bq` (`0x060005b8`) removes the selected segment through `ba` or `bd`, parses the incoming formatted value, and replaces the resulting placeholder or inserts at the resulting boundary. `ba.bq` (`0x060005ae`, offset 118716; raw CIL SHA-256 `864324fda2803519d0d46d2752d1d68f6999f0630b7c0624a6c2309e146865fb`) explicitly removes/rebuilds selected operand ranges.

The Rust subset represents whole subtrees only. Typing a character over a selected compound expression replaces that branch, preserves siblings and restores the selection on undo. Selected numbers bypass implicit unit multiplication and imaginary-literal continuation. Partial linear ranges, formatted-value parsing, matrix typeover and all native precedence rules remain outside this reconstruction.

## Command grouping

The original Paste CompoundCommand evidence is recorded in [0014](0014-expression-scope.md). Plain-text paste now uses `edit_group`, including nested input transformations. A committed Rust text event is also one group, so insertion of an implicit multiplication and its identifier cannot leave a half-completed operation after undo. This is a bounded event policy motivated by the original `insert value` command boundary; it does not assert Prime's complete ordinary-typing coalescing policy. Multi-event IME behavior is unverified.

Headless app tests cover RHS evaluation and display units, repeated calculation without edits, paste undo/redo with stable sibling IDs, malformed structural clipboard rejection, selected compound/number replacement, and committed-text grouping. These tests run Rust state transitions, not the original product.
