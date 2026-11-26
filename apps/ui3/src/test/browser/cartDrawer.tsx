// A page for CartDrawer.browser.test.ts: a long page with the cart drawer's opener far down it, run in a real browser.
import { createRoot } from "react-dom/client";
import { MemoryRouter } from "react-router";
import { evmAddress } from "@shop/model/address";
import { itemAsset } from "@shop/model/asset";
import { CHAIN } from "@shop/model/chain";
import "../../atoms/primitives.css";
import { createCartDrawer } from "../../shop/cart/drawer";
import { createCartStore } from "../../shop/cart/store";
import CartDrawer from "../../shop/checkout/components/CartDrawer";

const cart = createCartStore({ storage: () => null });
cart.add({ asset: itemAsset(CHAIN.POLYGON, evmAddress("0x000000000000000000000000000000000000c011"), 7n), listing: { acquisition: "store" }, display: { name: "Rainbow Hat", priceCredits: 12 } });
// The test reads the drawer's state as `window.cartDrawer`.
const drawer = createCartDrawer();
Object.assign(window, { cartDrawer: drawer });

createRoot(document.getElementById("root")!).render(
  <MemoryRouter initialEntries={["/shop/items"]}>
    <div style={{ height: 1500 }} />
    <button type="button" id="opener" onClick={() => drawer.open()}>
      Cart
    </button>
    <div style={{ height: 3000 }} />
    <CartDrawer cart={cart} drawer={drawer} />
  </MemoryRouter>,
);
