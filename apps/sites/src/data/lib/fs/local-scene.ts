import {
  emptySeed,
  seedFromCompositeJSON,
  type SceneEditorSeed,
} from "../catalyst/creator-hub/scene-editor";
import {
  openDirectoryOutcome,
  openFiles,
  readDirectoryFiles,
  readText,
} from "./disk";
import { activateLocalProjectScope, beginLocalProjectScope, currentProjectScope, type ProjectScope } from "@ui/editor/project-cache-name";

const SEED_KEY = "ch:local-scene-seed";
const COMPOSITE_KEY = "ch:local-scene-composite";
const IDB_DB = "ch-fs";
const IDB_STORE = "handles";
const HANDLE_KEY = "local-composite-handle";

const COMPOSITE_RE = /(^|\/)main\.composite$/i;
const SCENE_JSON_RE = /(^|\/)scene\.json$/i;
const GLB_RE = /\.(glb|gltf)$/i;

export type LocalSceneResult = {
  seed: SceneEditorSeed;
  compositeHandle: FileSystemFileHandle | null;
  compositeText: string;
  source: "directory" | "file";
  files: Record<string, File> | null;
  sceneJsonText: string | null;
};

function safeJson(text: string): unknown {
  try {
    return JSON.parse(text);
  } catch {
    return null;
  }
}

function sceneMeta(text: string | null): {
  title?: string;
  base?: string;
  parcels?: string[];
  template?: string;
} {
  if (!text) return {};
  const j = safeJson(text) as
    | {
        display?: { title?: string };
        scene?: { base?: string; parcels?: string[] };
        tags?: string[];
      }
    | null;
  const tag = Array.isArray(j?.tags) ? j?.tags?.find((t) => typeof t === "string" && t) : undefined;
  return {
    title: j?.display?.title,
    base: j?.scene?.base,
    parcels: j?.scene?.parcels,
    template: tag || undefined,
  };
}

function baseSeedFor(meta: {
  title?: string;
  base?: string;
  parcels?: string[];
  template?: string;
}): SceneEditorSeed {
  const base = meta.base ?? meta.parcels?.[0] ?? "0,0";
  const seed = emptySeed(base);
  seed.scene = {
    ...seed.scene,
    title: meta.title?.trim() || "Local scene",
    base,
    parcels: meta.parcels?.length ? meta.parcels : seed.scene.parcels,
    ...(meta.template ? { template: meta.template } : {}),
  };
  return seed;
}

async function fileHandleAt(
  dir: FileSystemDirectoryHandle,
  path: string,
): Promise<FileSystemFileHandle | null> {
  try {
    const parts = path.split("/");
    let cur = dir;
    for (let i = 0; i < parts.length - 1; i += 1) {
      cur = await cur.getDirectoryHandle(parts[i]);
    }
    return await cur.getFileHandle(parts[parts.length - 1]);
  } catch {
    return null;
  }
}

export const NO_COMPOSITE_HINT =
  "No main.composite found in that folder \u{2014} pick the project folder that contains your saved scene, the one holding main.composite.";
export const NO_COMPOSITE_CODE_ONLY_HINT =
  "No main.composite found in that folder \u{2014} it looks like a code-only SDK project (scene.json but no saved scene). Pick the project folder that contains your saved scene, the one holding main.composite.";
export const OPEN_FAILED_HINT =
  "Couldn't read that folder \u{2014} check that this site still has permission to access it, then try again.";

export async function stageLocalSceneForEditor(res: LocalSceneResult, signal?: AbortSignal): Promise<void> {
  signal?.throwIfAborted();
  const scope = beginLocalProjectScope();
  const { populateProjectRealm } = await import("./project-realm");
  signal?.throwIfAborted();
  if (res.files) {
    const pr = await populateProjectRealm(res.files, res.sceneJsonText, { signal, scope, composite: res.compositeText });
    signal?.throwIfAborted();
    if (!pr.ok) throw new Error(`Could not prepare the local project: ${pr.reason}`);
  } else {
    const pr = await populateProjectRealm({}, res.sceneJsonText, { signal, scope, composite: res.compositeText });
    if (!pr.ok) throw new Error(`Could not prepare the local project: ${pr.reason}`);
  }
  signal?.throwIfAborted();
  await stashCompositeHandle(res.compositeHandle, scope);
  signal?.throwIfAborted();
  stashLocalSeed(res.seed);
  stashLocalComposite(res.compositeText);
  activateLocalProjectScope(scope);
}

/** Import stages a folder before navigating; a saved project opened from the scenes list, a
    reload or a link arrives with an empty realm, so it is staged here from its stored folder. */
export async function stagePersistedLocalProject(slug: string, scope: ProjectScope, signal?: AbortSignal): Promise<void> {
  if (typeof caches !== "undefined" && await (await caches.open(scope.cacheName)).match(`${scope.realmBase}/about`)) return;
  const { handleStore, ensureHandlePermission } = await import("./handle-store");
  const handle = await handleStore.get(slug);
  if (!handle) throw new Error("This project's folder is no longer linked. Open it again with Import Scene.");
  if (!(await ensureHandlePermission(handle, "read"))) throw new Error("Allow access to the project folder to load it.");
  signal?.throwIfAborted();
  const files = await readDirectoryFiles(handle);
  const sjPath = Object.keys(files).find((p) => SCENE_JSON_RE.test(p));
  const sceneJsonText = sjPath ? await readText(files[sjPath]) : null;
  signal?.throwIfAborted();
  const { populateProjectRealm } = await import("./project-realm");
  const pr = await populateProjectRealm(files, sceneJsonText, { signal, scope });
  if (!pr.ok) throw new Error(`Could not prepare the local project: ${pr.reason}`);
}

export type OpenLocalSceneOutcome =
  | { status: "opened"; result: LocalSceneResult }
  | { status: "cancelled" }
  | { status: "no-composite"; fileCount: number; hasSceneJson: boolean };

export type OpenLocalScenePhase = "picking" | "reading";

export async function openLocalScene(opts?: {
  onPhase?: (phase: OpenLocalScenePhase) => void;
}): Promise<OpenLocalSceneOutcome> {
  opts?.onPhase?.("picking");
  const dirOutcome = await openDirectoryOutcome("readwrite");
  if (dirOutcome.status === "cancelled") return { status: "cancelled" };

  if (dirOutcome.status === "opened") {
    const dir = dirOutcome.handle;
    opts?.onPhase?.("reading");
    const files = await readDirectoryFiles(dir);
    const paths = Object.keys(files);
    const compPath = paths.find((p) => COMPOSITE_RE.test(p));
    if (!compPath) {
      return {
        status: "no-composite",
        fileCount: paths.length,
        hasSceneJson: paths.some((p) => SCENE_JSON_RE.test(p)),
      };
    }
    const sjPath = paths.find((p) => SCENE_JSON_RE.test(p));
    const sceneJsonText = sjPath ? await readText(files[sjPath]) : null;
    const meta = sceneMeta(sceneJsonText);
    const compositeText = await readText(files[compPath]);
    const json = safeJson(compositeText);
    const glbs = paths.filter((p) => GLB_RE.test(p));
    const seed = seedFromCompositeJSON(json, baseSeedFor(meta), glbs);
    return {
      status: "opened",
      result: {
        seed,
        compositeHandle: await fileHandleAt(dir, compPath),
        compositeText,
        source: "directory",
        files,
        sceneJsonText,
      },
    };
  }

  const picked = await openFiles({
    accept: { "application/json": [".composite", ".json"] },
    description: "Scene composite (main.composite)",
  });
  const comp = picked.find((p) => COMPOSITE_RE.test(p.name)) ?? picked[0];
  if (!comp) return { status: "cancelled" };
  opts?.onPhase?.("reading");
  const compositeText = await readText(comp.file);
  const json = safeJson(compositeText);
  const seed = seedFromCompositeJSON(
    json,
    baseSeedFor({ title: comp.name.replace(/\.composite$/i, "") }),
  );
  return {
    status: "opened",
    result: {
      seed,
      compositeHandle: comp.handle,
      compositeText,
      source: "file",
      files: null,
      sceneJsonText: null,
    },
  };
}


export function stashLocalSeed(seed: SceneEditorSeed): void {
  if (typeof sessionStorage === "undefined") return;
  try {
    sessionStorage.setItem(SEED_KEY, JSON.stringify(seed));
  } catch {
  }
}

export function readLocalSeed(): SceneEditorSeed | null {
  if (typeof sessionStorage === "undefined") return null;
  try {
    const raw = sessionStorage.getItem(SEED_KEY);
    return raw ? (JSON.parse(raw) as SceneEditorSeed) : null;
  } catch {
    return null;
  }
}

export function clearLocalSeed(): void {
  if (typeof sessionStorage !== "undefined") {
    try {
      sessionStorage.removeItem(SEED_KEY);
    } catch {
    }
  }
}


export function stashLocalComposite(text: string | null | undefined): void {
  if (typeof sessionStorage === "undefined") return;
  try {
    if (typeof text === "string" && text.trim() !== "") {
      sessionStorage.setItem(COMPOSITE_KEY, text);
    } else {
      sessionStorage.removeItem(COMPOSITE_KEY);
    }
  } catch {
  }
}

export function readLocalComposite(): string | null {
  if (typeof sessionStorage === "undefined") return null;
  try {
    return sessionStorage.getItem(COMPOSITE_KEY);
  } catch {
    return null;
  }
}

export function clearLocalComposite(): void {
  if (typeof sessionStorage !== "undefined") {
    try {
      sessionStorage.removeItem(COMPOSITE_KEY);
    } catch {
    }
  }
}


function openIdb(): Promise<IDBDatabase | null> {
  if (typeof indexedDB === "undefined") return Promise.resolve(null);
  return new Promise((resolve) => {
    const req = indexedDB.open(IDB_DB, 1);
    req.onupgradeneeded = () => req.result.createObjectStore(IDB_STORE);
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => resolve(null);
  });
}

export async function stashCompositeHandle(handle: FileSystemFileHandle | null, scope: ProjectScope = currentProjectScope()): Promise<void> {
  const db = await openIdb();
  if (!db) return;
  await new Promise<void>((resolve) => {
    const tx = db.transaction(IDB_STORE, "readwrite");
    const key = `${HANDLE_KEY}:${scope.id}`;
    if (handle) tx.objectStore(IDB_STORE).put(handle, key);
    else tx.objectStore(IDB_STORE).delete(key);
    tx.oncomplete = () => resolve();
    tx.onerror = () => resolve();
  });
}

export async function readCompositeHandle(): Promise<FileSystemFileHandle | null> {
  const scope = currentProjectScope();
  const db = await openIdb();
  if (!db) return null;
  return new Promise((resolve) => {
    const tx = db.transaction(IDB_STORE, "readonly");
    const r = tx.objectStore(IDB_STORE).get(`${HANDLE_KEY}:${scope.id}`);
    r.onsuccess = () => resolve((r.result as FileSystemFileHandle) ?? null);
    r.onerror = () => resolve(null);
  });
}
