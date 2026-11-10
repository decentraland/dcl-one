/**
 * The one check for where the site may send a reader next (after sign-in, after a redirect): a path on
 * this site, as its normalized pathname, query and fragment, so a dot segment can never turn into `//`.
 * A path is kept only when it starts with a single "/" and nothing in it, raw, percent-decoded or
 * normalized, can make a browser read it as another origin (`//host`, `/\host`, `/%2F%2Fhost`,
 * `/%5Chost`, `/.//host`), and it carries no whitespace, control character or backslash.
 * Given the page's URL (`here`), an absolute URL with exactly that origin, over https (http only on a
 * local host), is also kept, reduced to its path, which then passes the same checks: /auth/login ports
 * decentraland.org's sign-in, which other Decentraland apps link to with such a `redirectTo`.
 * Anything else, `javascript:` and `data:` included, is refused.
 */
export function safeReturn(
  raw: string | null | undefined,
  here?: URL,
): string | null {
  if (!raw) return null;
  if (raw[0] === "/") return sameSitePath(raw);
  return here ? sameOriginPath(raw, here) : null;
}

/** `safeReturn`, with the page's own `fallback` (`/` for sign-in, `/blog` for the blog) when refused. */
export function safeReturnPath(
  raw: string | null | undefined,
  { fallback, here }: { fallback: string; here?: URL },
): string {
  return safeReturn(raw, here) ?? fallback;
}

const BASE = "https://same-site.invalid";
const LOCAL_HOSTS = new Set(["localhost", "127.0.0.1", "[::1]"]);
// A path opening with `//` or `/\` reads as another host to the next parser.
const HOST_LIKE = /^\/[\\/]/;
// eslint-disable-next-line no-control-regex
const UNSAFE_RAW = /[\u0000- \u007f\\]/;

function sameSitePath(raw: string): string | null {
  if (UNSAFE_RAW.test(raw)) return null;
  let decoded: string;
  try {
    decoded = decodeURIComponent(raw);
  } catch {
    return null;
  }
  // eslint-disable-next-line no-control-regex
  if (HOST_LIKE.test(decoded) || /[\u0000-\u001f\u007f]/.test(decoded))
    return null;
  const url = new URL(raw, BASE);
  if (url.origin !== BASE || HOST_LIKE.test(url.pathname)) return null;
  return `${url.pathname}${url.search}${url.hash}`;
}

function sameOriginPath(raw: string, here: URL): string | null {
  if (UNSAFE_RAW.test(raw)) return null;
  let url: URL;
  try {
    url = new URL(raw);
  } catch {
    return null;
  }
  const schemeOk =
    url.protocol === "https:" ||
    (url.protocol === "http:" && LOCAL_HOSTS.has(url.hostname));
  if (!schemeOk || url.origin !== here.origin) return null;
  return sameSitePath(`${url.pathname}${url.search}${url.hash}`);
}
