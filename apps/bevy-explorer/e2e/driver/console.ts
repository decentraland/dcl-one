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

/** A whole run of base64 characters (either alphabet). */
const BASE64_RUN = /(?<![A-Za-z0-9+/_-])[A-Za-z0-9+/_-]{4,}={0,2}/g
const BASE64_WORD = /^[A-Za-z0-9+/_-]+={0,2}$/
/** Where an AuthIdentity's JSON may start once its base64 is decoded, as the commands parse it:
 *  any whitespace, then `{` and a quote. */
const OBJECT_START = /^\s*\{\s*"/
/** What makes it one: that object's first key, closed and followed by its colon. */
const OBJECT_KEY = /^\s*\{\s*"[\x20\x21\x23-\x5b\x5d-\x7e]{1,64}"\s*:/
/** JSON text: printable ASCII and whitespace. */
const JSON_TEXT = /^[\t\n\r\x20-\x7e]*$/
/** Where an identity's words end on its line: at the next word that starts with `-`, a flag or a
 *  typo of one. */
const NEXT_FLAG = /\s+-/
/** The quotes and punctuation a hand-typed line may close a word with. */
const CLOSERS = /["'`,;)\]]+$/
/** A payload sent as raw JSON, which the commands refuse after it was logged: the line's first
 *  `{` to its end, closed or cut. */
const RAW_JSON = /\{.*/s
/** Shorter than this, it could be part of other text; an identity is hundreds of characters. */
const MIN_SECRET = 16
/** An object's start with no readable key (a typo in each first key) is an identity this long. */
const LONG_IDENTITY = 128

const decoded = (base64: string): string => Buffer.from(base64, 'base64').toString('latin1')

/** Whether the bytes `length` base64 characters at `offset` of `joined` add are JSON text: a piece
 *  of the identity as it was meant, not one a typo changed. */
function jsonTextAt(joined: string, offset: number, length: number): boolean {
  const from = offset === 0 ? 0 : Math.floor((offset * 3) / 4) + 1
  return JSON_TEXT.test(decoded(joined.slice(0, offset + length)).slice(from))
}

/** The identity a line carries from `start`, if one starts there (its base64 `minimum` characters
 *  long or more, and with its first key readable when `keyed`): the text it spans on the line, up
 *  to the next flag (wrapped, split, quoted or mistyped, all of it), its pieces of base64, and how
 *  far into the line they go (a second identity after its padding is found on its own). */
function identityAt(line: string, start: number, first: string, minimum: number, keyed: boolean): { span: string; pieces: [number, string][]; joined: string; end: number } | undefined {
  const rest = line.slice(start)
  const flag = NEXT_FLAG.exec(rest)
  const span = (flag == null ? rest : rest.slice(0, flag.index)).trimEnd().replace(CLOSERS, '')
  let joined = first
  const pieces: [number, string][] = [[0, first]]
  const word = /\s+(\S+)/y
  word.lastIndex = first.length
  let end = first.length
  for (let next = word.exec(span); next != null; next = word.exec(span)) {
    const piece = next[1]!.replace(CLOSERS, '')
    if (joined.endsWith('=') || !BASE64_WORD.test(piece)) break
    pieces.push([joined.length, piece])
    joined += piece
    end = word.lastIndex
  }
  if (joined.length < minimum || (keyed && !OBJECT_KEY.test(decoded(joined.slice(0, 128))))) return undefined
  return { span, pieces, joined, end: start + end }
}

/** The credentials a line carries: the payload of `/login_identity` or `/hud_login`, an
 *  AuthIdentity (an ephemeral private key and a signed delegation that stays valid for days),
 *  found by what it decodes to wherever a hand-typed line puts it: after flags, as a flag's value,
 *  quoted, glued to other text, twice, wrapped, split or mistyped, or as raw JSON (the commands
 *  refuse such a line, but after it was logged). Once one starts, the line is a secret up to the
 *  next flag, so no piece of it is left; each piece is one of its own too, when its bytes are JSON
 *  text, for text that quotes it alone. Nothing else is one, so an empty or short word, or a
 *  realm's name, never turns lines into `<redacted>`. The web driver's `driver.mjs` has the same
 *  rule, which a test keeps in step. */
export function secretsOf(line: string): string[] {
  if (!/^\s*\/?(?:login_identity|hud_login)(?:\s|$)/.test(line)) return []
  const secrets = new Set<string>()
  let covered = 0
  for (const { 0: run, index } of line.matchAll(BASE64_RUN)) {
    if (index < covered) continue
    // the run holds an identity when any object's start in it has its first key readable; it then
    // starts at the run's first object start, so a typo in the outer key leaves nothing before
    // the inner object in clear. With no key readable (a typo in each), a long one is one still.
    let first = -1
    let keyed = false
    for (let at = 0; at + 4 <= run.length && !keyed; at++) {
      if (!OBJECT_START.test(decoded(run.slice(at, at + 64)))) continue
      if (first < 0) first = at
      keyed = identityAt(line, index + at, run.slice(at), MIN_SECRET, true) != null
    }
    if (first < 0) continue
    const identity = identityAt(line, index + first, run.slice(first), keyed ? MIN_SECRET : LONG_IDENTITY, false)
    if (identity == null) continue
    secrets.add(identity.span)
    for (const [offset, piece] of identity.pieces) if (piece.length >= MIN_SECRET && jsonTextAt(identity.joined, offset, piece.length)) secrets.add(piece)
    covered = identity.end
  }
  const raw = RAW_JSON.exec(line)?.[0].trimEnd()
  if (raw !== undefined && raw.length >= MIN_SECRET) secrets.add(raw)
  return [...secrets]
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
