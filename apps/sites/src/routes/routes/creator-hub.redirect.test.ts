import { describe, expect, it, vi } from "vitest";

vi.mock("@/core/lib/telemetry/track", () => ({ track: vi.fn() }));

import { createPathFor, loader } from "./creator-hub.$";

describe("/creator-hub/* redirects to /create/*", () => {
  it.each([
    ["/creator-hub", "", "/create"],
    ["/creator-hub/", "", "/create"],
    ["/creator-hub/manage?filter=unpublished", "manage", "/create/worlds?filter=unpublished"],
    ["/creator-hub/scene-editor?new=1&from=home", "scene-editor", "/create/scene-editor?new=1&from=home"],
    ["/creator-hub/world-settings?world=a.dcl.eth", "world-settings", "/create/world-settings?world=a.dcl.eth"],
    ["/creator-hub/activity/example.dcl.eth", "activity/example.dcl.eth", "/create/activity/example.dcl.eth"],
    ["/creator-hub/land", "land", "/create/land"],
    ["/creator-hub/deploy-world?project=x", "deploy-world", "/create/publish?project=x"],
    ["/creator-hub/world-permissions?world=a.dcl.eth", "world-permissions", "/create/admissions?world=a.dcl.eth"],
    ["/creator-hub/worlds-storage?quota=1", "worlds-storage", "/create/storage?quota=1"],
    ["/creator-hub/multiplayer-test", "multiplayer-test", "/create/preview"],
    ["/creator-hub/gate?world=a", "gate", "/create/admissions?world=a"],
  ])("%s -> %s", (path, splat, expected) => {
    expect(createPathFor(new URL("https://sites.test" + path), splat)).toBe(expected);
  });

  it("answers with a permanent, method-preserving redirect", async () => {
    const response = await loader({
      request: new Request("https://sites.test/creator-hub/manage"),
      params: { "*": "manage" },
      context: {},
    } as never);
    expect(response.status).toBe(308);
    expect(response.headers.get("Location")).toBe("/create/worlds");
  });
});
