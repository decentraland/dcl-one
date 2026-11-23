import { expect, it } from "vitest";

import { HotScene, hotSceneRows, ownContentFile } from "./hot-scenes";

const row = { id: "a", name: "Club", baseCoords: [1, 2], usersTotalCount: 3, parcels: [[1, 2]], thumbnail: "https://peer.example/content/contents/bafythumb" };

it("keeps a scene row and names its thumbnail at the node's own origin", () => {
  expect(hotSceneRows(HotScene, [row])).toEqual([{ ...row, thumbnail: "/content/contents/bafythumb" }]);
});

it("leaves out a row without a title, as the node writes a scene that has none", () => {
  const { name: _name, ...untitled } = row;
  expect(hotSceneRows(HotScene, [untitled, { ...row, name: null }, { ...row, name: 7 }, { ...row, baseCoords: ["1", "2"] }, null])).toEqual([]);
});

it("an answer that is not a list is an error, not an empty list", () => {
  expect(() => hotSceneRows(HotScene, { error: "down" })).toThrow();
});

it("a thumbnail that is not a content file is no image", () => {
  for (const thumbnail of ["https://tracker.example/pixel.png", "javascript:alert(1)", "", null, undefined]) {
    expect(hotSceneRows(HotScene, [{ ...row, thumbnail }])[0].thumbnail).toBe("");
  }
  expect(ownContentFile("/content/contents/bafythumb")).toBe("/content/contents/bafythumb");
});
