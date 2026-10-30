import { afterEach, describe, expect, test, vi } from "vitest";

import { clearProjectRealm, populateProjectRealm, PREVIEW_CACHE } from "./project-realm";
import { projectScopeForId } from "@ui/editor/project-cache-name";

afterEach(() => {
  vi.unstubAllGlobals();
});

function memoryCaches() {
  const stores = new Map<string, Map<string, Response>>();
  const cache = (name: string) => {
    if (!stores.has(name)) stores.set(name, new Map());
    const values = stores.get(name)!;
    return {
      keys: async () => [...values.keys()].map((url) => new Request(url)),
      match: async (key: string) => values.get(key)?.clone(),
      put: async (key: string, value: Response) => { values.set(key, value.clone()); },
      delete: async (key: string | Request) => values.delete(typeof key === "string" ? key : key.url),
    };
  };
  vi.stubGlobal("caches", { open: async (name: string) => cache(name) });
  return cache;
}

const RUNTIME = "module.exports.onStart = async () => {}";

function serveRuntime(extra: (url: string) => Response | null = () => null) {
  vi.stubGlobal("fetch", vi.fn(async (url: string) => {
    if (url === "/template-bundles/games.js") return new Response(RUNTIME);
    const response = extra(url);
    if (response) return response;
    throw new Error(`Unexpected request ${url}`);
  }));
}

function decodeLocalHash(hash: string): string {
  return new TextDecoder().decode(Uint8Array.from(atob(hash.slice("b64-".length)), (c) => c.charCodeAt(0)));
}

test("the project is staged as the engine's preview realm, scene runtime and authored state included", async () => {
  const cache = memoryCaches();
  serveRuntime();
  const scope = projectScopeForId("cccccccc-cccc-4ccc-8ccc-cccccccccccc");
  const composite = JSON.stringify({ version: 1, components: [{ name: "core::Transform", data: {
    512: { json: { position: { x: 1, y: 0, z: 1 }, rotation: { x: 0, y: 0, z: 0, w: 1 }, scale: { x: 1, y: 1, z: 1 }, parent: 0 } },
  } }] });
  const sceneJson = JSON.stringify({ display: { title: "Two parcels" }, scene: { parcels: ["4,-2", "5,-2"], base: "4,-2" } });
  const result = await populateProjectRealm({ "models/tree.glb": new File(["tree"], "tree.glb") }, sceneJson, { scope, composite });
  expect(result).toMatchObject({ ok: true, assets: 1 });

  const store = cache(PREVIEW_CACHE);
  const realm = `${scope.previewRealm}/`;
  expect(realm).toMatch(/\/play\/preview\/cccccccc-cccc-4ccc-8ccc-cccccccccccc\/$/);
  const { entity } = await (await store.match(`${realm}__manifest`))!.json();
  expect(entity.id).toBe(result.previewEntity);
  expect(entity.id).toMatch(/^b64-[A-Za-z0-9+=]+$/);
  expect(decodeLocalHash(entity.id)).toBe(`/${scope.id}-${scope.runtimeId}`);
  expect(entity.pointers).toEqual(["4,-2", "5,-2"]);
  expect(entity.metadata).toMatchObject({ main: "bin/index.js", display: { title: "Two parcels" }, scene: { base: "4,-2" } });
  const files = Object.fromEntries(entity.content.map((ref: { file: string; hash: string }) => [ref.file, ref.hash]));
  expect(Object.keys(files).sort()).toEqual(["bin/index.js", "main.crdt", "models/tree.glb"]);
  for (const [file, hash] of Object.entries(files) as [string, string][]) {
    expect(decodeLocalHash(hash)).toMatch(new RegExp(`^/${scope.id}/${file.replace(/\./g, "\\.")}\u0000\\d+-${scope.runtimeId}$`));
  }
  expect(await (await store.match(`${realm}content/contents/${files["bin/index.js"]}`))!.text()).toBe(RUNTIME);
  expect(await (await store.match(`${realm}content/contents/${files["models/tree.glb"]}`))!.text()).toBe("tree");
  expect((await (await store.match(`${realm}content/contents/${files["main.crdt"]}`))!.arrayBuffer()).byteLength).toBeGreaterThan(0);

  const own = cache(scope.cacheName);
  expect(await own.match(`${scope.realmBase}/about`)).toBeDefined();
  expect((await (await own.match(`${scope.contentBase}/entities/active`))!.json())[0].pointers).toEqual(["4,-2", "5,-2"]);
}, 30_000); // the first import of the SDK's serializers is slow on a cold transform cache

test("restaging replaces the realm's files and leaves other projects' realms alone", async () => {
  const cache = memoryCaches();
  serveRuntime();
  const scopeA = projectScopeForId("aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa");
  const scopeB = projectScopeForId("bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb");
  const transform = (x: number) => JSON.stringify({ version: 1, components: [{ name: "core::Transform", data: {
    512: { json: { position: { x, y: 0, z: 1 }, rotation: { x: 0, y: 0, z: 0, w: 1 }, scale: { x: 1, y: 1, z: 1 }, parent: 0 } },
  } }] });
  await populateProjectRealm({}, null, { scope: scopeA, composite: transform(1) });
  await populateProjectRealm({}, null, { scope: scopeB, composite: transform(1) });
  const crdtKey = async (scope: typeof scopeA) => {
    const { entity } = await (await cache(PREVIEW_CACHE).match(`${scope.previewRealm}/__manifest`))!.json();
    const hash = entity.content.find((ref: { file: string }) => ref.file === "main.crdt").hash;
    return `${scope.previewRealm}/content/contents/${hash}`;
  };
  const first = await crdtKey(scopeA);
  await populateProjectRealm({}, null, { scope: scopeA, composite: transform(2) });
  const second = await crdtKey(scopeA);
  expect(second).not.toBe(first);
  const keys = (await cache(PREVIEW_CACHE).keys()).map((request) => request.url);
  expect(keys).not.toContain(first);
  expect(keys).toContain(second);
  expect(keys).toContain(await crdtKey(scopeB));

  await clearProjectRealm(undefined, scopeA);
  const left = (await cache(PREVIEW_CACHE).keys()).map((request) => request.url);
  expect(left.some((url) => url.startsWith(`${scopeA.previewRealm}/`))).toBe(false);
  expect(left.some((url) => url.startsWith(`${scopeB.previewRealm}/`))).toBe(true);
});

test("an unreachable scene runtime leaves the previous preview intact", async () => {
  const put = vi.fn(), remove = vi.fn();
  vi.stubGlobal("caches", { open: async () => ({ match: async () => undefined, keys: async () => ["old"], delete: remove, put }) });
  vi.stubGlobal("fetch", vi.fn(async () => new Response("missing", { status: 404 })));
  expect(await populateProjectRealm({})).toEqual({ ok: false, reason: "scene-runtime-unreachable" });
  expect(remove).not.toHaveBeenCalled();
  expect(put).not.toHaveBeenCalled();
});

describe("populateProjectRealm cancellation", () => {
  test("aborts the in-flight realm request before opening the cache", async () => {
    const controller = new AbortController();
    const open = vi.fn();
    vi.stubGlobal("caches", { open });
    let received: AbortSignal | undefined;
    vi.stubGlobal("fetch", vi.fn((_: string, init?: RequestInit) => {
      received = init?.signal ?? undefined;
      return new Promise<Response>((_, reject) => {
        init?.signal?.addEventListener("abort", () => reject(new DOMException("Aborted", "AbortError")), { once: true });
      });
    }));

    const pending = populateProjectRealm({}, null, { signal: controller.signal });
    await vi.waitFor(() => expect(received).toBe(controller.signal));
    controller.abort();

    await expect(pending).rejects.toMatchObject({ name: "AbortError" });
    expect(open).not.toHaveBeenCalled();
  });
});

test("parallel local projects and template refresh retain only their own uploaded files", async () => {
  const stores = new Map<string, Map<string, Response>>();
  const scopeA = projectScopeForId("aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa");
  const scopeB = projectScopeForId("bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb");
  const cache = (name: string) => {
    if (!stores.has(name)) stores.set(name, new Map());
    const values = stores.get(name)!;
    return {
      keys: async () => [...values.keys()].map((url) => new Request(url)),
      match: async (key: string) => values.get(key)?.clone(),
      put: async (key: string, value: Response) => { values.set(key, value.clone()); },
      delete: async (key: Request) => values.delete(key.url),
    };
  };
  vi.stubGlobal("caches", { open: async (name: string) => cache(name) });
  serveRuntime();
  await cache("ch-project-v1").put("https://preview.test/legacy", new Response("unrelated legacy project"));
  await Promise.all([
    populateProjectRealm({ "assets/same.glb": new File(["model A"], "same.glb") }, JSON.stringify({ display: { title: "A" } }), { scope: scopeA }),
    populateProjectRealm({ "assets/same.glb": new File(["model B"], "same.glb") }, JSON.stringify({ display: { title: "B" } }), { scope: scopeB }),
  ]);
  const entity = async (scope: typeof scopeA) => (await (await cache(scope.cacheName).match(`${scope.contentBase}/entities/active`))!.json())[0];
  const first = await entity(scopeA), second = await entity(scopeB);
  expect(first.metadata.display.title).toBe("A");
  expect(second.metadata.display.title).toBe("B");
  const model = (scene: typeof first) => scene.content.find((ref: { file: string }) => ref.file === "assets/same.glb").hash;
  expect(model(first)).not.toBe(model(second));
  expect(await (await cache(scopeA.cacheName).match(`${scopeA.contentBase}/contents/${model(first)}`))!.text()).toBe("model A");
  expect(await cache(scopeB.cacheName).match(`${scopeA.contentBase}/contents/${model(first)}`)).toBeUndefined();
  await populateProjectRealm({}, JSON.stringify({ display: { title: "A refreshed" } }), { scope: scopeA });
  expect(model(await entity(scopeA))).toBe(model(first));
  expect(await (await cache(scopeA.cacheName).match(`${scopeA.contentBase}/contents/${model(first)}`))!.text()).toBe("model A");
  expect((await entity(scopeB)).metadata.display.title).toBe("B");
  expect(await (await cache("ch-project-v1").match("https://preview.test/legacy"))!.text()).toBe("unrelated legacy project");
});


test("unreadable prior preview metadata cannot erase cached assets", async () => {
  const put = vi.fn(), remove = vi.fn();
  const previous = new Response("truncated JSON {");
  vi.stubGlobal("caches", { open: async () => ({
    match: async () => previous.clone(), keys: async () => ["old-asset"], delete: remove, put,
  }) });
  serveRuntime();
  await expect(populateProjectRealm({})).rejects.toBeInstanceOf(SyntaxError);
  expect(remove).not.toHaveBeenCalled();
  expect(put).not.toHaveBeenCalled();
  expect(await previous.text()).toBe("truncated JSON {");
});

test("builder files are fetched together and the first of a path that arrives wins", async () => {
  memoryCaches();
  let inFlight = 0, peak = 0;
  serveRuntime((url) => {
    if (!url.startsWith("/builder-api/contents/")) return null;
    inFlight += 1;
    peak = Math.max(peak, inFlight);
    const cid = url.slice("/builder-api/contents/".length);
    queueMicrotask(() => { inFlight -= 1; });
    return cid === "QmMissing" ? new Response("gone", { status: 403 }) : new Response(`bytes of ${cid}`);
  });
  const scope = projectScopeForId("dddddddd-dddd-4ddd-8ddd-dddddddddddd");
  const result = await populateProjectRealm({ "models/own.glb": new File(["own"], "own.glb") }, null, {
    scope,
    assets: { "models/a.glb": "QmMissing", "models/b.glb": "QmB", "models/own.glb": "QmOwn" },
  });
  expect(result).toMatchObject({ ok: true, templateAssets: 1 });
  expect(peak).toBeGreaterThan(1);
});
