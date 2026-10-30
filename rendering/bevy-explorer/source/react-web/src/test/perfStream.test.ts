// DOMAIN: the page's half of the performance harness's browser stream — the offset protocol
// the engine's CSV and log bytes travel by (deploy/web/engine/perf_stream.js; the engine half is
// crates/perf_output, the server the harness's browser leg). A fake server stands in for it.

import { describe, expect, it, vi, type MockInstance } from 'vitest'
// plain JS served as is by the engine page (deploy/web/engine), so it has no declarations
// @ts-expect-error TS7016
import { createPerfStream, crashBlock, MAX_BODY_BYTES, streamBase } from '../../../deploy/web/engine/perf_stream.js'

/** The same text as `the_pages_own_block_is_a_panic_block_with_a_fixed_message` in
 *  crates/perf_output (src/format.rs): the block the page writes, with a fixed message, when the
 *  engine panicked and its own block did not arrive. */
const GOLDEN_CRASH_BLOCK =
  '13:14:15.678 - <color=#FF5C5C>[panic]</color>: \n' +
  'PanicException: the engine panicked and its own report did not arrive\n' +
  'DebugLogReportHandler:LogExceptionInternal\n'

/** A panic's block as the engine's panic hook hands it over: formatted and scrubbed by
 *  crates/perf_output (`layer::panic_block`), never by the page. */
const ENGINE_PANIC_BLOCK =
  '13:14:15.678 - <color=#FF5C5C>[panic]</color>: \n' +
  'PanicException: panicked at src/lib.rs:5:9:\n' +
  '    connect wss://h.example/r?access_token=REDACTED\n' +
  'DebugLogReportHandler:LogExceptionInternal\n'

const NOW = new Date(Date.UTC(2026, 9, 26, 13, 14, 15, 678))

describe('crashBlock', () => {
  it('is the exception block the engine writes for a panic, with the page\'s fixed message', () => {
    expect(crashBlock('panic', NOW)).toBe(GOLDEN_CRASH_BLOCK)
  })

  it('says what stopped the engine in fixed words, and holds no other text', () => {
    const message = (source: unknown) => crashBlock(source, NOW).split('\n')[1]
    expect(message('gpu')).toBe('PanicException: the GPU device was lost')
    expect(message('runtime')).toBe('PanicException: the engine logged the same fatal error every frame')
    expect(message('watchdog')).toBe('PanicException: the engine stopped responding')
    // whatever else is asked for gets the one generic message: no caller's text goes in
    for (const source of ['error', 'wss://h/r?access_token=SECRET', 'toString', '__proto__', undefined, 42]) {
      const block = crashBlock(source, NOW)
      expect(block.split('\n')[1]).toBe('PanicException: the engine stopped after an error')
      expect(block).not.toContain('SECRET')
    }
  })
})

/** The origin the streams under test belong to: the page they run on. */
const ORIGIN = 'https://play.example'

/** What a request carried besides its url: the options the page's own fetch is given. */
interface Call {
  origin: string
  init: Record<string, unknown>
}

interface Request {
  url: string
  offset: number | null
  body: Uint8Array
  status: number
}

/** The harness server's rules for a stream (its browser leg): append at the stored length, 204 for
 *  bytes it already has, 409 for a gap. A test can hold its responses back, make requests fail,
 *  answer them with a status of its own or leave them unanswered. */
function fakeServer() {
  const files: Record<string, number[]> = {}
  const requests: Request[] = []
  const calls: Call[] = []
  const ends: { exitCode: number; after: number }[] = []
  /** the whole body of each `end`, as the receiver reads it */
  const endBodies: Array<Record<string, unknown>> = []
  let inFlight = 0
  let maxInFlightPerUrl = 0
  const inFlightPerUrl: Record<string, number> = {}
  const failures: Record<string, number> = {}
  const answers: Record<string, number[]> = {}
  const silences: Record<string, number> = {}
  let hold: Promise<void> | null = null
  let ended = false

  const next = <T>(table: Record<string, T>, url: string, left: (value: T) => boolean) =>
    Object.keys(table).find((match) => url.includes(match) && left(table[match]))

  // the requests' urls are told apart by path; the origin each went to is kept in `calls`
  const fetch = async (input: string, init: { headers: Record<string, string>; body: Uint8Array | string; signal?: AbortSignal }) => {
    const target = new URL(input)
    const url = target.pathname
    calls.push({ origin: target.origin, init })
    inFlight++
    inFlightPerUrl[url] = (inFlightPerUrl[url] ?? 0) + 1
    maxInFlightPerUrl = Math.max(maxInFlightPerUrl, inFlightPerUrl[url])
    try {
      const silent = next(silences, url, (n) => n > 0)
      if (silent) {
        silences[silent]--
        // never answered: only the request's own signal ends it, as it ends a real fetch
        await new Promise((_, reject) => init.signal?.addEventListener('abort', () => reject(init.signal?.reason)))
      }
      if (hold) await hold
      const failing = next(failures, url, (n) => n > 0)
      if (failing) {
        failures[failing]--
        throw new TypeError('Failed to fetch')
      }
      const answering = next(answers, url, (statuses) => statuses.length > 0)
      if (answering) {
        const status = answers[answering].shift() as number
        requests.push({ url, offset: null, body: new Uint8Array(), status })
        return { status }
      }
      if (url.endsWith('/end')) {
        const report = JSON.parse(init.body as string)
        endBodies.push(report)
        ends.push({ exitCode: report.exitCode, after: requests.length })
        ended = true
        return { status: 204 }
      }
      const body = init.body as Uint8Array
      const offset = init.headers['X-Perf-Offset'] === undefined ? null : Number(init.headers['X-Perf-Offset'])
      const file = (files[url] ??= [])
      let status = 400
      if (ended) status = 409
      else if (offset != null && offset <= file.length) {
        status = 204
        for (const byte of body.subarray(file.length - offset)) file.push(byte)
      } else if (offset != null) status = 409
      requests.push({ url, offset, body: body.slice(), status })
      return { status }
    } finally {
      inFlight--
      inFlightPerUrl[url]--
    }
  }
  return {
    fetch,
    files,
    requests,
    calls,
    ends,
    endBodies,
    maxInFlightPerUrl: () => maxInFlightPerUrl,
    /** the next `n` requests whose url holds `match` fail the way a dropped connection does */
    failNext: (match: string, n: number) => (failures[match] = n),
    /** the next requests whose url holds `match` get these statuses, one each, and store nothing */
    answerNext: (match: string, ...statuses: number[]) => (answers[match] = statuses),
    /** the next `n` requests whose url holds `match` are never answered */
    silenceNext: (match: string, n: number) => (silences[match] = n),
    holdResponses: () => {
      let release!: () => void
      hold = new Promise<void>((resolve) => (release = resolve))
      return () => {
        hold = null
        release()
      }
    },
    text: (url: string) => new TextDecoder().decode(new Uint8Array(files[url] ?? [])),
    inFlight: () => inFlight
  }
}

const instant = () => Promise.resolve()

function stream(server: ReturnType<typeof fakeServer>, csv = true, path = '/__perf/change_build_1') {
  return createPerfStream({ path, csv }, { fetch: server.fetch, wait: instant, now: () => NOW, origin: ORIGIN })
}

/** Quiets the console lines a given-up stream prints, for the length of `body`. */
async function quietly(body: (errors: MockInstance<typeof console.error>) => Promise<void>) {
  const errors = vi.spyOn(console, 'error').mockImplementation(() => undefined)
  try {
    await body(errors)
  } finally {
    errors.mockRestore()
  }
}

describe('the offset protocol', () => {
  it('sends each stream to its own path, from offset 0, then the next bytes at the offset after them', async () => {
    const server = fakeServer()
    const s = stream(server)
    s.append('log', 'first\n')
    s.append('csv', '"Frame"\r\n')
    await s.end(0)
    s.append('log', 'after end\n')
    expect(server.text('/__perf/change_build_1/log')).toBe('first\n')
    expect(server.text('/__perf/change_build_1/csv')).toBe('"Frame"\r\n')
    expect(server.requests.map((r) => [r.url, r.offset, r.status])).toEqual([
      ['/__perf/change_build_1/log', 0, 204],
      ['/__perf/change_build_1/csv', 0, 204]
    ])
    expect(server.ends).toEqual([{ exitCode: 0, after: 2 }])
  })

  it('keeps one request in flight per stream and sends what was written meanwhile in the next', async () => {
    const server = fakeServer()
    const release = server.holdResponses()
    const s = stream(server)
    s.append('log', 'a')
    await Promise.resolve()
    s.append('log', 'b')
    s.append('log', 'c')
    s.append('csv', 'x')
    release()
    await s.end(0)
    expect(server.maxInFlightPerUrl()).toBe(1)
    const log = server.requests.filter((r) => r.url.endsWith('/log'))
    expect(log.map((r) => [r.offset, new TextDecoder().decode(r.body)])).toEqual([
      [0, 'a'],
      [1, 'bc']
    ])
    expect(server.text('/__perf/change_build_1/log')).toBe('abc')
  })

  it('counts offsets in UTF-8 bytes', async () => {
    const server = fakeServer()
    const s = stream(server)
    s.append('log', 'é')
    await new Promise((resolve) => setTimeout(resolve, 0))
    s.append('log', 'x')
    await s.end(0)
    expect(server.requests.map((r) => r.offset)).toEqual([0, 2])
  })

  it('retries a failed request with the same offset, and the file still has every byte once', async () => {
    const server = fakeServer()
    server.failNext('/log', 3)
    const s = stream(server)
    s.append('log', 'hello\n')
    await s.end(0)
    expect(server.requests.map((r) => [r.offset, r.status])).toEqual([[0, 204]])
    expect(server.text('/__perf/change_build_1/log')).toBe('hello\n')
    expect(server.ends).toHaveLength(1)
  })

  it('ends only after every byte of both streams was acknowledged', async () => {
    const server = fakeServer()
    const release = server.holdResponses()
    const s = stream(server)
    s.append('log', 'l\n')
    s.append('csv', 'c\r\n')
    let ended = false
    const end = s.end(0).then(() => (ended = true))
    await Promise.resolve()
    expect(server.ends).toEqual([])
    release()
    await end
    expect(ended).toBe(true)
    expect(server.ends).toEqual([{ exitCode: 0, after: 2 }])
  })

  it('sends no CSV without perfCsv', async () => {
    const server = fakeServer()
    const s = stream(server, false)
    s.append('csv', 'rows\r\n')
    s.append('log', 'line\n')
    await s.end(0)
    expect(server.requests.map((r) => r.url)).toEqual(['/__perf/change_build_1/log'])
  })

  it('ends once, with the first exit code', async () => {
    const server = fakeServer()
    const s = stream(server)
    const first = s.end(3)
    expect(s.end(0)).toBe(first)
    await first
    expect(server.ends).toEqual([{ exitCode: 3, after: 0 }])
  })

  it('ends the run even when a stream could not be sent', async () => {
    const server = fakeServer()
    server.failNext('/log', 1000)
    const s = stream(server)
    s.append('log', 'lost\n')
    s.append('csv', 'kept\r\n')
    await s.end(7)
    expect(server.text('/__perf/change_build_1/log')).toBe('')
    expect(server.text('/__perf/change_build_1/csv')).toBe('kept\r\n')
    expect(server.ends).toEqual([{ exitCode: 7, after: 1 }])
  })
})

describe('a body', () => {
  it('carries at most 1 MiB: a backlog goes in order, in bodies that each start where the last ended', async () => {
    const server = fakeServer()
    const release = server.holdResponses()
    const s = stream(server, false)
    const MiB = MAX_BODY_BYTES
    expect(MiB).toBe(1024 * 1024)
    // one write larger than a body, then two that share bodies with it; the second body ends in
    // the middle of the first 'é' (two bytes in UTF-8)
    const parts = ['a'.repeat(MiB + MiB / 2), 'b'.repeat(MiB / 2 - 1), 'é'.repeat(100_000)]
    for (const part of parts) s.append('log', part)
    release()
    await s.end(0)
    const total = 2 * MiB - 1 + 200_000
    expect(server.requests.map((r) => [r.offset, r.body.length, r.status])).toEqual([
      [0, MiB, 204],
      [MiB, MiB, 204],
      [2 * MiB, total - 2 * MiB, 204]
    ])
    // the character split between two bodies is whole again in the file
    expect(server.text('/__perf/change_build_1/log')).toBe(parts.join(''))
    expect(server.endBodies[0]).toMatchObject({ logBytes: total, failed: false })
  })
})

describe('a request', () => {
  /** A stream whose requests' signals the test aborts itself, with the timeout each asked for. */
  function timed(server: ReturnType<typeof fakeServer>) {
    const timeouts: number[] = []
    const signals: AbortController[] = []
    const s = createPerfStream(
      { path: '/__perf/change_build_1', csv: false },
      {
        fetch: server.fetch,
        wait: instant,
        now: () => NOW,
        origin: ORIGIN,
        timeout: (ms: number) => {
          timeouts.push(ms)
          const controller = new AbortController()
          signals.push(controller)
          return controller.signal
        }
      }
    )
    return { s, timeouts, signals }
  }

  it('the server never answers is dropped at its timeout and made again with the same bytes', async () => {
    const server = fakeServer()
    server.silenceNext('/log', 1)
    const { s, timeouts, signals } = timed(server)
    s.append('log', 'hello\n')
    const end = s.end(0)
    await vi.waitFor(() => expect(signals).toHaveLength(1))
    // nothing moves while the request hangs: not the stream, not the end
    await new Promise((resolve) => setTimeout(resolve, 0))
    expect(server.ends).toEqual([])
    signals[0].abort(new DOMException('The operation timed out.', 'TimeoutError'))
    await end
    expect(server.requests.map((r) => [r.offset, r.status])).toEqual([[0, 204]])
    expect(server.text('/__perf/change_build_1/log')).toBe('hello\n')
    expect(server.ends).toEqual([{ exitCode: 0, after: 1 }])
    // every attempt, the end's included, is given its own 15 s
    expect(timeouts).toEqual([15_000, 15_000, 15_000])
    expect(server.calls.map((call) => call.init.signal)).toEqual(signals.map((controller) => controller.signal))
  })

  it('has a timeout of its own by default', async () => {
    const server = fakeServer()
    const s = stream(server)
    s.append('log', 'line\n')
    await s.end(0)
    for (const call of server.calls) expect(call.init.signal).toBeInstanceOf(AbortSignal)
  })

  it('is refused for good by a 4xx other than 408 and 429: the stream is given up at once, and the end says so', async () => {
    for (const status of [400, 404, 409, 413]) {
      await quietly(async (errors) => {
        const server = fakeServer()
        server.answerNext('/csv', status)
        const s = stream(server)
        s.append('csv', 'rows\r\n')
        s.append('log', 'line\n')
        await new Promise((resolve) => setTimeout(resolve, 0))
        s.append('csv', 'more rows\r\n')
        await s.end(0)
        // one request, not one per retry, and nothing after it
        expect(server.requests.filter((r) => r.url.endsWith('/csv')).map((r) => r.status)).toEqual([status])
        expect(server.text('/__perf/change_build_1/csv')).toBe('')
        expect(server.text('/__perf/change_build_1/log')).toBe('line\n')
        expect(server.endBodies).toEqual([{ exitCode: 0, csvBytes: 0, logBytes: 5, failed: true }])
        expect(errors.mock.calls.some(([message]) => String(message).includes(`refused`) && String(message).includes(String(status)))).toBe(true)
      })
    }
  })

  it('is made again after a 408, a 429 or a 5xx, like a dropped one', async () => {
    const server = fakeServer()
    server.answerNext('/log', 408, 429, 503)
    const s = stream(server)
    s.append('log', 'hello\n')
    await s.end(0)
    expect(server.requests.map((r) => r.status)).toEqual([408, 429, 503, 204])
    expect(server.text('/__perf/change_build_1/log')).toBe('hello\n')
    expect(server.endBodies).toEqual([{ exitCode: 0, csvBytes: 0, logBytes: 6, failed: false }])
  })

  it('to end the run is not made again once refused', async () => {
    await quietly(async () => {
      const server = fakeServer()
      server.answerNext('/end', 400)
      const s = stream(server)
      await s.end(0)
      expect(server.requests.map((r) => [r.url, r.status])).toEqual([['/__perf/change_build_1/end', 400]])
      expect(server.calls).toHaveLength(1)
      expect(server.ends).toEqual([])
    })
  })
})

describe('the engine\'s own panic block', () => {
  it('goes last on the log, after what the engine handed over, and the run ends with 101', async () => {
    const server = fakeServer()
    const take = vi.fn(() => ({ csv: 'never asked\r\n', log: 'never asked\n' }))
    const s = createPerfStream({ path: '/__perf/change_build_1', csv: true }, { fetch: server.fetch, wait: instant, now: () => NOW, origin: ORIGIN, take })
    s.append('log', '13:14:15.000 - <color=#DBD6C8>[x]</color>: before\n')
    s.append('csv', 'held rows\r\n')
    await s.panicked(ENGINE_PANIC_BLOCK)
    expect(server.text('/__perf/change_build_1/log')).toBe('13:14:15.000 - <color=#DBD6C8>[x]</color>: before\n' + ENGINE_PANIC_BLOCK)
    expect(server.text('/__perf/change_build_1/csv')).toBe('held rows\r\n')
    expect(server.ends).toEqual([{ exitCode: 101, after: server.requests.length }])
    // the engine handed everything over itself: nothing is taken, and the page writes no block
    expect(take).not.toHaveBeenCalled()
  })

  it('is counted once, and a crash after it adds nothing', async () => {
    const server = fakeServer()
    const s = stream(server)
    void s.panicked(ENGINE_PANIC_BLOCK)
    void s.crash('panic')
    await s.panicked(ENGINE_PANIC_BLOCK)
    expect(server.text('/__perf/change_build_1/log')).toBe(ENGINE_PANIC_BLOCK)
    expect(server.ends).toHaveLength(1)
  })
})

describe('a crash with no block of the engine\'s', () => {
  const taking = (server: ReturnType<typeof fakeServer>, take: () => unknown, csv = true) =>
    createPerfStream({ path: '/__perf/change_build_1', csv }, { fetch: server.fetch, wait: instant, now: () => NOW, origin: ORIGIN, take })

  it('sends what the engine still held after everything already appended, then the page\'s block, and ends with 101', async () => {
    const server = fakeServer()
    const take = vi.fn(() => ({ csv: 'queued rows\r\n', log: 'queued line\n' }))
    const s = taking(server, take)
    s.append('log', 'before\n')
    s.append('csv', 'sent rows\r\n')
    await s.crash('panic')
    expect(take).toHaveBeenCalledTimes(1)
    expect(server.text('/__perf/change_build_1/log')).toBe('before\nqueued line\n' + GOLDEN_CRASH_BLOCK)
    expect(server.text('/__perf/change_build_1/csv')).toBe('sent rows\r\nqueued rows\r\n')
    expect(server.endBodies).toEqual([
      { exitCode: 101, csvBytes: 'sent rows\r\nqueued rows\r\n'.length, logBytes: ('before\nqueued line\n' + GOLDEN_CRASH_BLOCK).length, failed: false }
    ])
  })

  it('takes what the engine held once, and not at all after the run ended', async () => {
    const server = fakeServer()
    const take = vi.fn(() => ({ csv: 'rows\r\n', log: 'line\n' }))
    const s = taking(server, take)
    void s.crash('gpu')
    await s.crash('watchdog')
    expect(take).toHaveBeenCalledTimes(1)
    expect(server.text('/__perf/change_build_1/log')).toBe('line\n' + crashBlock('gpu', NOW))
    expect(server.ends).toHaveLength(1)

    const done = fakeServer()
    const late = vi.fn(() => ({ csv: 'rows\r\n', log: 'line\n' }))
    const ended = taking(done, late)
    await ended.end(0)
    await ended.crash('panic')
    expect(late).not.toHaveBeenCalled()
  })

  it('may find nothing held: an empty answer, a refusal or a throw', async () => {
    const answers: Array<() => unknown> = [
      () => ({ csv: '', log: '' }),
      () => null,
      () => undefined,
      () => ({}),
      () => ({ csv: 7, log: ['x'] }),
      () => {
        throw new Error('unreachable')
      }
    ]
    for (const take of answers) {
      const server = fakeServer()
      await taking(server, take).crash('panic')
      expect(server.text('/__perf/change_build_1/log')).toBe(GOLDEN_CRASH_BLOCK)
      expect(server.text('/__perf/change_build_1/csv')).toBe('')
      expect(server.ends).toHaveLength(1)
    }
  })

  it('drops the CSV without perfCsv, as any CSV that arrives', async () => {
    const server = fakeServer()
    await taking(server, () => ({ csv: 'rows\r\n', log: 'line\n' }), false).crash('panic')
    expect(server.text('/__perf/change_build_1/csv')).toBe('')
    expect(server.text('/__perf/change_build_1/log')).toBe('line\n' + GOLDEN_CRASH_BLOCK)
  })
})

describe('the end report', () => {
  const report = (server: ReturnType<typeof fakeServer>) => server.endBodies[0]

  it('says what the server acknowledged on each stream, in bytes', async () => {
    const server = fakeServer()
    const s = stream(server)
    s.append('log', 'first\n')
    s.append('csv', '"Frame"\r\n')
    await new Promise((resolve) => setTimeout(resolve, 0))
    s.append('log', 'é\n') // two bytes in UTF-8
    await s.end(0)
    expect(server.endBodies).toEqual([{ exitCode: 0, csvBytes: 9, logBytes: 9, failed: false }])
    // the report matches the files the server holds
    expect(server.files['/__perf/change_build_1/log']).toHaveLength(report(server).logBytes as number)
    expect(server.files['/__perf/change_build_1/csv']).toHaveLength(report(server).csvBytes as number)
  })

  it('counts no CSV without perfCsv, and an empty stream as zero', async () => {
    const server = fakeServer()
    const s = stream(server, false)
    s.append('csv', 'rows\r\n')
    await s.end(2)
    expect(server.endBodies).toEqual([{ exitCode: 2, csvBytes: 0, logBytes: 0, failed: false }])
  })

  it('says a stream was given up, with only the bytes that got through', async () => {
    await quietly(async () => {
      const server = fakeServer()
      const s = stream(server)
      s.append('log', 'sent\n')
      await new Promise((resolve) => setTimeout(resolve, 0))
      server.failNext('/log', 1000)
      s.append('log', 'lost\n')
      s.append('csv', 'kept\r\n')
      await s.end(0)
      expect(server.endBodies).toEqual([{ exitCode: 0, csvBytes: 6, logBytes: 5, failed: true }])
      // the file is exactly what the report counts: the receiver can tell it is not the whole run
      expect(server.text('/__perf/change_build_1/log')).toBe('sent\n')
    })
  })

  it('says the CSV was given up too', async () => {
    await quietly(async () => {
      const server = fakeServer()
      const s = stream(server)
      server.failNext('/csv', 1000)
      s.append('csv', 'lost\r\n')
      s.append('log', 'kept\n')
      await s.end(0)
      expect(server.endBodies).toEqual([{ exitCode: 0, csvBytes: 0, logBytes: 5, failed: true }])
    })
  })

  it('is sent once, with the first exit code, and is the same on every retry', async () => {
    const server = fakeServer()
    server.failNext('/end', 2)
    const s = stream(server)
    s.append('log', 'line\n')
    void s.end(3)
    await s.end(0)
    expect(server.endBodies).toEqual([{ exitCode: 3, csvBytes: 0, logBytes: 5, failed: false }])
  })
})

describe('the stream path', () => {
  it('is a path on the page\'s own origin, as an absolute url without trailing slashes', () => {
    expect(streamBase('/__perf/change_build_1', ORIGIN)).toBe(`${ORIGIN}/__perf/change_build_1`)
    expect(streamBase('/__perf/b///', ORIGIN)).toBe(`${ORIGIN}/__perf/b`)
    // encoded slashes stay inside the path
    expect(streamBase('/%2Fevil.example', ORIGIN)).toBe(`${ORIGIN}/%2Fevil.example`)
    expect(streamBase('/%5C%5Cevil.example', ORIGIN)).toBe(`${ORIGIN}/%5C%5Cevil.example`)
    // dot segments fold into a path that merely starts with two slashes: still this host, and
    // returned absolute so those slashes are never read as another host
    for (const odd of ['/.//evil.example', '/a/..//evil.example', '/../..//evil.example']) {
      const base = streamBase(odd, ORIGIN)
      expect(base).toBe(`${ORIGIN}//evil.example`)
      expect(new URL(`${base}/log`).origin).toBe(ORIGIN)
    }
  })

  it('is refused when it resolves anywhere else, or is no path at all', () => {
    for (const elsewhere of [
      'https://evil.example/x',
      'http://play.example/x', // another scheme is another origin
      'https://play.example.evil.example/x',
      '//evil.example/x',
      '/\\evil.example',
      '/\\/evil.example',
      '\\\\evil.example/x',
      '\\evil.example',
      'perf/b',
      './perf/b',
      '',
      '/',
      '///',
      '/a?b=c',
      '/a#b',
      '/a/?',
      'javascript:alert(1)',
      'data:text/plain,x',
      'blob:https://play.example/00000000-0000-0000-0000-000000000000',
      'file:///etc/passwd'
    ]) {
      expect(streamBase(elsewhere, ORIGIN), elsewhere).toBeNull()
    }
    for (const notAPath of [undefined, null, 42, {}, ['/x']]) expect(streamBase(notAPath, ORIGIN)).toBeNull()
    // a page with no usable origin has nowhere to send to
    for (const origin of [undefined, 'null', '', 'not an origin']) expect(streamBase('/__perf/b', origin)).toBeNull()
  })

  it('starts no stream and sends nothing when refused, and says so', async () => {
    const server = fakeServer()
    const errors = vi.spyOn(console, 'error').mockImplementation(() => undefined)
    try {
      for (const path of ['https://evil.example/x', '//evil.example/x', '/\\evil.example', 'perf/b', '/a?b']) {
        expect(stream(server, true, path)).toBeNull()
      }
      expect(errors).toHaveBeenCalledTimes(5)
      // the message names no path: it is the page's embedder who chose it
      for (const [message] of errors.mock.calls) expect(message).not.toMatch(/evil|perf\/b/)
      await Promise.resolve()
      expect(server.calls).toEqual([])
    } finally {
      errors.mockRestore()
    }
  })

  it('is resolved against the page\'s origin when none is given', () => {
    const errors = vi.spyOn(console, 'error').mockImplementation(() => undefined)
    try {
      expect(createPerfStream({ path: '/__perf/b', csv: false }, { fetch: fakeServer().fetch, wait: instant })).not.toBeNull()
      expect(createPerfStream({ path: `${location.origin}/__perf/b`, csv: false }, { fetch: fakeServer().fetch, wait: instant })).toBeNull()
      expect(streamBase('/__perf/b', location.origin)).toBe(`${location.origin}/__perf/b`)
      expect(errors).toHaveBeenCalledTimes(1)
    } finally {
      errors.mockRestore()
    }
  })

  it('is where every request goes, with no credentials, no redirect and nothing from another origin', async () => {
    const server = fakeServer()
    const s = stream(server, true, '/.//evil.example/run/')
    s.append('log', 'line\n')
    s.append('csv', 'row\r\n')
    await s.end(0)
    // the log, the csv and the end, each to this page's origin under the folded path
    expect(server.calls).toHaveLength(3)
    for (const call of server.calls) {
      expect(call.origin).toBe(ORIGIN)
      expect(call.init).toMatchObject({ method: 'POST', credentials: 'omit', redirect: 'error', mode: 'same-origin' })
    }
    expect(server.requests.map((r) => r.url)).toEqual(['//evil.example/run/log', '//evil.example/run/csv'])
    expect(server.ends).toEqual([{ exitCode: 0, after: 2 }])
  })

  it('gives every retry the same options', async () => {
    const server = fakeServer()
    server.failNext('/log', 2)
    server.failNext('/end', 1)
    const s = stream(server)
    s.append('log', 'hello\n')
    await s.end(0)
    expect(server.calls.length).toBeGreaterThanOrEqual(5)
    for (const call of server.calls) {
      expect(call.origin).toBe(ORIGIN)
      expect(call.init).toMatchObject({ credentials: 'omit', redirect: 'error', mode: 'same-origin' })
    }
  })

  it('treats a redirect, which `redirect: error` turns into a failed fetch, like any dropped request', async () => {
    const server = fakeServer()
    server.failNext('/log', 1000)
    const errors = vi.spyOn(console, 'error').mockImplementation(() => undefined)
    try {
      const s = stream(server, false)
      s.append('log', 'line\n')
      await s.end(5)
      // gave up on the log after its retries, and the run still ended
      expect(server.text('/__perf/change_build_1/log')).toBe('')
      expect(server.ends).toEqual([{ exitCode: 5, after: 0 }])
      expect(errors.mock.calls.some(([message]) => String(message).includes('gave up sending'))).toBe(true)
    } finally {
      errors.mockRestore()
    }
  })
})
