import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { loadPlaceTravel, sceneTravelHref } from "./world-scenes";

describe("when resolving place travel for a deployment", () => {
  let place: Parameters<typeof loadPlaceTravel>[0];
  let fetchImpl: ReturnType<typeof vi.fn<typeof fetch>>;
  let result: Awaited<ReturnType<typeof loadPlaceTravel>>;
  beforeEach(() => {
    place = { world: false, world_name: null, base_position: "34,17" };
    fetchImpl = vi.fn<typeof fetch>().mockResolvedValue(new Response(JSON.stringify({ configurations: { realmName: "this-node" } })));
    vi.stubEnv("BEVY_PLAY_URL", "/explorer");
    vi.stubEnv("PUBLIC_CATALYST_URL", "https://node.example");
  });
  afterEach(() => { vi.unstubAllEnvs(); vi.unstubAllGlobals(); vi.restoreAllMocks(); });

  it("should use the configured explorer entry and this node's realm for Genesis", async () => {
    result = await loadPlaceTravel(place, { base: "https://node.example", fetchImpl });
    expect(result.jumpHref).toBe("https://node.example/explorer/?realm=this-node&position=34%2C17");
    expect(fetchImpl).toHaveBeenCalledWith("https://node.example/about", expect.objectContaining({ signal: expect.any(AbortSignal) }));
  });

  describe("and the explorer is hosted on another origin", () => {
    beforeEach(() => {
      vi.stubEnv("BEVY_PLAY_URL", "https://explorer.example/app");
      place = { world: true, world_name: "gallery.dcl.eth", base_position: "4,-5" };
    });
    it("should preserve the serialized target for world scene links in the browser", async () => {
      result = JSON.parse(JSON.stringify(await loadPlaceTravel(place, { fetchImpl })));
      vi.stubEnv("BEVY_PLAY_URL", "");
      vi.stubGlobal("window", { __CATALYST_BASE__: "https://different.example" });
      expect(result.jumpHref).toBe("https://explorer.example/app/?realm=gallery.dcl.eth&position=4%2C-5");
      expect(sceneTravelHref("gallery.dcl.eth", "8,9", result.playBase)).toBe("https://explorer.example/app/?realm=gallery.dcl.eth&position=8%2C9");
      expect(fetchImpl).not.toHaveBeenCalled();
    });
  });

  describe("and this node's realm is unavailable", () => {
    beforeEach(() => { fetchImpl.mockRejectedValueOnce(new Error("offline")); });
    it("should expose a retryable failure instead of sending visitors to a default realm", async () => {
      result = await loadPlaceTravel(place, { fetchImpl });
      expect(result.jumpHref).toBeUndefined();
      expect(result.launchError).toBe("Travel is temporarily unavailable. Please retry.");
    });
  });

  describe("and the about response has no realm", () => {
    beforeEach(() => { fetchImpl.mockResolvedValueOnce(new Response(JSON.stringify({ configurations: {} }))); });
    it("should reject an unresolved realm as a launch target", async () => {
      expect(await loadPlaceTravel(place, { fetchImpl })).toHaveProperty("launchError");
    });
  });
});
