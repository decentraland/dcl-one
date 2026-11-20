// What the credit authorization helpers send: the line's whole quoted price, and a Credits part only when one is asked
// for, on the single route and on a one-line group alike.

import { describe, expect, it } from "vitest";

import type { ShopClient } from "../api/shop";
import { authorizeUsdCredit, authorizeUsdCreditGroup } from "./credits";

const ITEM = { contractAddress: "0x00000000000000000000000000000000000000c1", itemId: "7" };

function client() {
  const sent: { route: "single" | "group"; body: unknown }[] = [];
  const stub = {
    authorizations: {
      authorize: async (body: unknown) => (sent.push({ route: "single", body }), {}),
      authorizeGroup: async (body: unknown) => (sent.push({ route: "group", body }), {}),
    },
  } as unknown as ShopClient;
  return { stub, sent };
}

describe("authorizing a credit", () => {
  it("the single route sends the whole price, and the Credits part only when asked", async () => {
    const { stub, sent } = client();
    await authorizeUsdCredit(115, { item: ITEM, creditsCents: 50 }, stub);
    await authorizeUsdCredit(115, { item: ITEM }, stub);
    expect(sent.map((s) => s.body)).toEqual([
      { usdPriceCents: 115, ...ITEM, creditsCents: 50, source: "website" },
      { usdPriceCents: 115, ...ITEM, source: "website" },
    ]);
  });

  it("a one-line group carries its line's Credits part to the batch route", async () => {
    const { stub, sent } = client();
    await authorizeUsdCreditGroup([{ usdPriceCents: 115, ...ITEM, creditsCents: 50 }], "client", stub);
    expect(sent).toEqual([{ route: "group", body: { items: [{ usdPriceCents: 115, ...ITEM, creditsCents: 50 }], source: "client" } }]);
  });
});
