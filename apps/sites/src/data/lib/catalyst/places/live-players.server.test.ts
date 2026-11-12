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
