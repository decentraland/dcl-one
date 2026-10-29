import { useSyncExternalStore } from "react";

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

export function openSignIn(opts?: { redirectTo?: string }): void {
  if (hostSignIn) { hostSignIn(); return; }
  pendingRedirect = opts?.redirectTo ?? null;
  if (!open) {
    open = true;
    emit();
  }
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
