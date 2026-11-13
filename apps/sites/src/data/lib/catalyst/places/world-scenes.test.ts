import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { loadWorldScenes, sceneTravelHref } from "./world-scenes";

describe("when listing scenes in a world", () => {
  let requested: URL | undefined;
  let answer: Response;
  let fetchImpl: typeof fetch;
  beforeEach(() => {
    requested = undefined;
    answer = Response.json({ scenes: [
      { entityId: "bafyscene1", entity: { metadata: { display: { title: "Gallery" }, scene: { base: "4,-5" } } } },
      { entityId: "bafyscene2", parcels: ["7,7"] },
    ], total: 2 });
    fetchImpl = vi.fn(async (input: RequestInfo | URL) => { requested = new URL(String(input)); return answer; }) as typeof fetch;
  });
  afterEach(() => vi.restoreAllMocks());
  it("should read the worlds content server's scenes, as decentraland.org does", async () => {
    await expect(loadWorldScenes("SAMPLE.dcl.eth", { base: "https://worlds.example", fetchImpl })).resolves.toEqual([
      { entityId: "bafyscene1", title: "Gallery", base: "4,-5" },
      { entityId: "bafyscene2", title: "bafyscen", base: "7,7" },
    ]);
    expect(requested!.href).toBe("https://worlds.example/world/sample.dcl.eth/scenes");
  });
  it("should tell a failed read from a world with no scenes", async () => {
    answer = new Response("down", { status: 503 });
    await expect(loadWorldScenes("sample.dcl.eth", { base: "https://worlds.example", fetchImpl })).resolves.toBeNull();
  });
  it("should travel to the selected world scene coordinates", () => {
    expect(sceneTravelHref("sample.dcl.eth", "4,-5")).toBe("/play/?realm=sample.dcl.eth&position=4%2C-5");
  });
  it("should encode world names and refuse malformed coordinates", () => {
    expect(sceneTravelHref("world&realm=other", "invalid")).toBe("/play/?realm=world%26realm%3Dother");
  });
});
