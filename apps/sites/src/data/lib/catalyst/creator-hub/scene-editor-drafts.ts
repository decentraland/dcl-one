import { emptySeed, seedFromCompositeJSON, type SceneEditorSeed } from "./scene-editor";
import { hasTemplateComposite } from "../../fs/template-composites";
import type { ServerDraft } from "./scene-drafts-client";
import type { PageDefaults } from "@/core/lib/telemetry/page-loader";

export type SceneEditorLoaderData = {
  sdkProjectUrl?: string;
  sid: string;
  step: string | null;
  assignment: PageDefaults;
  seed: SceneEditorSeed;
  viewportSrc: string;
  previewSrc?: string;
  from?: string | null;
  projectSlug?: string;
  rawComposite?: string;
  draftAssets?: Record<string, string>;
  failedToLoadLocal?: boolean;
};

const COMPOSITE_RE = /(^|\/)main\.composite$/i;
const SCENE_JSON_RE = /(^|\/)scene\.json$/i;
const GLB_RE = /\.(glb|gltf)$/i;

function safeJson(text: string): unknown {
  try {
    return JSON.parse(text);
  } catch {
    return null;
  }
}

function baseSeedFromSceneJson(sceneJsonText: string | null): SceneEditorSeed {
  const j = safeJson(sceneJsonText ?? "") as
    | {
        display?: { title?: string };
        scene?: { base?: string; parcels?: string[] };
        tags?: string[];
      }
    | null;
  const base = j?.scene?.base ?? j?.scene?.parcels?.[0] ?? "0,0";
  const seed = emptySeed(base);
  const tag = j?.tags?.[0];
  const template = tag && hasTemplateComposite(tag) ? tag : undefined;
  seed.scene = {
    ...seed.scene,
    title: j?.display?.title?.trim() || "Local scene",
    base,
    parcels: j?.scene?.parcels?.length ? j.scene.parcels : seed.scene.parcels,
    ...(template ? { template } : {}),
  };
  return seed;
}

async function seedFromProjectMeta(
  slug: string,
  fallback: SceneEditorSeed,
): Promise<{
  seed: SceneEditorSeed;
  rawComposite?: string;
  draftAssets?: Record<string, string>;
  failedToLoadLocal?: boolean;
} | null> {
  try {
    const { handleStore } = await import("../../fs/handle-store");
    const meta = await handleStore.getMeta(slug);
    if (!meta) return null;
    const baseSeed: SceneEditorSeed = {
      ...fallback,
      scene: {
        ...fallback.scene,
        title: meta.title || fallback.scene.title,
        base: meta.base || fallback.scene.base,
        ...(meta.template && hasTemplateComposite(meta.template)
          ? { template: meta.template }
          : {}),
      },
    };
    const draftAssets =
      meta.assets && Object.keys(meta.assets).length > 0 ? meta.assets : undefined;
    if (!meta.composite) return { seed: baseSeed, ...(draftAssets ? { draftAssets } : {}) };
    const parsed = safeJson(meta.composite);
    if (parsed === null) {
      return { seed: baseSeed, failedToLoadLocal: true, ...(draftAssets ? { draftAssets } : {}) };
    }
    const seed = seedFromCompositeJSON(parsed, baseSeed, []);
    return { seed, rawComposite: meta.composite, ...(draftAssets ? { draftAssets } : {}) };
  } catch {
    return null;
  }
}

async function seedFromPersistedHandle(
  slug: string,
): Promise<{ seed: SceneEditorSeed; rawComposite?: string; failedToLoadLocal?: boolean } | null> {
  try {
    const { handleStore, ensureHandlePermission } = await import("../../fs/handle-store");
    const handle = await handleStore.get(slug);
    if (!handle) return null;
    if (!(await ensureHandlePermission(handle, "read"))) return null;
    const { readDirectoryFiles, readText } = await import("../../fs/disk");
    const files = await readDirectoryFiles(handle);
    const paths = Object.keys(files);
    const compPath = paths.find((p) => COMPOSITE_RE.test(p));
    if (!compPath) return null;
    const compositeText = await readText(files[compPath]);
    const sjPath = paths.find((p) => SCENE_JSON_RE.test(p));
    const sceneJsonText = sjPath ? await readText(files[sjPath]) : null;
    const glbs = paths.filter((p) => GLB_RE.test(p));
    const parsed = safeJson(compositeText);
    if (parsed === null) {
      return { seed: baseSeedFromSceneJson(sceneJsonText), failedToLoadLocal: true };
    }
    const seed = seedFromCompositeJSON(parsed, baseSeedFromSceneJson(sceneJsonText), glbs);
    return { seed, rawComposite: compositeText };
  } catch {
    return null;
  }
}

async function seedFromServerDraft(
  fallback: SceneEditorSeed,
  pending: Promise<ServerDraft | null>,
): Promise<{
  seed: SceneEditorSeed;
  rawComposite?: string;
  draftAssets?: Record<string, string>;
  serverUpdatedAt: number;
} | null> {
  try {
    const draft = await pending;
    if (!draft) return null;
    const parsed = safeJson(draft.blob.composite);
    if (parsed === null) return null;
    const title = draft.blob.title?.trim() || draft.title?.trim() || fallback.scene.title;
    const baseSeed: SceneEditorSeed = {
      ...fallback,
      scene: {
        ...fallback.scene,
        title,
        base: draft.blob.base || fallback.scene.base,
        ...(draft.blob.template && hasTemplateComposite(draft.blob.template)
          ? { template: draft.blob.template }
          : {}),
      },
    };
    const seed = seedFromCompositeJSON(parsed, baseSeed, []);
    return {
      seed,
      rawComposite: draft.blob.composite,
      serverUpdatedAt: draft.updatedAt,
      ...(draft.blob.assets ? { draftAssets: draft.blob.assets } : {}),
    };
  } catch {
    return null;
  }
}

async function localMetaUpdatedAt(slug: string): Promise<number | null> {
  try {
    const { handleStore } = await import("../../fs/handle-store");
    const meta = await handleStore.getMeta(slug);
    const at = (meta as { updatedAt?: unknown } | null)?.updatedAt;
    return typeof at === "number" && Number.isFinite(at) ? at : null;
  } catch {
    return null;
  }
}

export async function loadSceneEditorDraft(request: Request, basePromise: Promise<SceneEditorLoaderData>) {
  const url = new URL(request.url);
  const draftSlug = url.searchParams.get("draft")?.trim();
  const projectSlug = url.searchParams.get("source") === "local"
    ? url.searchParams.get("project")?.trim() : undefined;
  const drafts = new Map<string, Promise<ServerDraft | null>>();
  for (const slug of [draftSlug, projectSlug]) {
    if (!slug || drafts.has(slug)) continue;
    drafts.set(slug, import("./scene-drafts-client")
      .then(({ fetchServerDraft }) => fetchServerDraft(slug, request.signal)).catch(() => null));
  }
  const base = await basePromise;

  const pickNewest = async (
    slug: string,
    local: {
      seed: SceneEditorSeed;
      rawComposite?: string;
      draftAssets?: Record<string, string>;
      failedToLoadLocal?: boolean;
    } | null,
  ) => {
    const [server, localAt] = await Promise.all([
      seedFromServerDraft(base.seed, drafts.get(slug)!),
      localMetaUpdatedAt(slug),
    ]);
    if (!server) return local;
    if (!local || local.failedToLoadLocal) return server;
    // An unreadable timestamp cannot establish that the server draft is newer.
    return localAt !== null && server.serverUpdatedAt > localAt ? server : local;
  };

  if (draftSlug) {
    const seededDraft = await pickNewest(
      draftSlug,
      await seedFromProjectMeta(draftSlug, base.seed),
    );
    if (seededDraft) return { ...base, ...seededDraft };
  }

  if (url.searchParams.get("source") !== "local") return base;

  if (projectSlug) {
    const local =
      (await seedFromPersistedHandle(projectSlug)) ??
      (await seedFromProjectMeta(projectSlug, base.seed));
    const seeded = await pickNewest(projectSlug, local);
    if (seeded) return { ...base, ...seeded };
  }

  const { readLocalSeed, readLocalComposite } = await import("../../fs/local-scene");
  const seed = readLocalSeed();
  const rawComposite = readLocalComposite() ?? undefined;
  if (seed) return { ...base, seed, rawComposite };
  return base;
}
