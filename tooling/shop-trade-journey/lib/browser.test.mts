import assert from "node:assert/strict";
import { setImmediate } from "node:timers/promises";
import { test } from "node:test";
import type { Page } from "playwright-core";
import { waitForDiscountedPrice } from "./browser.mts";

for (const stage of ["navigation", "payment-panel", "detail-price-was"]) {
  test(`discount timeout drains ${stage} before the failure screenshot can run`, async (t) => {
    t.mock.timers.enable({ apis: ["setTimeout"] });
    let now = 0;
    t.mock.method(performance, "now", () => now);
    let release!: () => void;
    let settled = false;
    let photographed = false;
    let navigations = 0;
    const operations: string[] = [];
    const budgets: number[] = [];
    const blocked = new Promise<void>((resolve) => { release = () => { settled = true; resolve(); }; });
    const page = {
      goto: async (_url: string, options: { timeout: number }) => {
        navigations++;
        operations.push("navigation");
        budgets.push(options.timeout);
        if (stage === "navigation") await blocked;
      },
      getByTestId: (id: string) => ({
        waitFor: async (options: { timeout: number }) => {
          operations.push(id);
          budgets.push(options.timeout);
          if (stage === id) await blocked;
        },
        innerText: async () => { assert.fail("price read continued after cancellation"); },
      }),
    } as unknown as Page;
    const rejected = waitForDiscountedPrice(page, "/item", 20).catch((error) => {
      assert.equal(settled, true);
      photographed = true;
      return error;
    });
    await setImmediate();
    assert.equal(operations.at(-1), stage);
    now = 20;
    t.mock.timers.tick(20);
    await setImmediate();
    assert.equal(photographed, false);
    release();
    assert.match(String(await rejected), /Timed out: the discounted item price/);
    assert.equal(photographed, true);
    assert.equal(navigations, 1);
    assert.equal(operations.at(-1), stage);
    assert.ok(budgets.every((budget) => budget > 0 && budget <= 20));
    t.mock.timers.tick(30_000);
    await setImmediate();
    assert.equal(navigations, 1);
  });
}

test("discount readiness returns the normalized price immediately", async () => {
  const operations: string[] = [];
  const page = {
    goto: async (url: string) => { operations.push(url); },
    getByTestId: (id: string) => ({
      waitFor: async () => { operations.push(id); },
      innerText: async () => "MANA   1",
    }),
  } as unknown as Page;
  assert.equal(await waitForDiscountedPrice(page, "/item"), "MANA 1");
  assert.deepEqual(operations, ["/item", "payment-panel", "detail-price-was"]);
});

test("discount readiness preserves a hard navigation failure", async () => {
  const failure = new Error("navigation refused");
  let navigations = 0;
  const page = {
    goto: async () => { navigations++; throw failure; },
    getByTestId: () => { assert.fail("readiness continued after navigation failed"); },
  } as unknown as Page;
  await assert.rejects(waitForDiscountedPrice(page, "/item"), (error) => error === failure);
  assert.equal(navigations, 1);
});

test("discount retries preserve the five-second initial network pacing", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  let now = 0;
  t.mock.method(performance, "now", () => now);
  let navigations = 0;
  const page = {
    goto: async () => { navigations++; },
    getByTestId: (id: string) => ({
      waitFor: async () => { if (id === "detail-price-was" && navigations === 1) throw new Error("not indexed"); },
      innerText: async () => "MANA 1",
    }),
  } as unknown as Page;
  const ready = waitForDiscountedPrice(page, "/item");
  await setImmediate();
  assert.equal(navigations, 1);
  now = 4999;
  t.mock.timers.tick(4999);
  await setImmediate();
  assert.equal(navigations, 1);
  now = 5000;
  t.mock.timers.tick(1);
  assert.equal(await ready, "MANA 1");
  assert.equal(navigations, 2);
});
