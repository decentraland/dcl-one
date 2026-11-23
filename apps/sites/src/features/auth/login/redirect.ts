// Where the page sends a reader once signed in. Only this site qualifies (`sitePath`); anything else lands on the
// home page, so a crafted link cannot carry a fresh session somewhere else. The answer is a path, never a URL, so
// whatever follows it (a full navigation, or the social callback's in-app one) cannot leave the site either.

import { sitePath } from "@/core/lib/router/site-path";

export const HOME = "/";

/** The query's `redirectTo` as a path on this site (query and fragment kept), or `HOME` when it is not one. */
export function safeRedirectTarget(redirectTo: string | null, here: URL): string {
  return sitePath(redirectTo, here.origin) ?? HOME;
}

/** The sign-in page's URL for a reader who should come back to `returnTo` (a path on this site). */
export function loginPath(returnTo: string): string {
  return `/auth/login?redirectTo=${encodeURIComponent(returnTo)}`;
}
