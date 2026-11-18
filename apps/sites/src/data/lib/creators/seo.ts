const CANONICAL_BASE = 'https://decentraland.org/create'

/** The production URL search engines should index for an in-app path, whatever host serves the page. */
export function canonicalUrl(path: string): string {
  const trimmed = path.replace(/\/+$/, '')
  return trimmed ? `${CANONICAL_BASE}${trimmed.startsWith('/') ? '' : '/'}${trimmed}` : CANONICAL_BASE
}

/** Points the page's canonical link and `og:url` at `canonicalUrl(path)`. */
export function setCanonical(path: string): void {
  const url = canonicalUrl(path)
  document.querySelector('link[rel="canonical"]')?.setAttribute('href', url)
  document.querySelector('meta[property="og:url"]')?.setAttribute('content', url)
}
