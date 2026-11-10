import { describe, expect, it } from "vitest";

import { createShopClient, fetchTransport } from "@ui/shop/api";
import { REMOTE_FLAG, remoteFlagVariant } from "@ui/shop/discovery/lib/remoteFlags";

import { serverShopApiMode } from "./mode";
import { defaultShopSeo, shopMetaDescriptors } from "./pages";
import { loadShopPage, serverShopClient } from "./pages.server";
import { LAND_NETWORKS } from "@shop/land/contracts";

const C = "0xabc0000000000000000000000000000000000001";

type Route = { path: string; status?: number; body: unknown };

/** A node that answers the given paths (query ignored) and records every URL asked for. */
function node(routes: Route[]) {
  const asked: string[] = [];
  const fetchImpl = (async (input: RequestInfo | URL) => {
    const url = new URL(String(input));
    asked.push(url.pathname + url.search);
    const hit = routes.find((r) => r.path === url.pathname);
    if (!hit) return new Response(JSON.stringify({ error: "not found" }), { status: 404 });
    return new Response(JSON.stringify(hit.body), { status: hit.status ?? 200 });
  }) as typeof fetch;
  return { asked, client: createShopClient({ base: "http://node.test", mode: "canonical", signer: null, transport: fetchTransport(fetchImpl) }) };
}

const request = (path: string) => new Request(`https://sites.test${path}`);
const now = () => 1_700_000_000_000;

const ITEM_ROUTES: Route[] = [
  { path: "/shop-api/v1/items", body: { data: [{ name: "Pirate Hat", thumbnail: "https://img.test/hat.png", creator: "0x1", rarity: "rare", category: "wearable", chainId: 137, network: "MATIC", available: 3 }], total: 1 } },
  { path: "/shop-api/v2/catalog", body: { data: [{ data: { wearable: { description: "A hat for pirates." } } }] } },
  { path: "/shop-api/v1/collections", body: { data: [{ contractAddress: C, name: "Sea Things", creator: "0x1" }], total: 1 } },
  { path: "/shop-api/v3/catalog/unified", body: { data: [], total: 0 } },
];

describe("the item page's SSR data", () => {
  it("reads the public item facts over the canonical /shop-api and primes them under the page's keys", async () => {
    const { asked, client } = node(ITEM_ROUTES);
    const { data, status } = await loadShopPage("itemDetail", { request: request(`/shop/item/${C.toUpperCase().replace("0X", "0x")}/7`), params: { contractAddress: C.toUpperCase().replace("0X", "0x"), itemId: "7" }, client, now, env: {} });
    expect(status).toBe(200);
    expect(asked.every((u) => u.startsWith("/shop-api/"))).toBe(true);
    expect(data.queries.map(([key]) => key)).toEqual([
      ["shop-item", C, "7", "primary"],
      ["item-meta", C, "7"],
      ["item-desc", C, "7"],
      ["collection-meta", C],
    ]);
    expect(data.queries.every(([, , at]) => at === now())).toBe(true);
    // The creator's sale only: a resale is the Marketplace's, decided on the client for a web3 session.
    const unified = asked.filter((u) => u.startsWith("/shop-api/v3/catalog/unified"));
    expect(unified.length).toBeGreaterThan(0);
    expect(unified.every((u) => new URLSearchParams(u.split("?")[1]).get("listingType") === "primary")).toBe(true);
    expect(data.seo).toMatchObject({
      title: "Pirate Hat",
      description: "A hat for pirates.",
      image: "https://img.test/hat.png",
      canonical: `https://sites.test/shop/item/${C}/7`,
      noindex: false,
    });
  });

  it("an item no read knows is a 404, but an index outage is not", async () => {
    const empty = node([
      { path: "/shop-api/v1/items", body: { data: [], total: 0 } },
      { path: "/shop-api/v3/catalog/unified", body: { data: [], total: 0 } },
      { path: "/shop-api/v1/collections", body: { data: [], total: 0 } },
      { path: "/shop-api/v2/catalog", body: { data: [] } },
    ]);
    const missing = await loadShopPage("itemDetail", { request: request(`/shop/item/${C}/9`), params: { contractAddress: C, itemId: "9" }, client: empty.client, now, env: {} });
    expect(missing.status).toBe(404);
    expect(missing.data.seo.noindex).toBe(true);

    const down = node([
      { path: "/shop-api/v1/items", status: 503, body: { error: "unavailable", source: "nft-indexer" } },
      { path: "/shop-api/v3/catalog/unified", status: 503, body: { error: "unavailable", source: "trades" } },
    ]);
    const outage = await loadShopPage("itemDetail", { request: request(`/shop/item/${C}/9`), params: { contractAddress: C, itemId: "9" }, client: down.client, now, env: {} });
    expect(outage.status).toBe(200);
    expect(outage.data.queries.map(([key]) => key[0])).not.toContain("item-meta");
  });

  it("a malformed address is a 404 without asking the node", async () => {
    const { asked, client } = node(ITEM_ROUTES);
    const res = await loadShopPage("itemDetail", { request: request("/shop/item/nope/1"), params: { contractAddress: "nope", itemId: "1" }, client, now, env: {} });
    expect(res.status).toBe(404);
    expect(asked).toEqual([]);
  });

  it("a token page reads the token and names it", async () => {
    const { client } = node([
      ...ITEM_ROUTES,
      { path: "/shop-api/v1/nfts", body: { data: [{ nft: { id: `${C}-5`, contractAddress: C, tokenId: "5", name: "Pirate Hat #5", image: "https://img.test/5.png", category: "wearable", network: "MATIC", chainId: 137, owner: "0x2", data: {} }, order: null }], total: 1 } },
    ]);
    const { data, status } = await loadShopPage("tokenDetail", { request: request(`/shop/token/${C}/5`), params: { contractAddress: C, tokenId: "5" }, client, now, env: {} });
    expect(status).toBe(200);
    expect(data.queries.map(([key]) => key[0])).toContain("token");
    expect(data.seo).toMatchObject({ title: "Pirate Hat #5", image: "https://img.test/5.png", canonical: `https://sites.test/shop/token/${C}/5` });
  });
});

describe("the other pages", () => {
  it("the overview primes its rails (no Deals rail while creator sales are off, upstream's default); a collection names itself", async () => {
    const { client } = node([
      { path: "/shop-api/v3/catalog/unified", body: { data: [], total: 0 } },
      { path: "/shop-api/v3/catalog/trending", body: { data: [] } },
      { path: "/shop-api/v1/collections", body: { data: [{ contractAddress: C, name: "dcl://halloween_2019", creator: "0x1" }] } },
    ]);
    const overview = await loadShopPage("overview", { request: request("/shop/overview"), params: {}, client, now, env: {} });
    expect(overview.data.queries.map(([key]) => key)).toEqual([["overview-listings"], ["overview-trending", "primary"]]);
    const collection = await loadShopPage("collection", { request: request(`/shop/collection/${C}`), params: { contractAddress: C }, client, now, env: {} });
    expect(collection.data.seo.title).toBe("Halloween 2019");
  });

  it("the overview reads its Best Deals rail once decentraland.org's creator-sales rollout is on, and sends the flags on", async () => {
    const { asked, client } = node([
      { path: "/shop-api/v3/catalog/unified", body: { data: [], total: 0 } },
      { path: "/shop-api/v3/catalog/trending", body: { data: [] } },
    ]);
    const flags = { "dapps-shop-creator-sales": true, "dapps-shop-suggested-for-you": false };
    const overview = await loadShopPage("overview", { request: request("/shop/overview"), params: {}, client, now, env: {}, flags });
    expect(overview.data.queries.map(([key]) => key)).toEqual([["overview-listings"], ["overview-trending", "primary"], ["overview-deals"]]);
    expect(asked.some((u) => u.includes("discounted=true"))).toBe(true);
    // The browser primes the same answer before it hydrates.
    expect(overview.data.featureFlags).toEqual(flags);

    const off = await loadShopPage("overview", { request: request("/shop/overview"), params: {}, client, now, env: {}, flags: {} });
    expect(off.data.queries.map(([key]) => key)).toEqual([["overview-listings"], ["overview-trending", "primary"]]);
    // No variants, no field: the page data stays as it was.
    expect("featureFlagVariants" in overview.data).toBe(false);
  });

  it("sends the flags' variants on with them, primed on the server as in the browser", async () => {
    const { client } = node([]);
    const list = "0x00000000000000000000000000000000000000aa";
    const flags = { "dapps-shop-my-store": true };
    const variants = { "dapps-shop-my-store": list };
    const page = await loadShopPage("myStore", { request: request("/shop/my-store"), params: {}, client, now, env: {}, flags, flagVariants: variants });
    expect(page.data.featureFlags).toEqual(flags);
    expect(page.data.featureFlagVariants).toEqual(variants);
    expect(remoteFlagVariant(REMOTE_FLAG.myStore)).toBe(list);
  });

  it("the collectibles grid primes its first page under the key the page asks for", async () => {
    const { asked, client } = node([
      { path: "/shop-api/v3/catalog/unified", body: { data: [], total: 0 } },
      { path: "/shop-api/v1/nfts", body: { data: [], total: 0 } },
    ]);
    const items = await loadShopPage("items", { request: request("/shop/items?category=wearable&sort=price-asc"), params: {}, client, now, env: {} });
    expect(items.data.queries).toHaveLength(1);
    expect(items.data.queries[0]![0]).toMatchObject(["shop-items", { category: "wearable", sortBy: "cheapest", onSale: true }]);
    expect(asked.at(-1)).toContain("first=48");
  });

  it("the Marketplace's public pages (LAND, an account, the rankings) read nothing and are never indexed", async () => {
    // Drawn only for a web3 session with the rollout on (ui3 market/access.tsx), which the server can't know.
    const { asked, client } = node([{ path: "/shop-api/v1/nfts", body: { data: [], total: 0 } }]);
    for (const [page, path] of [["lands", "/shop/lands"], ["account", "/shop/accounts/0x1"], ["rankings", "/shop/rankings"]] as const) {
      const res = await loadShopPage(page, { request: request(path), params: { address: "0x1" }, client, now, env: {} });
      expect(res.status).toBe(200);
      expect(res.data.queries).toEqual([]);
      expect(res.data.seo.noindex).toBe(true);
      expect(res.data.seo.canonical).toBeUndefined();
    }
    expect(asked).toEqual([]);
  });

  it("a LAND token's page is the Marketplace's: never indexed, with no canonical", async () => {
    const { client } = node([]);
    const land = Object.values(LAND_NETWORKS)[0]!.land.toLowerCase();
    const res = await loadShopPage("tokenDetail", { request: request(`/shop/token/${land}/1`), params: { contractAddress: land, tokenId: "1" }, client, now, env: {} });
    expect(res.data.seo.noindex).toBe(true);
    expect(res.data.seo.canonical).toBeUndefined();
  });

  it("an account page is never indexed and asks the node for nothing", async () => {
    const { asked, client } = node([]);
    for (const page of ["myItems", "cart", "activity", "credits"] as const) {
      const res = await loadShopPage(page, { request: request("/shop/x"), params: {}, client, now, env: {} });
      expect(res.status).toBe(200);
      expect(res.data.queries).toEqual([]);
    }
    expect(asked).toEqual([]);
    expect(shopMetaDescriptors(defaultShopSeo("cart"))).toContainEqual({ name: "robots", content: "noindex" });
    expect(shopMetaDescriptors(defaultShopSeo("items", "https://x.test/shop/items"))).toContainEqual({ tagName: "link", rel: "canonical", href: "https://x.test/shop/items" });
  });

  it("PUBLIC_CATALYST_URL names the canonical origin", async () => {
    const { client } = node([]);
    const res = await loadShopPage("names", { request: request("/shop/names?x=1"), params: {}, client, now, env: { PUBLIC_CATALYST_URL: "https://example.org/" } });
    expect(res.data.seo.canonical).toBe("https://example.org/shop/names");
  });
});

describe("the server's Shop client", () => {
  it("canonical /shop-api unless SHOP_API_MODE=legacy, against CATALYST_URL", () => {
    expect(serverShopApiMode({})).toBe("canonical");
    expect(serverShopApiMode({ SHOP_API_MODE: "legacy" })).toBe("legacy");
    expect(serverShopApiMode({ VITE_SHOP_API_MODE: "legacy" })).toBe("legacy");
    const legacy = serverShopClient({ CATALYST_URL: "http://node.test:5210", SHOP_API_MODE: "legacy" });
    expect([legacy.api.mode, legacy.api.base]).toEqual(["legacy", "http://node.test:5210"]);
    const canonical = serverShopClient({ CATALYST_URL: "http://node.test:5210" });
    expect(canonical.api.mode).toBe("canonical");
  });
});
