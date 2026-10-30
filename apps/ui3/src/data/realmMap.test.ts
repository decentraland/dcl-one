import { describe, expect, it, vi } from "vitest";
import { fetchRealmScenes, realmBounds, realmMapBase, sceneAt, toRealmScene } from "./realmMap";

const ENTITY = {
  id: "b64-gather", type: "scene", pointers: ["45,81", "45,82", "46,81", "46,82"],
  metadata: { display: { title: "Gather 4" }, scene: { base: "45,81", parcels: ["45,81", "45,82", "46,81", "46,82"] }, spawnPoints: [{ name: "gate", default: true, position: { x: 8, y: 0, z: 24 } }] },
};

describe("realm map base", () => {
  it("is the preview server for a preview, the content base for a world, nothing for Genesis City", () => {
    expect(realmMapBase("?preview=true&realm=http%3A%2F%2Flocalhost%3A8002", "http://localhost:8002")).toBe("http://localhost:8002");
    expect(realmMapBase("?realm=https%3A%2F%2Fworlds.example%2Fworld%2Ftophub.dcl.eth", "https://worlds.example/world/tophub.dcl.eth")).toBe("https://worlds.example/world/tophub.dcl.eth");
    expect(realmMapBase("", "tophub.dcl.eth")).toMatch(/\/world\/tophub\.dcl\.eth$/);
    expect(realmMapBase("", "dcl-one")).toBeNull();
    expect(realmMapBase("", null)).toBeNull();
  });
});

describe("realm scenes", () => {
  it("normalises an entity into parcels, base, title and spawn points, and derives a base when metadata lacks one", () => {
    const scene = toRealmScene(ENTITY)!;
    expect(scene).toMatchObject({ id: "b64-gather", title: "Gather 4", base: [45, 81] });
    expect(scene.parcels).toHaveLength(4);
    expect(scene.spawns[0]).toMatchObject({ name: "gate", parcel: [45, 82], world: { x: 728, z: 1320 } });
    expect(toRealmScene({ id: "x", pointers: ["3,4", "2,9"] })).toMatchObject({ title: "Untitled scene", base: [2, 4] });
    expect(toRealmScene({ id: "x", pointers: [] })).toBeNull();
    expect(realmBounds([scene])).toEqual({ minX: 45, minY: 81, maxX: 46, maxY: 82 });
    expect(sceneAt([scene], 46, 82)?.id).toBe("b64-gather");
    expect(sceneAt([scene], 0, 0)).toBeNull();
  });

  it("reads local scene parcels through the content API and pinned scenesUrn entities, deduplicated", async () => {
    const calls: string[] = [];
    const fetchImpl = vi.fn(async (url: string | URL | Request, init?: RequestInit) => {
      const u = String(url);
      calls.push(`${init?.method ?? "GET"} ${u}`);
      if (u.endsWith("/about")) return new Response(JSON.stringify({ configurations: { localSceneParcels: ["45,81", "45,82", "46,81", "46,82"], scenesUrn: ["urn:decentraland:entity:b64-gather?=&baseUrl=http://localhost:8002/content/contents/", "urn:decentraland:entity:b64-hall"] }, content: { publicUrl: "http://localhost:8002/content" } }));
      if (u.endsWith("/entities/active")) return new Response(JSON.stringify([ENTITY]));
      if (u.endsWith("/contents/b64-gather")) return new Response(JSON.stringify({ ...ENTITY, id: undefined }));
      if (u.endsWith("/contents/b64-hall")) return new Response(JSON.stringify({ pointers: ["50,81"], metadata: { display: { title: "Hall" }, scene: { base: "50,81" } } }));
      return new Response("nope", { status: 404 });
    });
    const scenes = await fetchRealmScenes("http://localhost:8002", { fetchImpl });
    expect(scenes.map((s) => s.title)).toEqual(["Gather 4", "Hall"]);
    expect(calls.some((c) => c.startsWith("POST http://localhost:8002/content/entities/active"))).toBe(true);
    expect(calls).toContain("GET http://localhost:8002/content/contents/b64-hall");
  });
});
