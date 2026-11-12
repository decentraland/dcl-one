// The console is the one interface tests have to the client: a line in, the command's reply out.
// A transport carries it (the web driver in a browser, the Bevy Remote Protocol on the standalone)
// and reports a failed command through its own channel, since the reply text carries no marker.

export interface Console {
  readonly kind: 'web' | 'native' | 'fake'
  /** Run one console line and resolve with its `Ok` text; reject with a `ConsoleError` on `Err`. */
  run(line: string): Promise<string>
  close(): Promise<void>
  /** What the page said and how it looks now, the HUD included, where the transport can tell. */
  evidence?(): Promise<{ console: string[]; screenshot: Buffer | undefined }>
}

/** A whole run of base64 characters (either alphabet), long enough that it is no ordinary word;
 *  an identity's is hundreds of characters long. */
const BASE64_RUN = /(?<![A-Za-z0-9+/_-])[A-Za-z0-9+/_-]{16,}={0,2}/g
/** How an AuthIdentity's JSON starts once its base64 is decoded, as the commands parse it: any
 *  whitespace, then an object and the quote of its first key. Only the start is read, so a cut or
 *  mistyped identity is still found. */
const JSON_OBJECT_START = /^\s*\{\s*"/
/** A payload sent as raw JSON, which the commands refuse after it was logged: the line's first
 *  `{` to its last `}`. */
const RAW_JSON = /\{.*\}/s
const MIN_SECRET = 16

const encodesJsonObject = (run: string): boolean => JSON_OBJECT_START.test(Buffer.from(run.slice(0, 64), 'base64').toString('latin1'))

/** The credentials a line carries: the payload of `/login_identity` or `/hud_login`, an
 *  AuthIdentity (an ephemeral private key and a signed delegation that stays valid for days),
 *  found by what it decodes to wherever a hand-typed line puts it: after flags, as a flag's value,
 *  quoted, twice, or as raw JSON (the commands refuse such a line, but after it was logged).
 *  Nothing else is one, so an empty or short word, or a realm's name, never turns lines into
 *  `<redacted>`. The web driver's `driver.mjs` has the same rule, which a test keeps in step. */
export function secretsOf(line: string): string[] {
  if (!/^\s*\/?(?:login_identity|hud_login)(?:\s|$)/.test(line)) return []
  const secrets = (line.match(BASE64_RUN) ?? []).filter(encodesJsonObject)
  const raw = RAW_JSON.exec(line)?.[0]
  if (raw !== undefined && raw.length >= MIN_SECRET) secrets.push(raw)
  return secrets
}

/** `text` with each of `secrets` replaced wherever it appears, the longest first, so one that
 *  holds another is never left in part. */
export function replaceSecrets(secrets: Iterable<string>, text: string): string {
  return [...secrets].sort((a, b) => b.length - a.length).reduce((out, secret) => out.split(secret).join('<redacted>'), text)
}

/** `text` with the line's credentials replaced wherever they appear: a reply, an error or a
 *  timeout may quote the line. */
export function redact(line: string, text: string): string {
  return replaceSecrets(secretsOf(line), text)
}

/** The line as it may be logged. */
export function redactLine(line: string): string {
  return redact(line, line)
}

/** What replaces a credential's value in the page's console as the report keeps it. */
export const SCRUBBED = 'REDACTED'
/** The names whose string value is a credential, matched in any case. */
const CREDENTIAL_FIELDS = ['access_token', 'accesstoken', 'privatekey', 'private_key', 'signature']
/** A base64 run shorter than this cannot hold the text of an identity. */
const MIN_BLOB = 24

/** `text` (a line of the page's console) with the value of every credential the engine's strings
 *  are known to carry replaced, by the rules the engine's own benchmark log follows
 *  (`crates/perf_output/src/scrub.rs`) and a few more, since the report is public: a field named
 *  `access_token`, `accessToken`, `privateKey`, `private_key` or `signature`, quoted however JSON,
 *  `Debug` (`Some("…")` too) or an event field quotes it, or bare after `=` (a URL's query: a comms
 *  adapter's room credential); a `Bearer` token; and base64 that decodes to text naming a private
 *  key (an AuthIdentity as `/login_identity` takes it), wherever it starts. The rest stays. */
export function scrubCredentials(text: string): string {
  return scrubIdentities(scrubFields(text)).replace(/\b(Bearer\s+)[A-Za-z0-9._~+/=-]+/gi, `$1${SCRUBBED}`)
}

const endsBareValue = (c: string): boolean => /\s/.test(c) || '&#"\'`\\<>)]},;'.includes(c)

function scrubFields(text: string): string {
  // ASCII lowering keeps every offset of `text`
  const lowered = text.replace(/[A-Z]/g, (c) => c.toLowerCase())
  let out = ''
  let copied = 0
  let from = 0
  for (;;) {
    let at = -1
    let name = ''
    for (const field of CREDENTIAL_FIELDS) {
      const found = lowered.indexOf(field, from)
      if (found >= 0 && (at < 0 || found < at)) {
        at = found
        name = field
      }
    }
    if (at < 0) break
    from = at + name.length
    const span = valueSpan(text, at, at + name.length)
    if (span === undefined || span[0] === span[1]) continue
    out += text.slice(copied, span[0]) + SCRUBBED
    copied = span[1]
    from = span[1]
  }
  return copied === 0 ? text : out + text.slice(copied)
}

/** The range of the value after the name at `at..nameEnd`: an optional closing quote of the name,
 *  `:` or `=`, then a quoted string, or for the query names after `=` a bare value. */
function valueSpan(text: string, at: number, nameEnd: number): [number, number] | undefined {
  const quote = quoteBefore(text.slice(0, at))
  let rest = text.slice(nameEnd)
  if (quote !== '') {
    if (!rest.startsWith(quote)) return undefined
    rest = rest.slice(quote.length)
  }
  rest = rest.trimStart()
  const separator = rest[0]
  if (separator !== ':' && separator !== '=') return undefined
  rest = rest.slice(1).trimStart()
  // a Rust `Option` as `Debug` prints it
  if (rest.startsWith('Some(')) rest = rest.slice('Some('.length)
  const valueStart = text.length - rest.length
  const open = openingQuote(rest)
  if (open !== undefined) {
    const start = valueStart + open.length
    if (open === '"') return [start, start + unescapedQuote(text.slice(start))]
    const close = text.indexOf(open, start)
    return [start, close < 0 ? text.length : close]
  }
  if (separator === '=') {
    const end = [...rest].findIndex(endsBareValue)
    return [valueStart, end < 0 ? text.length : valueStart + end]
  }
  return undefined
}

/** The quote that closes a name written in quotes: `"`, or an escaped `\"`; empty for a bare name. */
function quoteBefore(before: string): string {
  if (!before.endsWith('"')) return ''
  const without = before.slice(0, -1)
  const backslashes = without.length - without.replace(/\\+$/, '').length
  return before.slice(before.length - 1 - backslashes)
}

function openingQuote(rest: string): string | undefined {
  const backslashes = rest.length - rest.replace(/^\\+/, '').length
  return rest[backslashes] === '"' ? rest.slice(0, backslashes + 1) : undefined
}

/** Where a JSON string that starts at the beginning of `text` ends: its first unescaped quote. */
function unescapedQuote(text: string): number {
  let escaped = false
  for (let i = 0; i < text.length; i++) {
    const c = text[i]
    if (escaped) escaped = false
    else if (c === '\\') escaped = true
    else if (c === '"') return i
  }
  return text.length
}

const isBase64 = (c: string | undefined): boolean => c !== undefined && /[A-Za-z0-9+/_-]/.test(c)

/** Base64 blobs whose decoded text names a private key. */
function scrubIdentities(text: string): string {
  let out = ''
  let copied = 0
  let at = 0
  while (at < text.length) {
    if (!isBase64(text[at])) {
      at += 1
      continue
    }
    const start = at
    while (isBase64(text[at])) at += 1
    for (let padding = 0; padding < 2 && text[at] === '='; padding++) at += 1
    const blob = text.slice(start, at)
    if (blob.length >= MIN_BLOB && namesPrivateKey(blob)) {
      out += text.slice(copied, start) + SCRUBBED
      copied = at
    }
  }
  return copied === 0 ? text : out + text.slice(copied)
}

/** Whether `blob` is base64 of text naming a private key; only whole groups of four are decoded,
 *  from each of the four places the base64 can start in a run glued to the text before it (a URL's
 *  path, a word). */
function namesPrivateKey(blob: string): boolean {
  const trimmed = blob.replace(/=+$/, '')
  for (let offset = 0; offset < 4; offset++) {
    const from = trimmed.slice(offset)
    const body = from.slice(0, from.length - (from.length % 4))
    const decoded = Buffer.from(body, /[-_]/.test(body) ? 'base64url' : 'base64').toString('latin1').toLowerCase()
    if (decoded.includes('privatekey') || decoded.includes('private_key')) return true
  }
  return false
}

/** A command the client answered `Err`. The line and the output are kept redacted: this error
 *  reaches the test runner's output, the JUnit report, the scenario's command log and the PR's
 *  status comment. */
export class ConsoleError extends Error {
  readonly line: string
  readonly output: string

  constructor(line: string, output: string) {
    const safeLine = redactLine(line)
    const safeOutput = redact(line, output)
    super(`\`${safeLine}\` failed: ${safeOutput}`)
    this.name = 'ConsoleError'
    this.line = safeLine
    this.output = safeOutput
  }
}

/** A line as the transports send it: one leading `/`, single spaces between arguments. */
export function normalizeLine(line: string): string {
  const trimmed = line.trim().replace(/\s+/g, ' ')
  if (trimmed === '') throw new Error('empty console command')
  return trimmed.startsWith('/') ? trimmed : `/${trimmed}`
}

/** The command name of a line, without the `/`. */
export function commandOf(line: string): string {
  return normalizeLine(line).slice(1).split(' ', 1)[0] ?? ''
}

export type Reply = string | ((line: string) => string | Promise<string>)

/** A console that answers from a table, for the driver's own tests: `replies` maps a command name
 *  (without the `/`) to its reply, or to a function of the whole line. A reply function may throw
 *  a `ConsoleError` to fail the command. Every line sent is kept in `lines`. */
export class FakeConsole implements Console {
  readonly kind = 'fake' as const
  readonly lines: string[] = []
  closed = false
  readonly #replies: Record<string, Reply>

  constructor(replies: Record<string, Reply> = {}) {
    this.#replies = replies
  }

  async run(line: string): Promise<string> {
    const normalized = normalizeLine(line)
    this.lines.push(normalized)
    const reply = this.#replies[commandOf(normalized)]
    if (reply === undefined) {
      const known = Object.keys(this.#replies)
        .sort()
        .map((name) => `"/${name}"`)
        .join(', ')
      throw new ConsoleError(normalized, `Command not recognized: \`${normalized.split(' ', 1)[0]}\`. Recognized commands: [${known}]`)
    }
    return typeof reply === 'string' ? reply : reply(normalized)
  }

  async close(): Promise<void> {
    this.closed = true
  }

  /** The lines sent for one command name. */
  sent(command: string): string[] {
    return this.lines.filter((line) => commandOf(line) === command)
  }
}
