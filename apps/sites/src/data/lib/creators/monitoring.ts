// Centralised error reporting. Two jobs:
//  1. ALWAYS surface failures in the console: a catch block that only shows a toast hides the real error.
//  2. Report them to Sites' client-error endpoint, through a seam (`setErrorForwarder`) so `captureError`
//     can be tested without it.
//
// Convention: pass a `flow` in the context (e.g. { flow: 'publish-collection' }) so failures group by
// user action. Never put secrets in the context; the report is redacted defensively anyway.
import { errorChain } from './walletErrors'

export type ErrorContext = Record<string, unknown>

let forward: ((error: unknown, context: ErrorContext) => void) | null = clientErrorForwarder

/** Replace the sink for captured errors. Passing null disables forwarding. */
export function setErrorForwarder(fn: ((error: unknown, context: ErrorContext) => void) | null): void {
  forward = fn
}

/**
 * An Error for a thrown value that is not one. Wallet and JSON-RPC failures arrive as plain objects
 * (`{ code, message, data }`); the original stack is kept when there is one, since a fresh Error's points here.
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
 * Provider messages get redacted and then the report says nothing at all; these two are numbers nothing
 * redacts, and they are low-cardinality.
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

  console.error(`[creators] ${label}`, error, enriched)
  if (forward) {
    try {
      forward(toReportable(error), enriched)
    } catch (forwardError) {
      // Reporting must never throw back into the caller's catch block — but a forwarder that is
      // broken would otherwise lose every error in silence.
      console.warn('[creators] error forwarder failed', forwardError)
    }
  }
}

// Secret scrubbing. A wallet address is public on-chain (fine to attach); AuthChain
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

const CLIENT_ERROR_ENDPOINT = '/internal/client-error'
const REPORT_BUDGET = 10
const DEDUPE_MS = 60_000
// The endpoint refuses bodies over 16 KB and clips to these lengths itself (the message's for the url and agent).
const MAX_MESSAGE = 2000
const MAX_STACK = 8000
const MAX_BODY_BYTES = 16 * 1024

const lastReported = new Map<string, number>()
let reported = 0

/** The context as one short line of its plain values, without the sensitive keys. */
function describeContext(context: ErrorContext): string {
  const facts = Object.entries(context)
    .filter(
      ([key, value]) =>
        key !== 'flow' && !SENSITIVE_KEY.test(key) && ['string', 'number', 'boolean'].includes(typeof value)
    )
    .map(([key, value]) => `${key}=${redact(String(value))}`)
  return facts.length > 0 ? ` [${facts.join(' ')}]` : ''
}

/** Posts a captured error to Sites' client-error endpoint: at most ten a page, the same error once a minute. */
export function clientErrorForwarder(error: unknown, context: ErrorContext): void {
  if (typeof window === 'undefined' || (navigator as Navigator & { webdriver?: boolean }).webdriver) return
  const name = error instanceof Error ? error.name : 'Error'
  const message = redact(error instanceof Error ? error.message : String(error)) + describeContext(context)
  const now = Date.now()
  const last = lastReported.get(`${name}:${message}`)
  if (reported >= REPORT_BUDGET || (last !== undefined && now - last < DEDUPE_MS)) return
  lastReported.set(`${name}:${message}`, now)
  reported += 1
  const report = {
    name,
    message: message.slice(0, MAX_MESSAGE),
    stack: redact(error instanceof Error ? (error.stack ?? '') : '').slice(0, MAX_STACK),
    source: `creators:${typeof context.flow === 'string' ? context.flow : 'unknown'}`,
    url: window.location.href.slice(0, MAX_MESSAGE),
    ua: navigator.userAgent.slice(0, MAX_MESSAGE),
    ts: new Date(now).toISOString()
  }
  // Escaping can still grow the body past the limit, so the stack gives way until it fits.
  let body = JSON.stringify(report)
  while (new TextEncoder().encode(body).length > MAX_BODY_BYTES && report.stack) {
    report.stack = report.stack.slice(0, Math.floor(report.stack.length / 2))
    body = JSON.stringify(report)
  }
  void fetch(CLIENT_ERROR_ENDPOINT, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body,
    keepalive: true
  })
    .then(response => response.body?.cancel())
    .catch(() => undefined)
}
