# Supplied Mathcad / Mathcraft PDF comparison

Date: 2026-10-16. Both complete pages of each supplied file were inspected after
headless PDF rendering. No application UI or runtime was opened. PDF contents
were treated as evidence, not instructions. This is an output comparison, not a
claim that the imported worksheet was recalculated successfully.

## Inputs and retained evidence

| File | SHA-256 | Bytes |
| --- | --- | ---: |
| `C:/Users/harry/Downloads/1015_LCC_S_mathcraft.pdf` | `52321f630a62df694a9e2abd673eb0f35168955fd0f59ddbf3146bd7f3156329` | 1,892,095 |
| `C:/Users/harry/Downloads/1015_LCC_S_mathcad.pdf` | `e0e340254feed28722df827393a0ba2524beedc45847b3dcb2d43002b499ec68` | 580,073 |

Ignored local evidence is in `artifacts/rea/pdf-comparison-20261016/`:
`metadata.json`, extracted native text, full-page PNG renders, and enlarged
radical/detail renders. The Mathcraft PDF has no extractable text, so its visible
values below were read from the rendered pages; OCR was not used.

## PDF representation

Both files have two portrait A3 pages, approximately 841.89 x 1190.55 points.
There is no observed page-count or paper-size disagreement in this sample.

| Property | Mathcad | Mathcraft |
| --- | --- | --- |
| Content | Selectable/searchable text and vector drawing | One full-page RGB image per page |
| Extracted text | 1,826 characters on p1; 1,185 on p2 | Zero on both pages |
| Drawing paths | 214 on p1; 156 on p2 | Zero |
| Raster images | Zero | 3508 x 4961 pixels per page, approximately 300 dpi |
| Embedded fonts | Tahoma; Malgun Gothic Bold; Mathcad UniMath Prime regular/italic | None |
| File size ratio | 1 | 3.26 |

The raster representation explains this sample's larger PDF and loss of text
selection/search and resolution-independent lettering. It does not, by itself,
prove the cause of interactive typing latency. The latter must be traced in the
application's edit/commit/calculation path.

## Missing content and styles

- **Page 1, upper right:** the complete current/wire-size reference table is
  absent from Mathcraft. The table headed by `I_dc`, `I_rms`, `A_mm2`, `D_mm`,
  temperature rise and `awg` is replaced by the literal summary
  `가져온 영역 · 읽기 전용`. This is content loss, not a font difference.
- **Page 1:** the purple `LCCL-S 시스템 설계` banner, yellow parameter/result
  highlights, and turquoise annotation backgrounds are absent. Text is present
  but displayed as plain black labels without the original colored rectangles
  and foreground styles.
- **Page 2:** turquoise section/annotation backgrounds are likewise absent,
  including `Lm+Ladd 튜닝`, the two operating-mode sections, and the theoretical
  and measured efficiency sections.
- Both pages retain roughly the original columns and grid, but text baselines,
  formula heights, line gaps, weight and unit typography differ. Matching page
  coordinates alone therefore does not reproduce the original worksheet.
- Empty radical-index editing placeholders appear as small blue rectangular
  marks in the Mathcraft PDF. They are visible around `V_rx_rms` and
  `I_rx_rms` on p1 and radicals on p2; the original output has no such marks.
  These must be suppressed in read-only/print formula rendering.

The Mathcad Express diagonal product/license watermark is absent from
Mathcraft. It is identified here only to avoid confusing a licensing overlay
with missing worksheet content; reproducing it is not an application feature.

## Result-format discrepancies

| Page / expression | Mathcad visible result | Mathcraft visible result | Interpretation |
| --- | --- | --- | --- |
| p2 theoretical efficiency `eta` | `94.359%` | `0.944` | Percentage result unit/format lost; precision changes with scaling |
| p2 correction factor `r_ma` | `97.163%` | `0.972` | Same percentage-format loss |
| p2 measured efficiency `eta` | `89.176%` | `0.892` | Same percentage-format loss |
| p2 complex input impedance `Z_in_add` | `(64.267 angle 35 degrees) ohm` | `(52.644 + 36.862i) ohm` | Polar-result format lost; rectangular value is consistent with the displayed polar result at the shown precision |

These examples demonstrate output-format incompatibility. The PDFs alone do
not establish incorrect underlying numerical values: the decimal efficiencies
are consistent with the original percentages rounded to three decimal places,
and the complex forms represent the same value to their displayed precision.

Other inspected displayed results agree at their shown precision. Examples:

- p1: `160 V`, `6.25 A`, `203.718 V`, `144.051 V`, `8.115 A`,
  `11.476 A`, `6.942 A`, `9.817 A`, `84 uH`, `18.841`, `7.848 uH`,
  `154 uH`, `17.751 ohm`, `52.644 ohm`, `1.865 kV`, `373.07 V`,
  `0.353 kV`, `3.673 kV`, `734.629 V`, `612.191 V`.
- p2: `36.862 ohm`, `16.297 uH`, `28.797 uH`, `64.267 ohm`,
  `229.444 V`, `254.848 V`, `4.358 A`, `280.099 V`, `311.112 V`,
  `40.244 W`, `14.534 W`, `0.611 ohm`, `0.302 ohm`, `79.6 W`,
  `31.774 W`.

Scientific notation also differs: Mathcad's grouped centered-dot form versus
Mathcraft's multiplication-sign form. The shown exponent/value agrees in the
inspected examples; grouping, font and result-style fidelity remain separate
work.

## Priorities supported by this evidence

1. Preserve explicit edit/commit boundaries: draft typing must not repeatedly
   rebuild/recalculate the committed worksheet.
2. Preserve percentage result units and complex polar formatting on import,
   saved presentation restoration and subsequent calculations.
3. Render imported content instead of printing a read-only summary label, and
   preserve rich text/background styles.
4. Keep print rendering free of editor placeholders and match formula metrics.
5. Replace full-page raster PDF output with selectable text/vector output when
   that can preserve the mathematical layout. The present raster PDF is a
   functional intermediate, not Prime-equivalent PDF fidelity.

The commit-boundary priority also incorporates the user's explicit report of
per-keystroke updates. It is not inferred from a static PDF.

## Subsequent correction

The source print path now uses an immutable, non-interactive formula renderer.
Its optional empty radical degree has no printed marker or reserved degree
width; explicit degree `0`/`3` and required empty operands remain visible, and
the editing renderer retains its placeholder. Measurement uses the same output
layout and label styles as painting. The three headless regressions cover these
distinctions plus input retention, unchanged source/selection and measurement
agreement. This change does not alter either supplied PDF or claim that the
other discrepancies above have been corrected.

Validation: `cargo test -p mathcraft-app --release --locked readonly_` passed all
five matching tests (including the three new regressions). Retained log:
`artifacts/rea/pdf-comparison-20261016/readonly-formula-tests.log`. Allocated
widget dimensions agree with measured layout within egui's physical-pixel
snapping tolerance.
The six existing `worksheet_ui::print_ui::` headless tests also passed; retained
log: `artifacts/rea/pdf-comparison-20261016/print-ui-tests.log`.

The subsequent [result-format implementation](0047-result-formatting-integration.md)
connects Percent and complex polar settings across the ribbon, MCDX persistence,
saved numeric data and output. The four pages compared above remain unchanged
input evidence. This later code correction is not a re-export or a claim that
the missing table, color/styles or raster PDF representation were repaired.
