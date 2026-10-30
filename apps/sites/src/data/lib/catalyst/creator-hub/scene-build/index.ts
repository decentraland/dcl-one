import {
  isBuildInput,
  isPrunedDir,
  type SceneBuildOptions,
  type SceneBuildRequest,
  type SceneBuildResult,
} from "./run";

export { isBuildInput, type SceneBuildOptions, type SceneBuildReport, type SceneBuildResult } from "./run";

// The main-thread side of the in-browser scene build: one long-lived worker
// (scene-build.worker.ts) that compiles the wasm on first use, fed only the
// files a build can read.

/** Served beside the prebuilt template runtime (deploy/nix/scene-build-wasm.nix). */
export const SCENE_BUILD_WASM_URL = "/template-bundles/scene-build.wasm";

/** Fetching and compiling the 10 MB module dominates; a build takes about 150 ms. */
const DEFAULT_TIMEOUT_MS = 120_000;

export type SceneBuildFile = { file: string; content: Uint8Array };
export type SceneBuilder = (
  files: Iterable<SceneBuildFile>,
  opts?: SceneBuildOptions,
) => Promise<SceneBuildResult>;

type WorkerLike = {
  postMessage: (msg: SceneBuildRequest) => void;
  terminate: () => void;
  onmessage: ((ev: MessageEvent<{ id: number; result: SceneBuildResult }>) => void) | null;
  onerror: ((ev: Event) => void) | null;
};

function spawnWorker(): WorkerLike | null {
  if (typeof Worker === "undefined") return null;
  return new Worker(new URL("./scene-build.worker.ts", import.meta.url), {
    type: "module",
    name: "scene-build",
  }) as unknown as WorkerLike;
}

export function createSceneBuilder({
  wasmUrl = SCENE_BUILD_WASM_URL,
  timeoutMs = DEFAULT_TIMEOUT_MS,
  spawn = spawnWorker,
}: { wasmUrl?: string; timeoutMs?: number; spawn?: () => WorkerLike | null } = {}): SceneBuilder {
  let worker: WorkerLike | null = null;
  let nextId = 1;
  const pending = new Map<number, (r: SceneBuildResult) => void>();

  const fail = (error: string) => {
    worker?.terminate();
    worker = null;
    for (const settle of pending.values()) settle({ ok: false, error, ms: 0 });
    pending.clear();
  };

  return (files, opts = {}) => {
    if (!worker) {
      // new Worker throws (a SecurityError, say) rather than firing onerror
      // when the page may not start one; that is a failed build, not a throw.
      try {
        worker = spawn();
      } catch (e) {
        const why = e instanceof Error ? e.message : String(e);
        return Promise.resolve({ ok: false, error: `the scene build worker could not start: ${why}`, ms: 0 });
      }
      if (!worker) {
        return Promise.resolve({ ok: false, error: "this browser cannot run web workers", ms: 0 });
      }
      worker.onmessage = (ev) => {
        pending.get(ev.data.id)?.(ev.data.result);
        pending.delete(ev.data.id);
      };
      worker.onerror = () => fail("the scene build worker failed to start");
    }
    const id = nextId++;
    const inputs: [string, Uint8Array][] = [];
    for (const f of files) if (isBuildInput(f.file)) inputs.push([f.file, f.content]);
    const running = worker;
    return new Promise<SceneBuildResult>((resolve) => {
      const timer = setTimeout(
        () => fail(`the scene build did not finish within ${Math.round(timeoutMs / 1000)} s`),
        timeoutMs,
      );
      pending.set(id, (result) => {
        clearTimeout(timer);
        resolve(result);
      });
      running.postMessage({ id, wasmUrl, files: inputs, options: opts });
    });
  };
}

let shared: SceneBuilder | null = null;

/** The page-wide builder; its worker and compiled module outlive one build. */
export const buildSceneInWorker: SceneBuilder = (files, opts) => {
  shared ??= createSceneBuilder();
  return shared(files, opts);
};

/** Read the files a build needs from a project folder, skipping assets and installs. */
export async function readProjectBuildInputs(
  dir: FileSystemDirectoryHandle,
  prefix = "",
): Promise<SceneBuildFile[]> {
  const out: SceneBuildFile[] = [];
  const entries = (dir as unknown as {
    entries: () => AsyncIterableIterator<[string, FileSystemHandle]>;
  }).entries();
  for await (const [name, handle] of entries) {
    const path = prefix ? `${prefix}/${name}` : name;
    if (handle.kind === "directory") {
      if (!isPrunedDir(name) && !(prefix === "" && name === "bin")) {
        out.push(...(await readProjectBuildInputs(handle as FileSystemDirectoryHandle, path)));
      }
    } else if (isBuildInput(path)) {
      const file = await (handle as FileSystemFileHandle).getFile();
      out.push({ file: path, content: new Uint8Array(await file.arrayBuffer()) });
    }
  }
  return out;
}
