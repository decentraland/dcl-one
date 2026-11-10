import { describe, expect, it } from "vitest";

import { loadWorldScenes } from "./world-scenes.server";

const BASE = "https://worlds.example.test";

describe("loadWorldScenes", () => {
  it("reads every page until the reported total", async () => {
    const total = 250;
    const offsets: number[] = [];
    const fetchImpl = (async (input: RequestInfo | URL) => {
      const url = new URL(String(input));
      const limit = Number(url.searchParams.get("limit"));
      const offset = Number(url.searchParams.get("offset"));
      offsets.push(offset);
      const scenes = Array.from({ length: Math.max(0, Math.min(limit, total - offset)) }, (_, i) => ({
        entityId: `bafy${offset + i}`,
        entity: { metadata: { scene: { base: `${offset + i},0`, parcels: [`${offset + i},0`] } } },
        parcels: [`${offset + i},0`],
      }));
      return new Response(JSON.stringify({ scenes, total }), {
        status: 200,
        headers: { "content-type": "application/json" },
      });
    }) as unknown as typeof fetch;

    const scenes = await loadWorldScenes("big.dcl.eth", { base: BASE, fetchImpl });

    expect(offsets).toEqual([0, 100, 200]);
    expect(scenes).toHaveLength(total);
    expect(scenes?.[total - 1]).toEqual({
      entityId: `bafy${total - 1}`,
      baseParcel: `${total - 1},0`,
      parcels: [`${total - 1},0`],
    });
  });
});
