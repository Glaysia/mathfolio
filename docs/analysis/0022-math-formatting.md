# Math formatting commands and persistence

This pass connects Prime's formatting controls to observable document state. Evidence is static CIL, shipped templates, and metadata only. No original process, CLR, GUI, or screenshot was used. Font-family, label color, and 11-point math-size facts are detailed in [0018](0018-prime-typography.md); ribbon hierarchy is in [0019](0019-prime-ribbon.md).

## Source identity

The installation root is `C:/Program Files/PTC/Mathcad Prime 12.0.0.1/`. Tokens are build-local and require the corresponding assembly identity.

| Assembly | SHA-256 | Evidence |
|---|---|---|
| `Ptc.Controls.Core.dll` | `654a0252f699c9c9763aa4f4102488279500ed89cb8ce285a38476c2dd24a977` | `ev_f208e991f9e1e1e33663525464eb86305e6bee7352187fed8011d74326426e64` |
| `Ptc.Controls.EquationControl.dll` | `0b6fd580eb6cc183f17b5b5e0967226bbafc5e12efc6da809f44cd16416bf9d1` | `ev_389338a9845c7ea83dfd310922316cf92ca1637bdbac6602a01c4690e52b8349` |
| `Ptc.EquationEditor.dll` | `d419032c0e99a62eab2f278e571a338d8c4b9662655af8ab462d482a028c0223` | `ev_419ab24312df266f2e58ff6b91d32e0261a9a2bba9105ba09ebecb7517b43fdf` |

Retained decoded CIL is in ignored `artifacts/rea/prime-appearance/controls-core-il.json` and `artifacts/rea/editor/{equation,control}-il.json`. Those supplemental decoders resolve token names against REA metadata and retain REA raw-IL hashes; they are not recovered original C#.

## Numeric defaults and bounds

`NumberFormat..ctor` in Controls.Core, token `0x060004d5`, initializes these values:

| Property | Default |
|---|---|
| DecimalPlaces | 3 |
| ShowTrailingDecimalZeros | false |
| NumberStyle | General (native value 7) |
| ExponentStyle | multiply by a power (native value 0) |
| ComplexStyle | native value 0 |
| ZeroThreshold | 15 |
| ExponentialThreshold | 3 |
| RadixStyle | decimal (native value 0) |
| ComplexThreshold | 10 |

`NumberFormat.IsValidDecimalPlaces`, token `0x060004b9`, accepts nullable inherited state, integers **0 through 15**, and sentinel **-1**. `NumberFormatSerializer.get_Precision` / `set_Precision`, tokens `0x06000b52` / `0x06000b53`, serialize the sentinel as `round-trip-decimal-places`. Missing precision falls back to 3 on writing. The milestone's unsigned 0–15 control is a bounded subset; it does not implement the round-trip sentinel or inherited/mixed control states.

This setting counts **decimal places**, not significant figures. `RealNumberFormatter.GetFormatStringForDouble`, token `0x06000597`, builds a fixed decimal pattern using mandatory zeroes when trailing zero display is enabled and optional digits otherwise. Sentinel -1 requests round-trip formatting. Thus the default decimal pattern is equivalent to `0.###`; setting 3 must not truncate a number such as 123.456 to three significant figures.

## General, scientific, and engineering display

Native NumberStyles coordinates recovered from `NumberFormatSerializer.WriteXml`, token `0x06000b71`, and named reader methods are:

| Style | Native value | Mathcraft ribbon dispatch value |
|---|---:|---:|
| General | 7 | 0 |
| Decimal | 2 | 1 |
| Scientific | 1 | 2 |
| Engineering | 0 | 3 |
| Percent | 6 | not exposed in this milestone |

The ribbon dispatch values are our independent interface and must not be mistaken for the native enum. XML uses named elements, so export need not depend on either numbering.

`RealNumberFormatter.FormatRealToGeneral`, token `0x0600058d`, chooses scientific or decimal using `NumberFormat.ShouldBeDisplayedAsScientific`, token `0x060004bd`. With decimal radix, a present exponential threshold `t`, and ordinary non-sentinel precision, the observed predicate is:

```
abs(value) < 10^-(t - 1) || abs(value) >= 10^t
```

The default `t = 3` therefore chooses scientific display below **0.01** and at or above **1000**. Equality at 0.01 remains decimal; equality at 1000 is scientific. Threshold 0 is locally treated as 1. With the round-trip sentinel, the small-value exponent limit is extended to at least 15. An absent exponential threshold follows a separate branch based on decimal places; this milestone does not expose that nullable configuration.

`FormatReal`, token `0x0600058b`, first applies `ShouldBeDisplayedAsZero` before style dispatch. The scalar predicate, NumberFormat token `0x060004cd`, is `abs(value) < 10^-ZeroThreshold` when that setting has a value, and false otherwise. Default 15 therefore suppresses values strictly below `1e-15`; equality remains nonzero. This predicate is absolute and takes no relative-component argument. `GetBaseVal`, token `0x06000590`, treats zero with exponent 0 and otherwise computes exponent `floor(log10(abs(value)) / step) * step`; scientific uses step 1 and engineering step 3. The default exponential notation is a multiplication by a base power. `CreateExponentialMultiplyByBaseToAPowerFormat`, token `0x06000594`, suppresses the multiplier when the formatted mantissa is zero or the exponent is zero, and renders exponent 1 without a superscript. `CreateExponentialENotationFormat`, token `0x06000591`, is the alternative notation. Plain `e` text in a reconstruction is a display approximation, not an exact match to the default original typesetting.

The predicate and these special cases are observed CIL logic. Replacement floating-point rounding, overflow handling, nonfinite numbers, radix variants, complex formatting, and every threshold interaction still require their own conformance work.

## Region formatting persists separately from the math AST

`EquationControl.GetRegionXmlNode`, EquationControl token `0x060000f0`, creates the worksheet-namespace `math` element, appends the equation AST, then writes optional local result and font overrides:

1. `resultFormat` is written when the display number format or result matrix format has a local value and expressions-only mode is off. `ResultFormat.WriteXml`, Controls.Core token `0x0600064d`, writes number and matrix formatting inside that container.
2. `formattingOverride` is written when math formatting is handled locally and at least one format property is set. Its serializer is instructed to omit unset properties.

`MathFormatSerializer.CreateChildren`, Controls.Core token `0x06000ad2`, maps local properties to these case-sensitive attributes: `FontFamily`, `FontSize`, `Bold`, `Italic`, `Underline`, `ForegroundColor`, and `BackgroundColor`. A math `FontSize="11"` is **points**; the math renderer's FontManager subsequently converts it to DIPs. This differs from the label-style `font-size="14.666666666666668"` in presentation settings, which carries the derived display size. Avoid double-converting either path.

Independent representative structure, using only the evidenced names and defaults:

```xml
<math xmlns="http://schemas.mathsoft.com/worksheet50">
  <!-- Math AST is stored here in its own namespace. -->
  <resultFormat>
    <general precision="3" show-trailing-zeros="false" radix="dec"
             zero-threshold="15" imaginary-value="i"
             exponential-threshold="3" complex-threshold="10" />
  </resultFormat>
  <formattingOverride FontSize="11" />
</math>
```

This is an explanatory reconstruction, not copied worksheet source. Format writers `WriteGeneral` (`0x06000b72`) and `WriteDecimal` (`0x06000b74`) write their named children. `WriteScientific` / `WriteEngineering` (`0x06000b75` / `0x06000b76`) additionally write `use-e-notation`, with the default `false`, and force radix `dec`. Scientific/engineering children carry precision, trailing-zero, radix, and imaginary-value attributes. Default presentation settings independently corroborate General, precision 3, false trailing zeroes, zero threshold 15, and exponential threshold 3.

The original distinguishes worksheet defaults from local overrides and preserves unset state. A per-region format containing concrete values is a useful milestone subset, but cannot yet represent all inheritance or mixed-selection semantics. Native export should preserve unknown settings and overrides while editing the supported fields.

## Identifier labels are semantic commands

`EquationControl.OnSetIdLabel`, token `0x0600017d`, sends the selected label enum through `VisualMathRegionNode.FireSetIdLabel`. `EquationModel.SetIdLabel`, EquationEditor token `0x060001ad`, dispatches an undoable command named `Set ID Label`. `CanPerformIdLabelOperations` follows the editor state, rather than enabling a style command on every nonidentifier node.

The six ribbon options Variable, Unit, Constant, Function, System, and Keyword change semantic identity and derive the default styles documented in 0018. They are not independent bold/italic formatting toggles. Explicit labels and engine-resolved contextual labels must remain distinct as established in [0016](0016-contextual-labels.md). The milestone ribbon emits `SetLabel` with the corresponding uppercase identity; the editor should validate the actual target identifier, and undo/redo must restore both identity and appearance.

## Implemented control surface and bounds

`ribbon.rs` now dispatches actual font-size, decimal-place, format, and label actions from the Math and Math Formatting tabs. The compact controls read current selected-region state. Matrix row/column insert/delete actions and XY plot insertion also dispatch through the same action surface; actual matrix ancestry and document edits are handled outside the ribbon.

The font control's 4-point minimum matches the original math font minimum. Its 72-point upper limit is a **milestone UI bound**, not a recovered Prime maximum. Precision 0–15 follows the observed non-sentinel range. Context-dependent disabled commands no longer claim that they are unimplemented; commands that are truly unavailable keep that explicit tooltip. This pass does not claim final visual, native export reopen, full label-selection, or rasterization parity.

## Raw CIL identity anchors

| Assembly | Token | Header offset | SHA-256 of raw CIL |
|---|---|---:|---|
| Controls.Core | `0x060004d5` | 43080 | `5361d10de2a1f20a7b55775e33b704b6c394b3c66862899e0321f6048e6a686f` |
| Controls.Core | `0x060004b9` | 41364 | `ae72f89d88c4712b7782ba37d4a4f7c278b538ef16ef691f6229db37ebc729b4` |
| Controls.Core | `0x060004bd` | 41572 | `bec5eded890157e0d9c33c31b5cab2f7c1e0190bd174be3e76cd331bb3cf7999` |
| Controls.Core | `0x06000597` | 52568 | `4cc4b32ff3fb4ae3e499b1040089b7b8d40ce54567a3dc8ca5b8e3c6abc691b9` |
| Controls.Core | `0x06000ad2` | 112272 | `a2a8fab991a73d2c81d4ecfbf87666ae9093873d63a3421d80701f79ab6c3bd5` |
| Controls.Core | `0x06000b71` | 121324 | `78511ae11e8bcc68b60b4255a31e7aae8e4299d232017d91834f0116ae96f020` |
| EquationControl | `0x060000f0` | 14368 | `e7bd3ee689b97548c17ffa55888042258a72ca13cb1e514fd9ab1d37c0a5cf82` |
| EquationControl | `0x0600017d` | 25470 | `c71d1f5e0397e1ae23d750bdbe6707bd099048165c17ace6972e97065fe85d51` |
| EquationEditor | `0x060001ad` | 37068 | `6f845a81f076d9eee8f457931dea1ddedd234d550148f375e43faf7de626077b` |
