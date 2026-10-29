// The page's half of the benchmark stream: sends the engine's CSV and log to the benchmark runner
// (local-harness/benchmark), which serves this page from localhost, so a browser run leaves the
// same two files a desktop run does (`--csv`, `--log-file`).
//
// The engine (src/web.rs, crates/perf_output) writes both files' bytes, in the formats the
// report parses and with credentials scrubbed, and hands them over with `__perfAppend(stream,
// text)`; this module only moves them. It lives on the page, not in the engine worker, because a
// panic traps the engine and the page can still finish the run: the engine's panic hook hands
// over what it still held and then the panic's own exception block (`panicked`), and the page
// ends the run. When the engine stops without handing over a block (`crash`: a lost GPU device, a
// hang, a trap the hook could not report), the page takes what it still held and writes a block
// of its own, with a fixed message: no text from the engine or the page reaches the log unless the
// engine scrubbed it.
//
// The offset protocol, per stream: `POST <path>/csv` and `POST <path>/log` carry raw bytes and
// `X-Perf-Offset: <offset of the body's first byte>`. The server appends only at its file's
// length, so a retry with the same offset is harmless; 204 acknowledges. One request is in
// flight per stream, carrying at most MAX_BODY_BYTES of what was written since the last one.
// `POST <path>/end` follows the last acknowledgement and closes the run, with `{"exitCode": n,
// "csvBytes": n, "logBytes": n, "failed": bool}`: the bytes the server acknowledged on each
// stream, and whether either was given up, so the receiver can tell a whole file from a truncated
// one.
//
// The path is resolved against the page's own origin and refused when it lands anywhere else, and
// every request is made without credentials, without following a redirect and only to the same
// origin: whoever sets `window.__bevyPerfStream` (the host page, never a link) gets a stream to its
// own server and to nothing else.

/** What every request of the stream carries besides its method, headers, body and signal. */
const SAME_ORIGIN_ONLY = { credentials: 'omit', redirect: 'error', mode: 'same-origin' }

/** Retries of one request before its stream is given up (about a minute at the longest pause). */
const MAX_ATTEMPTS = 40
const MAX_PAUSE_MS = 1500

/** How long one attempt may go unanswered before it is dropped and made again. */
const REQUEST_TIMEOUT_MS = 15_000

/** The most bytes one request carries; the rest waits for the next. Far below what the harness
 *  takes in one request (local-harness/benchmark/src/limits.rs `chunk_bytes`). */
export const MAX_BODY_BYTES = 1 << 20

/** Every exception's last line holds this; the harness counts an exception by it. */
const EXCEPTION_MARKER = 'DebugLogReportHandler:LogExceptionInternal'

/** The message of the page's own block, by what stopped the engine (`crash`'s `source`). */
const CRASH_MESSAGES = {
  panic: 'the engine panicked and its own report did not arrive',
  gpu: 'the GPU device was lost',
  runtime: 'the engine logged the same fatal error every frame',
  watchdog: 'the engine stopped responding'
}
const OTHER_CRASH = 'the engine stopped after an error'

/**
 * The exception block the page writes when the engine stopped without handing over its own: the
 * layout crates/perf_output's `format::exception` gives a panic with no location (an empty entry
 * line, its trailing space part of the format, `PanicException: message`, the marker line), with a
 * fixed message for `source`. It holds no text from the engine or the page: only the engine
 * scrubs credentials (crates/perf_output/src/scrub.rs), and the harness only counts the exception.
 * @param {string} source what stopped the engine: `panic`, `gpu`, `runtime` (a flood), `watchdog`
 * @param {Date} [now]
 */
export function crashBlock(source, now = new Date()) {
  const message = Object.hasOwn(CRASH_MESSAGES, source) ? CRASH_MESSAGES[source] : OTHER_CRASH
  const two = (n) => String(n).padStart(2, '0')
  const clock = `${two(now.getUTCHours())}:${two(now.getUTCMinutes())}:${two(now.getUTCSeconds())}.${String(now.getUTCMilliseconds()).padStart(3, '0')}`
  return `${clock} - <color=#FF5C5C>[panic]</color>: \nPanicException: ${message}\n${EXCEPTION_MARKER}\n`
}

/** A refusal for good: a 4xx other than 408 (Request Timeout) and 429 (Too Many Requests). The
 *  same request would get the same answer. */
const refusal = (status) => status >= 400 && status < 500 && status !== 408 && status !== 429

/**
 * One POST to `url`, made again until the server acknowledges it (204) or refuses it for good, at
 * most MAX_ATTEMPTS times. Each attempt is dropped after REQUEST_TIMEOUT_MS unanswered. Making a
 * request again is harmless: the server appends by offset, and `end` closes the run once.
 * @param {string} url
 * @param {{ headers: Record<string, string>, body: Uint8Array | string, keepalive?: boolean }} init
 * @param {{ fetch: typeof fetch, wait: (ms: number) => Promise<void>, timeout: (ms: number) => AbortSignal }} io
 * @returns {Promise<{ sent: boolean, refused?: number }>} `refused` is the status of a refusal
 */
async function post(url, init, { fetch, wait, timeout }) {
  for (let attempt = 1; ; attempt++) {
    try {
      const response = await fetch(url, { ...SAME_ORIGIN_ONLY, method: 'POST', ...init, signal: timeout(REQUEST_TIMEOUT_MS) })
      if (response.status === 204) return { sent: true }
      if (refusal(response.status)) return { sent: false, refused: response.status }
    } catch {
      // the server is not there yet, the network dropped it, or it never answered: try again
    }
    if (attempt >= MAX_ATTEMPTS) return { sent: false }
    await wait(Math.min(MAX_PAUSE_MS, 50 * 2 ** attempt))
  }
}

/**
 * One stream's sender: the bytes appended so far, sent in order with the offset protocol.
 * @param {string} url
 * @param {{ fetch: typeof fetch, wait: (ms: number) => Promise<void>, timeout: (ms: number) => AbortSignal }} io
 */
function createSender(url, io) {
  /** unsent bytes, oldest first */
  let pending = []
  /** bytes appended and not acknowledged: those in `pending` and those in flight */
  let pendingLength = 0
  /** bytes the server has acknowledged: the offset of the next request */
  let acknowledged = 0
  let failed = false
  /** the pump's promise while it runs */
  let running = null

  /** The next body: the oldest pending bytes, at most MAX_BODY_BYTES of them. */
  function nextBody() {
    let size = 0
    for (const chunk of pending) {
      size += chunk.length
      if (size >= MAX_BODY_BYTES) break
    }
    const body = new Uint8Array(Math.min(size, MAX_BODY_BYTES))
    let at = 0
    while (at < body.length) {
      const chunk = pending[0]
      const taken = Math.min(chunk.length, body.length - at)
      body.set(taken === chunk.length ? chunk : chunk.subarray(0, taken), at)
      at += taken
      if (taken === chunk.length) pending.shift()
      else pending[0] = chunk.subarray(taken)
    }
    return body
  }

  async function pump() {
    while (pendingLength > 0 && !failed) {
      const body = nextBody()
      const { sent, refused } = await post(
        url,
        { headers: { 'X-Perf-Offset': String(acknowledged), 'Content-Type': 'application/octet-stream' }, body },
        io
      )
      if (sent) {
        acknowledged += body.length
        pendingLength -= body.length
      } else {
        failed = true
        console.error(
          refused === undefined
            ? `[perf stream] gave up sending ${url} at byte ${acknowledged}`
            : `[perf stream] the server refused ${url} at byte ${acknowledged} (${refused}): gave it up`
        )
      }
    }
    running = null
  }

  return {
    append(bytes) {
      if (failed || bytes.length === 0) return
      pending.push(bytes)
      pendingLength += bytes.length
      running ??= pump()
    },
    /** Resolves once everything appended so far was acknowledged (or the stream was given up). */
    idle: () => running ?? Promise.resolve(),
    /** Bytes the server has acknowledged. */
    sent: () => acknowledged,
    /** True once the stream was given up: bytes appended after that, and those pending, never go. */
    failed: () => failed
  }
}

/**
 * The url the stream's requests go under: `path` resolved against `origin`, as an absolute url
 * with no trailing slash, or null when `path` is not a path on that origin. A path starts with one
 * slash and has no query or fragment (a suffix would end up inside it); one that resolves to
 * another origin (`//host`, `/\host`, an absolute url) is refused, and so is the origin itself.
 * An absolute url is always returned, so `//` in what the path resolved to can never be read as
 * another host.
 * @param {unknown} path
 * @param {string | undefined} origin
 * @returns {string | null}
 */
export function streamBase(path, origin) {
  if (typeof path !== 'string' || !path.startsWith('/') || /[?#]/.test(path) || typeof origin !== 'string') return null
  let url
  try {
    url = new URL(path, origin)
  } catch {
    return null
  }
  const trimmed = url.pathname.replace(/\/+$/, '')
  if (url.origin !== origin || trimmed === '') return null
  return url.origin + trimmed
}

/**
 * The run's stream to `path`, a path on the page's own origin (`?perfStream`), or null (and a line
 * on the console) when `path` is not one: nothing is sent then. `csv` (`?perfCsv`) says the CSV
 * goes too; without it the engine sends none and any that arrives is dropped.
 * `io.take` is the engine's `engine_perf_take`: `{ csv, log }`, the text it still holds that was not
 * handed over, empty where it cannot take it.
 * @param {{ path: string, csv: boolean }} options
 * @param {{ fetch?: typeof fetch, wait?: (ms: number) => Promise<void>, now?: () => Date, origin?: string,
 *   timeout?: (ms: number) => AbortSignal, take?: () => { csv?: string, log?: string } | null | undefined }} [io]
 *   `fetch`, `wait`, `now`, `origin` and `timeout` (a request's signal, `AbortSignal.timeout`) are for tests
 */
export function createPerfStream({ path, csv }, io = {}) {
  const base = streamBase(path, io.origin ?? globalThis.location?.origin)
  if (base == null) {
    console.error("[perf stream] the stream path is not a path on this page's origin: nothing is sent")
    return null
  }
  const sendIo = {
    fetch: io.fetch ?? ((...args) => globalThis.fetch(...args)),
    wait: io.wait ?? ((ms) => new Promise((resolve) => setTimeout(resolve, ms))),
    timeout: io.timeout ?? ((ms) => AbortSignal.timeout(ms))
  }
  const now = io.now ?? (() => new Date())
  const encoder = new TextEncoder()
  const senders = { log: createSender(`${base}/log`, sendIo), csv: csv ? createSender(`${base}/csv`, sendIo) : null }
  let ended = null

  /** `end`'s request, made like the others; nothing more to do if it never gets through. It
   *  reports what was sent, which is read once every stream has gone quiet. */
  async function sendEnd(exitCode) {
    const report = JSON.stringify({
      exitCode,
      csvBytes: senders.csv?.sent() ?? 0,
      logBytes: senders.log.sent(),
      failed: senders.log.failed() || senders.csv?.failed() === true
    })
    const { sent, refused } = await post(
      `${base}/end`,
      { headers: { 'Content-Type': 'application/json' }, body: report, keepalive: true },
      sendIo
    )
    if (!sent) console.error(`[perf stream] gave up ending the run${refused === undefined ? '' : ` (${refused})`}`)
  }

  const stream = {
    /** Queue `text` (UTF-8) on the `csv` or `log` stream. Nothing is accepted after `end`. */
    append(name, text) {
      if (ended) return
      senders[name]?.append(encoder.encode(text))
    },
    /**
     * The run is over, with this exit code: once every byte was acknowledged, tell the harness.
     * Returns a promise for that; asking again returns the same one.
     */
    end(exitCode) {
      ended ??= Promise.all([senders.log.idle(), senders.csv?.idle()]).then(() => sendEnd(exitCode))
      return ended
    },
    /**
     * The engine panicked and its panic hook handed over the panic's exception block (`__perfPanic`),
     * which crates/perf_output formatted and scrubbed as the desktop log's, after what it still held
     * (`__perfAppend`). The block goes last and the run ends with 101. Once is enough: a later call
     * only returns the end.
     * @param {string} block
     */
    panicked(block) {
      if (ended) return ended
      stream.append('log', String(block))
      return stream.end(101)
    },
    /**
     * The engine stopped and handed over no block of its own (a panic whose block did not arrive, a
     * lost GPU device, a flood of one fatal error, a hang): what it still held goes first (the CSV,
     * then the log, after everything already appended), then the page's own block for `source`
     * (`crashBlock`), and the run ends with 101. Once is enough: a later call only returns the end.
     * @param {string} source
     */
    crash(source) {
      if (ended) return ended
      let taken = null
      try {
        taken = io.take?.()
      } catch {
        // the engine is trapped: an export that throws has nothing to give
      }
      if (typeof taken?.csv === 'string') stream.append('csv', taken.csv)
      if (typeof taken?.log === 'string') stream.append('log', taken.log)
      stream.append('log', crashBlock(source, now()))
      return stream.end(101)
    }
  }
  return stream
}
