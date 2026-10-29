import { afterEach, expect, test, vi } from "vitest";

afterEach(() => {
  sessionStorage.clear();
  window.history.replaceState({}, "", "/");
  vi.unstubAllGlobals();
  vi.resetModules();
});

function memoryCaches() {
  const stores = new Map<string, Map<string, Response>>();
  const key = (request: RequestInfo | URL) => request instanceof Request ? request.url : String(request);
  return {
    has: async (name: string) => stores.has(name),
    open: async (name: string) => {
      if (!stores.has(name)) stores.set(name, new Map());
      const values = stores.get(name)!;
      return {
        keys: async () => [...values.keys()].map(url => new Request(url)),
        match: async (request: RequestInfo | URL) => values.get(key(request))?.clone(),
        put: async (request: RequestInfo | URL, response: Response) => { values.set(key(request), response.clone()); },
      };
    },
  };
}

test("project URLs isolate different browser projects while retries keep the same runtime", async () => {
  const { currentProjectScope, scopeProjectViewport } = await import("./project-cache-name");
  window.history.replaceState({}, "", "/create/scene-editor?new=1&name=First&template=empty");
  const first = currentProjectScope();
  const viewport = scopeProjectViewport("/_play/?realm=%2F_project&editorSession=first")!;
  expect(new URL(viewport, location.origin).searchParams.get("realm")).toBe(first.previewRealm);
  expect(first.previewRealm).toBe(`${location.origin}/play/preview/${first.runtimeId}`);
  expect(currentProjectScope()).toEqual(first);
  expect(scopeProjectViewport(viewport.replace("editorSession=first", "editorSession=retry"))).toContain(encodeURIComponent(first.previewRealm));
  window.history.replaceState({}, "", "/create/scene-editor?new=1&name=Second&template=empty");
  const second = currentProjectScope();
  expect(second.cacheName).not.toBe(first.cacheName);
  expect(second.id).not.toBe(first.id);
  expect(scopeProjectViewport("/_play/?realm=https%3A%2F%2Fexample.org%2Fworld%2Falice")).toBe("/_play/?realm=https%3A%2F%2Fexample.org%2Fworld%2Falice");
  expect(scopeProjectViewport("/_play/?realm=https%3A%2F%2Fcatalyst.example.org")).toBe("/_play/?realm=https%3A%2F%2Fcatalyst.example.org");
});

test("reload and duplicated sessionStorage fork runtime bytes and Play state, preserving uploaded assets", async () => {
  const cacheStorage = memoryCaches();
  vi.stubGlobal("caches", cacheStorage);
  window.history.replaceState({}, "", "/create/scene-editor?draft=private-draft");
  const initial = await import("./project-cache-name");
  const first = initial.currentProjectScope();
  initial.rememberProjectScope(first);
  const cache = await cacheStorage.open(first.cacheName);
  await cache.put(`${first.realmBase}/about`, Response.json({ content: { publicUrl: first.contentBase } }));
  await cache.put(`${first.contentBase}/contents/local-model`, new Response("private uploaded bytes"));
  await cache.put(`${first.contentBase}/contents/one-play-state`, Response.json({ playing: true }));
  // Module reload models a second document with copied sessionStorage, also
  // exactly how a real reload starts before the original JS context is gone.
  vi.resetModules();
  const next = await import("./project-cache-name");
  const second = next.currentProjectScope();
  expect(second.id).toBe(first.id);
  expect(second.runtimeId).not.toBe(first.runtimeId);
  expect(second.previousRuntimeId).toBe(first.runtimeId);
  const copied = await cacheStorage.open(second.cacheName);
  await copied.put(`${second.contentBase}/contents/one-play-state`, Response.json({ playing: false }));
  await next.restoreProjectCache(second);
  expect(await (await copied.match(`${second.contentBase}/contents/local-model`))!.text()).toBe("private uploaded bytes");
  expect(await (await copied.match(`${second.realmBase}/about`))!.json()).toEqual({ content: { publicUrl: second.contentBase } });
  expect(await (await copied.match(`${second.contentBase}/contents/one-play-state`))!.json()).toEqual({ playing: false });
  expect(await (await cache.match(`${first.contentBase}/contents/one-play-state`))!.json()).toEqual({ playing: true });
  await next.restoreProjectCache(second);
  expect(await (await copied.match(`${second.contentBase}/contents/one-play-state`))!.json()).toEqual({ playing: false });
});

test("opening a physical folder forks its cache without deleting the project being left", async () => {
  const { currentProjectScope, beginLocalProjectScope, activateLocalProjectScope } = await import("./project-cache-name");
  window.history.replaceState({}, "", "/create/scene-editor?source=local");
  const original = currentProjectScope();
  const next = beginLocalProjectScope();
  expect(next.cacheName).not.toBe(original.cacheName);
  expect(currentProjectScope()).toEqual(original);
  activateLocalProjectScope(next);
  expect(currentProjectScope()).toEqual(next);
});

test("legacy migration copies only this draft's explicit assets, not shared scene state", async () => {
  const storage = memoryCaches();
  vi.stubGlobal("caches", storage);
  const { currentProjectScope, migrateLegacyProjectAssets, rebaseLegacyProjectUrls } = await import("./project-cache-name");
  const scope = currentProjectScope();
  const legacy = await storage.open("ch-project-v1");
  const prefix = `${location.origin}/_project/content/contents/`;
  await legacy.put(`${prefix}local-mine`, new Response("my uploaded model"));
  await legacy.put(`${prefix}other-draft`, new Response("not this project"));
  await legacy.put(`${prefix}one-play-state`, Response.json({ playing: true }));
  const input = JSON.stringify({ components: [{ data: { 512: { json: { src: `${prefix}local-mine` } } } }] });
  await migrateLegacyProjectAssets(scope, input, {});
  const cache = await storage.open(scope.cacheName);
  expect(await (await cache.match(`${scope.contentBase}/contents/local-mine`))!.text()).toBe("my uploaded model");
  expect(await cache.match(`${scope.contentBase}/contents/other-draft`)).toBeUndefined();
  expect(await cache.match(`${scope.contentBase}/contents/one-play-state`)).toBeUndefined();
  expect(rebaseLegacyProjectUrls(input, scope)).toContain(`${scope.contentBase}/contents/local-mine`);
  expect(await (await legacy.match(`${prefix}other-draft`))!.text()).toBe("not this project");
});

test("reopened scoped composites fork explicit uploaded references, not another runtime's bytes or gate", async () => {
  const storage = memoryCaches();
  vi.stubGlobal("caches", storage);
  const { projectScopeForId, prepareProjectComposite } = await import("./project-cache-name");
  const original = projectScopeForId(crypto.randomUUID());
  const fork = projectScopeForId(crypto.randomUUID());
  const source = await storage.open(original.cacheName);
  await source.put(`${original.contentBase}/contents/my-model`, new Response("my original bytes"));
  await source.put(`${original.contentBase}/contents/not-referenced`, new Response("other bytes"));
  await source.put(`${original.contentBase}/contents/one-play-state`, Response.json({ playing: true }));
  await source.put(`${original.realmBase}/about`, Response.json({ scene: "another runtime" }));
  const input = JSON.stringify({ version: 1, components: [{ data: { 512: { json: {
    src: `${original.contentBase}/contents/my-model?variant=1#mesh`,
    gate: `${original.contentBase}/contents/one-play-state`,
    remote: "https://other.test/_project/content/contents/my-model",
    world: `${location.origin}/world/alice/content/contents/my-model`,
  } } } }] });
  const prepared = await prepareProjectComposite(input, fork);
  expect(prepared).toContain(`${fork.contentBase}/contents/my-model?variant=1#mesh`);
  expect(prepared).toContain(`${original.contentBase}/contents/one-play-state`);
  expect(prepared).toContain("https://other.test/_project/content/contents/my-model");
  expect(prepared).toContain(`${location.origin}/world/alice/content/contents/my-model`);
  const target = await storage.open(fork.cacheName);
  expect(await (await target.match(`${fork.contentBase}/contents/my-model`))!.text()).toBe("my original bytes");
  expect(await target.keys()).toHaveLength(1);
  await target.put(`${fork.contentBase}/contents/my-model`, new Response("fork's independent bytes"));
  await prepareProjectComposite(input, fork);
  expect(await (await target.match(`${fork.contentBase}/contents/my-model`))!.text()).toBe("fork's independent bytes");
  expect(await (await source.match(`${original.contentBase}/contents/my-model`))!.text()).toBe("my original bytes");
});

test("reference migration never searches unrelated caches for a missing asset", async () => {
  const storage = memoryCaches();
  vi.stubGlobal("caches", storage);
  const { projectScopeForId, prepareProjectComposite } = await import("./project-cache-name");
  const source = projectScopeForId(crypto.randomUUID());
  const unrelated = projectScopeForId(crypto.randomUUID());
  const target = projectScopeForId(crypto.randomUUID());
  await (await storage.open(unrelated.cacheName)).put(`${unrelated.contentBase}/contents/missing`, new Response("unrelated bytes"));
  await prepareProjectComposite(JSON.stringify({ src: `${source.contentBase}/contents/missing` }), target);
  expect(await (await storage.open(target.cacheName)).keys()).toHaveLength(0);
  expect(await storage.has(source.cacheName)).toBe(false);
});

test("recovery input canonicalization equates only namespace changes, not authored content changes", async () => {
  const { projectScopeForId, canonicalProjectComposite, rebaseLegacyProjectUrls } = await import("./project-cache-name");
  const original = projectScopeForId(crypto.randomUUID());
  const fork = projectScopeForId(crypto.randomUUID());
  const input = JSON.stringify({ components: [{ src: `${original.contentBase}/contents/my-model`, name: "Authored" }] });
  const rebased = rebaseLegacyProjectUrls(input, fork)!;
  expect(canonicalProjectComposite(input, fork)).toBe(canonicalProjectComposite(rebased, fork));
  expect(canonicalProjectComposite(input.replace("my-model", "different-model"), fork)).not.toBe(canonicalProjectComposite(input, fork));
  expect(canonicalProjectComposite(input.replace("Authored", "Saved elsewhere"), fork)).not.toBe(canonicalProjectComposite(input, fork));
  expect(canonicalProjectComposite("invalid JSON", fork)).toBe("invalid JSON");
});

test("missing explicit uploads report storage loss while non-local template hashes may fall through", async () => {
  const storage = memoryCaches();
  vi.stubGlobal("caches", storage);
  const { projectScopeForId, prepareProjectComposite, ProjectUploadUnavailableError } = await import("./project-cache-name");
  const source = projectScopeForId(crypto.randomUUID());
  const unrelated = projectScopeForId(crypto.randomUUID());
  const target = projectScopeForId(crypto.randomUUID());
  const token = "local-lost-model.glb";
  await (await storage.open(unrelated.cacheName)).put(`${unrelated.contentBase}/contents/${token}`, new Response("wrong project's bytes"));
  await expect(prepareProjectComposite(JSON.stringify({ src: `${source.contentBase}/contents/${token}` }), target)).rejects.toBeInstanceOf(ProjectUploadUnavailableError);
  await expect(prepareProjectComposite(JSON.stringify({ src: `${target.contentBase}/contents/${token}` }), target)).rejects.toThrow("Re-import the missing files");
  expect(await storage.has(source.cacheName)).toBe(false);
  expect(await (await storage.open(target.cacheName)).keys()).toHaveLength(0);
  expect(await prepareProjectComposite(JSON.stringify({ src: `${source.contentBase}/contents/template-hash` }), target)).toContain(`${target.contentBase}/contents/template-hash`);
});
