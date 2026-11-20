// The top creators' names: taken from the ranking when it carries their profiles, and asked of
// /lambdas/profiles only for the creators it does not.

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { setShopClient } from "../../api/host";
import { catalogClient } from "../fixtures";
import { invalidateShopQueries } from "../query";
import { loadTopCreators } from "./TopCreators";

const CARRIED = "0x00000000000000000000000000000000000000aa";
const MISSING = "0x00000000000000000000000000000000000000bb";

const asked: string[][] = [];
beforeEach(() => {
  asked.length = 0;
  vi.stubGlobal(
    "fetch",
    vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = String(input instanceof Request ? input.url : input);
      if (!url.includes("/lambdas/profiles")) return new Response("{}", { status: 200 });
      const ids = (JSON.parse(String(init?.body ?? "{}")) as { ids?: string[] }).ids ?? [];
      asked.push(ids);
      const avatars = ids.map((id) => ({ avatars: [{ ethAddress: id, userId: id, name: "Asked", hasClaimedName: true, avatar: {} }] }));
      return new Response(JSON.stringify(avatars), { status: 200, headers: { "content-type": "application/json" } });
    }),
  );
  invalidateShopQueries();
});
afterEach(() => {
  setShopClient(null);
  vi.unstubAllGlobals();
});

describe("loadTopCreators", () => {
  it("asks /lambdas/profiles only for the creators the ranking carries no profile for", async () => {
    const ranked = (id: string, extra: object = {}) => ({ id, sales: 9, totalSales: 9, collections: 1, items: 3, ...extra });
    setShopClient(
      catalogClient("ok", {
        "/v3/catalog/creators": { data: [ranked(CARRIED, { profile: { name: "Carried", face: null, claimed: true } }), ranked(MISSING)] },
      }),
    );
    const creators = await loadTopCreators();
    expect(asked.flat()).toEqual([MISSING]);
    const named = Object.fromEntries(creators.map((c) => [c.address.toLowerCase(), c.name]));
    expect(named[CARRIED]).toBe("Carried");
    expect(named[MISSING]).toBe("Asked");
  });
});
