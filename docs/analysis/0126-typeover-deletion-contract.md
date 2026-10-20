# Prime 12 ordinary failed deletion and Typeover

The installed editor has a separate Typeover mode, not an ordinary Scope. A failed structural deletion can select a term without modifying the expression. The next delete removes the selected segment; value input replaces it. This report establishes a bounded whole-Fraction and Power-postfix subset and preserves the general segment limitations.

## Evidence identity and method coordinates

Equation assembly SHA-256 `d419032c0e99a62eab2f278e571a338d8c4b9662655af8ab462d482a028c0223`, MVID `d8627dca-3258-470c-b4d1-c00bbe2f2441`; retained REA `ev_419ab24312df266f2e58ff6b91d32e0261a9a2bba9105ba09ebecb7517b43fdf`. Control assembly SHA-256 `0b6fd580eb6cc183f17b5b5e0967226bbafc5e12efc6da809f44cd16416bf9d1`, MVID `cf1d7cb5-c73f-4618-8e6b-b83959d13b98`; retained REA `ev_389338a9845c7ea83dfd310922316cf92ca1637bdbac6602a01c4690e52b8349`.

Focused fresh REA `inspect_managed_members(type_names=["bq"])` is `ev_8e02207420dc7f1efe4c995cfd87b78c0474e4fc3c2f4186da2098a82efec1e5`. Its selected 05fd/05fe/0605 rows are retained in `artifacts/rea/typeover-bq-focused-rea-20261019.json`. All relevant body hashes and raw static decodes are indexed in `artifacts/rea/typeover-method-ledger-20261019.json`. `typeover-query-20261019.py` reads installed PE bytes with dncil, checks the assembly and exact raw CIL hashes, and resolves metadata coordinates. It does not load an assembly into a CLR. No original execution, GUI, or screenshots were used.

## First failed key: rollback then selection

The earlier [0122 deletion contract](0122-structural-deletion-contract.md) remains valid. `l.j` 0075 calls ax04e6 and bh05c8. `a0.b` 0524 SHA `b0c241e89aac8755a76e2f86e7bb03bbcc2267b743dab4b37c532c652514aab8` executes working edits inside a temporary CompoundCommand. On failure IL0021..002b undoes it and invokes fallback `bh.bs` 05c9 SHA `c45fca78297c6f242784e5f45a439f45128ecc74bd283a99dd3096d4f6111c61`.

05c9 copies the original navigator/direction into a fresh `ao`, then `ap.am` 0431 runs a separate fallback sequence. `ap.a` 042f creates its iterator; iterator 046b yields 044e,044d,044c,044f,0450,0451,0452,0453,0454,0455,0456,0457,0458 in that order. It is not a second invocation of the rewrite pipeline with the intermediate working caret.

An ar(TypeoverState) result takes 0075 IL004a..0070: set model.p, change mode to n, fire its notification, return false. The ordinary successful edit branch FireExpressionValueChanged/true is skipped. Therefore the first failed delete changes temporary editor state but not document content or its undo history. Do not treat false alone as proof that nothing changed visually.

`n.e` 02a7 returns mode 2. TypeoverState 12ae stores a NavigatorSegment, the original NavState, and IncludePrimalChildren; 12a9 defaults the latter false. Scope is a different state and model field.

## Verified selected segments and asymmetries

The table describes editable, complete terms with no placeholder descendants. It does not supersede earlier direct rewrite stages.

| Starting native position | First key | Confirmed fallback | Second delete |
|---|---|---|---|
| Complete Power with atomic base, outer Right | Backspace | 0454 selects its postfix analysis handle; base is excluded | Either Backspace or Delete removes the script and preserves base at Right entry |
| Complete root Fraction, outer Right | Backspace | 0454 selects the Fraction term; denominator is a separate container | Same |
| Complete root Fraction, outer Left | Delete | 0454 selects the Fraction term; numerator is a separate container | Same |
| Root Power outer Right / Fraction outer Right | Delete | Outward navigation has no neighboring term; no whole-term selection established | No forced deletion |
| Root Fraction outer Left | Backspace | Outward navigation has no neighboring term | No forced deletion |
| Explicit Power Left | Either | Power does not enable an ordinary own Left state in its descriptor | Not claimed |
| Group Left/Right | Either | Direct 043b fence removal from 0122, before fallback | Not Typeover |

0454 SHA `3661d320c55eaf45566518a1fb1353bdebb2d703c03148e0fb50716eeac590d7` requires direction=Opposite(CaretSide), moves one native navigator state, and compares that state's ContainerRoot with the current term's ContainerRoot (IL0019..0048). A difference creates LinearSegment(current,current), original NavState, IncludePrimalChildren=false. Special-fixity primal/inherent branches can set it true (IL0072..00c8); the Fraction/Power subset does not use that extension. 05fe is exactly one 05fd move followed by Current, not an arbitrary nearest-leaf search. Power descriptor 0bbb IL0d14..0d28 sets container slot1 and AllowCaretOnRight=true; it does not set AllowCaretOnLeft. Fraction slots0/1 and edge states are established in 0114.

Incomplete PH-containing terms may trigger earlier placeholder, collapse, or repair handlers. They are deliberately excluded from the new whole-term fallback API until those complete paths are established. Nested terms also require the parent/container/linear-segment rules rather than an unconditional subtree selection.

## Second delete and Undo

`n.j` 02b9 SHA `dc36723f5d6f06da2b43c47c40a434e3e137aada03b49884f02a661d9f76257d` does not read its direction parameter. Both keys delete the current Typeover segment through ax04e8. Matrix partial selections have a separate branch. On success it fires expression change once and 02af clears Typeover and returns to Normal.

ax04e8 SHA `1eb05c7074d0d8bc83e253a80a8a7f1379d34c34d383bf4ebf480aa0b7980e95` converts the navigator selection to a linear segment where possible, preserving IncludePrimalChildren and original NavState into ba/bd. ba05ae removes selected handles, may preserve primal operands when instructed, reparses and repairs. For a whole Fraction its existing analysis handle is opaque, so removing that selected handle leaves no operand from the fraction. IL02f3..0318 inserts PH; IL03ef..03fc selects its Left entry. **Power is different despite actual descriptor Fixity=Infix.** Its effective analysis handle is Postfix and its base is emitted separately. The first implementation incorrectly treated this as a whole-term selection; the following complete segment/parse chain corrects that error.

LinearSegment(Power,Power) converts through128b→128c→129b. For the left boundary of the Postfix handle,129b uses child0 Right entry followed by one Right step, reaching Power Center after the base. The right boundary is Power Right.129a's reverse conversion returns exactly the Power handle, not the separate base handle. ba05ae deletes this selected handle and inserts PH after the surviving base. at0495 IL015d..019a removes that redundant PH because the preceding base handle is neither Prefix nor Infix (as047e/047f). The atomic base therefore survives, with Right entry selected. IncludePrimalChildren remains false:0454 only extends that flag for actual Special5, whereas Power's actual fixity is3; it has no primal child declaration.

`typeover_excluded_operand()` exposes the Power base for rendering exclusion. Whole-tree/value/template replacement helpers reject this partial Power selection rather than erasing the base. Supporting typed replacement of the script requires the native analysis segment reparse; substituting an entire Power tree is not equivalent. The implementation currently supports second-delete for atomic Power bases, not arbitrary base-expression parser repair.

Undo is subtle. Public Delete0173 wraps the action in command e.b6b; snapshot c.b5a captures DetachedTypeoverState at IL00b6..00c2 and current mode at 00c7..00ce. e.Undo b68 restores that snapshot via c.b5c. c.b5c IL02f7..0319 restores TypeoverState and mode. Thus Undo after the second delete restores **the expression with its red selection**, not Normal mode before the first key. Redo restores the resulting PH (Fraction) or retained base (Power) in Normal mode. The first selection itself contributes no separate document undo command.

## Cancel, navigation, and pointer placement

CancelTypeover0163 dispatches to n.bc02ad→02ae, which restores TypeoverState.NavState via02af, clears model.p, switches to Normal, and sends normal navigator notification. No expression mutation occurs.

MoveLeft0153→b.z020e→n.k02b1; MoveRight0155→b.y020f→n.k02b1. The n override calls cancel only and ignores direction. The first arrow cancels and restores the original caret, without moving another character/state. Other cancel-only overrides have the same body; they must be mapped to public commands individually before claiming all keys.

Explicit set_NavigatorState0113 first invokes current mode.a1. n.a1(02b0) clears Typeover and returns to Normal; 0113 then sets the requested new navigator. Hence clicking a different editing position cancels the pending selection and uses the clicked position. Invalid requested positions fail validation before this transition. An Escape→CancelTypeover key binding has not been verified; the public cancel API is confirmed, the physical key mapping remains unknown.

## Typed value, pasted expression, and operators

Formatted value input n.x02c0 uses ax04f6→bc05b8. It first deletes the selected segment, constructs the new value using the original factory, replaces resulting PH, and uses bq.c(Right,newValue) before repair (05b8 IL009b..00d5). The action stays atomic: failed factory/repair returns Empty and the command wrapper rolls working mutations back; do not clear red selection first and then attempt a separate insertion.

Operator n.u02bd uses ax04f1→04f2, and expression-factory n.v02be uses04f2; these are Typeover replacement routes rather than normal Scope wrapping. 04f2 passes IncludePrimalChildren into be; bb05b2 deletes the segment then inserts into PH using bm. An operator's resulting template and focus depend on that insertion contract. This report does not infer a universal first-placeholder cursor or full support for every operator/pasted tree. The portable helper accepts a caller-supplied validated tree and explicit entry side; atom replacement uses Right. `replace_typeover_editor` additionally imports a caller-built template together with its exact root-relative caret path; it copies no history or selection. This avoids confusing Fraction own-Left with numerator-PH focus.

For ordinary newly inserted Infix/Fraction/Power into a whole-root Typeover selection, be05c2 IL00b7..010f derives Left insertion direction from the PH Left entry produced by deletion, then bg05c4 builds the new operator. bg05c6 IL00db..00f3 starts at operator Center and moves Left once, reaching the left PH. Prefix/Postfix with child0 PH use that PH directly (IL0061..008c). Therefore reuse of the existing blank-editor typed template and its exact caret is established for ordinary binary, Fraction, Power, unary minus and logical-not. Ordinary postfix descriptors (Factorial/Percent/Degree/Transpose) also replace the old term by a new unary(PH), not a wrapper around the old term. be05c2 IL012a..0155 preserves the returned cursor when it is already PH; bg05c6's common prefix/postfix PH branch puts that child at Center. Generic GROUPING/Absolute Outfix insertion uses bg05c6 IL00fa..010b: its left-side operator Center moves Right to child PH Center. Caller adapters may construct those exact blank templates.

Public CloseParen is different: EquationModel015a calls virtual b.n0245; Typeover n does not override it, and0245 simply returns false (SHA84a0936ff1fba6d2811d094757231685673447aed2170738872305076026a316). Thus ')' through that public command leaves the temporary mode/content intact. It is not a synonym for generic InsertOperator(GROUPING). Define/Eval glyph and multi-key/public dispatch are not established here merely from the generic insertion branch. Do not extend these positives to every Special/primal template solely because the transplantation API can represent it.

## APPLY and general segment limits

0452 is a dedicated function path: ARGUMENT_LIST or EMPTY_FUNCTION_NAME, a one-state move when required, and explicit NodeSide checks. Ordinary argument-fence selection can be LinearSegment(APPLY,ARGUMENT_LIST), while EMPTY_FUNCTION_NAME+Backspace uses a different left endpoint. Therefore it is unsafe to silently replace every Call with a whole-subtree red selection. Primal exclusions, pair-nary parts, matrix cells and multi-line selections likewise require the full TypeoverState segment, not Scope(Subtree).

Implemented subset is Fraction whole-handle and atomic-base Power postfix selection, explicit outer CaretSide Left/Right (not operator Center/NodeSide aliases), no PH descendants. The target may be the region expression root or directly Evaluation.left/Definition.RHS/GlobalDefinition.RHS. In those worksheet shells0454 still constructs the segment from the **current** Fraction/Power node; no parent is selected. ba removes the corresponding handle within the existing outer analysis list, retaining the evaluation/definition and sibling IDs. For Fraction, the replacement PH is an operand of that existing shell. Other arbitrary nested parents remain unsupported.

The implementation uses a separate temporary Typeover state retaining target, origin and Power-postfix distinction. Its architecture may be extended to navigator segments, but those omitted native paths are still unsupported. This is a fidelity increment, not completion of structural deletion or a progress-percentage claim.

## Red selection rendering

C VisualScoper.BindToTypeover0c4b SHA `28bc554870e4d2f1f0b9023b80a12f65f5232e0fc208c5679f115a4d6178738f` binds RubberBandHighlighter.Color to TypeoverColor and opacity to CaretHighlightOpacity, CornerStyle=2. UpdateHighlight0c49 converts selected core elements plus excluded nodes into FitAroundElements. The established052e highlighter fills selection bounds geometry. This is a red area overlay, not proof of red glyph foreground.

Pure PE CustomAttribute extraction in `typeover-setting-defaults-20261019.json` establishes defaults: TypeoverColor Property232='Red' (blob0100035265640000), CaretHighlightOpacity Property225='0.25' (010004302e32350000), and ParenthesizeOperands Property229='True' (010004547275650000). They are UserScopedSettings; current user's override values were not read. Separate OvertypeAdorner1438/1439 also uses a red brush+opacity and DrawRectangle, but its call sites are SpecTableView/EditableComboBox and are not evidence for ordinary formula geometry. No exact native pixel parity is claimed.

## Reconstruction validation

`crates/mathcraft-editor/src/typeover.rs` models the bounded whole-root mode independently of Scope. Five focused tests cover first-key content/revision/history stability, both second-delete directions, PH Left focus, Undo/Redo of the red state, cancellation/explicit click, failed oversized replacement atomicity, and caller template-caret transplantation. Editor snapshots and history-free clones retain the temporary mode; a successful mutation saves it before clearing it. New navigation placement clears it after validating the target. Matrix selection likewise clears it only after range validation. Whole-node templates and unsupported boundaries are not silently discarded. Two additional regressions verify Evaluation/Definition/GlobalDefinition owner+sibling identity, native base label/suffix preservation, Power replacement rejection, and red-state Undo. Repository test totals are validation evidence, not a percentage of Mathcad feature completion.

## Addendum: physical definition/evaluation and paste dispatch

Installed Help `definition_operator.html` establishes `:` as the regular definition shortcut (hash in `area-and-definition-help-evidence-20261019.json`). Generic InsertOperator017b validates readonly/expressions-only mode before routing the command. AllowExpressionsOnly rejects DEFINITION/GLOBAL_DEFINITION before mutation. In Typeover n.u02bd routes through04f1/04f2 to be05c2; ordinary bare-Fraction selection can be deleted then replaced by a new DEFINITION(PH,PH), with left-PH focus. Its IgnoreOperandsOnOvertype flag does not mean preserve the old term. Definition descriptor RestrictParent ROOT (0bbb1a15..1a26) and precedence10 mean that replacement inside an existing evaluation can require whole-container reparsing to place Definition above Eval20. A direct subtree-only insertion of Define into Eval.left is not established as equivalent and remains unsupported by the bounded helper.

Public `ProcessEqualsKey`01a0 first attempts LOCAL_RANGE_VAR_DEF, returning early if that succeeds. This descriptor restricts parent to SUMMATION or PRODUCT (0bbb2668..2685), so the bounded root/worksheet-shell Fraction contexts do not match. If EvaluationRoot is already numeric EVALUATION,01a0 skips insertion and only calls RequestEvaluation; it does **not** clear Typeover or delete the selected term. If none exists and expressions-only is false, Model.d01ef→b.a0233→n.bd02c1 first calls b.p0235 to insert EVALUATION at the equation's right extreme, then deletes the original Typeover segment through n.j. For whole Fraction this yields an existing/new Eval whose LHS becomes PH, Left entry, and Typeover clears. RequestEvaluation is invoked when this succeeds. Getter0f9 recognizes equation-root Eval or an Eval directly on Definition/GlobalDefinition RHS. In an existing Eval the callback-only branch preserves the tree and pending selection; a definition whose RHS has no Eval may acquire an Eval on RHS before the selected fraction is deleted. This is not generic blank evaluation-operator replacement.

Pasted/factory expressions use n.v02be→ax04f2, which supplies the selected NavigatorSegment and IncludePrimalChildren to be05c2. A new term is built before the transactional replacement; failure preserves source and temporary selection. This establishes replacement rather than old-term wrapping, but not a universal pasted-tree cursor: operator fixity and segment geometry determine bg05c6 focus. The portable transplant API imports a caller's verified caret without inventing those policies. Full Power-postfix paste/value reparse and arbitrary nested definition replacement remain unsupported.

Raw follow-up proof is retained in `feedback/typeover-equals-*-cil-20261019.txt`, `typeover-public-key-guards-cil-20261019.txt`, `typeover-term-segment-expansion-cil-20261019.txt`, `typeover-segment-boundary-node-cil-20261019.txt`, `typeover-linear-segment-recovery-cil-20261019.txt` and `typeover-postdelete-placeholder-cil-20261019.txt`. This correction supersedes the initial incomplete whole-Power interpretation; neither the tests nor feature inventory should claim that old interpretation.
