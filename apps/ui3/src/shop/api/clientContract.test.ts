// The client's wire contract, pinned before its host wiring moves out: the exact URL, signed path and signer
// arguments for each endpoint kind in both modes, who signs, and what reaches the transport. A change to how
// the client is configured must leave every one of these as it is.
import { describe, expect, it } from "vitest";
import { createShopApi, isShopApiError } from "./client";
import { ENDPOINTS } from "@shop/api/endpoints";
import type { ShopSigner, ShopTransport, TransportRequest } from "./signer";

function recorder(): { transport: ShopTransport; log: TransportRequest[] } {
  const log: TransportRequest[] = [];
  return { log, transport: async (req) => (log.push(req), { status: 200, body: "{}" }) };
}

function signerAs(address: string): ShopSigner & { calls: [string, string, Record<string, unknown>][] } {
  const calls: [string, string, Record<string, unknown>][] = [];
  return { address, calls, sign: async (method, path, metadata) => (calls.push([method, path, metadata]), { "x-test-signed": `${address} ${method} ${path}` }) };
}

const ESCAPED_ID = "0xAbC d+e%f/é";
const QUERY = { q: "a b+c%d/é&x=1", tags: ["x y", "z+"], empty: "", nul: null, und: undefined, n: 0, b: false };
const ID_PATH = "0xAbC%20d%2Be%25f%2F%C3%A9";
const QUERY_STRING = "?q=a+b%2Bc%25d%2F%C3%A9%26x%3D1&tags=x+y&tags=z%2B&n=0&b=false";

describe("the client's wire contract", () => {
  for (const [mode, prefix, trades] of [
    ["canonical", "/shop-api", "/v1/trades"],
    ["legacy", "/marketplace-api", "/v1/trades"],
  ] as const) {
    it(`builds the same URLs, signed paths and signer arguments (${mode})`, async () => {
      const rec = recorder();
      const signer = signerAs("0xabc");
      const api = createShopApi({ base: "https://node.test", mode, signer, transport: rec.transport });
      await api.request(ENDPOINTS.trade, { params: { id: ESCAPED_ID }, query: QUERY });
      await api.request(ENDPOINTS.createTrade, { body: { usdPriceCents: 200, note: "é" }, metadata: { intent: "x" } });
      await api.request(ENDPOINTS.catalogSuggested, { query: { first: 2 } });
      await api.request(ENDPOINTS.catalogShop);
      expect(rec.log.map((r) => [r.method, r.url, r.signPath, r.signed, r.body ?? null])).toEqual([
        ["GET", `https://node.test${prefix}${trades}/${ID_PATH}${QUERY_STRING}`, `${prefix}${trades}/${ID_PATH}`, false, null],
        ["POST", `https://node.test${prefix}${trades}`, `${prefix}${trades}`, true, '{"usdPriceCents":200,"note":"é"}'],
        ["GET", `https://node.test${prefix}/v3/catalog/suggested?first=2`, `${prefix}/v3/catalog/suggested`, true, null],
        ["GET", `https://node.test${prefix}/v3/catalog/shop`, `${prefix}/v3/catalog/shop`, false, null],
      ]);
      // The query never enters the signed path; the signer gets the method, that path and the metadata.
      expect(signer.calls).toEqual([
        ["POST", `${prefix}${trades}`, { intent: "x" }],
        ["GET", `${prefix}/v3/catalog/suggested`, {}],
      ]);
      expect(rec.log[0]!.headers).toEqual({ accept: "application/json" });
      expect(rec.log[1]!.headers).toEqual({
        accept: "application/json",
        "content-type": "application/json",
        "x-test-signed": `0xabc POST ${prefix}${trades}`,
      });
    });
  }

  it("drops one trailing slash from the base it is given", async () => {
    const rec = recorder();
    await createShopApi({ base: "https://node.test/", mode: "canonical", signer: null, transport: rec.transport }).request(ENDPOINTS.catalogShop);
    expect(rec.log[0]!.url).toBe("https://node.test/shop-api/v3/catalog/shop");
  });

  it("a public route never signs, even with a signer at hand", async () => {
    const rec = recorder();
    const signer = signerAs("0xabc");
    await createShopApi({ base: "https://node.test", mode: "canonical", signer, transport: rec.transport }).request(ENDPOINTS.catalogShop);
    expect(signer.calls).toEqual([]);
    expect(rec.log[0]!.signed).toBe(false);
  });

  it("a signed route without a signer fails before anything reaches the transport", async () => {
    const rec = recorder();
    const api = createShopApi({ base: "https://node.test", mode: "canonical", signer: null, transport: rec.transport });
    await expect(api.request(ENDPOINTS.createTrade, { body: {} })).rejects.toMatchObject({ status: 401, code: "signInRequired" });
    expect(rec.log).toEqual([]);
  });

  it("two clients with their own base and signer never borrow each other's", async () => {
    const a = recorder();
    const b = recorder();
    const one = createShopApi({ base: "https://one.test", mode: "canonical", signer: signerAs("0x1"), transport: a.transport });
    const two = createShopApi({ base: "https://two.test", mode: "legacy", signer: signerAs("0x2"), transport: b.transport });
    await Promise.all([one.request(ENDPOINTS.catalogSuggested), two.request(ENDPOINTS.catalogSuggested), one.request(ENDPOINTS.catalogShop)]);
    // Signing is async, so the order across requests is not part of the contract.
    expect(a.log.map((r) => [r.url, r.headers["x-test-signed"] ?? null]).sort()).toEqual([
      ["https://one.test/shop-api/v3/catalog/shop", null],
      ["https://one.test/shop-api/v3/catalog/suggested", "0x1 GET /shop-api/v3/catalog/suggested"],
    ]);
    expect(b.log.map((r) => [r.url, r.headers["x-test-signed"] ?? null])).toEqual([
      ["https://two.test/marketplace-api/v3/catalog/suggested", "0x2 GET /marketplace-api/v3/catalog/suggested"],
    ]);
  });

  it("hands the transport the caller's own signal", async () => {
    const rec = recorder();
    const signal = new AbortController().signal;
    await createShopApi({ base: "https://node.test", mode: "canonical", signer: null, transport: rec.transport }).request(ENDPOINTS.catalogShop, { signal });
    expect(rec.log[0]!.signal).toBe(signal);
  });

  it("an abort stays an abort; any other transport failure is a network error", async () => {
    const abort = Object.assign(new Error("aborted"), { name: "AbortError" });
    const aborting: ShopTransport = async () => {
      throw abort;
    };
    const api = (transport: ShopTransport) => createShopApi({ base: "https://node.test", mode: "canonical", signer: null, transport });
    await expect(api(aborting).request(ENDPOINTS.catalogShop)).rejects.toBe(abort);
    const failing: ShopTransport = async () => {
      throw new TypeError("Failed to fetch");
    };
    const error = await api(failing).request(ENDPOINTS.catalogShop).catch((e: unknown) => e);
    expect(isShopApiError(error)).toBe(true);
    expect(error).toMatchObject({ status: 0, code: "network", url: "https://node.test/shop-api/v3/catalog/shop" });
  });
});
