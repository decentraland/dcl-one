import { expect, test } from "vitest";
import itemDetailCss from "./itemdetail.css?raw";
import paymentPanelCss from "./components/paymentpanel.css?raw";

function phoneRules(css: string, selector: string): CSSStyleDeclaration[] {
  const style = document.createElement("style");
  style.textContent = css;
  document.head.append(style);
  const out: CSSStyleDeclaration[] = [];
  for (const rule of Array.from(style.sheet!.cssRules)) {
    if (!(rule instanceof CSSMediaRule) || !rule.media.mediaText.includes("max-width: 900px")) continue;
    for (const inner of Array.from(rule.cssRules)) {
      if (inner instanceof CSSStyleRule && inner.selectorText === selector) out.push(inner.style);
    }
  }
  style.remove();
  return out;
}

test("at phone width the fixed purchase bar is not capped under the cards after the item", () => {
  expect(phoneRules(paymentPanelCss, ".shop-pdp .shop-pay__ctas[data-dual]").map((s) => [s.position, s.zIndex])).toContainEqual(["fixed", "40"]);
  expect(phoneRules(itemDetailCss, ".shop-pdp .shop-detail__main").map((s) => s.getPropertyValue("isolation"))).toContain("auto");
  expect(phoneRules(itemDetailCss, ".shop-pdp .shop-detail__main::before").map((s) => s.display)).toContain("none");
});
