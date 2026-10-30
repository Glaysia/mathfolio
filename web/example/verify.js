// Actual browser integration tests against the packaged Rust WASM and WebRunner.
import { MathfolioEditor, MathcraftEditor } from "../wasm-sdk/sdk.js";
import { MathfolioSession } from "../wasm-sdk/mathfolio.js";
const sleep = (ms) => new Promise(resolve => setTimeout(resolve, ms));
const encode = (value) => new TextEncoder().encode(JSON.stringify(value));
const tree = (kind, ...children) => ({ kind, children });
const number = (n) => tree({ Number: String(n) });
const identifier = (name, labels = "VARIABLE") => tree({ AnnotatedIdentifier: { name, labels } });
const binary = (op, a, b) => tree({ Binary: op }, a, b);
const evaluate = (a, unit = tree("Placeholder")) => binary("Evaluate", a, unit);
const scalar = (row) => row?.value?.quantity?.value?.[0];
async function until(predicate, label, timeout = 30000) {
  const start = performance.now();
  while (!predicate()) { if (performance.now() - start > timeout) throw Error(`Timeout: ${label}`); await sleep(20); }
}
export async function verify(editor, { canvas, font, bytes }) {
  const checks = []; const started = performance.now();
  const assert = (condition, label, details) => { if (!condition) throw Error(`${label}: ${JSON.stringify(details)}`); checks.push(label); console.log(`PASS ${label}`); };
  const reject = (fn, label) => { let rejected = false; try { fn(); } catch { rejected = true; } assert(rejected, label); };
  const post = async (receipt) => {
    document.body.dataset.test = receipt.ok ? "passed" : "failed";
    const report = document.querySelector("#report"); report.hidden = false; report.textContent = JSON.stringify(receipt, null, 2);
    await fetch("/receipt", { method: "POST", body: JSON.stringify(receipt) });
  };
  let other;
  try {
    reject(() => new MathfolioSession(999), "protocol mismatch rejected");
    assert(document.title === "Mathfolio WASM SDK", "verification host uses the Mathfolio title");
    assert(MathcraftEditor === MathfolioEditor, "legacy SDK class alias uses the same implementation");
    assert(canvas.width > 0 && canvas.height > 0, "real WebRunner sized canvas");
    assert(editor.state().worker_count === 0, "idle mount does not start a calculation Worker");
    let duplicateRejected = false;
    try { await MathfolioEditor.mount({ canvas }); } catch { duplicateRejected = true; }
    assert(duplicateRejected, "a canvas cannot be mounted by two editors");
    const initial = JSON.parse(editor.inspectDocument());
    const fixture = { ...initial, title: "SDK units matrix 한글", next_id: 7, native_archive: null, regions: [
      binary("Define", identifier("x"), number(6)),
      evaluate(binary("Add", identifier("x"), number(2))),
      evaluate(binary("Multiply", number(2), identifier("m", "UNIT")), identifier("cm", "UNIT")),
      evaluate(tree({ Matrix: { rows: 2, cols: 2 } }, number(1), number(2), number(3), number(4))),
      evaluate(tree({ StringLiteral: "한글" })),
    ].map((math, index) => ({ id: index + 1, left: 38, top: 38 + index * 76, content: { Math: math }, source_region: null })) };
    const source = encode(fixture);
    editor.loadDocument("sdk.mathfolio", source);
    await sleep(250);
    assert(editor.state().regions === 5 && !editor.state().dirty, "own bytes opened clean");
    assert(editor.inspectResults().size === 0 && !editor.state().calculating, "open never executes formulas");
    const beforeTheme = editor.inspectDocument(); const beforeRevision = editor.state().revision;
    editor.setThemeMode("dark"); editor.setThemeMode("nord");
    assert(editor.inspectDocument() === beforeTheme && editor.state().revision === beforeRevision, "theme never changes source or revision");
    reject(() => editor.setTheme({ dark: true }), "invalid host tokens rejected atomically");
    let ticks = 0; const heartbeat = setInterval(() => ticks++, 10);
    editor.command("calculate_worksheet");
    assert(editor.state().calculating, "calculation dispatched asynchronously");
    await until(() => !editor.state().calculating, "worker calculation"); clearInterval(heartbeat);
    const results = editor.inspectResults();
    assert(scalar(results.get(2)) === 8, "worker scalar result", [...results]);
    assert(results.get(3)?.display_value?.quantity?.value?.[0] === 200, "worker display units", results.get(3));
    assert(results.get(4)?.value?.rows === 2 && results.get(4)?.value?.elements?.length === 4, "worker matrix result", results.get(4));
    assert(results.get(5)?.value?.value === "한글", "worker Unicode string result", results.get(5));
    assert(ticks > 0, "UI heartbeat continues while WASM worker runs", ticks);
    const saved = editor.exportDocument("mathfolio");
    editor.loadDocument("restored.mathfolio", saved, true); await sleep(200);
    assert(!editor.state().calculating, "saved result reopen stays inert");
    const restored = JSON.parse(editor.inspectDocument());
    assert(restored.calculation_cache?.rows?.length >= 4, "saved calculation cache survives byte roundtrip", restored.calculation_cache);
    const legacySaved = editor.exportDocument("mathcraft");
    editor.loadDocument("legacy.mathcraft", legacySaved, true);
    assert(JSON.parse(editor.inspectDocument()).calculation_cache?.rows?.length >= 4 && !editor.state().calculating, "legacy extension and SDK format alias preserve saved results without recalculation");
    editor.loadDocument("pre-rename.mathcraft", await bytes("../fixtures/pre-rename.mathcraft"), true);
    assert(editor.state().regions === 7 && JSON.parse(editor.inspectDocument()).calculation_cache?.rows?.length === 7, "unchanged pre-rename fixture restores all seven calculation rows");
    assert(!editor.state().calculating && editor.state().worker_count === 0 && editor.inspectResults().size === 0, "opening historical cached results starts no worker or fresh calculation");
    editor.loadDocument("restored.mathfolio", saved, true);
    const native = editor.exportDocument("mcdx");
    assert(native[0] === 0x50 && native[1] === 0x4b, "MCDX export returns ZIP bytes");
    editor.loadDocument("roundtrip.mcdx", native, true);
    editor.command("calculate_worksheet"); await until(() => !editor.state().calculating, "native roundtrip calculation");
    assert(scalar(editor.inspectResults().get(2)) === 8, "MCDX reopen uses same engine");
    const pdf = editor.exportPdf();
    assert(new TextDecoder().decode(pdf.slice(0, 5)) === "%PDF-", "vector PDF returned as bytes");
    const pdfText = new TextDecoder("latin1").decode(pdf);
    assert(pdfText.includes("/FontFile") && !pdfText.includes("/Subtype /Image"), "text/path PDF includes embedded fonts without page bitmap");
    const stable = editor.inspectDocument();
    reject(() => editor.loadDocument("bad.mcdx", new Uint8Array([1, 2, 3]), true), "malformed native bytes rejected");
    assert(editor.inspectDocument() === stable, "failed load preserves current document");
    reject(() => editor.command("not_a_command"), "unknown command rejected");
    // Actual DOM keyboard events traverse eframe's web event handlers. This is
    // synthetic input, not a claim of physical keyboard/IME validation.
    const key = async (key, ctrl = false, shift = false) => {
      canvas.focus();
      for (const type of ["keydown", "keyup"]) canvas.dispatchEvent(new KeyboardEvent(type, { key, code: key === "Enter" ? "Enter" : key.length === 1 ? `Key${key.toUpperCase()}` : key, ctrlKey: ctrl, shiftKey: shift, bubbles: true, cancelable: true }));
      await sleep(80);
    };
    const empty = { ...initial, regions: [], next_id: 1 };
    editor.loadDocument("keyboard.mathfolio", encode(empty), true); await sleep(100);
    await key("t", true); await key("한"); await key("글");
    const typed = JSON.parse(editor.inspectDocument());
    assert(typed.regions.some(r => JSON.stringify(r.content).includes("한글")), "browser input reaches shared Rust editor", typed.regions);
    assert(editor.state().dirty, "browser edit marks session dirty");
    reject(() => editor.loadDocument("blocked.mathfolio", source), "unsaved replacement needs explicit discard");
    const failed = editor.prepareSave("keyboard.mathfolio");
    assert(!editor.acknowledgeSave(failed.id, false) && editor.state().dirty, "failed host save retains dirty");
    const older = editor.prepareSave("keyboard.mathfolio");
    const latest = editor.prepareSave("keyboard.mathfolio");
    reject(() => editor.acknowledgeSave(older.id, true), "superseded save acknowledgement rejected");
    await key("!");
    assert(!editor.acknowledgeSave(latest.id, true) && editor.state().dirty, "write of earlier revision cannot clear newer edit");
    const current = editor.prepareSave("keyboard.mathfolio");
    assert(editor.acknowledgeSave(current.id, true) && !editor.state().dirty, "host success acknowledges exact saved revision");
    await key("s", true);
    // Disable example request consumer before asserting transport requests.
    editor.command("open"); const requests = editor.takeRequests();
    assert(requests.some(r => r.kind === "open"), "host request transport returns IDs and intent", requests);
    editor.command("undo"); editor.command("redo");
    assert(JSON.parse(editor.inspectDocument()).regions.some(r => JSON.stringify(r.content).includes("한글")), "shared Undo/Redo preserves browser text");
    editor.command("close"); // dirty document: opens Rust's unsaved confirmation
    editor.loadDocument("replace-open-dialog.mathfolio", encode(empty), true);
    await key("t", true); await key("정");
    assert(editor.inspectDocument().includes("정"), "document replacement clears previous confirmation and draft UI");
    editor.loadDocument("math-input.mathfolio", encode(empty), true);
    await key("m", true, true); await key("2"); await key("+"); await key("3"); await key("=");
    const mathId = JSON.parse(editor.inspectDocument()).regions[0]?.id;
    assert(mathId && !editor.inspectResults().get(mathId)?.value, "math draft stays unevaluated before Enter");
    await key("Enter"); await until(() => !editor.state().calculating, "Enter commit calculation");
    assert(scalar(editor.inspectResults().get(mathId)) === 5, "Enter commits and calculates through original editor", [...editor.inspectResults()]);
    editor.loadDocument("literal.mathfolio", encode(empty), true);
    await key("m", true, true); await key("r"); await key("-", true); await key("1"); await key("Enter");
    assert(editor.inspectDocument().includes('"subscript":"1"'), "Ctrl+- remains literal subscript in browser", editor.inspectDocument());
    const program = (...lines) => tree({ Program: { statement_count: lines.length } }, ...lines);
    const local = (name, value) => tree("LocalDefine", identifier(name), value);
    const call = (name, ...args) => tree({ Call: { argument_count: args.length } }, identifier(name, "FUNCTION"), ...args);
    const workload = (iterations, size) => ({ ...empty, next_id: 2, regions: [{ id: 1, left: 38, top: 38, source_region: null, content: { Math: evaluate(program(local("total", number(0)), tree("For", identifier("i"), tree({ Range: { has_step: false } }, number(1), number(iterations)), program(local("total", binary("Add", identifier("total"), call("det", call("identity", number(size))))))), identifier("total"))) } }] });
    const heavy = workload(16384, 128);
    editor.loadDocument("workload-proof.mathfolio", encode(workload(1, 4)), true); editor.command("calculate_worksheet");
    await until(() => !editor.state().calculating, "bounded workload proof");
    assert(scalar(editor.inspectResults().get(1)) === 1, "cancellation workload is a valid matrix computation", [...editor.inspectResults()]);
    editor.loadDocument("long.mathfolio", encode(heavy), true); editor.command("calculate_worksheet");
    await until(() => editor.state().worker_started_generation === editor.state().calculation_generation, "worker actually entered job");
    assert(editor.state().calculating, "expensive job is still executing in Worker", { state: editor.state(), results: [...editor.inspectResults()] });
    const stopStart = performance.now(); editor.command("stop"); await until(() => !editor.state().calculating, "worker termination");
    const stopMs = performance.now() - stopStart;
    assert(stopMs < 500 && editor.state().worker_count === 0, "stop terminates executing worker without blocking UI", stopMs);
    editor.loadDocument("resume.mathfolio", source, true); editor.command("calculate_worksheet");
    await until(() => !editor.state().calculating, "worker recreation");
    assert(scalar(editor.inspectResults().get(2)) === 8, "calculation resumes on a fresh Worker");
    const workerMemory = editor.state().worker_memory_bytes;
    editor.loadDocument("long-again.mathfolio", encode(heavy), true); editor.command("calculate_worksheet");
    await until(() => editor.state().worker_started_generation === editor.state().calculation_generation, "second executing job");
    editor.loadDocument("replacement.mathfolio", source, true); editor.command("calculate_worksheet");
    await until(() => !editor.state().calculating, "replacement job");
    assert(scalar(editor.inspectResults().get(2)) === 8 && editor.state().regions === 5, "old document job cannot overwrite replacement session");
    const stale = editor.prepareSave("replacement.mathfolio"); editor.loadDocument("another.mathfolio", source, true);
    reject(() => editor.acknowledgeSave(stale.id, true), "old document save acknowledgement rejected");
    const secondCanvas = document.createElement("canvas"); secondCanvas.style.cssText = "width:400px;height:300px"; document.body.append(secondCanvas);
    other = await MathfolioEditor.mount({ canvas: secondCanvas, fonts: [{ name: "Nanum Gothic", bytes: font }] });
    other.loadDocument("other.mathfolio", encode({ ...fixture, title: "isolated second session" }));
    assert(other.state().regions === 5 && other.inspectDocument() !== editor.inspectDocument(), "two simultaneous sessions isolate document state");
    const changed = structuredClone(fixture); changed.regions[0].content.Math.children[1] = number(60);
    other.loadDocument("other.mathfolio", encode(changed), true);
    editor.command("calculate_worksheet"); other.command("calculate_worksheet");
    await until(() => !editor.state().calculating && !other.state().calculating, "two simultaneous calculators");
    assert(scalar(editor.inspectResults().get(2)) === 8 && scalar(other.inspectResults().get(2)) === 62, "parallel sessions isolate Worker bindings");
    other.dispose(); other.dispose(); reject(() => other.state(), "disposed session rejects later operations"); secondCanvas.remove(); other = null;
    const inputAgents = document.querySelectorAll("main input").length;
    for (let cycle = 0; cycle < 3; cycle++) {
      const target = document.createElement("canvas"); target.style.cssText = "width:300px;height:200px"; document.querySelector("main").append(target);
      other = await MathfolioEditor.mount({ canvas: target });
      if (cycle === 0) {
        const firstPdf = other.exportPdf();
        assert(new TextDecoder().decode(firstPdf.slice(0, 5)) === "%PDF-", "PDF export does not wait for a UI paint frame");
      }
      if (cycle === 2) other.subscribe(() => other.dispose()); else other.dispose();
      if (cycle === 0) { other = await MathfolioEditor.mount({ canvas: target }); other.dispose(); assert(true, "same canvas can be remounted after dispose"); }
      target.remove(); other = null;
      assert(document.querySelectorAll("main input").length === inputAgents, `mount/dispose cycle ${cycle + 1} releases input agent`);
    }
    const stateBeforeInactive = editor.state(); editor.setActive(false); await sleep(150); const frameCount = editor.state().frames; await sleep(150);
    assert(editor.state().frames === frameCount, "inactive surface stops worksheet frames"); editor.setActive(true);
    assert(editor.state().revision === stateBeforeInactive.revision, "visibility changes preserve revision");
    // Existing real-world source fixture, including tables and saved colors.
    for (const name of ["1015_LCC_S.mcdx", "1016_LCC_S.mcdx"]) {
      const data = await bytes(`/fixtures/${name}`); editor.loadDocument(name, data, true); await sleep(150);
      const doc = JSON.parse(editor.inspectDocument());
      assert(doc.regions.length > 10 && !editor.state().calculating, `${name}: original source and inert saved results loaded`);
      const own = editor.exportDocument("mathfolio"); const exported = editor.exportDocument("mcdx");
      editor.loadDocument("reopened.mathfolio", own, true);
      assert(JSON.parse(editor.inspectDocument()).regions.length === doc.regions.length && exported.length > 0, `${name}: own and native byte exports`);
    }
    const memoryBytes = editor.memoryBytes();
    editor.command("close");
    assert(editor.takeRequests().some(r => r.kind === "close"), "close is delegated to the host view");
    editor.dispose(); editor.dispose(); reject(() => editor.command("save"), "dispose is idempotent and blocks callbacks");
    assert(document.querySelectorAll("main input").length === 0, "dispose removes eframe text agent");
    assert(window.ownedWorkerCount() === 0, "all owned Workers receive terminate during cleanup");
    await sleep(100); // Let asynchronous worker-close events settle before retaining evidence.
    await post({ ok: true, checks, elapsed_ms: performance.now() - started, memory_bytes_ui_instance: memoryBytes, memory_bytes_worker: workerMemory, executing_worker_stop_ms: stopMs, cross_origin_isolated: crossOriginIsolated, user_agent: navigator.userAgent, synthetic_input: true, wasm_sha256: [...new Uint8Array(await crypto.subtle.digest("SHA-256", await bytes("../wasm-sdk/mathfolio_bg.wasm")))].map(b => b.toString(16).padStart(2, "0")).join("") });
  } catch (error) {
    other?.dispose(); await post({ ok: false, checks, error: String(error), stack: error?.stack, elapsed_ms: performance.now() - started }); throw error;
  }
}
