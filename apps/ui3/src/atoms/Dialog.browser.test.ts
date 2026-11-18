// @vitest-environment node
// A press that reaches the browser as input runs microtasks between the event listeners. React commits the close
// of a select in that gap, before the dialog's own listener runs; a test in jsdom does not have that gap.
import { fileURLToPath } from "node:url";
import { createServer, type ViteDevServer } from "vite";
import { afterAll, beforeAll, describe, expect, test } from "vitest";
import { findBrowser, openSession } from "../../../../library/ui/figma/review/scripts/capture-browser.mjs";

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

/** A point on the backdrop, away from the dialog and from anything open over it. */
const onBackdrop: Point = { x: 8, y: 8 };

beforeAll(async () => {
  server = await createServer({
    configFile: false,
    root: fileURLToPath(new URL("../..", import.meta.url)),
    appType: "custom",
    logLevel: "error",
    resolve: { dedupe: ["react", "react-dom"] },
    // Bundled before the page loads: a dependency found on the first load would reload the page under the test.
    optimizeDeps: { noDiscovery: true, include: ["react", "react-dom", "react-dom/client", "react/jsx-dev-runtime"] },
    server: { host: "127.0.0.1", port: 0 },
  });
  server.middlewares.use(async (request, response, next) => {
    if (request.url !== "/") return next();
    response.setHeader("Content-Type", "text/html");
    response.end(await server!.transformIndexHtml("/",
      '<!doctype html><div id="root"></div><script type="module" src="/src/test/browser/dialogOverlay.tsx"></script>'));
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
}, 60_000);

afterAll(async () => {
  try {
    await session?.close();
  } finally {
    await server?.close();
  }
});

async function openPage() {
  await session!.navigate(server!.resolvedUrls!.local[0]!);
  await until(`document.querySelector('[role="dialog"]')`);
}

describe("Dialog in a browser", () => {
  test("a press on the backdrop while a select is open closes the select and keeps the dialog open", async () => {
    await openPage();
    await press(await center('[role="combobox"]'));
    await until(`document.querySelector('[role="listbox"]')`);

    await press(onBackdrop);
    await until(`!document.querySelector('[role="listbox"]')`);

    expect(await evaluate("window.dialogCloses")).toEqual([]);
    expect(await evaluate(`!!document.querySelector('[role="dialog"]')`)).toBe(true);
  }, 30_000);

  test("a press on the backdrop while a menu is open closes the menu and keeps the dialog open", async () => {
    await openPage();
    await press(await center('[data-trigger="menu"]'));
    await until(`document.querySelector('[role="menu"]')`);

    await press(onBackdrop);
    await until(`!document.querySelector('[role="menu"]')`);

    expect(await evaluate("window.dialogCloses")).toEqual([]);
    expect(await evaluate(`!!document.querySelector('[role="dialog"]')`)).toBe(true);
  }, 30_000);

  test("a press on the backdrop with nothing open over the dialog closes it", async () => {
    await openPage();
    await press(onBackdrop);
    await until(`!document.querySelector('[role="dialog"]')`);

    expect(await evaluate("window.dialogCloses")).toEqual(["backdrop"]);
  }, 30_000);
});
