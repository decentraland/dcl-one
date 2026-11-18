import { type HTTPRequest } from 'puppeteer'
import { type BuilderServer } from './builderServer'
import { type Market } from './market'
import { previewPageHtml, type PreviewOptions } from './preview'
import { type Chain } from './rpc'

export const CORS = {
  'access-control-allow-origin': '*',
  'access-control-allow-headers': '*',
  'access-control-allow-methods': 'GET,POST,PUT,PATCH,DELETE,OPTIONS'
}

// 1×1 transparent PNG.
const PNG = Buffer.from(
  'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==',
  'base64'
)

export type Flags = Record<string, boolean>
/** Forced responses keyed `[METHOD ]path-suffix`, e.g. `{ 'POST /collections/<id>/tos': { status: 500 } }`. */
export type ForcedErrors = Record<string, { status: number; body?: unknown }>

export type Credits = { balanceCents: number; credits: number }

/** The Blender add-on's local bridge: the state it reports and the model it serves. Mutate `state` to export again. */
export type Bridge = { port: number; state: Record<string, unknown>; model: Buffer }

export type RouteContext = {
  appOrigin: string
  builder: BuilderServer
  chain: Chain
  market: Market
  credits: Credits
  bridge?: Bridge
  /** Controller answers and held loads the fake preview reported, in order. */
  previewAnswers: Array<{ method: string; result: string }>
  flags: Flags
  errors: ForcedErrors
  preview: PreviewOptions
  /** Builder storage files by content hash. */
  storage: Record<string, Buffer>
  data: ExternalData
}

export type ExternalData = {
  hotScenes: unknown[]
  blogPosts: unknown[]
  /** What the Catalyst answers to `POST /content/entities/active`. */
  catalystEntities: unknown[]
}

export const DEFAULT_EXTERNAL_DATA: ExternalData = {
  hotScenes: [
    {
      id: 'scene-1',
      name: 'Neon Rooftop',
      baseCoords: [10, 20],
      usersTotalCount: 12,
      parcels: [[10, 20]],
      thumbnail: 'https://e2e.invalid/scene-1.png'
    },
    {
      id: 'scene-2',
      name: 'Pixel Gallery',
      baseCoords: [-5, 33],
      usersTotalCount: 4,
      parcels: [[-5, 33]],
      thumbnail: 'https://e2e.invalid/scene-2.png'
    }
  ],
  blogPosts: [
    {
      sys: { id: 'post-1' },
      fields: { id: 'creator-spotlight', title: 'Creator spotlight', publishedDate: '2026-10-30T00:00:00.000Z' }
    }
  ],
  catalystEntities: []
}

/**
 * Stands in for analytics.js: replays the snippet's queue and forwards every later call to node (`__e2eAnalytics`),
 * so tracked events survive reloads and navigations.
 */
const ANALYTICS_JS = `(function () {
  var queue = window.analytics || [];
  function record(method, args) {
    var plain = [].slice.call(args).filter(function (a) { return typeof a !== 'function'; });
    try { window.__e2eAnalytics && window.__e2eAnalytics(method, JSON.parse(JSON.stringify(plain))); } catch (e) {}
  }
  var analytics = { initialize: true, invoked: true };
  ['track', 'page', 'identify', 'group', 'alias'].forEach(function (method) {
    analytics[method] = function () { record(method, arguments); return Promise.resolve(); };
  });
  ['reset', 'on', 'once', 'off', 'debug', 'setAnonymousId', 'addSourceMiddleware', 'addDestinationMiddleware'].forEach(
    function (method) { analytics[method] = function () {}; }
  );
  analytics.ready = function (cb) { if (typeof cb === 'function') cb(); };
  analytics.user = function () { return { anonymousId: function () { return 'e2e-anonymous-id'; }, id: function () { return null; } }; };
  window.analytics = analytics;
  for (var i = 0; i < queue.length; i++) {
    var call = queue[i];
    if (Array.isArray(call) && typeof analytics[call[0]] === 'function') analytics[call[0]].apply(analytics, call.slice(1));
  }
})();`

const json = (req: HTTPRequest, body: unknown, status = 200) =>
  req.respond({ status, headers: { 'content-type': 'application/json', ...CORS }, body: JSON.stringify(body) })

const empty = (req: HTTPRequest, contentType = 'text/plain') =>
  req.respond({ status: 200, headers: { 'content-type': contentType, ...CORS }, body: '' })

function parseBody(req: HTTPRequest): unknown {
  const raw = req.postData()
  if (!raw) return undefined
  try {
    return JSON.parse(raw)
  } catch {
    return undefined
  }
}

/** Answers every request the page makes: the app's own assets pass through, everything else is mocked. */
export function route(req: HTTPRequest, ctx: RouteContext): Promise<void> {
  const url = new URL(req.url())
  const { hostname, pathname } = url
  const method = req.method()

  if (ctx.bridge && url.hostname === 'localhost' && url.port === String(ctx.bridge.port)) {
    if (method === 'OPTIONS') return req.respond({ status: 204, headers: CORS })
    if (pathname === '/state') return json(req, ctx.bridge.state)
    if (pathname === '/model.glb') {
      return req.respond({
        status: 200,
        headers: { 'content-type': 'model/gltf-binary', ...CORS },
        body: ctx.bridge.model
      })
    }
    return json(req, {}, 404)
  }
  // Vite proxies /auth to the real auth site, so a sign-in click must stop here.
  // The same match as the dev proxy in vite.config.ts, which leaves `/authorizations` alone.
  if (url.origin === ctx.appOrigin && /^\/auth(\/|$)/.test(pathname)) {
    return req.respond({
      status: 200,
      headers: { 'content-type': 'text/html' },
      body: '<!doctype html><title>auth</title>'
    })
  }
  if (url.origin === ctx.appOrigin || url.protocol === 'data:' || url.protocol === 'blob:') return req.continue()
  // A failed preflight would surface as "Failed to fetch" and hide the forced error the spec asked for.
  if (method === 'OPTIONS') return req.respond({ status: 204, headers: CORS })

  const forced = Object.entries(ctx.errors).find(([key]) => {
    const [forcedMethod, fragment] = key.includes(' ') ? key.split(' ', 2) : [undefined, key]
    return (!forcedMethod || forcedMethod === method) && pathname.endsWith(fragment)
  })
  if (forced) return json(req, forced[1].body ?? { ok: false, error: 'forced e2e error' }, forced[1].status)

  if (hostname.startsWith('feature-flags.')) {
    return json(req, { flags: ctx.flags })
  }

  if (hostname.startsWith('builder-api.')) {
    const path = pathname.replace(/^\/v1/, '')
    const storage = path.match(/^\/storage\/contents\/(.+)$/)
    if (storage) {
      const file = ctx.storage[storage[1]]
      return file
        ? req.respond({ status: 200, headers: { 'content-type': 'application/octet-stream', ...CORS }, body: file })
        : req.respond({ status: 404, headers: CORS, body: '' })
    }
    // The bytes were captured in the page before the request left (see UPLOAD_CAPTURE_SCRIPT).
    if (method === 'POST' && /^\/items\/[^/]+\/(files|videos)$/.test(path)) return json(req, { ok: true })
    const answer = ctx.builder.handle(method, path, url.searchParams, parseBody(req))
    if (answer) return json(req, answer.body, answer.status)
    process.stderr.write(`[e2e] unmocked builder-server route: ${method} ${path}\n`)
    return json(req, { ok: false, error: 'unmocked e2e route' }, 501)
  }

  if (hostname.startsWith('peer.')) {
    if (pathname.startsWith('/lambdas/profiles/')) return json(req, { avatars: [] })
    if (pathname.startsWith('/lambdas/collections/wearables')) return json(req, { wearables: [] })
    if (pathname.startsWith('/content/contents/')) return req.respond({ status: 200, headers: CORS, body: PNG })
    if (pathname === '/content/entities/active') return json(req, ctx.data.catalystEntities)
    return json(req, [])
  }

  if (hostname.startsWith('realm-provider')) return json(req, ctx.data.hotScenes)

  if (hostname === 'cms-api.decentraland.org') {
    if (pathname.endsWith('/blog/posts'))
      return json(req, { items: ctx.data.blogPosts, total: ctx.data.blogPosts.length })
    if (pathname.endsWith('/blog/categories')) return json(req, { items: [] })
    // No campaign and no cover assets unless a spec serves them.
    return json(req, {}, 404)
  }

  if (hostname === 'api.github.com') return json(req, { assets: [] })

  if (hostname.startsWith('credits.')) {
    if (/^\/users\/[^/]+\/credits$/.test(pathname)) return json(req, { usd: ctx.credits })
    if (method === 'POST' && pathname === '/credits/authorize-publication') {
      return json(req, {
        credit: {
          id: '0x' + '55'.repeat(32),
          amount: '1000000000000000000000',
          expiresAt: Math.floor(Date.now() / 1000) + 900,
          signature: '0x' + 'ab'.repeat(65)
        },
        externalCallSignature: '0x' + 'cd'.repeat(65)
      })
    }
  }

  if (hostname.startsWith('marketplace-api.')) {
    const answer = ctx.market.handle(method, pathname, url.searchParams, parseBody(req))
    if (answer) return json(req, answer.body, answer.status)
  }

  if (hostname.startsWith('transactions-api.') && method === 'POST' && pathname.endsWith('/transactions')) {
    return json(req, ctx.chain.relay(req.postData() ?? '{}'))
  }

  // The read-only provider: no chain is ever reached.
  if (hostname.startsWith('rpc-') || hostname.includes('rpc.') || pathname.includes('/rpc')) {
    return json(req, ctx.chain.handleRpc(req.postData() ?? '{}'))
  }

  // Monitoring and analytics sinks.
  if (hostname.endsWith('sentry.io') || hostname.startsWith('api.e.')) return empty(req)
  if (hostname.startsWith('evs.e.') || hostname === 'cdn.segment.com') {
    return req.respond({
      status: 200,
      headers: { 'content-type': 'application/javascript', ...CORS },
      body: ANALYTICS_JS
    })
  }
  if (hostname.includes('intercom')) return empty(req, 'application/javascript')

  // The preview iframe can't render headless; this page speaks its protocol instead.
  if (hostname.startsWith('wearable-preview') && pathname === '/__e2e/answer') {
    ctx.previewAnswers.push({
      method: url.searchParams.get('method') ?? '',
      result: url.searchParams.get('result') ?? ''
    })
    return req.respond({ status: 204, headers: CORS })
  }
  if (hostname.startsWith('wearable-preview')) {
    return req.respond({
      status: 200,
      headers: { 'content-type': 'text/html', ...CORS },
      body: previewPageHtml(ctx.preview)
    })
  }

  const type = req.resourceType()
  if (type === 'image')
    return req.respond({ status: 200, headers: { 'content-type': 'image/png', ...CORS }, body: PNG })
  if (type === 'font' || type === 'media' || type === 'stylesheet') return empty(req)
  // Third-party embeds (the footer's newsletter form) and top-level navigations away from the app both need
  // HTML: anything else makes a navigation a download.
  if (type === 'document') {
    return req.respond({
      status: 200,
      headers: { 'content-type': 'text/html', ...CORS },
      body: '<!doctype html><title>external</title>'
    })
  }

  process.stderr.write(`[e2e] unmocked request: ${method} ${req.url()}\n`)
  return json(req, {}, 404)
}
