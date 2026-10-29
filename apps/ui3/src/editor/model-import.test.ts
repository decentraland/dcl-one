import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { importModelFile, modelAssetPaths } from "./model-import";
import type { ProjectAssets } from "./types";

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

const file = (name: string, text: string) => ({ name, arrayBuffer: async () => new TextEncoder().encode(text).buffer }) as File;

function fakeAssets() {
  const written = new Map<string, ArrayBuffer>();
  const assets: ProjectAssets = {
    preparePreview: async () => Object.fromEntries([...written.keys()].map(path => [path, "hash"])),
    list: async () => [...written.keys()].map(path => ({ path, size: written.get(path)!.byteLength })),
    read: async path => ({ content: written.get(path)!, revision: "1" }),
    write: async (path, content) => { written.set(path, content); },
    remove: async () => {},
  };
  return { assets, written };
}

test("copies the chosen model into the project assets, registers it, and returns the scene path", async () => {
  const { assets, written } = fakeAssets();
  const rpc = vi.fn(async (method: string, [contents]: [Record<string, string>]) => {
    expect(method).toBe("registerContent");
    return Object.keys(contents).length;
  });
  const src = await importModelFile({ current: { rpc } as never }, file("my door.glb", "glb bytes"), assets);
  expect(src).toBe("assets/imported/local-my_door-glb/my_door.glb");
  expect(new TextDecoder().decode(written.get(src)!)).toBe("glb bytes");
  expect(rpc).toHaveBeenCalledOnce();
});

test("falls back to the cached upload url when the project has no asset store", async () => {
  const src = await importModelFile({ current: null }, file("lamp.gltf", "gltf"));
  expect(src).toMatch(/\/contents\/local-[\w-]+-lamp\.gltf$/);
});

test("refuses anything that is not a model file", async () => {
  await expect(importModelFile({ current: null }, file("notes.txt", "x"))).rejects.toThrow(/\.glb \/ \.gltf/);
});

test("lists only the project's model files, sorted", () => {
  expect(modelAssetPaths([{ path: "b.glb" }, { path: "tex.png" }, { path: "a/door.gltf" }])).toEqual(["a/door.gltf", "b.glb"]);
});
