import { afterEach, describe, expect, it, vi } from "vitest";

import { LiveDataSchema, wcsBase } from "./wcs";

afterEach(() => {
  vi.unstubAllEnvs();
});

describe("wcsBase", () => {
  it("derives from the ONE base, honours the old env names and an override without its trailing slash", () => {
    expect(wcsBase()).toBe("http://127.0.0.1:5210/worlds-content-server");
    expect(wcsBase("https://wcs.example.test/")).toBe("https://wcs.example.test");
    vi.stubEnv("CATALYST_URL", "https://catalyst.example.org/");
    expect(wcsBase()).toBe("https://catalyst.example.org/worlds-content-server");
    vi.stubEnv("WORLDS_URL", "http://127.0.0.1:5143");
    expect(wcsBase()).toBe("http://127.0.0.1:5143");
    vi.stubEnv("WORLDS_CONTENT_SERVER_URL", "https://worlds-content-server.decentraland.org/");
    expect(wcsBase()).toBe("https://worlds-content-server.decentraland.org");
  });
});

describe("LiveDataSchema", () => {
  it("parses the live-data envelope and rejects a body missing perWorld", () => {
    const live = LiveDataSchema.parse({
      data: { totalUsers: 5, perWorld: [{ worldName: "petbarn.dcl.eth", users: 3 }] },
      lastUpdated: "2026-08-01T09:52:30.775Z",
    });
    expect(live.data.perWorld[0]).toEqual({ worldName: "petbarn.dcl.eth", users: 3 });
    expect(LiveDataSchema.parse({ data: { totalUsers: 0, perWorld: [] } }).lastUpdated).toBeNull();
    expect(LiveDataSchema.safeParse({ data: { totalUsers: 0 }, lastUpdated: null }).success).toBe(false);
  });
});
