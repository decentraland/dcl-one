// The journey's Chromium. CHROMIUM_BIN overrides; otherwise the newest /nix/store chromium, else
// playwright-core's own download (`npx playwright-core install chromium`).
import { existsSync, readdirSync } from "node:fs";
import { chromium, type Browser, type Page } from "playwright-core";
import { waitUntil } from "../../../library/async/index.ts";

function nixChromium(): string | undefined {
  if (!existsSync("/nix/store")) return undefined;
  const version = (d: string) => d.match(/-chromium-(\d+(?:\.\d+)*)$/)?.[1]?.split(".").map(Number);
  const newer = (a: number[], b: number[]) => {
    for (let i = 0; i < Math.max(a.length, b.length); i++) if ((a[i] ?? 0) !== (b[i] ?? 0)) return (a[i] ?? 0) - (b[i] ?? 0);
    return 0;
  };
  const found = readdirSync("/nix/store")
    .filter((d) => version(d) && existsSync(`/nix/store/${d}/bin/chromium`))
    .sort((a, b) => newer(version(a)!, version(b)!))
    .at(-1);
  return found ? `/nix/store/${found}/bin/chromium` : undefined;
}

export function launch(): Promise<Browser> {
  return chromium.launch({ executablePath: process.env.CHROMIUM_BIN || nixChromium(), headless: true });
}

const DISCOUNT_PRICE_TIMEOUT_MS = 3 * 60_000;
const PAGE_ACTION_TIMEOUT_MS = 30_000;
const PAYMENT_PANEL_TIMEOUT_MS = 60_000;
const PRICE_READY_TIMEOUT_MS = 15_000;
const DISCOUNT_PRICE_BACKOFF = { initialDelayMs: 5000, maxDelayMs: 15_000 };

export async function waitForDiscountedPrice(page: Page, itemUrl: string, timeoutMs = DISCOUNT_PRICE_TIMEOUT_MS): Promise<string> {
  const controller = new AbortController();
  const deadline = performance.now() + timeoutMs;
  const stopped = () => controller.signal.aborted || performance.now() >= deadline;
  const remaining = (maximum: number) => Math.max(1, Math.min(maximum, deadline - performance.now()));
  let active: Promise<string | undefined> | undefined;
  try {
    return await waitUntil((remainingMs) => {
      active = (async () => {
        await page.goto(itemUrl, { timeout: Math.min(PAGE_ACTION_TIMEOUT_MS, remainingMs) });
        if (stopped()) return;
        await page.getByTestId("payment-panel").waitFor({ timeout: remaining(PAYMENT_PANEL_TIMEOUT_MS) }).catch(() => {});
        if (stopped()) return;
        const discounted = await page.getByTestId("detail-price-was").waitFor({ timeout: remaining(PRICE_READY_TIMEOUT_MS) }).then(() => true, () => false);
        if (stopped() || !discounted) return;
        return (await page.getByTestId("item-price").innerText({ timeout: remaining(PAGE_ACTION_TIMEOUT_MS) }).catch(() => "")).replace(/\s+/g, " ");
      })();
      return active;
    }, { timeoutMs, ...DISCOUNT_PRICE_BACKOFF, description: "the discounted item price", signal: controller.signal });
  } finally {
    controller.abort();
    await active?.catch(() => {});
  }
}
