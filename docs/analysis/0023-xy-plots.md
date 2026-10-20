# Editable XY plot regions

> Follow-up: [0027-multi-trace-xy-plots.md](0027-multi-trace-xy-plots.md)
> adds the bounded multi-trace, axis-label/range, and trace-style model while
> retaining the native serialization boundary documented here.

This milestone implements one editable XY trace on the worksheet. Its axis expressions use the existing Rust structural worksheet evaluator and the definitions visible at the plot's position. It is a bounded reconstruction of the original authoring workflow, not complete Prime plot compatibility.

## Static original evidence

The original application was not launched, loaded through a CLR or COM, or inspected through images. REA inspected the installed `MathcadPrime.dll`; an inert BAML decoder followed its resource reference into `ko/MathcadPrime.resources.dll`.

| Evidence | Observed fact |
| --- | --- |
| REA `ev_46613d337f8410e1795558e286d748c3088bda38e7ae65f7b92873fb7e161b91` | `Spirit.PlotCreationRibbonBar.InitializeComponent`, token `0x0600043e`, loads `plots/plotcreationribbonbar.xaml` |
| Raw CIL SHA-256 `2ca00afebad8092c8250997b4336910505abf577009ceb6d08739624f59a14fc` | Binds the above method to installed main assembly SHA-256 `68c0d708abf70dd687e7996de6f6d7105f29a1ffd7e47fd719e161076fcc140f`, MVID `b31b3d5c-d238-48c2-955f-41c63882886d` |
| BAML record at 3691, tooltip at 4093 | XY insertion dispatches `WorksheetCommands.NewPlot`; the tooltip identifies `Ctrl+2` |
| Same BAML at 2402 | The plot-creation ribbon group is named 그래프선 |
| REA satellite resource evidence `ev_2e64a3c5431a2fbcaea4d8a711761138916f0159a5ad240aeece5c0e7e48562a` | Original resource identity and static-only inspection boundary; full ribbon provenance is in analysis 0019 |

The installed help documents provide additional shipped behavior, independently of runtime rendering:

| Installed `Help/en/PTC_Mathcad_Help/` file | SHA-256 | Relevant contract |
| --- | --- | --- |
| `about_plotting_functions.html` | `8addbd5a9d09dd3829859a0d185ec000d58fca437ff699b79fe3440837927501` | Built-in/user functions and an axis expression's own function body; default domain −10 to 10 and 500 XY points; a defined range controls samples |
| `about_plotting_data.html` | `c86f7dd387c5ff4d45345e9fa61ff84f5be5672e861b7bae25d1352979eb0395` | Default line trace; matrix data use the first column; differing array lengths use their common prefix; original maximum is 2,000,000 points; NaN data are omitted |
| `about_plot_regions.html` | `1e484b018ac88a2a06b1edda0bfa091d0136bb16e7a84b1b3de72b1817324053` | Plot regions can be moved, copied, pasted, disabled and resized, within the worksheet |
| `example_using_range_variables_in_plots.html` | `389d13171f85199e803465ef1c347509a3eec2cb43ce51c3bd02b63c6b5726a1` | A range supplies the independent variable to both axis expressions, including parametric `cos(t)`/`sin(t)` examples |

Ignored evidence under `artifacts/rea/xy-plots/` retains selected method identities, decoded ribbon BAML and help text with hashes. No original artwork or application code is used as a product asset.

## Independent implementation

`CanvasContent::Plot(PlotSpec)` persists the x/y expressions and the plot's width/height in a normal `.mathcraft` canvas. The independent initial presentation is 420×260 DIP with `x` and `sin(x)`; the dimensions are project choices, not recovered Prime defaults. Axis edits and resizing are document edits. Panning and zooming the displayed axes do not change source and do not consume document Undo steps.

The plot parser consumes the complete expression through `mathcraft_core::parse`, then converts its AST to a `PortableTree` and native MathML through `editor_bridge`. It does not replay keystrokes, discard unsupported characters, run Python, or evaluate expressions through the legacy scalar environment. Arithmetic, ordinary function calls and names therefore use the same compatibility evaluator as placed math regions.

The caller supplies preceding math regions, plus any globally visible definitions. Temporary evaluation queries use fresh local IDs and the plot's position, preserving geometric scope. Their trees and results are never inserted into the saved document. Existing definitions, including function definition captures, remain unchanged.

For a bare x-axis identifier with a valid worksheet range/vector binding, the y-axis expression is evaluated point by point when it references that identifier. This lets `f(x)` or `x^2` work over a vector without changing vector powers into matrix powers or rebinding x in the worksheet. Both axes may also evaluate independent vectors/range outputs, including parametric expressions over a previously defined range. Matrix axes take their first column; unequal array axes use their common prefix. Scalar expressions can be broadcast over the other axis; this broadcasting is an independent bounded policy, not a claim of full Prime mixed-shape semantics.

An actually undefined bare independent variable gets a plot-local default domain of −10 to 10 with 500 uniformly spaced inclusive samples. The source domain and sample count are supported by the shipped help; exact original sample placement was not runtime-verified. A small in-sheet indication makes this implicit domain visible. A failed existing definition is not replaced by a default range. The plot never writes a fallback definition into the worksheet.

The in-sheet expression fields and interactive axes are implemented with egui and egui_plot. They preserve the worksheet model rather than opening a separate plot dashboard. The widget reports the focused axis field's ID so worksheet keyboard handling does not take input from it. Size changes are bounded to 120–2000 by 100–2000 DIP.

## Limits and validation

There is one trace, a 2000-point limit and a 4096-byte limit for each axis expression. Both axes must produce finite real dimensionless values. Unit-bearing values can be explicitly divided by a unit in the expression. Complex values, undefined samples and non-finite values produce an error rather than silently losing data. This differs from Prime's larger maximum and NaN-omission behavior. Unsupported parser syntax, complete native plot import/export, multiple traces, original marker/line styles, axis formatting, adaptive discontinuity handling, unit-aware axes and 3D/polar/contour plots remain outside this milestone.

Canvas region formatting is now stored in `MathFormat`: font size, General/Decimal/Scientific/Engineering result format and precision, plus an `explicit` flag. Missing formatting in older v2 documents defaults to 11 points, General, precision 3 and `explicit=false`. Font size must be finite and between 1 and 200 points; precision is at most 15. The explicit flag lets the export layer preserve imported formatting it cannot fully represent unless the author has actually changed it.

Headless tests cover default sampling and full-input rejection; prior vector plus user-function evaluation without rebinding; explicit ranges and independent vectors; first-column extraction and differing array lengths; and rejection of failed definitions, dimensions, complex results, undefined samples and excessive ranges. Canvas tests cover persistent plots/local formatting, defaults for older v2 files, and rejected size/expression/font/precision limits. These tests execute Mathcraft's Rust implementation; they do not validate the original UI or claim visual parity.
