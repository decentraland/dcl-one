import { afterEach, describe, expect, it } from "vitest";

import { cartPicturesScript, PRIORITY_LINE_COUNT } from "./pictures";
import { hat, picture, preloaded, preloads, SIGNED_OUT } from "./pictures.test-helper";
import { ANONYMOUS, CART_KEY_PREFIX, createCartStore } from "./store";

// No session here: apps/sites/src/routes/shop/cart-pictures.test.ts signs in with the site's own session writers.
const run = () => new Function(cartPicturesScript(SIGNED_OUT))();

/** The pictures of the first lines as the cart store reads them back: the lines the cart page draws first. */
function storeFirst(): string[] {
  const lines = createCartStore().getSnapshot().lines.slice(0, PRIORITY_LINE_COUNT);
  return lines.flatMap((line) => (line.display.thumbnail ? [line.display.thumbnail] : []));
}

/** Changes the anonymous cart's record as the store wrote it. */
function edit(change: (record: { version: number; lines: Record<string, unknown>[] }) => void) {
  const key = CART_KEY_PREFIX + ANONYMOUS;
  const record = JSON.parse(localStorage.getItem(key)!);
  change(record);
  localStorage.setItem(key, JSON.stringify(record));
}

afterEach(() => {
  localStorage.clear();
  for (const link of preloads()) link.remove();
});

describe("the stored cart's pictures, before the page's code", () => {
  it("preloads with high priority the pictures of the first lines that the store reads", () => {
    const cart = createCartStore();
    [hat(1), hat(2, false), hat(3), hat(4), hat(5)].forEach((line) => cart.add(line));
    run();
    expect(preloaded()).toEqual([picture(1), picture(3)]);
    expect(preloaded()).toEqual(storeFirst());
    expect(preloads().every((link) => link.getAttribute("fetchpriority") === "high")).toBe(true);
  });

  it("skips the lines that the store skips: a repeated id, no name, an odd asset or payment choice", () => {
    const cart = createCartStore();
    [1, 2, 3, 4, 5, 6].forEach((n) => cart.add(hat(n)));
    edit(({ lines }) => {
      lines.unshift({ ...lines[5]!, display: { name: "Again", thumbnail: picture(99) } });
      (lines[1]!.display as Record<string, unknown>).name = 7;
      (lines[2]!.asset as Record<string, unknown>).contract = "0x1";
      lines[3]!.method = "card";
    });
    run();
    expect(preloaded()).toEqual([picture(99), picture(4), picture(5)]);
    expect(preloaded()).toEqual(storeFirst());
  });

  it("does nothing for a record of another version, a broken record or a cart without pictures", () => {
    const cart = createCartStore();
    cart.add(hat(1));
    edit((record) => {
      record.version = 2;
    });
    run();
    expect(storeFirst()).toEqual([]);
    expect(preloads()).toEqual([]);
    localStorage.setItem(CART_KEY_PREFIX + ANONYMOUS, "{");
    expect(run).not.toThrow();
    localStorage.clear();
    createCartStore().add(hat(1, false));
    run();
    expect(preloads()).toEqual([]);
  });

  it("runs in a scope of its own", () => {
    const globals = Object.keys(window).length;
    createCartStore().add(hat(1));
    run();
    expect(Object.keys(window)).toHaveLength(globals);
    expect(preloads()).toHaveLength(1);
  });
});
