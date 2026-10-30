// This module is packaged locally beside the WASM; no CDN or helper server.
import init, { CalculatorWorker } from "./mathfolio.js";
const ready = init().then(() => new CalculatorWorker());
// Install before asynchronous WASM loading so the first message cannot be lost.
self.onmessage = async ({ data }) => {
  try {
    const worker = await ready;
    self.postMessage({ kind: "started", generation: data?.[0]?.generation ?? 0 });
    self.postMessage(worker.run(data));
  }
  catch (error) {
    const generation = data?.[0]?.generation ?? 0;
    self.postMessage({ generation, updates: [], plots: [], error: String(error), cancelled: false });
  }
};
