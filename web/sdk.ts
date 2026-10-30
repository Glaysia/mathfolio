import init, { MathfolioSession } from "./mathfolio.js";

export const protocolVersion = 1;
let initialization: ReturnType<typeof init> | undefined;
const mountedCanvases = new WeakSet<HTMLCanvasElement>();
export type ThemeMode = "follow_host" | "light" | "dark" | "nord";
export type RGBA = [number, number, number, number];
export interface HostTheme {
  dark: boolean; high_contrast: boolean;
  workspace: RGBA; panel: RGBA; surface: RGBA; border: RGBA;
  text: RGBA; muted_text: RGBA; accent: RGBA; button: RGBA; button_text: RGBA;
  hover: RGBA; selection: RGBA; selection_text: RGBA; focus: RGBA;
  error: RGBA; warning: RGBA; font_size: number; corner_radius: number;
}
export interface SessionState {
  revision: number; document_epoch: number; dirty: boolean; calculating: boolean;
  pending: number; regions: number; selected?: number; status: string;
  frames: number; undo: number; redo: number;
  worker_memory_bytes?: number; worker_count: number;
  calculation_generation: number; worker_started_generation?: number;
}
export interface HostRequest { id: number; kind: "open" | "save" | "save_as" | "export_mcdx" | "pdf" | "print" | "close" }
export interface SavePacket { id: number; revision: number; bytes: Uint8Array }
export type Command = "undo" | "redo" | "calculate_now" | "calculate_worksheet" | "stop" | "new" | "save" | "open" | "close";
export interface MountOptions {
  canvas: HTMLCanvasElement;
  /** Override for CSP/resource URI handling; packaged relative URLs are the default. */
  wasm?: RequestInfo | URL | Response | BufferSource | WebAssembly.Module;
  workerUrl?: string | URL;
  fonts?: { name: string; bytes: Uint8Array }[];
}
/** Thin lifecycle/IO binding. The document, editor, undo and calculations live in Rust. */
export class MathfolioEditor {
  private disposed = false;
  private observers = new Set<(state: SessionState) => void>();
  private requestObservers = new Set<(request: HostRequest) => void>();
  private timer?: ReturnType<typeof setInterval>;
  private lastState = "";
  private active = true;
  private constructor(private readonly session: MathfolioSession, private readonly canvas: HTMLCanvasElement) {}
  static async mount(options: MountOptions): Promise<MathfolioEditor> {
    initialization ??= init({ module_or_path: options.wasm ?? new URL("mathfolio_bg.wasm", import.meta.url) }).catch(error => { initialization = undefined; throw error; });
    await initialization;
    if (mountedCanvases.has(options.canvas)) throw new Error("Canvas already has a Mathfolio editor");
    mountedCanvases.add(options.canvas);
    let session: MathfolioSession | undefined;
    try {
      session = new MathfolioSession(protocolVersion);
      for (const font of options.fonts ?? []) session.provideFont(font.name, font.bytes);
      await session.mount(options.canvas, String(options.workerUrl ?? new URL("worker.js", import.meta.url)));
      return new MathfolioEditor(session, options.canvas);
    } catch (error) { session?.dispose(); session?.free(); mountedCanvases.delete(options.canvas); throw error; }
  }
  private check(): void { if (this.disposed) throw new Error("Editor disposed"); }
  loadDocument(name: string, bytes: Uint8Array, discardUnsaved = false): void {
    this.check(); this.session.loadDocument(name, bytes, discardUnsaved);
  }
  state(): SessionState { this.check(); return this.session.state() as SessionState; }
  takeRequests(): HostRequest[] { this.check(); return this.session.takeRequests() as HostRequest[]; }
  command(command: Command): void { this.check(); this.session.command(command); }
  exportDocument(format: "mathfolio" | "mathcraft" | "mcdx"): Uint8Array { this.check(); return this.session.exportDocument(format); }
  exportPdf(): Uint8Array { this.check(); return this.session.exportPdf(); }
  prepareSave(name: string): SavePacket { this.check(); return this.session.prepareSave(name) as SavePacket; }
  acknowledgeSave(id: number, success: boolean): boolean {
    this.check(); if (!Number.isSafeInteger(id) || id < 1 || id > 0xffffffff) throw new Error("Invalid save ID");
    return this.session.acknowledgeSave(id, success);
  }
  setTheme(tokens: HostTheme | null): boolean { this.check(); return this.session.setTheme(tokens); }
  setThemeMode(mode: ThemeMode): void { this.check(); this.session.setThemeMode(mode); }
  setActive(active: boolean): void { this.check(); this.active = active; this.session.setActive(active); }
  /** Small state only, at most 10Hz; no per-frame document/AST serialization. */
  subscribe(callback: (state: SessionState) => void): () => void {
    this.check(); this.observers.add(callback); callback(this.state());
    if (this.disposed) return () => {};
    this.startPolling();
    return () => { this.observers.delete(callback); this.stopUnusedPolling(); };
  }
  onRequest(callback: (request: HostRequest) => void): () => void {
    this.check(); this.requestObservers.add(callback); this.startPolling();
    return () => { this.requestObservers.delete(callback); this.stopUnusedPolling(); };
  }
  private startPolling(): void {
    if (!this.timer) this.timer = setInterval(() => {
      if (!this.active) return;
      const state = this.state(); const key = JSON.stringify({ ...state, frames: 0 });
      if (key !== this.lastState) { this.lastState = key; for (const observer of [...this.observers]) { observer(state); if (this.disposed) return; } }
      if (this.requestObservers.size) for (const request of this.takeRequests()) for (const observer of [...this.requestObservers]) { observer(request); if (this.disposed) return; }
    }, 100);
  }
  private stopUnusedPolling(): void {
    if (!this.observers.size && !this.requestObservers.size) { clearInterval(this.timer); this.timer = undefined; }
  }
  /** Diagnostic snapshots are explicit and are never used by the render loop. */
  inspectDocument(): string { this.check(); return this.session.inspectDocument(); }
  inspectResults(): Map<number, unknown> { this.check(); return this.session.inspectResults() as Map<number, unknown>; }
  /** Memory is for the shared UI WASM instance, not per-session or total worker memory. */
  memoryBytes(): number { this.check(); return this.session.memoryBytes(); }
  dispose(): void {
    if (this.disposed) return;
    clearInterval(this.timer); this.observers.clear(); this.requestObservers.clear(); this.session.dispose(); this.session.free(); this.disposed = true; mountedCanvases.delete(this.canvas);
  }
}

/** @deprecated Use MathfolioEditor. Retained for clients of the 0.1.0 SDK. */
export { MathfolioEditor as MathcraftEditor };
