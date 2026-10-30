import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { projectScopeForId, restoreProjectCache } from "./project-cache-name";
import { loadProjectModels, stashProjectModel } from "./project-models";

beforeEach(() => {
  const stores = new Map<string, Map<string, Response>>();
  vi.stubGlobal("caches", {
    has: async (name: string) => stores.has(name),
    open: async (name: string) => {
      if (!stores.has(name)) stores.set(name, new Map());
      const entries = stores.get(name)!;
      return {
        keys: async () => [...entries.keys()].map(url => new Request(url)),
        match: async (url: string | Request) => entries.get(typeof url === "string" ? url : url.url)?.clone(),
        put: async (url: string, response: Response) => { entries.set(url, response.clone()); },
      };
    },
  });
});
afterEach(() => { vi.unstubAllGlobals(); });
const scope = () => projectScopeForId(crypto.randomUUID());
const file = (name: string, text: string) => ({ name, arrayBuffer: async () => new TextEncoder().encode(text).buffer }) as File;

test("unplaced uploads reopen from their scoped inventory without sharing same-named model bytes", async () => {
  const first = scope(), second = scope();
  const a = await stashProjectModel(file("my model.gltf", "first model"), first);
  const b = await stashProjectModel(file("my model.gltf", "second model"), second);
  expect(a).not.toBe(b);
  expect(await loadProjectModels(first)).toEqual([{ path: "my model.gltf", folder: "uploaded to this project", url: a }]);
  expect(await loadProjectModels(second)).toEqual([{ path: "my model.gltf", folder: "uploaded to this project", url: b }]);
  expect(await (await (await caches.open(first.cacheName)).match(a))!.text()).toBe("first model");
  expect(await (await (await caches.open(second.cacheName)).match(b))!.text()).toBe("second model");
});

test("reload forks the upload inventory and its URLs while preserving original display names", async () => {
  const previous = scope();
  const original = await stashProjectModel(file("a nice model.glb", "model bytes"), previous);
  const next = { ...scope(), previousRuntimeId: previous.runtimeId };
  await restoreProjectCache(next);
  const models = await loadProjectModels(next);
  expect(models).toHaveLength(1);
  expect(models[0]!.path).toBe("a nice model.glb");
  expect(models[0]!.url).toBe(original.replace(previous.realmBase, next.realmBase));
  expect(await (await (await caches.open(next.cacheName)).match(models[0]!.url))!.text()).toBe("model bytes");
});

test("simultaneous same-named uploads keep distinct bytes and both inventory entries", async () => {
  const project = scope();
  const urls = await Promise.all([stashProjectModel(file("same.glb", "first"), project), stashProjectModel(file("same.glb", "second"), project)]);
  expect(new Set(urls).size).toBe(2);
  expect((await loadProjectModels(project)).map(model => model.url).sort()).toEqual(urls.sort());
});

test("pre-inventory scoped uploads are discoverable without scanning unrelated caches", async () => {
  const project = scope(), other = scope();
  await (await caches.open(project.cacheName)).put(`${project.contentBase}/contents/local-oldtime-old_model.glb`, new Response("mine"));
  await (await caches.open(other.cacheName)).put(`${other.contentBase}/contents/local-oldtime-other_model.glb`, new Response("other"));
  expect((await loadProjectModels(project)).map(model => model.path)).toEqual(["old_model.glb"]);
});

test("inventory waits for its lineage even when opened before viewport preparation", async () => {
  const previous = scope();
  await stashProjectModel(file("not placed.glb", "inherited model bytes"), previous);
  const next = { ...scope(), previousRuntimeId: previous.runtimeId };
  expect(await caches.has(next.cacheName)).toBe(false);
  const models = await loadProjectModels(next);
  expect(models.map(model => model.path)).toEqual(["not placed.glb"]);
  expect(models[0]!.url).toContain(next.realmBase);
});

test("a restored duplicate never imports uploads added later in its source tab", async () => {
  const original = scope();
  await stashProjectModel(file("initial.glb", "inherited"), original);
  const duplicate = { ...scope(), previousRuntimeId: original.runtimeId };
  expect((await loadProjectModels(duplicate)).map(model => model.path)).toEqual(["initial.glb"]);
  await stashProjectModel(file("later.glb", "not inherited"), original);
  expect((await loadProjectModels(duplicate)).map(model => model.path)).toEqual(["initial.glb"]);
  expect((await loadProjectModels(original)).map(model => model.path).sort()).toEqual(["initial.glb", "later.glb"]);
});

test("an upload joins lineage restoration before extending the inherited inventory", async () => {
  const original = scope();
  await stashProjectModel(file("original model.glb", "inherited"), original);
  const duplicate = { ...scope(), previousRuntimeId: original.runtimeId };
  await Promise.all([loadProjectModels(duplicate), stashProjectModel(file("new model.glb", "new"), duplicate)]);
  const models = await loadProjectModels(duplicate);
  expect(models.map(model => model.path).sort()).toEqual(["new model.glb", "original model.glb"]);
  expect(models.every(model => model.url.startsWith(duplicate.contentBase))).toBe(true);
});
