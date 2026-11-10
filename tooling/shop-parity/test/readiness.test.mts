import assert from "node:assert/strict";
import { test } from "node:test";
import { runInNewContext } from "node:vm";
import type { Page } from "playwright-core";
import { PAGES } from "../pages.mts";
import { scroll, visibleImages } from "../lib/capture.mts";

const timeout = () => Object.assign(new Error("fixture timeout"), { name: "TimeoutError" });

test("lazy images outside either viewport axis do not block capture", async () => {
  const boxes = [
    { left: 900, right: 1000, top: 0, bottom: 100, width: 100, height: 100 },
    { left: -100, right: 0, top: 0, bottom: 100, width: 100, height: 100 },
    { left: 0, right: 100, top: 600, bottom: 700, width: 100, height: 100 },
  ];
  let ready = false;
  await visibleImages({ waitForFunction: async (check: Function) => {
    ready = runInNewContext(`(${check})()`, {
      document: { images: boxes.map((box) => ({ complete: false, getBoundingClientRect: () => box })) },
      innerWidth: 800, innerHeight: 600,
    });
    if (!ready) throw timeout();
  } } as unknown as Page);
  assert.equal(ready, true);
});

test("a slow visible image is best effort, while page failures propagate", async () => {
  await visibleImages({ waitForFunction: async () => { throw timeout(); } } as unknown as Page);
  await assert.rejects(visibleImages({ waitForFunction: async () => { throw new Error("page closed"); } } as unknown as Page), /page closed/);
});

async function cartSetup(lostClick = 0) {
  let stored = 0, rendered = 0, clicks = 0;
  let hydrated = false, drawerOpen = false, probes = 0;
  const clickHydration: boolean[] = [];
  const cart = { get textContent() { return rendered ? `Cart (${rendered})` : "Cart"; }, getAttribute: () => null };
  const page = {
    url: () => "http://fixture/shop/items/first",
    goto: async (url: string) => { if (!url.endsWith("/shop/cart")) { rendered = 0; hydrated = false; } },
    getByRole: () => ({ isVisible: async () => drawerOpen, waitFor: async () => assert.equal(drawerOpen, false) }),
    getByTestId: () => ({ click: async () => {
      probes++;
      if (hydrated) drawerOpen = true;
      else hydrated = true;
    } }),
    keyboard: { press: async (key: string) => { assert.equal(key, "Escape"); drawerOpen = false; } },
    locator: () => ({ first: () => ({ waitFor: async () => {}, click: async () => {
      clicks++; clickHydration.push(hydrated);
      if (hydrated && clicks !== lostClick) { stored++; rendered = stored; }
    } }), evaluate: async () => [cart.textContent, null] }),
    waitForFunction: async (check: Function, argument: unknown) => {
      const read = () => runInNewContext(`(${check})(argument)`, { argument, document: { querySelector: () => cart } });
      if (!read() && !hydrated) { hydrated = true; rendered = stored; }
      if (!read()) throw timeout();
    },
  };
  const setup = PAGES.find((page) => page.id === "cart-two")!.setup!;
  await setup(page as unknown as Page, "dcl-one", { item: "/shop/items/first", item2: "/shop/items/second" });
  return { stored, clickHydration, probes };
}

test("both cart clicks wait for hydration, including the initially empty server render", async () => {
  const result = await cartSetup();
  assert.equal(result.stored, 2);
  assert.deepEqual(result.clickHydration, [true, true]);
  assert.equal(result.probes, 2);
});

test("hydration alone cannot satisfy a lost second add-to-cart click", async () => {
  await assert.rejects(cartSetup(2), { name: "TimeoutError" });
});

test("hydration alone cannot satisfy a lost first add-to-cart click", async () => {
  await assert.rejects(cartSetup(1), { name: "TimeoutError" });
});


test("a bottom scroll keeps its original clamped target when lazy content grows", async () => {
  const document = { documentElement: { scrollHeight: 1200 } };
  const context = {
    document, innerHeight: 600, scrollY: 0,
    window: { scrollTo: ({ top }: { top: number }) => { context.scrollY = top; document.documentElement.scrollHeight += 600; } },
    requestAnimationFrame: (callback: () => void) => callback(),
  };
  const page = {
    evaluate: (check: Function, argument: unknown) => runInNewContext(`(${check})(argument)`, { ...context, argument }),
    waitForFunction: async (check: Function, argument: unknown) => {
      assert.equal(argument, 600);
      assert.equal(runInNewContext(`(${check})(argument)`, { ...context, argument }), true);
    },
  };
  await scroll(page as unknown as Page, 2000);
  assert.equal(context.scrollY, 600);
  assert.equal(document.documentElement.scrollHeight, 1800);
});
