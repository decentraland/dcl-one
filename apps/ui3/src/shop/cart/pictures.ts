import { PAYMENT_METHODS } from "@shop/model/settlement";

import { ANONYMOUS, CART_KEY_PREFIX } from "./store";

/** The cart lines whose pictures load at once: the cart page's first screen. */
export const PRIORITY_LINE_COUNT = 3;

/** Where the cart document finds the session that names the cart's account (apps/sites/src/data/lib/auth). */
export type CartPicturesSession = {
  /** This site's own session record (`session.ts` `readOwn`). */
  own: string;
  /** The key prefix of the sessions that decentraland.org's dapps share (`single-sign-on.ts` `freshestShared`). */
  shared: string;
  /** The property of the parent window where a host that owns the session waits (`creator-hub-host.ts`). */
  host: string;
};

type CartPicturesConfig = CartPicturesSession & {
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
 * of the readers that the page uses: `readOwn` and `freshestShared` for the session, `readLines` and `decodeLine` for
 * the cart. It only reads. It does nothing while a host that frames the page owns the session.
 */
function preloadCartPictures(config: CartPicturesConfig): void {
  const HEX_KEY = /^0x[0-9a-fA-F]{64}$/;
  const ADDRESS = /^0x[0-9a-fA-F]{40}$/;
  const DECIMAL = /^[0-9]{1,78}$/;
  const LINK_TYPES = ["SIGNER", "ECDSA_EPHEMERAL", "ECDSA_SIGNED_ENTITY"];
  const record = (value: unknown): Json | null => (value && typeof value === "object" ? (value as Json) : null);
  const parse = (raw: string | null): Json | null => {
    try {
      return raw ? record(JSON.parse(raw)) : null;
    } catch {
      return null;
    }
  };
  try {
    try {
      const host = window.parent !== window ? record((window.parent as unknown as Json)[config.host]) : null;
      if (host && host.version === 1) return;
    } catch {
      // A parent window of another origin is not a host.
    }
    const storage = window.localStorage;
    const now = Date.now();

    const own = (): string | null => {
      const identity = parse(storage.getItem(config.own));
      if (!identity) return null;
      const { signer, expiration, authChain } = identity;
      const ephemeral = record(identity.ephemeral);
      if (typeof signer !== "string" || !signer || typeof expiration !== "string" || !Array.isArray(authChain)) return null;
      if (!ephemeral || typeof ephemeral.address !== "string" || typeof ephemeral.privateKey !== "string" || !HEX_KEY.test(ephemeral.privateKey)) return null;
      for (const value of authChain) {
        const link = record(value);
        if (!link || LINK_TYPES.indexOf(link.type as string) < 0 || typeof link.payload !== "string" || typeof link.signature !== "string") return null;
      }
      const time = Date.parse(expiration);
      return Number.isFinite(time) && time > now ? signer : null;
    };

    const shared = (): string | null => {
      let best: string | null = null;
      let bestTime = 0;
      for (let i = 0; i < storage.length; i++) {
        const key = storage.key(i);
        if (!key || key.indexOf(config.shared + "0x") !== 0) continue;
        const identity = parse(storage.getItem(key));
        if (!identity) continue;
        const ephemeral = record(identity.ephemeralIdentity);
        const authChain = identity.authChain;
        if (!ephemeral || typeof ephemeral.privateKey !== "string" || !HEX_KEY.test(ephemeral.privateKey) || typeof ephemeral.address !== "string") continue;
        if (!Array.isArray(authChain)) continue;
        const payload = record(authChain.find((link) => record(link)?.type === "SIGNER"))?.payload;
        const signer = typeof payload === "string" ? payload.trim() : "";
        if (!ADDRESS.test(signer) || key !== config.shared + signer.toLowerCase()) continue;
        const time = new Date((identity.expiration as string | number | undefined) ?? "").getTime();
        if (Number.isNaN(time) || time <= now) continue;
        if (best === null || time > bestTime) {
          best = signer.toLowerCase();
          bestTime = time;
        }
      }
      return best;
    };

    const decimal = (value: unknown) => typeof value === "string" && DECIMAL.test(value) && BigInt(value) < 2n ** 256n;
    const lines = (account: string): { id: string; picture: unknown }[] => {
      const cart = parse(storage.getItem(config.carts + account));
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

    const signer = own() ?? shared();
    const ids: string[] = [];
    const pictures: unknown[] = [];
    for (const account of signer ? [signer.toLowerCase(), config.anonymous] : [config.anonymous]) {
      for (const line of lines(account)) {
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

/** `preloadCartPictures` as an inline script for the head of the cart document, called in a scope of its own. */
export function cartPicturesScript(session: CartPicturesSession): string {
  const config: CartPicturesConfig = { ...session, carts: CART_KEY_PREFIX, anonymous: ANONYMOUS, methods: PAYMENT_METHODS, count: PRIORITY_LINE_COUNT };
  return `(${preloadCartPictures.toString()})(${JSON.stringify(config).replace(/</g, "\\u003c")})`;
}
