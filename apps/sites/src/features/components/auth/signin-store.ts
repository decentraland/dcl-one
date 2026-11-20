import { loginPath } from "../../auth/login/redirect";

let hostSignIn: (() => void) | undefined;

const beforeBrowserSignIn = new Set<(returnTo: string) => boolean | void>();

export function onBeforeBrowserSignIn(listener: (returnTo: string) => boolean | void): () => void {
  beforeBrowserSignIn.add(listener);
  return () => {
    beforeBrowserSignIn.delete(listener);
  };
}

export function installHostSignIn(signIn: () => void): () => void {
  hostSignIn = signIn;
  return () => {
    if (hostSignIn === signIn) hostSignIn = undefined;
  };
}

export function openSignIn(opts?: { redirectTo?: string }): void {
  if (hostSignIn) {
    hostSignIn();
    return;
  }
  if (typeof window === "undefined") return;
  const { pathname, search, hash } = window.location;
  const returnTo = opts?.redirectTo ?? pathname + search + hash;
  for (const listener of beforeBrowserSignIn) {
    if (listener(returnTo) === false) return;
  }
  window.location.assign(loginPath(returnTo));
}
