import { runtimeBase } from './review-base'

// Every URL the Review builds at runtime goes through here. Pages keep logical paths under /dcl-ui-review/, and
// reviewUrl maps a logical path onto wherever the app is served: the root, a CDN prefix such as
// /@scope/package/1.2.3/, or a relative base.
const LOGICAL_ROOT = '/dcl-ui-review/'
// The build puts every script under <app root>/dcl-ui-review/assets/ (assetsDir in review.vite.config.ts), so a
// module's own URL leads back to the app root by this many levels.
const ASSETS_DEPTH_FROM_ROOT = '../../'

/** The app root as a URL prefix ending in "/": Vite's base when it is absolute, else derived from the module. */
function appRoot(base: string, moduleUrl: string): string {
  if (base === '' || base.startsWith('.')) return new URL(ASSETS_DEPTH_FROM_ROOT, moduleUrl).href
  return base.endsWith('/') ? base : `${base}/`
}

/** Maps a logical /dcl-ui-review/... path (query and hash included) onto the app root. */
function resolveReviewUrl(logicalPath: string, base: string, moduleUrl: string): string {
  if (!logicalPath.startsWith(LOGICAL_ROOT)) throw new Error(`Not a Review path: ${logicalPath}`)
  return appRoot(base, moduleUrl) + logicalPath.slice(1)
}

/** A gallery page URL for a base and module: the query string, without the leading "?". */
function resolvePageUrl(query: string, base: string, moduleUrl: string): string {
  return resolveReviewUrl(`${LOGICAL_ROOT}?${query}`, base, moduleUrl)
}

function reviewUrl(logicalPath: string): string {
  return resolveReviewUrl(logicalPath, runtimeBase(), import.meta.url)
}

/** A gallery page URL: the query string, without the leading "?". */
function reviewPageUrl(query: string): string {
  return resolvePageUrl(query, runtimeBase(), import.meta.url)
}

export { resolvePageUrl, resolveReviewUrl, reviewPageUrl, reviewUrl }
