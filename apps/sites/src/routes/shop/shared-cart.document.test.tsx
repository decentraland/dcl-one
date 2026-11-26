// @vitest-environment node
import { Suspense, use } from "react";
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";

import { evmAddress } from "@shop/model/address";
import { itemAsset } from "@shop/model/asset";
import { CHAIN } from "@shop/model/chain";
import { createCartStore, type CartStorage } from "@ui/shop/cart/store";

import { testRequestHandler } from "@/core/lib/server-build.test-helper";

import * as entry from "../entry.server";
import * as root from "../root";
import * as shop from "../routes/shop";
import * as cart from "../routes/shop.cart";
import { SHARED_CART_CACHE_CONTROL, SHARED_CART_CDN_CACHE_CONTROL, SHARED_CART_VARY, sharedCartMiddleware } from "./shared-cart";

const A = "0xa11ce00000000000000000000000000000a11ce1";
const SID_A = "sid-of-visitor-a-5f0c";
const LINE_NAME = "Visitor A Velvet Hat";
const LINE_CONTRACT = "0x00000000000000000000000000000000c0ffee01";
const NAVIGATION = {
  accept: "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
  "sec-fetch-dest": "document",
  "sec-fetch-mode": "navigate",
  "sec-fetch-site": "none",
};

/** Visitor A's browser storage: the signed identity and a cart with one line. */
function storageOfA(): CartStorage {
  const data = new Map<string, string>();
  const storage: CartStorage = { getItem: (k) => data.get(k) ?? null, setItem: (k, v) => void data.set(k, v), removeItem: (k) => void data.delete(k) };
  storage.setItem("dcl:auth:identity:v1", JSON.stringify({ signer: A, expiration: "2099-01-01T00:00:00.000Z" }));
  const lines = createCartStore({ storage: () => storage });
  for (const account of [null, A]) {
    lines.scopeTo(account);
    lines.add({ asset: itemAsset(CHAIN.POLYGON, evmAddress(LINE_CONTRACT), 7n), listing: { acquisition: "store" }, display: { name: LINE_NAME } });
  }
  return storage;
}

/** The rollout flag service: one answer for everyone. */
const flags = (async (input: RequestInfo | URL) =>
  String(input).endsWith("/dapps.json") ? Response.json({ flags: { "dapps-shop-my-store": true } }) : new Response(null, { status: 404 })) as typeof fetch;

/** The pipeline `react-router-serve` runs (`createRequestHandler`) over the real entry, root, Shop layout and cart. */
const handler = testRequestHandler(entry, [
  { id: "root", path: "", module: root },
  { id: "routes/shop", parentId: "root", path: "shop", module: shop, loader: false },
  { id: "routes/shop.cart", parentId: "routes/shop", path: "cart", module: cart, clientLoader: true },
]);

/** A page whose render fails after the first part of the page is ready: its one boundary waits, then throws. */
let failing: Promise<never> | undefined;
function Late(): null {
  use((failing ??= new Promise<never>((_, reject) => setTimeout(() => reject(new Error("late render error")), 10))));
  return null;
}

/** The same pipeline over root and that page, with the shared cart's middleware. */
const lateHandler = testRequestHandler(entry, [
  { id: "root", path: "", module: root },
  {
    id: "routes/late",
    parentId: "root",
    path: "late",
    module: { middleware: [sharedCartMiddleware], loader: () => null, default: () => <Suspense fallback={null}><Late /></Suspense> },
  },
]);

type Answer = { response: Response; body: string };

/** A request of visitor A (wallet, session and language cookies), with A's storage reachable while it is answered. */
async function visitorA(path: string, headers: Record<string, string>): Promise<Answer> {
  vi.stubGlobal("localStorage", storageOfA());
  try {
    return await visitorB(path, { ...headers, cookie: `dcl_wallet=${A}; dcl_wallet_verified=${A}; sid=${SID_A}; lang=es` });
  } finally {
    vi.unstubAllGlobals();
    vi.stubGlobal("fetch", flags);
  }
}

/** A first visitor: no cookie. */
async function visitorB(path: string, headers: Record<string, string>): Promise<Answer> {
  const response = await handler(new Request(`https://dcl.test${path}`, { headers }));
  return { response, body: await response.text() };
}

const carries = (body: string, value: string) => body.toLowerCase().includes(value.toLowerCase());

beforeAll(() => vi.stubGlobal("fetch", flags));
afterEach(() => vi.unstubAllEnvs());

describe("when the shared cart setting is on", () => {
  beforeEach(() => {
    vi.stubEnv("SHOP_CART_SHARED_CACHE", "true");
    vi.stubEnv("PUBLIC_CATALYST_URL", "https://dcl.test");
  });

  describe("and a signed-in visitor with a cart line and a signed-out first visitor open the cart", () => {
    let a: Answer;
    let b: Answer;

    beforeEach(async () => {
      a = await visitorA("/shop/cart", NAVIGATION);
      b = await visitorB("/shop/cart", { ...NAVIGATION, "x-forwarded-proto": "http" });
    });

    it("should send both the same cart page, byte for byte, also when a request names another scheme", () => {
      expect(a.response.status).toBe(200);
      expect(a.body).toContain("shop-chrome");
      expect(a.body).toBe(b.body);
    });

    it("should carry the signed-in visitor's address, session id and cart line in neither body", () => {
      for (const body of [a.body, b.body]) {
        for (const secret of [A, SID_A, LINE_NAME, LINE_CONTRACT]) expect(carries(body, secret), secret).toBe(false);
      }
    });

    it("should let a shared cache keep both answers, with no cookie and no COEP", () => {
      for (const { response } of [a, b]) {
        expect(response.headers.get("Cache-Control")).toBe(SHARED_CART_CACHE_CONTROL);
        expect(response.headers.get("CDN-Cache-Control")).toBe(SHARED_CART_CDN_CACHE_CONTROL);
        expect(response.headers.get("Vary")).toBe(SHARED_CART_VARY);
        expect(response.headers.get("Set-Cookie")).toBeNull();
        expect(response.headers.get("Cross-Origin-Embedder-Policy")).toBeNull();
      }
    });
  });

  describe("and a navigation asks for Markdown", () => {
    it("should get the same shared page, so the answer does not vary on Accept", async () => {
      const markdown = await visitorB("/shop/cart", { ...NAVIGATION, accept: "text/markdown" });
      expect(markdown.response.headers.get("Cache-Control")).toBe(SHARED_CART_CACHE_CONTROL);
      expect(markdown.body).toBe((await visitorB("/shop/cart", NAVIGATION)).body);
    });
  });

  describe("and the signed-in visitor's client reads the cart data", () => {
    it("should keep the data private, with the visitor's session in it", async () => {
      const a = await visitorA("/shop/cart.data", { accept: "*/*", "sec-fetch-dest": "empty", "sec-fetch-site": "same-origin" });
      expect(a.response.headers.get("Cache-Control")).toBe("private, no-store");
      expect(carries(a.body, A) && carries(a.body, SID_A)).toBe(true);
    });
  });

  describe("and the Explorer frames the cart from the same origin", () => {
    it("should answer the private framed document, with COEP", async () => {
      const a = await visitorA("/shop/cart", { ...NAVIGATION, "sec-fetch-dest": "iframe", "sec-fetch-site": "same-origin" });
      expect(a.response.headers.get("Cache-Control")).toBe("private, no-store");
      expect(a.response.headers.get("Cross-Origin-Embedder-Policy")).toBe("credentialless");
      expect(carries(a.body, A)).toBe(true);
    });
  });

  describe("and the shared document's render fails after the first part of the page", () => {
    afterEach(() => {
      failing = undefined;
    });

    it("should answer a 500 that no cache keeps", async () => {
      vi.spyOn(console, "error").mockImplementation(() => {});
      const response = await lateHandler(new Request("https://dcl.test/late", { headers: NAVIGATION }));
      await response.text();
      expect(response.status).toBe(500);
      expect(response.headers.get("Cache-Control")).toBe("no-store");
      expect(response.headers.get("CDN-Cache-Control")).toBeNull();
    });
  });

  describe("and a first visitor's client sends no fetch metadata", () => {
    it("should answer the private document and set the session cookie", async () => {
      const b = await visitorB("/shop/cart", { accept: "text/html" });
      expect(b.response.headers.get("Cache-Control")).toBe("private, no-store");
      expect(b.response.headers.get("Set-Cookie")).toMatch(/^sid=/);
    });
  });
});

describe("when the shared cart setting is off", () => {
  describe("and the signed-in visitor and a first visitor open the cart", () => {
    let a: Answer;
    let b: Answer;

    beforeEach(async () => {
      a = await visitorA("/shop/cart", NAVIGATION);
      b = await visitorB("/shop/cart", NAVIGATION);
    });

    it("should keep the private document with the visitor's wallet and session", () => {
      expect(a.response.headers.get("Cache-Control")).toBe("private, no-store");
      expect(a.response.headers.get("Vary")).toBe("Sec-Fetch-Dest, Sec-Fetch-Site");
      expect(carries(a.body, A) && carries(a.body, SID_A)).toBe(true);
    });

    it("should set the session cookie on the first visit", () => {
      expect(b.response.headers.get("Set-Cookie")).toMatch(/^sid=/);
    });
  });
});

describe("when the shared cart setting is on without PUBLIC_CATALYST_URL", () => {
  it("should keep the private document, as its own address would come from the request", async () => {
    vi.stubEnv("SHOP_CART_SHARED_CACHE", "true");
    vi.stubEnv("PUBLIC_CATALYST_URL", "");
    const b = await visitorB("/shop/cart", NAVIGATION);
    expect(b.response.headers.get("Cache-Control")).toBe("private, no-store");
    expect(b.response.headers.get("Set-Cookie")).toMatch(/^sid=/);
  });
});
