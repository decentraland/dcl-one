import { act, renderHook } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { evmAddress } from "../model/address";
import { itemAsset, tokenAsset } from "../model/asset";
import { CHAIN } from "../model/chain";
import { storeMint, web3 } from "../payment/fixtures";
import { partitionCart } from "./partition";
import { CART_KEY_PREFIX, CHECKOUT_KEY_PREFIX, createCartStore, useCart, type CartStorage } from "./store";

function memory(): CartStorage & { data: Map<string, string> } {
  const data = new Map<string, string>();
  return {
    data,
    getItem: (k) => data.get(k) ?? null,
    setItem: (k, v) => void data.set(k, v),
    removeItem: (k) => void data.delete(k),
  };
}

const collection = evmAddress("0x000000000000000000000000000000000000c011");
const mint = { asset: itemAsset(CHAIN.POLYGON, collection, 7n), listing: { acquisition: "store" as const }, display: { name: "Hat" } };
const token = { asset: tokenAsset(CHAIN.POLYGON, collection, 42n), listing: { tradeId: "t-1" }, display: { name: "Hat #42" } };
const A = "0x00000000000000000000000000000000000000aa";
const B = "0x00000000000000000000000000000000000000bb";

describe("one cart per account", () => {
  it("a mint line counts units up to its stock; a token line stays one", () => {
    const store = createCartStore({ storage: memory, now: () => 5 });
    store.add({ ...mint, available: 2 });
    store.add({ ...mint, available: 2 });
    store.add({ ...mint, available: 2 });
    store.add(token);
    store.add(token);
    expect(store.getSnapshot().lines.map((l) => [l.display.name, l.quantity])).toEqual([
      ["Hat", 2],
      ["Hat #42", 1],
    ]);
    const id = store.getSnapshot().lines[1]!.id;
    store.setQuantity(id, 5);
    expect(store.getSnapshot().lines[1]!.quantity).toBe(1);
  });

  it("signing in adopts the anonymous lines; switching accounts loads the other account's own cart", () => {
    const storage = memory();
    const store = createCartStore({ storage: () => storage });
    store.add(mint);
    store.setMethod(store.getSnapshot().lines[0]!.id, "mana");
    store.scopeTo(A.toUpperCase().replace("0X", "0x"));
    expect(store.getSnapshot().account).toBe(A);
    expect(store.getSnapshot().lines.map((l) => [l.display.name, l.method])).toEqual([["Hat", null]]);
    expect(storage.data.has(`${CART_KEY_PREFIX}anonymous`)).toBe(false);
    expect(storage.data.has(`${CART_KEY_PREFIX}${A}`)).toBe(true);

    store.scopeTo(B);
    expect(store.getSnapshot().lines).toEqual([]);
    store.add(token);
    store.scopeTo(A);
    expect(store.getSnapshot().lines.map((l) => l.display.name)).toEqual(["Hat"]);
    store.scopeTo(null);
    expect(store.getSnapshot().lines, "signed out shows the anonymous cart, not the last account's").toEqual([]);

    const reloaded = createCartStore({ storage: () => storage });
    reloaded.scopeTo(B);
    expect(reloaded.getSnapshot().lines.map((l) => l.display.name)).toEqual(["Hat #42"]);
  });

  it("the checkout progress is persisted with its account and refuses another one", () => {
    const storage = memory();
    const store = createCartStore({ storage: () => storage, now: () => 7 });
    expect(() => store.startCheckout([])).toThrow();
    store.scopeTo(A);
    const s = web3();
    const offer = storeMint(s);
    const line = store.add({ asset: offer.asset, listing: { acquisition: "store" }, display: { name: "Hat" }, seen: offer });
    const groups = partitionCart([{ line, offer }], s, { report: () => {} }).groups;
    const started = store.startCheckout(groups);
    store.dispatch(started.groups[0]!.keyId, { type: "authorize" });
    expect(JSON.parse(storage.data.get(`${CHECKOUT_KEY_PREFIX}${A}`)!).groups[0].status).toBe("authorizing");
    store.scopeTo(B);
    expect(store.getSnapshot().checkout).toBeNull();
    expect(() => store.replaceCheckout(started)).toThrow();
    store.scopeTo(A);
    expect(store.getSnapshot().checkout?.groups[0]!.status).toBe("authorizing");
    expect(store.getSnapshot().lines[0]!.seen).toEqual(offer);
  });

  it("drops malformed persisted lines instead of trusting them", () => {
    const storage = memory();
    storage.setItem(
      `${CART_KEY_PREFIX}anonymous`,
      JSON.stringify({ version: 1, lines: [{ id: "x", asset: { chainId: 137, contract: "nope", token: { kind: "item", itemId: "1" } }, display: { name: "bad" } }, { ...mint, id: "ok", quantity: 2, method: "credits" }] }),
    );
    const store = createCartStore({ storage: () => storage });
    expect(store.getSnapshot().lines.map((l) => [l.id, l.quantity, l.method])).toEqual([["ok", 2, "credits"]]);
  });

  it("useCart re-renders on change", () => {
    const store = createCartStore({ storage: memory });
    const { result } = renderHook(() => useCart((s) => s.lines.length, store));
    expect(result.current).toBe(0);
    act(() => void store.add(mint));
    expect(result.current).toBe(1);
  });
});
