import { useSyncExternalStore } from "react";

import { isDesktopShell } from "@/data/lib/auth/native-shell";
import { loginPath } from "../../auth/login/redirect";

let open = false;
let pendingRedirect: string | null = null;
const listeners = new Set<() => void>();
let hostSignIn: (() => void) | undefined;

export function installHostSignIn(signIn: () => void): () => void {
  hostSignIn = signIn;
  return () => { if (hostSignIn === signIn) hostSignIn = undefined; };
}

function emit(): void {
  for (const l of listeners) l();
}

/**
 * Asks the reader to sign in. A host (the Explorer) signs in its own way, and the desktop shell hands sign-in to the
 * system browser from the modal; everywhere else the reader goes to /auth/login, as on decentraland.org, and comes
 * back to `redirectTo` (this page when absent). `inPlace` keeps the reader on the page, in the sign-in modal: for a
 * caller asking mid-task, whose unsaved input a navigation would lose.
 */
export function openSignIn(opts?: { redirectTo?: string; inPlace?: boolean }): void {
  if (hostSignIn) { hostSignIn(); return; }
  if (typeof window !== "undefined" && !isDesktopShell() && !opts?.inPlace) {
    const here = `${window.location.pathname}${window.location.search}${window.location.hash}`;
    window.location.assign(loginPath(opts?.redirectTo ?? here));
    return;
  }
  pendingRedirect = opts?.redirectTo ?? null;
  if (!open) {
    open = true;
    emit();
  }
}

/** The page the reader asked to come back to, left in place (a sign-in that may still fail keeps it). */
export function peekPendingRedirect(): string | null {
  return pendingRedirect;
}

export function takePendingRedirect(): string | null {
  const r = pendingRedirect;
  pendingRedirect = null;
  return r;
}

export function closeSignIn(): void {
  if (open) {
    open = false;
    emit();
  }
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

function getSnapshot(): boolean {
  return open;
}

function getServerSnapshot(): boolean {
  return false;
}

export function useSignInOpen(): boolean {
  return useSyncExternalStore(subscribe, getSnapshot, getServerSnapshot);
}
