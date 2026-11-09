// Shop stylesheets that are still global stay global once loaded, and a single-page visit loads them as it goes: one
// defining a class another screen uses restyles that screen for the rest of the visit. The cart's payment choice once
// defined `.shop-pay`, the detail page's price panel, which then lost its spacing whenever Buy now's dialog (or a visit
// to the cart) loaded it. New styles are CSS Modules (docs/ui-standards.md), which cannot collide; these two legacy
// sheets are the pair that did.
import { expect, test } from "vitest";

import paymentChoiceCss from "./paymentchoice.css?raw";
import paymentPanelCss from "../../detail/components/paymentpanel.css?raw";

const classes = (css: string): Set<string> => new Set([...css.replace(/\/\*[\s\S]*?\*\//g, "").matchAll(/\.([a-zA-Z][\w-]*)/g)].map((m) => m[1]!));

test("the cart's payment choice and the detail page's price panel define no class in common", () => {
  const panel = classes(paymentPanelCss);
  // `shop-price*` are PriceTag's own classes, which both style inside themselves.
  const shared = [...classes(paymentChoiceCss)].filter((c) => panel.has(c) && !c.startsWith("shop-price"));
  expect(shared).toEqual([]);
});
