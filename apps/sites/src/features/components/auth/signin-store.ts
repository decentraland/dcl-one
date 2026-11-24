import { loginPath } from "../../auth/login/redirect";

let hostSignIn: (() => void) | undefined;

type BeforeBrowserSignIn = (returnTo: string, resume: () => void) => boolean | void;
const beforeBrowserSignIn = new Set<BeforeBrowserSignIn>();

export function onBeforeBrowserSignIn(listener: BeforeBrowserSignIn): () => void {
  beforeBrowserSignIn.add(listener);
  return () => {
    beforeBrowserSignIn.delete(listener);
  };
}

/** Gives pages a chance to save their drafts or ask before leaving for a new session. */
export function navigateForAccountChange(returnTo: string, go: () => void): void {
  const resume = () => navigateForAccountChange(returnTo, go);
  for (const listener of beforeBrowserSignIn) {
    if (listener(returnTo, resume) === false) return;
  }
  go();
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
  navigateForAccountChange(returnTo, () => window.location.assign(loginPath(returnTo)));
}
