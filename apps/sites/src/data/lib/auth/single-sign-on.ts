// Matches the storage format of @dcl/single-sign-on-client@0.1.0 (localStorageStoreIdentity, localStorageGetIdentity,
// localStorageClearIdentity: `single-sign-on-<lowercased address>`, JSON, dropped once expired) and @dcl/crypto's
// AuthIdentity (`ephemeralIdentity` with its public key), read as ui3's play/features/auth/sso.ts does (the freshest
// unexpired entry is the signed-in account).
// The session as every decentraland.org dapp keeps it, so a sign-in here is one there and the reverse. This site's
// own record (`dcl:auth:identity:v1`) stays its source of truth: this one mirrors it, and the freshest entry is
// adopted only while the own record is empty.

import type { AuthIdentity, AuthLink } from "./types";

export const SINGLE_SIGN_ON_PREFIX = "single-sign-on-";

type SharedIdentity = {
  ephemeralIdentity: { address: string; publicKey: string; privateKey: string };
  expiration: string;
  authChain: AuthLink[];
};

const ADDRESS = /^0x[0-9a-fA-F]{40}$/;

function key(address: string): string | null {
  return ADDRESS.test(address) ? `${SINGLE_SIGN_ON_PREFIX}${address.toLowerCase()}` : null;
}

function storage(): Storage | null {
  try {
    return typeof window === "undefined" ? null : window.localStorage;
  } catch {
    return null;
  }
}

const HEX_KEY = /^0x[0-9a-fA-F]+$/;

/** This site's identity in the shape decentraland.org's dapps share, with the ephemeral key's public half. */
export function toShared(identity: AuthIdentity, publicKey: string): SharedIdentity {
  return {
    ephemeralIdentity: {
      address: identity.ephemeral.address,
      publicKey,
      privateKey: identity.ephemeral.privateKey,
    },
    expiration: identity.expiration,
    authChain: identity.authChain,
  };
}

/** A shared identity as this site keeps one, or null when it is malformed, expired or not a signed-in account. */
export function fromShared(raw: unknown, now = Date.now()): AuthIdentity | null {
  if (!raw || typeof raw !== "object") return null;
  const shared = raw as Partial<SharedIdentity>;
  const privateKey = shared.ephemeralIdentity?.privateKey;
  const address = shared.ephemeralIdentity?.address;
  const publicKey = shared.ephemeralIdentity?.publicKey;
  if (typeof privateKey !== "string" || !/^0x[0-9a-fA-F]{64}$/.test(privateKey) || typeof address !== "string") return null;
  if (!Array.isArray(shared.authChain)) return null;
  const signer = shared.authChain.find((link) => link?.type === "SIGNER")?.payload?.trim();
  if (!signer || !ADDRESS.test(signer)) return null;
  const expiration = new Date(shared.expiration ?? "");
  if (Number.isNaN(expiration.getTime()) || expiration.getTime() <= now) return null;
  return {
    signer: signer.toLowerCase(),
    ephemeral: {
      address: address.toLowerCase(),
      privateKey: privateKey as `0x${string}`,
      ...(typeof publicKey === "string" && HEX_KEY.test(publicKey) ? { publicKey: publicKey as `0x${string}` } : {}),
    },
    expiration: expiration.toISOString(),
    authChain: shared.authChain,
  };
}

// Bumped by every write and clear, so a write still deriving its public key never lands after a later change.
let generation = 0;

/**
 * Mirrors a session under its account's shared key (an expired one is not written, as upstream). A session made here
 * carries its public key and is written at once; one without it (kept before it was recorded) derives it first.
 */
export function storeShared(identity: AuthIdentity): void | Promise<void> {
  const k = key(identity.signer);
  const store = storage();
  if (!k || !store || new Date(identity.expiration).getTime() <= Date.now()) return;
  const mine = ++generation;
  const write = (publicKey: string) => {
    if (mine !== generation) return;
    try {
      store.setItem(k, JSON.stringify(toShared(identity, publicKey)));
    } catch {
      // Storage full or blocked: this site's own record still holds the session.
    }
  };
  if (identity.ephemeral.publicKey) {
    write(identity.ephemeral.publicKey);
    return;
  }
  return import("viem/accounts")
    .then(({ privateKeyToAccount }) => write(privateKeyToAccount(identity.ephemeral.privateKey).publicKey))
    .catch(() => {});
}

/** Ends an account's shared session, here and in every dapp on this origin. */
export function clearShared(address: string): void {
  generation++;
  const k = key(address);
  try {
    if (k) storage()?.removeItem(k);
  } catch {
    // Blocked storage holds nothing to clear.
  }
}

/** Ends every shared session on this origin: a sign-out here signs the reader out of every dapp. */
export function clearAllShared(): void {
  generation++;
  const store = storage();
  if (!store) return;
  try {
    const keys: string[] = [];
    for (let i = 0; i < store.length; i++) {
      const k = store.key(i);
      if (k?.startsWith(SINGLE_SIGN_ON_PREFIX)) keys.push(k);
    }
    for (const k of keys) store.removeItem(k);
  } catch {
    // Blocked storage holds nothing to clear.
  }
}

/** The freshest unexpired shared session on this origin (the account signed in last), or null. */
export function freshestShared(now = Date.now()): AuthIdentity | null {
  const store = storage();
  if (!store) return null;
  let best: AuthIdentity | null = null;
  try {
    for (let i = 0; i < store.length; i++) {
      const k = store.key(i);
      if (!k?.startsWith(`${SINGLE_SIGN_ON_PREFIX}0x`)) continue;
      let parsed: unknown;
      try {
        parsed = JSON.parse(store.getItem(k) ?? "");
      } catch {
        continue;
      }
      const identity = fromShared(parsed, now);
      // The key names the account: an entry filed under another address is not that account's session.
      if (!identity || k !== key(identity.signer)) continue;
      if (!best || new Date(identity.expiration) > new Date(best.expiration)) best = identity;
    }
  } catch {
    return null;
  }
  return best;
}
