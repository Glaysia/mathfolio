# Reopening results and viewing content beyond the right page edge

2026-10-15. These findings come from the Mathcraft source and headless regressions.
No Mathcad runtime, GUI automation or screenshots were used.

The worksheet recalculation loop previously returned before evaluating any
regions when it encountered an opaque imported region. Imported math that the
editor could not decode was also excluded, even where the calculation engine
could evaluate it (for example global definitions). The application now extracts
the namespace-preserving native expression and passes it to the same worksheet
engine. Unsupported non-math regions receive a local fault and independent
calculations continue. Unsupported definition metadata is replaced in the
calculation copy with a failed definition, retaining the declared identity so
dependent expressions cannot reuse an earlier successful binding.

The symbolic layout displayed the input and command but omitted the computed
result. It now appends the supplied result; the result parser also unwraps
`symResult` before formatting its expression. Solve-block summaries and opaque
math regions now display available computed values.

The page width previously remained fixed even when measured expressions extended
beyond it. The initial status-bar toggle was removed after the user clarified
that the expected behavior is Prime's existing Page/Draft view switch. Installed
`to_customize_the_worksheet.html` (SHA256
`76cd28a3046e5d512bdc234012168e558ab4fc2b33d5635745b02cfc731b0763`)
documents a right-side Switch to Draft View button when draft content exists,
hidden draft content that still participates in calculations, and a Draft view
without margins, headers or footers, with a dashed vertical boundary. The app
now places that switch at the page's right edge, exposes Page/Draft in the
Document ribbon, and permits editing beyond the print boundary in Draft view.
The print page dimensions and stored expression positions remain unchanged.
An invented per-result expansion button and separate result window were removed
on 2026-10-16 following the user's correction. They had no established Prime
counterpart. Computed results remain embedded in the worksheet; the Page/Draft
switch is separate and controls the area beyond the print boundary.

Regressions cover opening a saved document containing a native global definition
and unsupported component, independent results, failed redefinition invalidation,
long-formula horizontal extent, symbolic answer geometry, and symbolic wrapper formatting. These checks
prove the reproduced source-level failures; they do not establish full native-file
or original-product compatibility.

## Later source-contract correction — 2026-10-16

Installed `about_controlling_region_calculation.html` explicitly distinguishes saved results on worksheet open from explicit recalculation, and Calculate Now from Calculate Worksheet. Prime does not automatically recalculate when opening a worksheet even with Auto Calculation enabled. The fixes described above did not implement that original lifecycle. Subsequent saved-presentation restoration and bounded F5/full Ctrl+F5 calculation are recorded in [0032](0032-calculation-commands-and-saved-results.md). The consolidated inventory now records HD005/HD006/HD009 as partial, retaining the remaining cache/dependency/disabled-region limits.

Installed `to_view_large_symbolic_results.html` documents selecting a large symbolic result and dragging its right resize handle to change region width. That original behavior remains unimplemented (EA-087). Removing the unsupported modal result viewer did not implement the original width control. See [the expanded audit](0031-feature-inventory-audit.md) for the source identities.
