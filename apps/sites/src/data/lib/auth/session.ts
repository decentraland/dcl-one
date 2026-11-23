import { check } from "@web-core/validate";

import { AuthIdentitySchema } from "../persisted-schemas";

import { isIdentityExpired } from "./expiry";
import { recordSignIn } from "./signin-kind";
import { clearAllShared, clearShared, freshestShared, SINGLE_SIGN_ON_PREFIX, storeShared } from "./single-sign-on";
import type { AuthIdentity } from "./types";
import { clearWalletCookie, serializeWalletCookie, walletInCookies } from "./wallet-cookie";

function syncWalletCookie(identity: AuthIdentity | null): void {
  if (typeof document === "undefined") return;
  document.cookie = identity
    ? serializeWalletCookie(identity.signer, identity.expiration)
    : clearWalletCookie();
}

export const SESSION_STORAGE_KEY = "dcl:auth:identity:v1";
const SUSPENDED_WALLET_KEY = "dcl:auth:suspended-wallet";

/**
 * The host's session, this site's own record, or the freshest one other dapps on this origin share. During a
 * wallet switch this tab waits for the selected account instead of adopting a fallback from another dapp.
 */
function readStorage(): AuthIdentity | null {
  if (typeof window === "undefined") return null;
  if (hostIdentity !== undefined) return hostIdentity;
  return readOwn() ?? (suspendedSigner ? null : freshestShared());
}

function readOwn(): AuthIdentity | null {
  let parsed: unknown;
  try {
    const raw = window.localStorage.getItem(SESSION_STORAGE_KEY);
    if (!raw) return null;
    parsed = JSON.parse(raw);
  } catch {
    return null;
  }
  const identity = check(AuthIdentitySchema, parsed, "persisted/auth-identity");
  if (
    !identity?.signer ||
    !identity?.ephemeral?.privateKey ||
    !Array.isArray(identity.authChain)
  ) {
    return null;
  }
  if (isIdentityExpired(identity) && identity.signer.toLowerCase() !== suspendedSigner) {
    try {
      window.localStorage.removeItem(SESSION_STORAGE_KEY);
    } catch {
    }
    return null;
  }
  return identity;
}

/**
 * Keeps a session in this site's record and the shared one. Wallet selection can leave the previous signature
 * available. Explicit sign-out ends every shared session on this origin.
 */
function writeStorage(
  identity: AuthIdentity | null,
  previous: AuthIdentity | null,
  keepPrevious: boolean,
): void {
  if (typeof window === "undefined") return;
  try {
    if (identity) {
      window.localStorage.setItem(SESSION_STORAGE_KEY, JSON.stringify(identity));
    } else {
      window.localStorage.removeItem(SESSION_STORAGE_KEY);
    }
  } catch {
  }
  if (!identity) clearAllShared();
  else if (!keepPrevious && previous && previous.signer !== identity.signer) clearShared(previous.signer);
  if (identity) storeShared(identity);
  syncWalletCookie(identity);
}

let current: AuthIdentity | null = null;
let hostIdentity: AuthIdentity | null | undefined;
let hydrated = false;
let suspendedSigner: string | null = null;
const listeners = new Set<() => void>();

function emit() {
  for (const l of listeners) l();
}

function ensureHydrated() {
  if (hydrated || typeof window === "undefined") return;
  try {
    suspendedSigner = window.sessionStorage.getItem(SUSPENDED_WALLET_KEY);
  } catch {
  }
  current = readStorage();
  hydrated = true;
}

export function getIdentity(): AuthIdentity | null {
  ensureHydrated();
  return hostIdentity === undefined && current?.signer.toLowerCase() === suspendedSigner ? null : current;
}

function setSuspendedSigner(signer: string | null): void {
  suspendedSigner = signer;
  try {
    if (signer) window.sessionStorage.setItem(SUSPENDED_WALLET_KEY, signer);
    else window.sessionStorage.removeItem(SUSPENDED_WALLET_KEY);
  } catch {
  }
}

/** Keeps the saved session available to the wallet watcher while this tab is signed out. */
export function getWebsiteIdentity(): AuthIdentity | null {
  ensureHydrated();
  return hostIdentity === undefined ? current : null;
}

export function getSuspendedWalletSigner(): string | null {
  ensureHydrated();
  return hostIdentity === undefined && !getIdentity() ? suspendedSigner : null;
}

export function suspendWalletIdentity(): void {
  const active = getWebsiteIdentity();
  if (!active || !getIdentity()) return;
  setSuspendedSigner(active.signer.toLowerCase());
  syncWalletCookie(null);
  emit();
}

function updateIdentity(identity: AuthIdentity | null, keepPrevious: boolean): void {
  if (hostIdentity !== undefined) throw new Error("Use the explorer to change accounts.");
  ensureHydrated();
  const previous = current;
  if (suspendedSigner) setSuspendedSigner(null);
  current = identity;
  hydrated = true;
  writeStorage(identity, previous, keepPrevious);
  emit();
}

export function setIdentity(identity: AuthIdentity | null): void {
  updateIdentity(identity, false);
}

/** A wallet-driven switch keeps the previous signature reusable. */
export function setBrowserWalletIdentity(identity: AuthIdentity, rdns: string, fromWalletEvent = false): void {
  if (hostOwnsSession()) throw new Error("Use the explorer to change accounts.");
  const suspended = getWebsiteIdentity() !== null && getIdentity() === null;
  recordSignIn(identity.signer, "web3", rdns);
  updateIdentity(identity, fromWalletEvent || suspended);
}

/** Native marketplace host owns this session. Never overwrite the website wallet. */
export function setHostIdentity(identity: AuthIdentity | null | undefined): void {
  hostIdentity = identity;
  current = identity === undefined ? readStorage() : identity;
  hydrated = true;
  emit();
}

export function clearIdentity(): void {
  setIdentity(null);
}

/** Whether a host (the Explorer) owns this document's session, leaving the website's untouched. */
export function hostOwnsSession(): boolean {
  return hostIdentity !== undefined;
}

/**
 * Points the server-side wallet hint at this document's session: the website's own on a page of
 * its own (mending one the Explorer's Create panel left behind when its tab closed or crashed), or
 * the Explorer's in a hub it frames (taking it back from a website tab that re-pointed it).
 * Returns whether the hint named another wallet until now, so loaders ran for that account.
 */
export function resyncWalletCookie(): boolean {
  if (typeof document === "undefined") return false;
  const identity = getIdentity();
  const mine = identity?.signer.toLowerCase() ?? null;
  const before = walletInCookies(document.cookie);
  syncWalletCookie(identity);
  // Only a write that took effect counts: with cookies blocked a caller must not reload forever.
  return before !== null && before !== mine && walletInCookies(document.cookie) === mine;
}

/**
 * Takes the wallet hint back for this document's session whenever the document is used again
 * (focused, pressed, shown, restored from the back/forward cache). Meanwhile other documents of
 * the origin point it at their own account, the Explorer's Create panel at the Explorer's and a
 * website tab at the website's, and this document's next loader request must not read theirs.
 */
export function claimWalletCookieOnUse(win: Window): () => void {
  const claim = () => void resyncWalletCookie();
  const onVisibility = () => {
    if (win.document.visibilityState === "visible") claim();
  };
  win.addEventListener("focus", claim);
  win.addEventListener("pointerdown", claim, { capture: true });
  win.addEventListener("pageshow", claim);
  win.document.addEventListener("visibilitychange", onVisibility);
  return () => {
    win.removeEventListener("focus", claim);
    win.removeEventListener("pointerdown", claim, { capture: true });
    win.removeEventListener("pageshow", claim);
    win.document.removeEventListener("visibilitychange", onVisibility);
  };
}

function onStorage(e: StorageEvent) {
  const shared = e.key?.startsWith(SINGLE_SIGN_ON_PREFIX) ?? false;
  // Another dapp on this origin signed this account out (it removed the account's shared session): this site's
  // record of the same account goes too, or the next read would sign the reader straight back in. Other accounts'
  // shared sessions stay.
  if (
    hostIdentity === undefined &&
    shared &&
    e.newValue === null &&
    current &&
    e.key === `${SINGLE_SIGN_ON_PREFIX}${current.signer.toLowerCase()}`
  ) {
    try {
      window.localStorage.removeItem(SESSION_STORAGE_KEY);
    } catch {
    }
  }
  if (hostIdentity === undefined && (e.key === null || e.key === SESSION_STORAGE_KEY || shared)) {
    current = readStorage();
    syncWalletCookie(getIdentity());
    emit();
  }
}

export function subscribe(listener: () => void): () => void {
  ensureHydrated();
  listeners.add(listener);
  if (typeof window !== "undefined" && listeners.size === 1) {
    window.addEventListener("storage", onStorage);
  }
  return () => {
    listeners.delete(listener);
    if (typeof window !== "undefined" && listeners.size === 0) {
      window.removeEventListener("storage", onStorage);
    }
  };
}

export function getServerSnapshot(): AuthIdentity | null {
  return null;
}
