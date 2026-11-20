import { afterEach, describe, expect, it, onTestFinished, vi } from "vitest";

import { createShopClient, fetchTransport } from "@ui/shop/api";
import { REMOTE_FLAG, remoteFlagVariant } from "@ui/shop/discovery/lib/remoteFlags";

import { resetDappsFlags } from "../feature-flags.server";
import { serverShopApiMode } from "./mode";
import { defaultShopSeo, shopMetaDescriptors } from "./pages";
import { loadShopPage, readBodyThenWait, serverShopClient, SHOP_SSR_SHARE_MS, SHOP_SSR_STALE_MS } from "./pages.server";
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
  return { asked, fetchImpl, client: createShopClient({ base: "http://node.test", mode: "canonical", signer: null, transport: fetchTransport(fetchImpl) }) };
}

const request = (path: string) => new Request(`https://sites.test${path}`);
const now = () => 1_700_000_000_000;

const ITEM_ROUTES: Route[] = [
  { path: "/shop-api/v1/items", body: { data: [{ name: "Pirate Hat", thumbnail: "https://img.test/hat.png", creator: "0x1", rarity: "rare", category: "wearable", chainId: 137, network: "MATIC", available: 3, data: { wearable: { description: " A hat for pirates. " } } }], total: 1 } },
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
      ["collection-meta", C],
    ]);
    expect(data.queries.find(([key]) => key[0] === "item-meta")?.[1]).toMatchObject({ description: "A hat for pirates." });
    // The description comes with the item's metadata: no catalogue read for it.
    expect(asked.some((u) => u.startsWith("/shop-api/v2/catalog"))).toBe(false);
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

describe("the page's flags, read by the server", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
    resetDappsFlags();
  });

  it("starts the item reads with the flags read, and maps an answer that arrives first with the page's flags", async () => {
    // Creator sales off from an earlier page: an answer mapped before this page's flags would lose its sale price.
    await loadShopPage("cart", { request: request("/shop/cart"), params: {}, client: node([]).client, now, env: {}, flags: {} });
    resetDappsFlags();
    const onSale = { listingType: "primary", contractAddress: C, itemId: "7", name: "Pirate Hat", priceCredits: 5, compareAtCredits: 7 };
    const routes = [...ITEM_ROUTES.filter((r) => r.path !== "/shop-api/v3/catalog/unified"), { path: "/shop-api/v3/catalog/unified", body: { data: [onSale], total: 1 } }];
    const asked: string[] = [];
    let answerFlags: (res: Response) => void = () => {};
    vi.stubGlobal("fetch", async (input: RequestInfo | URL) => {
      const url = new URL(String(input));
      if (url.pathname === "/dapps.json") return new Promise<Response>((resolve) => (answerFlags = resolve));
      asked.push(url.pathname);
      const hit = routes.find((r) => r.path === url.pathname);
      return new Response(JSON.stringify(hit?.body ?? {}), { status: hit ? 200 : 404 });
    });

    const load = loadShopPage("itemDetail", { request: request(`/shop/item/${C}/7`), params: { contractAddress: C, itemId: "7" }, now, env: { CATALYST_URL: "http://node.test" } });
    await vi.waitFor(() => expect(asked).toHaveLength(routes.length));
    answerFlags(new Response(JSON.stringify({ flags: { "dapps-shop-creator-sales": true } })));
    const { data } = await load;
    expect(data.featureFlags).toEqual({ "dapps-shop-creator-sales": true });
    expect(data.queries.find(([key]) => key[0] === "shop-item")?.[1]).toMatchObject({ priceCredits: 5, compareAtCredits: 7 });
  });

  it("hands on an answer with no body (204, 304) as it came, and any other with its body", async () => {
    for (const status of [204, 304]) {
      const answer = await readBodyThenWait(async () => new Response(null, { status }), Promise.resolve())("http://node.test/x");
      expect(answer.status).toBe(status);
      expect(await answer.text()).toBe("");
    }
    const ok = await readBodyThenWait(async () => new Response('{"a":1}', { status: 200, headers: { "content-type": "application/json" } }), Promise.resolve())("http://node.test/x");
    expect(ok.status).toBe(200);
    expect(ok.headers.get("content-type")).toBe("application/json");
    expect(await ok.json()).toEqual({ a: 1 });
  });

  it("reads each answer's body under its own deadline before it waits for the flags", async () => {
    // Every read's deadline, run out by the test while the flags are still unanswered.
    const deadlines: AbortController[] = [];
    const timeout = vi.spyOn(AbortSignal, "timeout").mockImplementation(() => {
      const controller = new AbortController();
      deadlines.push(controller);
      return controller.signal;
    });
    onTestFinished(() => timeout.mockRestore());
    const item = "0xabc0000000000000000000000000000000000006";
    const asked: string[] = [];
    let answerFlags: (res: Response) => void = () => {};
    vi.stubGlobal("fetch", async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = new URL(String(input));
      if (url.pathname === "/dapps.json") return new Promise<Response>((resolve) => (answerFlags = resolve));
      asked.push(url.pathname);
      const hit = ITEM_ROUTES.find((r) => r.path === url.pathname);
      const text = JSON.stringify(hit?.body ?? {});
      // As a real fetch's: a body not read yet fails once the request's signal aborts.
      let read = false;
      const body = new ReadableStream<Uint8Array>({
        start(controller) {
          init?.signal?.addEventListener("abort", () => {
            if (!read) controller.error(init.signal!.reason);
          });
        },
        pull(controller) {
          read = true;
          controller.enqueue(new TextEncoder().encode(text));
          controller.close();
        },
      }, { highWaterMark: 0 });
      return new Response(body, { status: hit ? 200 : 404 });
    });

    const load = loadShopPage("itemDetail", { request: request(`/shop/item/${item}/7`), params: { contractAddress: item, itemId: "7" }, now, env: { CATALYST_URL: "http://node.test" } });
    await vi.waitFor(() => expect(asked).toHaveLength(3));
    await new Promise((settled) => setTimeout(settled, 10));
    for (const deadline of deadlines) deadline.abort(new DOMException("Request deadline exceeded", "TimeoutError"));
    answerFlags(new Response(JSON.stringify({ flags: {} })));
    const { data, status } = await load;
    expect(status).toBe(200);
    expect(data.queries.map(([key]) => key[0])).toEqual(["shop-item", "item-meta", "collection-meta"]);
    expect(data.seo.title).toBe("Pirate Hat");
  });
});

describe("the item and collection documents' shared reads", () => {
  afterEach(() => vi.unstubAllGlobals());

  /** The server's own client (only it shares reads) against a node whose answers wait while `hold` is on. */
  function sharedNode(routes: Route[]) {
    const { asked, fetchImpl } = node(routes);
    let gate = Promise.resolve();
    let release = () => {};
    vi.stubGlobal("fetch", async (input: RequestInfo | URL) => {
      await gate;
      return fetchImpl(input);
    });
    return {
      asked,
      hold: () => (gate = new Promise((done) => (release = done))),
      release: () => release(),
    };
  }
  const env = { CATALYST_URL: "http://node.test" };

  it("read each public fact of an item once, then serve the last answer at once while one reload runs", async () => {
    const item = "0xabc0000000000000000000000000000000000002";
    const remote = sharedNode(ITEM_ROUTES);
    const t0 = 1_700_000_000_000;
    let t = t0;
    const load = () => loadShopPage("itemDetail", { request: request(`/shop/item/${item}/7`), params: { contractAddress: item, itemId: "7" }, now: () => t, env, flags: {} });
    const readAt = (page: Awaited<ReturnType<typeof load>>) => [...new Set(page.data.queries.map(([, , at]) => at))];

    const first = await load();
    expect(first.data.queries.map(([key]) => key[0])).toEqual(["shop-item", "item-meta", "collection-meta"]);
    expect(remote.asked).toHaveLength(3);
    t = t0 + SHOP_SSR_SHARE_MS - 1;
    expect((await load()).data.queries).toEqual(first.data.queries);
    expect(remote.asked).toHaveLength(3);

    // Past the share: both documents answer before the node does, with the first read's time, so the page's own
    // staleness rule still refetches the listing on mount; one reload per fact runs behind them.
    t = t0 + SHOP_SSR_SHARE_MS;
    remote.hold();
    const stale = await Promise.all([load(), load()]);
    expect(stale.map(readAt)).toEqual([[t0], [t0]]);
    remote.release();
    await vi.waitFor(async () => expect(readAt(await load())).toEqual([t0 + SHOP_SSR_SHARE_MS]));
    expect(remote.asked).toHaveLength(6);

    // Past the stale window too: the document waits for a new read.
    t = t0 + 2 * SHOP_SSR_SHARE_MS + SHOP_SSR_STALE_MS;
    expect(readAt(await load())).toEqual([t]);
    expect(remote.asked).toHaveLength(9);
  });

  it("a token's owner is read for every document; a collection page shares its read", async () => {
    const item = "0xabc0000000000000000000000000000000000003";
    const remote = sharedNode([
      ...ITEM_ROUTES,
      { path: "/shop-api/v1/nfts", body: { data: [{ nft: { id: `${item}-5`, contractAddress: item, tokenId: "5", name: "Pirate Hat #5", image: "https://img.test/5.png", category: "wearable", network: "MATIC", chainId: 137, owner: "0x2", data: {} }, order: null }], total: 1 } },
    ]);
    const token = () => loadShopPage("tokenDetail", { request: request(`/shop/token/${item}/5`), params: { contractAddress: item, tokenId: "5" }, now, env, flags: {} });
    await token();
    await token();
    const paths = remote.asked.map((u) => u.split("?")[0]);
    expect(paths.filter((u) => u === "/shop-api/v1/nfts")).toHaveLength(2);
    expect(paths.filter((u) => u === "/shop-api/v1/collections")).toHaveLength(1);

    const other = "0xabc0000000000000000000000000000000000004";
    const collection = () => loadShopPage("collection", { request: request(`/shop/collection/${other}`), params: { contractAddress: other }, now, env, flags: {} });
    const before = remote.asked.length;
    expect((await collection()).data.seo.title).toBe("Sea Things");
    expect((await collection()).data.queries).toEqual([[["collection-meta", other], expect.anything(), now()]]);
    expect(remote.asked.length - before).toBe(1);
  });

  it("a miss is not served past its share, and a malformed item id is read for its document only", async () => {
    const item = "0xabc0000000000000000000000000000000000005";
    const routes: Route[] = [
      { path: "/shop-api/v1/items", body: { data: [], total: 0 } },
      { path: "/shop-api/v3/catalog/unified", body: { data: [], total: 0 } },
      { path: "/shop-api/v1/collections", body: { data: [], total: 0 } },
    ];
    const remote = sharedNode(routes);
    const t0 = 1_700_000_000_000;
    let t = t0;
    const load = (itemId = "7") => loadShopPage("itemDetail", { request: request(`/shop/item/${item}/${itemId}`), params: { contractAddress: item, itemId }, now: () => t, env, flags: {} });
    expect((await load()).status).toBe(404);

    // Indexed since: past the share, the next document reads again instead of serving the miss.
    routes[0]!.body = { data: [{ name: "Pirate Hat", thumbnail: "https://img.test/hat.png", category: "wearable", chainId: 137, network: "MATIC", data: {} }], total: 1 };
    t = t0 + SHOP_SSR_SHARE_MS;
    const found = await load();
    expect(found.status).toBe(200);
    expect(found.data.seo.title).toBe("Pirate Hat");

    const before = remote.asked.length;
    await load("not-an-id");
    await load("not-an-id");
    expect(remote.asked.length - before).toBe(6);
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
