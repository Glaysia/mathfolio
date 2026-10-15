# Prime 12 typography and math geometry

This report uses installed Mathcad Prime 12.0.0.1 artifacts and retained REA evidence. No original executable, CLR, COM, window, screenshot, or image was used. The following are static observations; they do not establish pixel-identical rendering in the replacement renderer.

## Source identity

All installed paths below are relative to `C:/Program Files/PTC/Mathcad Prime 12.0.0.1/`.

| Artifact | SHA-256 | MVID | REA evidence |
|---|---|---|---|
| `Ptc.Controls.Core.dll` | `654a0252f699c9c9763aa4f4102488279500ed89cb8ce285a38476c2dd24a977` | `b5e8e4fd-c134-4713-b1b1-eea8cddcdd9f` | `ev_f208e991f9e1e1e33663525464eb86305e6bee7352187fed8011d74326426e64` |
| `Ptc.Core.dll` | `ace9265338ef2aecf73b3e62540e026c5f0966df61a00127a2083110f4a2a4ab` | `c84389f9-d0db-4c3d-8363-9c28af318f99` | `ev_8ebe82dd9876db1a7a4243a15f754c914ecad3a7a7477f71c8e3a69d362ae721` |
| `Ptc.Controls.EquationControl.dll` | `0b6fd580eb6cc183f17b5b5e0967226bbafc5e12efc6da809f44cd16416bf9d1` | `cf1d7cb5-c73f-4618-8e6b-b83959d13b98` | `ev_389338a9845c7ea83dfd310922316cf92ca1637bdbac6602a01c4690e52b8349` |
| `en-US/Ptc.Controls.Core.resources.dll` | `63773814e3f4902894bb665be7f4a5d721ecaf33e79397d53db5911ad3390fd4` | `79ec4c51-6b0b-460a-868c-cbe8e412e8ca` | `ev_4bc292b00317042106aed2f5f82ff58e6d27d043d7d1dd45e505082a903bab5a` |

Supplemental decoded CIL, resource metadata, template facts, and independent parsers are retained under ignored `artifacts/rea/prime-appearance/`. The decoder resolves REA metadata tokens and preserves the REA raw-CIL hashes. It does not recover original C# source.

## Math size and styles

`Ptc.Controls.Core.FontConstants..cctor`, token `0x06000267`, establishes the math family `Mathcad UniMath Prime`, default size **11 points**, black foreground, transparent highlight, and regular upright base format. Noneditable math has a separate **DimGray** brush. `FontManager.GetFont`, token `0x0600025f`, converts point size to DIPs before constructing `ExtendedFont`. The actual `Ptc.Wpf.WpfUtils.PointsToDips`, in **Ptc.Core.dll** token `0x060005f7`, computes `points / 72 * 96`.

Therefore the default math em size at 100% worksheet zoom is **14.666666666666668 DIPs**. This is not an 11-pixel font. OS DPI scaling applies after document units, and worksheet zoom should scale the document rather than application chrome.

`FontConstants.GetDefaultIdLabelFormat`, token `0x06000265`, supplies the following defaults. Its parameter order is confirmed by `FontManager.GetFont`: family, point size, italic, bold, underline, color.

| Semantic label | Italic | Bold | Foreground |
|---|---|---|---|
| Variable, unspecified, or None | Yes | No | Black `#000000` |
| Unit | Yes | Yes | DarkBlue `#00008B` |
| Constant | Yes | Yes | DarkOliveGreen `#556B2F` |
| Function | No | No | Black `#000000` |
| System | No | Yes | Black `#000000` |
| Keyword | No | Yes | Black `#000000` |

All these label defaults use the same math family and size. Underline is false. `VisualIdNode.SetMathFormatPropertiesFromLabelFormat`, EquationControl token `0x06000746`, applies label weight/style/decorations/color to the identifier's math format; an explicit override-label-color flag prevents its color replacement. Missing/None labels explicitly request italic. A reconstruction should derive this style from semantic identity rather than merely recognizing text such as `m` or `sin`.

`VisualMathNode.SetOperatorFontProperties`, EquationControl token `0x0600090e`, forces the original math family and clears italic, bold, and underline. Thus operator styling differs from neighboring italic variable identifiers.

## Script sizing

The same font initializer defines:

| Setting | Observed value |
|---|---|
| `ScriptFontSizeRatio` | `7.0 / 11.0` |
| `SubscriptIDFontSizeRatio` | `0.7` |
| `MinimumFontSizeInPoints` | `4.0` |

`FontManager.GetScaledFontSize`, token `0x0600025e`, uses the current math format's point size (or 11), multiplies by the requested ratio, rounds to **two decimal places with AwayFromZero midpoint handling**, and clamps at 4 points. `VisualSuperscriptBinaryNode<T>.ModifyChildMathFormat`, EquationControl token `0x06000c96`, applies `ScriptFontSizeRatio` only to operand 1. The default exponent therefore has size **7 points / 9.333333333333334 DIPs**. Literal identifier subscripts have a distinct ratio, yielding 7.7 points at the default size. Do not merge these two ratios into a generic 70% script rule.

## Packaged font facts

REA observed manifest resource `Ptc.Controls.Core.g.en-US.resources`, token `0x28000001`, length 1,041,051, SHA-256 `d1e2fa871542343f38cab17763e03519f3bc1d00e98b35e1da6e38cd0450e34c` in the satellite above. A supplemental static .resources parser found four OTF entries. Each is a .NET stream consisting of a 4-byte little-endian byte length followed by an OpenType font; hashes below exclude that framing.

| Resource name | Font bytes | Font SHA-256 | Face |
|---|---:|---|---|
| `fonts/mathcadunimathprime.otf` | 244,812 | `16c5e7b1a4289f54cff4583c6c8cfda53dd81a512b76365cbbb3ba4763b0c266` | Mathcad UniMath Prime Regular, index 0 |
| `fonts/mathcadunimathprimeitalic.otf` | 251,040 | `2d0cac39c7afc5d2000fc2458f4261ee53a6c326c06b72fad0226d379f5337b4` | Mathcad UniMath Prime Italic, index 0 |
| `fonts/mathcadunimath.otf` | 233,816 | `26b87f237d05fb5bc2ab382d58c52b6672d50feb2371608750842f382df9ed26` | Legacy Mathcad UniMath Regular, index 0 |
| `fonts/mathcadunimathitalic.otf` | 240,692 | `f5702bc98a7437d2dfa8dea345abfc5bbd158d0601653a678022b41643749f13` | Legacy Mathcad UniMath Italic, index 0 |

The Prime pair is single-face OTTO/CFF, 1000 units per em, hhea ascent 900, descent -300, line gap 0, OS/2 weight 400, 2,102 cmap entries, and has no OpenType MATH table. There are no Prime bold or bold-italic entries in this satellite. WPF synthetic bold is a plausible inference, not a verified rasterization observation.

The bounded searches found no loose `.otf`/`.ttf` files under this installation and no Mathcad-named files in `C:/Windows/Fonts`. An optional user-supplied font-file override can avoid redistribution. An installed-DLL override would need a static PE/.resources reader; passing a DLL pathname directly to a font loader does not select this embedded face. The cross-platform default must use a separately licensed font. None of the proprietary font bytes, resources, or outlines were copied into tracked files.

## Text and template corroboration

The installed `Default Templates/en-US/Default.mctx` package has SHA-256 `50066dcccb83fd6dca81a44fb5c20e78b8b436e5768245906ca49a38478685ef`. Its `mathcad/settings/presentation.xml` part has SHA-256 `8a6decf1e29c7058ddc104ab22495056fd0c7d9dda65709a3d747a452b0d0756`. It independently records all six math label styles above at 14.666666666666668, and a base math formatting size of 11. Its page model is Letter, portrait, standard grid and margins. The blank worksheet template carries the identical presentation part.

The Korean default package SHA-256 is `e78990d3d3fd473e0e79df545e78e396a9c64b9d449c132be75c8ea1d486d48f`; its presentation part SHA-256 is `1b358577ac4f5f8ee31f35cc1c69fd43d8e4a3986896b03c35384c181d41a3e5`. It uses **A4** and proofing language `ko-kr`, while keeping the math styles above.

Both templates list Tahoma, size **11**, regular upright black for area/header/footer text styles. These named layout styles do not prove the default body text-region style. `TextFormatSerializer` maps `font-size` directly to the text format FontSize property, and `TextFormat` owns the WPF TextElement FontSize property.

Independently, `Ptc.Core.TextFormat.SetDefaults`, token `0x0600102e`, explicitly initializes **Tahoma, FontSizeInPoints = 11**, regular upright, no text decorations, transparent background, black foreground, left alignment, and zero text indent. Its `set_FontSizeInPoints`, token `0x0600104f`, calls `PointsToDips` before setting FontSize. The base text-format fallback is therefore also **14.666666666666668 DIPs**, established by its own initialization rather than inferred from the math size. The template layout style's direct `font-size="11"` and this 11-point fallback are different unit paths. A particular region may inherit or override its base format; this pass did not finish tracing every text style inheritance and text-region initialization path.

## Geometry defaults and metric dependence

These are CIL/DependencyProperty defaults. Styles or user settings can override DependencyProperty values; this pass does not establish that no BAML resource changes them.

| Source | Observed geometry |
|---|---|
| EquationControl `VisualFractionOperatorNode..cctor` `0x060006fb` | Side overhang **2.4 DIPs** per side; vertical bar padding **2.4 DIPs fallback**, superseded by the bound setting below |
| `VisualFractionOperatorNode.MeasureStaticLayout` `0x060006f6` | Width is max child width + twice overhang. Text baseline is numerator desired height + pad - font baseline adjustment. Children retain their own math formats. |
| `VisualFractionOperatorNode.DenominatorY` `0x060006f8` | Denominator starts after numerator height + actual measured fraction-bar height + twice vertical pad. |
| `VisualNthRootOperatorNode..cctor` `0x06000b6a` | Expression top pad **2**, horizontal pad **1**, root symbol x adjustment **3**, y adjustment **0.45**, index expression bottom pad **1** |
| `ExtendedFont.CalculateTypeface` `0x06000286` | Baseline adjustment is derived from the **bottom of the hyphen glyph's ink bounds**, rather than a fixed fraction of the em size. Caches M/n/j glyph runs. |
| EquationControl `Fence.CalculateGlyphGap` `0x06000490` | Fence gap derives from glyph alignment box versus ink bounding box. Fallback glyph face is the embedded regular Prime font. |

The fraction constructor `0x060006ee` binds vertical pad to `MathcadSettings.Instance.FractionBarVerticalMargin`. Its getter `0x06000285` delegates to `EquationProperties.Settings.Default`, whose getter `0x06000438` reads the named setting. Supplemental metadata inspection confirms `System.Configuration.DefaultSettingValueAttribute("2")` on Property row 227 (`0x170000e3`), with serialized attribute bytes `010001320000`. Consequently the shipped setting default is **2 DIPs**, overriding the 2.4-DIP DependencyProperty fallback. The original fraction bar is `HorizontalFence` initialized with **U+2015 HORIZONTAL BAR**, measured through its glyph metrics. It is not a fixed-thickness painter line in the original.

Compiled setting attributes also establish caret RGB **0,128,255** (Property row 219), scope **DarkGray**, opacity **0.25** (rows 221/222), typeover **Red** (row 232), and read-only caret **Gray** (row 240). These are user-scoped defaults, not the currently observed preferences. The shipped `Ptc.Controls.EquationControl.dll.config` independently records these values; its SHA-256 is `9fb53effef3e2cdb5626fe347247b44d3060d4d60f1ad003198deab5558fadac`. That config names the older `Ptc.Controls.Properties.Settings` section, so the compiled attributes are stronger evidence of the current `EquationProperties.Settings` defaults than assuming that this config section is loaded.

Exact fraction rule thickness, final fence assembly/stretch behavior, operator-specific spacing overrides, and a substitute font's optical compensation remain incomplete. Avoid describing guessed em ratios as recovered Prime constants.

## Method provenance

The following offsets are file offsets of method headers; hashes cover only raw CIL bytes. Assembly identity above is necessary because tokens are build-local.

| Assembly | Token | Header offset | Raw CIL SHA-256 |
|---|---|---:|---|
| Controls.Core | `0x06000267` | 22844 | `5b2316469a2a062668e1326013fa0fbe705591710b13582c4379e3216176ea38` |
| Controls.Core | `0x06000265` | 22692 | `1c1521e82e398690ac7a7eee43876275faa7cf9f3e89b51899ba0c44a090b771` |
| Controls.Core | `0x0600025e` | 22048 | `a736c5a99e1ab32d3d8874191180168bb9121791257b9420d548457a47e36215` |
| Controls.Core | `0x0600025f` | 22112 | `bb782f9f27018fd3d543a0518f166f0c70bec399cd1a724b48ba15c2cd95997c` |
| Controls.Core | `0x06000286` | 24344 | `f16a5203985588a2e5168aff54e10f9a86f1c677c4eadd19d3a62ecb5074eaa6` |
| Core | `0x060005f7` | 65055 | `62804dfcad55763d7812bf320a7205e9c26c7f4c134d27d7bcb8f98816a61658` |
| Core | `0x0600102e` | 128476 | `fbf12674d3b5e7a23a38d673d153a451e3feec2c31cc118b49009d1bf2be2049` |
| Core | `0x0600104f` | 132735 | `8ed8f97662048813c2f565ec9cb6a0bfbc7472d0f273246f2b1d096920f96a23` |
| EquationControl | `0x06000746` | 104688 | `07090bd58828fc434553d6ff82489b2d32bf55baf25023ec81de1e743c01bd27` |
| EquationControl | `0x0600090e` | 132396 | `f466bf24b64f0443050a66476a5f9017f5dce0c6537527dd11a861644ba5138d` |
| EquationControl | `0x06000c96` | 190792 | `1102117f0dcbbe72bf15fd35661aae1ccc46d56a446b235a99686051e514b85a` |
| EquationControl | `0x060006fb` | 100908 | `f72fcea35f4b20186c2ad4447e7c2c0c028f3e0c7e97fdf6554908b3bfed4579` |
| EquationControl | `0x06000b6a` | 172204 | `339a0e1b99bcfd37aa6e1df635e5c086b8a7d04a59b8e01a4f27e63a652377f5` |

For implementation, the independently observed constants above are suitable inputs to the renderer. Substituting an open font and implementing own layout are reconstruction choices; source coordinates and constants do not establish identical glyph appearance or final visual fidelity.
