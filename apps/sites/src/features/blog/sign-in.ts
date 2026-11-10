import { safeReturnPath } from "@/core/lib/navigation/safe-return";

const BLOG = "/blog";
const SIGN_IN_PATH = "/blog/sign-in";

/**
 * Where /blog/sign-in returns after signing in: the request's `redirectTo` when it is a same-site
 * path, else /blog. A target that is /blog/sign-in itself (with any query or hash, a trailing slash,
 * other letter case or percent-encoding) is replaced by /blog, so the page never sends the visitor back to itself.
 */
export function blogSignInTarget(
  redirectTo: string | null | undefined,
): string {
  const safe = safeReturnPath(redirectTo, { fallback: BLOG });
  // The router matches paths case-insensitively and decoded, so compare the same way.
  const { pathname } = new URL(safe, "https://same-site.invalid");
  let path: string;
  try {
    path = decodeURIComponent(pathname);
  } catch {
    return BLOG;
  }
  return path.replace(/\/+$/, "").toLowerCase() === SIGN_IN_PATH ? BLOG : safe;
}
