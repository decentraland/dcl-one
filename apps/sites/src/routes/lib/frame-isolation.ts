/**
 * The Explorer's Create panel frames the site inside /play, which is cross-origin isolated (COEP
 * credentialless); a browser renders a document framed there only if it declares COEP as well.
 * Same-origin frames get it; top-level pages and other sites' frames are unchanged.
 */
export function applyFrameIsolation(request: Request, headers: Headers): void {
  headers.append("Vary", "Sec-Fetch-Dest, Sec-Fetch-Site");
  if (request.headers.get("sec-fetch-dest") === "iframe" && request.headers.get("sec-fetch-site") === "same-origin") {
    headers.set("Cross-Origin-Embedder-Policy", "credentialless");
  }
}
