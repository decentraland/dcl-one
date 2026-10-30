import { buildTemplateCompositeText, hasTemplateComposite, templateAssetContents } from "./template-composites";
import { currentProjectScope, type ProjectScope } from "@ui/editor/project-cache-name";

// The scene runtime of every browser project: the template games' SDK7 bundle. It runs the
// template's game when the scene metadata names one, and idles otherwise.
const GAME_BUNDLE_URL = "/template-bundles/games.js";

// The pinned engine's service worker serves `<scope.previewRealm>/` from this cache
// (PREVIEW_REALM.md in the pinned Explorer source). It only reads it; the page writes it.
export const PREVIEW_CACHE = "dcl-editor-preview-v1";

// What `sdk-commands init` writes for an empty scene; a project's own scene.json overrides the
// title, parcels and spawn points.
const SCENE_JSON = {
  ecs7: true,
  runtimeVersion: "7",
  display: { title: "My New Scene" },
  owner: "",
  main: "bin/index.js",
  tags: [] as string[],
  scene: { parcels: ["0,0"], base: "0,0" },
  spawnPoints: [{ name: "spawn", default: true, position: { x: [8], y: [0], z: [8] }, cameraTarget: { x: 8, y: 1, z: 8 } }],
  featureToggles: {},
};

export const PLAY_STATE_FILE = "one-play-state";

type TypedIpfsRef = { file: string; hash: string };

export type PopulateResult = {
  ok: boolean;
  reason?: string;
  sceneHash?: string;
  /** The entity id the engine knows the project by (`/reload <id>` reloads it in place). */
  previewEntity?: string;
  assets?: number;
  game?: string;
  templateAssets?: number;
};

export type PopulateOptions = {
  template?: string | null;
  /** The project's main.composite, compiled into the main.crdt the engine starts the scene from. */
  composite?: string | null;
  assets?: Record<string, string> | null;
  signal?: AbortSignal;
  scope?: ProjectScope;
};

function abortError(): Error {
  const error = new Error("Project realm preparation was cancelled");
  error.name = "AbortError";
  return error;
}

function throwIfAborted(signal?: AbortSignal): void {
  if (signal?.aborted) throw abortError();
}

const NON_ASSET_RE =
  /(^|\/)(node_modules|\.git|bin)\/|(^|\/)(main\.composite|main\.crdt|scene\.json|package\.json|package-lock\.json|tsconfig\.json|\.gitignore)$|\.(ts|tsx|md|log)$/i;

function isAssetPath(p: string): boolean {
  return !NON_ASSET_RE.test(p);
}

const MIME: Record<string, string> = {
  glb: "model/gltf-binary",
  gltf: "model/gltf+json",
  bin: "application/octet-stream",
  png: "image/png",
  jpg: "image/jpeg",
  jpeg: "image/jpeg",
  gif: "image/gif",
  webp: "image/webp",
  ktx2: "image/ktx2",
  ktx: "image/ktx",
  basis: "application/octet-stream",
  hdr: "image/vnd.radiance",
  mp3: "audio/mpeg",
  m4a: "audio/mp4",
  wav: "audio/wav",
  ogg: "audio/ogg",
  mp4: "video/mp4",
  json: "application/json",
};

function contentTypeFor(path: string): string {
  const ext = path.split(".").pop()?.toLowerCase() ?? "";
  return MIME[ext] ?? "application/octet-stream";
}

async function tokenFor(buf: ArrayBuffer): Promise<string> {
  const digest = await crypto.subtle.digest("SHA-256", buf);
  const hex = Array.from(new Uint8Array(digest))
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
  return `b64-${hex}`;
}

const PARCEL = /^-?\d+,-?\d+$/;

function sceneMetadata(sceneJsonText?: string | null): Record<string, unknown> {
  const meta: Record<string, unknown> = JSON.parse(JSON.stringify(SCENE_JSON));
  try {
    if (!sceneJsonText) return meta;
    const sj = JSON.parse(sceneJsonText) as {
      display?: { title?: unknown };
      scene?: { parcels?: unknown; base?: unknown };
      spawnPoints?: unknown;
    };
    if (typeof sj.display?.title === "string" && sj.display.title) {
      meta.display = { ...(meta.display as Record<string, unknown>), title: sj.display.title };
    }
    const parcels = sj.scene?.parcels;
    const base = sj.scene?.base;
    if (Array.isArray(parcels) && parcels.length > 0 && parcels.every((p) => typeof p === "string" && PARCEL.test(p))
      && typeof base === "string" && parcels.includes(base)) {
      meta.scene = { parcels, base };
    }
    if (Array.isArray(sj.spawnPoints) && sj.spawnPoints.length > 0) meta.spawnPoints = sj.spawnPoints;
  } catch {
  }
  return meta;
}

function templateFromSceneJson(sceneJsonText?: string | null): string | null {
  try {
    if (!sceneJsonText) return null;
    const sj = JSON.parse(sceneJsonText) as { tags?: unknown };
    const tag = Array.isArray(sj.tags) ? (sj.tags[0] as unknown) : null;
    return typeof tag === "string" && hasTemplateComposite(tag) ? tag : null;
  } catch {
    return null;
  }
}

async function fetchGameBundle(signal?: AbortSignal): Promise<ArrayBuffer | null> {
  try {
    throwIfAborted(signal);
    const res = await fetch(GAME_BUNDLE_URL, { cache: "no-store", signal });
    throwIfAborted(signal);
    if (!res.ok) return null;
    const bundle = await res.arrayBuffer();
    throwIfAborted(signal);
    return bundle;
  } catch (error) {
    if (signal?.aborted) throw error;
    return null;
  }
}

function base64(text: string): string {
  let binary = "";
  for (const byte of new TextEncoder().encode(text)) binary += String.fromCharCode(byte);
  return btoa(binary);
}

// sdk-commands' local ids (PREVIEW_REALM.md "Ids"), which make the engine treat the scene as a
// local one it reloads in place: the project root is the browser project, the machine id this
// runtime of it.
function previewEntityId(scope: ProjectScope): string {
  return `b64-${base64(`/${scope.id}-${scope.runtimeId}`)}`;
}

// The version comes from the file's content hash in the project cache (a SHA-256 token or a
// CID), so an unchanged file keeps its id and is neither read nor copied again.
async function previewContentHash(scope: ProjectScope, file: string, contentHash: string): Promise<string> {
  const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", new TextEncoder().encode(contentHash)));
  let version = 0n;
  for (const byte of digest.subarray(0, 6)) version = (version << 8n) | BigInt(byte);
  return `b64-${base64(`/${scope.id}/${file}\0${version}-${scope.runtimeId}`)}`;
}

/** Write the realm the engine loads: every file under its local hash, then the manifest. */
async function stagePreviewRealm(
  scope: ProjectScope,
  files: { file: string; hash: string; stored: Response }[],
  metadata: Record<string, unknown>,
  signal?: AbortSignal,
): Promise<string> {
  const realm = `${scope.previewRealm}/`;
  const store = await caches.open(PREVIEW_CACHE);
  throwIfAborted(signal);
  const content: TypedIpfsRef[] = [];
  const keep = new Set([`${realm}__manifest`]);
  for (const { file, hash: contentHash, stored } of files) {
    const hash = await previewContentHash(scope, file, contentHash);
    throwIfAborted(signal);
    const key = `${realm}content/contents/${hash}`;
    content.push({ file, hash });
    keep.add(key);
    if (!(await store.match(key))) await store.put(key, stored);
    throwIfAborted(signal);
  }
  const entity = {
    id: previewEntityId(scope),
    type: "scene",
    pointers: (metadata.scene as { parcels: string[] }).parcels,
    timestamp: Date.now(),
    content,
    metadata,
  };
  await store.put(`${realm}__manifest`, Response.json({ version: 1, entity }));
  for (const request of await store.keys()) {
    if (request.url.startsWith(realm) && !keep.has(request.url)) await store.delete(request);
  }
  return entity.id;
}

async function clearPreviewRealm(scope: ProjectScope): Promise<void> {
  const store = await caches.open(PREVIEW_CACHE);
  for (const request of await store.keys()) {
    if (request.url.startsWith(`${scope.previewRealm}/`)) await store.delete(request);
  }
}

export async function clearProjectRealm(signal?: AbortSignal, scope = currentProjectScope()): Promise<void> {
  throwIfAborted(signal);
  if (typeof caches === "undefined") return;
  try {
    const cache = await caches.open(scope.cacheName);
    throwIfAborted(signal);
    for (const key of await cache.keys()) {
      throwIfAborted(signal);
      await cache.delete(key);
      throwIfAborted(signal);
    }
    await clearPreviewRealm(scope);
  } catch (error) {
    if (signal?.aborted) throw error;
  }
}

export async function populateProjectRealm(
  files: Record<string, File> | null,
  sceneJsonText?: string | null,
  opts: PopulateOptions = {},
): Promise<PopulateResult> {
  const { signal, scope = currentProjectScope() } = opts;
  throwIfAborted(signal);
  if (typeof caches === "undefined" || typeof crypto === "undefined" || !crypto.subtle) {
    return { ok: false, reason: "no-cache-or-subtlecrypto" };
  }
  if (!files) return { ok: false, reason: "no-files" };

  const runtimeBuf = await fetchGameBundle(signal);
  throwIfAborted(signal);
  if (!runtimeBuf) return { ok: false, reason: "scene-runtime-unreachable" };
  const runtime = { token: await tokenFor(runtimeBuf), buf: runtimeBuf };
  throwIfAborted(signal);

  const existingCache = await caches.open(scope.cacheName);
  const priorResponse = await existingCache.match(`${scope.contentBase}/entities/active`);
  const priorEntities = priorResponse ? await priorResponse.json() : [];
  const priorContent: TypedIpfsRef[] = Array.isArray(priorEntities?.[0]?.content)
    ? priorEntities[0].content.filter((ref: TypedIpfsRef) => typeof ref.file === "string" && typeof ref.hash === "string" && isAssetPath(ref.file))
    : [];

  const assetPaths = Object.keys(files).filter(isAssetPath);
  const assetRefs: TypedIpfsRef[] = [];
  const assetBytes: { token: string; buf: ArrayBuffer; ct: string }[] = [];
  for (const p of assetPaths) {
    const buf = await files[p].arrayBuffer();
    throwIfAborted(signal);
    const token = await tokenFor(buf);
    throwIfAborted(signal);
    assetRefs.push({ file: p, hash: token });
    assetBytes.push({ token, buf, ct: contentTypeFor(p) });
  }

  const gameTemplate =
    (opts.template && hasTemplateComposite(opts.template) ? opts.template : null) ??
    templateFromSceneJson(sceneJsonText);
  const compositeFile = Object.keys(files).find((p) => /(^|\/)main\.composite$/i.test(p));
  const compositeText = opts.composite
    ?? (compositeFile ? await files[compositeFile].text() : null)
    ?? (gameTemplate ? buildTemplateCompositeText(gameTemplate) : null);
  throwIfAborted(signal);
  let crdt: { token: string; buf: ArrayBuffer } | null = null;
  if (compositeText) {
    const { compositeToMainCrdt } = await import("./composite-crdt");
    throwIfAborted(signal);
    const buf = compositeToMainCrdt(compositeText).slice().buffer as ArrayBuffer;
    crdt = { token: await tokenFor(buf), buf };
    throwIfAborted(signal);
  }

  const fetchFromBuilder = async (cid: string): Promise<ArrayBuffer | null> => {
    try {
      throwIfAborted(signal);
      const cached = await existingCache.match(`${scope.contentBase}/contents/${cid}`);
      const res = cached ?? await fetch(`/builder-api/contents/${cid}`, { credentials: "omit", signal });
      throwIfAborted(signal);
      if (!res.ok) return null;
      const buf = await res.arrayBuffer();
      throwIfAborted(signal);
      return buf;
    } catch (error) {
      if (signal?.aborted) throw error;
      return null;
    }
  };
  // The template's files, then the draft's placed ones; the first of a path that arrives wins.
  // Fetched together: a slow content server times each one out on its own.
  const candidates = [
    ...Object.entries(gameTemplate ? templateAssetContents(gameTemplate) : {}),
    ...Object.entries(opts.assets ?? {}).filter((entry): entry is [string, string] => typeof entry[1] === "string" && !!entry[1]),
  ];
  const fetched = await Promise.all(candidates.map(([, cid]) => fetchFromBuilder(cid)));
  throwIfAborted(signal);
  const templateAssets: { path: string; cid: string; buf: ArrayBuffer }[] = [];
  const taken = new Set(assetPaths.map((p) => p.toLowerCase()));
  candidates.forEach(([path, cid], i) => {
    const buf = fetched[i];
    if (!buf || taken.has(path.toLowerCase())) return;
    templateAssets.push({ path, cid, buf });
    taken.add(path.toLowerCase());
  });

  const content: TypedIpfsRef[] = [...new Map([
    ...priorContent,
    ...assetRefs,
    ...templateAssets.map((a) => ({ file: a.path, hash: a.cid })),
    { file: "bin/index.js", hash: runtime.token },
    ...(crdt ? [{ file: "main.crdt", hash: crdt.token }] : []),
  ].map(ref => [ref.file.toLowerCase(), ref])).values()];

  const metadata = sceneMetadata(sceneJsonText);
  if (gameTemplate) {
    metadata.one_play = { template: gameTemplate, gated: true };
  }
  const pointers = (metadata.scene as { parcels: string[] }).parcels;
  const sceneHash = await tokenFor(
    new TextEncoder().encode(JSON.stringify(content) + "|" + pointers.join(";")).buffer as ArrayBuffer,
  );
  throwIfAborted(signal);

  const entity = {
    id: sceneHash,
    version: "v3",
    type: "scene",
    pointers,
    timestamp: Date.now(),
    content,
    metadata,
  };
  // The project's own record of what it serves; the engine reads the preview realm staged below.
  const about = {
    healthy: true,
    configurations: { realmName: "LocalPreview", scenesUrn: [`urn:decentraland:entity:${sceneHash}?=&baseUrl=${scope.contentBase}/contents/`] },
    content: { healthy: true, publicUrl: scope.contentBase },
  };

  const cache = await caches.open(scope.cacheName);
  throwIfAborted(signal);

  const json = (v: unknown) => JSON.stringify(v);
  const headers = (ct: string) => ({
    "content-type": ct,
    "access-control-allow-origin": "*",
  });
  const put = async (url: string, body: BodyInit, ct: string) => {
    throwIfAborted(signal);
    await cache.put(url, new Response(body, { headers: headers(ct) }));
    throwIfAborted(signal);
  };

  await put(`${scope.realmBase}/about`, json(about), "application/json");
  await put(`${scope.contentBase}/entities/active`, json([entity]), "application/json");
  await put(`${scope.contentBase}/contents/${sceneHash}`, json(entity), "application/json");
  for (const a of assetBytes) {
    await put(`${scope.contentBase}/contents/${a.token}`, a.buf, a.ct);
  }
  for (const a of templateAssets) {
    await put(`${scope.contentBase}/contents/${a.cid}`, a.buf, contentTypeFor(a.path));
  }
  await put(`${scope.contentBase}/contents/${runtime.token}`, runtime.buf, "application/javascript");
  if (crdt) await put(`${scope.contentBase}/contents/${crdt.token}`, crdt.buf, "application/octet-stream");
  if (gameTemplate) {
    await put(
      `${scope.contentBase}/contents/${PLAY_STATE_FILE}`,
      json({ playing: false }),
      "application/json",
    );
  }

  const staged: { file: string; hash: string; stored: Response }[] = [];
  for (const ref of content) {
    const stored = await cache.match(`${scope.contentBase}/contents/${ref.hash}`);
    throwIfAborted(signal);
    // An earlier upload whose bytes are gone is left out rather than served as a 404.
    if (stored) staged.push({ file: ref.file, hash: ref.hash, stored });
  }
  const previewEntity = await stagePreviewRealm(scope, staged, metadata, signal);

  return {
    ok: true,
    sceneHash,
    previewEntity,
    assets: assetBytes.length,
    templateAssets: templateAssets.length,
    ...(gameTemplate ? { game: gameTemplate } : {}),
  };
}

export async function populateTemplateRealm(opts: {
  template: string;
  name?: string;
  composite?: string | null;
  assets?: Record<string, string> | null;
  signal?: AbortSignal;
  scope?: ProjectScope;
}): Promise<PopulateResult> {
  throwIfAborted(opts.signal);
  if (!hasTemplateComposite(opts.template)) {
    return { ok: false, reason: "unknown-template" };
  }
  const sceneJsonText = opts.name
    ? JSON.stringify({ display: { title: opts.name }, tags: [opts.template] })
    : JSON.stringify({ tags: [opts.template] });
  return populateProjectRealm({}, sceneJsonText, {
    template: opts.template,
    composite: opts.composite,
    assets: opts.assets ?? null,
    signal: opts.signal,
    scope: opts.scope,
  });
}
