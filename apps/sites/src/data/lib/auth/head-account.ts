// The account of the stored session for a script in the document head, which runs before the page's code. It repeats
// the rules of this folder's readers: `readStorage`, `readOwn` and `getIdentity` (session.ts) and `freshestShared`
// (single-sign-on.ts). routes/shop/cart-pictures.test.ts checks it against those readers.
import { CREATOR_HUB_HOST_KEY } from "@web-core/creator-hub-host";

import { SESSION_STORAGE_KEY, SUSPENDED_WALLET_KEY } from "./session";
import { SINGLE_SIGN_ON_PREFIX } from "./single-sign-on";

type Keys = {
  /** This site's own session record. */
  own: string;
  /** The key prefix of the sessions that decentraland.org's dapps share. */
  shared: string;
  /** The property of the parent window where a host that owns the session waits (`creator-hub-host.ts`). */
  host: string;
  /** The tab's key that names the account a wallet switch put aside. */
  suspended: string;
};

type Json = Record<string, unknown>;

/**
 * The lower-case signer that `getIdentity` gives on a page load of a production build, or null for none. False while a
 * host that frames the page owns the session. It uses nothing outside its own body. Blocked storage throws.
 */
function headAccount(keys: Keys): string | false | null {
  const HEX_KEY = /^0x[0-9a-fA-F]{64}$/;
  const ADDRESS = /^0x[0-9a-fA-F]{40}$/;
  const record = (value: unknown): Json | null => (value && typeof value === "object" ? (value as Json) : null);
  const parse = (raw: string | null): Json | null => {
    try {
      return raw ? record(JSON.parse(raw)) : null;
    } catch {
      return null;
    }
  };
  try {
    const host = window.parent !== window ? record((window.parent as unknown as Json)[keys.host]) : null;
    if (host && host.version === 1) return false;
  } catch {
    // A parent window of another origin is not a host.
  }
  const storage = window.localStorage;
  const now = Date.now();

  // A production build reads a record that fails the schema check as it is (`check`), so only `readOwn`'s own checks
  // apply.
  const own = (): string | null => {
    const identity = parse(storage.getItem(keys.own));
    if (!identity || typeof identity.signer !== "string" || !identity.signer) return null;
    if (!record(identity.ephemeral)?.privateKey || !Array.isArray(identity.authChain)) return null;
    const time = Date.parse(identity.expiration as string);
    return Number.isFinite(time) && time > now ? identity.signer : null;
  };

  const shared = (): string | null => {
    let best: string | null = null;
    let bestTime = 0;
    for (let i = 0; i < storage.length; i++) {
      const key = storage.key(i);
      if (!key || key.indexOf(keys.shared + "0x") !== 0) continue;
      const identity = parse(storage.getItem(key));
      if (!identity) continue;
      const ephemeral = record(identity.ephemeralIdentity);
      const authChain = identity.authChain;
      if (!ephemeral || typeof ephemeral.privateKey !== "string" || !HEX_KEY.test(ephemeral.privateKey) || typeof ephemeral.address !== "string") continue;
      if (!Array.isArray(authChain)) continue;
      const payload = record(authChain.find((link) => record(link)?.type === "SIGNER"))?.payload;
      const signer = typeof payload === "string" ? payload.trim() : "";
      if (!ADDRESS.test(signer) || key !== keys.shared + signer.toLowerCase()) continue;
      const time = new Date((identity.expiration as string | number | undefined) ?? "").getTime();
      if (Number.isNaN(time) || time <= now) continue;
      if (best === null || time > bestTime) {
        best = signer.toLowerCase();
        bestTime = time;
      }
    }
    return best;
  };

  // During a wallet switch the page takes no shared session, and no session of the account put aside.
  let suspended: string | null = null;
  try {
    suspended = window.sessionStorage.getItem(keys.suspended);
  } catch {
    // Blocked storage: no switch is known.
  }
  const signer = (own() ?? (suspended ? null : shared()))?.toLowerCase() ?? null;
  return signer === suspended ? null : signer;
}

/** `headAccount` as the source of a function with no parameters, for a head script to call. */
export function headAccountScript(): string {
  const keys: Keys = {
    own: SESSION_STORAGE_KEY,
    shared: SINGLE_SIGN_ON_PREFIX,
    host: CREATOR_HUB_HOST_KEY,
    suspended: SUSPENDED_WALLET_KEY,
  };
  return `function(){return(${headAccount.toString()})(${JSON.stringify(keys).replace(/</g, "\\u003c")})}`;
}
