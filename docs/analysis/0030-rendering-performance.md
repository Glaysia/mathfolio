# Rendering work and executable size

2026-10-15. Source inspection and headless tests; no GUI or screenshots.

The previous executable offered for testing was an unoptimized development
binary, 57,157,120 bytes. The release profile now uses thin LTO, one codegen unit,
and stripped debug information. Windows selects eframe's WGPU renderer; Linux
selects Glow rather than enabling both desktop renderer integrations everywhere.
`scripts/run.ps1` now builds/runs release. Development builds optimize project
code at level 1 and dependencies at level 2.

Three avoidable per-frame operations were identified and removed:

- A whole-document Undo snapshot was cloned even in idle frames. A snapshot is
  now taken only on frames with input that may mutate the document; Undo history
  remains one command per actual edit or movement start.
- Every region was instantiated and typeset even well below the viewport.
  Vertically offscreen inactive regions are culled. Previously measured heights
  are used for regions above the viewport; unknown tall regions are measured
  conservatively, and selected/focused editors still receive events.
- The formula widget laid out the same tree twice in frames with no mutation.
  It now reuses the initial layout unless an edit changed the tree.

The `idle_large_worksheet_only_instantiates_visible_editors` regression checks
1,000 calculated regions, only 9 instantiated visible editors, unchanged source,
no dirty flag, and no idle Undo entries. A release headless run took 14.0414 ms
for 120 warmed idle frames. This measures CPU-side egui layout and shape
construction, excludes GPU presentation, and is not a measured before/after
comparison of user-visible frame rate. Input/Undo, math geometry, navigation and
save/reopen regressions remain required alongside this check.
