import { MathcraftEditor } from "../wasm-sdk/sdk.js";
const status = document.querySelector("#status");
const canvas = document.querySelector("canvas");
// Test-host instrumentation, absent from the SDK and normal example execution.
if (new URL(location.href).searchParams.has("test")) {
  const OriginalWorker = globalThis.Worker;
  const live = new Set();
  globalThis.Worker = class extends OriginalWorker {
    constructor(...args) { super(...args); live.add(this); }
    terminate() { live.delete(this); super.terminate(); }
  };
  window.ownedWorkerCount = () => live.size;
}
const bytes = async (url) => { const response = await fetch(url); if (!response.ok) throw Error(`${url}: ${response.status}`); return new Uint8Array(await response.arrayBuffer()); };
const download = (data, name, type) => {
  const url = URL.createObjectURL(new Blob([data], { type }));
  const a = document.createElement("a"); a.href = url; a.download = name; a.click(); setTimeout(() => URL.revokeObjectURL(url), 1000);
};
try {
  const font = await bytes("../wasm-sdk/fonts/nanum/NanumGothic-Regular.ttf");
  const editor = await MathcraftEditor.mount({ canvas, fonts: [{ name: "Nanum Gothic", bytes: font }] });
  const lifetime = new AbortController();
  window.editor = editor;
  editor.setThemeMode("nord");
  let name = "worksheet.mathcraft";
  const save = () => { const packet = editor.prepareSave(name); download(packet.bytes, name, "application/octet-stream"); /* A download is not a confirmed host write: do not ack. */ };
  document.querySelector("#open").onchange = async ({ target }) => {
    const file = target.files[0]; if (file) { editor.loadDocument(file.name, new Uint8Array(await file.arrayBuffer()), confirm("현재 문서를 교체할까요?")); name = file.name; }
  };
  document.querySelector("#save").onclick = save;
  document.querySelector("#pdf").onclick = () => download(editor.exportPdf(), "worksheet.pdf", "application/pdf");
  editor.subscribe((state) => {
    status.textContent = `${state.regions}개 영역 · ${state.calculating ? "계산 중" : state.dirty ? "수정됨" : "준비됨"}`;
  });
  if (!new URL(location.href).searchParams.has("test")) editor.onRequest((request) => {
      if (request.kind === "open") document.querySelector("#open").click();
      else if (request.kind === "close") {
        lifetime.abort(); editor.dispose(); canvas.remove(); status.textContent = "편집기 닫힘";
        document.querySelectorAll("header button, header input").forEach(element => { element.disabled = true; });
      }
      else if (request.kind === "pdf" || request.kind === "print") download(editor.exportPdf(), "worksheet.pdf", "application/pdf");
      else if (request.kind === "export_mcdx") download(editor.exportDocument("mcdx"), "worksheet.mcdx", "application/octet-stream");
      else save();
  });
  document.addEventListener("visibilitychange", () => editor.setActive(!document.hidden), { signal: lifetime.signal });
  window.addEventListener("pagehide", () => { lifetime.abort(); editor.dispose(); }, { once: true, signal: lifetime.signal });
  window.mathcraftReady = true;
  const params = new URL(location.href).searchParams;
  if (params.has("scene")) { name=params.get("scene"); editor.loadDocument(name, await bytes(`/fixtures/${encodeURIComponent(name)}`)); }
  document.body.dataset.ready = "true";
  if (params.has("test")) {
    await (await import("./verify.js")).verify(editor, { canvas, font, bytes });
    lifetime.abort();
  }
} catch (error) {
  status.textContent = `실패: ${error}`; console.error(error);
  await fetch("/receipt", { method: "POST", body: JSON.stringify({ ok: false, error: String(error), stack: error?.stack }) }).catch(() => {});
}
