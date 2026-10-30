import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative } from "node:path";

import { describe, expect, it, vi } from "vitest";

import { createSceneBuilder, isBuildInput, readProjectBuildInputs } from "./index";
import { isPrunedDir, runSceneBuild, SCENE_ROOT, type SceneBuildRequest, type SceneBuildResult } from "./run";

class FakeWorker {
  onmessage: ((ev: MessageEvent<{ id: number; result: SceneBuildResult }>) => void) | null = null;
  onerror: ((ev: Event) => void) | null = null;
  posted: SceneBuildRequest[] = [];
  terminated = false;
  postMessage(msg: SceneBuildRequest) {
    this.posted.push(msg);
  }
  terminate() {
    this.terminated = true;
  }
  reply(id: number, result: SceneBuildResult) {
    this.onmessage?.({ data: { id, result } } as MessageEvent<{ id: number; result: SceneBuildResult }>);
  }
}

const utf8 = (s: string) => new TextEncoder().encode(s);

describe("isBuildInput", () => {
  it("keeps sources, JSON and composites; drops assets, installs, tooling state and bin/", () => {
    for (const p of ["scene.json", "tsconfig.json", "src/index.ts", "src/ui.tsx", "src/x.mjs", "main.composite", "assets/scene/main.composite", "src/bin/helper.ts"]) {
      expect(isBuildInput(p), p).toBe(true);
    }
    for (const p of ["models/a.glb", "images/a.png", "main.crdt", "bin/index.js", "node_modules/@dcl/sdk/package.json", "lib/node_modules/x/index.js", ".dcl-one/entrypoint.ts", ".git/config.json"]) {
      expect(isBuildInput(p), p).toBe(false);
    }
  });
});

// A FileSystemDirectoryHandle over a nested object: strings are files.
type Tree = { [name: string]: string | Tree };
function folder(tree: Tree): FileSystemDirectoryHandle {
  return {
    kind: "directory",
    async *entries() {
      for (const [name, v] of Object.entries(tree)) {
        yield [
          name,
          typeof v === "string"
            ? { kind: "file", getFile: async () => new Blob([v]) }
            : folder(v),
        ];
      }
    },
  } as unknown as FileSystemDirectoryHandle;
}

describe("readProjectBuildInputs", () => {
  it("reads only what a build can use, without descending into installs, tooling state or bin/", async () => {
    const files = await readProjectBuildInputs(
      folder({
        "scene.json": "{}",
        src: { "index.ts": "export {}", bin: { "tool.ts": "" } },
        bin: { "index.js": "old" },
        node_modules: { "@dcl": { sdk: { "package.json": "{}" } } },
        ".dcl-one": { "entrypoint.ts": "" },
        models: { "a.glb": "glb" },
        "main.composite": "{}",
      }),
    );
    expect(files.map((f) => f.file).sort()).toEqual(["main.composite", "scene.json", "src/bin/tool.ts", "src/index.ts"]);
    expect(new TextDecoder().decode(files.find((f) => f.file === "src/index.ts")!.content)).toBe("export {}");
  });
});

describe("createSceneBuilder", () => {
  it("posts only build inputs and settles each build by its id", async () => {
    const worker = new FakeWorker();
    const build = createSceneBuilder({ wasmUrl: "/w.wasm", spawn: () => worker });
    const files = [
      { file: "scene.json", content: utf8("{}") },
      { file: "models/big.glb", content: new Uint8Array(8) },
      { file: "src/index.ts", content: utf8("export {}") },
    ];
    const first = build(files, { production: true });
    const second = build(files);
    expect(worker.posted.map((m) => m.id)).toEqual([1, 2]);
    expect(worker.posted[0].wasmUrl).toBe("/w.wasm");
    expect(worker.posted[0].options).toEqual({ production: true });
    expect(worker.posted[0].files.map(([p]) => p)).toEqual(["scene.json", "src/index.ts"]);
    worker.reply(2, { ok: false, error: "second", ms: 1 });
    worker.reply(1, { ok: false, error: "first", ms: 1 });
    expect(await first).toMatchObject({ error: "first" });
    expect(await second).toMatchObject({ error: "second" });
  });

  it("gives up on a build that outlives the timeout and starts a fresh worker next time", async () => {
    vi.useFakeTimers();
    try {
      const workers: FakeWorker[] = [];
      const build = createSceneBuilder({
        timeoutMs: 1000,
        spawn: () => {
          workers.push(new FakeWorker());
          return workers[workers.length - 1];
        },
      });
      const hung = build([]);
      vi.advanceTimersByTime(1000);
      expect(await hung).toMatchObject({ ok: false, error: expect.stringMatching(/did not finish/) });
      expect(workers[0].terminated).toBe(true);
      void build([]);
      expect(workers).toHaveLength(2);
    } finally {
      vi.useRealTimers();
    }
  });

  it("fails pending builds when the worker errors, and reports a browser without workers", async () => {
    const worker = new FakeWorker();
    const build = createSceneBuilder({ spawn: () => worker });
    const pending = build([]);
    worker.onerror?.(new Event("error"));
    expect(await pending).toMatchObject({ ok: false, error: expect.stringMatching(/worker/) });
    expect(await createSceneBuilder({ spawn: () => null })([])).toMatchObject({
      ok: false,
      error: expect.stringMatching(/web workers/),
    });
  });

  it("settles a build whose worker cannot be constructed, and tries again on the next build", async () => {
    let attempts = 0;
    const worker = new FakeWorker();
    const build = createSceneBuilder({
      spawn: () => {
        attempts += 1;
        if (attempts === 1) throw new Error("blocked");
        return worker;
      },
    });
    expect(await build([])).toEqual({ ok: false, error: "the scene build worker could not start: blocked", ms: 0 });
    const next = build([]);
    expect(attempts).toBe(2);
    worker.reply(1, { ok: false, error: "built", ms: 1 });
    expect(await next).toMatchObject({ error: "built" });
  });
});

// The wasm under this shim against a scene dcl-scene-cli built natively. Needs
// the module (deploy/nix/scene-build-wasm.nix, or the cargo command in the
// crate docs) and the built scene:
//   SCENE_BUILD_WASM=<scene-build.wasm> SCENE_BUILD_SCENE=<scene after
//   `dcl-scene-cli init --node-modules-only` and
//   `dcl-scene-cli build --built-in --production`> pnpm vitest run scene-build
// SCENE_BUILD_PRODUCTION=0 for a scene built without --production. CI's
// scene-build job runs it over the crate's vendored sdk7-scene-template and
// fails if it skipped.
const wasmPath = process.env.SCENE_BUILD_WASM;
const sceneDir = process.env.SCENE_BUILD_SCENE;

function sources(root: string, dir = root, out: [string, Uint8Array][] = []): [string, Uint8Array][] {
  for (const name of readdirSync(dir)) {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) {
      if (!isPrunedDir(name)) sources(root, path, out);
    } else {
      out.push([relative(root, path), new Uint8Array(readFileSync(path))]);
    }
  }
  return out;
}

const INLINE_MAP = "\n//# sourceMappingURL=data:application/json;charset=utf-8;base64,";

function masked(bytes: Uint8Array, root: string): string {
  let text = new TextDecoder().decode(bytes);
  const at = text.lastIndexOf(INLINE_MAP);
  if (at >= 0) {
    const map = Buffer.from(text.slice(at + INLINE_MAP.length).trim(), "base64").toString("utf8");
    text = `${text.slice(0, at)}\n//# sourceMappingURL (decoded) ${map}`;
  }
  return text.split(root).join("<root>");
}

describe.skipIf(!wasmPath || !sceneDir)("the scene-build wasm under this shim", () => {
  it("rebuilds a natively built scene to the same bytes", async () => {
    const wasm = await WebAssembly.compile(readFileSync(wasmPath!));
    const production = process.env.SCENE_BUILD_PRODUCTION !== "0";
    const result = await runSceneBuild(wasm, sources(sceneDir!), { production });
    if (!result.ok) throw new Error(result.error);
    expect(result.report.unresolved).toEqual([]);
    const nativeBin = readdirSync(join(sceneDir!, "bin")).map((n) => `bin/${n}`).sort();
    expect(Object.keys(result.files).filter((p) => p.startsWith("bin/")).sort()).toEqual(nativeBin);
    for (const [rel, bytes] of Object.entries(result.files)) {
      const native = new Uint8Array(readFileSync(join(sceneDir!, rel)));
      expect(masked(bytes, `/${SCENE_ROOT}`), rel).toBe(masked(native, sceneDir!));
    }
  });
});
