// The offer wire keeps what the server says about a line's split: an offer the node would authorize a Credits part for
// says so, any other does not, and the cart's stored copy keeps it.

import { describe, expect, it } from "vitest";

import { creatorOrder, storeMint, web3 } from "../payment/fixtures";
import { decodePurchaseOffer, encodePurchaseOffer } from "./wire";

describe("an offer's split", () => {
  it("a splittable offer is decoded as one, any other is not, and the stored copy keeps it", () => {
    const mint = storeMint(web3());
    const pegged = creatorOrder(web3());
    expect(mint.splittable).toBe(true);
    expect(pegged.splittable).toBeUndefined();
    const wire = (o: unknown) => JSON.parse(JSON.stringify(o, (_k, v) => (typeof v === "bigint" ? v.toString() : v)));
    expect(decodePurchaseOffer(wire(encodePurchaseOffer(mint)))?.splittable).toBe(true);
    expect(decodePurchaseOffer(wire(encodePurchaseOffer(pegged)))).not.toHaveProperty("splittable");
    expect(decodePurchaseOffer({ ...wire(encodePurchaseOffer(pegged)), splittable: "yes" })).not.toHaveProperty("splittable");
  });
});
