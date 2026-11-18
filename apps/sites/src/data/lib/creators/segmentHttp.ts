// Posts single calls to Segment's HTTP Tracking API, outside analytics.js. analytics.js can only run one
// write key per page, and its fetch is cancelled when a same-tab link unloads the page; a beacon is not.
// The write key travels in the body (no Authorization header) so `navigator.sendBeacon` can carry it.

type Props = Record<string, unknown>

const LIBRARY = { name: 'creators-website-beacon', version: '1.0.0' }

export type SegmentCall = { type: 'track'; event: string } | { type: 'page'; name: string }

export type SegmentHttpInput = {
  writeKey: string
  call: SegmentCall
  properties: Props
  anonymousId: string
  userId?: string
  /** `host/basePath` without protocol, as sites' `SEGMENT_API_HOST`. */
  apiHost: string
  app: { name: string; version: string }
  search: string
}

// Chromium-only and missing from the TS DOM lib; analytics-next attaches this low-entropy slice too.
type UserAgentData = { brands?: Array<{ brand: string; version: string }>; mobile?: boolean; platform?: string }

function userAgentData(): UserAgentData | undefined {
  const data = (navigator as Navigator & { userAgentData?: UserAgentData }).userAgentData
  return data ? { brands: data.brands, mobile: data.mobile, platform: data.platform } : undefined
}

function timezone(): string | undefined {
  try {
    return Intl.DateTimeFormat().resolvedOptions().timeZone || undefined
  } catch {
    return undefined
  }
}

/** `context.campaign` as analytics.js derives it: every `utm_*` param, unprefixed, `campaign` as `name`. */
export function campaignContext(search: string): Props | undefined {
  const campaign: Props = {}
  for (const [key, value] of new URLSearchParams(search)) {
    if (!key.startsWith('utm_') || key.length <= 4) continue
    const name = key.slice(4)
    campaign[name === 'campaign' ? 'name' : name] = value
  }
  return Object.keys(campaign).length ? campaign : undefined
}

function messageId(): string {
  const id =
    typeof crypto !== 'undefined' && 'randomUUID' in crypto
      ? crypto.randomUUID()
      : `${Date.now()}-${Math.random().toString(36).slice(2)}`
  return `${LIBRARY.name}-${id}`
}

/** The HTTP API body for one call, in the shape analytics.js sends. */
export function segmentBody(input: SegmentHttpInput): Props {
  const page = {
    path: window.location.pathname,
    referrer: document.referrer,
    search: input.search,
    title: document.title,
    url: window.location.href
  }
  const timestamp = new Date().toISOString()
  const agentData = userAgentData()
  const zone = timezone()
  const campaign = campaignContext(input.search)
  return {
    writeKey: input.writeKey,
    ...(input.call.type === 'track' ? { event: input.call.event } : { name: input.call.name }),
    ...(input.userId ? { userId: input.userId } : {}),
    anonymousId: input.anonymousId,
    integrations: {},
    // Segment's page spec repeats the page fields as properties.
    properties: input.call.type === 'page' ? { ...page, ...input.properties } : input.properties,
    messageId: messageId(),
    timestamp,
    sentAt: timestamp,
    context: {
      // Without it Segment treats a call with a custom `library` as server-side and drops the client IP.
      direct: true,
      page,
      ...(campaign ? { campaign } : {}),
      userAgent: navigator.userAgent,
      ...(agentData ? { userAgentData: agentData } : {}),
      locale: navigator.language,
      ...(zone ? { timezone: zone } : {}),
      library: LIBRARY,
      app: input.app
    }
  }
}

/** Sends one call over an unload-safe transport. Best effort: never throws. */
export function postToSegment(input: SegmentHttpInput): void {
  const url = `https://${input.apiHost}/${input.call.type}`
  const body = JSON.stringify(segmentBody(input))
  // text/plain keeps the request CORS-simple; a JSON content type would need a preflight, unsafe at unload.
  if (typeof navigator.sendBeacon === 'function') {
    try {
      if (navigator.sendBeacon(url, new Blob([body], { type: 'text/plain' }))) return
    } catch {
      // Falls through to fetch.
    }
  }
  void fetch(url, {
    method: 'POST',
    headers: { 'Content-Type': 'text/plain' },
    body,
    keepalive: true,
    mode: 'cors',
    credentials: 'omit'
  }).catch(() => undefined)
}
