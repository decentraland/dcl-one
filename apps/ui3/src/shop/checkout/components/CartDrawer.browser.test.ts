// @vitest-environment node
// The cart drawer is a modal <dialog>: the browser moves focus into it, keeps the page behind still, closes it on Esc
// and gives focus back on close. jsdom has none of these, so this drives Chrome or Chromium.
import { fileURLToPath } from "node:url";
import { createServer, loadConfigFromFile, type ViteDevServer } from "vite";
import { afterAll, beforeAll, describe, expect, test } from "vitest";
import { findBrowser, openSession } from "../../../../../../library/ui/figma/review/scripts/capture-browser.mjs";

type Point = { x: number; y: number };

let server: ViteDevServer | undefined;
let session: Awaited<ReturnType<typeof openSession>> | undefined;

async function evaluate<T>(expression: string): Promise<T> {
  const result = (await session!.send("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true })) as {
    result: { value: T };
    exceptionDetails?: { text: string; exception?: { description?: string } };
  };
  if (result.exceptionDetails) throw new Error(result.exceptionDetails.exception?.description ?? result.exceptionDetails.text);
  return result.result.value;
}

/** Waits until the expression is true in the page, then for two more frames. */
const until = (expression: string) => evaluate(
  `(async()=>{for(let n=0;n<100&&!(${expression});n++)await new Promise(r=>setTimeout(r,50));if(!(${expression}))throw new Error(${JSON.stringify(expression)});` +
  `for(let n=0;n<2;n++)await new Promise(r=>requestAnimationFrame(r))})()`,
);

const center = (selector: string) => evaluate<Point>(
  `(()=>{const r=document.querySelector(${JSON.stringify(selector)}).getBoundingClientRect();return {x:r.x+r.width/2,y:r.y+r.height/2}})()`,
);

async function press(point: Point) {
  await session!.send("Input.dispatchMouseEvent", { type: "mouseMoved", ...point });
  await session!.send("Input.dispatchMouseEvent", { type: "mousePressed", button: "left", clickCount: 1, ...point });
  await session!.send("Input.dispatchMouseEvent", { type: "mouseReleased", button: "left", clickCount: 1, ...point });
}

async function escape() {
  const key = { key: "Escape", code: "Escape", windowsVirtualKeyCode: 27 };
  await session!.send("Input.dispatchKeyEvent", { type: "rawKeyDown", ...key });
  await session!.send("Input.dispatchKeyEvent", { type: "keyUp", ...key });
}

const DRAWER = 'dialog[aria-label="Shopping cart"]';
const open = `document.querySelector('${DRAWER}')?.matches(":modal")&&!document.querySelector('${DRAWER}').getAnimations({subtree:true}).length`;
const closed = `!document.querySelector('${DRAWER}')&&!window.cartDrawer.getSnapshot().open`;

beforeAll(async () => {
  const ui3 = await loadConfigFromFile({ command: "serve", mode: "development" }, fileURLToPath(new URL("../../../../vitest.config.ts", import.meta.url)));
  server = await createServer({
    configFile: false,
    root: fileURLToPath(new URL("../../../..", import.meta.url)),
    appType: "custom",
    logLevel: "error",
    // ui3's aliases (@shop and the like). library/ has no node_modules: viem resolves to ui3's copy.
    resolve: { alias: ui3?.config.resolve?.alias, dedupe: ["react", "react-dom", "viem"] },
    // Bundled before the page loads: a dependency found on the first load would reload the page under the test.
    optimizeDeps: { noDiscovery: true, include: ["react", "react-dom", "react-dom/client", "react/jsx-dev-runtime", "react/jsx-runtime", "react-router", "viem"] },
    server: { host: "127.0.0.1", port: 0 },
  });
  server.middlewares.use(async (request, response, next) => {
    if (request.url !== "/") return next();
    response.setHeader("Content-Type", "text/html");
    response.end(await server!.transformIndexHtml("/",
      '<!doctype html><div id="root"></div><script type="module" src="/src/test/browser/cartDrawer.tsx"></script>'));
  });
  await server.listen();
  session = await openSession(await findBrowser([
    process.env.BROWSER_PATH,
    process.env.CHROME_PATH,
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    "/usr/bin/google-chrome",
    "/usr/bin/chromium",
    "/usr/bin/chromium-browser",
    "/run/current-system/sw/bin/chromium",
  ]));
  await session.send("Emulation.setDeviceMetricsOverride", { width: 1024, height: 768, deviceScaleFactor: 1, mobile: false });
}, 120_000);

afterAll(async () => {
  try {
    await session?.close();
  } finally {
    await server?.close();
  }
});

/** Loads the page with its opener in the middle of the screen, far down the page, and opens the drawer from it. */
async function openFromOpener(): Promise<number> {
  await session!.navigate(server!.resolvedUrls!.local[0]!);
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
    await session!.send("Input.dispatchMouseEvent", { type: "mouseWheel", x: 200, y: 300, deltaX: 0, deltaY: 600 });
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
