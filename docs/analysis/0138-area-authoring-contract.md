# Original empty Area authoring contract

2026-10-19. Static original command/factory/CIL/schema/package evidence only. No original GUI, screenshot, worksheet runtime or managed assembly execution. Existing Area storage/printing products remain unchanged by this investigation.

## Identity

Fresh REA `ev_399d4abae6dcece22d853cf7ba26c2c0116de8a92cbf1dc94368e2ace927a142` inspects installed WorksheetCommands and AddItemAtInsertionPointAndActivateCommand,9 methods. Full retained observation: `artifacts/rea/area-authoring-fresh-rea-20261019.json`.

Worksheet DLL SHA `c1a70ec8007228ff119d4a31b36b9062132be3a10c771459c1dbe3fd0a7baddd`, MVID `44f31187-dcfc-4afd-a7d6-50fbee8dc361`. Area DLL SHA `63418ae37c2e16fc49398205f28279dc0d1b7d5301940458497a504309be5590`, MVID `ea79fc32-ec3c-4c02-8e2c-af8c0b3cd512`. Other exact identities, all selected raw CIL hashes/paths and generic method-spec resolutions are in `area-authoring-method-ledger-20261019.json`. Every raw CIL dump rechecks installed artifact bytes against the retained assembly/body hashes. Tokens are build-local coordinates.

## Creation command: empty, not selection wrapping

| Step | Exact installed path |
|---|---|
| Shortcut | WorksheetCommands03f0 IL0992–09a6 registers NewCollapsibleArea with CtrlShiftA; command construction01bd–01d6 has functionality key771. |
| Executed handler | Worksheet0adf IL0447–0464 binds action0b80 and predicate0b81.0b80 calls CreateCollapsibleAreaControl0a2d only. |
| New control |0a2d passes current worksheet GridCellSize to factory0082; the factory constructs a new Area control/model and sets RegionFactory. No selected regions are passed/copied/moved. |
| Visible initial height |0a2d reads PageLayout.MajorGridLines and calls FrameworkElementFacade.SetHeight, then AddItemAtInsertionPointAndActivate0aad. Metric major spacing is2cm (controls-core0886); US major spacing is1inch (0887), independent of fine-grid cell switching0869. |
| Initial content | Area model0061 calls WhiteboardModel017b, which allocates an empty WhiteboardItemsCollection. No default math/placeholder child exists. Writer0034 obtains that empty Regions collection, making an empty regions element a supported native shape. |
| Initial view state | Model0061 sets border visible; collapse bool is false/default, protection is absent. Area VM00a5 stores insertion point0.2inch/0.2inch directly (19.2DIP each axis);00c8 returns the same default. This direct default assignment is not the clicked-grid snapping path. |

Raw groups: `feedback/area-authoring-{shortcut,bindings,dispatch,factory,defaults,add-transaction}-cil-20261019.txt`.

Creation is **not** an original wrap-selected-regions command. Predicate0b81 resolves generic CanCreateControlOfType<ICollapsibleAreaControl>0a90. GetAddableContainer0a92 uses the active section only when both ActiveItem is null and selected count is0; otherwise it requires ActiveDescendant to implement a writable IWhiteboardItemContainer. Whiteboard01c4 returns null without active item, an ordinary active math item itself, or the deepest active container. Consequently ordinary selected math/text, or a multiple selection with no active container, cannot satisfy the new-Area predicate. No conversion of that selection into Area children appears in this path.

Type restrictions use0a93 and Worksheet VM cctor096e's exclusion map: Area is excluded from header/footer, aggregate/solve-block containers, Excel/OLE component containers and Area. Area's own excluded children include worksheet page break, templated text box and another Area. This agrees with shipped Area help. Area CanAddItems00b5 requires expanded state, not activation-protected, and base CanAddItems; it is not an unrestricted editable-parent test.

## Location, focus and Undo

For a plain worksheet insertion, AddItemAtInsertionPointAndActivateCommand0e65 remembers the old section insertion point,0e68 adds the one new item at that point, activates it and makes it the section ActiveItem. Undo0e66 removes that item and restores the insertion point; Redo0e67 reuses AddItem. This is one ordinary worksheet command, not several per-child insertions.

Area implements IPageWideSizable. Whiteboard0145 treats that interface as ignoring horizontal movement/width. Worksheet0a11 therefore changes the Area insertion X to0, applies its original collision/grid adjustment to Y and can push later regions vertically. Only the blank/no-overlap case is fully bounded for new authoring here; do not invent arbitrary selected-item wrapping or a simplified overlap policy. Raw0a11 includes the full branch for follow-up.

Area clicked insertion point00c5 snaps to the Area grid. If snapped Y is0 it moves down one grid cell; if Y equals current ContentHeight it moves up one cell. This is distinct from the19.2/19.2 constructor default. The Area-local point is not a new global worksheet coordinate. Protected/collapsed Area cannot accept new children.

## First typed character creates a child immediately

Worksheet0a39 HandleTextInput first checks generic CanCreateControlOfType<IEquationControl>. It starts CommandScope("Create Equation Control Via Keypress") at005a–0064, creates/validates a text-input equation control, calls CreateNewEquation0a12(null), waits for its load and sends the first character via Keypress0086–0096. CreateNewEquation creates a new equation, then AddItemAtInsertionPointAndActivate0aad. If an Area is the active container,0aad delegates to that parent's AddItem; Whiteboard00e3 routes to AddItemAndActivate when there is no nested active container. The remaining substring Keypress occurs after scope disposal (0a39:00a7–00bb).

Thus the positive original contract is immediate child existence on the first supported text key and a command scope grouping creation plus that first character. Deferring the child's existence until Enter is a separate reconstruction draft policy, not established original behavior. Later character/IME undo grouping and unsupported text factory behavior remain separate editor contracts. If an ordinary child equation is active, the generic new-control guard does not treat it as an addable container; that child handles input itself.

Raw: `area-authoring-{first-input,new-equation,active-selection,protection-add-guard}-cil-20261019.txt`.

## Critical height distinction and correction to earlier shortcuts

`AreaHeight` is a **retained expanded ContentHeight**, not a reliable live height for every expanded Area.

- VM constructor00a5 does not set `_areaHeight`, leaving0.00a98/0099 convert that field to/from grid-height units without rounding.
- On collapse,00d4 IL000b–0012 copies live VM.ContentHeight into AreaHeight, then sets minimum height0, hides crosshair and sets ContentHeight to CollapsedAreaHeight.00d7 initializes that collapsed content-height constant to1.0DIP.
- On expansion,00d4 IL00a2–00bd computes minimum size and restores ContentHeight from the retained AreaHeight. While collapsed, child growth can increase the retained height (00c3). Ordinary expanded child growth takes the base path, so a previously retained AreaHeight can become stale as a live-size estimate.
- Writer Area0034 serializes the retained getter value. It does not read WPF ActualHeight for AreaHeight. Shipped expanded template Areas omit AreaHeight/default0 even with many visible children, consistent with this distinction.

Worksheet RegionFactory0325 separately serializes a non-NaN FrameworkElement.Height into owning-region `height` (00eb–0101), then FrameworkElement.ActualHeight into `actualHeight` (0121–0130) and ActualWidth into actualWidth.002ef chooses the control itself unless a resizable sub-element explicitly requests dimension serialization. Area control has no IResizableElementInfo implementation in the inspected hierarchy. The source `height`, measured `actualHeight`, VM.ContentHeight and retained AreaHeight are distinct coordinates/state values. The complete WPF binding equivalence between the first three was not proven here; do not silently equate stored actualHeight with current live ContentHeight.

This corrects any interpretation of0132/0134 flag-only helper behavior as complete original collapse geometry. Such helpers preserve source display attrs, but do not independently reproduce the live-height capture required for a newly authored Area whose retained AreaHeight is0. AreaUI/print should not use AreaHeight as the sole live expanded size.

### Bounded API guidance

A safe app transition should accept an explicit finite current **live ContentHeight** from the Area renderer/model. On false→true, retain that height as AreaHeight, set collapsed state and return next live content height1. On true→false, return the stored expanded height and set expanded state; do not replace the stored value with the old collapsed height. Border-only toggles do not change geometry. Invalidate obsolete measured owning-region dimensions when a state/source transition changes geometry.

A readonly geometry accessor should expose separate optional owning-region `height`, `actualHeight`, retained AreaHeight and collapse state instead of presenting one guessed height. New empty expanded authoring can faithfully state retained AreaHeight0 and visible initial control height one major grid interval. Native writer may omit AreaHeight0 by default. Exact placement of that live height into source `height` versus app-only geometry should remain explicit until the WPF binding path is fully established. Merely copying old actualHeight into AreaHeight is only a source-fallback approximation and must not be labelled exact.

## Remaining limits

No new creation/wrapping product code is implemented by this investigation. Exact overlap-Y relocation and neighbor push transaction effects, WPF Height/ContentHeight binding, original first-key factory edge cases, fully protected/encrypted payload creation and broader text/plot/solve-block child authoring are not claimed. Parent/root authoring workers own bounded implementations using these positive contracts. No runtime serialization sample from a freshly inserted original Area was captured.
