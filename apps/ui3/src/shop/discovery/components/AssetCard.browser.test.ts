// @vitest-environment node
import { afterAll, beforeAll, expect, it } from "vitest";
import { browserPage } from "../../../test/browser/harness";

const page = browserPage("/src/test/browser/assetCard.tsx", {
  aliases: true,
  dedupe: ["viem"],
  prebundle: ["react/jsx-runtime", "react-router", "viem"],
});
beforeAll(page.open, 120_000);
afterAll(page.close);

it.each([360, 390])("keeps the alternate MANA price clear of the cart button at %i px", async (width) => {
  await page.send("Emulation.setDeviceMetricsOverride", { width, height: 844, deviceScaleFactor: 1, mobile: false });
  await page.load();
  await page.until('document.querySelector("[data-testid=card-price-mana]")');
  const layout = await page.evaluate<{ gap: number; requiredGap: number; buttonWidth: number }>(`(() => {
    const alternative = document.querySelector("[data-testid=card-price-mana]");
    const button = document.querySelector("[data-testid=card-add-round]");
    const range = document.createRange();
    range.selectNodeContents(alternative);
    const right = Math.max(...Array.from(range.getClientRects(), rect => rect.right));
    const target = button.getBoundingClientRect();
    return { gap: target.left - right, requiredGap: parseFloat(getComputedStyle(button.parentElement).columnGap), buttonWidth: target.width };
  })()`);
  expect(layout.buttonWidth).toBe(32);
  expect(layout.gap).toBeGreaterThanOrEqual(layout.requiredGap - 0.5);
});
