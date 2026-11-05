// from: auth@e081cc6:src/hooks/redirection.ts (useAfterLoginRedirection), src/shared/locations.ts
//       (extractRedirectToFromSearchParameters); @dcl/schemas validateUrlInstance (allowLocalhost, allowedPorts)
// decentraland/auth is Apache-2.0.
// Where the page sends a reader once signed in. Only this site qualifies: a path, or an absolute URL with this page's
// exact origin, over https (http only on a local host). Anything else lands on the home page, so a crafted link cannot
// carry a fresh session somewhere else. The answer is a path, never a URL, so whatever follows it (a full navigation,
// or the social callback's in-app one) cannot leave the site either.

export const HOME = "/";

const LOCAL_HOSTS = new Set(["localhost", "127.0.0.1", "[::1]"]);

/** The query's `redirectTo` as a path on this site (query and fragment kept), or `HOME` when it is not one. */
export function safeRedirectTarget(redirectTo: string | null, here: URL): string {
  if (!redirectTo) return HOME;
  let target: URL;
  try {
    target = new URL(redirectTo, here.origin);
  } catch {
    return HOME;
  }
  const schemeOk = target.protocol === "https:" || (target.protocol === "http:" && LOCAL_HOSTS.has(target.hostname));
  // A path opening with `//` or `/\` reads as another host to whatever parses it next (history, the address bar).
  const pathOk = target.pathname.startsWith("/") && !/^\/[/\\]/.test(target.pathname);
  if (!schemeOk || !pathOk || target.origin !== here.origin) return HOME;
  return `${target.pathname}${target.search}${target.hash}`;
}

/** The sign-in page's URL for a reader who should come back to `returnTo` (a path on this site). */
export function loginPath(returnTo: string): string {
  return `/auth/login?redirectTo=${encodeURIComponent(returnTo)}`;
}
