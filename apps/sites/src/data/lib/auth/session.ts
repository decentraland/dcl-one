import { check } from "@ui/validate";

import { AuthIdentitySchema } from "../persisted-schemas";

import { isIdentityExpired } from "./expiry";
import type { AuthIdentity } from "./types";
import { clearWalletCookie, serializeWalletCookie, walletInCookies } from "./wallet-cookie";

function syncWalletCookie(identity: AuthIdentity | null): void {
  if (typeof document === "undefined") return;
  document.cookie = identity
    ? serializeWalletCookie(identity.signer, identity.expiration)
    : clearWalletCookie();
}

export const SESSION_STORAGE_KEY = "dcl:auth:identity:v1";

function readStorage(): AuthIdentity | null {
  if (typeof window === "undefined") return null;
  if (hostIdentity !== undefined) return hostIdentity;
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
  if (isIdentityExpired(identity)) {
    try {
      window.localStorage.removeItem(SESSION_STORAGE_KEY);
    } catch {
    }
    return null;
  }
  return identity;
}

function writeStorage(identity: AuthIdentity | null): void {
  if (typeof window === "undefined") return;
  try {
    if (identity) {
      window.localStorage.setItem(SESSION_STORAGE_KEY, JSON.stringify(identity));
    } else {
      window.localStorage.removeItem(SESSION_STORAGE_KEY);
    }
  } catch {
  }
  syncWalletCookie(identity);
}

let current: AuthIdentity | null = null;
let hostIdentity: AuthIdentity | null | undefined;
let hydrated = false;
const listeners = new Set<() => void>();

function emit() {
  for (const l of listeners) l();
}

function ensureHydrated() {
  if (hydrated || typeof window === "undefined") return;
  current = readStorage();
  hydrated = true;
}

export function getIdentity(): AuthIdentity | null {
  ensureHydrated();
  return current;
}

export function setIdentity(identity: AuthIdentity | null): void {
  if (hostIdentity !== undefined) throw new Error("Use the explorer to change accounts.");
  current = identity;
  hydrated = true;
  writeStorage(identity);
  emit();
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
  if (hostIdentity === undefined && (e.key === null || e.key === SESSION_STORAGE_KEY)) {
    current = readStorage();
    syncWalletCookie(current);
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
