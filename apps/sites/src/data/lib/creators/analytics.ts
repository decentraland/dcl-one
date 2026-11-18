// Segment wrapper for the creator funnel (spec: docs/TRACKING_SPEC.md). This is the ONLY place the
// app talks to Segment: callers use `track`/`identify`/`trackPage`/`sendDirect`, never `window.analytics`.
//
// Events land in the SAME Segment source as the legacy builder, and reuse its event names wherever a
// flow exists there, so old UI and new UI are comparable. Every event carries `source` to tell them
// apart — legacy builder sends none, so an absent `source` is the legacy app. `source` is therefore a
// reserved prop name: no event may pass its own.
//
// Nothing here is user-facing, so the copy/i18n rules don't apply. Never emit PII or secrets; wallet
// addresses are pseudonymous public ids and are allowed.
import { isbot } from 'isbot'
import { createAnonymousIdResolver, readStoredId } from '~/lib/segmentAnonymousId.helpers'
import { APP_VERSION, config, currentSearch } from '~/config'
import { currentAddress } from '~/lib/currentAddress'
import { postToSegment, type SegmentCall } from '~/lib/segmentHttp'
import { isWalletRejection, isWalletStateError } from '~/lib/walletErrors'

type Props = Record<string, unknown>

type SegmentApi = {
  track: (event: string, props?: Props) => void
  identify: (id: string, traits?: Props) => void
  page: (name?: string, props?: Props) => void
  reset: () => void
  ready: (cb: () => void) => void
  user: () => { anonymousId?: () => string | undefined } | undefined
  addSourceMiddleware?: (middleware: SourceMiddleware) => void
}

/** A call on its way out, before analytics.js hands it to the destinations. */
type SegmentPayload = { obj: { context?: Props } }
type SourceMiddleware = (params: { payload: SegmentPayload; next: (payload: SegmentPayload) => void }) => void

export const SOURCE = 'creators-website'

// One id per page load, so the steps of a single visit stitch together in the warehouse.
const SESSION_ID =
  typeof crypto !== 'undefined' && 'randomUUID' in crypto ? crypto.randomUUID() : `s-${String(performance.now())}`

// Crawlers would otherwise inflate every funnel metric. Evaluated once: the user agent can't change.
export const IS_BOT = typeof navigator !== 'undefined' && isbot(navigator.userAgent)

function segment(): SegmentApi | undefined {
  if (IS_BOT || typeof window === 'undefined') return undefined
  return (window as unknown as { analytics?: SegmentApi }).analytics
}

// Stamped on every event. The address is read at call time so pre- and post-sign-in events share one
// anonymousId. `version` mirrors the legacy builder, which stamps its package version on every payload.
function context(): Props {
  const address = currentAddress()
  return {
    source: SOURCE,
    version: APP_VERSION,
    address: address ?? null,
    is_signed_in: !!address,
    session_id: SESSION_ID,
    app_env: config.get('ENVIRONMENT')
  }
}

/** An event's properties as every call of this app sends them: the common props, then the caller's. */
export function eventProperties(props: Props = {}): Props {
  // `source` goes last: it is what separates this app from the legacy builder in the shared Segment
  // source, so no event may overwrite it.
  return { ...context(), ...props, source: SOURCE }
}

export function track(event: string, props: Props = {}): void {
  const payload = eventProperties(props)
  const analytics = segment()
  if (analytics) analytics.track(event, payload)
  else if (import.meta.env.DEV && !IS_BOT) console.debug('[analytics] track', event, payload)
}

// No `source` here on purpose: traits are USER-level and sticky, so it would permanently tag anyone who
// ever opened this app, whichever app they use next. Telling the two apart is the job of the event prop.
export function identify(address: string, traits: Props = {}): void {
  const analytics = segment()
  if (analytics) analytics.identify(address.toLowerCase(), traits)
  else if (import.meta.env.DEV && !IS_BOT) console.debug('[analytics] identify', address, traits)
}

/** Drops the identity↔anonymousId association so events after sign-out aren't attributed to the account. */
export function reset(): void {
  const analytics = segment()
  if (analytics) analytics.reset()
  else if (import.meta.env.DEV && !IS_BOT) console.debug('[analytics] reset')
}

/** A page view under a stable route name (the legacy builder sends the url only). */
export function trackPage(name: string): void {
  const analytics = segment()
  if (analytics) analytics.page(name, context())
  else if (import.meta.env.DEV && !IS_BOT) console.debug('[analytics] page', name)
}

/**
 * A coarse bucket for a failed flow. Never the raw message: it can carry ids, urls or wallet copy, and
 * free text doesn't aggregate. The domain errors (`PublishCollectionError`, `SellItemError`, …) already
 * carry the reason the UI branches on, so that is what gets reported.
 */
export function errorCode(error: unknown): string {
  const reason = (error as { reason?: unknown } | null)?.reason
  if (typeof reason === 'string' && reason !== '') return reason
  if (isWalletRejection(error)) return 'rejected'
  if (isWalletStateError(error)) return 'wallet'
  return 'unknown'
}

/** Segment's anonymous id, once analytics.js is loaded — what stitches Intercom to the analytics identity. */
export function getAnonymousId(): string | undefined {
  const analytics = segment()
  // Before analytics.js loads, `user` is a queueing stub and has no id to return yet.
  const user = analytics?.user?.()
  return typeof user?.anonymousId === 'function' ? user.anonymousId() : undefined
}

const USER_ID_KEY = 'ajs_user_id'

const anonymousIdResolver = createAnonymousIdResolver(getAnonymousId)

/** Uses the same browser identity for direct beacons and the loaded SDK. */
export const ensureAnonymousId = anonymousIdResolver.ensure

/**
 * Sends one call to the Segment source of `writeKey` over the HTTP API, with this app's common props and
 * the visitor's analytics.js identity. Unload-safe, so it also carries clicks on same-tab links.
 */
export function sendDirect(writeKey: string, call: SegmentCall, props: Props = {}): void {
  if (IS_BOT || typeof window === 'undefined') return
  const properties = eventProperties(props)
  if (!writeKey) {
    if (import.meta.env.DEV) console.debug('[analytics] direct', call, properties)
    return
  }
  postToSegment({
    writeKey,
    call,
    properties,
    anonymousId: ensureAnonymousId(),
    // The id analytics.js persisted on `identify`, so both transports agree on it byte for byte.
    userId: readStoredId(USER_ID_KEY),
    apiHost: config.get('SEGMENT_API_HOST', ''),
    app: { name: SOURCE, version: APP_VERSION },
    search: currentSearch()
  })
}

/** Runs `cb` once analytics.js is loaded (immediately-ish; never if analytics is disabled). */
export function onAnalyticsReady(cb: () => void): void {
  segment()?.ready(cb)
}

// Segment's snippet: stubs every method so calls made before analytics.js loads are queued and replayed.
const METHODS = [
  'track',
  'identify',
  'page',
  'group',
  'alias',
  'ready',
  'on',
  'once',
  'off',
  'reset',
  'user',
  'debug',
  'setAnonymousId',
  'addSourceMiddleware',
  'addDestinationMiddleware'
] as const

// A queued call of one of these carries the page context of the moment it was made, not of the replay.
const METHODS_WITH_PAGE_CONTEXT = new Set(['track', 'page', 'identify', 'alias', 'group'])

type Snippet = Props & {
  invoked?: boolean
  initialize?: boolean
  push: (args: unknown[]) => void
  addSourceMiddleware?: (middleware: SourceMiddleware) => void
}

function installSnippet(): Snippet | undefined {
  const anyWindow = window as unknown as { analytics?: Snippet }
  if (anyWindow.analytics?.initialize || anyWindow.analytics?.invoked) return anyWindow.analytics
  const analytics = (anyWindow.analytics ?? []) as unknown as Snippet
  analytics.invoked = true
  for (const method of METHODS) {
    analytics[method] = (...args: unknown[]) => {
      // Once analytics.js has loaded the global is the real one; forward to it, keeping it as the
      // receiver because its methods rely on the analytics instance as `this`.
      const loaded = anyWindow.analytics
      if (loaded?.initialize && loaded !== analytics) {
        return (loaded[method] as (...a: unknown[]) => unknown).apply(loaded, args)
      }
      if (METHODS_WITH_PAGE_CONTEXT.has(method)) {
        args.push({
          __t: 'bpc',
          p: window.location.pathname,
          u: window.location.href,
          s: window.location.search,
          t: document.title,
          r: document.referrer
        })
      }
      analytics.push([method, ...args])
      return analytics
    }
  }
  anyWindow.analytics = analytics
  return analytics
}

/**
 * The origin analytics.js loads from. Ad blockers drop `cdn.segment.com`, so the deployed config points
 * at a first party proxy — and `_cdn` has to point there too, because analytics.js fetches its SETTINGS
 * separately and would otherwise still hit (and lose them to) Segment's CDN.
 */
export function resolveAnalyticsUrl(url: string): URL | undefined {
  if (!url) return undefined
  try {
    const resolved = new URL(url, window.location.href)
    // https only: anything else is a misconfigured value, and falling back to Segment's CDN loses
    // ad-blocked visitors, where loading a bad url would lose everyone.
    return resolved.protocol === 'https:' ? resolved : undefined
  } catch {
    return undefined
  }
}

/**
 * Names this app in `context.app`, Segment's own field for it, on EVERY call the page makes — page and
 * identify included, where an event prop can't reach. It arrives in the warehouse as the canonical
 * `context_app_name` / `context_app_version` columns, which no caller prop can collide with; the
 * `source` and `version` props stay as the cross-app filter the legacy builder's queries already use.
 */
export function stampApp(analytics: Pick<SegmentApi, 'addSourceMiddleware'>): void {
  analytics.addSourceMiddleware?.(({ payload, next }) => {
    // `next` in a finally: a middleware that throws (or one handed an unexpected payload shape by a
    // future analytics.js) would otherwise swallow the call, and every event with it.
    try {
      const context = payload.obj.context
      if (context) context.app = { name: SOURCE, version: APP_VERSION }
    } finally {
      next(payload)
    }
  })
}

let initialized = false

/** Loads analytics.js. No-ops without a write key, and for bots. The first page view comes from the router. */
export function initAnalytics(): void {
  if (initialized || IS_BOT || typeof window === 'undefined') return
  initialized = true
  const writeKey = config.get('SEGMENT_API_KEY', '')
  if (!writeKey) {
    if (import.meta.env.DEV) console.debug('[analytics] no SEGMENT_API_KEY → events log to the console only')
    return
  }
  const snippet = installSnippet()
  if (!snippet) return
  stampApp(snippet)
  const proxy = resolveAnalyticsUrl(config.get('SEGMENT_ANALYTICS_URL', ''))
  if (proxy) snippet._cdn = proxy.origin
  snippet._writeKey = writeKey

  const script = document.createElement('script')
  script.async = true
  script.setAttribute('data-global-segment-analytics-key', 'analytics')
  script.src = proxy?.href ?? `https://cdn.segment.com/analytics.js/v1/${encodeURIComponent(writeKey)}/analytics.min.js`
  document.head.appendChild(script)
}
