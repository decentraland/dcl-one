import { act, renderHook, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { usdCents } from "../model/money";
import type { CartLine, PurchaseOffer } from "../model/purchase";
import { parcelResale, storeMint, web2, web3 } from "../payment/fixtures";
import { diffLine, useCartRevalidation, type OfferFetcher } from "./revalidate";

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
