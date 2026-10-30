# Mathfolio WASM SDK 0.1.2-rc1 / protocol 1

The native Rust worksheet editor, calculator, unit system, history, formatting
and vector PDF writer, hosted on a Canvas. No separate TS math/editor model.
Obsidian and VS Code plugins belong to separate repositories.

```ts
import { MathfolioEditor } from "./sdk.js";

const editor = await MathfolioEditor.mount({
  canvas,
  fonts: [{ name: "Nanum Gothic", bytes: koreanFontBytes }],
});
editor.setThemeMode("nord");
editor.loadDocument("example.mcdx", documentBytes);
editor.onRequest(request => {
  // Connect host open/save/export/PDF actions here.
});

const save = editor.prepareSave("example.mcdx");
try {
  await hostWrite(save.bytes);
  editor.acknowledgeSave(save.id, true);
} catch (error) {
  editor.acknowledgeSave(save.id, false);
  throw error;
}
// On tab/view destruction:
editor.dispose();
```

Serve/package `sdk.js`, `mathfolio.js`, `mathfolio_bg.wasm`, and `worker.js`
together. Default URLs are resolved relative to sdk.js; override workerUrl and
wasm for host resource URIs. If using a custom worker entry, it must initialize
the same SDK module/version. Keep worker.js and its sibling WASM/glue resolvable.
No CDN, local server or native helper is required by the SDK. The repository's
loopback example server is only a test harness.

For restrictive hosts, allow the packaged script/worker/resource URIs and WASM
compilation (`wasm-unsafe-eval`). Do not enable general JS `unsafe-eval`.
SharedArrayBuffer/cross-origin isolation is not required. Chromium WebGL is used
for this web build; the Windows native renderer is unchanged.

`loadDocument` rejects replacement of dirty documents unless discardUnsaved is
explicit. Save packets are snapshots; acknowledge only after the actual host
write succeeds. Acknowledging an earlier revision does not clear newer edits.
The host must detect external file conflicts before writing; this is not a
filesystem compare-and-swap service. A browser download is not write confirmation.

Supply fonts before mount. The included optional Nanum Gothic CJK font is
`fonts/nanum/NanumGothic-Regular.ttf`; its bytes are not inside WASM. STIX and
egui default fonts are inside WASM. Theme mode and semantic RGBA tokens follow
the supplied UI theme without changing worksheet source colors.

Canvas CSS size/DPI are observed by WebRunner; call setActive(false) on hidden
views. setActive resumes rendering, not calculations. subscribe reports small
state at up to 10Hz; onRequest separately receives explicit host actions.
Unsubscribe or dispose to remove SDK timers. No document/AST snapshot crosses
the boundary per frame. inspectDocument/inspectResults are explicit diagnostics.
state().selected_regions reports selected region IDs. CAD marquee selection and
group move/copy/cut/paste/delete use the same Rust path as the native app.
inspectRegionBounds() explicitly returns last-measured Canvas bounds in egui
points for diagnostics, omitting regions not yet measured. It uses committed
positions during movement previews; do not poll it every frame.

Workers start lazily on the first calculation job, not on idle mount or saved
result opening. Calculations use one persistent Worker per calculation session. Stop
terminates it; restarting recreates bindings only when the next calculation
command authorizes evaluation. Prior display values stay pending; stopped
bindings are not marked fresh. Jobs carry increasing generations and document
replacement resets their transport. Incremental source deltas and a shared
multi-view calculation scheduler are not part of this version.

memoryBytes is the high-water size of the shared UI WASM memory, not total app
RSS or a per-view allocation. State exposes the latest responding worker's
memory size; it is unknown until its first reply. WASM memory can grow and need
not shrink after freeing views. Disk/package size is a separate measurement.

Browser integration is tested by the repository's web/example/verify.js. Actual
Obsidian/VS Code host integration, host conflict/backup policy, physical Korean
IME workflows and host keybinding arbitration must be verified in plugin repos.

Mathfolio: AGPL-3.0-only (LICENSE). Third-party fonts/palette retain their own
licenses under licenses/ and fonts/nanum/. Preserve these with distributions.
