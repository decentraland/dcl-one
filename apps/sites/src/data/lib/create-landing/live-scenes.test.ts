import { expect, it } from "vitest";

import { selectLiveScenes, type HotScene } from "./live-scenes";

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
