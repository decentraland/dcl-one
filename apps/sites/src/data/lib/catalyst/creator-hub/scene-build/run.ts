import { MemFs, Wasi } from "./wasi";

// One scene build: the project's files go into a fresh in-memory tree under
// /scene-root, the scene-build wasm runs over it as a WASI command, and the files
// its JSON report names come back out. Used by the worker and, under node, by
// the parity test; nothing here touches the DOM.

// Distinctive on purpose: a development build names it in its sourcemap,
// and the parity test masks it there.
export const SCENE_ROOT = "scene-root";

export type SceneBuildOptions = { production?: boolean; ignoreComposite?: boolean };

/** The wasm's stdout report (library/dcl-scene-build `Report::to_json`). */
export type SceneBuildReport = {
  ok: true;
  main: string;
  outputs: string[];
  smartItems: boolean;
  maxCompositeEntity: number;
  mainCrdt: "skipped" | { native: number } | { unsupported: string };
  unresolved: string[];
  warnings: string[];
};

export type SceneBuildResult =
  | { ok: true; report: SceneBuildReport; files: Record<string, Uint8Array>; ms: number }
  | { ok: false; error: string; ms: number };

/** One message to the worker; its reply is `{ id, result: SceneBuildResult }`. */
export type SceneBuildRequest = {
  id: number;
  wasmUrl: string;
  files: [string, Uint8Array][];
  options: SceneBuildOptions;
};

export async function compileSceneBuild(
  wasmUrl: string,
  fetchImpl: typeof fetch = fetch,
): Promise<WebAssembly.Module> {
  const res = await fetchImpl(wasmUrl, { credentials: "omit" });
  if (!res.ok) throw new Error(`the in-browser scene builder is not available here (${wasmUrl}: HTTP ${res.status})`);
  return WebAssembly.compile(await res.arrayBuffer());
}

// What a scene build can read: module sources, JSON (scene.json, tsconfig,
// imported data) and composites. Everything else (models, textures, audio) is
// referenced by path at run time, so it stays out of the worker's memory.
const INPUT_EXT = /\.(?:[cm]?[jt]s|[jt]sx|json|composite)$/i;

/** Not a build's own output (top-level bin/), an install (node_modules) or tooling state (dot directories). */
export function isBuildInput(path: string): boolean {
  if (!INPUT_EXT.test(path) || path.startsWith("bin/")) return false;
  return !path
    .split("/")
    .slice(0, -1)
    .some((dir) => isPrunedDir(dir));
}

export function isPrunedDir(name: string): boolean {
  return name === "node_modules" || name.startsWith(".");
}

export async function runSceneBuild(
  wasm: WebAssembly.Module,
  files: Iterable<[string, Uint8Array]>,
  opts: SceneBuildOptions = {},
): Promise<SceneBuildResult> {
  const started = performance.now();
  const elapsed = () => Math.round(performance.now() - started);
  const fs = new MemFs();
  for (const [path, bytes] of files) {
    if (isBuildInput(path)) fs.writeFile(`${SCENE_ROOT}/${path}`, bytes);
  }
  const args = ["scene-build", `/${SCENE_ROOT}`];
  if (opts.production) args.push("--production");
  if (opts.ignoreComposite) args.push("--ignore-composite");
  const wasi = new Wasi({ args, fs });
  let status: number;
  try {
    const instance = await WebAssembly.instantiate(wasm, {
      wasi_snapshot_preview1: wasi.imports,
    });
    status = wasi.start(instance);
  } catch (e) {
    const trace = wasi.stderr().trim();
    return {
      ok: false,
      error: `the scene build crashed: ${e instanceof Error ? e.message : String(e)}${trace ? `\n${trace}` : ""}`,
      ms: elapsed(),
    };
  }
  type Printed = SceneBuildReport | { ok: false; error: string };
  let parsed: Printed | null;
  try {
    parsed = JSON.parse(wasi.stdout()) as Printed;
  } catch {
    parsed = null;
  }
  if (!parsed || status !== 0 || !parsed.ok) {
    const error =
      parsed && !parsed.ok
        ? parsed.error
        : `the scene build exited with status ${status}: ${wasi.stderr().trim() || "no report"}`;
    return { ok: false, error, ms: elapsed() };
  }
  const out: Record<string, Uint8Array> = {};
  for (const rel of parsed.outputs) {
    const bytes = fs.readFile(`${SCENE_ROOT}/${rel}`);
    if (!bytes) return { ok: false, error: `the scene build reported ${rel} but did not write it`, ms: elapsed() };
    out[rel] = bytes.slice();
  }
  return { ok: true, report: parsed, files: out, ms: elapsed() };
}
