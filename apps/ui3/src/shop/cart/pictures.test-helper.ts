// Fixtures of the cart pictures head script's tests: pictures.test.ts, CartPage.test.tsx and
// apps/sites/src/routes/shop/cart-pictures.test.ts.
import { evmAddress } from "@shop/model/address";
import { itemAsset } from "@shop/model/asset";
import { CHAIN } from "@shop/model/chain";

/** The script's account function for these tests: signed out. The Sites test supplies the site's real one. */
export const SIGNED_OUT = "function(){return null}";

const collection = evmAddress("0x000000000000000000000000000000000000c011");

export const picture = (n: number) => `https://example.test/${n}.png`;

/** A store mint of hat `n`, with its picture unless `pictured` is false. */
export const hat = (n: number, pictured = true) => ({
  asset: itemAsset(CHAIN.POLYGON, collection, BigInt(n)),
  listing: { acquisition: "store" as const },
  display: { name: `Hat ${n}`, ...(pictured ? { thumbnail: picture(n) } : {}) },
});

/** The image preloads in the document head. */
export const preloads = () => [...document.head.querySelectorAll<HTMLLinkElement>('link[rel="preload"][as="image"]')];

/** The pictures of those preloads, in order. */
export const preloaded = () => preloads().map((link) => link.getAttribute("href"));
