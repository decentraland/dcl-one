const PREFIX = "ch-project-v2:";
const STORAGE_PREFIX = "ch:project-scope:";
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
const fallbackScopes = new Map<string, string>();
const documentScopes = new Map<string, ProjectScope>();
const cacheRestorations = new Map<string, Promise<void>>();

export interface ProjectScope {
  id: string;
  runtimeId: string;
  cacheName: string;
  realmBase: string;
  contentBase: string;
  /** The realm the engine loads: its service worker's preview realm (PREVIEW_REALM.md in the
      pinned Explorer source), staged from this scope by `populateProjectRealm`. */
  previewRealm: string;
  previousRuntimeId?: string;
  storageKey?: string;
}

/** Transport sessions change on Retry. Project storage must not. */
export function editorProjectIdentity(): string {
  if (typeof window === "undefined") return "server";
  const url = new URL(window.location.href);
  const q = url.searchParams;
  if (q.get("project")) return `local:${q.get("project")}`;
  if (q.get("draft")) return `draft:${q.get("draft")}`;
  if (q.get("source") === "local") return "local:staged";
  // Different starter projects, including differently named copies, must not
  // share either mutable runtime bytes or their Play/Stop gate.
  for (const key of ["editorSession", "bootNonce", "step", "from"]) q.delete(key);
  q.sort();
  return `${url.pathname}?${q}`;
}

/** The engine's service worker scope; its preview realms live under `preview/`. */
export const ENGINE_DIR = "/play/";

export function projectScopeForId(id: string): ProjectScope {
  if (!UUID.test(id)) throw new Error("Invalid browser project scope");
  const origin = typeof window === "undefined" ? "http://localhost" : window.location.origin;
  const realmBase = `${origin}/_project/sessions/${id}`;
  return {
    id,
    runtimeId: id,
    cacheName: `${PREFIX}${id}`,
    realmBase,
    contentBase: `${realmBase}/content`,
    previewRealm: `${origin}${ENGINE_DIR}preview/${id.toLowerCase()}`,
  };
}

export function projectScopeForIdentity(identity: string): ProjectScope {
  const key = `${STORAGE_PREFIX}${identity}`;
  let id = fallbackScopes.get(key);
  try { id = sessionStorage.getItem(key) ?? id; } catch { /* storage may be disabled */ }
  if (!id || !UUID.test(id)) {
    id = crypto.randomUUID();
    try { sessionStorage.setItem(key, id); } catch { /* keep this document usable */ }
    fallbackScopes.set(key, id);
  }
  const existing = documentScopes.get(key);
  if (existing?.id === id) return existing;
  // sessionStorage is copied by window.open/duplicating a tab. Never use that
  // persisted ID as the mutable runtime namespace: two tabs of the same draft
  // must still be able to Play and Stop independently.
  const runtimeId = crypto.randomUUID();
  let previousRuntimeId: string | undefined;
  try {
    const previous = sessionStorage.getItem(`${key}:runtime`);
    if (previous && UUID.test(previous)) previousRuntimeId = previous;
  } catch { /* reload persistence unavailable */ }
  const scope = { ...projectScopeForId(runtimeId), id, previousRuntimeId, storageKey: key };
  documentScopes.set(key, scope);
  return scope;
}

/** Opening a different physical project never clears the project being left. */
export function beginLocalProjectScope(): ProjectScope {
  return projectScopeForId(crypto.randomUUID());
}

export function activateLocalProjectScope(scope: ProjectScope): void {
  const key = `${STORAGE_PREFIX}local:staged`;
  fallbackScopes.set(key, scope.id);
  try {
    sessionStorage.setItem(key, scope.id);
    sessionStorage.setItem(`${key}:runtime`, scope.runtimeId);
  } catch { /* use this document's fallback */ }
  documentScopes.set(key, scope);
}

/** Publish lineage only after a fully prepared runtime is safe to reopen. */
export function rememberProjectScope(scope: ProjectScope): void {
  if (!scope.storageKey) return;
  try { sessionStorage.setItem(`${scope.storageKey}:runtime`, scope.runtimeId); }
  catch { /* cache and recovery still work until the document closes */ }
}

/** Resolve once at the start of an async operation; never follow a new page mid-write. */
export function currentProjectScope(): ProjectScope {
  return projectScopeForIdentity(editorProjectIdentity());
}

export function projectScopeFromViewport(src: string | null | undefined): ProjectScope | null {
  if (!src) return null;
  try {
    const origin = typeof window === "undefined" ? "http://localhost" : window.location.origin;
    const viewport = new URL(src, origin);
    const realm = new URL(viewport.searchParams.get("realm") ?? "", origin);
    if (realm.origin !== origin) return null;
    const scoped = /^\/_project\/sessions\/([^/]+)\/?$/.exec(realm.pathname)
      ?? new RegExp(`^${ENGINE_DIR}preview/([^/]+)/?$`).exec(realm.pathname);
    if (scoped?.[1]) {
      const current = currentProjectScope();
      return current.runtimeId === scoped[1] ? current : projectScopeForId(scoped[1]);
    }
    return /^\/_project\/?$/.test(realm.pathname) ? currentProjectScope() : null;
  } catch { return null; }
}

/** Copy this project's previous runtime; never search arbitrary project caches. */
export function restoreProjectCache(scope: ProjectScope): Promise<void> {
  if (!scope.previousRuntimeId || typeof caches === "undefined") return Promise.resolve();
  const key = `${scope.runtimeId}:${scope.previousRuntimeId}`;
  const existing = cacheRestorations.get(key);
  if (existing) return existing;
  // A duplicated tab inherits one snapshot, not a live link to uploads added
  // later in the source. Preparation, inventory reads and uploads join one copy.
  const pending = copyProjectCache(scope);
  cacheRestorations.set(key, pending);
  void pending.catch(() => { if (cacheRestorations.get(key) === pending) cacheRestorations.delete(key); });
  return pending;
}

async function copyProjectCache(scope: ProjectScope): Promise<void> {
  if (!scope.previousRuntimeId || typeof caches === "undefined") return;
  const previous = projectScopeForId(scope.previousRuntimeId);
  if (!await caches.has(previous.cacheName)) return;
  const source = await caches.open(previous.cacheName);
  const target = await caches.open(scope.cacheName);
  for (const request of await source.keys()) {
    const response = await source.match(request);
    if (!response) continue;
    const url = request.url.replace(previous.realmBase, scope.realmBase);
    if (url.endsWith("/contents/one-play-state")) continue;
    // Playback cleanup may already have seeded one-play-state while this
    // copy awaits CacheStorage. That does not mean assets were restored.
    // Fill only missing entries so repeated/cancelled preparation is safe.
    if (await target.match(url)) continue;
    const path = new URL(url).pathname;
    const metadata = path.endsWith("/about") || path.endsWith("/entities/active");
    const copy = metadata
      ? new Response((await response.text()).split(previous.realmBase).join(scope.realmBase), response)
      : response;
    await target.put(url, copy);
  }
}

export function scopeProjectViewport(src: string | undefined): string | undefined {
  if (!src || typeof window === "undefined") return src;
  const scope = projectScopeFromViewport(src);
  if (!scope) return src;
  const url = new URL(src, window.location.origin);
  url.searchParams.set("realm", scope.previewRealm);
  return src.startsWith("/") ? `${url.pathname}${url.search}${url.hash}` : url.href;
}

function projectAssetReference(value: string, scope: ProjectScope): { url: URL; cacheName: string; token: string } | null {
  try {
    const url = new URL(value, scope.realmBase);
    if (url.origin !== new URL(scope.realmBase).origin) return null;
    const match = /^\/_project\/(?:sessions\/([^/]+)\/)?content\/contents\/([^/]+)$/.exec(url.pathname);
    if (!match?.[2] || (match[1] && !UUID.test(match[1]))) return null;
    const token = decodeURIComponent(match[2]);
    if (!token || token === "one-play-state" || token === "." || token === ".." || /[\\/]/.test(token)) return null;
    return { url, cacheName: match[1] ? `${PREFIX}${match[1]}` : "ch-project-v1", token: match[2] };
  } catch { return null; }
}

function rewriteProjectReferences(composite: string | null | undefined, scope: ProjectScope, base: string, canonical = false): string | null | undefined {
  if (!composite) return composite;
  try {
    let changed = false;
    const rewritten = JSON.stringify(JSON.parse(composite, (_key, value: unknown) => {
      if (typeof value !== "string") return value;
      const asset = projectAssetReference(value, scope);
      if (!asset) return value;
      const next = `${base}/contents/${asset.token}${asset.url.search}${asset.url.hash}`;
      changed ||= next !== value;
      return next;
    }));
    return changed || canonical ? rewritten : composite;
  } catch { return composite; }
}

/** Both v1 drafts and scoped snapshots must bind uploaded URLs to this runtime. */
export function rebaseLegacyProjectUrls(composite: string | null | undefined, scope: ProjectScope): string | null | undefined {
  return rewriteProjectReferences(composite, scope, scope.contentBase);
}

/** Equivalence for recovery input matching only; never a fetch/storage namespace. */
export function canonicalProjectComposite(composite: string | null, scope: ProjectScope): string | null {
  return rewriteProjectReferences(composite, scope, `${new URL(scope.realmBase).origin}/_project/content`, true) ?? null;
}

/** Copy exact references only. Never scan caches or import another scene/Play gate. */
export async function migrateLegacyProjectAssets(scope: ProjectScope, composite: string | null | undefined, contents: Record<string, string>): Promise<void> {
  if (typeof caches === "undefined") return;
  const references = new Map<string, NonNullable<ReturnType<typeof projectAssetReference>>>();
  const add = (value: string) => {
    const asset = projectAssetReference(value, scope);
    if (asset && asset.cacheName !== scope.cacheName) references.set(asset.url.origin + asset.url.pathname, asset);
  };
  const origin = new URL(scope.realmBase).origin;
  for (const hash of Object.values(contents)) {
    if (hash && !/[\\/?#]/.test(hash)) add(`${origin}/_project/content/contents/${hash}`);
  }
  try {
    JSON.parse(composite || "null", (_key, value: unknown) => {
      if (typeof value === "string") add(value);
      return value;
    });
  } catch { /* malformed input is handled by composite validation */ }
  if (!references.size) return;
  const target = await caches.open(scope.cacheName);
  for (const [sourceUrl, asset] of references) {
    const destination = `${scope.contentBase}/contents/${asset.token}`;
    if (await target.match(destination)) continue;
    if (!await caches.has(asset.cacheName)) continue;
    const response = await (await caches.open(asset.cacheName)).match(sourceUrl);
    if (response) await target.put(destination, response);
  }
}

export class ProjectUploadUnavailableError extends Error {
  constructor() {
    super("Some uploaded models are no longer in browser storage. Re-import the missing files or reopen the original project folder. Your scene changes are preserved.");
    this.name = "ProjectUploadUnavailableError";
  }
}

export async function prepareProjectComposite(composite: string | null, scope: ProjectScope): Promise<string | null> {
  await migrateLegacyProjectAssets(scope, composite, {});
  const local = new Set<string>();
  try {
    JSON.parse(composite || "null", (_key, value: unknown) => {
      const asset = typeof value === "string" ? projectAssetReference(value, scope) : null;
      if (asset?.token.startsWith("local-")) local.add(asset.token);
      return value;
    });
  } catch { /* malformed input is handled by composite validation */ }
  if (local.size) {
    if (typeof caches === "undefined") throw new ProjectUploadUnavailableError();
    const target = await caches.open(scope.cacheName);
    for (const token of local) {
      if (!await target.match(`${scope.contentBase}/contents/${token}`)) throw new ProjectUploadUnavailableError();
    }
  }
  return rebaseLegacyProjectUrls(composite, scope) ?? null;
}
