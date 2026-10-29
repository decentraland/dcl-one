import { buildScaffoldFiles } from "../../fs/scaffold-project";
import { SEEDED_ASSET_SRC_RE } from "../../fs/save-scene";
import type { DeployContentFile } from "./deploy-scene";
import { buildSceneInWorker, type SceneBuilder } from "./scene-build";

export const SEEDED_ASSET_DIR = "assets/imported/template-assets";
const GAME_RUNTIME_URL = "/template-bundles/games.js";
const DEFAULT_SCENE_MAIN = "bin/index.js";

/** Where a published scene's `main` comes from when the project has none built. */
export type SceneMainSource =
  | { kind: "built"; main: string; ms: number }
  | { kind: "runtime"; main: string; reason: string };

export function sceneMainNote(source: SceneMainSource): string {
  if (source.kind === "built") {
    return (
      `${source.main} is built from your src/ code in the browser for this publish ` +
      "(the production build dcl-scene-cli runs), so the published scene runs your code."
    );
  }
  return (
    `Your src/ code did not build in the browser (${source.reason}), so this publish ` +
    `ships the prebuilt template runtime as ${source.main} \u{2014} the same one the ` +
    "editor's Play button uses \u{2014} and your code edits do not change what runs. " +
    "Fix the error, or build the project locally (npm install && npm run build) so " +
    `${source.main} exists before publishing.`
  );
}

export const SCENE_BUILDING_NOTE =
  "Building your src/ code in the browser to check what this publish will run\u{2026}";

const ABS_BUILDER_SRC_RE = /^(?:https?:\/\/[^/]+)?\/builder-api\/contents\/([A-Za-z0-9]+)$/;

const IMPORTED_ITEM_SRC_RE =
  /^assets\/imported\/([0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})\/(.+)$/i;

const ASSET_PACKS_URL = "/builder-api/v1/assetPacks";

function extractGltfSrcHolders(
  composite: unknown,
): { obj: Record<string, unknown>; src: string }[] {
  const out: { obj: Record<string, unknown>; src: string }[] = [];
  const components =
    composite && typeof composite === "object"
      ? (composite as { components?: unknown }).components
      : undefined;
  if (!Array.isArray(components)) return out;
  for (const block of components) {
    if (!block || typeof block !== "object") continue;
    const name = (block as { name?: unknown }).name;
    if (typeof name !== "string" || !/GltfContainer$/.test(name)) continue;
    const data = (block as { data?: unknown }).data;
    if (!data || typeof data !== "object") continue;
    for (const entry of Object.values(data as Record<string, unknown>)) {
      const val =
        entry && typeof entry === "object" && "json" in (entry as object)
          ? (entry as { json?: unknown }).json
          : entry;
      if (!val || typeof val !== "object") continue;
      const src = (val as { src?: unknown }).src;
      if (typeof src === "string" && src.trim()) {
        out.push({ obj: val as Record<string, unknown>, src: src.trim() });
      }
    }
  }
  return out;
}

function relativeAssetPresent(fileKeys: readonly string[], src: string): boolean {
  const norm = src.replace(/^\.?\//, "");
  if (!norm) return false;
  const suffix = `/${norm}`;
  return fileKeys.some((k) => k === norm || k.endsWith(suffix));
}

function countEmptyGltfSrcs(composite: unknown): number {
  const components =
    composite && typeof composite === "object"
      ? (composite as { components?: unknown }).components
      : undefined;
  if (!Array.isArray(components)) return 0;
  let empty = 0;
  for (const block of components) {
    if (!block || typeof block !== "object") continue;
    const name = (block as { name?: unknown }).name;
    if (typeof name !== "string" || !/GltfContainer$/.test(name)) continue;
    const data = (block as { data?: unknown }).data;
    if (!data || typeof data !== "object") continue;
    for (const entry of Object.values(data as Record<string, unknown>)) {
      const val =
        entry && typeof entry === "object" && "json" in (entry as object)
          ? (entry as { json?: unknown }).json
          : entry;
      if (!val || typeof val !== "object") continue;
      const src = (val as { src?: unknown }).src;
      if (typeof src !== "string" || !src.trim()) empty += 1;
    }
  }
  return empty;
}

export const EMPTY_SRC_ERROR = (count: number): string =>
  `Cannot publish: ${count} placed item${count === 1 ? "" : "s"} reference no ` +
  `model file (empty GltfContainer src) \u{2014} this usually means the scene was ` +
  `saved while the 3D engine was unavailable. Re-open the scene in the editor, ` +
  `wait for the viewport to load, and Save again before publishing.`;

type PackagedAssets = {
  compositeText: string;
  changed: boolean;
  extra: DeployContentFile[];
  missing: string[];
  emptySrc: number;
};

export async function packageSceneAssets(input: {
  compositeText: string;
  fileKeys: readonly string[];
  fetchImpl?: typeof fetch;
}): Promise<PackagedAssets> {
  const doFetch = input.fetchImpl ?? fetch;
  const none: PackagedAssets = {
    compositeText: input.compositeText,
    changed: false,
    extra: [],
    missing: [],
    emptySrc: 0,
  };
  let composite: unknown;
  try {
    composite = JSON.parse(input.compositeText);
  } catch {
    return none;
  }
  const emptySrc = countEmptyGltfSrcs(composite);
  const holders = extractGltfSrcHolders(composite);
  if (holders.length === 0) return { ...none, emptySrc };

  const extraByPath = new Map<string, DeployContentFile>();
  const missing: string[] = [];
  const failedCids = new Set<string>();
  let changed = false;

  let importedItems: Map<string, Record<string, string>> | null = null;
  const itemContentsOf = async (
    id: string,
  ): Promise<Record<string, string> | undefined> => {
    if (!importedItems) {
      importedItems = new Map();
      try {
        const res = await doFetch(ASSET_PACKS_URL, { credentials: "omit" });
        if (res.ok) {
          const body = (await res.json()) as {
            data?: { assets?: { id?: string; contents?: Record<string, string> }[] }[];
          };
          for (const pack of body.data ?? []) {
            for (const asset of pack.assets ?? []) {
              if (asset.id && asset.contents) {
                importedItems.set(asset.id.trim(), asset.contents);
              }
            }
          }
        }
      } catch {
      }
    }
    return importedItems.get(id);
  };

  const ensureBuilderFile = async (cid: string, path: string): Promise<boolean> => {
    if (extraByPath.has(path) || relativeAssetPresent(input.fileKeys, path)) return true;
    if (failedCids.has(cid)) return false;
    try {
      const res = await doFetch(`/builder-api/contents/${cid}`, { credentials: "omit" });
      if (!res.ok) {
        failedCids.add(cid);
        return false;
      }
      extraByPath.set(path, {
        file: path,
        content: new Uint8Array(await res.arrayBuffer()),
      });
      return true;
    } catch {
      failedCids.add(cid);
      return false;
    }
  };

  for (const h of holders) {
    const abs = ABS_BUILDER_SRC_RE.exec(h.src);
    if (abs) {
      const cid = abs[1];
      const path = `${SEEDED_ASSET_DIR}/${cid}.glb`;
      if (await ensureBuilderFile(cid, path)) {
        h.obj.src = path;
        changed = true;
      } else {
        missing.push(h.src);
      }
      continue;
    }
    const seeded = SEEDED_ASSET_SRC_RE.exec(h.src);
    if (seeded) {
      if (!(await ensureBuilderFile(seeded[1], h.src))) missing.push(h.src);
      continue;
    }
    const imported = IMPORTED_ITEM_SRC_RE.exec(h.src);
    if (imported) {
      if (relativeAssetPresent(input.fileKeys, h.src)) continue;
      const contents = await itemContentsOf(imported[1]);
      const refKey = contents
        ? Object.keys(contents).find(
            (k) => k.toLowerCase() === imported[2].toLowerCase(),
          )
        : undefined;
      if (!contents || !refKey || !(await ensureBuilderFile(contents[refKey], h.src))) {
        missing.push(h.src);
        continue;
      }
      const dir = h.src.slice(0, h.src.length - imported[2].length);
      for (const [file, hash] of Object.entries(contents)) {
        if (file !== refKey) await ensureBuilderFile(hash, dir + file);
      }
      continue;
    }
    if (/^https?:\/\//i.test(h.src)) continue;
    if (h.src.startsWith("/") || !relativeAssetPresent(input.fileKeys, h.src)) {
      missing.push(h.src);
    }
  }

  let compositeText = input.compositeText;
  if (changed) {
    try {
      compositeText = JSON.stringify(composite);
    } catch {
      compositeText = input.compositeText;
      changed = false;
    }
  }
  return {
    compositeText,
    changed,
    extra: Array.from(extraByPath.values()),
    missing: Array.from(new Set(missing)),
    emptySrc,
  };
}

export async function fetchGameRuntime(
  fetchImpl?: typeof fetch,
): Promise<Uint8Array | null> {
  try {
    const doFetch = fetchImpl ?? fetch;
    const res = await doFetch(GAME_RUNTIME_URL, { credentials: "omit" });
    if (!res.ok) return null;
    return new Uint8Array(await res.arrayBuffer());
  } catch {
    return null;
  }
}

/**
 * The production scene build (dcl-scene-cli's, compiled to wasm, in a worker)
 * over a project's files: every file it writes (`main`, its chunks,
 * main.crdt), or why there is no usable `main`.
 */
export async function buildSceneMain(
  files: Iterable<DeployContentFile>,
  main: string,
  build: SceneBuilder = buildSceneInWorker,
): Promise<{ ok: true; files: Record<string, Uint8Array>; ms: number } | { ok: false; reason: string }> {
  const result = await build(files, { production: true });
  if (!result.ok) return { ok: false, reason: result.error };
  if (result.report.unresolved.length > 0) {
    return { ok: false, reason: `unresolved imports: ${result.report.unresolved.join("; ")}` };
  }
  // dcl-scene-cli covers these composites through its node data-layer, which a
  // browser lacks: fresh code beside a stale (or no) main.crdt is not this
  // scene, so it is a failed build too.
  const { mainCrdt } = result.report;
  if (typeof mainCrdt === "object" && "unsupported" in mainCrdt) {
    return { ok: false, reason: `main.crdt was not regenerated: ${mainCrdt.unsupported}` };
  }
  if (!result.files[main]) {
    return { ok: false, reason: `the build wrote ${result.report.main}, not ${main}` };
  }
  return { ok: true, files: result.files, ms: result.ms };
}

/**
 * Give a project that has no built `main` one: `buildSceneMain`'s files, or,
 * when the build fails, the prebuilt template runtime (unless `requireBuild`).
 * The returned source says which, for the publish page to show.
 */
export async function provideSceneMain(
  byPath: Map<string, Uint8Array>,
  main: string,
  opts: { build?: SceneBuilder; fetchImpl?: typeof fetch; requireBuild?: boolean } = {},
): Promise<SceneMainSource> {
  const files = Array.from(byPath, ([file, content]) => ({ file, content }));
  const built = await buildSceneMain(files, main, opts.build);
  if (built.ok) {
    for (const [file, content] of Object.entries(built.files)) byPath.set(file, content);
    return { kind: "built", main, ms: built.ms };
  }
  const { reason } = built;
  if (opts.requireBuild) {
    throw new Error(`Cannot publish: your src/ code did not build (${reason}).`);
  }
  const runtime = await fetchGameRuntime(opts.fetchImpl);
  if (!runtime) {
    throw new Error(
      `This project has no built ${main}, it did not build in the browser (${reason}) and ` +
        "the prebuilt scene runtime is unavailable \u{2014} build the project locally " +
        "(npm install && npm run build) before publishing.",
    );
  }
  byPath.set(main, runtime);
  return { kind: "runtime", main, reason };
}

export function sceneMainOf(metadata: unknown): string {
  const main =
    metadata && typeof metadata === "object"
      ? (metadata as { main?: unknown }).main
      : undefined;
  return typeof main === "string" && main.trim() ? main.trim() : DEFAULT_SCENE_MAIN;
}

type DraftProjectMeta = {
  title: string;
  base?: string;
  template?: string;
  composite?: string;
  codeFiles?: Record<string, string>;
};

export type DraftPackage = {
  files: DeployContentFile[];
  metadata: Record<string, unknown>;
  /** Null when the project already had its `main`. */
  mainSource: SceneMainSource | null;
  missing: string[];
  emptySrc: number;
};

const utf8 = (text: string) => new TextEncoder().encode(text);

export async function buildDraftDeployFiles(
  meta: DraftProjectMeta,
  opts: { fetchImpl?: typeof fetch; build?: SceneBuilder } = {},
): Promise<DraftPackage> {
  const scaffold = buildScaffoldFiles({
    name: meta.title,
    template: meta.template,
    parcels: [meta.base?.trim() || "0,0"],
  });
  const byPath = new Map<string, Uint8Array>();
  for (const f of scaffold) byPath.set(f.path, utf8(f.text));
  if (meta.composite?.trim()) byPath.set("main.composite", utf8(meta.composite));
  for (const [path, text] of Object.entries(meta.codeFiles ?? {})) {
    if (typeof text === "string") byPath.set(path, utf8(text));
  }

  const sceneJsonBytes = byPath.get("scene.json");
  const metadata = JSON.parse(
    new TextDecoder().decode(sceneJsonBytes ?? utf8("{}")),
  ) as Record<string, unknown>;

  const compositeBytes = byPath.get("main.composite");
  let missing: string[] = [];
  let emptySrc = 0;
  if (compositeBytes) {
    const packaged = await packageSceneAssets({
      compositeText: new TextDecoder().decode(compositeBytes),
      fileKeys: Array.from(byPath.keys()),
      fetchImpl: opts.fetchImpl,
    });
    if (packaged.changed) byPath.set("main.composite", utf8(packaged.compositeText));
    for (const f of packaged.extra) byPath.set(f.file, f.content);
    missing = packaged.missing;
    emptySrc = packaged.emptySrc;
  }

  const main = sceneMainOf(metadata);
  const mainSource = byPath.has(main) ? null : await provideSceneMain(byPath, main, opts);

  return {
    files: Array.from(byPath, ([file, content]) => ({ file, content })),
    metadata,
    mainSource,
    missing,
    emptySrc,
  };
}
