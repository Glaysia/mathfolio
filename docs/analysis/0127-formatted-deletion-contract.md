# Formatted value deletion and identifier fusion (Prime12)

Static installed-artifact REA, 2026-10-19. No original GUI, screenshots, target runtime, CLR or COM execution. This extends [0122](0122-structural-deletion-contract.md); failed-delete Typeover remains the distinct [0126](0126-typeover-deletion-contract.md) investigation. These outcomes are derived from CIL, rather than recorded original UI observations.

## Artifact and retained evidence

E = `C:\Program Files\PTC\Mathcad Prime 12.0.0.1\Ptc.EquationEditor.dll`, SHA256 `d419032c0e99a62eab2f278e571a338d8c4b9662655af8ab462d482a028c0223`, MVID `d8627dca-3258-470c-b4d1-c00bbe2f2441`.

Fresh REA managed-members parent `ev_61e9f3e139e7c0c98a765193f83930bc0855ec975bf367f90175b723d4ff6d8c`, summary `ev_c97e61173dfa578c77ea9d195c7b3aec168419b352377d094af7f2e5ca40b9dc`: selected at/r, identifier info, formatted string and XML transforms; seven returned types, 279 methods, no unmatched types, metadata coverage complete. Some #US terminal flags are reported noncanonical; readable UTF16 remains retained. Method bodies were independently read from the installed PE and their exact raw CIL SHA256 checked against REA metadata. Tokens below are build-local coordinates, not stable public APIs.

Ignored receipts: `artifacts/rea/feedback/formatted-deletion-rea-summary-20261019.json`, `formatted-deletion-method-ledger-20261019.json`, `formatted-deletion-{character,label-format,numeric,repair}-cil.txt`. Initial all-method parent remains `ev_419ab24312df266f2e58ff6b91d32e0261a9a2bba9105ba09ebecb7517b43fdf`.

## Native value coordinates and character deletion

0435 first handles a selected ValueRange; otherwise it returns at whole-value start+Backspace or whole-value end+Delete, permitting structural stages. The insertion flag does not replace that whole formatted coordinate. For `AB` followed by literal suffix `dc`, suffix-local0 is native offset2, and suffix-local2 is native offset4. A base-only end offset2 is not the whole identifier end.

The method clones/removes from FormattedString for identifiers and uses String.Remove for numeric info. It computes DefaultCaretInsertionType before mutation (0435 IL00e2..00ef), reclassifies a newly simple value through factory002e4 while retaining compatible NodeInfo (021b..0241), repairs, and restores the resulting cursor (02c4..0307). 1328/1329 use the character preceding a nonzero caret; position0 uses the first character. Nonzero subscript depth selects Subscript insertion.

| Represented input/key | Derived result | Own caret mapping |
|---|---|---|
| `AB_dc`, suffix-local0 Backspace | `A_dc` | Normal base offset1 |
| `AB_dc`, Normal base-end2 Delete | `AB_c` | Normal base offset2 |
| `AB_dc`, suffix-end2 Delete | character stage returns; structural stages may reach neighbor operator | whole-end mapping, not toggle-only |
| `AB_c`, suffix-end1 Backspace | `AB`; removed empty suffix inline | pending Subscript offset0, saved suffix absent |
| `V_a𝛼`, entire suffix selected in either direction, Backspace/Delete | `V`; selected suffix removed | Normal base offset1, saved suffix absent |
| imaginary `12j`, semantic whole Right Backspace | numeric `12` | Normal offset2 |
| imaginary `12j`, coefficient-end2 Delete | numeric `12` | Normal offset2 |
| imaginary `2i`, coefficient-start0 Delete | identifier `i` | Normal offset0 |

The pending suffix caret after removal is editor state, not a saved empty span. The original constructor accepts the previously computed insertion mode even after reclassification. Own `editing_text`/`place_subscript_selection` therefore permit an empty pending field on NativeIdentifier without a saved suffix. Subsequent typing can create a suffix again. This does not add a document undo entry for caret placement.

Whole-suffix selection is distinct from collapsed suffix-end deletion. 0435 IL00da..00ef passes the pre-edit SelectionStart to1328;1329 IL0036..0044 reads position-1 when that start is nonzero. At the first suffix character, that is the preceding baseline character, so deleting the complete suffix selects Normal insertion, even when the original selection was made in Subscript mode. 0435 IL02f6..0302 preserves that computed mode through constructor130d (SHA256 `08c5818a8aa55aeb4f47a0cae11bb783e362cdae5ee13da4d7705f903a569baa`). Thus the next Backspace removes the final baseline character rather than merely exiting suffix mode. This is a static-derived contract, not an original runtime observation. The core regression covers both selection directions and delete keys, exact UTF16 offsets, and one-step Undo/Redo.

Native Remove operates on UTF16 units. Own edits reject a boundary that splits a Unicode scalar; this is an explicit portable implementation limit, not a claim that the original handles graphemes identically.

## Fresh NodeInfo and label priority

0493 concatenates plain strings or clones/appends formatted strings, then creates a fresh candidate through002e6/002e9. It does not clone the entire left NodeInfo. 1079 initializes a fresh identifier with nullable IdLabel unset, ResolvedIdLabel=None6 and empty algorithm. 1080/108a set only the nullable IdLabel field; they do not copy contextual caches or algorithms.

0493 IL0149..0184 contains a special branch when aq.a equals the previous left value. The ordinary delete caller045b creates aq with d/e=true, sets f=true, and leaves aq.a null (0135..0141). Thus this caller cannot take that equality branch: for a resulting identifier it copies the left identifier's IdLabel if present. The right explicit label does not override an unlabeled left value. Numeric left info carries no IdLabel.

125e reader IL00a0..00e4 makes `label-is-contextual=true` mean IdLabel remains null while the serialized label becomes ResolvedIdLabel. Absent/false contextual means an explicit nullable IdLabel. 1176 writer uses explicit IdLabel when HasValue; only a null explicit label may use a non-None resolved cache and write the contextual attribute. Consequently contextual UNIT/FUNCTION must not become explicit after fusion. Explicit None6 is distinct from nullable null and serializes as `*`; `NONE` is the converter alias established in0093.

| Left metadata, right metadata | New identifier semantics |
|---|---|
| explicit UNIT, explicit FUNCTION | explicit UNIT |
| contextual UNIT, explicit FUNCTION | nullable label null; cached UNIT is discarded |
| ordinary unlabeled, explicit FUNCTION | nullable label null |
| explicit `*`/`NONE`, explicit VARIABLE | explicit None6 (`*`), no contextual attribute |

Own name-only null result is Identifier; explicit result is AnnotatedIdentifier. When a suffix requires NativeIdentifier, nullable null/ResolvedNone is represented as `labels="*", contextual=true`; this is not explicit None. Subsequent existing bounded bridge inference is independent of this editor label transfer. It does not reconstruct the original complete contextual-label resolver. Unknown explicit metadata remains atomic unsupported; known enum labels use the native reader's first space-separated label token. A contextual cached string is not promoted to an explicit choice.

## Formatted append and representation boundaries

0d73 dispatches a simple right value to Append(string)0d74; that appends to the final root Run or creates a new baseline Run after a suffix Span. A nonsimple right value is cloned, its root children appended, and cleanup0d85 merges compatible Runs/Spans and removes empty inline containers. ToString0d7b/0d7e preserves subscript markers for factory matching; it is not a flattened visible base name.

| Operands whose operator is deleted | Derived formatted sequence / supported mapping |
|---|---|
| plain `x` and `y_dc` | baseline `xy`, suffix `dc`; own NativeIdentifier can represent it |
| contextual `𝑥` and explicit `y_dc` | same formatted append, join UTF16 offset2, nullable label from left |
| `x_rms` and plain `y` | baseline x, suffix rms, baseline y; not `xy_rms` |
| `x_rms` and `y_dc` | baseline x, suffix rms, baseline y, suffix dc |
| numeric2 and `x_dc` | formatted candidate starts with a digit and fails identifier matching; separate operands become Scale |

The latter interleaved baseline/suffix sequences cannot be saved by the current single `name + optional suffix` IR. They are rejected atomically, rather than reordered or silently flattened. Pure-suffix values after deleting the complete base are likewise outside that IR. This is a concrete representation gap, not an original prohibition or uncertainty about Append. An empty left suffix followed by a simple right value is also rejected, because Append(string) does not run the nonsimple cleanup path.

## Imaginary numeric lexemes and fusion

The editor coefficient/symbol split must be converted back to the original complete string before0493 factory matching. Imaginary(2) is `2i`, and ImaginarySymbol(2,j) is `2j`; `1` followed by `2j` fuses to `12j`, not real12. Original writer119c creates `<imag symbol="j">12</imag>` from that string; symbol preservation is explicit. Conversely `2i` followed by3 produces invalid `2i3`, so preprocessing inserts OP_PLACEHOLDER and repair changes it to Scale. Its directional caret follows0122: Backspace enters the RHS left; Delete enters the LHS right. Own whole imaginary end uses the semantic Right boundary; coefficient-local end lies before the suffix character and is not whole end.

The represented decimal/scientific/imaginary and plain-name factory is reused. Radix/base-number spellings, complete namespace/punctuation names and the full Unicode-category descriptor regex remain incomplete. A potentially valid unsupported radix spelling is not silently treated as a failed factory and changed to Scale. Values not representable by the existing numeric IR are rejected atomically.

## Editor integration and verification

`Editor::delete_formatted_value(backwards) -> Option<bool>` handles the complete-value character stage for suffix/imaginary values. Some(true) is one source edit; Some(false) is a represented unsupported edit with source/caret/history unchanged. None means an outward boundary or another value handled by the existing text route. The app must delegate an outward suffix boundary to `delete_structure`, rather than toggle insertion mode and stop. Scope and Typeover are separate modes and retain their own routing.

The character-stage route also covers name-only NativeIdentifier and AnnotatedIdentifier. These are still the same installed IdentifierNodeInfo category after suffix removal; sending a subsequent final-character delete to a generic string editor would incorrectly create an empty identifier. The factory path0435 IL021b..0241 instead produces a placeholder for empty text, retaining its Left caret, and reclassifies a numeric remainder through the numeric descriptor. Represented remaining names retain their explicit label; incompatible numeric NodeInfo drops identifier-only label metadata. Core regressions verify whole-suffix removal followed by another Backspace, exact two-step Undo, last annotated-name deletion, remaining-label retention and `x12`→numeric12.

The structural candidate uses the shared factory helper. Successful fusion allocates fresh value info and computes join offset as new native value length minus the old right native value length (0493 IL01b9..01d9). Name/suffix range mapping does not move cached labels into explicit state. Candidate validation enforces atom/node/depth bounds before the original single checkpoint; failure preserves revision, cursor, selection, source and redo. Existing0122 reparsing and native container boundaries remain in force.

Local focused formatted tests:12 pass; full current editor215 pass; all-target Clippy(-D warnings) passes after the question-mark correction. Whole workspace, app keyboard/native reopen and both-OS final checks are recorded by the integration owner; these local results do not substitute for that gate.

## Verified raw CIL bodies

| Token | Method | Raw IL SHA256 |
|---|---|---|
| `0x06000435` | `ap.aj` | `e5160af8053dd8a74264e5aa4517247f10e00f1ed937d1b91131b812cb1c5ce1` |
| `0x0600045b` | `ap.a` | `91d7d57905c9d7e6f49cb5f979806af169326160168cd3ec026a29ac3e71d1c6` |
| `0x06000471` | `aq..ctor` | `2b82d886276226b05e0cfd1b22d0c833e3c735fc97d7834c467dc9cb5861bb03` |
| `0x06000493` | `at.b` | `15926f51041e45c0e74368556914ed4c20d4a380f309a4fd109d98a91fe4c3e3` |
| `0x06000494` | `at.a` | `28e495bb509de33b85bca2e9772e1205d0c2e8d4357c640e09c4dabe848a4fdd` |
| `0x0600049c` | `c.b` | `f8503bb4aa0b49d324ea47d333cc90706be3954919a5bec833e1afaec200860d` |
| `0x060002e6` | `r.b` | `98e508cec039cb54178bac1decfdfc55029a2421a119f73a232d39bf46650d6f` |
| `0x060002e8` | `r.a` | `cfccdd3517cffcac275a60048ffeb19643f240daa08ba5f5d3cfb69084712611` |
| `0x060002e9` | `r.a` | `dd7ffe7cb85e1dc1702f2a236a8c64296af66d42408e1083c063b351eeecf3a1` |
| `0x060002ea` | `r.a` | `f2d149923f589069bfb54d619f6792ec29ee69a3fe811d39c91a9d130cb97af0` |
| `0x06000bc8` | `Ptc.EquationEditor.Tree.MathNodeDescriptorTable.FindMatchingValueDescriptor` | `22d683590e025a1224054a7f15f884af80c60382a6fc8b5bfe6dab77d0bc6c14` |
| `0x06000bc9` | `Ptc.EquationEditor.Tree.MathNodeDescriptorTable.IsMatchingDescriptor` | `ed06b4bd4526b6835acadbd1437a3ecf2667d13f2097adfc258a8ba0252b05ab` |
| `0x06000d6e` | `Ptc.EquationEditor.Tree.FormattedString.get_Length` | `714d8b902a9050d2d63ef74d6ff2286c95fd5197b20bc58c356a18f73b09db16` |
| `0x06000d6f` | `Ptc.EquationEditor.Tree.FormattedString.get_IsSimpleString` | `8e0e858065821f8080126c798554ea4732b02edec0f1179dce64a8e21c28bbbe` |
| `0x06000d73` | `Ptc.EquationEditor.Tree.FormattedString.Append` | `1e30f3a7c8838b4e4e31ddd8b198b4f5deccbff7d536b702039e3bdf4e8faa75` |
| `0x06000d74` | `Ptc.EquationEditor.Tree.FormattedString.Append` | `f61de358208261ca8d6ff6c8115d47ef928739cde1d7606b8f7481fcd8e8ca2c` |
| `0x06000d75` | `Ptc.EquationEditor.Tree.FormattedString.Remove` | `ae48a9ccadc10f15bce1ebfa462576e0fad9edbfdd807ab5ca84f5107467ef00` |
| `0x06000d79` | `Ptc.EquationEditor.Tree.FormattedString.GetSubscriptDepthAtPosition` | `940ad1b348e76ef4530363d387752313840e626100c1645ae83bce782949c6dc` |
| `0x06000d7b` | `Ptc.EquationEditor.Tree.FormattedString.ToString` | `5e392718c62e3f8b47e2447837615db45fcbb77fa27f2323a2c81678584f7572` |
| `0x06000d7e` | `Ptc.EquationEditor.Tree.FormattedString.a` | `70103bf390f2e10f57b011f8585ec14918294d19fbc7221a2fe28a6a85503086` |
| `0x06000d85` | `Ptc.EquationEditor.Tree.FormattedString.b` | `2673646bb2b6f495395b301f91d8856cce57fd26085f8f2e2ba6b1c6c8122cca` |
| `0x06001053` | `b.c` | `ec54fa4e57f770c824ecda1c297fddf3459020d5d62d2d0a9a8c06ea2f44cd03` |
| `0x06001079` | `Ptc.EquationEditor.Tree.IdentifierNodeInfo..ctor` | `6b4cfe91a58ae2ab36f31027585c7e7a2641c24eeb280b94ed42331e6d61197f` |
| `0x0600107f` | `Ptc.EquationEditor.Tree.IdentifierNodeInfo.get_IdLabel` | `379dcf7962d32c8f57b5346a18a916a4bfb9c766b6d668a9fddf6ced438ebb8e` |
| `0x06001080` | `Ptc.EquationEditor.Tree.IdentifierNodeInfo.set_IdLabel` | `ac0b0c3626d707d08db86de5d5a69b8e7377ee46aa14bf1cda4d3dc840804dc6` |
| `0x06001081` | `Ptc.EquationEditor.Tree.IdentifierNodeInfo.get_HasNoLabel` | `df05d28a8545836de85fc2fdd6155341e59d4870d0dd4070f004916411bfd979` |
| `0x06001082` | `Ptc.EquationEditor.Tree.IdentifierNodeInfo.get_ResolvedIdLabel` | `70dd9097148719b64ea37fcfd640977c90a865fb1b4f040bbfd313cf8c2c7629` |
| `0x06001083` | `Ptc.EquationEditor.Tree.IdentifierNodeInfo.set_ResolvedIdLabel` | `cfe559f019322b5fe8d32e75c31c55f8ec2b62268113c530f098f19dbf53b765` |
| `0x0600108a` | `a.b` | `5c668bdb7af4cde86961a4ef76694948049a127254d9432b06bce7953d73684c` |
| `0x06001176` | `Ptc.EquationEditor.Tree.Transform.MathNodeToXmlTransform.h` | `e4e82c7c8863695980620eda29a334866387c286f2d9ca35822e26822b2cf6ed` |
| `0x0600125e` | `Ptc.EquationEditor.Tree.Transform.XmlNodeToMathNodeTransform.a` | `2929e0d2ec78839dc0a44615a5c526f39b1fa223bc9895602831bdfe076b9cb3` |
| `0x06001328` | `Ptc.EquationEditor.Tree.Navigation.NavigatorState.DefaultCaretInsertionType` | `ccdae4fe550e3b619bad7e466799ef3a99865c93f560ad86fa3963262a37c75e` |
| `0x06001329` | `Ptc.EquationEditor.Tree.Navigation.NavigatorState.a` | `c039b1877580107b7f3c579bd57098fe448f2e29a460469658d43eba601f413d` |
| `0x0600119c` | `Ptc.EquationEditor.Tree.Transform.MathNodeToXmlTransform.e` | `a925fed378b6e4bee31218741e2a79d3674376094faec3b14890bc8c39c48855` |
