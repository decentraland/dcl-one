import { readFileSync } from "node:fs";

import { describe, expect, it } from "vitest";

import { TEMPLATE_COMPOSITE_IDS } from "../../fs/template-composites";
import {
  buildDraftDeployFiles,
  packageSceneAssets,
  provideSceneMain,
  sceneMainNote,
  sceneMainOf,
  EMPTY_SRC_ERROR,
  SEEDED_ASSET_DIR,
} from "./deploy-packaging";
import type { SceneBuilder, SceneBuildReport } from "./scene-build";
import { runSceneBuild } from "./scene-build/run";

const CID = "QmWM8PmLyebMayuY3YkKLX9mZoAAWb9kk69hueAyTwfkvq";

const GLB_BYTES = new Uint8Array([1, 2, 3, 4]);
const RUNTIME_BYTES = new TextEncoder().encode("game-runtime");

function stubFetch(handlers: Record<string, Uint8Array>): typeof fetch {
  return (async (input: RequestInfo | URL) => {
    const url = String(input);
    for (const [prefix, bytes] of Object.entries(handlers)) {
      if (url.startsWith(prefix)) {
        return new Response(bytes.slice().buffer as ArrayBuffer, { status: 200 });
      }
    }
    return new Response(null, { status: 404 });
  }) as typeof fetch;
}

function compositeWith(srcs: string[]): string {
  return JSON.stringify({
    version: 1,
    components: [
      {
        name: "core::GltfContainer",
        data: Object.fromEntries(srcs.map((src, i) => [String(513 + i), { json: { src } }])),
      },
    ],
  });
}

describe("packageSceneAssets", () => {
  it("rewrites absolute builder srcs to seeded relative paths and fetches seeded srcs missing from disk", async () => {
    const seeded = `${SEEDED_ASSET_DIR}/${CID}.glb`;
    const absolute = await packageSceneAssets({
      compositeText: compositeWith([`https://catalyst.example.org/builder-api/contents/${CID}`]),
      fileKeys: ["scene.json"],
      fetchImpl: stubFetch({ "/builder-api/contents/": GLB_BYTES }),
    });
    expect(absolute.changed).toBe(true);
    expect(absolute.missing).toEqual([]);
    expect(absolute.extra.map((f) => f.file)).toEqual([seeded]);
    expect(absolute.compositeText).toContain(seeded);
    expect(absolute.compositeText).not.toContain("https://");

    const relative = await packageSceneAssets({
      compositeText: compositeWith([seeded]),
      fileKeys: ["scene.json"],
      fetchImpl: stubFetch({ "/builder-api/contents/": GLB_BYTES }),
    });
    expect(relative.changed).toBe(false);
    expect(relative.missing).toEqual([]);
    expect(relative.extra.map((f) => f.file)).toEqual([seeded]);
  });

  it("leaves present files alone and flags unknown relative srcs and unfetchable builder assets as missing", async () => {
    const seeded = `${SEEDED_ASSET_DIR}/${CID}.glb`;
    const present = await packageSceneAssets({
      compositeText: compositeWith([seeded, "models/custom.glb"]),
      fileKeys: ["scene.json", seeded],
      fetchImpl: stubFetch({}),
    });
    expect(present.extra).toEqual([]);
    expect(present.missing).toEqual(["models/custom.glb"]);

    const unfetchable = await packageSceneAssets({
      compositeText: compositeWith([`/builder-api/contents/${CID}`]),
      fileKeys: [],
      fetchImpl: stubFetch({}),
    });
    expect(unfetchable.changed).toBe(false);
    expect(unfetchable.missing).toEqual([`/builder-api/contents/${CID}`]);
  });

  it("resolves imported catalog items through the asset-packs catalog, flags unknown ones, and skips ones already on disk", async () => {
    const itemId = "a2f47727-3f6c-4313-ae76-8034fafa2e5b";
    const src = `assets/imported/${itemId}/pebbles.glb`;
    const packs = new TextEncoder().encode(
      JSON.stringify({
        data: [
          {
            assets: [
              { id: itemId, contents: { "pebbles.glb": CID, "thumbnail.png": `${CID}t` } },
            ],
          },
        ],
      }),
    );
    const resolved = await packageSceneAssets({
      compositeText: compositeWith([src]),
      fileKeys: ["scene.json"],
      fetchImpl: stubFetch({ "/builder-api/v1/assetPacks": packs, "/builder-api/contents/": GLB_BYTES }),
    });
    expect(resolved.changed).toBe(false);
    expect(resolved.missing).toEqual([]);
    expect(resolved.extra.map((f) => f.file).sort()).toEqual([
      src,
      `assets/imported/${itemId}/thumbnail.png`,
    ]);

    const unknown = await packageSceneAssets({
      compositeText: compositeWith([src]),
      fileKeys: ["scene.json"],
      fetchImpl: stubFetch({
        "/builder-api/v1/assetPacks": new TextEncoder().encode('{"data":[]}'),
        "/builder-api/contents/": GLB_BYTES,
      }),
    });
    expect(unknown.missing).toEqual([src]);

    const onDisk = await packageSceneAssets({
      compositeText: compositeWith([src]),
      fileKeys: ["scene.json", src],
      fetchImpl: stubFetch({}),
    });
    expect(onDisk.extra).toEqual([]);
    expect(onDisk.missing).toEqual([]);
  });

  it("counts empty GltfContainer srcs so the deploy gate can reject stripped composites, zero for a healthy one", async () => {
    const stripped = await packageSceneAssets({
      compositeText: compositeWith(["", "  ", "models/real.glb"]),
      fileKeys: ["scene.json", "models/real.glb"],
      fetchImpl: stubFetch({}),
    });
    expect(stripped.emptySrc).toBe(2);
    expect(stripped.missing).toEqual([]);
    expect(EMPTY_SRC_ERROR(2)).toContain("2 placed items");
    expect(EMPTY_SRC_ERROR(2)).toContain("empty GltfContainer src");
    const healthy = await packageSceneAssets({
      compositeText: compositeWith(["models/real.glb"]),
      fileKeys: ["models/real.glb"],
      fetchImpl: stubFetch({}),
    });
    expect(healthy.emptySrc).toBe(0);
  });
});

const noBuild: SceneBuilder = async () => ({ ok: false, error: "no scene builder here", ms: 0 });

function report(over: Partial<SceneBuildReport> = {}): SceneBuildReport {
  return {
    ok: true,
    main: "bin/index.js",
    outputs: [],
    smartItems: false,
    maxCompositeEntity: 512,
    mainCrdt: "skipped",
    unresolved: [],
    warnings: [],
    ...over,
  };
}

describe("buildDraftDeployFiles", () => {
  const fetchImpl = stubFetch({
    "/builder-api/contents/": GLB_BYTES,
    "/template-bundles/games.js": RUNTIME_BYTES,
  });

  it("packages a template draft with composite, code edits, assets and the game runtime; sceneMainOf falls back to bin/index.js", async () => {
    const pack = await buildDraftDeployFiles(
      {
        title: "My Tower",
        base: "0,0",
        template: "tower-defense",
        composite: compositeWith([`/builder-api/contents/${CID}`]),
        codeFiles: { "src/index.ts": "export function main() {}\n" },
      },
      { fetchImpl, build: noBuild },
    );
    expect(pack.missing).toEqual([]);
    expect(pack.mainSource).toEqual({ kind: "runtime", main: "bin/index.js", reason: "no scene builder here" });
    const names = pack.files.map((f) => f.file);
    expect(names).toContain("scene.json");
    expect(names).toContain("main.composite");
    expect(names).toContain("src/index.ts");
    expect(names).toContain("bin/index.js");
    expect(names).toContain(`${SEEDED_ASSET_DIR}/${CID}.glb`);
    const idx = pack.files.find((f) => f.file === "src/index.ts")!;
    expect(new TextDecoder().decode(idx.content)).toBe("export function main() {}\n");
    const comp = pack.files.find((f) => f.file === "main.composite")!;
    expect(new TextDecoder().decode(comp.content)).toContain(SEEDED_ASSET_DIR);
    expect(pack.metadata.main).toBe("bin/index.js");
    const scene = pack.metadata.scene as { parcels?: string[]; base?: string };
    expect(scene.parcels).toEqual(["0,0"]);
    expect((pack.metadata.tags as string[])[0]).toBe("tower-defense");
    expect(sceneMainOf({})).toBe("bin/index.js");
    expect(sceneMainOf({ main: "bin/game.js" })).toBe("bin/game.js");
  });

  it("packages an empty draft with the idle runtime and no template tag, and throws when the runtime cannot be fetched", async () => {
    const pack = await buildDraftDeployFiles({ title: "Blank" }, { fetchImpl, build: noBuild });
    expect(pack.mainSource?.kind).toBe("runtime");
    expect(pack.metadata.tags).toEqual([]);
    const bin = pack.files.find((f) => f.file === "bin/index.js")!;
    expect(new TextDecoder().decode(bin.content)).toBe("game-runtime");
    await expect(
      buildDraftDeployFiles({ title: "Blank" }, { fetchImpl: stubFetch({}), build: noBuild }),
    ).rejects.toThrow(/runtime/i);
  });

  it("publishes what the in-browser build writes from the draft's sources", async () => {
    const seen: { files: string[]; production?: boolean }[] = [];
    const build: SceneBuilder = async (files, opts) => {
      seen.push({ files: Array.from(files, (f) => f.file), production: opts?.production });
      return {
        ok: true,
        report: report({ outputs: ["bin/index.js", "bin/scene.js", "bin/sdk-runtime.js", "main.crdt"] }),
        files: {
          "bin/index.js": utf8("loader"),
          "bin/scene.js": utf8("scene"),
          "bin/sdk-runtime.js": utf8("sdk"),
          "main.crdt": new Uint8Array([7]),
        },
        ms: 90,
      };
    };
    const pack = await buildDraftDeployFiles(
      { title: "Mine", codeFiles: { "src/index.ts": "export function main() {}\n" } },
      { fetchImpl, build },
    );
    expect(seen).toHaveLength(1);
    expect(seen[0].production).toBe(true);
    expect(seen[0].files).toEqual(expect.arrayContaining(["scene.json", "tsconfig.json", "src/index.ts", "main.composite"]));
    expect(pack.mainSource).toEqual({ kind: "built", main: "bin/index.js", ms: 90 });
    const text = (file: string) => new TextDecoder().decode(pack.files.find((f) => f.file === file)!.content);
    expect(text("bin/index.js")).toBe("loader");
    expect(text("bin/scene.js")).toBe("scene");
    expect(pack.files.map((f) => f.file)).toContain("main.crdt");
    expect(sceneMainNote(pack.mainSource!)).toMatch(/built from your src\/ code/);
  });
});

const utf8 = (s: string) => new TextEncoder().encode(s);

describe("provideSceneMain", () => {
  const fetchImpl = stubFetch({ "/template-bundles/games.js": RUNTIME_BYTES });

  it("falls back to the runtime, with the reason, when the build fails or leaves imports unresolved", async () => {
    const unresolved: SceneBuilder = async () => ({
      ok: true,
      report: report({ outputs: ["bin/index.js"], unresolved: ["src/index.ts: cannot resolve 'left-pad'"] }),
      files: { "bin/index.js": utf8("half built") },
      ms: 5,
    });
    const byPath = new Map([["src/index.ts", utf8("import 'left-pad'")]]);
    const source = await provideSceneMain(byPath, "bin/index.js", { build: unresolved, fetchImpl });
    expect(source).toMatchObject({ kind: "runtime", reason: expect.stringMatching(/left-pad/) });
    expect(new TextDecoder().decode(byPath.get("bin/index.js"))).toBe("game-runtime");
    expect(sceneMainNote(source)).toMatch(/did not build in the browser \(unresolved imports: .*left-pad/);

    const wrongMain: SceneBuilder = async () => ({
      ok: true,
      report: report({ main: "bin/game.js", outputs: ["bin/game.js"] }),
      files: { "bin/game.js": utf8("x") },
      ms: 5,
    });
    expect(await provideSceneMain(new Map(), "bin/index.js", { build: wrongMain, fetchImpl })).toMatchObject({
      kind: "runtime",
      reason: "the build wrote bin/game.js, not bin/index.js",
    });
  });

  it("falls back to the runtime when the build could not regenerate main.crdt, keeping the project's own", async () => {
    const noCrdt: SceneBuilder = async () => ({
      ok: true,
      report: report({
        outputs: ["bin/index.js"],
        mainCrdt: { unsupported: "nested composites" },
        warnings: ["main.crdt was not regenerated: nested composites"],
      }),
      files: { "bin/index.js": utf8("fresh code") },
      ms: 5,
    });
    const byPath = new Map([
      ["src/index.ts", utf8("export function main() {}")],
      ["main.crdt", new Uint8Array([1])],
    ]);
    const source = await provideSceneMain(byPath, "bin/index.js", { build: noCrdt, fetchImpl });
    expect(source).toEqual({
      kind: "runtime",
      main: "bin/index.js",
      reason: "main.crdt was not regenerated: nested composites",
    });
    expect(new TextDecoder().decode(byPath.get("bin/index.js"))).toBe("game-runtime");
    expect(byPath.get("main.crdt")).toEqual(new Uint8Array([1]));
    await expect(
      provideSceneMain(new Map(), "bin/index.js", { build: noCrdt, fetchImpl, requireBuild: true }),
    ).rejects.toThrow(/main\.crdt was not regenerated: nested composites/);
  });

  it("refuses to swap in the runtime when the build is required", async () => {
    const byPath = new Map<string, Uint8Array>();
    await expect(
      provideSceneMain(byPath, "bin/index.js", { build: noBuild, fetchImpl, requireBuild: true }),
    ).rejects.toThrow(/did not build \(no scene builder here\)/);
    expect(byPath.size).toBe(0);
  });
});

// Every scaffold the Creator Hub writes, built by the real wasm (see
// scene-build/scene-build.test.ts for SCENE_BUILD_WASM).
const wasmPath = process.env.SCENE_BUILD_WASM;

describe.skipIf(!wasmPath)("drafts built by the scene-build wasm", () => {
  it.each(["empty", ...TEMPLATE_COMPOSITE_IDS])("builds the %s scaffold's own code", async (template) => {
    const wasm = await WebAssembly.compile(readFileSync(wasmPath!));
    const build: SceneBuilder = (files, opts) =>
      runSceneBuild(wasm, Array.from(files, (f) => [f.file, f.content] as [string, Uint8Array]), opts);
    const fetchImpl = stubFetch({ "/builder-api/contents/": GLB_BYTES });
    const pack = await buildDraftDeployFiles({ title: `Scaffold ${template}`, template }, { fetchImpl, build });
    expect(pack.mainSource).toMatchObject({ kind: "built", main: "bin/index.js" });
    const names = pack.files.map((f) => f.file);
    expect(names).toEqual(expect.arrayContaining(["bin/index.js", "bin/scene.js", "bin/sdk-runtime.js", "main.crdt"]));
    const scene = new TextDecoder().decode(pack.files.find((f) => f.file === "bin/scene.js")!.content);
    expect(scene.length).toBeGreaterThan(0);
  });
});
