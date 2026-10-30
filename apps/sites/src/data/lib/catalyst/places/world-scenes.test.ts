import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { loadWorldScenes, sceneTravelHref } from "./world-scenes";

describe("when listing scenes in a world", () => {
  let requested: URL | undefined;
  let fetchImpl: typeof fetch;
  beforeEach(() => {
    requested = undefined;
    fetchImpl = vi.fn(async (input: RequestInfo | URL) => {
      requested = new URL(String(input));
      return new Response(JSON.stringify({ ok: true, data: [{ id: "child", title: "Gallery", base_position: "4,-5", world: true, world_name: "sample.dcl.eth" }], total: 21 }), { headers: { "content-type": "application/json" } });
    }) as typeof fetch;
  });
  afterEach(() => vi.restoreAllMocks());
  it("should read the world's scene page with server pagination", async () => {
    await expect(loadWorldScenes("SAMPLE.dcl.eth", 20, { base: "https://node.example", fetchImpl })).resolves.toMatchObject({ total: 21, scenes: [{ id: "child" }] });
    expect(requested!.pathname).toBe("/places/api/worlds/sample.dcl.eth/scenes");
    expect(Object.fromEntries(requested!.searchParams)).toEqual({ offset: "20", limit: "20" });
  });
  it("should travel to the selected world scene coordinates", () => {
    expect(sceneTravelHref("sample.dcl.eth", "4,-5")).toBe("/play/?realm=sample.dcl.eth&position=4%2C-5");
  });
  it("should encode world names and refuse malformed coordinates", () => {
    expect(sceneTravelHref("world&realm=other", "invalid")).toBe("/play/?realm=world%26realm%3Dother");
  });
});
