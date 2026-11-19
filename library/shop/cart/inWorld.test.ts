// The in-world cart model with a fake host: what the core does with no browser behind it. The browser host
// (library/ui/apps/shop/cartStore.ts) is covered by apps/ui3/src/play/test/inWorldCart.test.ts.
import { describe, expect, it } from "vitest";
import { createInWorldCart, localCartLock, type CartStorage, type InWorldCartInput } from "./inWorld";
import { MAX_PURCHASE_UNITS } from "./lines";

const A = "0x" + "a".repeat(40);
const CONTRACT = "0x" + "c".repeat(40);

function memory(): CartStorage & { data: Map<string, string> } {
  const data = new Map<string, string>();
  return {
    data,
    getItem: (key) => data.get(key) ?? null,
    setItem: (key, value) => void data.set(key, value),
    removeItem: (key) => void data.delete(key),
  };
}

const item = (itemId: string, over: Partial<InWorldCartInput> = {}): InWorldCartInput => ({
  contract: CONTRACT,
  itemId,
  name: `Item ${itemId}`,
  rarity: "epic",
  creator: A,
  credits: 10,
  available: null,
  ...over,
});

function cart(storage: CartStorage | null = memory()) {
  let id = 0;
  return createInWorldCart({
    storage: () => storage,
    watch: () => () => {},
    lock: localCartLock(),
    newId: () => `00000000-${String(++id).padStart(4, "0")}`,
    now: () => 1,
  });
}

describe("the in-world cart model", () => {
  it("adds an item as a line, then one more of the same line", async () => {
    const c = cart();
    c.scopeTo(A);
    await c.add(item("1"));
    await c.add(item("1"));
    expect(c.getSnapshot()).toMatchObject([{ itemId: "1", quantity: 2 }]);
  });

  it("refuses more than the stock it knows of, or than one purchase buys", async () => {
    const c = cart();
    c.scopeTo(A);
    expect(await c.add(item("1", { available: 1 }))).toBe(true);
    expect(await c.add(item("1", { available: 1 }))).toBe(false);
    await c.setQuantity({ contract: CONTRACT, itemId: "2" }, 5);
    expect(c.getSnapshot()).toHaveLength(1);
    for (let i = 0; i < MAX_PURCHASE_UNITS + 2; i++) await c.add(item("3"));
    expect(c.getSnapshot().reduce((n, l) => n + l.quantity, 0)).toBe(MAX_PURCHASE_UNITS);
  });

  // An operation id as the purchase flow makes them (OPERATION_ID refuses anything else when a record is read).
  const OPERATION = "0f3a9c2e-0001";

  it("takes a purchase out once, however often it is heard", async () => {
    const c = cart();
    c.scopeTo(A);
    await c.add(item("1"));
    await c.add(item("1"));
    await c.applyPurchase(A, OPERATION, [{ contract: CONTRACT, itemId: "1", quantity: 1 }]);
    await c.applyPurchase(A, OPERATION, [{ contract: CONTRACT, itemId: "1", quantity: 1 }]);
    expect(c.getSnapshot()).toMatchObject([{ itemId: "1", quantity: 1 }]);
  });

  it("takes a purchase out once after a reload, from what storage remembers", async () => {
    const storage = memory();
    const before = cart(storage);
    before.scopeTo(A);
    await before.add(item("1"));
    await before.add(item("1"));
    await before.applyPurchase(A, OPERATION, [{ contract: CONTRACT, itemId: "1", quantity: 1 }]);

    const after = cart(storage);
    after.scopeTo(A);
    await after.applyPurchase(A, OPERATION, [{ contract: CONTRACT, itemId: "1", quantity: 1 }]);
    expect(after.getSnapshot()).toMatchObject([{ itemId: "1", quantity: 1 }]);
  });

  it("keeps the page's change when there is no storage", async () => {
    const c = cart(null);
    c.scopeTo(A);
    await c.add(item("1"));
    expect(c.getSnapshot()).toMatchObject([{ itemId: "1", quantity: 1 }]);
  });

  it("keeps each account's cart under its own key, a signed-out cart joining the account's", async () => {
    const storage = memory();
    const c = cart(storage);
    await c.add(item("1"));
    c.scopeTo(A);
    await c.add(item("2"));
    expect(c.getSnapshot().map((l) => l.itemId).sort()).toEqual(["1", "2"]);
    expect([...storage.data.keys()].some((k) => k.endsWith(A))).toBe(true);
  });
});
