// Centralised error reporting. Two jobs:
//  1. ALWAYS surface failures in the console — a catch block that only shows a toast hides the real error.
//  2. Forward them to Sentry when it's configured, through a seam (`setErrorForwarder`) so `captureError`
//     stays independent of Sentry and testable without it.
//
// Convention: pass a `flow` in the context (e.g. { flow: 'publish-collection' }) so console and Sentry
// group failures by user action. Never put secrets in the context — `beforeSend` scrubs defensively anyway.
import * as Sentry from '@sentry/react'
import { config } from '~/config'
import { currentAddress } from '~/lib/currentAddress'
import { afterLoadIdle } from '~/lib/idle'
import { errorChain } from '~/lib/walletErrors'

export type ErrorContext = Record<string, unknown>

let forward: ((error: unknown, context: ErrorContext) => void) | null = null

/** Wire a downstream sink (Sentry) for captured errors. Passing null disables forwarding. */
export function setErrorForwarder(fn: ((error: unknown, context: ErrorContext) => void) | null): void {
  forward = fn
}

/**
 * A thrown value Sentry can put a NAME on, derived from one it cannot.
 *
 * Wallet and JSON-RPC failures arrive as plain objects — `{ code, message, data }` — not Errors, and
 * Sentry titles those after the frame that captured them, which is its own minified `captureException`.
 * The original stack is transplanted when there is one: a fresh Error's stack points here, so without it
 * every wallet failure in the app groups into a single meaningless issue.
 */
export function toReportable(value: unknown): unknown {
  if (value instanceof Error || !value || typeof value !== 'object') return value
  const raw = value as { message?: unknown; code?: unknown; stack?: unknown }
  if (typeof raw.message !== 'string' || raw.message === '') return value

  const code = typeof raw.code === 'string' || typeof raw.code === 'number' ? ` (code ${raw.code})` : ''
  const error = new Error(`${raw.message}${code}`) as Error & { cause?: unknown }
  error.cause = value
  if (typeof raw.stack === 'string' && raw.stack !== '') error.stack = raw.stack
  return error
}

/**
 * The machine-readable facts inside a wallet/RPC failure, under names of our own.
 *
 * Provider messages get scrubbed (by Sentry's server-side filters, or by ours) and then the event says
 * nothing at all. These two are numbers nothing scrubs, and they are low-cardinality, so `tagsFrom`
 * promotes them to tags — "how many publishes died on an RPC 401 this week" becomes answerable.
 */
export function rpcFactsFrom(value: unknown): ErrorContext {
  const facts: ErrorContext = {}
  // Innermost first: the wallet's or node's own code, not a wrapper's generic one (viem's UnknownRpcError is -1).
  for (const link of errorChain(value).reverse()) {
    if (typeof link !== 'object' || link === null) continue
    const raw = link as { code?: unknown; data?: unknown }
    if (facts.rpc_code === undefined && (typeof raw.code === 'number' || typeof raw.code === 'string')) {
      facts.rpc_code = raw.code
    }
    if (facts.http_status === undefined && raw.data && typeof raw.data === 'object') {
      const status = (raw.data as { httpStatus?: unknown }).httpStatus
      if (typeof status === 'number' || typeof status === 'string') facts.http_status = status
    }
  }
  return facts
}

/** Log an error to the console (always) and forward it to the reporter (if wired). Never throws. */
export function captureError(error: unknown, context: ErrorContext = {}): void {
  const label = typeof context.flow === 'string' ? `error in ${context.flow}` : 'error'
  // The caller's own context wins: these are a fallback read off the thrown value, never an override.
  const enriched = { ...rpcFactsFrom(error), ...context }

  console.error(`[creators-website] ${label}`, error, enriched)
  if (forward) {
    try {
      forward(toReportable(error), enriched)
    } catch (forwardError) {
      // Reporting must never throw back into the caller's catch block — but a forwarder that is
      // broken would otherwise lose every error in silence.
      console.warn('[creators-website] error forwarder failed', forwardError)
    }
  }
}

// Sentry wiring — PII/secret scrubbing. A wallet address is public on-chain (fine to attach); AuthChain
// signatures, ephemeral identity keys and bearer tokens must never leave the device.
const SIGNATURE_RE = /0x[a-fA-F0-9]{130}\b/g
// Nothing mints JWTs here today, but a bearer token from any future API would sail through the
// hex-shaped patterns below. Matches the trailing segments too: the payload is the readable part.
const JWT_RE = /eyJ[A-Za-z0-9_-]{8,}(?:\.[A-Za-z0-9_-]+)*/g
const HEX32_RE = /0x[a-fA-F0-9]{64}\b/g
const SECRET_RE = /[A-Za-z0-9-]*secret[A-Za-z0-9-]*/gi
const SENSITIVE_KEY =
  /(signature|private|identity|authchain|auth_chain|ephemeral|token|secret|password|cookie|authorization)/i

/** Redact secret-shaped substrings from free text (messages, exception values, urls). */
export function redact(input: string): string {
  return input
    .replace(SIGNATURE_RE, '<signature>')
    .replace(JWT_RE, '<jwt>')
    .replace(SECRET_RE, '<secret>')
    .replace(HEX32_RE, '<hex32>')
}

/** Redact the free text in a bag of context values and drop the sensitive keys outright, in place. */
function cleanBag(bag?: Record<string, unknown>): void {
  if (!bag) return
  for (const key of Object.keys(bag)) {
    if (SENSITIVE_KEY.test(key)) {
      delete bag[key]
      continue
    }
    if (typeof bag[key] === 'string') bag[key] = redact(bag[key])
  }
}

/**
 * Scrub an outgoing Sentry event — an error OR a transaction: redact free text, drop sensitive keys,
 * strip cookies/headers.
 *
 * Breadcrumb and span DATA matter as much as messages: a fetch/xhr breadcrumb carries the request url
 * under `data.url` and a span carries its own under `data`, neither of which is a message, so a token
 * in a query string would otherwise sail straight through.
 */
export function scrubEvent<T extends Sentry.Event>(event: T): T {
  if (event.message) event.message = redact(event.message)
  for (const exception of event.exception?.values ?? []) {
    if (exception.value) exception.value = redact(exception.value)
  }
  for (const breadcrumb of event.breadcrumbs ?? []) {
    if (breadcrumb.message) breadcrumb.message = redact(breadcrumb.message)
    cleanBag(breadcrumb.data)
  }
  for (const span of event.spans ?? []) {
    if (span.description) span.description = redact(span.description)
    cleanBag(span.data)
  }
  if (event.transaction) event.transaction = redact(event.transaction)
  if (event.request) {
    if (event.request.url) event.request.url = redact(event.request.url)
    delete event.request.cookies
    delete event.request.headers
  }
  cleanBag(event.tags)
  cleanBag(event.extra)
  return event
}

/**
 * The low-cardinality context fields worth INDEXING, promoted from `extra` to Sentry tags — Sentry does
 * not index `extra`, so anything left there can't be searched, filtered or charted. Only these four are
 * promoted: the rest of the context carries ids and addresses that would blow past tag cardinality limits.
 */
export function tagsFrom(context: ErrorContext): Record<string, string> {
  const tags: Record<string, string> = {}
  for (const key of ['flow', 'step'] as const) {
    const value = context[key]
    if (typeof value === 'string' && value !== '') tags[key] = value
  }
  // Read off the thrown value (see rpcFactsFrom), so a number is the normal case — `-32603`, `401`.
  for (const key of ['rpc_code', 'http_status'] as const) {
    const value = context[key]
    if (typeof value === 'number') tags[key] = String(value)
    else if (typeof value === 'string' && value !== '') tags[key] = value
  }
  return tags
}

/** The sink `initSentry` wires into the seam. Exported so the tag promotion is reachable from a test. */
export function sentryForwarder(error: unknown, context: ErrorContext): void {
  Sentry.captureException(error, { tags: tagsFrom(context), extra: context })
}

let initialized = false

// Replays draw on the quota of the shared project, so the rates match the legacy builder's: 1% of sessions
// recorded, 1% buffered in case of an error. Only that 2% downloads the replay bundle at all, so inside it the
// SDK's own rates are scaled back up to land on the same overall shares.
const REPLAY_SESSION_RATE = 0.01
const REPLAY_ON_ERROR_RATE = 0.01
const REPLAY_LOAD_RATE = REPLAY_SESSION_RATE + REPLAY_ON_ERROR_RATE

/** Loads the replay recorder, a separate chunk, once the page is idle, so it never weighs on the first load. */
function loadReplay(): void {
  afterLoadIdle(() => {
    import('~/lib/sentryReplay')
      .then(({ replayIntegration }) => Sentry.addIntegration(replayIntegration()))
      // A failed chunk load only costs the recording; it is not an app failure worth reporting.
      .catch(() => undefined)
  })
}

/** Local hosts share dev.json's config, so without this guard local runs would report to the dev DSN. */
export function isLocalhost(hostname: string = typeof location !== 'undefined' ? location.hostname : ''): boolean {
  return (
    hostname === 'localhost' ||
    hostname === '127.0.0.1' ||
    hostname === '0.0.0.0' ||
    hostname === '[::1]' ||
    hostname.endsWith('.local')
  )
}

/**
 * Initialise Sentry. No-ops on localhost and without a DSN, so local and test never send. Safe to call
 * once at startup; afterwards every `captureError` is forwarded (scrubbed).
 */
export function initSentry(): void {
  if (initialized) return
  if (isLocalhost()) {
    if (import.meta.env.DEV) console.debug('[monitoring] localhost → Sentry disabled')
    return
  }
  const dsn = config.get('SENTRY_DSN', '')
  if (!dsn) return
  initialized = true
  Sentry.init({
    dsn,
    environment: config.get('ENVIRONMENT'),
    // Shares the legacy builder's Sentry project; the release prefix is what separates the two apps.
    // Baked in by vite.config, which uploads the source maps under this exact string — a release name
    // that doesn't match byte for byte means no map is ever applied and every stack stays minified.
    release: __SENTRY_RELEASE__,
    integrations: [Sentry.browserTracingIntegration()],
    tracesSampleRate: 0.01,
    replaysSessionSampleRate: REPLAY_SESSION_RATE / REPLAY_LOAD_RATE,
    replaysOnErrorSampleRate: 1,
    // Every category defaults to on.
    dataCollection: {
      userInfo: false,
      cookies: false,
      httpHeaders: false,
      httpBodies: [],
      urlQueryParams: false,
      stackFrameVariables: false
    },
    // Expected user actions, not bugs.
    ignoreErrors: [/user rejected/i, /user denied/i, 'ResizeObserver loop limit exceeded'],
    beforeSend: scrubEvent,
    // Performance events don't go through `beforeSend`, and they carry urls of their own.
    beforeSendTransaction: scrubEvent
  })
  setMonitoringUser(safeAddress())
  setErrorForwarder(sentryForwarder)
  // Replay defaults mask every text node and block all media, so a recording never carries what
  // the creator typed or uploaded — only the layout and the clicks that led to the failure.
  if (Math.random() < REPLAY_LOAD_RATE) loadReplay()
}

/** Attach/detach the wallet as the Sentry user (the address is public). Call on sign-in / disconnect. */
export function setMonitoringUser(address: string | null): void {
  if (!initialized) return
  Sentry.setUser(address ? { id: address.toLowerCase() } : null)
}

/** Tags every later error carries (the preview renderer, say). Sends nothing on its own. */
export function setMonitoringTags(tags: Record<string, string>): void {
  if (!initialized) return
  Sentry.setTags(tags)
}

function safeAddress(): string | null {
  return currentAddress() ?? null
}
