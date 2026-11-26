// The in-world cart model with a fake host: what the core does with no browser behind it. The browser host
// (library/ui/apps/shop/cartStore.ts) is covered by apps/ui3/src/play/test/inWorldCart.test.ts.
import { beforeEach, describe, expect, it } from "vitest";
import { createInWorldCart, localCartLock, type CartStorage, type InWorldCartInput } from "./inWorld";
import { MAX_PURCHASE_UNITS } from "./lines";

const A = "0x" + "a".repeat(40);
const CONTRACT = "0x" + "c".repeat(40);
// An operation id as the purchase flow makes them (OPERATION_ID refuses anything else when a record is read).
const OPERATION = "0f3a9c2e-0001";

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
  let storage: ReturnType<typeof memory>;
  let c: ReturnType<typeof cart>;

  beforeEach(() => {
    storage = memory();
  });

  describe("when the same item is added twice", () => {
    beforeEach(async () => {
      c = cart(storage);
      c.scopeTo(A);
      await c.add(item("1"));
      await c.add(item("1"));
    });

    it("should hold one line of that item with a quantity of two", () => {
      expect(c.getSnapshot()).toMatchObject([{ itemId: "1", quantity: 2 }]);
    });
  });

  describe("when an item with one unit in stock is added", () => {
    let added: boolean;

    beforeEach(async () => {
      c = cart(storage);
      c.scopeTo(A);
      added = await c.add(item("1", { available: 1 }));
    });

    it("should accept the unit", () => {
      expect(added).toBe(true);
    });

    describe("and the same item is added again", () => {
      let again: boolean;

      beforeEach(async () => {
        again = await c.add(item("1", { available: 1 }));
      });

      it("should refuse the unit beyond the stock it knows of", () => {
        expect(again).toBe(false);
      });
    });

    describe("and the quantity of an item not in the cart is set", () => {
      beforeEach(async () => {
        await c.setQuantity({ contract: CONTRACT, itemId: "2" }, 5);
      });

      it("should not add a line for that item", () => {
        expect(c.getSnapshot()).toHaveLength(1);
      });
    });

    describe("and another item is added more times than one purchase buys", () => {
      beforeEach(async () => {
        for (let i = 0; i < MAX_PURCHASE_UNITS + 2; i++) await c.add(item("3"));
      });

      it("should cap the units of all lines at what one purchase buys", () => {
        expect(c.getSnapshot().reduce((n, l) => n + l.quantity, 0)).toBe(MAX_PURCHASE_UNITS);
      });
    });
  });

  describe("when the same purchase is heard twice", () => {
    beforeEach(async () => {
      c = cart(storage);
      c.scopeTo(A);
      await c.add(item("1"));
      await c.add(item("1"));
      await c.applyPurchase(A, OPERATION, [{ contract: CONTRACT, itemId: "1", quantity: 1 }]);
      await c.applyPurchase(A, OPERATION, [{ contract: CONTRACT, itemId: "1", quantity: 1 }]);
    });

    it("should take the purchase out once", () => {
      expect(c.getSnapshot()).toMatchObject([{ itemId: "1", quantity: 1 }]);
    });
  });

  describe("when a purchase taken out before a reload is heard again after it", () => {
    beforeEach(async () => {
      const before = cart(storage);
      before.scopeTo(A);
      await before.add(item("1"));
      await before.add(item("1"));
      await before.applyPurchase(A, OPERATION, [{ contract: CONTRACT, itemId: "1", quantity: 1 }]);

      c = cart(storage);
      c.scopeTo(A);
      await c.applyPurchase(A, OPERATION, [{ contract: CONTRACT, itemId: "1", quantity: 1 }]);
    });

    it("should take the purchase out once, from what storage remembers", () => {
      expect(c.getSnapshot()).toMatchObject([{ itemId: "1", quantity: 1 }]);
    });
  });

  describe("when there is no storage", () => {
    beforeEach(async () => {
      c = cart(null);
      c.scopeTo(A);
      await c.add(item("1"));
    });

    it("should keep the page's change", () => {
      expect(c.getSnapshot()).toMatchObject([{ itemId: "1", quantity: 1 }]);
    });
  });

  describe("when an item is added signed out and another after scoping to an account", () => {
    beforeEach(async () => {
      c = cart(storage);
      await c.add(item("1"));
      c.scopeTo(A);
      await c.add(item("2"));
    });

    it("should join the signed-out cart to the account's", () => {
      expect(c.getSnapshot().map((l) => l.itemId).sort()).toEqual(["1", "2"]);
    });

    it("should keep the account's cart under its own key", () => {
      expect([...storage.data.keys()].some((k) => k.endsWith(A))).toBe(true);
    });
  });
});
