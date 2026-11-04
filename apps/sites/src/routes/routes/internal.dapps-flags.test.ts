import { afterEach, describe, expect, it, vi } from "vitest";

import { resetDappsFlags } from "@/data/lib/feature-flags.server";

import { loader } from "./internal.dapps-flags";

afterEach(() => {
  vi.unstubAllGlobals();
  resetDappsFlags();
});

describe("the rollout flags endpoint", () => {
  it("answers decentraland.org's flags as this server read them, cacheable for their minute", async () => {
    vi.stubGlobal("fetch", vi.fn(async () => new Response(JSON.stringify({ flags: { "dapps-shop-creator-sales": true } }))));
    const res = await loader();
    expect(await res.json()).toEqual({ "dapps-shop-creator-sales": true });
    expect(res.headers.get("cache-control")).toBe("public, max-age=60");
  });
});
