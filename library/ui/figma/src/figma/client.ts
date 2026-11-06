import { existsSync } from 'node:fs'
import { redactFileKey } from '../config.js'

const FIGMA_API = 'https://api.figma.com'
const MAX_RETRIES = 3
const DEFAULT_TIMEOUT_MS = 60000
// Whole-file, whole-page and render calls: Figma answers 503 while it computes, and a held request has taken
// up to 159s to complete.
const LONG_READ_TIMEOUT_MS = 240000
const RETRYABLE_STATUS = new Set([500, 502, 503, 504])

class FigmaApiError extends Error {
  constructor(
    readonly status: number,
    path: string,
    message: string
  ) {
    // API paths embed the private file key; the error never carries it.
    super(redactFileKey(`Figma API ${status} on ${path}: ${message}`))
    this.name = 'FigmaApiError'
  }
}

interface TokenProvider {
  getToken(): Promise<string>
}

class EnvTokenProvider implements TokenProvider {
  constructor(private readonly env: NodeJS.ProcessEnv = process.env) {}

  async getToken(): Promise<string> {
    if (!this.env.FIGMA_TOKEN && existsSync('.env')) {
      process.loadEnvFile('.env')
    }
    const token = this.env.FIGMA_TOKEN
    if (!token || token === '<FIGMA_TOKEN>') {
      throw new Error('FIGMA_TOKEN is not set. Copy .env.example to .env and fill it in.')
    }
    return token
  }
}

type Sleep = (ms: number) => Promise<void>

interface ClientOptions {
  sleep?: Sleep
  // Receives one diagnostic line per attempt. It never carries headers, the token or response contents.
  log?: (line: string) => void
  timeoutMs?: number
  now?: () => number
}

interface RequestOptions {
  timeoutMs?: number
}

// A timeout (also while the body streams), an aborted request or a dropped connection.
function isTransientError(error: unknown): boolean {
  if (error instanceof TypeError) return true
  return error instanceof Error && (error.name === 'TimeoutError' || error.name === 'AbortError')
}

function causeOf(error: unknown): string {
  return error instanceof Error ? `${error.name}: ${error.message}` : String(error)
}

const KNOWN_ERROR_NAMES = new Set([
  'Error',
  'TypeError',
  'RangeError',
  'SyntaxError',
  'AbortError',
  'TimeoutError',
  'DOMException',
  'DownloadLimitError'
])

// Known error names and codes only (e.g. "TypeError Error UND_ERR_SOCKET"): a fetch error's message, name or
// cause can embed the request URL, and a signed URL's query is a credential.
function codesOf(error: unknown): string {
  const parts: string[] = []
  for (let current = error, depth = 0; current instanceof Error && depth < 5; depth++) {
    const { code } = current as { code?: unknown }
    parts.push(KNOWN_ERROR_NAMES.has(current.name) ? current.name : 'Error')
    if (typeof code === 'string' && code.length <= 64 && /^[A-Z0-9_]+$/.test(code)) parts.push(code)
    current = current.cause
  }
  return [...new Set(parts)].join(' ') || 'unknown error'
}

class DownloadLimitError extends Error {}

async function readBytes(response: Response, limit?: number, truncate = false) {
  if (!response.body) throw new Error('response has no body')
  const chunks: Uint8Array[] = []
  let size = 0
  for await (const chunk of response.body) {
    if (limit !== undefined && chunk.length > limit - size) {
      if (!truncate) throw new RangeError(`exceeds ${limit} bytes`)
      chunks.push(Buffer.from(chunk.subarray(0, limit - size)))
      break
    }
    size += chunk.length
    chunks.push(chunk)
    if (truncate && size === limit) break
  }
  return Buffer.concat(chunks)
}

async function bodyExcerpt(response: Response): Promise<string> {
  try {
    // Breaking the stream at the byte budget cancels the unread error body, including retry responses.
    return (await readBytes(response, 1024, true)).toString('utf8').slice(0, 200)
  } catch {
    return ''
  }
}

interface Attempt<T> {
  // Describes the target in errors and diagnostics; for a signed URL it must not contain the query.
  label: string
  url: string | URL
  init: () => Promise<RequestInit>
  read: (response: Response) => Promise<{ value: T; bytes: number }>
  // An HTTP failure that is not retried, or that exhausted the retries.
  fail: (status: number, detail: string) => Error
  timeoutMs: number
  // For a signed URL: causes are reduced to error names and codes, and are never attached.
  sanitize: boolean
}

class FigmaClient {
  private readonly sleep: Sleep
  private readonly log: (line: string) => void
  private readonly timeoutMs: number
  private readonly now: () => number

  constructor(
    private readonly tokens: TokenProvider,
    private readonly fetchFn: typeof fetch = fetch,
    options: ClientOptions = {}
  ) {
    this.sleep = options.sleep ?? ((ms) => new Promise((resolve) => setTimeout(resolve, ms)))
    this.log = options.log ?? ((line) => process.stderr.write(`${line}\n`))
    this.timeoutMs = options.timeoutMs ?? DEFAULT_TIMEOUT_MS
    this.now = options.now ?? Date.now
  }

  async get<T>(path: string, options: RequestOptions = {}): Promise<T> {
    return this.request<T>({
      // API paths embed the private file key; neither diagnostics nor errors carry it.
      label: redactFileKey(path),
      url: `${FIGMA_API}${path}`,
      init: async () => ({ headers: { 'X-Figma-Token': await this.tokens.getToken() } }),
      read: async (response) => {
        const bytes = await readBytes(response)
        return { value: JSON.parse(bytes.toString('utf8')) as T, bytes: bytes.length }
      },
      fail: (status, detail) => new FigmaApiError(status, path, detail),
      timeoutMs: options.timeoutMs ?? this.timeoutMs,
      sanitize: false
    })
  }

  // Unauthenticated download of a signed export URL: no token is sent and redirects are refused.
  async download(url: URL, label: string, limit: number): Promise<Buffer> {
    return this.request<Buffer>({
      label: `${url.host} ${label}`,
      url,
      init: async () => ({ redirect: 'error' }),
      read: async (response) => {
        const bytes = await readBytes(response, limit).catch((error: unknown) => {
          if (error instanceof RangeError)
            throw new DownloadLimitError(`Export exceeds ${limit / 1024 / 1024} MiB: ${label}`)
          throw error
        })
        return { value: bytes, bytes: bytes.length }
      },
      fail: (status) => new Error(`Export download failed for ${label}: ${status}`),
      timeoutMs: this.timeoutMs,
      sanitize: true
    })
  }

  private async request<T>(attempt: Attempt<T>): Promise<T> {
    const total = MAX_RETRIES + 1
    for (let retry = 0; ; retry++) {
      const number = retry + 1
      const started = this.now()
      const line = (outcome: string) =>
        this.log(
          redactFileKey(
            `[figma] GET ${attempt.label} attempt ${number}/${total} ${outcome} ${this.now() - started}ms`
          )
        )
      const canRetry = retry < MAX_RETRIES
      const reason = attempt.sanitize ? codesOf : causeOf
      // A non-retryable error from fetch or the body; for a signed URL it is replaced, never passed on.
      const fatal = (error: unknown) => {
        if (error instanceof DownloadLimitError) return error
        if (!attempt.sanitize)
          return error instanceof Error && redactFileKey(error.message) !== error.message
            ? new Error(redactFileKey(error.message))
            : error
        return new Error(`Request failed on ${attempt.label}: ${codesOf(error)}`)
      }
      const transient = async (cause: unknown, outcome: string) => {
        line(outcome)
        if (!canRetry)
          throw new Error(
            redactFileKey(
              `Figma request failed after ${total} attempts on ${attempt.label}: ${reason(cause)}`
            ),
            attempt.sanitize ? undefined : { cause }
          )
        await this.sleep(2000 * 2 ** retry)
      }

      let response: Response
      try {
        response = await this.fetchFn(attempt.url, {
          ...(await attempt.init()),
          signal: AbortSignal.timeout(attempt.timeoutMs)
        })
      } catch (error) {
        if (!isTransientError(error)) {
          line(`failed: ${reason(error)}`)
          throw fatal(error)
        }
        await transient(error, `failed: ${reason(error)}`)
        continue
      }

      // The status is classified before the body is touched, so a body error can never turn an HTTP
      // failure into a retryable one.
      const status = response.status
      if (status === 429) {
        const detail = await bodyExcerpt(response)
        line(`status 429`)
        if (!canRetry) throw attempt.fail(429, detail)
        const retryAfter = Number(response.headers.get('retry-after') ?? '1')
        if (!Number.isFinite(retryAfter) || retryAfter < 0 || retryAfter > 60)
          throw attempt.fail(
            429,
            'Rate limit requires a later run; no immediate retries were made.'
          )
        await this.sleep(retryAfter * 1000)
        continue
      }
      if (RETRYABLE_STATUS.has(status)) {
        const detail = await bodyExcerpt(response)
        line(`status ${status}`)
        if (!canRetry) throw attempt.fail(status, `${detail}; gave up after ${total} attempts`)
        await this.sleep(2000 * 2 ** retry)
        continue
      }
      if (!response.ok) {
        const detail = await bodyExcerpt(response)
        line(`status ${status}`)
        throw attempt.fail(status, detail)
      }

      let result: { value: T; bytes: number }
      try {
        result = await attempt.read(response)
      } catch (error) {
        if (!isTransientError(error)) {
          line(`status ${status} body failed: ${reason(error)}`)
          throw fatal(error)
        }
        await transient(error, `status ${status} body failed: ${reason(error)}`)
        continue
      }
      line(`status ${status} ${result.bytes}B`)
      return result.value
    }
  }
}

export { EnvTokenProvider, FigmaApiError, FigmaClient, LONG_READ_TIMEOUT_MS }
export type { ClientOptions, TokenProvider }
