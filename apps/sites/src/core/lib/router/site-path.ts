// from: auth@e081cc6:src/hooks/redirection.ts (useAfterLoginRedirection), src/shared/locations.ts
//       (extractRedirectToFromSearchParameters); @dcl/schemas validateUrlInstance (allowLocalhost, allowedPorts)
// decentraland/auth is Apache-2.0.
// A return destination is accepted only when it stays on this site: a path, or an absolute URL with the given
// exact origin, over https (plain HTTP URLs only on loopback). A root-relative path keeps the current scheme and
// origin, including on a LAN preview. The answer is the parsed path, never the raw text: browsers drop tabs and
// newlines from a URL, so text such as `/\t/host` that looks like a path is another host to whatever navigates to it.

const LOCAL_HOSTS = new Set(["localhost", "127.0.0.1", "[::1]"]);

function isRootRelative(path: string): boolean {
  return /^\/(?![/\\])/.test(path);
}

/** `raw` as a path on the site at `origin` (query and fragment kept), or null when it leads anywhere else. */
export function sitePath(raw: string | null, origin: string): string | null {
  if (!raw) return null;
  let target: URL;
  try {
    target = new URL(raw, origin);
  } catch {
    return null;
  }
  const schemeOk =
    target.protocol === "https:" || (target.protocol === "http:" && (isRootRelative(raw) || LOCAL_HOSTS.has(target.hostname)));
  // A path opening with `//` or `/\` reads as another host to whatever parses it next (history, the address bar).
  if (!schemeOk || !isRootRelative(target.pathname) || target.origin !== origin) return null;
  return `${target.pathname}${target.search}${target.hash}`;
}
