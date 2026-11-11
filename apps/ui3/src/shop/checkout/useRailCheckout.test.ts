import { describe, expect, it } from "vitest";

import type { CartPartition } from "@shop/cart/partition";
import { CHAIN } from "@shop/model/chain";
import { partitionMana } from "./useRailCheckout";

const group = (method: "credits" | "mana", chainId: number, amount: bigint) =>
  ({ key: { method, chainId }, total: method === "mana" ? { method, amount } : { method, quoted: 100, charged: 100 } }) as unknown;

describe("partitionMana", () => {
  it("sums the MANA groups on one chain and nothing else", () => {
    const partition = { groups: [group("mana", CHAIN.POLYGON, 2n), group("mana", CHAIN.POLYGON, 3n), group("mana", CHAIN.ETHEREUM, 7n), group("credits", CHAIN.POLYGON, 0n)] } as unknown as CartPartition;
    expect(partitionMana(partition, CHAIN.POLYGON)).toBe(5n);
    expect(partitionMana(partition, CHAIN.ETHEREUM)).toBe(7n);
    expect(partitionMana(null, CHAIN.POLYGON)).toBe(0n);
  });
});
