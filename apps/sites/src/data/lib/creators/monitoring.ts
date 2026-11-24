// Errors are reported through the console: Sites sends every `console.error` to its client-error endpoint, so
// what is logged is the report and goes out redacted.
//
// Convention: pass a `flow` in the context (e.g. { flow: 'publish-collection' }) so failures group by
// user action. Never put secrets in the context; the report is redacted defensively anyway.
import { errorChain } from './walletErrors'

export type ErrorContext = Record<string, unknown>

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

/** The context as one short line of its plain values, without the sensitive keys. */
function describeContext(context: ErrorContext): string {
  const facts = Object.entries(context)
    .filter(
      ([key, value]) =>
        key !== 'flow' && value !== '' && !SENSITIVE_KEY.test(key) && ['string', 'number', 'boolean'].includes(typeof value)
    )
    .map(([key, value]) => `${key}=${redact(String(value))}`)
  return facts.length > 0 ? ` [${facts.join(' ')}]` : ''
}

// A thrown value of unknown shape may hold anything, so only these fields of it are reported.
const VALUE_FACTS = ['code', 'status', 'statusCode', 'name', 'reason'] as const

/** A thrown value that is not an Error, by its type and known facts: `Object [code=4001]`, not `[object Object]`. */
function describeValue(value: unknown): string {
  if (typeof value !== 'object' || value === null) return String(value)
  const type = (value as { constructor?: { name?: unknown } }).constructor?.name
  const record = value as Record<string, unknown>
  const known = Object.fromEntries(VALUE_FACTS.filter(key => key in record).map(key => [key, record[key]]))
  return `${typeof type === 'string' && type !== '' ? type : 'Object'}${describeContext(known)}`
}

/** Logs a failure, redacted, with the flow that produced it and the plain facts of its context. Never throws. */
export function captureError(error: unknown, context: ErrorContext = {}): void {
  try {
    const label = typeof context.flow === 'string' ? `error in ${context.flow}` : 'error'
    // The caller's own context wins: these are a fallback read off the thrown value, never an override.
    const facts = describeContext({ ...rpcFactsFrom(error), ...context })
    const reportable = toReportable(error)
    const summary =
      reportable instanceof Error ? `${reportable.name}: ${redact(reportable.message)}` : redact(describeValue(reportable))
    // V8 stacks open with the name and message, which the summary already carries.
    const frames =
      reportable instanceof Error && reportable.stack
        ? reportable.stack.replace(`${reportable.name}: ${reportable.message}`, '').replace(/^\n/, '')
        : ''
    // The site's reporter keeps only the first 500 characters, so the facts go before a message that can be long.
    console.error(`[creators] ${label}${facts}: ${summary}${frames ? `\n${redact(frames)}` : ''}`)
  } catch {
    // Reporting must never throw back into the caller's catch block.
  }
}
