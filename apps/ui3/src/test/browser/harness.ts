// The *.browser.test files drive Chrome or Chromium over the DevTools protocol, on a page that Vite serves from an
// entry in this folder: jsdom has no top layer, no focus return and no input that reaches the browser as input.
import { fileURLToPath } from "node:url";
import { createServer, loadConfigFromFile, type ViteDevServer } from "vite";
import { findBrowser, openSession } from "../../../../../library/ui/figma/review/scripts/capture-browser.mjs";

export type Point = { x: number; y: number };

const BROWSERS = [
  process.env.BROWSER_PATH,
  process.env.CHROME_PATH,
  "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
  "/usr/bin/google-chrome",
  "/usr/bin/chromium",
  "/usr/bin/chromium-browser",
  "/run/current-system/sw/bin/chromium",
];

type PageOptions = {
  /** Resolve ui3's aliases (@shop and the like), as its vitest config does. */
  aliases?: boolean;
  /** One copy of these packages, also for code under library/, which has no node_modules. */
  dedupe?: string[];
  /** Bundled before the page loads: a dependency found on the first load would reload the page under the test. */
  prebundle?: string[];
};

/**
 * The page of `entry` (a path under ui3, such as `/src/test/browser/dialogOverlay.tsx`) in a 1024x768 browser
 * window. Give `open` to beforeAll and `close` to afterAll.
 */
export function browserPage(entry: string, { aliases = false, dedupe = [], prebundle = [] }: PageOptions = {}) {
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

  return {
    evaluate,
    send: (method: string, params?: Record<string, unknown>) => session!.send(method, params),
    load: () => session!.navigate(server!.resolvedUrls!.local[0]!),
    /** Waits until the expression is true in the page, then for two more frames. */
    until: (expression: string) => evaluate(
      `(async()=>{for(let n=0;n<100&&!(${expression});n++)await new Promise(r=>setTimeout(r,50));if(!(${expression}))throw new Error(${JSON.stringify(expression)});` +
      `for(let n=0;n<2;n++)await new Promise(r=>requestAnimationFrame(r))})()`,
    ),
    center: (selector: string) => evaluate<Point>(
      `(()=>{const r=document.querySelector(${JSON.stringify(selector)}).getBoundingClientRect();return {x:r.x+r.width/2,y:r.y+r.height/2}})()`,
    ),
    async press(point: Point) {
      await session!.send("Input.dispatchMouseEvent", { type: "mouseMoved", ...point });
      await session!.send("Input.dispatchMouseEvent", { type: "mousePressed", button: "left", clickCount: 1, ...point });
      await session!.send("Input.dispatchMouseEvent", { type: "mouseReleased", button: "left", clickCount: 1, ...point });
    },
    async open() {
      const ui3 = aliases
        ? await loadConfigFromFile({ command: "serve", mode: "development" }, fileURLToPath(new URL("../../../vitest.config.ts", import.meta.url)))
        : null;
      server = await createServer({
        configFile: false,
        root: fileURLToPath(new URL("../../..", import.meta.url)),
        appType: "custom",
        logLevel: "error",
        resolve: { alias: ui3?.config.resolve?.alias, dedupe: ["react", "react-dom", ...dedupe] },
        optimizeDeps: { noDiscovery: true, include: ["react", "react-dom", "react-dom/client", "react/jsx-dev-runtime", ...prebundle] },
        server: { host: "127.0.0.1", port: 0 },
      });
      server.middlewares.use(async (request, response, next) => {
        if (request.url !== "/") return next();
        response.setHeader("Content-Type", "text/html");
        response.end(await server!.transformIndexHtml("/", `<!doctype html><div id="root"></div><script type="module" src="${entry}"></script>`));
      });
      await server.listen();
      session = await openSession(await findBrowser(BROWSERS));
      await session.send("Emulation.setDeviceMetricsOverride", { width: 1024, height: 768, deviceScaleFactor: 1, mobile: false });
    },
    async close() {
      try {
        await session?.close();
      } finally {
        await server?.close();
      }
    },
  };
}
