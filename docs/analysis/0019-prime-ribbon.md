# Prime 12 ribbon reconstruction

The user's reference is Mathcad Prime, including its command organization and writing workflow. This pass replaces Mathcraft's generic menu plus formula toolbar with the original ribbon hierarchy. It does not establish visual or behavioral parity with Prime.

## Evidence and method

No GUI, image inspection, CLR loading, COM, or original application execution was used. REA inspected the installed managed artifacts. An inert decoder then read the .NET ResourceReader v2 directory and BAML records directly from bytes. It decoded element starts/ends, property literals, type and attribute tables, string references and extension references. Negative standard WPF property IDs remain numeric in the raw output; this pass uses their documented WPF identities only where established. End-of-document records were reached for every selected BAML stream. This does not decode all converters, triggers or runtime skin behavior.

| Source | Identity |
| --- | --- |
| `MathcadPrime.dll` | SHA-256 `68c0d708abf70dd687e7996de6f6d7105f29a1ffd7e47fd719e161076fcc140f`; MVID `b31b3d5c-d238-48c2-955f-41c63882886d` |
| REA main metadata | `ev_4e9c3ecc8793b8a16b1acc38c88749031956246dcc2feb8dd3e9b229d3de8643` |
| REA selected ribbon members | `ev_c4ab8d3972ee82d475dd01a06b6e3da004286b9a27f7bc12db0e1cbc176a91c9` |
| `ko/MathcadPrime.resources.dll` | SHA-256 `a511bb8b8c39391d107cbd3f5409e08ab5f258c42588dbd06f5d9acfd9ea457f` |
| REA satellite resources | `ev_2e64a3c5431a2fbcaea4d8a711761138916f0159a5ad240aeece5c0e7e48562a` |
| `MathcadPrime.g.ko.resources` | SHA-256 `e6c60b0177cf40547eb84391a34ac4ea1d18027fb51f9f4a4994d7e638da25bf`; 711 ResourceReader entries |

The ignored `artifacts/rea/ribbon/manifest.json`, `resource-evidence.json`, `main-focused.json` and `baml-selected.json` retain byte identities, metadata and decoded record locations. `decode.mjs` is the inert supplemental decoder. Proprietary resource payloads, images, fonts and icons are not product assets and remain outside Git.

## Recovered structure

`ribbonresources.baml` SHA-256 `99e160a0beefd6a6f5c6748a004d82ba94534178516f57d9e335f5cb5cc67d25` contains these tabs in this order. The element tree corroborates the resource names; these are not inferred from filenames alone.

| Tab | Declared child ribbon groups |
| --- | --- |
| 수학 | Regions; OperatorsAndSymbols; MathStyle; MathUnitsSettings; MathClipboard |
| 입력/출력 | MathcadWorksheets; DataMatrixFileAccess; Objects; InputControls; MCAD |
| 함수 | MathFunctions |
| 행렬/표 | DataMatrixDataAndMatrices; DataMatrixRowsAndColumns; MatricesAndTablesFormat |
| 도표 | PlotCreation; PlotLabels; PlotTraceStyles; PlotAxes; NumberFormat |
| 계산 서식 지정 | MathFormatWithDebugTab content |
| 텍스트 서식 지정 | FontFormat; TextAndRegionFormat; TextStyles; Links; Spelling |
| 계산 | CalculationControl; CalculationErrorTracing; CalculationWarning; CalculationOptions |
| 문서 | DocumentRegions; DocumentInsert; Page; DocumentView; HeadersAndFooters; RegionBorder; MathClipboard |
| 리소스 | Help; AskCommunity; TechnicalSupport; Learning |

The BAML positions for the first math groups are 7916, 7921, 7926, 7931 and 7936. Tab headers start at 7876, 7974, 8077, 8154, 8240, 8486, 8556, 8671, 8842 and 8953. Each normal `RibbonBarPanel` explicitly declares Height 85, for example offsets 7904, 8009 and 8105. That is the source of Mathcraft's 85 point content band; it is not a screenshot measurement. WPF uses device independent units.

`mathregionsribbonbar.baml` SHA-256 `6b1a3f9b0cc6a6f6f484a77561727e6fbdbbfc8689aac95185325c98f4028aa0` declares the group header 영역. Math, solve block and chart component use large 32 unit image slots with `ImagePosition=Top`; text block, text box and image use the smaller vertical command stack. The command targets distinguish `NewParagraph` from `NewTextRegion`: a full-width text block pushes regions down, while a text box is placed on the sheet. Mathcraft therefore keeps the unimplemented block command disabled rather than giving both commands the same behavior.

`mathoperatorsandsymbolsribbonbar.baml` declares 연산자 및 기호, with operator, symbol, programming, constant and symbolic galleries. The ordinary fraction/root/etc. operators belong inside the gallery; they do not constitute the main application toolbar. `mathstyleribbonbar.baml` declares 스타일 and exposes labels, literal subscript and equation line breaking. `mathunitssettingsribbonbar.baml` declares 단위, a unit gallery, and a separate worksheet unit-system dialog. A unit picker does not imply that changing the system of units has been implemented.

`mathclipboardribbonbar.baml` SHA-256 `e035668b524cfab7dfd9ca0a165627554b85c36effa6b680e8892e819f53d47e` declares Cut, Copy and Paste. `qatcontrol.baml` SHA-256 `4fedcce83f5954cce7b54cb6f8459be9c147b2486e8ce1a70d63d6e22ad22183` declares the compact New, Open, Save, Undo, Redo quick-access commands.

`calculationcontrolribbontab.baml` SHA-256 `d81ab2c173adc3455f5f354a8cc9ce277ad8a2e55f179835a787ef1cad68301f` separates automatic calculation, stopping calculations, full worksheet recalculation (`Ctrl+F5`), current calculation (`F5`) and disabling a region. Mathcraft currently recalculates synchronously and does not reproduce that scheduling distinction; its two available calculation buttons both request the existing evaluator.

The shell merges `DevComponents.WpfRibbon.themes.GraySkin` and dock `Office2007SilverSkin`. The ribbon tab templates include rounded top corners, a highlighted border, and normal/selected gradient states. For example, `ribbonstyles/ribbontab.baml` records `#FF8AB1C5` at offset 3534 and `#FF4E7E9B` at 4673; `ribbonstyles/ribbonbackgroundchrome.baml` has `#e6eff5` and `#f3fbff` at 3089 and 3062. These literals do not alone resolve the final runtime palette because other resources and triggers participate.

## Independent implementation and boundaries

`crates/mathcraft-app/src/ribbon.rs` reconstructs the quick-access row, tab row and grouped content, using typed actions for the existing editor and file operations. The math tab preserves the five group order and large-versus-stacked command arrangement. The operator, Greek symbol, constant, unit and basic function galleries dispatch editable source insertions. Matrix, index, range, function, plot, selection, clipboard, zoom and grid operations use the existing application model. Unavailable operations are disabled and say so on hover.

The drawing is independent Rust/egui code. All icons are drawn with geometric primitives and text. The 85 unit content height and 32/16 icon slot convention come from static properties. Group widths, quick-access height, tab height, simplified strokes, selected fills, spacing and some menu organization are implementation choices to fit egui. The original icon artwork, skin renderer, responsive group collapse, accelerator key-tip system, title-bar integration, rich formatting, complete galleries and contextual enable predicates are not reproduced. There has been no visual comparison or original-runtime validation in this pass.

The remaining work is to reduce the gap in authoring behavior and context-sensitive state, then expand supported commands within this recovered hierarchy. Adding enabled controls without corresponding document operations would make the reconstruction less faithful.
