# Normal infix entry at text positions (Prime12)

Static REA contract, 2026-10-19. No original GUI, runtime, CLR loading, or screenshots. This extends [0114](0114-parenthesis-adjacency-contract.md) for ordinary operator commands; syntax transitions below are derived from captured CIL, not runtime numerical observations.

## Identity and evidence

Installed `Ptc.EquationEditor.dll` SHA256 `d419032c0e99a62eab2f278e571a338d8c4b9662655af8ab462d482a028c0223`, MVID `d8627dca-3258-470c-b4d1-c00bbe2f2441`, REA `ev_419ab24312df266f2e58ff6b91d32e0261a9a2bba9105ba09ebecb7517b43fdf`. Descriptor initializer0bbb raw CIL SHA `90ff4afc15ec4d75068d5d2c0eab08be91548ff38974f8de5b54e02a25a6376a`. Retained editor metadata/CIL is under `artifacts/rea/editor/`, focused decoder subset under `context-labels-deep/`. Focused new installed-byte verification: `feedback/normal-infix-entry-verified-cil.txt`; companion replacement/dispatch/alternate evidence files use the `normal-infix-` prefix. Full token/hash ledger: `normal-infix-entry-method-ledger-20261019.json`.

## Dispatch: Normal text range is not Scope

`l.a`0088 IL0158..01c0 checks current CarriesValue, HasCaretPositions, and !DistinctNodeEdges, reads SelectionStart/End (missing values default-1), and calls ax04d9. It does not wrap the entire atom unconditionally. 04d9 selects these paths:

| Native text range | Path | Semantics |
|---|---|---|
| explicit [0,length] | 04ee ->04ed ->replacement | replace selected atom, no retained prefix/suffix |
| collapsed [0,0] |04d1 Left | insert before the atom in its container |
| collapsed [length,length] |04d1 Right | insert after the atom in its container |
| middle collapsed or partial nonempty range |bk05d4 | replace selected text with an operator between retained prefix/suffix |
| no range | values default-1; not full range | caller/navigation preconditions matter; never infer full selection |

04d9 IL000e..0024 tests full selection before the collapsed-start test. Scope selection enters a separate scoped-insertion command, with selected subtree preservation and a directional operand contract; it must not be represented as a full UTF16 text selection.

## Split and parse

bk05d4 first rejects a new PairNarySubElement (0000..0012), obtains the old atom's ContainerRoot and parent/index (0013..002c), clones NodeInfo and clears its value (0032..004b), and creates two fragments. IdentifierNodeInfo uses FormattedString.Substring(0,start) and Substring(end), numeric StringValueNodeInfo uses .NET String.Substring with the same indices. Selected text between start/end is removed. Both fragments must be created successfully (00fe..010a).

The list gets prefix, inserted operator handle, suffix (01bb..01ed); the old atom handle is removed (01ee..01f2). Parser runs (01f7..0200), replaces the same container parent/index (020c..0210), then repair runs (0215..022a). This makes arbitrary immediate-parent wrapping insufficient: parser boundaries and neighboring effective fixities govern final structure.

| Isolated plain atom and operator command | Derived result | Cursor |
|---|---|---|
| numeric12|34 plus |Add(12,34) | suffix34 text index0 |
| numeric1[23]4 plus |Add(1,4) | suffix4 text index0 |
| numeric12|34 multiplication |Multiply(12,34) | suffix34 text index0 |
| numeric12|34 fraction division |Fraction(12,34) | denominator34 text index0 |
| numeric12|34 exponentiation |Power(12,34) | exponent34 text index0 |
| identifierx|12 plus |Add(identifierx,number12) | suffix12 text index0 |
| identifierab|cd comparison/ordinary binary logic |Operator(identifierab,identifiercd) | suffixcd text index0 |
| numeric1234 full selected plus |Add(PH,PH) | missing left operand |
| numeric1234 full selected fraction/power |Fraction/Power(PH,PH) | missing left operand |

These tables describe the ordinary command descriptors, not every keyboard chord or multikey replacement gesture. Comparison/logical binary operators share the insertion parser when represented by ordinary infix descriptors. Their individual precedence/associativity still applies. Define/global define/evaluation have additional entry and same-family policies in0088 and are outside this table.

### Cursor after splitting

05d4 IL0232..0237 starts with suffix and directionLeft. If the inserted operator survives repair (0239..024b), leaf operators choose themselves/directionRight; otherwise its children are processed in order: a direct placeholder is selected immediately; for each nonplaceholder child whose IsInContainerSlot is true, that child's subtree is searched for its first placeholder before proceeding to the next child (026d..02d7). This is an interleaved child-by-child search, not a global direct-placeholder pass followed by a container pass. If the new operator disappears and prefix is PH, prefix becomes focus (02f0..02f9). The retained candidate must still belong to the repaired tree, else the parser's recorded cursor may be used (02fb..0329). Final032a..0338 calls bq.c(direction,candidate). A surviving ordinary binary whose two fragments are complete has no PH, so suffix left-entry is the result. It does not put the caret at the end of the whole expression. It also does not scan every old placeholder in ContainerRoot: for Add(PH_old,1234), rhs12|34 then plus yields Add(Add(PH_old,12),34), with cursor34 text0. A global first-placeholder fallback would incorrectly steal PH_old.

### Boundary insertion has a separate cursor rule

Collapsed start/end and explicit structural-side insertion use bg05c4, not splitter05d4. Its direction field is copied unchanged into local1 (002e..0034), then passed to05c6 (01d3..01dc). For an ordinary unrestricted infix,05c6 IL00db..00f2 constructs bq at the new operator center, checks whether it can move in that input direction, moves once if possible, and returns that navigation state. For completed Group(PH)Right followed by fraction insertion, the result Fraction(Group(PH_old),PH_new) focuses the right PH_new. Scanning its old numerator subtree would incorrectly focus PH_old. At an atom's start, the input directionLeft takes the new infix's adjacent left navigation state. Prefix05c6 chooses child0 directly if it is PH; otherwise it moves onceRight from the prefix center into its operand. Full replacement053e, partial split05d4, and boundary05c6 must remain distinct cursor policies.

## New operator handles versus existing expression linearization

New bg05c4 IL00f3..00fb and splitter05d4 IL0186..018e call MathNodeAnalysisHandle04a2. That constructor passes the new node's actual Fixity/Associativity to04a3. For ordinary Infix3 it retains Infix3 (04a3 IL0091..0093), even when child slots are containers. Therefore a newly inserted Fraction or Power consumes both retained fragments.

Existing trees use aw04b9 linearization instead. Both-container Fraction becomes an opaque effectiveNone operand; right-container Power/Index emits flattened base plus a postfix analysis handle retaining the exponent/index opaque. Outfix full containers likewise become opaque. General Special nodes have separate flattening logic and must not be inferred solely from an all-container flag. Existing Call is internally APPLY+ARGUMENT_LIST; slicing its callee requires that actual bundle, not pretending a flat portable Call is ordinary multiplication. The same bounded limitation applies to Scope transformations inside a callee when the portable helper cannot run native general parent repair. A container-root ancestry capability check can reject Call(Index(baseA,idx),args) edits from baseA (Index.slot0 then Call.callee0), while keeping edits inside idx (Index.slot1 is a native container). This is an implementation boundary, not a claim that Prime forbids the authoring action. Whole-Call selection remains distinct.

04a3 normalizes unspecified infix associativity toLeft (0098..00ab). Parser0567 and reductions056c(prefix),0568(postfix),056e(binary) consume handles according to native precedence. The two-pass repair of temporal OP_PLACEHOLDER200 to SCALE142 remains as established in0114. An implementation may use transient handles rather than storing a permanent unknown operator, but must preserve container slots, associativity, and old subtree identities.

## Container boundaries

General bg05c4 and splitter05d4 both work on ContainerRoot. Getter0c56 stops where parent.IsContainerSlot(index). Power exponent[1], Fraction numerator/denominator[0,1], Index[1], Group[0], Arguments[0], and Program/BODY statement slots are boundaries. Thus entering plus at exponent3 end in2^3 yields Power(2,Add(3,PH)); denominator3 end in2/3 yields Fraction(2,Add(3,PH)). Explicit PowerRight/FractionRight is a different navigation state and permits an operator outside the completed structure. Evaluation.child0 is not a declared native container slot; its precedence20 keeps higher-precedence math within evaluation. See0114 for descriptor IL offsets and the repaired Scale/Power-base distinctions.

## Minus is context dependent

SUBTRACTION descriptor0bbb IL063a..0704 has Infix3, arity2, precedence100, alternates[NEGATION] (06d3..06e3). NEGATION IL24xx..254d has Prefix1, arity1, precedence150, AllowLefttrue, default AllowRightfalse, alternates[SUBTRACTION] (251c..252c). Both are Family0.

Preprocessor at048a chooses an alternate by neighboring effective fixities. 047e/047f says missing previous or previous Prefix/Infix requires a following operand. 0482/0483 identifies Infix/Postfix as requiring a left operand. At048a IL0075..00c1 when a left operand is missing and current is Infix/Postfix, factory002fb selects an alternate whose fixity is not Infix/Postfix. SUBTRACTION becomes NEGATION. Conversely complete previous operand before a Prefix can select an Infix/Postfix alternate (048a IL0002..0048). Therefore explicit NEGATION between12 and34 normalizes to SUBTRACTION(12,34); it must not be assumed to yield Scale(12,Neg(34)). A prefix such as LOGICAL_NOT without an infix alternate retains its prefix and requires the implicit-adjacency path instead. These checks concern analysis handles, not a simple current-atom-kind test.

| Normal command | Derived result |
|---|---|
| start of1234, SUBTRACTION |Neg(1234) |
| middle12|34, SUBTRACTION |Subtract(12,34) |
| end1234, SUBTRACTION |Subtract(1234,PH) |
| blank/full selected isolated scalar, SUBTRACTION |Neg(PH) after alternate normalization |

The full replacement first discards selected text through053e; a lone Subtraction analysis handle then gets its Prefix alternate during parse. A blanket full-selected-infix ->two placeholders rule must account for this alternate.

## Full selected text replacement

04d9 full range goes through04ee/04ed into a2 replacement. 053e accepts oldChildCount0 and newChildCount>0, substitutes a new operator handle, reparses the container and repairs; text is discarded. It only transfers NodeInfo when the destination InfoType accepts the old metadata (00c7..0101), so a unit label must not be blindly copied onto an arithmetic operator. Final cursor for an ordinary infix checks its left-adjacent navigation state for PH and InitialChildPosition0(default):053e IL0294..02e7 moves toward the left missing operand. Prefix result uses its child0 left entry (01b5..01d3). The Normal full-range caller04d9 explicitly passes false to04ee. Handler053e itself does not insert a Group parent before whole-container parsing. For Multiply(2,1234), full selection of1234 then plus yields handles[2,Multiply140,Plus100], hence Add(Multiply(2,PH_a),PH_b), not Multiply(2,Group(Add(PH,PH))). Cursor is PH_a: at newAdd center, oneLeft finds that placeholder and InitialChildPosition0 chooses it (0294..02e7). Scanning the newAdd direct placeholder first would incorrectly focus PH_b. This full-replacement cursor differs from05d4 partial-entry cursor.

## Scope preservation and cursor

Scope05d9 differs from Normal splitting/full replacement. DirectionLeft0 fills the new operator's final child with the selected expression (03be..03de), DirectionRight1 fills child0 (0394..03b7), subject to each actual scoped-insertion rule. For ordinary infix, final059a..05ab calls bq.b(scopeDirection,newoperator), focusing the missing operand on that side. ScopeLeft plus produces Add(PH,selected); ScopeRight plus produces Add(selected,PH).

05da/05db/05dc test whether the descriptor can be inserted on the requested scope side; if not,05d9 IL01f0..0268 searches alternates. A Prefix NEGATION permits ScopeLeft and wraps the selected expression; cursor is bq(newoperator-center) then oneRight (0664..068e), entering the retained operand's first native navigation state, not an arbitrary first-leaf approximation. ScopeRight requires AllowRight, which NEGATION lacks, so its SUBTRACTION alternate is chosen. SUBTRACTION is already infix and permits both scope directions. Do not conflate ScopeLeft SUBTRACTION(PH,selected) with Normal-at-start conversion to unary negative.

## Metadata and UTF16 limits

Factory002ea dispatches simple FormattedString to002e8, which calls002e4(string,null preferred descriptor) before attaching cloned metadata. Therefore simple fragments are reclassified by matching descriptor. ASCII digit suffix12 of x12 becomes numeric, not Identifier(12). NUMERIC.MatchRegex is declared0bbb IL0247..0267. Non-simple FormattedString instead uses the formatted-descriptor list and IdentifierNodeInfo. Literal suffix/name formatting and explicit/contextual labels require the same formatted substring and compatible metadata conversion; copying only visible base text is insufficient.

Native .NET String.Substring indices and FormattedString offsets are UTF16 code-unit positions. The retained path does not establish grapheme-aware segmentation or prove that UI can produce a caret inside a surrogate pair. A portable implementation may reject an invalid scalar boundary atomically rather than inventing repaired Unicode. Numeric12|34, plain identifierx|12, and valid scalar boundaries are positive cases; arbitrary formatted subscripts, unsupported numeric spellings, and mismatched metadata conversions remain explicit limits until their own rules are implemented.

No original execution established calculation outcomes, native Undo storage shape, or every multikey operator overtype branch. The command pipeline is one logical insertion; Mathcraft candidate validation plus one history checkpoint is its independent transactional implementation, not a claim about Prime's serialized undo representation.

## Method hashes

| E token | Method | Raw CIL SHA256 |
|---|---|---|
| 0x06000088 | l::a | d2e82ba51ec7275cbe5e6b911eaf06401ff6a634851661ae6462fb31e78c74e6 |
| 0x060004d9 | ax::a | 0f370e2df8a25d14436c655db42b7b8b4ffde38843443bb088e6a5074245b978 |
| 0x060004d1 | ax::b | 0514cebf711dc69a7e0971a04023ce2ab380ba387dcdbbd6d1aedd636c01c792 |
| 0x060004d5 | ax::a | b5be9228eb51f7c4dbf360a8645890c3121e92443d928f30afce7f29f6df3f3e |
| 0x060004ee | ax::a | b161e07ba4890e8f795dcb2535753b80b9a1f798257b2e92fa4298c0f83e6484 |
| 0x060005c4 | bg::bq | 25d960678b62705f5d8537a636f5d5997588222b4a40f9e3af9b9c8ab0dd39e5 |
| 0x060005c6 | bg::a | 3bc6ac5566ace12d0ee48c1c7b8795bbaffb44737548ba64e75b270f85ef6a69 |
| 0x060004a2 | Ptc.EquationEditor.Tree.Parsing.MathNodeAnalysisHandle::.ctor | 15ae3834d70a9327d4b3669cba983b8b9ab92bb355b17160e7c723d0a1f528c0 |
| 0x060004a3 | Ptc.EquationEditor.Tree.Parsing.MathNodeAnalysisHandle::.ctor | acb67204ca305298a85aa10a49361c91ab96018ad41dcf8b34fb628f7057fd41 |
| 0x060004b9 | aw::e | 6da8936382101eba061218fb998362348611f4fd6b51fc025f1c4625b556d443 |
| 0x06000567 | a4::a | 89423e939abbfec7cfe3ad0e1de55166d02983a931e2d8e24694cfdbbc6dee32 |
| 0x06000568 | a4::h | bc0c4cba5c1bf7240ea8f9321979e168c0db571398d56982e06ca74a4eb0d5aa |
| 0x0600056c | a4::d | 7dec8495acef818b49a2e8738d8248c9391d085afba18cc8088b1a7ce4b3bd03 |
| 0x0600056e | a4::b | 4f5025b1875e3c997a94712240b0d04a40ac7e5d363fe4cfef4b70b6173ecba9 |
| 0x0600048a | at::h | 01618e183cdba8c6a872f551eab55649f97c10a1f38b9e72be9046b98cfb9770 |
| 0x0600047e | as::b | b0f0a0d7502d643eb9e60ad8b555e2cd586caf7b603de72e31592b54f31e01a8 |
| 0x0600047f | as::b | 078900818f9e8740ee42b8f7b89c6dfbc9441ca436f8d0def3ec5d6beec3ada7 |
| 0x06000480 | as::b | 89320af6f1159bfbe7e73448f35b23d9ad94a9b06e27d166b80c674b3fbca04a |
| 0x06000481 | as::a | b1a809b26bdc370de40f4f0d0bec046d33b1eac292b4b167ccde2d281e4548b6 |
| 0x06000482 | as::a | 66ecf07938d449885204f0a3fd1fb05a67ed4ae6ec0fe6b4e3d130347b52996b |
| 0x06000483 | as::a | fbc1096146a6ecce2f498d75504852021dd54bfb4a140deac66d82b3e5987a28 |
| 0x060002fb | r::a | 3948cd1ad3946c238e1ce706910b253f92d37c6fe162c27b864d6878fb068ec1 |
| 0x0600053e | a2::c | 5efe79dc7a2840ea0604dce868f545e85f757ee60a5fb9a6859247dc1804ad9f |
| 0x060005d9 | bl::bq | f148d85443e73a00d367e1c62e214985432ced2e469f475b955181ba75379dbe |
| 0x060005da | bl::a | 68aba3c90bbc2edaee3eab69752741a541cface1c895e3f518c01258343e6b80 |
| 0x060005db | bl::c | 9d234508502b6b3a7ecf2314ede48d1cb0f63bb786ae04157bf1cd273f70be77 |
| 0x060005dc | bl::b | f882ad5784cb428f5953aa92fe53c0569457ebf6ae41fb1b8f21fff0fcadb459 |
| 0x060005dd | bl::a | 70d4a59d9839e3a5a2bb6c8611b13c90859f7b257d9c2f576b2cbd0d1ff17e31 |
| 0x06000c56 | Ptc.EquationEditor.Tree.MathNode::get_ContainerRoot | 972c9d3809b54161a88ed249607f37b22dd36a55cfec4b1d32eed4a1733afffd |
| 0x06000ca9 | Ptc.EquationEditor.Tree.MathNode::IsContainerSlot | b0c823e4f923a7c1a62b3040bc80b346a649e1628dcded85cd749625cfe12f89 |
| 0x060002e4 | r::a | 297c0fe30a01d5f970d96749c9154c65a0dc4a542c060c1412aac95d411100eb |
| 0x060002e8 | r::a | cfccdd3517cffcac275a60048ffeb19643f240daa08ba5f5d3cfb69084712611 |
| 0x060002ea | r::a | f2d149923f589069bfb54d619f6792ec29ee69a3fe811d39c91a9d130cb97af0 |
| 0x060005d4 | bk::bq | 87158c560d81ce05a28fc3e890b330777f49848e1ca74e655f86ffccf16a7adc |

## Supplement: Ctrl+minus exit from literal subscript

The earlier0057 report established the gesture/delegation but did not establish concrete mode behavior. Static reinspection now finds Normal override l.m0078, raw CIL SHA `5158ee9fdbeb760904a138d5b3d1f6d25bb6249a88ba51824c4c88d032fe8887`: require a caret position and IDENTIFIER, then call NavigatorState.ToggleCaretInsertionType1323. Its raw CIL SHA is `ccdde69312b7a2fc5a8de344fc2c7a887b62f166d376017a04780e82af2b67ec`; IL000d..002c retains Current, CaretSide, OperatorPartIndex, EnterSide, and the exact ValueRange, changing only CaretInsertionType0/1. It does not move the caret to the base-name end.

For I_dc with the caret after dc, native full formatted offset is3 (baseI length1 plus suffixdc length2). Toggle-off preserves offset3 in Normal mode: a whole-identifier right-end position. A portable editor with a separate suffix field can translate that specific collapsed suffix-end exit to its semantic whole-node right boundary. It must not treat every Normal base-name offset1 as equivalent, nor map an arbitrary mid-suffix toggle-off to nodeRight: native preserves the middle formatted range as well. Retaining mid-suffix positions or rejecting operator insertion there is a separate representation boundary.

These two supplemental bodies were verified against installed bytes in `feedback/normal-infix-subscript-toggle-verified-cil.txt`; they are separate from the initial34-method inventory and do not constitute a runtime recording.
