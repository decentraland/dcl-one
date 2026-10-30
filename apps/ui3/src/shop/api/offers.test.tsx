import { renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { describe, expect, it, vi } from "vitest";

import { createDemo, DemoShell } from "../checkout/demo";
import { ShopServicesProvider } from "../checkout/services";
import { useCheckout } from "../checkout/useCheckout";
import { useOffers, type OfferQuery } from "../detail/offers";
import type { Session } from "../model/settlement";
import { encodePurchaseOffer } from "../model/wire";
import { BOOK, SIGNED_OUT, storeMint, web2, web3 } from "../payment/fixtures";
import { purchaseOfferSource } from "./offers";
import { createShopClient } from "./shop";
import type { ShopSigner, TransportRequest } from "./signer";

const stamp = 1_800_000_000_000;
const offer = (session = web3(), item = 7n) => ({ ...storeMint(session, "wearable", item), quotedAt: stamp });
const query = (key = "hat", item = 7n): OfferQuery => ({ key, asset: offer(web3(), item).asset, listing: { acquisition: "store" } });
const wire = (session = web3(), item = 7n) => encodePurchaseOffer(offer(session, item));

function setup(response: unknown, signer: ShopSigner | null | (() => ShopSigner | null) = null) {
  const log: TransportRequest[] = [];
  const client = createShopClient({ base: "https://node.test", signer, transport: async request => {
    log.push(request);
    return { status: 200, body: JSON.stringify(response) };
  } });
  return { log, client, source: purchaseOfferSource(client) };
}

describe("the node purchase offer source", () => {
  it("sends listing identity and quantity, keeps exact money, and forwards cancellation", async () => {
    const { source, log } = setup({ offers: [wire()] });
    const controller = new AbortController();
    const asked = { ...query(), quantity: 3, price: "untrusted" };
    const result = await source([asked], web3(), controller.signal);
    expect(result.hat).toEqual(offer());
    expect(result.hat?.quotes).toEqual([{ method: "credits", quoted: 115 }, { method: "mana", amount: 2_300_000_000_000_000_000n }]);
    expect(log).toHaveLength(1);
    expect(log[0]).toMatchObject({ url: "https://node.test/shop-api/v1/purchase-offers", method: "POST", signed: false });
    expect(log[0]!.signal).toBe(controller.signal);
    expect(JSON.parse(log[0]!.body!)).toEqual({ lines: [{ asset: asked.asset, listing: asked.listing, quantity: 3 }] });
  });

  it("signs wallet kind over the canonical path and resolves the active signer each time", async () => {
    const signed: unknown[] = [];
    let active: ShopSigner | null = null;
    const { source, log } = setup({ offers: [wire()] }, () => active);
    for (const session of [SIGNED_OUT, web2(), web3(BOOK.reseller)]) {
      const address = session.kind === "signedOut" ? null : session.kind === "web2" ? session.account : session.wallet;
      active = address ? { address, sign: async (method, path, metadata) => {
        signed.push({ address, method, path, metadata });
        return {};
      } } : null;
      await source([query()], session, new AbortController().signal);
    }
    expect(log.map(request => request.signed)).toEqual([false, true, true]);
    expect(signed).toEqual([
      { address: BOOK.buyer, method: "POST", path: "/shop-api/v1/purchase-offers", metadata: { walletKind: "web2" } },
      { address: BOOK.reseller, method: "POST", path: "/shop-api/v1/purchase-offers", metadata: { walletKind: "web3" } },
    ]);
  });

  it("keys ordered replies by the caller's keys, including prototype names", async () => {
    const { source } = setup({ offers: [wire(), wire(web3(), 8n)] });
    const result = await source([query("__proto__"), query("second", 8n)], web3(), new AbortController().signal);
    expect(Object.keys(result)).toEqual(["__proto__", "second"]);
    expect(result["__proto__"]).toEqual(offer());
    expect(result.second).toEqual(offer(web3(), 8n));
  });

  it.each([
    null, {}, { offers: [] }, { offers: [wire(), wire()] },
    { offers: [null] }, { offers: [{ ...wire(), quotedAt: 0 }] },
    { offers: [{ ...wire(), quotes: [{ method: "mana", amount: 2.3 }] }] },
    { offers: [wire(web3(), 8n)] },
  ])("rejects an incomplete, malformed or wrong-asset response: %j", async response => {
    await expect(setup(response).source([query()], web3(), new AbortController().signal)).rejects.toThrow(/purchase quote response/i);
  });

  it.each(["Bad Request", "payment_rule_refused"])("preserves an indexed unavailable line: %s", async error => {
    const { source } = setup({ offers: [wire(), { index: 1, error, message: "withdrawn" }] });
    await expect(source([query(), query("second", 8n)], web3(), new AbortController().signal)).resolves.toEqual({ hat: offer(), second: null });
  });

  it("keeps source outages distinct from withdrawn listings and rejects misplaced errors", async () => {
    const signal = new AbortController().signal;
    await expect(setup({ offers: [{ index: 0, error: "Service Unavailable", message: "items index not ready" }] }).source([query()], web3(), signal)).rejects.toThrow("items index not ready");
    await expect(setup({ offers: [{ index: 1, error: "Bad Request" }] }).source([query()], web3(), signal)).rejects.toThrow("another line");
  });

  it("rejects duplicate keys and oversized requests before transport; empty requests do nothing", async () => {
    const { source, log } = setup({});
    const signal = new AbortController().signal;
    await expect(source([], web3(), signal)).resolves.toEqual({});
    await expect(source([query(), query()], web3(), signal)).rejects.toThrow("Invalid purchase quote request");
    await expect(source(Array.from({ length: 51 }, (_, i) => query(String(i))), web3(), signal)).rejects.toThrow("Invalid purchase quote request");
    expect(log).toEqual([]);
  });
});

describe("the default detail and cart quote connection", () => {
  it("detail uses the services client and cancels an old session's pending quote", async () => {
    const requests: TransportRequest[] = [];
    const pending: ((answer: { status: number; body: string }) => void)[] = [];
    const client = createShopClient({ transport: request => {
      requests.push(request);
      return new Promise(resolve => pending.push(resolve));
    } });
    const wrapper = ({ children }: { children: ReactNode }) => <ShopServicesProvider value={{ client }}>{children}</ShopServicesProvider>;
    const { result, rerender } = renderHook(({ session }: { session: Session }) => useOffers([query()], session), { wrapper, initialProps: { session: web3() } });
    await waitFor(() => expect(requests).toHaveLength(1));
    rerender({ session: web2(BOOK.reseller) });
    await waitFor(() => expect(requests).toHaveLength(2));
    expect(requests[0]!.signal?.aborted).toBe(true);
    pending[1]!({ status: 200, body: JSON.stringify({ offers: [wire(web2(BOOK.reseller))] }) });
    await waitFor(() => expect(result.current.status).toBe("ready"));
    pending[0]!({ status: 200, body: JSON.stringify({ offers: [wire()] }) });
    await waitFor(() => expect(result.current.offers.hat).toEqual(offer(web2(BOOK.reseller))));
  });

  it("cart revalidation uses the same endpoint without an injected offer fetcher", async () => {
    const quote = vi.fn(() => ({ offers: [wire()] }));
    const demo = createDemo({ session: "web3", lines: [{ offer: storeMint, name: "Hat", listing: { acquisition: "store" } }], handlers: { purchaseOffers: quote } });
    delete demo.services.fetchOffers;
    const wrapper = ({ children }: { children: ReactNode }) => <DemoShell demo={demo}>{children}</DemoShell>;
    const { result } = renderHook(() => useCheckout(), { wrapper });
    await waitFor(() => expect(result.current.review.kind).toBe("current"));
    expect(quote).toHaveBeenCalled();
    expect(result.current.offers[demo.lines[0]!.id]).toEqual(offer());
    expect(result.current.views[demo.lines[0]!.id]?.options.map(option => option.method)).toEqual(["credits", "mana"]);
  });
});
