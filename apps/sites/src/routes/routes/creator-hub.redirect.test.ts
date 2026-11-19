import { RouterContextProvider } from "react-router";
import { describe, expect, it, vi } from "vitest";

vi.mock("@/core/lib/telemetry/track", () => ({ track: vi.fn() }));

import { createPathFor, loader } from "./creator-hub.$";

describe("/creator-hub/* redirects to /create/*", () => {
  it.each([
    ["/creator-hub", "", "/create"],
    ["/creator-hub/", "", "/create"],
    ["/creator-hub/manage?filter=unpublished", "manage", "/download/creator-hub"],
    ["/creator-hub/scene-editor?new=1&from=home", "scene-editor", "/download/creator-hub"],
    ["/creator-hub/activity/example.dcl.eth", "activity/example.dcl.eth", "/download/creator-hub"],
    ["/creator-hub/wearables?tab=published", "wearables", "/create/wearables?tab=published"],
    ["/creator-hub/multiplayer-test?x=1", "multiplayer-test", "/create/preview?x=1"],
  ])("%s -> %s", (path, splat, expected) => {
    expect(createPathFor(new URL("https://sites.test" + path), splat)).toBe(expected);
  });

  it("answers with a permanent, method-preserving redirect", async () => {
    const response = await loader({
      request: new Request("https://sites.test/creator-hub/wearables"),
      params: { "*": "wearables" },
      context: new RouterContextProvider(),
    } as never);
    expect(response.status).toBe(308);
    expect(response.headers.get("Location")).toBe("/create/wearables");
  });
});
