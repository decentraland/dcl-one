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
    it("should fail fast during the short outage cache window", async () => {
      await expect(loadPlaceTravel(place, { base })).resolves.toBeNull();
      await expect(loadPlaceTravel(place, { base })).resolves.toBeNull();
      expect(fetchImpl).toHaveBeenCalledTimes(1);
    });
    it("should refresh immediately for an explicit Retry", async () => {
      await expect(loadPlaceTravel(place, { base })).resolves.toBeNull();
      await expect(loadPlaceTravel(place, { base, refreshRealm: true })).resolves.toHaveProperty("jumpHref");
      expect(fetchImpl).toHaveBeenCalledTimes(2);
    });
    it("should allow ordinary reads to recover after the outage cache expires", async () => {
      await expect(loadPlaceTravel(place, { base })).resolves.toBeNull();
      vi.setSystemTime(new Date("2026-11-03T18:00:11Z"));
      await expect(loadPlaceTravel(place, { base })).resolves.toHaveProperty("jumpHref");
      expect(fetchImpl).toHaveBeenCalledTimes(2);
    });
  });

  describe("and concurrent requests arrive before the node responds", () => {
    let resolveRead: (response: Response) => void;
    let first: ReturnType<typeof loadPlaceTravel>;
    let second: ReturnType<typeof loadPlaceTravel>;
    let upstreamSignal: AbortSignal;
    beforeEach(() => {
      fetchImpl.mockImplementationOnce(async (_input, init) => {
        upstreamSignal = init!.signal!;
        return new Promise<Response>((resolve) => { resolveRead = resolve; });
      });
      first = loadPlaceTravel(place, { base, signal: controller.signal });
      second = loadPlaceTravel({ ...place, base_position: "3,4" }, { base });
    });
    it("should share one upstream lookup and build each place's own target", async () => {
      expect(fetchImpl).toHaveBeenCalledTimes(1);
      resolveRead(new Response(JSON.stringify({ configurations: { realmName: "shared-node" } })));
      await expect(first).resolves.toMatchObject({ jumpHref: expect.stringContaining("position=1%2C2") });
      await expect(second).resolves.toMatchObject({ jumpHref: expect.stringContaining("position=3%2C4") });
    });
    it("should cancel one waiter without cancelling the shared read or the other waiter", async () => {
      controller.abort();
      await expect(first).rejects.toMatchObject({ name: "AbortError" });
      expect(upstreamSignal.aborted).toBe(false);
      resolveRead(new Response(JSON.stringify({ configurations: { realmName: "shared-node" } })));
      await expect(second).resolves.toHaveProperty("jumpHref");
      await expect(loadPlaceTravel(place, { base })).resolves.toHaveProperty("jumpHref");
      expect(fetchImpl).toHaveBeenCalledTimes(1);
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
