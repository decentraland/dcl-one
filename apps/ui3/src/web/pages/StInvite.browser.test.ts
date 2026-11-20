import { readFileSync } from "node:fs";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import buttonStyles from "../../../../../library/ui/components/Button/Button.module.css";
import buttonCss from "../../../../../library/ui/components/Button/Button.module.css?inline";
import { findBrowser, openSession } from "../../../../../library/ui/figma/review/scripts/capture-browser.mjs";
import StInvite from "./StInvite";

const pageCss = [
  "../../../../../library/ui/foundations/colors/colors.css",
  "../../../../../library/ui/foundations/layout/layout.css",
  "../../../../../library/ui/foundations/spacing/spacing.css",
  "./stinvite.css",
]
  .map((file) => readFileSync(new URL(file, import.meta.url), "utf8"))
  .join("\n");

// The inline CSS has its own module hash. Use the class names in the server markup.
const buttonClasses: Record<string, string> = buttonStyles;
const buttonRules = buttonCss.replace(/\._([A-Za-z][\w-]*)_[0-9a-z]+\b/g, (match, name: string) =>
  buttonClasses[name] ? `.${buttonClasses[name]}` : match,
);

describe.each([
  { width: 485, height: 878 },
  { width: 390, height: 640 },
  { width: 320, height: 568 },
  { width: 320, height: 480 },
])("when Invite opens on a $width × $height phone", ({ width, height }) => {
  let session: Awaited<ReturnType<typeof openSession>>;

  beforeEach(async () => {
    const browser = await findBrowser([
      process.env.BROWSER_PATH,
      process.env.CHROME_PATH,
      "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
      "/usr/bin/google-chrome",
      "/usr/bin/chromium",
    ]);
    session = await openSession(browser);
    await session.send("Emulation.setDeviceMetricsOverride", { width, height, deviceScaleFactor: 1, mobile: true });
    const markup = renderToStaticMarkup(createElement(StInvite, { referrer: { name: "Brai", ethAddress: "0xABC" } }));
    const html = `<!doctype html><meta name="viewport" content="width=device-width, initial-scale=1">
      <style>body{margin:0;font-family:Inter,sans-serif}${pageCss}${buttonRules}</style>${markup}`;
    await session.navigate(`data:text/html,${encodeURIComponent(html)}`);
  }, 40_000);

  afterEach(async () => session?.close(), 30_000);

  it("should expose the entire Join Now link to the invited player before scrolling", async () => {
    const expression = `(() => {
      const link = [...document.links].find(link => link.textContent.trim() === "JOIN NOW");
      const { top, right, bottom, left } = link.getBoundingClientRect();
      const visible = top >= 0 && left >= 0 && bottom <= visualViewport.height && right <= visualViewport.width;
      const hit = document.elementFromPoint((left + right) / 2, (top + bottom) / 2)?.closest("a") === link;
      const href = new URL(link.getAttribute("href"), "https://example.org");
      return { visible, hit, scrollY, referrer: href.searchParams.get("referrer") };
    })()`;
    const { result } = (await session.send("Runtime.evaluate", { expression, returnByValue: true })) as {
      result: { value: { visible: boolean; hit: boolean; scrollY: number; referrer: string } };
    };
    expect(result.value).toEqual({ visible: true, hit: true, scrollY: 0, referrer: "0xabc" });
  });
});
