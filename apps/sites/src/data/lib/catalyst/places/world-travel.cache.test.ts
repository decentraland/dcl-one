import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { loadPlaceTravel } from "./world-scenes";

describe("when repeated place requests resolve the same node's realm", () => {
  let base: string;
  let place: Parameters<typeof loadPlaceTravel>[0];
  let fetchImpl: ReturnType<typeof vi.fn<typeof fetch>>;
  let controller: AbortController;
  beforeEach(() => {
    base = `https://realm-${crypto.randomUUID()}.example`;
    place = { world: false, world_name: null, base_position: "1,2" };
    fetchImpl = vi.fn<typeof fetch>().mockImplementation(async () => new Response(JSON.stringify({ configurations: { realmName: "cached-node" } })));
    controller = new AbortController();
    vi.useFakeTimers({ toFake: ["Date"] });
    vi.setSystemTime(new Date("2026-11-03T18:00:00Z"));
    vi.stubGlobal("fetch", fetchImpl);
  });
  afterEach(() => { vi.useRealTimers(); vi.unstubAllGlobals(); vi.restoreAllMocks(); });

  it("should reuse a successful realm across different places on the same node", async () => {
    await loadPlaceTravel(place, { base });
    await expect(loadPlaceTravel({ ...place, base_position: "3,4" }, { base })).resolves.toMatchObject({ jumpHref: expect.stringContaining("position=3%2C4") });
    expect(fetchImpl).toHaveBeenCalledTimes(1);
  });

  describe("and another node is requested", () => {
    it("should resolve that node independently", async () => {
      await loadPlaceTravel(place, { base });
      await loadPlaceTravel(place, { base: `${base}/other-node` });
      expect(fetchImpl).toHaveBeenCalledTimes(2);
    });
  });

  describe("and the cached realm expires", () => {
    beforeEach(async () => { await loadPlaceTravel(place, { base }); });
    it("should refresh the observation rather than retain it indefinitely", async () => {
      vi.setSystemTime(new Date("2026-11-03T18:06:00Z"));
      await loadPlaceTravel(place, { base });
      expect(fetchImpl).toHaveBeenCalledTimes(2);
    });
  });

  describe("and a lookup fails", () => {
    beforeEach(() => { fetchImpl.mockResolvedValueOnce(new Response("Unavailable", { status: 503 })); });
    it("should leave the failure uncached so Retry can recover immediately", async () => {
      await expect(loadPlaceTravel(place, { base })).resolves.toBeNull();
      await expect(loadPlaceTravel(place, { base })).resolves.toHaveProperty("jumpHref");
      expect(fetchImpl).toHaveBeenCalledTimes(2);
    });
  });

  describe("and one request is aborted after a successful lookup", () => {
    beforeEach(async () => { await loadPlaceTravel(place, { base }); controller.abort(); });
    it("should honor that cancellation without discarding the completed observation", async () => {
      await expect(loadPlaceTravel(place, { base, signal: controller.signal })).rejects.toMatchObject({ name: "AbortError" });
      await expect(loadPlaceTravel(place, { base })).resolves.toHaveProperty("jumpHref");
      expect(fetchImpl).toHaveBeenCalledTimes(1);
    });
  });
});
