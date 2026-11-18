// The overview page's sender (spec: docs/TRACKING_SPEC.md "Overview"). Its events feed sites' warehouse
// tables and Metabase cards, so they go to sites' Segment source. A second analytics.js instance with
// sites' key (the app's own instance carries the builder key) loads that source's browser destinations
// (GA4, GTM) as sites did; until it is ready, calls queue, and leave over the HTTP API if the page goes first.
import type { Analytics } from '@segment/analytics-next'
import { config } from '~/config'
import { eventProperties, IS_BOT, resolveAnalyticsUrl, sendDirect, stampApp } from '~/lib/analytics'
import { afterLoadIdle } from '~/lib/idle'
import type { SegmentCall } from '~/lib/segmentHttp'

type Props = Record<string, unknown>
type Queued = { call: SegmentCall; props: Props; calledAt: number }

const writeKey = () => config.get('SITES_SEGMENT_API_KEY', '')

let sites: Analytics | undefined
let state: 'idle' | 'loading' | 'ready' | 'failed' = 'idle'
let queue: Queued[] = []

// analytics-next's settings fetch has no timeout; past this, the queue stops waiting for it.
const LOAD_TIMEOUT_MS = 10_000

// `track_*` mirror sites' observability fields: `track_deferred` marks a call that did not go through
// analytics.js the moment it was made.
function deliver({ call, props, calledAt }: Queued, deferred: boolean): void {
  if (call.type === 'page') {
    if (state === 'ready' && sites) void sites.page(call.name, eventProperties())
    else sendDirect(writeKey(), call)
    return
  }
  const fields = { ...props, track_called_at: calledAt, track_delivered_at: Date.now(), track_deferred: deferred }
  if (state === 'ready' && sites) void sites.track(call.event, eventProperties(fields))
  else sendDirect(writeKey(), call, fields)
}

// `pagehide` is unreliable on mobile, where a backgrounded tab is often killed without it.
function flushIfHidden(): void {
  if (document.visibilityState === 'hidden') flush()
}

function flush(): void {
  const pending = queue
  queue = []
  for (const queued of pending) deliver(queued, true)
}

function load(): void {
  state = 'loading'
  // A visitor who leaves before analytics.js is ready still gets counted, over the unload-safe transport.
  window.addEventListener('pagehide', flush)
  document.addEventListener('visibilitychange', flushIfHidden)
  afterLoadIdle(() => {
    const timeout = setTimeout(() => {
      if (state !== 'loading') return
      state = 'failed'
      flush()
    }, LOAD_TIMEOUT_MS)
    void import('@segment/analytics-next')
      .then(({ AnalyticsBrowser }) => {
        const cdnURL = resolveAnalyticsUrl(config.get('SEGMENT_ANALYTICS_URL', ''))?.origin
        const apiHost = config.get('SEGMENT_API_HOST', '')
        const browser = AnalyticsBrowser.load(
          { writeKey: writeKey(), ...(cdnURL ? { cdnURL } : {}) },
          // keepalive: overview links leave the page in the same tab, which would cancel a plain fetch.
          {
            integrations: {
              'Segment.io': { ...(apiHost ? { apiHost } : {}), deliveryStrategy: { config: { keepalive: true } } }
            }
          }
        )
        stampApp(browser as unknown as Parameters<typeof stampApp>[0])
        return browser
      })
      .then(([analytics]) => {
        // Also after a timeout: a late instance still serves every later call.
        sites = analytics
        state = 'ready'
      })
      .catch(() => {
        if (state === 'loading') state = 'failed'
      })
      .finally(() => {
        clearTimeout(timeout)
        window.removeEventListener('pagehide', flush)
        document.removeEventListener('visibilitychange', flushIfHidden)
        flush()
      })
  })
}

function send(call: SegmentCall, props: Props): void {
  if (IS_BOT) return
  // Without a key (local dev) sendDirect logs the call instead.
  if (state === 'idle' && writeKey()) load()
  const queued = { call, props, calledAt: Date.now() }
  if (state === 'loading') queue.push(queued)
  else deliver(queued, state !== 'ready')
}

/** Sends an overview event. The first call starts loading sites' analytics.js, after load-idle. */
export function sendOverviewTrack(event: string, props: Props): void {
  send({ type: 'track', event }, props)
}

/** Sends the overview page view. */
export function sendOverviewPage(name: string): void {
  send({ type: 'page', name }, {})
}
