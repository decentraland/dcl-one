// decentraland.org's `/sign-in` and `/blog/sign-in` only sent the visitor to the sign-in page, with a return path
// worked out from the URL they were opened with. These are those rules, so old links keep landing where they did.

/** `new URL(path, origin)` as the reference resolved it, kept to its path and query; null when it cannot be read. */
function pathAndQuery(path: string, origin: string): string | null {
  try {
    const url = new URL(path, origin);
    return url.pathname + url.search;
  } catch {
    return null;
  }
}

/**
 * The return path of `/sign-in` (decentraland/sites `pages/SignInRedirect.tsx` and `utils/authRedirect.ts`): a
 * non-empty `redirectTo`, else the URL's own path and query, resolved against this origin and kept to its path and
 * query.
 */
export function signInReturnPath(url: URL): string {
  const own = url.pathname + url.search;
  return pathAndQuery(url.searchParams.get("redirectTo") || own, url.origin) ?? own;
}

/**
 * The return path of `/blog/sign-in` (decentraland/sites `utils/blogAuthRedirect.ts`): a `redirectTo` even when
 * empty, else the URL's own path and query. On a decentraland.org, .zone or .today host the path is put under
 * `/blog` (the blog's old basename), after removing one leading `/blog` so it is not doubled.
 */
export function blogSignInReturnPath(url: URL): string {
  const basename = /^decentraland.(zone|org|today)$/.test(url.host) ? "/blog" : "";
  const own = url.pathname + url.search;
  let path = pathAndQuery(url.searchParams.get("redirectTo") ?? own, url.origin) ?? own;
  if (basename && path.startsWith(basename)) path = path.slice(basename.length) || "/";
  return `${basename}${path}`;
}
