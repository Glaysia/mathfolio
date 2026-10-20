# Area vector printing boundary

2026-10-19. Dedicated readonly helper: `crates/mathcraft-app/src/worksheet_ui/area_print.rs`. Original Area storage/ordering/protection boundaries are in0132/0134, saved display preparation is in0135. No original GUI, screenshots, Mathcad runtime, managed assembly execution or worksheet recalculation was used.

## Original evidence

Installed `Ptc.Controls.CollapsibleArea.dll` SHA `63418ae37c2e16fc49398205f28279dc0d1b7d5301940458497a504309be5590`, MVID `ea79fc32-ec3c-4c02-8e2c-af8c0b3cd512`, REA `ev_07adf4e8941895b508576d733cda7b00f7240f33d342632a94655225838ef572`.

New raw installed-byte-verified CIL: `artifacts/rea/feedback/area-print-state-cil-20261019.txt`. Area001e `HandlePrintViewportChanged` SHA `4b4daf8de90bd0310f5efefc2cf2cc28187f9b1bf01b56f15f1553e8b38d83bd`: IL0019–0046 checks PrintViewport and IsCollapsed; a collapsed Area receives Rect.Empty for child print traversal. IL0075–008a converts an empty viewport to common IsInPrinting=false, then00cc–00e8 propagates the child state. Otherwise children are tested against the supplied viewport (0107–015d). This establishes that collapsed child content must not reach the printer. It does not establish every border resource trigger or exact stroke geometry.

Shipped-help text and exact hashes are in `area-and-definition-help-evidence-20261019.json`:

- `to_collapse_and_exapnd_an_area.html`, SHA `d7f978ee02264f02a7d78a6be39847844dd24fddf71a902f80a2fd87e716307f`: collapsed areas and collapse/expand icons are not printed.
- `hide_borders_area.html`, SHA `a4c9f7142dd1b779351085ddd43c970bdcf689a05c0e16f3cc6077945bf3281c`: shown borders print, hidden borders do not; a collapsed error Area has a red line which prints even when its border is hidden.

The bounded helper omits normal collapsed content/icon entirely and preserves the explicitly documented collapsed-error line. The interaction between the general shown-border statement and normal collapsed border resources remains unverified; normal collapsed non-error output is currently omitted following the explicit collapse-help statement. Exact original RGB, stroke width, line offsets and printing of unusual protection combinations are not claimed. The helper uses the existing app gray/error palette and0.8 stroke as an explicit reconstruction choice.

## API and retained data

`AreaPrintSnapshot::new(xml, child_views, width, default_font_points, has_calculation_error)` receives namespace-complete owning-region XML and already prepared presentation. It clones committed math trees into editors without undo histories, formatted results/labels and native rich visuals. It never parses saved results as input code, evaluates formulas, alters source XML, changes disabled flags or opens a printer dialog. `has_calculation_error` is explicit: unsupported native visual errors should not be silently reclassified as original calculation errors.

`measure(&Ui) -> Vec2` supplies owner height for PrintPlan and selected-page bounds. `paint(&mut Ui, origin, clip) -> Rect` paints ordinary egui text/path shapes, retaining the existing vector PDF backend. The caller transforms the owner to the selected page origin; the helper keeps child source-local coordinates. Math uses the existing native-baseline correction; rich text/specification tables use the existing inert native_presentation layout/alignment policy. No new Area header/icon/menu is printed.

Expanded source math requires the supported tree bridge. Expanded non-math children require a resolved native rich visual. Unknown/unsupported child presentation or protected expanded Area returns a job error rather than dropping original content or rasterizing a fake substitute. Collapsed Areas do not load/parse hidden child content, including protected or unsupported children. Oversized/nonfinite print coordinates fail explicitly.

## Root hookup instructions

No existing print central file is modified by this module.

1. Register `mod area_print;` in worksheet_ui.rs.
2. While `WorksheetApp::print_snapshot` still has access to native Area entries and prepared child views, construct one snapshot per Opaque kind Area. Store `HashMap<u64,Result<AreaPrintSnapshot,String>>` in PrintSnapshot so invalid source is reported from plan rather than losing content. Obtain child views from the same prepared live/saved/native data used by AreaUI; supply effective defaults and the actual Area/page width.
3. In PrintSnapshot::plan, check the Area result before generic Opaque handling, measure its height and store owner alignment offset0. Do not count hidden child content when planning a collapsed Area. Keep source document positions intact; collapse is not a new global document reflow operation.
4. In PrintSnapshot::paint, call the Area helper with the already transformed owner origin and existing content_clip, then continue before generic Opaque placeholder/native output. This prevents printing the generic imported-region label on top of Area content.
5. Commit Area drafts through the existing app transaction path before snapshot creation, just as normal formulas/tables are committed. Printing must consume current committed presentation and must not trigger a calculation.

The existing root-only NativeVisuals import and107-visual oracle are unchanged. New Area-only visual entries remain path-keyed and are not flattened into top-level CanvasRegions.

## Headless fixture and validation

Module tests cover local coordinate/baseline placement, retained rich text and math producing a PDF with ToUnicode fonts and no image XObjects, collapsed content/icon omission, the hidden-border red error line, and explicit unsupported/protected expanded errors. The ignored `area_vector_pdf_export_probe` writes three vector-only fixtures (`area-expanded.pdf`, `area-collapsed.pdf`, `area-collapsed-error.pdf`) under `MATHCRAFT_AREA_PRINT_OUTPUT`. It uses independent test XML and existing display data, not a copied original worksheet or runtime output.

Validation completed: helper tests3 passed/1 ignored, ignored PDF fixture probe passed, app all-target Clippy -Dwarnings passed, dedicated rustfmtcheck passed. Three generated PDFs were independently read by pypdf and PyMuPDF: expanded text includes the source math/result46 and “Area vector text Ω”; both collapsed files have no text/icon; the hidden-border error file has exactly one red vector line. All three have zero image XObjects. `artifacts/rea/area-print-fixtures-20261019/validation.json` records each byte count/SHA and both extracted texts; `verify-area-print-fixtures-20261019.py` reproduces validation. These helper fixtures do not replace root's full PrintSnapshot integration probe. No original reopening or pixel parity claim follows from these tests.
