import { act, renderHook, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { usdCents } from "@shop/model/money";
import type { CartLine, PurchaseOffer } from "@shop/model/purchase";
import { parcelResale, storeMint, web2, web3 } from "@shop/payment/fixtures";
import { diffLine, SEEN_RECENT_MS, seenRecently, useCartRevalidation, type OfferFetcher } from "./revalidate";

function lineFor(offer: PurchaseOffer, over: Partial<CartLine> = {}): CartLine {
  return {
    id: "l0",
    asset: offer.asset,
    quantity: 1,
    method: null,
    listing: {},
    display: { name: "Hat" },
    provenance: {},
    seen: offer,
    addedAt: 0,
    ...over,
  };
}

describe("diffLine", () => {
  const s = web3();
  const offer = storeMint(s);

  it("nothing changed", () => {
    expect(diffLine(lineFor(offer), offer, s)).toEqual([]);
  });

  it("a repriced, sold, restricted or out-of-stock line is a change", () => {
    const repriced: PurchaseOffer = { ...offer, quotes: [{ method: "credits", quoted: usdCents(130) }, offer.quotes[1]!] };
    expect(diffLine(lineFor(offer), repriced, s)).toEqual([
      { lineId: "l0", kind: "priceChanged", method: "credits", before: { method: "credits", quoted: 115 }, after: { method: "credits", quoted: 130 } },
    ]);
    expect(diffLine(lineFor(offer), null, s)).toEqual([{ lineId: "l0", kind: "unavailable" }]);
    expect(diffLine(lineFor(offer, { method: "mana" }), storeMint(web2()), web2())).toEqual([
      { lineId: "l0", kind: "methodWithdrawn", method: "mana", allowed: ["credits"] },
    ]);
    expect(diffLine(lineFor(offer, { quantity: 4 }), { ...offer, available: 3 }, s)).toEqual([
      { lineId: "l0", kind: "insufficientStock", available: 3, wanted: 4 },
    ]);
  });

  it("an outage is a change the buyer sees, not a switch of method", () => {
    const outage: PurchaseOffer = { ...offer, quotes: offer.quotes.slice(1), unavailable: [{ method: "credits", missing: "manaUsdRate" }] };
    expect(diffLine(lineFor(offer), outage, s)).toEqual([{ lineId: "l0", kind: "quoteUnavailable", method: "credits", missing: "manaUsdRate" }]);
  });

  it("a restricted line stays MANA", () => {
    const parcel = parcelResale(s);
    expect(diffLine(lineFor(parcel), parcel, s)).toEqual([]);
  });
});

describe("useCartRevalidation", () => {
  it("re-quotes before confirmation, blocks on a change until accepted, and again after a switch", async () => {
    const s = web3();
    const offer = storeMint(s);
    const repriced: PurchaseOffer = { ...offer, quotes: [{ method: "credits", quoted: usdCents(130) }, offer.quotes[1]!] };
    const fetchOffers = vi.fn<OfferFetcher>().mockResolvedValueOnce({ l0: repriced }).mockResolvedValue({ l0: repriced });
    const onSeen = vi.fn();
    const lines = [lineFor(offer)];
    const { result, rerender } = renderHook(({ revision }) => useCartRevalidation(lines, s, fetchOffers, { revision, onSeen }), {
      initialProps: { revision: "137" },
    });
    await waitFor(() => expect(result.current.status).toBe("changed"));
    expect(result.current.canConfirm).toBe(false);
    act(() => result.current.accept());
    expect(onSeen).toHaveBeenCalledWith("l0", repriced);
    expect(result.current.status).toBe("current");
    expect(result.current.canConfirm).toBe(true);
    rerender({ revision: "1" });
    await waitFor(() => expect(fetchOffers).toHaveBeenCalledTimes(2));
  });

  it("a failed review blocks confirmation", async () => {
    const s = web3();
    const lines = [lineFor(storeMint(s))];
    const { result } = renderHook(() => useCartRevalidation(lines, s, () => Promise.reject(new Error("503"))));
    await waitFor(() => expect(result.current.status).toBe("failed"));
    expect(result.current.canConfirm).toBe(false);
  });
});

describe("a recent offer the buyer saw", () => {
  /** A fetcher that answers only when the test says so. */
  function held() {
    let answer: (offers: Record<string, PurchaseOffer | null>) => void = () => {};
    const fetchOffers = vi.fn<OfferFetcher>(() => new Promise((resolve) => (answer = resolve)));
    return { fetchOffers, answer: (offers: Record<string, PurchaseOffer | null>) => answer(offers) };
  }

  it("is one quoted in the last moments, on either side of the clock", () => {
    const offer = { ...storeMint(web3()), quotedAt: 1_000_000 };
    expect(seenRecently(lineFor(offer), 1_000_000 + SEEN_RECENT_MS)).toBe(true);
    expect(seenRecently(lineFor(offer), 1_000_000 - SEEN_RECENT_MS)).toBe(true);
    expect(seenRecently(lineFor(offer), 1_000_001 + SEEN_RECENT_MS)).toBe(false);
    expect(seenRecently(lineFor(offer, { seen: null }), 1_000_000)).toBe(false);
  });

  it("stands while the check runs, and confirmation still waits for the check", async () => {
    const s = web3();
    const offer = { ...storeMint(s), quotedAt: Date.now() };
    const { fetchOffers, answer } = held();
    const lines = [lineFor(offer)];
    const { result } = renderHook(() => useCartRevalidation(lines, s, fetchOffers, { revision: "137" }));
    await waitFor(() => expect(fetchOffers).toHaveBeenCalledTimes(1));
    expect(result.current.status).toBe("checking");
    expect(result.current.seenRecently).toBe(true);
    expect(result.current.canConfirm).toBe(false);
    await act(async () => answer({ l0: { ...offer, quotes: [{ method: "credits", quoted: usdCents(130) }, offer.quotes[1]!] } }));
    expect(result.current.status).toBe("changed");
    expect(result.current.seenRecently).toBe(false);
    expect(result.current.canConfirm).toBe(false);
  });

  it("an old offer, or a session or wallet switch, waits for the server", async () => {
    const s = web3();
    const old = [lineFor({ ...storeMint(s), quotedAt: Date.now() - SEEN_RECENT_MS - 1_000 })];
    const first = held();
    const stale = renderHook(() => useCartRevalidation(old, s, first.fetchOffers));
    await waitFor(() => expect(first.fetchOffers).toHaveBeenCalledTimes(1));
    expect(stale.result.current.status).toBe("checking");
    expect(stale.result.current.seenRecently).toBe(false);

    const lines = [lineFor({ ...storeMint(s), quotedAt: Date.now() })];
    const next = held();
    const { result, rerender } = renderHook(({ revision }) => useCartRevalidation(lines, s, next.fetchOffers, { revision }), { initialProps: { revision: "137" } });
    await waitFor(() => expect(result.current.seenRecently).toBe(true));
    rerender({ revision: "1" });
    await waitFor(() => expect(next.fetchOffers).toHaveBeenCalledTimes(2));
    expect(result.current.status).toBe("checking");
    expect(result.current.seenRecently).toBe(false);
  });

  it("a check the buyer asks for again waits for the server", async () => {
    const s = web3();
    const lines = [lineFor({ ...storeMint(s), quotedAt: Date.now() })];
    const { fetchOffers } = held();
    const { result } = renderHook(() => useCartRevalidation(lines, s, fetchOffers));
    await waitFor(() => expect(result.current.seenRecently).toBe(true));
    act(() => result.current.revalidate());
    await waitFor(() => expect(fetchOffers).toHaveBeenCalledTimes(2));
    expect(result.current.seenRecently).toBe(false);
  });
});
