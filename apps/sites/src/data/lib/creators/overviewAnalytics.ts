// Overview page analytics (spec: docs/TRACKING_SPEC.md "Overview"). The page moved here from the sites
// repo, so the event names and sites' props are kept for warehouse continuity.
import type { SyntheticEvent } from 'react'
import { SITE_PATH } from '~/config'
import { collectCampaignParams } from '~/lib/campaignParams'
import { macArchHint } from '~/lib/creatorHubDownload'
import { sendOverviewPage, sendOverviewTrack } from '~/lib/overviewSegment'

/** `section_viewed` values, one per section of the overview page. */
export const OverviewSection = {
  HERO: 'Creators Hero',
  WHY: 'Creators Why',
  CREATE: 'Creators Create',
  LIVE_SCENES: 'Creators Live Scenes',
  CONNECT: 'Creators Connect',
  LEARN: 'Creators Learn',
  BLOG: 'Creators Blog',
  FAQS: 'Creators Faqs'
} as const

export type OverviewSection = (typeof OverviewSection)[keyof typeof OverviewSection]

export const SECTION_VIEWED_EVENT = 'Section Viewed'
export const CLICK_EVENT = 'Click'
/** sites' `SegmentEvent.DOWNLOAD`, the `event` subtype of a Creator Hub download click. */
export const DOWNLOAD_CLICK = 'Download'
/** sites' `DownloadTarget.CREATOR_HUB`. */
export const CREATOR_HUB_TARGET = 'creator_hub'

/**
 * The page's mobile cut, shared by every check that changes its layout or its `mobile` flag. sites used
 * `(max-width: 767px)` and ui2's `down('xs')` (767.95px): below the app's 768px canonical breakpoint.
 */
export const OVERVIEW_MOBILE_QUERY = '(max-width: 767.98px)'

/** Fired once per section per page load, the first time it scrolls into view. */
export function trackSectionViewed(section: OverviewSection, mobile: boolean): void {
  sendOverviewTrack(SECTION_VIEWED_EVENT, { section_viewed: section, mobile })
}

// The warehouse dimensions of a click, read from the element that describes itself through them. A
// whitelist: styled primitives stamp their own `data-*` (variant, size, testid) that must not leak.
const CLICK_DIMENSIONS = [
  ['place', 'place'],
  ['title', 'title'],
  ['card', 'card'],
  ['tab', 'tab'],
  ['event', 'event'],
  ['download-target', 'download_target'],
  ['download-mode', 'download_mode'],
  ['os', 'os'],
  ['arch', 'arch'],
  ['download-option', 'download_option']
] as const

/** The `Click` payload: the URL's campaign params, then the element's whitelisted `data-*` values. */
export function clickPayload(element: Element): Record<string, string> {
  const payload: Record<string, string> = collectCampaignParams()
  for (const [attribute, prop] of CLICK_DIMENSIONS) {
    const value = element.getAttribute(`data-${attribute}`)
    if (value) payload[prop] = value
  }
  // sites drops an `event` that would only repeat the event name.
  if (payload.event === CLICK_EVENT) delete payload.event
  // sites' Intel-Mac cohort: every download CTA click carries the Mac's chip, read at click time.
  const macArch = payload.download_target ? macArchHint() : null
  if (macArch) payload.mac_arch = macArch
  return payload
}

/** Tracks a click on an element that describes itself through `data-place` / `data-title` / … */
export function trackClick(event: SyntheticEvent<Element>): void {
  sendOverviewTrack(CLICK_EVENT, clickPayload(event.currentTarget))
}

/** `place` of sites' shared navbar and footer clicks. */
export const LandingPlace = {
  NAVBAR: 'Landing Navbar',
  FOOTER_LINK: 'Landing Footer Link',
  FOOTER_SOCIAL: 'Landing Footer Social'
} as const

/** The overview route, the only page whose navbar and footer clicks feed sites' landing tables. */
export const isOverviewPath = (pathname: string) => pathname === '/'

/** A navbar / footer click, in sites' shape: `{ place, event: 'click', action | link | platform }`. */
export function trackLandingClick(
  place: (typeof LandingPlace)[keyof typeof LandingPlace],
  extra: Record<string, string>
) {
  sendOverviewTrack(CLICK_EVENT, { place, event: 'click', ...extra })
}

/** The overview page view, under the full pathname sites sent. */
export function trackOverviewPage(): void {
  sendOverviewPage(SITE_PATH)
}
