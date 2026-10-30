// DOM events exercise the real eframe input path; diagnostics supply measured
// canvas coordinates. The clipboard stub belongs only to this verification host.
export async function verifyRegionSelection(editor, canvas, assert) {
  const encode = value => new TextEncoder().encode(JSON.stringify(value));
  const doc = () => JSON.parse(editor.inspectDocument());
  const paint = async () => { await new Promise(requestAnimationFrame); await new Promise(requestAnimationFrame); };
  const until = async (predicate, label) => {
    const end = performance.now() + 10000;
    while (!predicate()) { if (performance.now() > end) throw Error(`Timeout: ${label}; state=${JSON.stringify(editor.state())}`); await paint(); }
  };
  const initial = doc();
  const regions = [
    { id: 1, left: 100, top: 100, content: { Math: { kind: { Number: "1234" }, children: [] } }, source_region: null },
    { id: 2, left: 230, top: 180, content: { Text: "group clipboard text" }, source_region: null },
    { id: 3, left: 500, top: 350, content: { Math: { kind: { Number: "9999" }, children: [] } }, source_region: null },
  ];
  editor.loadDocument("group-selection.mathfolio", encode({ ...initial, title: "CAD selection test", width: 800, height: 1100, page_setup: null, next_id: 4, native_archive: null, calculation_cache: null, regions }), true);
  await until(() => editor.inspectRegionBounds().length === 3, "measured region geometry");
  const bounds = () => new Map(editor.inspectRegionBounds().map(r => [r.id, r]));
  const pointer = (type, point, buttons) => {
    const rect = canvas.getBoundingClientRect(); canvas.focus();
    canvas.dispatchEvent(new PointerEvent(type, { pointerId: 1, pointerType: "mouse", isPrimary: true,
      clientX: rect.left + point.x, clientY: rect.top + point.y, button: 0, buttons, bubbles: true, cancelable: true }));
  };
  const drag = async (from, to) => {
    pointer("pointermove", from, 0); pointer("pointerdown", from, 1); await paint();
    pointer("pointermove", to, 1); await paint(); pointer("pointerup", to, 0); await paint();
  };
  const keyboard = async key => {
    canvas.focus();
    for (const type of ["keydown", "keyup"]) canvas.dispatchEvent(new KeyboardEvent(type, { key, code: key, bubbles: true, cancelable: true }));
    await paint();
  };
  const a = bounds().get(1), b = bounds().get(2);
  const from = { x: a.left - 20, y: a.top - 20 }, to = { x: (b.left + b.right) / 2, y: b.bottom + 20 };
  const clean = editor.inspectDocument(), cleanState = editor.state();
  // At the default 1x zoom the first source anchor is (100,100),
  // with 94.5-point metric margins. These starts are 45 points outside
  // the body, or 12 points outside the paper; measured bounds locate them.
  for (const [label, start] of [
    ["left page margin", { x: a.left - 145, y: a.top - 20 }],
    ["top page margin", { x: a.left - 20, y: a.top - 145 }],
    ["canvas left of the paper", { x: a.left - 206, y: a.top - 20 }],
    ["canvas above the paper", { x: a.left - 20, y: a.top - 206 }],
  ]) {
    await drag(start, { x: b.right + 20, y: b.bottom + 20 });
    assert(JSON.stringify(editor.state().selected_regions) === "[1,2]" && editor.inspectDocument() === clean
      && editor.state().undo === cleanState.undo && !editor.state().dirty
      && editor.state().calculation_generation === cleanState.calculation_generation,
      `WASM marquee starts in ${label} without editing source or starting calculation`, editor.state());
  }
  await drag(from, to);
  assert(JSON.stringify(editor.state().selected_regions) === "[1]", "WASM CAD left-to-right requires complete enclosure", editor.state());
  await drag(to, from);
  assert(JSON.stringify(editor.state().selected_regions) === "[1,2]", "WASM CAD right-to-left includes crossing regions", editor.state());
  assert(editor.inspectDocument() === clean && editor.state().revision === cleanState.revision && !editor.state().dirty && editor.state().calculation_generation === cleanState.calculation_generation,
    "WASM marquee changes no source, revision, history, dirty state or calculation generation");
  const hit = bounds().get(2), start = { x: (hit.left + hit.right) / 2, y: (hit.top + hit.bottom) / 2 }, end = { x: start.x + 70, y: start.y + 45 };
  pointer("pointermove", start, 0); pointer("pointerdown", start, 1); await paint(); pointer("pointermove", end, 1); await paint();
  assert(editor.inspectDocument() === clean && editor.state().undo === cleanState.undo, "WASM group movement previews without editing source or history");
  pointer("pointerup", end, 0); await paint();
  const moved = doc();
  assert(moved.regions[0].left !== regions[0].left && moved.regions[1].left !== regions[1].left && JSON.stringify(moved.regions[2]) === JSON.stringify(JSON.parse(clean).regions[2]), "WASM drop moves the selected group only");
  assert(Math.abs((moved.regions[1].left - moved.regions[0].left) - (regions[1].left - regions[0].left)) < 1e-8 && editor.state().undo === cleanState.undo + 1, "WASM group drop preserves relative offsets and adds one Undo");
  editor.command("undo"); await paint();
  assert(JSON.stringify(doc().regions) === JSON.stringify(JSON.parse(clean).regions) && editor.state().selected_regions.length === 2, "WASM Undo restores both moved regions and selection");
  editor.command("redo"); await paint();
  assert(JSON.stringify(doc().regions) === JSON.stringify(moved.regions), "WASM Redo restores the group movement");

  const clipboard = navigator.clipboard;
  const previousWrite = Object.getOwnPropertyDescriptor(clipboard, "writeText");
  let copied = "";
  Object.defineProperty(clipboard, "writeText", { configurable: true, value: async value => { copied = value; } });
  const clipboardEvent = async (type, text = "") => {
    const data = new DataTransfer(); data.setData("text/plain", text); canvas.focus();
    canvas.dispatchEvent(new ClipboardEvent(type, { bubbles: true, cancelable: true, clipboardData: data }));
    await paint();
  };
  try {
    const beforeCopy = editor.inspectDocument(), copyRevision = editor.state().revision;
    await clipboardEvent("copy"); await until(() => copied.startsWith("mathfolio-regions:"), "group copy packet");
    assert(editor.inspectDocument() === beforeCopy && editor.state().revision === copyRevision, "WASM group Copy produces one structured packet without changing source");
    const beforePasteHistory = editor.state().undo;
    await clipboardEvent("paste", copied); await until(() => editor.state().regions === 5 && !editor.state().calculating, "group paste");
    const pasted = doc(), pastedIds = editor.state().selected_regions;
    assert(pastedIds.length === 2 && pastedIds.every(id => id > 3) && editor.state().undo === beforePasteHistory + 1, "WASM group Paste uses fresh IDs and one Undo");
    assert(JSON.stringify(pasted.regions.slice(0, 3)) === JSON.stringify(moved.regions) && JSON.stringify(pasted.regions[3].content) === JSON.stringify(moved.regions[0].content) && JSON.stringify(pasted.regions[4].content) === JSON.stringify(moved.regions[1].content), "WASM group Paste preserves sources and the existing regions");
    await keyboard("Delete"); await until(() => editor.state().regions === 3, "group deletion");
    assert(editor.state().selected_regions.length === 0 && JSON.stringify(doc().regions) === JSON.stringify(moved.regions), "WASM Delete removes the selected group only");
    editor.command("undo"); await paint();
    assert(editor.state().regions === 5 && JSON.stringify(editor.state().selected_regions) === JSON.stringify(pastedIds), "WASM Undo restores all deleted regions and selection");
    copied = ""; await clipboardEvent("cut"); await until(() => editor.state().regions === 3 && copied.startsWith("mathfolio-regions:"), "group cut");
    assert(JSON.stringify(doc().regions) === JSON.stringify(moved.regions), "WASM Cut copies then removes the complete selection");
    editor.command("undo"); await paint();
    const beforeInvalid = editor.inspectDocument(), invalidHistory = editor.state().undo;
    const invalid = JSON.parse(copied.slice("mathfolio-regions:".length)); invalid.items[1].region = { content: { Math: { kind: "BadKind", children: [] } } };
    await clipboardEvent("paste", "mathfolio-regions:" + JSON.stringify(invalid));
    assert(editor.inspectDocument() === beforeInvalid && editor.state().undo === invalidHistory, "WASM malformed multi-region Paste is atomic");
  } finally {
    if (previousWrite) Object.defineProperty(clipboard, "writeText", previousWrite); else delete clipboard.writeText;
  }
}
