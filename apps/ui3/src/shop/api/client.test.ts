import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

import { generateEphemeralKey, type AuthIdentity } from "../../data/auth/identity";
import { createShopApi, creditsDisabled, insufficientCredits, ShopApiError, tooManyLiveCredits } from "./client";
import { ENDPOINTS } from "./endpoints";
import { setupServer } from "msw/node";

import { FIXTURE_BUYER, fixtureTransport, wireHandlers } from "./fixtures";
import { build, OUTPUT } from "./fixtures/generate.mjs";
import { createShopClient } from "./shop";
import { identitySigner, type ShopSigner, type TransportRequest } from "./signer";

const SHOP_UI_ROUTES: readonly [route: string, legacyAliases: string[]][] = [
  ["DELETE /shop-api/v1/cart/items/{}/{}", ["DELETE /credits/cart/items/{}/{}"]],
  ["DELETE /shop-api/v1/lists/{}", ["DELETE /marketplace-api/v1/lists/{}"]],
  ["DELETE /shop-api/v1/lists/{}/access", ["DELETE /marketplace-api/v1/lists/{}/access"]],
  ["DELETE /shop-api/v1/lists/{}/picks/{}", ["DELETE /marketplace-api/v1/lists/{}/picks/{}"]],
  ["DELETE /shop-api/v1/notify-requests", ["DELETE <shop-server host>/notify-requests"]],
  ["DELETE /shop-api/v1/outfits/{}", ["DELETE <shop-server host>/v1/outfits/{}"]],
  ["DELETE /shop-api/v1/picks/{}", ["DELETE /marketplace-api/v1/picks/{}"]],
  ["DELETE /shop-api/v1/users/{}", ["DELETE /credits/users/{}"]],
  ["GET /shop-api/v1/accounts", ["GET /marketplace-api/v1/accounts"]],
  ["GET /shop-api/v1/activity", ["GET /marketplace-api/v1/activity"]],
  ["GET /shop-api/v1/bids", ["GET /marketplace-api/v1/bids"]],
  ["GET /shop-api/v1/captcha", ["GET /credits/captcha"]],
  ["GET /shop-api/v1/cart", ["GET /credits/cart"]],
  ["GET /shop-api/v1/catalog", ["GET /marketplace-api/v1/catalog"]],
  ["GET /shop-api/v1/checkouts/{}", ["GET /credits/checkout/{}"]],
  ["GET /shop-api/v1/collections", ["GET /marketplace-api/v1/collections"]],
  ["GET /shop-api/v1/contracts", ["GET /marketplace-api/v1/contracts"]],
  ["GET /shop-api/v1/coupons", ["GET /marketplace-api/v1/coupons"]],
  ["GET /shop-api/v1/coupons/{}", ["GET /marketplace-api/v1/coupons/{}"]],
  ["GET /shop-api/v1/credit-program/season", ["GET /credits/season"]],
  ["GET /shop-api/v1/credit-program/seasons", ["GET /credits/seasons"]],
  ["GET /shop-api/v1/credit-topups/mana/balance/{}", ["GET /transactions-api/v1/payments/balance/{}", "GET /v1/payments/balance/{}"]],
  ["GET /shop-api/v1/credit-topups/mana/config", ["GET /transactions-api/v1/payments/config", "GET /v1/payments/config"]],
  ["GET /shop-api/v1/credit-topups/mana/nonce/{}", ["GET /transactions-api/v1/payments/nonce/{}", "GET /v1/payments/nonce/{}"]],
  ["GET /shop-api/v1/credit-topups/mana/quote", ["GET /credits/topup/mana/quote"]],
  ["GET /shop-api/v1/items", ["GET /marketplace-api/v1/items"]],
  ["GET /shop-api/v1/lists", ["GET /marketplace-api/v1/lists"]],
  ["GET /shop-api/v1/lists/{}", ["GET /marketplace-api/v1/lists/{}"]],
  ["GET /shop-api/v1/lists/{}/picks", ["GET /marketplace-api/v1/lists/{}/picks"]],
  ["GET /shop-api/v1/names/credit-route", ["GET /credits/credits-name-route"]],
  ["GET /shop-api/v1/nfts", ["GET /marketplace-api/v1/nfts"]],
  ["GET /shop-api/v1/notify-requests", ["GET <shop-server host>/notify-requests"]],
  ["GET /shop-api/v1/orders", ["GET /marketplace-api/v1/orders"]],
  ["GET /shop-api/v1/orders/open-by-items", ["GET /marketplace-api/v1/orders/open-by-items"]],
  ["GET /shop-api/v1/outfits", ["GET <shop-server host>/v1/outfits"]],
  ["GET /shop-api/v1/outfits/all", ["GET <shop-server host>/v1/outfits/all"]],
  ["GET /shop-api/v1/outfits/thumbnails/{}", ["GET <shop-server host>/v1/outfits/thumbnails/{}"]],
  ["GET /shop-api/v1/outfits/{}", ["GET <shop-server host>/v1/outfits/{}"]],
  ["GET /shop-api/v1/owners", ["GET /marketplace-api/v1/owners"]],
  ["GET /shop-api/v1/pack-orders/{}", ["GET /credits/credits/orders/{}"]],
  ["GET /shop-api/v1/pack-orders/{}/cancel-return", ["GET /credits/credits/orders/{}/cancel-return"]],
  ["GET /shop-api/v1/packs", ["GET /credits/credits/packs", "GET /credits/packs"]],
  ["GET /shop-api/v1/picks/stats", ["GET /marketplace-api/v1/picks/stats"]],
  ["GET /shop-api/v1/picks/{}", ["GET /marketplace-api/v1/picks/{}"]],
  ["GET /shop-api/v1/picks/{}/stats", ["GET /marketplace-api/v1/picks/{}/stats"]],
  ["GET /shop-api/v1/prices", ["GET /marketplace-api/v1/prices"]],
  ["GET /shop-api/v1/rankings/{}/{}", ["GET /marketplace-api/v1/rankings/{}/{}"]],
  ["GET /shop-api/v1/rental-listings/prices", ["GET /signatures-api/v1/rental-listings/prices", "GET /v1/rental-listings/prices"]],
  ["GET /shop-api/v1/rentals-listings", ["GET /signatures-api/v1/rentals-listings", "GET /v1/rentals-listings"]],
  ["GET /shop-api/v1/sales", ["GET /marketplace-api/v1/sales"]],
  ["GET /shop-api/v1/sales/summary", ["GET /marketplace-api/v1/sales/summary"]],
  ["GET /shop-api/v1/stats/{}/{}", ["GET /marketplace-api/v1/stats/{}/{}"]],
  ["GET /shop-api/v1/trades", ["GET /marketplace-api/v1/trades", "GET /v1/trades"]],
  ["GET /shop-api/v1/trades-mirror", ["GET /marketplace-api/v1/trades-mirror", "GET /v1/trades-mirror"]],
  ["GET /shop-api/v1/trades/{}", ["GET /marketplace-api/v1/trades/{}", "GET /v1/trades/{}"]],
  ["GET /shop-api/v1/trades/{}/accept", ["GET /marketplace-api/v1/trades/{}/accept"]],
  ["GET /shop-api/v1/transak/orders/{}", ["GET /marketplace-api/v1/transak/orders/{}"]],
  ["GET /shop-api/v1/trendings", ["GET /marketplace-api/v1/trendings"]],
  ["GET /shop-api/v1/users/{}/credit-orders", ["GET /credits/users/{}/credit-orders"]],
  ["GET /shop-api/v1/users/{}/credits", ["GET /credits/users/{}/credits", "GET /credits/wallet/{}/balance"]],
  ["GET /shop-api/v1/users/{}/credits/stream", ["GET /credits/users/{}/credits/stream"]],
  ["GET /shop-api/v1/users/{}/emotes", ["GET /marketplace-api/v1/users/{}/emotes"]],
  ["GET /shop-api/v1/users/{}/emotes/grouped", ["GET /marketplace-api/v1/users/{}/emotes/grouped"]],
  ["GET /shop-api/v1/users/{}/emotes/urn-token", ["GET /marketplace-api/v1/users/{}/emotes/urn-token"]],
  ["GET /shop-api/v1/users/{}/names", ["GET /marketplace-api/v1/users/{}/names"]],
  ["GET /shop-api/v1/users/{}/names/names-only", ["GET /marketplace-api/v1/users/{}/names/names-only"]],
  ["GET /shop-api/v1/users/{}/progress", ["GET /credits/users/{}/progress"]],
  ["GET /shop-api/v1/users/{}/purchases", ["GET /credits/users/{}/purchases"]],
  ["GET /shop-api/v1/users/{}/status", ["GET /credits/users/{}/status"]],
  ["GET /shop-api/v1/users/{}/wearables", ["GET /marketplace-api/v1/users/{}/wearables"]],
  ["GET /shop-api/v1/users/{}/wearables/grouped", ["GET /marketplace-api/v1/users/{}/wearables/grouped"]],
  ["GET /shop-api/v1/users/{}/wearables/urn-token", ["GET /marketplace-api/v1/users/{}/wearables/urn-token"]],
  ["GET /shop-api/v1/volume/{}", ["GET /marketplace-api/v1/volume/{}"]],
  ["GET /shop-api/v2/catalog", ["GET /marketplace-api/v2/catalog"]],
  ["GET /shop-api/v2/credit-program/seasons", ["GET /credits/v2/seasons"]],
  ["GET /shop-api/v3/catalog/creators", ["GET /marketplace-api/v3/catalog/creators"]],
  ["GET /shop-api/v3/catalog/creators/search", ["GET /marketplace-api/v3/catalog/creators/search"]],
  ["GET /shop-api/v3/catalog/importable", ["GET /marketplace-api/v3/catalog/importable"]],
  ["GET /shop-api/v3/catalog/items", ["GET /marketplace-api/v3/catalog/items"]],
  ["GET /shop-api/v3/catalog/legacy", ["GET /marketplace-api/v3/catalog/legacy"]],
  ["GET /shop-api/v3/catalog/related", ["GET /marketplace-api/v3/catalog/related"]],
  ["GET /shop-api/v3/catalog/shop", ["GET /marketplace-api/v3/catalog/shop"]],
  ["GET /shop-api/v3/catalog/suggest", ["GET /marketplace-api/v3/catalog/suggest"]],
  ["GET /shop-api/v3/catalog/suggested", ["GET /marketplace-api/v3/catalog/suggested"]],
  ["GET /shop-api/v3/catalog/trending", ["GET /marketplace-api/v3/catalog/trending"]],
  ["GET /shop-api/v3/catalog/unified", ["GET /marketplace-api/v3/catalog/unified"]],
  ["PATCH /shop-api/v1/rentals-listings/{}", ["PATCH /signatures-api/v1/rentals-listings/{}", "PATCH /v1/rentals-listings/{}"]],
  ["POST /shop-api/v1/captcha", ["POST /credits/captcha"]],
  ["POST /shop-api/v1/cart/items", ["POST /credits/cart/items"]],
  ["POST /shop-api/v1/checkouts", ["POST /credits/checkout"]],
  ["POST /shop-api/v1/coupons", ["POST /marketplace-api/v1/coupons"]],
  ["POST /shop-api/v1/credit-authorizations", ["POST /credits/credits/authorize"]],
  ["POST /shop-api/v1/credit-authorizations/batch", ["POST /credits/credits/authorize/batch"]],
  ["POST /shop-api/v1/credit-authorizations/cancel", ["POST /credits/credits/authorize/cancel"]],
  ["POST /shop-api/v1/credit-authorizations/publication", ["POST /credits/credits/authorize-publication"]],
  ["POST /shop-api/v1/credit-authorizations/submitted", ["POST /credits/credits/authorize/submitted"]],
  ["POST /shop-api/v1/credit-quotes", ["POST /credits/prices/quote"]],
  ["POST /shop-api/v1/credit-topups/mana", ["POST /credits/topup/mana"]],
  ["POST /shop-api/v1/iap/quote", ["POST /credits/credits/iap/quote"]],
  ["POST /shop-api/v1/iap/register", ["POST /credits/credits/iap/register"]],
  ["POST /shop-api/v1/iap/verify", ["POST /credits/credits/iap/verify"]],
  ["POST /shop-api/v1/lists", ["POST /marketplace-api/v1/lists"]],
  ["POST /shop-api/v1/lists/{}/access", ["POST /marketplace-api/v1/lists/{}/access"]],
  ["POST /shop-api/v1/lists/{}/picks", ["POST /marketplace-api/v1/lists/{}/picks"]],
  ["POST /shop-api/v1/notify-requests", ["POST <shop-server host>/notify-requests"]],
  ["POST /shop-api/v1/outfits", ["POST <shop-server host>/v1/outfits"]],
  ["POST /shop-api/v1/outfits/thumbnails", ["POST <shop-server host>/v1/outfits/thumbnails"]],
  ["POST /shop-api/v1/pack-orders", ["POST /credits/credits/checkout"]],
  ["POST /shop-api/v1/pack-orders/{}/cancel", ["POST /credits/credits/orders/{}/cancel"]],
  ["POST /shop-api/v1/pack-orders/{}/resume", ["POST /credits/credits/orders/{}/resume"]],
  ["POST /shop-api/v1/packs/{}/payment-intents", ["POST /credits/packs/{}/intent"]],
  ["POST /shop-api/v1/picks/{}", ["POST /marketplace-api/v1/picks/{}"]],
  ["POST /shop-api/v1/rentals-listings", ["POST /signatures-api/v1/rentals-listings", "POST /v1/rentals-listings"]],
  ["POST /shop-api/v1/trades", ["POST /marketplace-api/v1/trades"]],
  ["POST /shop-api/v1/transak/widget-url", ["POST /marketplace-api/v1/transak/widget-url"]],
  ["POST /shop-api/v1/users", ["POST /credits/users"]],
  ["POST /shop-api/v1/wert/sign", ["POST /marketplace-api/v1/wert/sign"]],
  ["PUT /shop-api/v1/lists/{}", ["PUT /marketplace-api/v1/lists/{}"]],
  ["PUT /shop-api/v1/outfits/{}", ["PUT <shop-server host>/v1/outfits/{}"]],
];

function norm(path: string): string {
  return path.replace(/\{[A-Za-z_]+\}/g, "{}").replace(/:[A-Za-z_]+/g, "{}");
}

function shopUiRoutes(): Map<string, Set<string>> {
  const routes = new Map(SHOP_UI_ROUTES.map(([route, aliases]) => [route, new Set(aliases)]));
  const quoteSource = readFileSync(join(__dirname, "../../../../../services/catalyst/shop/src/mana/quote.rs"), "utf8");
  expect(quoteSource).toContain('pub const ROUTE: &str = "/v1/purchase-offers";');
  routes.set("POST /shop-api/v1/purchase-offers", new Set());
  return routes;
}

describe("the endpoint table is the pinned canonical namespace", () => {
  const routes = shopUiRoutes();
  const table = Object.values(ENDPOINTS).map((e) => ({ key: `${e.method} ${norm(`/shop-api${e.path}`)}`, e }));

  it("every pinned Shop UI route has an endpoint, and no endpoint is invented", () => {
    const listed = new Set(table.map((t) => t.key));
    expect([...routes.keys()].filter((k) => !listed.has(k))).toEqual([]);
    expect(table.map((t) => t.key).filter((k) => !routes.has(k))).toEqual([]);
  });

  it("each legacy alias is one the pinned routes name", () => {
    for (const { key, e } of table) {
      const aliases = routes.get(key)!;
      if (e.legacy === null) {
        expect([...aliases].every((a) => a.includes("<shop-server host>")), key).toBe(true);
      } else {
        expect(aliases, key).toContain(`${e.method} ${norm(e.legacy)}`);
      }
    }
  });
});

function identity(): AuthIdentity {
  const signer = generateEphemeralKey();
  return {
    signer: signer.address,
    ephemeral: generateEphemeralKey(),
    expiration: new Date(Date.now() + 60_000).toISOString(),
    authChain: [{ type: "SIGNER", payload: signer.address, signature: "" }],
  };
}

function recorder(status = 200, body = "{}") {
  const log: TransportRequest[] = [];
  return { log, transport: async (req: TransportRequest) => (log.push(req), { status, body }) };
}

describe("the client", () => {
  it("calls the canonical path, or the legacy alias behind the flag", async () => {
    const canonical = recorder();
    await createShopApi({ base: "https://node.test", transport: canonical.transport }).request(ENDPOINTS.userNames, {
      params: { address: "0xabc" },
      query: { first: 5, skip: undefined },
    });
    expect(canonical.log[0]!.url).toBe("https://node.test/shop-api/v1/users/0xabc/names?first=5");
    const legacy = recorder();
    await createShopApi({ base: "https://node.test", mode: "legacy", transport: legacy.transport }).request(ENDPOINTS.packs);
    expect(legacy.log[0]!.url).toBe("https://node.test/credits/credits/packs");
    expect(() => createShopApi({ mode: "legacy" }).path(ENDPOINTS.outfits)).toThrow(ShopApiError);
  });

  it("signs over the method and the exact path requested, alias included", async () => {
    const who = identity();
    for (const mode of ["canonical", "legacy"] as const) {
      const rec = recorder();
      const api = createShopApi({ base: "https://node.test", mode, signer: identitySigner(who), transport: rec.transport });
      await api.request(ENDPOINTS.authorizeCredit, { body: { usdPriceCents: 200 } });
      const headers = rec.log[0]!.headers;
      const chain = Object.keys(headers).filter((k) => k.startsWith("x-identity-auth-chain-"));
      expect(chain).toHaveLength(2);
      const signed = JSON.parse(headers["x-identity-auth-chain-1"]!) as { type: string; payload: string };
      const path = mode === "canonical" ? "/shop-api/v1/credit-authorizations" : "/credits/credits/authorize";
      expect(signed.type).toBe("ECDSA_SIGNED_ENTITY");
      expect(signed.payload).toBe(`post:${path}:${headers["x-identity-timestamp"]}:{}`);
    }
  });

  it("a signed route without a session asks for sign-in; an optional one goes unsigned", async () => {
    const rec = recorder();
    const api = createShopApi({ transport: rec.transport, signer: null });
    await expect(api.request(ENDPOINTS.userCredits, { params: { address: "0xabc" } })).rejects.toMatchObject({ status: 401, code: "signInRequired" });
    await api.request(ENDPOINTS.nfts);
    expect(rec.log[0]!.signed).toBe(false);
  });

  it("marketplace writes carry the signed intent metadata upstream requires", async () => {
    const seen: Record<string, unknown>[] = [];
    const signer: ShopSigner = { address: "0xabc", sign: async (_m, _p, metadata) => (seen.push(metadata), {}) };
    const client = createShopClient({ signer, transport: recorder().transport });
    await client.market.createTrade({});
    await client.market.activity();
    expect(seen).toEqual([{ signer: "dcl:marketplace", intent: "dcl:create-trade" }, { signer: "dcl:marketplace" }]);
  });
});

describe("fixture-backed mocks (tests/fixtures/shop/credits-server@465e628bc/wire)", () => {
  const signer: ShopSigner = { address: FIXTURE_BUYER, sign: async () => ({}) };
  const now = () => 1_800_000_000_000;

  it("the generated fixture file is current", () => {
    expect(readFileSync(OUTPUT as string, "utf8")).toBe((build as () => string)());
  });

  it("answers the authorization routes with the pinned cases", async () => {
    const client = createShopClient({ signer, transport: fixtureTransport({ now }) });
    const one = await client.authorizations.authorize({ usdPriceCents: 200, tradeId: "trade-99" });
    expect(one).toMatchObject({ usdCents: 200, maxCreditedValue: "1851851835000000000", credit: { signature: "0xsignature" } });
    expect(one.credit.expiresAt).toBe(1_800_000_000 + 900);
    expect(one.credit.id).toMatch(/^0x[0-9a-f]{64}$/);

    const group = await client.authorizations.authorizeGroup({
      items: [
        { usdPriceCents: 110, contractAddress: "0x2cf05cd6b8c7a6eb9fc98eb0499dbc8837c1d58e", itemId: "2" },
        { usdPriceCents: 65, contractAddress: "0x2cf05cd6b8c7a6eb9fc98eb0499dbc8837c1d58e", itemId: "3" },
        { usdPriceCents: 90, tradeId: "trade-9" },
      ],
    });
    expect(group.usdCents).toBe(270);
    expect(group.lines.map((l) => l.usdCents)).toEqual([110, 70, 90]);

    const salts = ["0x" + "a1".repeat(32), "0x" + "b2".repeat(32)];
    expect(await client.authorizations.cancel({ salts })).toEqual({ released: 2, retired: 2, spendableAtSeconds: 1800000000 });
    expect(await client.authorizations.submitted({ salts, txHash: "0x" + "d4".repeat(32) })).toEqual({ recorded: 2 });
  });

  it("the refusals are typed: 402 insufficient, 404 disabled, 409 too many live", async () => {
    const at = (scenario: string) =>
      createShopClient({ signer, transport: fixtureTransport({ scenario: { authorize: scenario } }) }).authorizations.authorize({ usdPriceCents: 200 });
    expect(insufficientCredits(await at("balance below the rounded price").catch((e: unknown) => e))).toEqual({ balanceCents: 150, requiredCents: 200 });
    expect(creditsDisabled(await at("USD credits flag off").catch((e: unknown) => e))).toBe(true);
    expect(tooManyLiveCredits(await at("too many live credits").catch((e: unknown) => e))).toBe(true);
  });

  it("the legacy alias is answered by the same cases; anything unpinned is 501", async () => {
    const legacy = createShopClient({ mode: "legacy", signer, transport: fixtureTransport({ now }) });
    expect((await legacy.authorizations.authorize({ usdPriceCents: 184 })).usdCents).toBe(190);
    await expect(legacy.authorizations.authorize({ usdPriceCents: 7 })).rejects.toMatchObject({ status: 501 });
  });
});

describe("the same fixtures as MSW handlers", () => {
  it("answers the canonical path and the alias over fetch", async () => {
    const server = setupServer(...wireHandlers({ now: () => 1_800_000_000_000 }));
    server.listen({ onUnhandledFrame: "error" });
    try {
      const signer: ShopSigner = { address: FIXTURE_BUYER, sign: async () => ({ "x-identity-auth-chain-0": "{}" }) };
      for (const mode of ["canonical", "legacy"] as const) {
        const client = createShopClient({ base: "https://node.test", mode, signer });
        expect((await client.authorizations.authorize({ usdPriceCents: 200, tradeId: "trade-99" })).usdCents).toBe(200);
      }
    } finally {
      server.close();
    }
  });
});
