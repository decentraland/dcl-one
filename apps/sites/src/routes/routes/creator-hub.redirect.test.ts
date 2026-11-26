import { RouterContextProvider } from "react-router";
import { describe, expect, it, vi } from "vitest";

vi.mock("@/core/lib/telemetry/track", () => ({ track: vi.fn() }));

import { createPathFor, loader } from "./creator-hub.$";

describe("/creator-hub/* redirects to /create/*", () => {
  it.each([
    ["/creator-hub", "", "/create"],
    ["/creator-hub/", "", "/create"],
    ["/creator-hub/scenes?sort=recent", "scenes", "/create/scenes?sort=recent"],
    ["/creator-hub/collections?status=draft", "collections", "/create/collections?status=draft"],
    ["/creator-hub/multiplayer-test?x=1", "multiplayer-test", "/create/preview?x=1"],
  ])("%s -> %s", (path, splat, expected) => {
    expect(createPathFor(new URL("https://sites.test" + path), splat)).toBe(expected);
  });

  it("answers with a permanent, method-preserving redirect", async () => {
    const response = await loader({
      request: new Request("https://sites.test/creator-hub/collections"),
      params: { "*": "collections" },
      context: new RouterContextProvider(),
    } as never);
    expect(response.status).toBe(308);
    expect(response.headers.get("Location")).toBe("/create/collections");
  });
});
