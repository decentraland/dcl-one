// Saved /create scene drafts live on the node as builder projects: one
// project per draft, its builder manifest the draft (the node's
// `/builder-api/v1/projects/{id}/manifest`, upstream builder-server's shape:
// `{version, project, scene: {sdk7: {id, composite, mappings}}}` plus a
// `draft` member with what only /create reads). The manifest's revision is
// the draft version: a save names the revision it read (If-Match, or
// If-None-Match: * for a first save) and a stale one answers 412 with the
// server copy. A draft id is a local slug or a project id: a slug's project
// is the uuid v5 of `<wallet>/<slug>`, a uuid is its own project, so a draft
// saved on another device lists under its project id.
//
// The drafts sites used to keep on its own disk (`/api/creator-hub/drafts`)
// move here once per wallet and session: each is written to the node
// unless the node already has that draft, then deleted from the disk.
import { getIdentity } from "../../auth/session";
import { signRequest } from "../../auth/signer";
import type { AuthIdentity } from "../../auth/types";
import { BUILDER_PREFIX } from "../bases";
import type { Draft, DraftMeta } from "./scene-drafts";
import { hashComposite, isSafeDraftId, renameComposite } from "./scene-drafts";
export { hashComposite } from "./scene-drafts";

export type ServerDraftBlob = {
  composite: string;
  title?: string;
  base?: string;
  template?: string;
  assets?: Record<string, string>;
};

export function parseServerDraftBlob(blob: unknown): ServerDraftBlob | null {
  if (!blob || typeof blob !== "object") return null;
  const b = blob as Record<string, unknown>;
  if (typeof b.composite !== "string" || !b.composite.trim()) return null;
  try {
    JSON.parse(b.composite);
  } catch {
    return null;
  }
  const out: ServerDraftBlob = { composite: b.composite };
  if (typeof b.title === "string") out.title = b.title;
  if (typeof b.base === "string") out.base = b.base;
  if (typeof b.template === "string") out.template = b.template;
  if (b.assets && typeof b.assets === "object" && !Array.isArray(b.assets)) {
    const assets: Record<string, string> = {};
    for (const [k, v] of Object.entries(b.assets as Record<string, unknown>)) {
      if (typeof v === "string") assets[k] = v;
    }
    if (Object.keys(assets).length > 0) out.assets = assets;
  }
  return out;
}

export function serverCopyIsNewer(
  localUpdatedAt: number | null | undefined,
  serverUpdatedAt: number | null | undefined,
): boolean {
  return (serverUpdatedAt ?? 0) > (localUpdatedAt ?? 0);
}

export type ServerDraft = {
  id: string;
  version: number;
  updatedAt: number;
  title: string;
  blob: ServerDraftBlob;
};

export type ServerDraftMeta = {
  id: string;
  version: number;
  updatedAt: number;
  title: string;
};

const API = `${BUILDER_PREFIX}/v1`;
const LEGACY = "/api/creator-hub/drafts";
const MANIFEST_VERSION = 1;
const UNTITLED = "Untitled scene";

// ── project ids ───────────────────────────────────────────────────────────

const UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
// The uuid v5 namespace of /create draft ids (fixed: changing it orphans
// every saved draft).
const DRAFT_NAMESPACE = "3f6c2a1e-8d4b-4c7a-9e15-b2d8f0a46c93";

function uuidBytes(uuid: string): Uint8Array {
  const hex = uuid.replace(/-/g, "");
  const out = new Uint8Array(16);
  for (let i = 0; i < 16; i++) out[i] = parseInt(hex.slice(i * 2, i * 2 + 2), 16);
  return out;
}

function formatUuid(bytes: Uint8Array): string {
  const hex = Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20, 32)}`;
}

/** RFC 4122 §4.3: the SHA-1 name-based uuid of `name` under `namespace`. */
export async function uuidV5(namespace: string, name: string): Promise<string> {
  const ns = uuidBytes(namespace);
  const text = new TextEncoder().encode(name);
  const input = new Uint8Array(ns.length + text.length);
  input.set(ns);
  input.set(text, ns.length);
  const hash = new Uint8Array(await crypto.subtle.digest("SHA-1", input)).slice(0, 16);
  hash[6] = (hash[6]! & 0x0f) | 0x50;
  hash[8] = (hash[8]! & 0x3f) | 0x80;
  return formatUuid(hash);
}

/** The node project that holds `wallet`'s draft `id`. */
export async function draftProjectId(wallet: string, id: string): Promise<string> {
  if (UUID_RE.test(id)) return id.toLowerCase();
  return uuidV5(DRAFT_NAMESPACE, `${wallet.trim().toLowerCase()}/${id}`);
}

// ── manifest <-> draft ────────────────────────────────────────────────────

type ManifestProject = {
  id: string;
  title: string;
  description: string;
  scene_id: string;
  cols: number;
  rows: number;
  manifest_revision?: number;
  updated_at?: string;
};

export type DraftManifest = {
  version: number;
  project: ManifestProject;
  scene: Record<string, unknown>;
  draft: { id: string; hash: string; base?: string; template?: string; blob?: unknown };
};

function object(value: unknown): value is Record<string, unknown> {
  return Boolean(value && typeof value === "object" && !Array.isArray(value));
}

/** The bounding box of the editor's scene layout (`SceneMetadata-v3`), 1x1 without one. */
function layoutOf(composite: unknown): { cols: number; rows: number } {
  const components = object(composite) && Array.isArray(composite.components) ? composite.components : [];
  for (const component of components) {
    if (!object(component) || component.name !== "inspector::SceneMetadata-v3" || !object(component.data)) continue;
    const root = component.data["0"];
    let metadata: unknown = object(root) ? ("json" in root ? root.json : "value" in root ? root.value : root) : null;
    if (typeof metadata === "string") {
      try { metadata = JSON.parse(metadata); } catch { metadata = null; }
    }
    const parcels = object(metadata) && object(metadata.layout) && Array.isArray(metadata.layout.parcels) ? metadata.layout.parcels : [];
    const points = parcels
      .map((p): [number, number] | null => {
        if (object(p) && typeof p.x === "number" && typeof p.y === "number") return [p.x, p.y];
        if (typeof p === "string") {
          const [x, y] = p.split(",").map(Number);
          return Number.isInteger(x) && Number.isInteger(y) ? [x!, y!] : null;
        }
        return null;
      })
      .filter((p): p is [number, number] => p !== null);
    if (points.length === 0) break;
    const span = (i: 0 | 1) => Math.min(1000, Math.max(...points.map((p) => p[i])) - Math.min(...points.map((p) => p[i])) + 1);
    return { cols: span(0), rows: span(1) };
  }
  return { cols: 1, rows: 1 };
}

/** The manifest a save writes: a /create blob as an sdk7 scene, anything else kept whole in `draft.blob`. */
export function draftManifest(
  id: string,
  projectId: string,
  input: { hash: string; blob: unknown; title?: string },
): DraftManifest {
  const parsed = parseServerDraftBlob(input.blob);
  const title = (input.title ?? parsed?.title ?? "").slice(0, 512) || UNTITLED;
  const draft: DraftManifest["draft"] = { id, hash: input.hash };
  let scene: Record<string, unknown> = {};
  let layout = { cols: 1, rows: 1 };
  if (parsed) {
    const composite: unknown = JSON.parse(parsed.composite);
    layout = layoutOf(composite);
    scene = { sdk7: { id: projectId, composite, mappings: parsed.assets ?? {} } };
    if (parsed.base !== undefined) draft.base = parsed.base;
    if (parsed.template !== undefined) draft.template = parsed.template;
  } else {
    draft.blob = input.blob;
  }
  return {
    version: MANIFEST_VERSION,
    project: { id: projectId, title, description: "", scene_id: projectId, ...layout },
    scene,
    draft,
  };
}

/** The draft a manifest holds; `null` for one /create cannot open (an sdk6 builder scene). */
export function manifestDraft(manifest: unknown): Draft | null {
  if (!object(manifest) || !object(manifest.project)) return null;
  const project = manifest.project as Partial<ManifestProject>;
  if (typeof project.id !== "string") return null;
  const draft = object(manifest.draft) ? manifest.draft : {};
  const title = typeof project.title === "string" ? project.title : "";
  let blob: unknown;
  if ("blob" in draft) {
    blob = draft.blob;
  } else {
    const sdk7 = object(manifest.scene) && object(manifest.scene.sdk7) ? manifest.scene.sdk7 : null;
    if (!sdk7 || !object(sdk7.composite)) return null;
    const composite = JSON.stringify(sdk7.composite);
    const out: ServerDraftBlob = { composite, title };
    if (typeof draft.base === "string") out.base = draft.base;
    if (typeof draft.template === "string") out.template = draft.template;
    if (object(sdk7.mappings) && Object.keys(sdk7.mappings).length > 0) {
      out.assets = Object.fromEntries(
        Object.entries(sdk7.mappings).filter((e): e is [string, string] => typeof e[1] === "string"),
      );
    }
    blob = out;
  }
  const id = typeof draft.id === "string" && isSafeDraftId(draft.id) ? draft.id : project.id;
  const updatedAt = typeof project.updated_at === "string" ? Date.parse(project.updated_at) : NaN;
  const meta: DraftMeta = {
    id,
    hash: typeof draft.hash === "string" ? draft.hash : hashComposite(JSON.stringify(blob)),
    version: typeof project.manifest_revision === "number" ? project.manifest_revision : 0,
    updatedAt: Number.isFinite(updatedAt) ? updatedAt : 0,
    title,
  };
  return { ...meta, blob };
}

// ── node requests ─────────────────────────────────────────────────────────

async function nodeFetch(
  identity: AuthIdentity,
  method: string,
  path: string,
  init: { body?: unknown; headers?: Record<string, string>; signal?: AbortSignal } = {},
): Promise<Response> {
  const { headers } = await signRequest(identity, method, path);
  init.signal?.throwIfAborted();
  return fetch(path, {
    method,
    signal: init.signal,
    headers: {
      ...headers,
      ...init.headers,
      ...(init.body !== undefined ? { "content-type": "application/json" } : {}),
    },
    body: init.body !== undefined ? JSON.stringify(init.body) : undefined,
  });
}

function manifestPath(projectId: string): string {
  return `${API}/projects/${projectId}/manifest`;
}

export type PutDraftResult =
  | { ok: true; version: number }
  | { ok: false; conflict: true; version: number; server: Draft | null }
  | { ok: false; conflict?: false; status: number };

/** One conditional save: the manifest at `baseVersion` (0: no draft yet). */
async function putManifest(
  identity: AuthIdentity,
  id: string,
  input: { baseVersion: number; hash: string; blob: unknown; title?: string },
  signal?: AbortSignal,
): Promise<PutDraftResult> {
  const projectId = await draftProjectId(identity.signer, id);
  const precondition: Record<string, string> =
    input.baseVersion > 0 ? { "if-match": `"${input.baseVersion}"` } : { "if-none-match": "*" };
  const res = await nodeFetch(identity, "PUT", manifestPath(projectId), {
    body: { manifest: draftManifest(id, projectId, input) },
    headers: precondition,
    signal,
  });
  const body = (await res.json().catch(() => null)) as { data?: Record<string, unknown> } | null;
  if (res.status === 412) {
    // No live draft on the node (deleted meanwhile): the retry creates it.
    const server = manifestDraft(body?.data?.manifest);
    const revision = body?.data?.revision;
    return { ok: false, conflict: true, version: server && typeof revision === "number" ? revision : 0, server };
  }
  const version = body?.data?.manifest_revision;
  if (!res.ok || typeof version !== "number") return { ok: false, status: res.status };
  return { ok: true, version };
}

/** The sync engine's transport: one conditional save, the server copy on a conflict. */
export async function putServerDraft(
  id: string,
  input: { baseVersion: number; hash: string; blob: unknown; title?: string },
): Promise<PutDraftResult> {
  const identity = getIdentity();
  if (!identity) throw new Error("sync: not signed in");
  const result = await putManifest(identity, id, input);
  if (result.ok) knownVersions.set(id, result.version);
  else knownVersions.delete(id);
  return result;
}

export async function deleteServerDraft(id: string): Promise<void> {
  const identity = getIdentity();
  if (!identity) return;
  const projectId = await draftProjectId(identity.signer, id);
  const response = await nodeFetch(identity, "DELETE", `${API}/projects/${projectId}`);
  if (!response.ok && response.status !== 404) throw new Error("The saved scene could not be deleted. Try again.");
  knownVersions.delete(id);
}

async function readDraft(identity: AuthIdentity, id: string, signal?: AbortSignal): Promise<Draft | null> {
  const projectId = await draftProjectId(identity.signer, id);
  const res = await nodeFetch(identity, "GET", manifestPath(projectId), { signal });
  if (res.status === 404) return null;
  if (!res.ok) throw new Error(`reading the saved scene answered ${res.status}`);
  const draft = manifestDraft(await res.json());
  return draft ? { ...draft, id } : null;
}

/** A read-modify-write of a saved draft under its revision, retried once on a conflict. */
async function rewriteDraft(
  identity: AuthIdentity,
  id: string,
  change: (draft: Draft) => { hash: string; blob: unknown; title: string },
): Promise<boolean> {
  for (let attempt = 0; attempt < 2; attempt++) {
    const draft = await readDraft(identity, id);
    if (!draft) return false;
    const result = await putManifest(identity, id, { baseVersion: draft.version, ...change(draft) });
    if (result.ok) {
      knownVersions.set(id, result.version);
      return true;
    }
    if (!result.conflict) return false;
  }
  return false;
}

function renamedBlob(draft: Draft, title: string): { hash: string; blob: unknown; title: string } {
  const parsed = parseServerDraftBlob(draft.blob);
  if (!parsed) return { hash: draft.hash, blob: draft.blob, title };
  const composite = renameComposite(parsed.composite, title);
  return { hash: hashComposite(composite), blob: { ...parsed, composite, title }, title };
}

export async function renameServerDraft(id: string, title: string): Promise<void> {
  const identity = getIdentity();
  if (!identity) throw new Error("Sign in to rename this saved scene.");
  const name = title.trim();
  if (!name || name.length > 512) throw new Error("A scene name of 1 to 512 characters is required.");
  if (!(await rewriteDraft(identity, id, (draft) => renamedBlob(draft, name)))) {
    knownVersions.delete(id);
    throw new Error("The scene could not be renamed. Refresh and try again.");
  }
}

export async function duplicateServerDraft(id: string): Promise<ServerDraft> {
  const identity = getIdentity();
  if (!identity) throw new Error("Sign in to duplicate this saved scene.");
  const draft = await readDraft(identity, id).catch(() => null);
  if (!draft) throw new Error("The scene could not be duplicated. Refresh and try again.");
  const copyId = crypto.randomUUID();
  const copy = renamedBlob(draft, `${draft.title || UNTITLED} copy`);
  const result = await putManifest(identity, copyId, { baseVersion: 0, ...copy });
  const blob = parseServerDraftBlob(copy.blob);
  if (!result.ok || !blob) throw new Error("The scene could not be duplicated. Refresh and try again.");
  knownVersions.set(copyId, result.version);
  return { id: copyId, version: result.version, updatedAt: Date.now(), title: copy.title, blob };
}

// ── the one-time move off the sites disk ──────────────────────────────────

const imported = new Set<string>();

/**
 * Moves the signer's drafts from the old sites disk store to the node, once
 * per wallet and session: each one the node does not already hold is
 * written there, then the disk copy is deleted. Returns how many moved.
 */
export async function importLegacyDrafts(identity: AuthIdentity): Promise<number> {
  const wallet = identity.signer.toLowerCase();
  if (imported.has(wallet)) return 0;
  imported.add(wallet);
  let moved = 0;
  try {
    const { headers } = await signRequest(identity, "GET", LEGACY);
    const res = await fetch(LEGACY, { headers });
    if (res.status >= 500) imported.delete(wallet);
    if (!res.ok) return 0;
    const listed = (await res.json()) as { drafts?: unknown };
    const metas = Array.isArray(listed?.drafts) ? (listed.drafts as Array<Partial<DraftMeta>>) : [];
    for (const meta of metas) {
      if (typeof meta?.id !== "string" || !isSafeDraftId(meta.id)) continue;
      const path = `${LEGACY}/${encodeURIComponent(meta.id)}`;
      const signed = await signRequest(identity, "GET", path);
      const one = await fetch(path, { headers: signed.headers });
      if (!one.ok) continue;
      const draft = (await one.json()) as Draft;
      const result = await putManifest(identity, meta.id, {
        baseVersion: 0,
        hash: typeof draft.hash === "string" ? draft.hash : "",
        blob: draft.blob,
        title: typeof draft.title === "string" ? draft.title : undefined,
      });
      if (!result.ok && !result.conflict) continue;
      const del = await signRequest(identity, "DELETE", path);
      await fetch(path, { method: "DELETE", headers: del.headers });
      if (result.ok) moved++;
    }
  } catch {
    // the next session tries again; the disk copies stay until moved
    imported.delete(wallet);
  }
  return moved;
}

// ── listing and reading ───────────────────────────────────────────────────

type ListedProject = { id: string; title: string; revision: number; updatedAt: number };

/**
 * The signer's saved drafts as the node lists its projects (metadata, no
 * manifests): those holding a manifest, by project id. Moves the old disk
 * drafts first. Throws when the node does not answer with a listing.
 */
async function listProjects(identity: AuthIdentity, signal?: AbortSignal): Promise<Map<string, ListedProject>> {
  await importLegacyDrafts(identity);
  const res = await nodeFetch(identity, "GET", `${API}/projects`, { signal });
  if (!res.ok) throw new Error(`The node could not list your saved scenes (${res.status}).`);
  const raw = (await res.json()) as { data?: { items?: unknown } };
  if (!Array.isArray(raw?.data?.items)) throw new Error("The node answered without a list of your saved scenes.");
  const out = new Map<string, ListedProject>();
  for (const item of raw.data.items as Array<Record<string, unknown>>) {
    if (!object(item) || typeof item.id !== "string") continue;
    const revision = typeof item.manifest_revision === "number" ? item.manifest_revision : 0;
    if (revision <= 0) continue;
    const updatedAt = typeof item.updated_at === "string" ? Date.parse(item.updated_at) : NaN;
    out.set(item.id, {
      id: item.id,
      title: typeof item.title === "string" ? item.title : "",
      revision,
      updatedAt: Number.isFinite(updatedAt) ? updatedAt : 0,
    });
  }
  return out;
}

/**
 * The signer's saved drafts. A draft whose project one of `localSlugs`
 * names is listed under that slug; one saved elsewhere under its project
 * id, which every other call here takes as its draft id. `null` signed
 * out; rejects when the node could not list them, which is not "none".
 */
export async function listServerDrafts(localSlugs: Iterable<string> = []): Promise<ServerDraftMeta[] | null> {
  const identity = getIdentity();
  if (!identity || typeof fetch === "undefined") return null;
  const projects = await listProjects(identity);
  const slugs = new Map<string, string>();
  for (const slug of localSlugs) slugs.set(await draftProjectId(identity.signer, slug), slug);
  return [...projects.values()].map((p) => ({
    id: slugs.get(p.id) ?? p.id,
    version: p.revision,
    updatedAt: p.updatedAt,
    title: p.title,
  }));
}

/** The saved draft `id`; `null` when there is none (asked without a 404 by listing first). */
export async function fetchServerDraft(id: string, signal?: AbortSignal): Promise<ServerDraft | null> {
  const identity = getIdentity();
  if (!identity || typeof fetch === "undefined") return null;
  try {
    const projects = await listProjects(identity, signal);
    if (!projects.has(await draftProjectId(identity.signer, id))) return null;
    const draft = await readDraft(identity, id, signal);
    const blob = parseServerDraftBlob(draft?.blob);
    if (!draft || !blob) return null;
    knownVersions.set(id, draft.version);
    return { id, version: draft.version, updatedAt: draft.updatedAt, title: draft.title, blob };
  } catch {
    return null;
  }
}

const knownVersions = new Map<string, number>();

export function forgetServerDraftVersions(): void {
  knownVersions.clear();
  imported.clear();
}

async function serverDraftVersion(identity: AuthIdentity, id: string): Promise<number> {
  const known = knownVersions.get(id);
  if (known !== undefined) return known;
  const projects = await listProjects(identity).catch(() => null);
  return projects?.get(await draftProjectId(identity.signer, id))?.revision ?? 0;
}

/**
 * Saves `blob` as draft `id`, last writer wins: a save that finds the node
 * moved retries once at the node's revision.
 */
export async function pushServerDraft(
  id: string,
  blob: ServerDraftBlob,
  signal?: AbortSignal,
): Promise<boolean> {
  const identity = getIdentity();
  if (!identity || typeof fetch === "undefined") return false;

  signal?.throwIfAborted();
  const putOnce = async (baseVersion: number): Promise<PutDraftResult | null> => {
    signal?.throwIfAborted();
    try {
      return await putManifest(
        identity,
        id,
        { baseVersion, hash: hashComposite(blob.composite), title: blob.title, blob },
        signal,
      );
    } catch {
      return null;
    }
  };

  let result = await putOnce(await serverDraftVersion(identity, id));
  if (result && !result.ok && result.conflict) result = await putOnce(result.version);
  if (!result?.ok) {
    knownVersions.delete(id);
    return false;
  }
  knownVersions.set(id, result.version);
  return true;
}
