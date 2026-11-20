import { readFileSync } from "node:fs";
import { appendFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { afterAll, beforeAll, beforeEach, describe, expect, it } from "vitest";
import {
  type CaptureSession,
  START_BUDGET_MS,
  startCaptureBrowser,
} from "./capture-browser.helpers";

const root = new URL("../../../../../../", import.meta.url);
const css = [
  "apps/ui3/src/components/site-navigation.css",
  "apps/ui3/src/web/frames/sitelayout.css",
  "apps/ui3/src/shop/discovery/frames/shoptheme.css",
  "apps/ui3/src/social/social-page.css",
]
  .map((file) => readFileSync(fileURLToPath(new URL(file, root)), "utf8"))
  .join("\n");
let session: CaptureSession;
let startup: Promise<CaptureSession> | undefined;
const startupController = new AbortController();
// Teardown also owns a startup that outlives this fixture's integration budget.
beforeAll(async () => {
  startup = (async () => {
    try {
      return await startCaptureBrowser(
        "Header geometry",
        startupController.signal,
      );
    } catch (error) {
      if (
        error instanceof Error &&
        "code" in error &&
        error.code === "CAPTURE_BROWSER_PORT_MISSING"
      ) {
        try {
          if (process.env.GITHUB_OUTPUT) {
            await appendFile(
              process.env.GITHUB_OUTPUT,
              "capture-missing-port=true\n",
            );
          }
          console.error(
            "Header geometry missing-port receipt: capture-missing-port=true",
          );
        } catch {
          // A diagnostic receipt cannot replace the startup failure.
        }
      }
      throw error;
    }
  })();
  session = await startup;
}, START_BUDGET_MS + 10_000);
afterAll(async () => {
  startupController.abort(new Error("Header geometry fixture disposed"));
  const opened = await startup?.catch(() => undefined);
  await opened?.close();
}, 30_000);

type Offsets = {
  root: string;
  child: string;
  shop: string;
  height: number;
  social: number;
};

describe("when native and legacy chrome hosts resolve their header offsets", () => {
  for (const width of [390, 992, 1024, 1440]) {
    for (const host of ["create", "social", "shop"] as const) {
      describe(`and the native header wraps the ${host} host at ${width}px`, () => {
        const height = width < 992 ? 64 : 92;
        let actual: Offsets;

        beforeEach(async () => {
          await session.send("Emulation.setDeviceMetricsOverride", {
            width,
            height: 900,
            deviceScaleFactor: 1,
            mobile: false,
          });
          const child = host === "create" ? "ch" : "cs";
          const html = `<style>body{margin:0}${css}</style><div class="site-layout ui2" data-header-layout="native"><div class="site-layout__navigation"></div><div class="site-layout__content"><div class="${child} ui2 ${host === "shop" ? "shop-chrome" : ""} ${host === "social" ? "social-web" : ""}"><div class="soc"></div></div></div></div>`;
          await session.navigate(`data:text/html,${encodeURIComponent(html)}`);
          const response = await session.send("Runtime.evaluate", {
            expression: `(() => {const root=document.querySelector('.site-layout'), child=document.querySelector('.site-layout__content>div');return {root:getComputedStyle(root).getPropertyValue('--site-header-h').trim(), child:getComputedStyle(child).getPropertyValue('--site-header-h').trim(), shop:getComputedStyle(child).getPropertyValue('--shop-nav-h').trim(), height:child.getBoundingClientRect().height, social:document.querySelector('.soc').getBoundingClientRect().height}})()`,
            returnByValue: true,
          });
          actual = (response as { result: { value: Offsets } }).result.value;
        });

        it(`should set the layout's header token to ${height}px`, () => {
          expect(actual.root).toBe(`${height}px`);
        });

        it(`should propagate the header token to the ${host} scope`, () => {
          expect(actual.child).toBe(`${height}px`);
        });

        if (host === "shop") {
          it("should propagate the header height to the shop navigation token", () => {
            expect(actual.shop).toBe(`${height}px`);
          });
        }

        it(`should size the ${host} content to the viewport below the header`, () => {
          expect(actual.height).toBe(900 - height);
        });

        if (host === "social") {
          it("should size the social page to the viewport below the header", () => {
            expect(actual.social).toBe(900 - height);
          });
        }
      });
    }
  }

  describe("and legacy standalone hosts render at desktop width", () => {
    let tokens: string[];

    beforeEach(async () => {
      await session.send("Emulation.setDeviceMetricsOverride", {
        width: 1440,
        height: 900,
        deviceScaleFactor: 1,
        mobile: false,
      });
      await session.navigate(
        `data:text/html,${encodeURIComponent(`<style>${css}</style><div class="ui2" id="legacy"></div><div class="ui2 shop-chrome" id="shop"></div>`)}`,
      );
      const response = await session.send("Runtime.evaluate", {
        expression: `[...document.querySelectorAll('[id]')].map(el=>getComputedStyle(el).getPropertyValue('--site-header-h').trim())`,
        returnByValue: true,
      });
      tokens = (response as { result: { value: string[] } }).result.value;
    });

    it("should preserve the legacy standalone desktop header token", () => {
      expect(tokens).toEqual([
        "72px",
        "92px",
      ]);
    });
  });
});
