# Multi-trace XY plot model and rendering

This pass extends Mathcraft's editable XY plot from one trace to a bounded
multi-trace model with axis labels, explicit view ranges, trace labels, colors,
line styles and markers. The reconstruction uses static Prime 12 artifacts and
shipped help only. Mathcad was not launched, loaded through CLR/COM, or
inspected through screenshots.

## Evidence ledger

| Evidence ID | Type and boundary | Observation |
| --- | --- | --- |
| `XSD-WS50-21bc9c62` | Installed `worksheet50.xsd`, SHA-256 `21bc9c62da5d026732dc4c0735583b29cb3ca2e4d2b19518e1fb197ee3dee543` | `plotBaseType` owns an unbounded sequence of `trace2DType`; `xyPlotType` has one x axis and at most two y axes. A trace contains a style, optional label, optional point count, and result reference. Each axis can contain a title, an unbounded list of plot equations, and optional `start`/`end` values. |
| `HELP-TRACES-baf556ff` | Shipped `about_traces_and_axis_expressions.html`, SHA-256 `baf556ff6816bf5c774f90891fb8f98e830d2bf121684ee0ee2ba56630795ce3` | Additional axis expressions create traces. Unequal x/y expression lists pair sequentially, then reuse the last expression on the shorter axis. Default XY appearance is blue, no symbol, solid, and thin. |
| `HELP-ADD-TRACE-56e2c06d` | Shipped `to_add_or_delete_a_trace.html`, SHA-256 `56e2c06dc32991e95d024498c5e2473f8b2ee3b3089c2baff3349a3db1f1c71ac` | A new y expression initially shares the existing x expression; adding an x expression changes the pairing. Deleting the defining axis expressions deletes a trace after evaluation. |
| `HELP-TRACE-STYLE-09e70c87` | Shipped `to_customize_a_trace.html`, SHA-256 `09e70c87a1f0725640ffb2e07dc9d004d0372e9796632a37dea724b0a2bbad08` | The author can choose symbol, line style, color, and thickness. Line choices include none, solid, dotted, dashed, and alternating dash/dot forms. If both line and symbol are set to none, Prime retains the last visible setting. |
| `HELP-PLOT-LABELS-2cd6af35` | Shipped `plot_lables.html`, SHA-256 `2cd6af358489afcab0a0919dca7dac61f204be05e27017fe6b30fd462d951ff2` | XY plots support plot and per-axis titles. Toggling a removed title back restores its previous content. |
| `HELP-PLOT-POINTS-cc1dcc36` | Shipped `to_change_the_number_of_plotted_points.html`, SHA-256 `cc1dcc36152ed5398485116a20b6444e37cad95a28d9839024f0f1db79c5fa1a` | Function-plot point counts can be selected from 100 through 3000; the native XY data ceiling is much larger. |
| REA `ev_9a5e1c81c479277743b2aa9bb2f738adb35a69ec0b4998f2eec451e46bc48d2e` | Static managed metadata from shipped `MCDXSerializer.dll`, SHA-256 `9dc0ccf194318bfd1de0ebe244150c6fa7ffbe39759960fc17f7531870708f55`, MVID `d25c4052-0a68-41e0-aee7-611908895c9e` | The inspected serializer surface establishes package-level serialization identities. It did not expose a plot-specific equation-to-trace binding contract. |

The schema further closes several formatting vocabularies. Line style is one of
`None`, `Solid`, `Dot`, `Dash`, `DashDot`, and `DashDotDot`. Symbols cover x,
plus, circle, rectangle, rhombus, point, star, triangle, and filled variants.
Line and symbol weight is a double from 0.25 through 9. Symbol frequency is an
unsigned integer and affects markers only; all available points still feed the
line. Colors are RGB or ARGB hex values. These are direct schema observations.

## Independent model

`PlotSpec::traces` stores normalized, explicit x/y expression pairs. Each pair
has a label and `PlotTraceStyle`. This differs internally from Prime's two
independent expression lists, but represents their evaluated pairing without
depending on native cache or result-reference layout. The authoring layer can
reproduce Prime's sequential/reuse rule while creating these pairs.

The old `x_expression` and `y_expression` fields remain the sole source when
`traces` is empty. This preserves existing version-2 `.mathcraft` documents.
`promote_legacy_trace` materializes that pair once; `add_trace` and
`remove_trace` are atomic helpers for an editor. The first trace remains
accessible through `primary_x_expression_mut` and
`primary_y_expression_mut`, so the existing in-sheet fields keep working after
promotion. `resolved_traces` is the read API for evaluation and rendering.

Mathcraft permits at most 16 traces and 2000 points per trace. Both values are
project resource limits rather than recovered Prime limits. Every trace uses
the same structural worksheet evaluator and geometric scope as the original
single trace implementation. An error identifies its one-based trace number.
The original `PlotData.points` field mirrors trace 1 for source compatibility;
new code consumes `PlotData.traces`.

Axis labels are plain strings. Optional finite `min < max` ranges set the
initial and reset viewport in `egui_plot`; authors can still pan and zoom. They
do not change an expression's sampling range. Trace labels enable the legend.
RGBA color, six line choices, line width, eight marker shapes, fill for shapes
supported by the renderer, marker size, and marker frequency persist in the
document. A line style of none is allowed when a marker remains visible, and a
marker of none is allowed when a line remains visible.

Prime's behavior for line and symbol both set to none depends on the previously
visible setting. Mathcraft does not retain that history, so validation rejects
that combination. `egui_plot` has no alternating dash/dot primitive;
`DashDot` and `DashDotDot` therefore use distinguishable dashed/dotted
approximations. Filled stars also use the renderer's asterisk shape. These are
explicit presentation limits, not parity claims.

## Native MCDX serialization boundary

The XSD establishes the legal containers and attribute vocabularies, but it
does not establish how axis `plotEquation` entries bind to the unbounded trace
list, which `resultRef` and cached calculation records must accompany them, or
how the optional pair of `math` children represents scaling. The schema itself
contains an unresolved design question about the maximum number of equations
per axis. No installed sample or user worksheet inspected in this pass
contained a native `<plot>` region that could establish those links.

Consequently, a Mathcraft-authored `CanvasContent::Plot` still fails MCDX
export before replacing or creating the destination. Emitting a subtree that
merely passes XSD validation would guess a semantic contract and is not treated
as compatibility. A plot already imported as `CanvasContent::Opaque` remains
source-owned and its raw region XML is preserved by the existing archive edit
path. Native plot import/export should be added after at least one authored
Prime 12 sample provides the equation, trace, result-reference, and cache
relationships.

## Verification

Canvas tests cover old-document defaults, multi-trace save/reopen, filled
marker state, labels, ranges, style limits, atomic add/remove behavior, and the
at-least-one-trace invariant. App tests cover independent evaluation of two
function traces, first-trace compatibility, label/style retention, explicit
axis state, and a headless rendering pass. Existing export tests retain the
fail-before-write contract for authored plots. These tests exercise Mathcraft's
Rust implementation only; they do not establish Prime rendering parity or
native plot-package compatibility.
