// Shop stylesheets are global once loaded, and a single-page visit loads them as it goes: one defining a class another
// screen uses restyles that screen for the rest of the visit. The cart's payment choice once defined `.shop-pay`, the
// detail page's price panel, which then lost its spacing whenever Buy now's dialog (or a visit to the cart) loaded it.
import { expect, test } from "vitest";

import checkoutModalCss from "./checkoutmodal.css?raw";
import paymentChoiceCss from "./paymentchoice.css?raw";
import paymentPanelCss from "../../detail/components/paymentpanel.css?raw";

const classes = (css: string): Set<string> => new Set([...css.replace(/\/\*[\s\S]*?\*\//g, "").matchAll(/\.([a-zA-Z][\w-]*)/g)].map((m) => m[1]!));

test("the payment choice, the detail page's price panel and the checkout dialog define no class in common", () => {
  const sheets = { paymentChoice: classes(paymentChoiceCss), paymentPanel: classes(paymentPanelCss), checkoutModal: classes(checkoutModalCss) };
  const names = Object.keys(sheets) as (keyof typeof sheets)[];
  for (const a of names) {
    for (const b of names) {
      if (a >= b) continue;
      const shared = [...sheets[a]].filter((c) => sheets[b].has(c) && !c.startsWith("shop-price"));
      expect(shared, `${a} and ${b}`).toEqual([]);
    }
  }
});
