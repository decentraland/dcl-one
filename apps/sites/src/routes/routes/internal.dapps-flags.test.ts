import { afterEach, describe, expect, it, vi } from "vitest";

import { resetDappsFlags } from "@/data/lib/feature-flags.server";

import { loader } from "./internal.dapps-flags";

afterEach(() => {
  vi.unstubAllGlobals();
  resetDappsFlags();
});

describe("the rollout flags endpoint", () => {
  it("answers decentraland.org's flags and their variants' payloads as this server read them, cacheable for their minute", async () => {
    const list = "0x00000000000000000000000000000000000000aa";
    vi.stubGlobal(
      "fetch",
      vi.fn(
        async () =>
          new Response(
            JSON.stringify({
              flags: { "dapps-shop-creator-sales": true, "dapps-shop-my-store": true },
              variants: { "dapps-shop-my-store": { name: "testers", enabled: true, payload: { type: "string", value: list } } },
            }),
          ),
      ),
    );
    const res = await loader({ request: new Request("https://node.example/internal/dapps-flags?variants=1") });
    expect(await res.json()).toEqual({
      flags: { "dapps-shop-creator-sales": true, "dapps-shop-my-store": true },
      variants: { "dapps-shop-my-store": list },
    });
    expect(res.headers.get("cache-control")).toBe("public, max-age=60");
    // Without the query, the bare flags map a page loaded before the variants reads: its flags stay on.
    const bare = await loader({ request: new Request("https://node.example/internal/dapps-flags") });
    expect(await bare.json()).toEqual({ "dapps-shop-creator-sales": true, "dapps-shop-my-store": true });
  });
});
