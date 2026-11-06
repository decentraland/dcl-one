// One Chromium for both sides of a capture, so rasterisation differences cancel out.
// CHROMIUM_BIN overrides; otherwise the newest /nix/store chromium,
// else playwright-core's own download (`npx playwright-core install chromium`).
import { existsSync, readdirSync } from "node:fs";
import { chromium, type Browser, type BrowserContext } from "playwright-core";

export type Viewport = { name: string; width: number; height: number; mobile: boolean };

export const VIEWPORTS: Viewport[] = [
  { name: "desktop", width: 1440, height: 900, mobile: false },
  { name: "mobile", width: 390, height: 844, mobile: true },
];

const MOBILE_UA =
  "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1";

function nixChromium(): string | undefined {
  if (!existsSync("/nix/store")) return undefined;
  const found = readdirSync("/nix/store")
    .map((d) => ({ d, m: d.match(/-chromium-(\d+(?:\.\d+)*)$/) }))
    .filter((c) => c.m && existsSync(`/nix/store/${c.d}/bin/chromium`))
    .sort((a, b) => {
      const va = a.m![1]!.split(".").map(Number), vb = b.m![1]!.split(".").map(Number);
      for (let i = 0; i < Math.max(va.length, vb.length); i++) if ((va[i] ?? 0) !== (vb[i] ?? 0)) return (va[i] ?? 0) - (vb[i] ?? 0);
      return 0;
    })
    .at(-1);
  return found ? `/nix/store/${found.d}/bin/chromium` : undefined;
}

export async function launch(): Promise<{ browser: Browser; executable: string }> {
  const executablePath = process.env.CHROMIUM_BIN || nixChromium();
  const browser = await chromium.launch({
    executablePath,
    headless: true,
    args: ["--use-angle=swiftshader", "--enable-unsafe-swiftshader", "--font-render-hinting=none", "--disable-lcd-text", "--hide-scrollbars"],
  });
  return { browser, executable: executablePath ?? chromium.executablePath() };
}

/** A fresh context for one side and viewport: fixed locale, zone, scale and (seeded) randomness. */
export async function sideContext(browser: Browser, vp: Viewport, opts: { now: number; seed: number }): Promise<BrowserContext> {
  const context = await browser.newContext({
    viewport: { width: vp.width, height: vp.height },
    deviceScaleFactor: 1,
    isMobile: vp.mobile,
    hasTouch: vp.mobile,
    userAgent: vp.mobile ? MOBILE_UA : undefined,
    locale: "en-US",
    timezoneId: "UTC",
    colorScheme: "light",
    serviceWorkers: "block",
  });
  await context.clock.setFixedTime(opts.now);
  await context.addInitScript((seed: number) => {
    // mulberry32: the same sequence on both sides, so shuffled rails agree.
    let s = seed >>> 0;
    Math.random = () => {
      s = (s + 0x6d2b79f5) >>> 0;
      let t = s;
      t = Math.imul(t ^ (t >>> 15), t | 1);
      t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
      return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
    };
  }, opts.seed);
  return context;
}
