import { trackPage } from '~/lib/analytics'
import { trackOverviewPage } from '~/lib/overviewAnalytics'

// Stable page names for the funnel: a raw pathname carries collection ids and would never group.
const PAGE_NAMES: Record<string, string> = {
  '/collections': 'collections',
  '/collections/editor': 'item_editor',
  '/curation': 'curation',
  '/live-preview': 'live_preview'
}

// Redirect-only routes: the page they land on sends its own view, so old links don't count twice.
const REDIRECT_PATHS = ['/overview']

/** The page view for a new pathname (spec: docs/TRACKING_SPEC.md "Page views"). */
export function trackPageView(pathname: string): void {
  const path = pathname.replace(/\/+$/, '') || '/'
  if (REDIRECT_PATHS.includes(path)) return
  // The overview's view feeds sites' warehouse, under sites' name for it.
  if (path === '/') return trackOverviewPage()
  trackPage(PAGE_NAMES[path] ?? (path.startsWith('/collections/') ? 'collection_detail' : 'other'))
}
