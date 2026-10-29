import { compileSceneBuild, runSceneBuild, type SceneBuildRequest, type SceneBuildResult } from "./run";

// The scene-build worker: compiles the wasm once per URL, then runs one build
// per message. Output buffers are transferred back, not copied.

const scope = globalThis as unknown as {
  onmessage: ((ev: MessageEvent<SceneBuildRequest>) => void) | null;
  postMessage: (msg: { id: number; result: SceneBuildResult }, transfer: Transferable[]) => void;
};

const modules = new Map<string, Promise<WebAssembly.Module>>();

scope.onmessage = (ev) => {
  const { id, wasmUrl, files, options } = ev.data;
  void (async () => {
    let result: SceneBuildResult;
    try {
      let module = modules.get(wasmUrl);
      if (!module) {
        module = compileSceneBuild(wasmUrl);
        modules.set(wasmUrl, module);
        // A failed fetch or compile is not cached; the next build retries it.
        // This build still sees the rejection through the await below.
        module.catch(() => {
          modules.delete(wasmUrl);
        });
      }
      result = await runSceneBuild(await module, files, options);
    } catch (e) {
      result = { ok: false, error: e instanceof Error ? e.message : String(e), ms: 0 };
    }
    const transfer = result.ok ? Object.values(result.files).map((b) => b.buffer as ArrayBuffer) : [];
    scope.postMessage({ id, result }, transfer);
  })();
};
