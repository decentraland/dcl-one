import { afterEach, describe, expect, it, vi } from "vitest";
vi.mock("../client", async (original) => ({ ...await original<typeof import("../client")>(), getJSON: vi.fn() }));
import { getJSON } from "../client";
import type { Place } from "../schema";
import { loadLivePlayers } from "./live-players.server";

const world = { world: true, world_name: "Sample.dcl.eth", positions: ["0,0"], base_position: "0,0" } as Place;

describe("when counting the players in a world", () => {
  afterEach(() => vi.resetAllMocks());
  it("should read the world's count from the live data, as decentraland.org does, whatever its observation time", async () => {
    vi.mocked(getJSON).mockResolvedValueOnce({ data: { totalUsers: 9, perWorld: [{ worldName: "sample.dcl.eth", users: 2 }] }, lastUpdated: "2020-01-01T00:00:00.000Z" });
    await expect(loadLivePlayers(world)).resolves.toBe(2);
    expect(getJSON).toHaveBeenCalledWith("/live-data", expect.anything());
  });
  it("should report no count when the live data cannot be read", async () => {
    vi.mocked(getJSON).mockRejectedValueOnce(new Error("offline"));
    await expect(loadLivePlayers(world)).resolves.toBeNull();
  });
});

describe("when counting the players in a Genesis City place", () => {
  afterEach(() => vi.resetAllMocks());
  const place = { world: false, positions: ["1,95", "1,96"], base_position: "1,95" } as Place;
  it("should read the hot scenes at their one path, with no query, and count a scene that has no title", async () => {
    vi.mocked(getJSON).mockResolvedValueOnce([{ baseCoords: ["1", "96"], usersTotalCount: 50 }, { id: "s", baseCoords: [1, 96], usersTotalCount: 7 }]);
    await expect(loadLivePlayers(place)).resolves.toBe(7);
    expect(getJSON).toHaveBeenCalledWith("/hot-scenes", { signal: undefined });
  });
  it("should count no players when no hot scene is on the place", async () => {
    vi.mocked(getJSON).mockResolvedValueOnce([{ baseCoords: [9, 9], usersTotalCount: 7 }]);
    await expect(loadLivePlayers(place)).resolves.toBe(0);
  });
});
