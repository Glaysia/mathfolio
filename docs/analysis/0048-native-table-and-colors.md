# Native table and color presentation

2026-10-16. Analysis and verification use inert MCDX/XML data, static REA CIL
metadata and headless egui tests. No original application runtime, GUI or screen
capture is used. The shipped target fonts and other product resources are not
copied into the implementation.

## Original facts and the exact supplied worksheet

The exact source is `Y:/26_q4/CKT/1015/1015_LCC_S.mcdx`, SHA-256
`217994a631de58e096a3cfdb8a8251aca91501e62151f869950b99f738fc18b5`.
An identical local copy is retained under the ignored selected-source directory;
neither original worksheet was modified.

The missing reference table in the supplied PDF is **region 26, `spec-table`**,
not a FlowDocument Table, bitmap or ordinary result matrix. The spelling in the
file and installed worksheet50 schema has a hyphen. The region contains six
`math` definitions, resultRef 18..23. Each defines a 10 x 1 literal matrix,
optionally scaled by its header unit:

| Variable | Unit | Source entries |
| --- | --- | --- |
| I with literal subscript dc | A | 10 numbers |
| I with literal subscript rms | A | 10 numbers |
| A with literal subscript mm2 | mm squared | 10 numbers, including `1.00` |
| D with literal subscript mm | mm | 10 numbers |
| Delta T with literal subscript fea_K | K | 8 numbers and 2 placeholders |
| awg | A | 7 numbers and 3 placeholders |

Thus the body contains **55 literal values and 5 empty cells**. The corresponding
saved resultData nodes are Synchronized but contain no result payload. Displaying
the original input cells therefore requires no calculation, cache synthesis or
hardcoded replacement data. The original bounds are approximately 292.0933 x
317.0432 DIP.

The three initially known source worksheets contain only math/text regions and
no table: 1kW has 50/16, 0818 has 56/16, detune has 66/21. Their 53 packaged text
documents were useful color evidence but could not establish the exact missing
table structure. The subsequent exact source resolves that earlier uncertainty.

Color storage is separate from calculation:

- Worksheet `text/FlowDocument` carries region-wide Background, Foreground,
  FontFamily, FontSize, FontWeight and related text properties. The title uses
  purple `#FF662D91`, ordinary annotations turquoise `#FF31ADC2`, and their
  foreground white.
- `math/formattingOverride` carries the yellow `BackgroundColor="#FFFFF200"`.
- `text/@item-idref` resolves through the owning worksheet part's relationship
  file to a nested XamlPackage. `Xaml/Document.xaml` contains Section, Paragraph
  and Run data, including run-specific background/style properties.
- Text XAML FontSize is already DIP. Math formattingOverride FontSize is points.

Static REA reconfirmed `Ptc.Controls.Core.dll` SHA
`654a0252f699c9c9763aa4f4102488279500ed89cb8ce285a38476c2dd24a977`, MVID
`b5e8e4fd-c134-4713-b1b1-eea8cddcdd9f`. `McdxSerializer.PackFlowDocument`
(`0x06000b37`, CIL SHA
`8386c7ad88459f98caff66e358aa60869a3c8d1c74c68505a71cf8ca71ecfd71`)
calls GetTextRangeFromStartToEnd, SaveTextRangeAsXamlPackage and package relationship
creation. TextFormatSerializer.CreateChildren reads foreground/background.

For specification tables, REA inspected `Ptc.EquationEditor.dll` SHA
`d419032c0e99a62eab2f278e571a338d8c4b9662655af8ab462d482a028c0223`.
SpecTableColumn exposes header/data cells and equation XML. SpecTableModel's
`IsExtraDataCellOrEmptyScale` (`0x06000aae`, CIL SHA
`ebcbc597e7df71e2a3db8d87e63b389b63599d5c269226435d004c9cd6f42ef0`)
checks equation roots and IsPlaceholder; CreateMatrixFromTableCells clones
selected source math nodes. These static calls support treating table cells as
source math, rather than fetching a fabricated scalar calculation result. They
do not establish exact pixel geometry or complete table interaction semantics.

## Independent implementation

`mathcraft-document::native_visual` reads immutable source presentation once.
`NativeVisuals.region` maps source IDs to bounded parsed content or an explicit
error. Changed plain text is compared with its original importer-compatible
paragraph text; a stale native original is not painted over the user's edits.
Canvas document serialization fields are unchanged by this presentation cache.

The parser supports styled text paragraphs/runs, basic FlowDocument tables with
row/column spans, declared columns, padding and borders, and literal
column-oriented `spec-table` data. It retains the original numeric spelling,
including trailing zeros, and renders placeholder data cells as empty cells.
Specification headers and unit expressions remain structured portable math
trees; mm squared is not flattened into a plain `mm2` string.

`native_presentation` provides the same readonly measure/draw path for canvas and
print. Math headers use the existing readonly formula renderer. Numeric body
cells use independent mathematical fonts; original installed OS text fonts are
used when available, with independent configured fallbacks. Color brushes,
underlines and supported weight/italic properties are retained. A table's source
height supplies its minimum row geometry; the independent typesetter may need
additional space for substituted font metrics.

Layout caches are stored behind Arc: an idle-frame lookup does not deep-clone
the complete table/text model. Glyph geometry is rebuilt when the source visual,
width, font atlas or scale changes. Readonly output consumes no keyboard input or
editor selection, and retains no caret or blue editing placeholders.

Missing relationships, external targets, unsupported embedded objects, malformed
XML, table bounds/span errors and source limits produce explicit failures.
XAML is parsed as inert data; WPF objects, arbitrary attributes and callbacks are
never instantiated. A generic text summary is not evidence that an unsupported
object was restored.

## Verification and remaining differences

Retained evidence and logs are under
`artifacts/rea/native-visuals-20261016/`. They include native inventories, the exact
table source subtree, REA identities/methods/call edges and focused headless tests.
Synthetic tests exercise relationship lookup, source preservation, rich style
and table parsing, spans, invalid objects, text edit staleness, source trailing
zeros, unit-power math rendering, measurement and color drawing. A selected-source
probe asserts the exact 1015 table's 6 columns, 12 display rows, 55 literal body
values, 5 empty cells and 12 math header cells without executing the worksheet.

Focused verification completed: `cargo check -p mathcraft-app --release --locked`
passed. The `native_` filter passed 20 application and 10 document tests, with
three explicitly ignored file probes. The selected-package probe then passed on
all four actual sources: 66/72/87/107 region visuals respectively, no restoration
errors; the exact source restored 27 text documents plus the specification table
and its 72 display cells. It independently asserted 55 nonempty values, five
empty body cells and all 12 structured header/unit cells. Logs:
`native-tests.log`, `actual-native-visuals.log`, `app-check.log` in the evidence
directory. These checks do not execute any worksheet.

The first integrated vector PDF exposed a separate print clip: the last table
column lay inside the physical sheet but beyond the standard body margin. The
original PDF prints that column. The table's parsed width and resulting drawing
extent already matched the source (right edge approximately 800.17 points), so
compressing table columns would have concealed the print bug. The integration
work corrected the right clip instead. Final PDF content/count checks belong to
that integration result and are not inferred from these parser tests.

Source hashes at the agent validation cutoff:

- `native_visual.rs`: `c52283eedb0c02508027fc003d50cc1b9c7db1b03c02e704badb517e551a1a2a`
- `native_presentation.rs`: `1763b697a72b433065011d3398485c4265685ea9429e088bf5e77c2c9dc282a4`

### Native alignment anchor follow-up

The first integrated PDF comparison exposed a separate vertical placement error:
native `top` is an alignment anchor, not the visual bounding-box top. Static REA
confirms the general placement contract: `WhiteboardViewModel.ArrangeOverride`
(`Ptc.Controls.Whiteboard.dll`, method `0x0600022d`, IL SHA
`0852dbfc4d6a47d230980134d6baca9ecea6a3833dcec2b34c4a2bf56ec6ec4d`)
adds each item's `VerticalAlignmentOffset` to its saved Y before arranging it.
`EquationControl.get_VerticalAlignmentOffset` (`0x060000b2`) returns the negative
of its transformed text baseline. These facts explain why copying saved `top`
directly into a painted top-left shifts both text and math downward.

`SpecTableView.get_VerticalAlignmentOffset` (`Ptc.Controls.EquationControl.dll`,
`0x06000371`, IL SHA
`fca21f628d8986bc99c2fdd1e4d99245677c93ea2eeaf2ae22ef2fbbbe8d08f5`)
returns `-(b + (t-b)/2)`, where `b` is the maximum bottom of layout row 1 and
`t` is the minimum top of layout row 2 across all columns. The native table
anchor is therefore between its two header rows and its first data row. Empty
or incomplete arrangements return zero. This is a measured geometry contract;
it does not justify hardcoding the approximately 40-point difference seen in
the supplied PDF.

A further static inspection refines this contract: the table manager's stored
cell arrangements are content rectangles, not full row-border rectangles.
`CalculateRowAndColumnMetrics` (`0x060006ed`, IL SHA
`0daff74a1b514f4f40cc91bd38a4c1d98cb51f4904187ac2026035f01fd24b6b`)
sets each row baseline to the maximum cell baseline, descent to the maximum
`cell height - cell baseline`, and height to their sum. `Measure`
(`0x060006eb`, IL SHA
`7ea01c6dc9101cc08d1b7ab509699895bac80d0304e6b6484c1698bacb8ad752`)
places each content rectangle at `rowY + rowBaseline - cellBaseline` with its
actual content size. `VisualTableContents.MeasureOverride` (`0x060007e6`)
obtains that baseline from `IBaselinedVisual.TextBaselineY`, or zero for a child
without the interface. Thus all mathematical cells in a row align baselines.
The first independent table renderer used row boxes for this gap. The final
integration applies per-row baseline/descent maxima, offsets each specification
cell by its baseline difference, and computes the anchor from the actual content
rectangles. A regression mixes four different equation font sizes and verifies
both shared row baselines and the content-based anchor. Font metrics and minimum
cell sizing remain independent approximations; this correction is not a claim
of identical WPF table geometry.

For native rich text, `Ptc.Controls.Text.dll` SHA
`6d3b7c7a01aaca4d04c5e6771064105df145c13605b7c27a013e52a29c53b2e1`,
`RichTextBase.CalculateBaseline` (`0x060000ff`) uses the initial text element's
font baseline ratio multiplied by its size, with document defaults as fallback.
`get_VerticalAlignmentOffset` (`0x06000100`) negates the transformed
`Baseline + Padding.Top` position of `PART_ContentHost`. REA retained parent:
`ev_ab141bcd07fea9246633fbc7e78aa7727422ab8229e43b35dd475704672626ce`.
Supplemental static instruction decoding and original REA method identities are
retained in `anchor-contract-cil.json`; no product assembly was loaded or run.

The independent renderer now exposes cached `alignment_offset`: rich text uses
the first laid-out glyph baseline, including paragraph margin; specification
tables use the measured row-1-bottom / row-2-top midpoint. The cache remains
behind `Arc`. Two headless regressions verify these different anchors, including
nonuniform header heights and a nonzero row gap. The application `native_`
filter passed 22 tests with two intentionally ignored file probes. This narrow
follow-up changes `native_presentation.rs` after the earlier source hash cutoff;
the caller's native coordinate conversion and final PDF comparisons are
separate integration checks. Exact WPF font metrics and template padding are
not established as identical by these independent tests.

This work restores readonly presentation for supported native content. It does
not implement spreadsheet-style table editing, table column bindings in the
calculation engine, all WPF rich document objects, arbitrary fonts/brushes,
bidirectional text, or every original table layout/printing metric. Source parts
remain authoritative and unsupported data must remain preserved. Integrated PDF
vector export, overall responsiveness and unit rendering changes are documented
separately by their owning implementation work.

### Final application integration

The caller now applies native alignment to both canvas and print without changing
saved coordinates. The exact worksheets on Windows and peets restore 107 regions,
64 results and the complete six-column table without recalculation. Independent
PDF checks recover all 55 table literals (including `1.00`), the five blank input
cells and the final awg column, with purple/turquoise/yellow fills and white text.
The right clipping correction and baseline evidence are in [0052](0052-native-alignment-and-print-integration.md).
The final source hashes and 430-test totals supersede the earlier narrow cutoff
hashes; see [CONFORMANCE](../CONFORMANCE.md).

A separate headless test module in an ignored copy of the validated source
opened the exact 1015 worksheet, saved/reopened `.mathcraft`, exported/reopened
MCDX, and compared every supported region's rich document and background colors.
Both reopen paths preserved 107 regions, 28 rich text/table documents and all
14 explicit math backgrounds (including the ten yellow highlights), with no
calculations. All 27 native XamlPackage parts were byte-identical after MCDX
export; its worksheet/header/footer/presentation parts passed installed XSDs.
The product source was unchanged by this separate probe. It does not establish
that editing plain text preserves arbitrary character-level rich formatting.
