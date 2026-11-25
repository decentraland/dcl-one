// @vitest-environment node
// The cart drawer is a modal <dialog>: the browser moves focus into it, keeps the page behind still, closes it on Esc
// and gives focus back on close. jsdom has none of these, so this drives Chrome or Chromium.
import { afterAll, beforeAll, describe, expect, test } from "vitest";
import { browserPage } from "../../../test/browser/harness";

const page = browserPage("/src/test/browser/cartDrawer.tsx", {
  // library/ has no node_modules: viem resolves to ui3's copy.
  aliases: true,
  dedupe: ["viem"],
  prebundle: ["react/jsx-runtime", "react-router", "viem"],
});
const { evaluate, until, center, press } = page;
beforeAll(page.open, 120_000);
afterAll(page.close);

async function escape() {
  // Chrome on macOS opens its Help tab when Escape also carries native key code 27.
  const key = { key: "Escape", code: "Escape", windowsVirtualKeyCode: 27 };
  await page.send("Input.dispatchKeyEvent", { type: "rawKeyDown", ...key });
  await page.send("Input.dispatchKeyEvent", { type: "keyUp", ...key });
}

const DRAWER = 'dialog[aria-label="Shopping cart"]';
const open = `document.querySelector('${DRAWER}')?.matches(":modal")&&!document.querySelector('${DRAWER}').getAnimations({subtree:true}).length`;
const closed = `!document.querySelector('${DRAWER}')&&!window.cartDrawer.getSnapshot().open`;

/** Loads the page with its opener in the middle of the screen, far down the page, and opens the drawer from it. */
async function openFromOpener(): Promise<number> {
  await page.load();
  await until(`document.getElementById("opener")`);
  await evaluate(`document.getElementById("opener").scrollIntoView({block:"center"})`);
  const top = await evaluate<number>("scrollY");
  expect(top).toBeGreaterThan(1000);
  await press(await center("#opener"));
  await until(open);
  expect(await evaluate(`document.querySelector('${DRAWER}').contains(document.activeElement)`)).toBe(true);
  return top;
}

describe("the cart drawer in a browser", () => {
  test("Esc closes it; the page does not scroll while it is open, nor when focus goes back to the opener", async () => {
    const top = await openFromOpener();
    await page.send("Input.dispatchMouseEvent", { type: "mouseWheel", x: 200, y: 300, deltaX: 0, deltaY: 600 });
    await evaluate("new Promise((r) => setTimeout(r, 500))");
    expect(await evaluate("scrollY")).toBe(top);

    // The page moves under the drawer: the opener is off the screen when focus goes back to it.
    await evaluate("scrollTo(0, 0)");
    await escape();
    await until(closed);
    expect(await evaluate("document.activeElement.id")).toBe("opener");
    expect(await evaluate("scrollY")).toBe(0);
  }, 60_000);

  test.each([
    ["Continue shopping", 'button[data-variant="glass"]'],
    ["Go to cart, once the page has changed,", 'a[href="/shop/cart"]'],
  ])("%s closes it, and focus goes back to the opener without a scroll", async (_, control) => {
    await openFromOpener();
    await evaluate("scrollTo(0, 0)");
    await press(await center(`${DRAWER} ${control}`));
    await until(closed);
    expect(await evaluate("document.activeElement.id")).toBe("opener");
    expect(await evaluate("scrollY")).toBe(0);
  }, 60_000);
});
