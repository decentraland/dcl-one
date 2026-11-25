import { PAYMENT_METHODS } from "@shop/model/settlement";

import { ANONYMOUS, CART_KEY_PREFIX } from "./store";

/** The cart lines whose pictures load at once: the cart page's first screen. */
export const PRIORITY_LINE_COUNT = 3;

type CartPicturesConfig = {
  carts: string;
  anonymous: string;
  methods: readonly string[];
  count: number;
};

type Json = Record<string, unknown>;

/**
 * Preloads with high priority the pictures of the first `count` stored cart lines, in the order that the cart store
 * merges them: the lines of the signed-in account, then the anonymous lines that it does not have. It runs while the
 * document's head parses, before the page's code. Thus it uses nothing outside its own body, and it repeats the checks
 * of the cart readers that the page uses: `readLines` and `decodeLine`. `account` gives the lower-case signer of the
 * stored session, null when signed out, or false while the page must not read the stored session. It only reads.
 */
function preloadCartPictures(config: CartPicturesConfig, account: () => string | false | null): void {
  const ADDRESS = /^0x[0-9a-fA-F]{40}$/;
  const DECIMAL = /^[0-9]{1,78}$/;
  const record = (value: unknown): Json | null => (value && typeof value === "object" ? (value as Json) : null);
  const parse = (raw: string | null): Json | null => {
    try {
      return raw ? record(JSON.parse(raw)) : null;
    } catch {
      return null;
    }
  };
  try {
    const signer = account();
    if (signer === false) return;
    const storage = window.localStorage;
    const decimal = (value: unknown) => typeof value === "string" && DECIMAL.test(value) && BigInt(value) < 2n ** 256n;
    const lines = (owner: string): { id: string; picture: unknown }[] => {
      const cart = parse(storage.getItem(config.carts + owner));
      if (!cart || cart.version !== 1 || !Array.isArray(cart.lines)) return [];
      const read: { id: string; picture: unknown }[] = [];
      for (const value of cart.lines) {
        const line = record(value);
        const asset = record(line?.asset);
        const token = record(asset?.token);
        const display = record(line?.display);
        if (!line || typeof line.id !== "string" || !display || typeof display.name !== "string") continue;
        if (line.method != null && config.methods.indexOf(line.method as string) < 0) continue;
        if (!asset || !token || typeof asset.contract !== "string" || !ADDRESS.test(asset.contract)) continue;
        if (typeof asset.chainId !== "number" || !Number.isSafeInteger(asset.chainId) || asset.chainId <= 0) continue;
        if (token.kind === "item" ? !decimal(token.itemId) : token.kind !== "token" || !decimal(token.tokenId)) continue;
        read.push({ id: line.id, picture: display.thumbnail });
      }
      return read;
    };

    const ids: string[] = [];
    const pictures: unknown[] = [];
    for (const owner of signer ? [signer, config.anonymous] : [config.anonymous]) {
      for (const line of lines(owner)) {
        if (ids.length === config.count || ids.indexOf(line.id) >= 0) continue;
        ids.push(line.id);
        pictures.push(line.picture);
      }
    }
    for (const picture of pictures) {
      if (typeof picture !== "string" || !picture) continue;
      const link = document.createElement("link");
      link.rel = "preload";
      link.setAttribute("as", "image");
      link.setAttribute("fetchpriority", "high");
      link.href = picture;
      document.head.appendChild(link);
    }
  } catch {
    // Blocked storage or a record that no reader expects: the page loads the pictures itself.
  }
}

/**
 * `preloadCartPictures` as an inline script for the head of the cart document, called in a scope of its own. `account`
 * is the source of its `account` function: the site that owns the session supplies it.
 */
export function cartPicturesScript(account: string): string {
  const config: CartPicturesConfig = { carts: CART_KEY_PREFIX, anonymous: ANONYMOUS, methods: PAYMENT_METHODS, count: PRIORITY_LINE_COUNT };
  return `(${preloadCartPictures.toString()})(${JSON.stringify(config).replace(/</g, "\\u003c")},${account})`;
}
