import { afterEach, expect, it, vi } from "vitest";

import { fetchHotScenes, selectLiveScenes, type HotScene } from "./live-scenes";

afterEach(() => vi.restoreAllMocks());

const scene = (id: string, name: string, usersTotalCount: number): HotScene => ({
  id,
  name,
  baseCoords: [0, 0],
  usersTotalCount,
  parcels: [[0, 0]],
  thumbnail: "",
});

it("keeps the six busiest occupied scenes and leaves Genesis Plaza out", () => {
  const scenes = [
    scene("a", "Genesis Plaza", 90),
    scene("b", "Empty", 0),
    ...Array.from({ length: 7 }, (_, i) => scene(`s${i}`, `Scene ${i}`, i + 1)),
  ];
  expect(selectLiveScenes(scenes).map((s) => s.id)).toEqual(["s6", "s5", "s4", "s3", "s2", "s1"]);
});

it("reads only the rows that are scene cards: a scene without a title is left out and does not stop the page", async () => {
  const titled = scene("a", "Club", 4);
  const { name: _name, ...untitled } = scene("b", "", 9);
  vi.spyOn(globalThis, "fetch").mockResolvedValueOnce(Response.json([titled, untitled, { ...scene("c", "x", 2), name: null }]));
  const scenes = await fetchHotScenes();
  expect(scenes).toEqual([titled]);
  expect(selectLiveScenes(scenes).map((s) => s.id)).toEqual(["a"]);
});

it("an unavailable feed fails the read and makes up no scenes", async () => {
  vi.spyOn(globalThis, "fetch").mockResolvedValueOnce(new Response("down", { status: 503 })).mockResolvedValueOnce(Response.json({ error: "down" }));
  await expect(fetchHotScenes()).rejects.toThrow();
  await expect(fetchHotScenes()).rejects.toThrow();
});
