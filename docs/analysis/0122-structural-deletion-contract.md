# Ordinary structural deletion contract (Prime12)

Static REA investigation, 2026-10-19. Original GUI, runtime, CLR/COM execution and screenshots were not used. Product sources were not edited. [0104](0104-try-deletion-contract.md) remains the separate TRY contract; its special handlers must not be generalized to ordinary operators.

## Identity and sources

E = installed `Ptc.EquationEditor.dll`, SHA256 `d419032c0e99a62eab2f278e571a338d8c4b9662655af8ab462d482a028c0223`, MVID `d8627dca-3258-470c-b4d1-c00bbe2f2441`, REA `ev_419ab24312df266f2e58ff6b91d32e0261a9a2bba9105ba09ebecb7517b43fdf`. C = `Ptc.Controls.EquationControl.dll`, SHA256 `0b6fd580eb6cc183f17b5b5e0967226bbafc5e12efc6da809f44cd16416bf9d1`, MVID `cf1d7cb5-c73f-4618-8e6b-b83959d13b98`.

Retained metadata and CIL: `artifacts/rea/editor/`, focused decoded evidence `context-labels-deep/`, new `feedback/structural-delete-*.txt`. Installed byte/body verification is `structural-delete-installed-verified-cil.txt`; full method ledger is `structural-delete-method-ledger-20261019.json`. The investigation does not claim to execute the original stage machine.

Root extracted shipped help text only in `artifacts/rea/deletion-help-evidence-20261019.json`. `to_replace_or_delete_an_operator.html` SHA256 `dea0897eb9525f6b265710bb12be2a63c991ccfb95caf180db74069c1051704d` says unary deletion retains its operand, two integer operands merge, and a binary with a noninteger operand becomes scaling. When an operator cannot be deleted without its operands, the term is selected in red and a second Backspace removes the term. Tutorial task2-2 explicitly gives5^2 ->52 after deleting the exponentiation operator and removal of both selected parentheses. The full static implementation below is more precise than the help's simplified integer/noninteger wording; its outcomes are derived from CIL, not runtime observations.

## One key is a staged transaction

C Backspace0188 ->FireDeleteLeft; Delete018a ->FireDeleteRight. E EquationModel.Delete0173 creates one command named Delete. Normal l.j0075 ->ax04e6 ->bh05c8 initializes initial/current navigator and direction. Main stage array042e contains35 handlers. Text deletion0435 is stage2; generic operator0433/leaf/outfix/container/program handlers precede stage21; stage21 ap.t0447 performs one internal navigation step; ordinary operator handler ap.x0443 appears at stage14 and32. Stop-on-handled runner0432 means one Delete command may navigate to an operator and remove it without a separate arrow key.

Normal l.j distinguishes a TypeoverState result from a navigator/document mutation. a0.b0524 opens a CompoundCommand; if rewrite fails it undoes that compound and calls bh.bs05c9. The rollback path runs a separate handler list and can create a TypeoverState (red selection). Working ao.b navigation alone is therefore not proof that the actual caret is committed. bh05c8 also rejects a successful document result when the resulting expression is unchanged. An independent implementation must preserve redo on no edit; it must not silently drop filled operands or commit speculative traversal from an unhandled branch.

## Text selection and ordinary text deletion

ap.aj0435 first handles any explicit nonempty ValueRange selection. It removes precisely the selected .NET UTF16 range using FormattedString.Remove for IdentifierNodeInfo or String.Remove for StringValueNodeInfo, retaining compatible metadata. Full text selection is still text deletion, distinct from Scope of a subtree. With no selection, Backspace at position0 and Delete at valueLength return from this handler and permit structural stages to proceed (004f..00a3). Other positions delete one code unit at/preceding the caret, normalize the descriptor using the factory, repair, and choose the resulting text cursor. Unicode scalar/grapheme policy is not established merely by .NET Remove; a portable implementation can reject invalid scalar boundaries.

Scope mode m.j027e is separate: it checks readonly, obtains the selected LinearSegment, invokes ax04ea scope deletion and restores Normal mode after success. Deletion direction is not used by that method to choose a surviving operand. Scope of a whole term can remove that selected term; it is not equivalent to clicking an ordinary operator center while retaining operands. General partial-linear Scope repair remains outside the bounded reconstruction below.

## Ordinary operator removal and operand preservation

ap.x0443 requires CaretSideCenter(4), remembers the candidate node, then calls045a. 045a skips nodes with a primal child or n-ary nodes, accepts ordinary Prefix1/Postfix2/Infix3, and chooses a directional neighbor bias (Prefix/Infix Right, Postfix Left) before045b. In particular, a filled RHS is not discarded simply because the operator is deleted.

045b obtains ContainerRoot, asks the analysis builder to expose the deleted node's children, removes the operator handle (012c..0130), and parses with aq.f=true (0135..0153), then repairs. Thus existing Fraction's container operands and Power's opaque exponent are deliberately exposed for this deletion. Reusing the r121 insertion normalizer's unconditional operand-adjacency Scale would miss the value-fusion stage; simply keeping child0 would lose data.

### Confirmed integer positives

| Starting state and key | Derived result | Cursor |
|---|---|---|
| Add(12,34), RHS text0 Backspace |Number1234 |UTF16 join2 |
| Add(12,34), LHS textLength Delete |Number1234 |join2 |
| Explicit Add center, either key |Number1234 |join2 |
| Fraction(12,34), explicit bar center |Number1234 |join2 |
| Fraction numerator end Delete / denominator start Backspace |Number1234 |join2 |
| Power(5,2), explicit operator center |Number52 |join1; also explicitly supported by shipped tutorial |
| Power base end Delete / exponent start Backspace |Number52 |join1 |

Boundary text handler0435 returns at the outward edge. bq navigation060d and stage21 then reach the parent operator center; stage32 performs the same removal as stage14's explicit center. This is one transaction, not a first key that merely places the caret on Add.

### Fusion guards, metadata, and a retained discrepancy

Parser iterator049c IL00b3..00e2 executes general fusion0493 when aq is null or aq.d is true. aq ctor0471 sets d=true and e=true;045b sets f=true. An interim message read brfalse00c6 backwards; the verified branch skips fusion only when d=false. The initial integer join path is valid. Do not cite the interim skip interpretation.

0493 requires adjacent value-carrying, non-ExplicitValue nodes. Two StringValueNodeInfo values are concatenated and passed to002e6 (008f..00c7); formatted identifiers use FormattedString.Append and factory002e9 (00cd..0138). A successful valid node must pass0494. Numeric12 and34 therefore become a new numeric1234, not reuse/drop one operand. 0493 IL01b9..01d9 records newLength-rightOldLength, hence join2. It removes the old previous handle and replaces the current handle (01de..01f6).

For an identifier result, the helper may transfer IdLabel from an old identifier (0149..0184); this does not prove every contextual/format field is copied. Unlike insertion splitting, factory002e6/002e9 starts without a cloned old NodeInfo. Numeric radix licensing/mode is checked by0494; its visible code is not a general IsInteger predicate.

### Resolved ordinary name/decimal rule: valid lexical fusion, then Scale fallback

The complete static chain has been rechecked independently by the editor worker. aw04b3/04b8/04bf expose value handles without mutating their descriptors or ExplicitValue. Descriptor ctor0f74 leaves ExplicitValue field004be false; NUMERIC and IDENTIFIER initializers do not set it. Getter0cf0 delegates to descriptor100d, not a value-dependent integer flag. The three explicit-value assignments elsewhere in the table do not apply to these ordinary nodes.

A candidate fusion is created by the same factory used elsewhere:002e6 ->002e8 for two plain strings,002e9 ->002ea for formatted append. For simple xy and1.23, factory descriptor matching succeeds. IsValid0c67 ->0c98 ->0c9a checks ValidValueRegex; its remaining helpers validate parent/children/pattern/descendant/arity constraints, and add no integer-only test for a plain zero-child value under a root. 0494 checks radix/base-number licensing/mode, not integerness. After repair,045b uses ay's candidate cursor directly if Current.IsImplicit is false (0181..019e ->0259). No later Scale override or restoration of the deleted operator was found for these plain successful fusions.

| Ordinary operands after deleting operator | Static derived outcome | Cursor |
|---|---|---|
| integer12 and34 |Number1234 |join2, both key directions |
| plain unlabeled identifiersx andy |Identifierxy |join1, both key directions |
| decimal1.2 andinteger3 |Number1.23 |join3, both key directions |
| decimals1.2 and3.4 |fusion1.23.4 invalid; Scale(1.2,3.4) |Backspace ->RHS text0; Delete ->LHS textLength3 |
| numeric2 andnamex |fusion2x invalid for numeric/name grammar; Scale(2,x) |Backspace ->RHS start; Delete ->LHS end |

For failed fusion, handles remain separate. Preprocessor048d IL0021..0054 inserts OP_PLACEHOLDER and records a navigator at its center through0496; normal repair turns that same node into SCALE. 045b sees Current.IsImplicit=true, considers whether previously opaque container operands need grouping, and at0227..0238 moves one navigation state Opposite(deleteDirection) from the implicit operator center. The false branch at0202 also lands at0227: this cursor move happens even when no grouping was needed. Backspace(Left) therefore enters the RHS from the left; Delete(Right) enters the LHS from the right. A simple deleted Add has no old container slots to restore.

This explains why an insertion normalizer that always adds Scale is incorrect for successful lexical fusion, and why concatenating every pair is also incorrect. The installation's static code supports the table above; H192's two-integer/noninteger wording is a simplification rather than a sufficient classifier. These are **static derived** outcomes. They have not been recorded from an original running application. General radix/imaginary syntax, formatted literal-subscript joining and all contextual metadata conversions are not claimed from the plain-name/decimal table. Native explicit IdLabel transfer is observed, but a portable implementation may initially restrict fusion to plain unlabeled names and represented numeric spellings until it reproduces the additional metadata rules.

## Grouping fences

Outfix handler ap.af043b runs at stage7 and26. It requires actual Outfix4, NodeSide other than4, and exactly one child (000c..002b); it does **not** test deletion direction. Family1 GROUPING bypasses the conservative precedence branch used by other outfix operators. aw04b7 expands the child into the enclosing analysis list and removes the grouping handle; parse and repair follow.

For isolated Group(Number12), outerLeft or outerRight with either Backspace or Delete removes the pair and retains12. Cursor follows the old NodeSide: Left ->left entry/text0; Right ->right entry/textLength. Group center fence parts0/1 carry Left/Right NodeSide and use the same handler. A raw generic center with NodeSide4 is not the same state. Both parentheses are removed, not one displayed character.

For non-Family1 outfix nodes, the handler compares parent/child precedence and associativity. It may convert the descriptor to GROUPING while retaining operands instead of unwrapping. Do not extend the Group result to Absolute, Matrix, determinant or any outfix by appearance alone. When removing Group inside a larger expression, its content is reparsed with neighbors; the old child subtree is not guaranteed to remain a single opaque operand. Metadata/merge cursor across neighboring formatted atoms needs its own bounded handling.

## Fraction and Power placeholder boundaries

| State | Confirmed structural transition |
|---|---|
| Fraction(PH,34), numeratorPH +Delete |remove bar, retain34, left entry/text0 |
| Fraction(12,PH), denominatorPH +Backspace |remove bar, retain12, right entry/textLength |
| Power(12,PH), exponentPH +Backspace |remove exponentiation operator, retain12, right entry/textLength |
| Fraction numeratorPH +Backspace |one working step reaches outerFractionLeft, not bar center; no forced unwrap inferred |
| Fraction denominatorPH +Delete |outerFractionRight interposes; no forced unwrap inferred |
| Power exponentPH +Delete |outerPowerRight interposes; no forced unwrap inferred |

After operator removal, placeholder normalizer0495 removes a redundant leading/trailing PH next to a complete operand and records that operand's entry state. A completed value at exponent/denominator text0 is different from deleting its last character: normal text deletion has priority at interior positions. Explicit outerPowerRight/FractionRight Backspace can first traverse into a filled child; the eventual rollback/Typeover result must be established before claiming a committed navigation or rewrite. These outward-boundary and double-placeholder cases remain limited; existing arbitrary last-leaf descent is not evidence of parity.

## Ordinary unary and root determinant refinement

045a chooses neighbor biasRight for Prefix/Infix andLeft for Postfix. If no parser-recorded cursor supersedes it,045b IL023b..0258 normalizes Opposite(bias) on the retained analysis neighbor: Prefix center deletion enters its operand from the left; Postfix center deletion enters it from the right. PostfixRight+Backspace reaches that postfix center in the internal stage21 step, so Factorial/Transpose/Percent/Degree retain their operand and focus its right entry.

CONJUGATE and VECTORIZATION are actual Prefix1 operators with container[0] and AllowLeft/Right, not Outfix. Full-container unary handler043e accepts their center deletion and Left+Delete, retains child0, and returns its left entry. After child replacement,043e IL009e..00aa runs a3 normalization on the parent before root reparsing. a3.055e (SHA `f8b10b6317b97fff459d28f262dc8bce14012e97d6fd22cc008cfdf5f6d43092`) IL00e2..0114 inserts grouping where a lower-precedence child needs to retain its subtree under a stronger parent. Therefore Multiply(Conjugate(Add(1,2)),3) center deletion yields Multiply(Group(Add(1,2)),3), not Add(1,Multiply(2,3)). A portable generic remove-prefix-and-reparse path needs this additional repair or must reject that nested case atomically. It explicitly returns for the outerRight state before generic traversal; Right+Backspace is not proof of force-unwrapping these operators. Existing independent tests requiring that unwrap should be corrected to a native operator-center positive rather than justify an arbitrary fallback.

DETERMINANT is Outfix4, Family defaultnull. At a root, RootMathNode.cctor10a6 (SHA `acc2692e0c70101bd9ce3fefb9dba8edbd5e21526e6d95dd2fa2072e02b4ee7a`) creates descriptorROOT with default precedence0, sets NoVisual and childCount1, and does not change precedence. For scalar12 (or another operand with precedence>0),043b's non-Family1 parent-precedence conversion guard does not fire, so root Determinant12 outerRight+Backspace unwraps and retains12 with right-entry cursor. This bounded root positive does not remove the precedence/associativity guard for nested non-Family1 outfix operators.

## Actual binary glyph hit

C VisualLinearBinaryOperatorNode.GetNodeAtPoint0892, SHA `920d6e5d668ca51c4ab90ead38b99524cb424e74c8ae420afd2f9c42aa65263d`, obtains VisualSymbolNode.SymbolBounds.Left/Right (0001..002e), transforms the point into operator coordinates, and for Left<=x<Right (005a..006c) returns VisualMathNodeHitTestResult(this,4,4,point) (009a..009e). Thus an ordinary visible binary glyph has explicit caret center and node center. The remainder routes to first/second operand hit testing; splitting the whole compound rectangle into left/right halves cannot produce this state. Installed C bytes were checked in `feedback/structural-delete-binary-pointer-cil.txt`. C region.GetHitTestResult0a17 delegates to GetNodeAtPoint, SHA `8471fa16a315a0ef3df7ec088bc2bddf7c82c31f575f6964f5eb53dd12826122`.

Selection paint is also distinguished statically. C05cc MeasureOverride clones OperatorElement.IGeometryProvider.Geometry into _operatorGeometry, raw SHA `761480c3ce25da1386209d4e2fa756a2b1655a7c334585fda3652e0ba7a0f34f`. C05cd OnRender fills that glyph/shape geometry with _caretBrush and a null pen, SHA `1bfd6070c15f097ce690add174ec5c48d45bb8a8e3046e9cdaec14905dd0d8dd`: the operator highlight is foreground geometry, not a blue rectangle behind the symbol. Operand highlighters are created by05c5/05c6 and generic0935 as RubberBandHighlighter. C0933 binds its Color/Opacity to settings NearestNeighborColor/NearestNeighborOpacity, SHA `aefb1451069b0356b1c6e4aea189dce2c6dd7d1832904376dc02ed4e0ad41ae1`. C052d builds rectangle geometry from bounds; C052e fills it with _brush, SHA `b25c9f2d9b8afd5b86b21b719d02bf8d073e1da96107e22911ac023e7760e7d0`. Thus gray operands use a bounds fill/highlighter overlay rather than gray text foreground. Exact RGB, opacity, overlay stacking, every selected-state visibility branch and multikey pointer dispatch are not established here. Additional captured CIL files use structural-delete-binary-selection, binary-geometry, operand-highlighter and highlight-fill prefixes.

## Concrete existing implementation gaps and limits

At investigation start, `Editor::delete_structure` immediately rejected a non-PH leaf at a parent boundary, arbitrarily kept one child for a PH, and unwrapped Group only on Right+Backspace. These disagree with confirmed integer joining and Group states above. Its populated binary outer-right fallback chose a last leaf without the native rollback/Typeover contract. Existing TRY, matrix, program-line and selected-scope code has separate evidence and must not be replaced by symmetry.

Implementation-ready positives here are represented valid lexical value joins (integer/plain-name/decimal table), failed-fusion Scale with directional cursor, the corresponding operator-center state, isolated Family1 grouping removal, and the specifically listed inward PH cases. Arbitrary formatted-name fusion/contextual metadata, generic Special/program operators, outer-boundary Typeover selection, double-placeholder repair, and all partial Scope deletion are explicitly incomplete. One command should be source-atomic with cursor/history restoration; no unsupported path should silently throw away a filled operand.

## E raw method body hashes

| Token | Method | SHA256 |
|---|---|---|
| 0x06000075 | l::j | 3a29cc5f97d477bbe5329e3896f79d0b5236c0898acfaf3c3e38ee077952d559 |
| 0x06000173 | Ptc.EquationEditor.Equation.EquationModel::Delete | d1f9ec8d9a4021489c69227bf6c03096daab5f25c12e669f1db7b70c6677006c |
| 0x0600027e | m::j | 000112932d37e77df01886de6678a71718a8b98828ecd129ab1b142413796c51 |
| 0x0600042e | ap::b | b98fe5c621a8025d7612d3a83874e72e8dbdce39ea75c237217986068ee57263 |
| 0x06000430 | ap::an | fe0ba2c667cf2850df85258636bd62990a13e7aa4c2fe6741b91204353f9a383 |
| 0x06000431 | ap::am | bc56a398c257dd5cc98343d5d8390676e08436f6b05da0201173826d56df35e4 |
| 0x06000432 | ap::a | cf922c95dacd5e9ec4e196cc9d2c599b1b0d7a1361ad4b8ec866cd78996d477c |
| 0x06000435 | ap::aj | e5160af8053dd8a74264e5aa4517247f10e00f1ed937d1b91131b812cb1c5ce1 |
| 0x0600043b | ap::af | f3ab8b295c629cc23bd5bc395337d480ac1d079667757030b3b1bf741621fb04 |
| 0x0600043c | ap::ae | 439c1eea6c3426979e41c137238144890d8191693698b406ca8df7c2986ae0c7 |
| 0x0600043d | ap::ad | fa402b7022a6d17774441a1038b3303110861ed1f11008547b1705f6984d88e4 |
| 0x0600043e | ap::ac | 84afcfd095ea09dccc9df38afd25861800935ed5a626a931cf5d3bc15b2535a5 |
| 0x06000443 | ap::x | 97217653b27cb2c3288ff1857612de223e2da9f8d318787bfcc39aa8ef81f3ff |
| 0x06000446 | ap::u | 76a9e25120fa3dfc5495b128b72c43247a00b291563ef8ee016b3d483a22cc07 |
| 0x06000447 | ap::t | f9d5b2c4750f7d13052c2480c0a391f12e19707443fbb0b0072a4159ebc6c0b3 |
| 0x06000448 | ap::s | 020acab0ecf0444021c30f89e8e51c2e2c6574f4acc9b3fb7dc9431c6eaf4f86 |
| 0x06000449 | ap::r | df4a4c7cbdfec2beea3a01b23b6f4764e2d2a5938b74bd51ccc6d440158fe31d |
| 0x0600044e | ap::m | f3a2b5689bc8eacd5594a61561a00c72e9d9bdce494ad82d20d6982993d373bc |
| 0x06000450 | ap::k | fd5ff6298e6b78a17a36cdfd9cbde11fcc32319d55e99f69466d1d014a0774d7 |
| 0x06000455 | ap::f | 7b8a1042594b97451b7f5c7cad7173f7e250b650b9774b44bc1c269aa4e643e5 |
| 0x0600045a | ap::a | 1c1a9e1807edef651e281b92d9fbf8d0840ed64599023b826102d1c095822484 |
| 0x0600045b | ap::a | 91d7d57905c9d7e6f49cb5f979806af169326160168cd3ec026a29ac3e71d1c6 |
| 0x06000471 | aq::.ctor | 2b82d886276226b05e0cfd1b22d0c833e3c735fc97d7834c467dc9cb5861bb03 |
| 0x06000493 | at::b | 15926f51041e45c0e74368556914ed4c20d4a380f309a4fd109d98a91fe4c3e3 |
| 0x06000494 | at::a | 28e495bb509de33b85bca2e9772e1205d0c2e8d4357c640e09c4dabe848a4fdd |
| 0x06000495 | at::a | 4796919fb8a71976f9990211a14fd423f97e38b293adfc4d81ecdae4e33eb6d1 |
| 0x0600049c | c::b | f8503bb4aa0b49d324ea47d333cc90706be3954919a5bec833e1afaec200860d |
| 0x060004a0 | c::f | b5de69bc08a9fe8069b0ff96d225eea5266bca79ccf0fd31c4174e1d64156a17 |
| 0x060004b7 | aw::a | 2e367b95442a2ce632f1e8d3e0798ad8e057606823b1ba39ee863307e2427a3f |
| 0x060004e6 | ax::a | 14eccf2568e9f2d4b794da659641fda9b821a4557a07b6de08120b6b75bf7ee6 |
| 0x060004ea | ax::b | b704818fcfc0fd1b5130004d47376d9319bd0b8ce3fc1dc216e7720a915c6abb |
| 0x06000524 | a0::b | b0c241e89aac8755a76e2f86e7bb03bbcc2267b743dab4b37c532c652514aab8 |
| 0x060005c9 | bh::bs | c45fca78297c6f242784e5f45a439f45128ecc74bd283a99dd3096d4f6111c61 |
| 0x06000600 | bq::c | 46ca7eee9126d02c7e6e6d739fe5973aea13db14544cee65e49d6d0578359ece |
| 0x0600060c | bq::f | 42ead008bbbf5ddbf30f54951ecfcd994c966bc6d13c0ac530b3222947eec087 |
| 0x0600060d | bq::e | 9c0e87635657600ad6ce47cdbcf86351e5eb7e38032f08c473f3971ebb2e6b72 |
| 0x0600060f | bq::c | 11b0ac08a2f346c6e450b962526993361615f39785f68306b9fe74a3976f7f37 |
| 0x06000c56 | Ptc.EquationEditor.Tree.MathNode::get_ContainerRoot | 972c9d3809b54161a88ed249607f37b22dd36a55cfec4b1d32eed4a1733afffd |
| 0x06000ca9 | Ptc.EquationEditor.Tree.MathNode::IsContainerSlot | b0c823e4f923a7c1a62b3040bc80b346a649e1628dcded85cd749625cfe12f89 |
| 0x060002e6 | r::b | 98e508cec039cb54178bac1decfdfc55029a2421a119f73a232d39bf46650d6f |
| 0x060002e8 | r::a | cfccdd3517cffcac275a60048ffeb19643f240daa08ba5f5d3cfb69084712611 |
| 0x060002e9 | r::a | dd7ffe7cb85e1dc1702f2a236a8c64296af66d42408e1083c063b351eeecf3a1 |
| 0x060002ea | r::a | f2d149923f589069bfb54d619f6792ec29ee69a3fe811d39c91a9d130cb97af0 |
| 0x060005c8 | bh::br | 3acd6d0e157fedc948767a260648fd6ef6345f8126b53506c2dcb2615b245c1d |

## Selection default settings

C base editor BindToSettings0928 binds CaretColorProperty to MathcadSettings.Instance.CaretColor (raw SHA `eeb9139190e80b1ffb064eed23d026a2910b984fac368af0b921682441401133`). C binary editor05cf converts the color to a solid brush, SHA `cc17eab2b887dd0aa102b6b4fef6476fb75ffec4ad49794d06f21f4549a64b68`. MathcadSettings026b/0279/027b delegate to EquationProperties.Settings.Default getters0428/043e/0440. The property metadata DefaultSettingValueAttribute gives CaretColor property219 = `0, 128, 255` (#0080FF), NearestNeighborColor property230 = `DarkGray` (WPF named color #A9A9A9), NearestNeighborOpacity property231 = `0.25`.

These attributes were read directly from installed PE metadata using dnfile, with no CLR loading, and DLL SHA checked. Blobs:219 `01000b302c203132382c203235350000`,230 `0100084461726b477261790000`,231 `010004302e32350000`. Evidence `artifacts/rea/structural-delete-color-default-attributes-20261019.json`. Shipped EquationControl.dll.config contains the same defaults, but names the older Ptc.Controls.Properties.Settings section, so it is corroboration, not the primary binding/default proof. Settings are UserScoped: this establishes defaults, not the user's live overridden theme. No runtime theme or current-color reading was performed.
