# Parenthesis adjacency and closure contract (Prime 12.0.0.1)

Static investigation, 2026-10-19. No original application execution, CLR loading, GUI, or screenshots. This document distinguishes derived editor transitions from evaluation results; it does not claim runtime parity.

## Identity and evidence

`Ptc.EquationEditor.dll`: SHA256 `d419032c0e99a62eab2f278e571a338d8c4b9662655af8ab462d482a028c0223`, MVID `d8627dca-3258-470c-b4d1-c00bbe2f2441`, REA evidence `ev_419ab24312df266f2e58ff6b91d32e0261a9a2bba9105ba09ebecb7517b43fdf`.

`Ptc.Controls.EquationControl.dll`: SHA256 `0b6fd580eb6cc183f17b5b5e0967226bbafc5e12efc6da809f44cd16416bf9d1`, MVID `cf1d7cb5-c73f-4618-8e6b-b83959d13b98`, REA evidence `ev_389338a9845c7ea83dfd310922316cf92ca1637bdbac6602a01c4690e52b8349`.

Retained sources: `artifacts/rea/editor/{equation,control}-{members,il}.json`, focused `context-labels-deep/equation-il.json`, new `feedback/parenthesis-*-cil.txt`, method ledger `parenthesis-adjacency-method-ledger-20261019.json`. Method hashes below are raw CIL body SHA256, not whole-file hashes.

## Opening insertion

The GROUPING descriptor in E `0x06000bbb` IL `2e15..2eff` has native XML `parens`, one child, actual fixity Outfix(4), precedence299, two navigation parts [0,1], left/right caret allowed, container slot[0], Family1. Initializer CIL SHA `90ff4afc15ec4d75068d5d2c0eab08be91548ff38974f8de5b54e02a25a6376a`.

Normal InsertOperator `l.a` 0088 dispatches value atoms with caret positions through `ax`04d9. Full explicit UTF16 selection [0,length] uses the replacement path; collapsed start/end use directional insertion; middle/partial selection uses splitting. The absence of a text range is not proof of full selection. GROUPING Family1 avoids the ordinary existing-Outfix replacement branch (0088 IL02ff..033b).

`bg`05c4 IL00a0..00c6 changes newly inserted GROUPING's analysis-handle fixity to Prefix(1), retaining its actual descriptor Outfix(4). The prefix reduction 056c consumes the following operand. At an end boundary adjacency first becomes OP_PLACEHOLDER; repair chooses APPLY for identifier/prime/index followed by GROUPING, or SCALE otherwise. This choice checks descriptor shape, not resolved labels or whether the identifier is a function/unit. Native `m(3)` with m labeled UNIT consequently has APPLY syntax; success or an evaluation error is a separate question. Exact callable mapping: OP_PLACEHOLDER repair IL36c0..3708 contains literal MATRIX_INDEX at36dd, and APPLY callee allowlist IL3101..3137 repeats MATRIX_INDEX at3124. Its descriptor IL0d78..0e29 writes XMLFormat indexer at0d86..0d8b, arity2, right container[1], precedence170. Generic operator writer1177 uses this XMLFormat and reader1267 resolves it, so this is Mathcraft Index(ml:indexer), not literal subscript metadata. No HINDEX, FUNCTION, or FUNCTION_LITERAL name occurs in these rule arrays; such aliases must not widen the callable set. The full APPLY callee allowlist is EMPTY_FUNCTION_NAME, IDENTIFIER, PRIME, MATRIX_INDEX, OVERLOADED_BRACKET.

| Current state | Derived native structure after `(` | Initial cursor |
|---|---|---|
| Blank | Group(PH) | PH center |
| Number end/right boundary | Scale(number,Group(PH)) | PH center |
| Identifier end/right boundary | Apply(identifier,Arguments(PH)) | argument PH center |
| Completed Group right | Scale(oldGroup,Group(PH)) | PH center |
| Completed MatrixIndex right | Apply(index,Arguments(PH)) | argument PH center |
| Number start/left boundary | Group(oldNumber) | entry into retained number |
| Full selected atom | Group(oldAtom) | left-entry state of retained atom |
| Scope selected expression | Group(expression) | Group left/right matching scope direction |
| Numeric 12|34 | Scale(12,Group(34)) | suffix number text start |
| Numeric 1[23]4 | Scale(1,Group(4)) | suffix number text start |
| Identifier ab|cd | Apply(ab,Arguments(cd)) | suffix identifier text start |

Full-value wrapping is E053a IL00a7..00aa `InsertAsParent`, and cursor selection IL0157..016a. Scope GROUPING uses05d9 IL063b..0647 with the scope direction. Normal new blank grouping uses05c6 IL00fa..0111: Group center part0, then one Right, reaching PH. Native semantic boundary construction bq.c(Right,node)0602 IL0029..006b sets text range[length,length]. A raw NavigatorState(atom,Right) constructor is not equivalent: default enter-side initialization can start at0.

Partial splitting 05d4 obtains ContainerRoot, clones NodeInfo/ClearValue (0032..0049), uses FormattedString.Substring for identifier fragments and String.Substring for numbers, omits selected text, inserts the new Prefix handle between fragments, and reparses (01bb..01f2). Empty fragments become placeholders and preprocessing removes an unnecessary leading placeholder before Prefix. Simple identifier fragments are reclassified: factory002ea (SHA f2d149923f589069bfb54d619f6792ec29ee69a3fe811d39c91a9d130cb97af0) dispatches IsSimpleString to002e8;002e8 (SHA cfccdd3517cffcac275a60048ffeb19643f240daa08ba5f5d3cfb69084712611) selects descriptor through002e4(string,null) before attaching cloned metadata. Plain ASCII digit fragment12 matches NUMERIC.MatchRegex at0bbb IL0247..0267, so x|12 yields an identifier callee x and numeric argument12, not Identifier(12). Arbitrary formatted/nondecimal fragment classification requires the full matching rules; cloning NodeInfo does not preserve the old descriptor unconditionally. Whole formatted-name/subscript selection must be measured with native formatted UTF16 indices. Mathcraft's separate literalSuffix representation is not by itself proof that a base-only range is a full native identifier selection; rejecting unsupported suffix splits is an explicit implementation boundary.

`sqrt`/`abs` spelling alone does not select Root/Absolute operators in this path. The constructor FunctionOperatorsList does not contain either spelling. `about_entering_math_expressions.html` states3x implies multiplication and xy is one identifier; `square_root_and_nth_root_operator.html` gives backslash for the root operator; `absolute_value_of_a_scalar.html` describes abs(z). A shipped help native example has a named two-argument sqrt call; this demonstrates syntax and possible user definitions, not a proof that every version supplies builtin sqrt(x).

## Container boundaries and precedence

ContainerRoot acquisition is unconditional in05c4 IL0035..0076, before the GROUPING-only fixity override. Thus the boundary applies to ordinary inserted math operators too. Getter0c56 stops at a node whose parent IsContainerSlot(index) is true. DFT slots include Power[1] IL0d20, Fraction[0,1]0aeb, MatrixIndex[1]0ded, Group[0]2e92, Arguments[0]31cd, and all Program/BODY children. Add/Multiply/Scale/APPLY have no explicit slots; constructor initializes the empty slot array. Evaluation.child0 is not a declared native container slot: precedence20 keeps evaluation outside these higher-precedence expressions. An implementation may use an equivalent boundary, but must not call it an observed flag.

Consequences distinguish an atom's end from an explicit operator-right boundary:

| Position | Next insertion | Derived structure |
|---|---|---|
| exponent3 end in2^3 | `(` | Power(2,Scale(3,Group(PH))) |
| exponent3 end in2^3 | `+4` | Power(2,Add(3,4)) |
| denominator3 end in2/3 | `+4` | Fraction(2,Add(3,4)) |
| explicit Power right | `(` | Scale(Power(2,3),Group(PH)) |
| final3 in2+3 | `(` | Add(2,Scale(3,Group(PH))) |
| final3 in2*3 | `(` | Multiply(2,Scale(3,Group(PH))) |

These are static parser derivations, not captured runtime numeric observations. Linearizer04b9 treats both-container operands as opaque, right-container Power/Index as flattened left plus a postfix analysis handle with the right slot retained. General parser behavior must not be approximated solely by arithmetic precedence across container boundaries.

OP_PLACEHOLDER precedence200 is temporary. TransformTo0592 IL0000..0043 records whether precedence changed. Repair058a IL01d4..0230 then reparses the affected container through0564. SCALE precedence142 therefore controls the final parse. Handle constructor04a3 IL0098..00ab normalizes unspecified infix associativity toLeft; parser0567 IL0133..0177 reduces equal-precedence left-associative infix operators. Consequently an existing Scale(2,3) followed at3-end by grouping becomes Scale(Scale(2,3),Group(PH)); unary negative150 binds before Scale142. At the base atom2 end of Power(2,exponent), the flattened sequence is2,Scale142,GroupPrefix299,PowerPostfix160(exponent opaque); postfix reduction0568 assigns the preceding Group into Power.child0 before Scale reduces. Thus the derived result is Scale(2,Power(Group(PH),oldExponent)), distinct from explicit PowerRight -> Scale(Power(2,oldExponent),Group(PH)). This static derivation preserves the native analysis handles; it has not been captured in original runtime.

## Closing parenthesis

Actual binding is C GestureBinder.Bind0495 IL0017..0029 (`)` -> FireExitGroupRight), C CIL SHA `9cbf25768a68ebe2462fb6e240d70849cdcbea3872aee63cffeacdd5869108ad`. E Presenter0622 IL024f..0267 binds this gesture to CloseParen, E CIL SHA `d098fe8f08c9f5767e428e65782936c939ff3c4b6c2db665f997816dc44b749b`. Concrete E015a delegates to mode CloseParen; the previously suggested0484 token was not the handler.

Normal mode0079 constructs bq from current state, moves exactly one Right, and checks whether that next state has Family1 and NodeSideRight. If so it moves one more Right and commits only navigation (returnfalse/no document mutation). Otherwise it inserts GROUPING with an explicit Postfix(2) analysis fixity. Scope mode0286 invokes ordinary GROUPING wrapping instead. There is no nearest-ancestor search and no arity-zero call mutation in this handler.

bq060a removes text range/enter-side for matching the navigation list. Hence middle-text and end-text of the same atom have the same next structural state for this close command; a close helper must not require text-end just to leave an already matching Group. bq060b builds the list with060c;0600 uses adjacent list index. No JumpThrough filter is used in this path.

| Current structural position | Result of `)` |
|---|---|
| Final atom/PH inside Group | next Group center part1(Family1/right), then Group right; navigation only |
| Final argument atom/PH inside APPLY Arguments | next Arguments center part1(Family1/right), then Arguments right; navigation only |
| Final child through Add/Multiply/Scale | no ownRight interposes; matching outer Group can close |
| Final child through Neg/LogicalNot/Return | no ownRight unless slot/AllowRight; matching outer Group can close |
| Exponent atom inside Power inside Group | Power right interposes, Family is not1; fallback postfix grouping inside exponent |
| Denominator atom inside Fraction inside Group | Fraction right interposes; fallback grouping denominator |
| Explicit Power/Fraction right inside Group | next outerGroup closing state; exits outerGroup |
| Already Group right | no matching same-group close ahead; postfix fallback rather than nearest ancestor |
| Bare completed number/identifier | Postfix reduction0568 wraps previous atom as Group(atom); cursor Group right |

Infix060d emits ownRight iff right child is a container or AllowRight. Prefix060e analogously checks child0. Postfix060f ALWAYS emits child navigation, operator-center, operator-right. This matters for polar/degrees: POLAR initializer0bbb IL0bf7..0c7d is infix120, no slots/AllowRight/Family override; DEGREE IL2d88..2e08 is postfix180, AllowRighttrue, Family defaultnull. From explicit DegreeRight, navigation can pass through Polar's final child and close the outer Group. From numeric90 inside Degree, the next state is DegreeCenter, which must not be skipped. Degree insertion's normal postfix cursor is the explicit DegreeRight boundary.

This proves that typing `(2^3)` while still at exponent3 is not automatically equivalent to leaving Power then closing the outer Group. An explicit ArrowRight to PowerRight before `)` produces the outer close. The same distinction applies to a fraction denominator. Parenthesis examples claiming an outer close must state that navigation action.

## Native calls, serialization, limits

APPLY repairs GROUPING to ARGUMENT_LIST (DFT30db..30fc). Arguments are Family1 with container0/Righttrue, JumpThroughtrue; this does not cause additional skipping in the close pipeline. Transformer1178 emits arguments as sequence under APPLY; for a single nonseparator child it returns the transformed child directly (005e..0080). Placeholder1172 emits `<placeholder/>`;116e only unwraps NoLogicOnlyVisual unary nodes. No optional-placeholder dropping or empty-call-arity cleanup was found in these captured methods. Static survey of shipped native help XML found4701 named APPLY nodes, zero zero-argument calls and zero one-placeholder calls. This is absence of a positive sample, not proof that zero-argument calls are forbidden. See `artifacts/rea/parenthesis-native-call-survey-20261019.json`.

Bounded implementation should not claim arbitrary literalSuffix splitting, every unsupported operator's navigation state, raw-constructor equivalence, or all caller-side postprocessing from this contract. The native editor parse path, not numerical evaluation, establishes the syntax table. Native schema alone cannot establish actual calculation acceptance.

## Help hashes

| Shipped help file | SHA256 |
|---|---|
| about_entering_math_expressions.html | a14179af3ff4653d27f22019de4c129ce772378b406286eaf9f82fcd80f4e290 |
| square_root_and_nth_root_operator.html | 09892e43920d42d8ae151c4370d4808b04e0e9fbef941d9216b6e23fa6a1f485 |
| absolute_value_of_a_scalar.html | 559e5a491937ce62f87099b785ad73a2b017b3769efa864f803e45b1abd35a1f |
| keyboard_shortcuts_for_algebra_operators.html | 83cb8fd824ce84bf123e8ad62ade0fb43160d5d6f5947b3a0c3c97f00021e1a5 |
| example_looping_and_control.html | 736819a3230afb48e9a6b678a0bd26f09b1722111f38793892e60a294a577712 |

## Method body ledger

| E token | Method | Raw CIL SHA256 |
|---|---|---|
| 0x06000088 | l::a | d2e82ba51ec7275cbe5e6b911eaf06401ff6a634851661ae6462fb31e78c74e6 |
| 0x060004d9 | ax::a | 0f370e2df8a25d14436c655db42b7b8b4ffde38843443bb088e6a5074245b978 |
| 0x060005c4 | bg::bq | 25d960678b62705f5d8537a636f5d5997588222b4a40f9e3af9b9c8ab0dd39e5 |
| 0x0600056c | a4::d | 7dec8495acef818b49a2e8738d8248c9391d085afba18cc8088b1a7ce4b3bd03 |
| 0x060005d4 | bk::bq | 87158c560d81ce05a28fc3e890b330777f49848e1ca74e655f86ffccf16a7adc |
| 0x0600053a | a2::g | 06cacd01672daf0a2a86ea0bf1ec35747c422a484a4c6a1ee83fbafb78365aed |
| 0x060005c6 | bg::a | 3bc6ac5566ace12d0ee48c1c7b8795bbaffb44737548ba64e75b270f85ef6a69 |
| 0x06000c56 | Ptc.EquationEditor.Tree.MathNode::get_ContainerRoot | 972c9d3809b54161a88ed249607f37b22dd36a55cfec4b1d32eed4a1733afffd |
| 0x06000ca9 | Ptc.EquationEditor.Tree.MathNode::IsContainerSlot | b0c823e4f923a7c1a62b3040bc80b346a649e1628dcded85cd749625cfe12f89 |
| 0x060004b9 | aw::e | 6da8936382101eba061218fb998362348611f4fd6b51fc025f1c4625b556d443 |
| 0x06000592 | a::a | 7eb80db1fe6c3ee0d489e6224c31caad712587198cd1ef19368277593113e70a |
| 0x0600058a | a6::a | 9dd1ba695580acfdb117baf9ec9e66d935abec094dbb64371239ed30e72fd1fc |
| 0x06000564 | a4::a | 2556d48e4f0501a3e7833f9f3ca7683a4baf48088b2a83e8036776f107fc6927 |
| 0x060004a3 | Ptc.EquationEditor.Tree.Parsing.MathNodeAnalysisHandle::.ctor | acb67204ca305298a85aa10a49361c91ab96018ad41dcf8b34fb628f7057fd41 |
| 0x06000567 | a4::a | 89423e939abbfec7cfe3ad0e1de55166d02983a931e2d8e24694cfdbbc6dee32 |
| 0x06000568 | a4::h | bc0c4cba5c1bf7240ea8f9321979e168c0db571398d56982e06ca74a4eb0d5aa |
| 0x06000602 | bq::b | b8895b847f85c77805434631282167e9420536025d77c168f1a7d56fd878f424 |
| 0x06000079 | l::n | 913b1c7457997c0ba419a21cd570891ef25b18a1c01536834b6801660583736a |
| 0x06000286 | m::n | e98541330f7daa6ee27a623f4cf597a4ef7cee22071546972320ea10aff18115 |
| 0x0600060c | bq::f | 42ead008bbbf5ddbf30f54951ecfcd994c966bc6d13c0ac530b3222947eec087 |
| 0x0600060d | bq::e | 9c0e87635657600ad6ce47cdbcf86351e5eb7e38032f08c473f3971ebb2e6b72 |
| 0x0600060e | bq::d | b6376032d55ea5236dcbcf20ccffe63af1007cdfb621fc12919b11ac7f060b7b |
| 0x0600060f | bq::c | 11b0ac08a2f346c6e450b962526993361615f39785f68306b9fe74a3976f7f37 |
| 0x06001178 | Ptc.EquationEditor.Tree.Transform.MathNodeToXmlTransform::TransformArgumentsList | 80de3abd04ba458643efa132ace8570e0fe103677c2c98ae1e64ef0b8c73cad0 |
| 0x06001172 | Ptc.EquationEditor.Tree.Transform.MathNodeToXmlTransform::TransformPlaceholderNode | a6104914f73b7928f603bae7fb08a63611189cfd2a5578ef3302d578defaf14a |
| 0x0600116e | Ptc.EquationEditor.Tree.Transform.MathNodeToXmlTransform::ConvertChildNodes | 0cc4320f4d5a29595e722ea87ba8ae7d35c61de69f40682be483b820af73df5b |
| 0x060002e4 | r::a | 297c0fe30a01d5f970d96749c9154c65a0dc4a542c060c1412aac95d411100eb |
| 0x060002e8 | r::a | cfccdd3517cffcac275a60048ffeb19643f240daa08ba5f5d3cfb69084712611 |
| 0x060002ea | r::a | f2d149923f589069bfb54d619f6792ec29ee69a3fe811d39c91a9d130cb97af0 |
| 0x06000bc7 | Ptc.EquationEditor.Tree.MathNodeDescriptorTable::FindMatchingValueDescriptor | 57fbc2bdf72c46214bc3bde7739c2e88ed6ac6852798ca523e62c25074957c28 |
| 0x06000bc9 | Ptc.EquationEditor.Tree.MathNodeDescriptorTable::IsMatchingDescriptor | ed06b4bd4526b6835acadbd1437a3ecf2667d13f2097adfc258a8ba0252b05ab |
| 0x0600105c | c::c | ab8379573623ac78ec1dee3375a7af425e37e0f6cabd1977b0e7979b56008e21 |
