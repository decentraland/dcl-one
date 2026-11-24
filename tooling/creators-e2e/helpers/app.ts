import { chromium, type Browser, type Page } from 'playwright-core'
import { createBuilderServer, type BuilderSeed, type BuilderServer } from './builderServer'
import { APP_PATH, BASE, chromiumPath, SEGMENT_HOST } from './env'
import { fixtureStorage } from './files'
import { createMarket, type Market, type MarketSeed } from './market'
import {
  DEFAULT_EXTERNAL_DATA,
  requestOf,
  route,
  type Bridge,
  type Credits,
  type ExternalData,
  type Flags,
  type ForcedErrors,
  type RouteContext
} from './route'
import { type PreviewOptions } from './preview'
import { createChain, type ChainOptions, type SentTransaction } from './rpc'
import { OFFLINE_WEBSOCKET_SCRIPT, POLYGON_AMOY, sessionInitScript, testSession } from './session'

export const VIEWPORTS = {
  desktop: { width: 1280, height: 900 },
  mobile: { width: 390, height: 844 }
} as const

export type TrackedEvent = { event: string; props: Record<string, unknown> }

export type App = {
  browser: Browser
  page: Page
  builder: BuilderServer
  market: Market
  /** Builder storage by content hash: fixture files plus everything the app uploaded. */
  storage: Record<string, Buffer>
  /** Every write the app sent to a mocked service (`METHOD /path`), in order; chain reads and analytics excluded. */
  writes: string[]
  /** Every transaction the wallet sent, decoded where the function is known. */
  transactions: SentTransaction[]
  /** URLs the page tried to open in a new tab or window. */
  openedUrls: string[]
  /** What the fake preview reported, in order: its answers to controller questions (`emote.hasSound`) and held `load`s. */
  previewAnswers: Array<{ method: string; result: string }>
  /** Messages of the browser dialogs (confirm, beforeunload) the page raised; each one is accepted. */
  dialogs: string[]
  /** The page's console errors and uncaught exceptions, across reloads, for failure reports. */
  consoleErrors: string[]
  /** The telemetry events the page sent so far, across reloads. */
  events: () => Promise<TrackedEvent[]>
  /** Resolves with the first tracked `event` whose props match, polling until `timeout`. */
  waitForEvent: (event: string, match?: Record<string, unknown>, timeout?: number) => Promise<TrackedEvent>
  close: () => Promise<void>
}

export type LaunchOptions = {
  path?: string
  signedOut?: boolean
  builder?: BuilderSeed
  market?: MarketSeed
  chain?: ChainOptions
  credits?: Partial<Credits>
  /** A fake Blender bridge on a localhost port, for `/live-preview?bridge=<port>`. */
  bridge?: Bridge
  /** Flag values served by the flag service, keyed `${application}-${flag}`. Everything else is off. */
  flags?: Flags
  errors?: ForcedErrors
  /** How the fake wearable-preview iframe behaves. */
  preview?: PreviewOptions
  /** Builder storage files by content hash, on top of the fixture files. */
  storage?: Record<string, Buffer>
  data?: Partial<ExternalData>
  chainId?: number
  viewport?: keyof typeof VIEWPORTS
  waitUntil?: 'load' | 'domcontentloaded' | 'networkidle'
}

/**
 * Request interception never sees the bytes of a Blob in a multipart body, so item uploads are read in the page,
 * before they leave, and handed to node (`__e2eUpload`) to store by content hash.
 */
const UPLOAD_CAPTURE_SCRIPT = `(() => {
  const realFetch = window.fetch;
  const toBase64 = async blob => {
    const bytes = new Uint8Array(await blob.arrayBuffer());
    let binary = '';
    for (let i = 0; i < bytes.length; i += 0x8000) binary += String.fromCharCode.apply(null, bytes.subarray(i, i + 0x8000));
    return btoa(binary);
  };
  window.fetch = async function (input, init) {
    const url = typeof input === 'string' ? input : input instanceof URL ? input.href : input.url;
    const upload = /\\/items\\/([^/?]+)\\/(files|videos)$/.exec(new URL(url, location.href).pathname);
    if (upload && init && init.body instanceof FormData) {
      for (const [name, value] of init.body.entries()) {
        if (value instanceof Blob) await window.__e2eUpload(upload[1], name, await toBase64(value));
      }
    }
    return realFetch.call(this, input, init);
  };
})()`

// The page has no tab to open, and a real one would escape the request mocks.
const WINDOW_OPEN_SCRIPT = `window.open = function (url) { window.__e2eOpen && window.__e2eOpen(String(url)); return null; };`

// A beacon's body is a Blob the request interception cannot read; refused, the page posts the same text with fetch.
const NO_BEACON_SCRIPT = `navigator.sendBeacon = () => false;`

/** A headless page on the e2e dev server with the wallet and every off-origin request mocked. */
export async function launchApp(opts: LaunchOptions = {}): Promise<App> {
  const browser = await chromium.launch({
    executablePath: chromiumPath(),
    headless: true,
    args: [
      // CI machines report no hover and a coarse pointer, which would run every hover rule's touch branch.
      '--blink-settings=primaryHoverType=2,availableHoverTypes=2,primaryPointerType=4,availablePointerTypes=4',
      // Software WebGL, for the posed thumbnail render on machines without a GPU.
      '--enable-unsafe-swiftshader',
      '--use-angle=swiftshader'
    ]
  })
  try {
    return await openPage(browser, opts)
  } catch (error) {
    // The spec never gets an App to close, so a failed launch would leave Chrome running.
    await browser.close()
    throw error
  }
}

async function openPage(browser: Browser, opts: LaunchOptions): Promise<App> {
  const chain = createChain(opts.chain)
  const builder = createBuilderServer(opts.builder)
  chain.onTransaction(tx => builder.applyTransaction(tx))
  const market = createMarket(opts.market)
  chain.onTransaction(tx => market.applyTransaction(tx))

  const mobile = opts.viewport === 'mobile'
  const context = await browser.newContext({
    viewport: VIEWPORTS[opts.viewport ?? 'desktop'],
    isMobile: mobile,
    hasTouch: mobile,
    permissions: ['clipboard-read', 'clipboard-write']
  })
  const page = await context.newPage()
  // Leaving a page with unsaved work asks first; specs that reload mean to, and can assert what was asked.
  const dialogs: string[] = []
  page.on('dialog', dialog => {
    dialogs.push(dialog.type() === 'beforeunload' ? 'beforeunload' : dialog.message())
    void dialog.accept().catch(() => {})
  })

  const storage = { ...fixtureStorage(), ...opts.storage }
  const tracked: TrackedEvent[] = []
  const openedUrls: string[] = []
  await page.exposeFunction('__e2eWallet', (method: string, params: unknown[]) => chain.walletRequest(method, params))
  await page.exposeFunction('__e2eUpload', (itemId: string, name: string, base64: string) => {
    // Files go under their content hash; the video goes under `video.mp4`, whose hash the item already names.
    const hash = builder.items.get(itemId)?.contents[name] ?? name
    storage[hash] = Buffer.from(base64, 'base64')
  })
  await page.exposeFunction('__e2eOpen', (url: string) => void openedUrls.push(url))
  await page.addInitScript(OFFLINE_WEBSOCKET_SCRIPT)
  await page.addInitScript(NO_BEACON_SCRIPT)
  await page.addInitScript(WINDOW_OPEN_SCRIPT)
  await page.addInitScript(UPLOAD_CAPTURE_SCRIPT)
  if (!opts.signedOut) {
    await page.addInitScript(sessionInitScript(await testSession(), opts.chainId ?? POLYGON_AMOY))
  }

  const writes: string[] = []
  const ctx: RouteContext = {
    appOrigin: new URL(BASE).origin,
    builder,
    chain,
    market,
    credits: { balanceCents: 0, credits: 0, ...opts.credits },
    bridge: opts.bridge,
    previewAnswers: [],
    flags: opts.flags ?? {},
    errors: opts.errors ?? {},
    preview: opts.preview ?? {},
    storage,
    data: { ...DEFAULT_EXTERNAL_DATA, ...opts.data }
  }
  const consoleErrors: string[] = []
  page.on('console', message => {
    if (message.type() === 'error') consoleErrors.push(message.text().slice(0, 300))
  })
  page.on('pageerror', error => consoleErrors.push(`uncaught: ${String(error).slice(0, 300)}`))
  await context.route('**/*', intercepted => {
    const req = requestOf(intercepted)
    const url = new URL(req.url())
    const body = req.postData() ?? ''
    if (url.hostname === SEGMENT_HOST) {
      try {
        const call = JSON.parse(body || '{}') as { event?: string; properties?: Record<string, unknown> }
        if (call.event) tracked.push({ event: call.event, props: call.properties ?? {} })
      } catch (error) {
        process.stderr.write(`[e2e] unreadable telemetry event: ${String(error)}\n`)
      }
      return req.respond({ status: 200, headers: { 'access-control-allow-origin': '*' }, body: '' })
    }
    if (
      !['GET', 'HEAD', 'OPTIONS'].includes(req.method()) &&
      url.origin !== ctx.appOrigin &&
      !body.includes('"jsonrpc"')
    ) {
      writes.push(`${req.method()} ${url.pathname}`)
    }
    // A throw inside the router would otherwise leave the request unanswered until the spec times out.
    return Promise.resolve()
      .then(() => route(req, ctx))
      .catch(async (error: unknown) => {
        process.stderr.write(`[e2e] router failed on ${req.method()} ${req.url()}: ${String(error)}\n`)
        await req.respond({ status: 500, body: String(error) }).catch(() => {})
      })
  })

  await page.goto(`${BASE}${APP_PATH}${opts.path ?? '/collections'}`, {
    // The editor keeps the preview and the chain busy, so the network seldom goes quiet: specs wait for what they need.
    waitUntil: opts.waitUntil ?? 'load',
    timeout: 45_000
  })

  const events = () => Promise.resolve([...tracked])

  const waitForEvent = async (event: string, match: Record<string, unknown> = {}, timeout = 10_000) => {
    const deadline = Date.now() + timeout
    for (;;) {
      const found = tracked.find(
        e => e.event === event && Object.entries(match).every(([key, value]) => e.props[key] === value)
      )
      if (found) return found
      if (Date.now() > deadline) {
        const seen = tracked.filter(e => e.event === event).map(e => JSON.stringify(e.props))
        throw new Error(`no "${event}" event matching ${JSON.stringify(match)}; seen: ${seen.join(' | ') || 'none'}`)
      }
      await new Promise(r => setTimeout(r, 200))
    }
  }

  return {
    browser,
    page,
    builder,
    market,
    storage: ctx.storage,
    writes,
    transactions: chain.transactions,
    openedUrls,
    previewAnswers: ctx.previewAnswers,
    dialogs,
    consoleErrors,
    events,
    waitForEvent,
    close: () => browser.close()
  }
}
