import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { EnvTokenProvider, FigmaApiError, FigmaClient, type ClientOptions } from './client.js'

describe('EnvTokenProvider', () => {
  describe('when FIGMA_TOKEN is set', () => {
    it('should return the token', async () => {
      const provider = new EnvTokenProvider({ FIGMA_TOKEN: 'figd_example' })

      await expect(provider.getToken()).resolves.toBe('figd_example')
    })
  })

  describe('when FIGMA_TOKEN is the placeholder from .env.example', () => {
    it('should reject', async () => {
      const provider = new EnvTokenProvider({ FIGMA_TOKEN: '<FIGMA_TOKEN>' })

      await expect(provider.getToken()).rejects.toThrow('FIGMA_TOKEN is not set')
    })
  })
})

describe('FigmaClient', () => {
  describe('when the response is rate limited once', () => {
    it('should retry and return the second response', async () => {
      const responses = [
        new Response('', { status: 429, headers: { 'retry-after': '0' } }),
        new Response('{"ok":true}', { status: 200 })
      ]
      const fetchFn = (async () => responses.shift()!) as unknown as typeof fetch
      const client = new FigmaClient({ getToken: async () => 'token' }, fetchFn)

      await expect(client.get('/v1/x')).resolves.toEqual({ ok: true })
    })
  })

  describe('when the API answers with an error status', () => {
    it('should throw a FigmaApiError carrying the status', async () => {
      const fetchFn = (async () => new Response('nope', { status: 403 })) as unknown as typeof fetch
      const client = new FigmaClient({ getToken: async () => 'token' }, fetchFn)

      await expect(client.get('/v1/x')).rejects.toMatchObject({ status: 403 })
      await expect(client.get('/v1/x')).rejects.toBeInstanceOf(FigmaApiError)
    })
  })

  describe('when the failing path contains the configured file key', () => {
    afterEach(() => vi.unstubAllEnvs())

    it('should never put the key in the error message', async () => {
      vi.stubEnv('FIGMA_FILE_KEY', 'FAKEKEY000example000000')
      const fetchFn = (async () =>
        new Response('file FAKEKEY000example000000 not found', {
          status: 404
        })) as unknown as typeof fetch
      const client = new FigmaClient({ getToken: async () => 'token' }, fetchFn)

      const error = (await client
        .get('/v1/files/FAKEKEY000example000000?depth=1')
        .catch((e: unknown) => e)) as Error

      expect(error.message).not.toContain('FAKEKEY000example000000')
      expect(error.message).toContain('/v1/files/<DCL_FIGMA_FILE>?depth=1')
    })

    it('should keep the key out of the retry diagnostics and the error after the retries run out', async () => {
      vi.stubEnv('FIGMA_FILE_KEY', 'FAKEKEY000example000000')
      const { client, lines } = harness([timeout(), timeout(), timeout(), timeout()])

      const error = (await client
        .get('/v1/files/FAKEKEY000example000000/nodes?ids=1')
        .catch((e: unknown) => e)) as Error

      expect(error.message).toContain('after 4 attempts on /v1/files/<DCL_FIGMA_FILE>/nodes')
      expect(error.message).not.toContain('FAKEKEY000example000000')
      expect(lines).toHaveLength(4)
      expect(lines.join('\n')).not.toContain('FAKEKEY000example000000')
    })

    it('should keep the key out of a non-retryable fetch error that quotes the url', async () => {
      vi.stubEnv('FIGMA_FILE_KEY', 'FAKEKEY000example000000')
      const { client, lines } = harness([
        new Error('Failed to parse URL from https://api.figma.com/v1/files/FAKEKEY000example000000')
      ])

      const error = (await client
        .get('/v1/files/FAKEKEY000example000000')
        .catch((e: unknown) => e)) as Error

      expect(error.message).toBe(
        'Failed to parse URL from https://api.figma.com/v1/files/<DCL_FIGMA_FILE>'
      )
      expect(lines.join('\n')).not.toContain('FAKEKEY000example000000')
    })
  })
})

describe.each(['3600', '-1', 'invalid'])('when Retry-After is %s', (retryAfter) => {
  let client: FigmaClient
  beforeEach(() => {
    client = new FigmaClient(
      { getToken: async () => 'token' },
      async () => new Response('', { status: 429, headers: { 'retry-after': retryAfter } })
    )
  })
  it('should fail with a rate-limit error instead of waiting indefinitely or retrying immediately', async () => {
    await expect(client.get('/v1/x')).rejects.toMatchObject({ status: 429 })
  })
})

type Step = Response | Error | ((signal: AbortSignal) => Promise<Response>)

function harness(steps: Step[], options: ClientOptions = {}) {
  const delays: number[] = []
  const lines: string[] = []
  const calls: { url: string; init: RequestInit }[] = []
  const fetchFn = (async (url: string | URL, init: RequestInit) => {
    calls.push({ url: String(url), init })
    const step = steps.shift()
    if (!step) throw new Error('unexpected request')
    if (step instanceof Error) throw step
    if (typeof step === 'function') return step(init.signal!)
    return step
  }) as unknown as typeof fetch
  const client = new FigmaClient({ getToken: async () => 'figd_secret_token' }, fetchFn, {
    sleep: async (ms) => {
      delays.push(ms)
    },
    log: (line) => lines.push(line),
    ...options
  })
  return { client, delays, lines, calls }
}

const timeout = () => new DOMException('The operation was aborted due to timeout', 'TimeoutError')
const ok = () => new Response('{"ok":true}', { status: 200 })
// A 200 whose body stream breaks halfway, as undici reports a dropped connection.
const brokenBody = () =>
  new Response(
    new ReadableStream({
      pull(controller) {
        controller.error(new TypeError('terminated'))
      }
    }),
    { status: 200 }
  )

describe('FigmaClient transient failures', () => {
  describe('when the first attempt times out', () => {
    it('should retry after 2s and return the next response', async () => {
      const { client, delays, calls } = harness([timeout(), ok()])

      await expect(client.get('/v1/x')).resolves.toEqual({ ok: true })
      expect(delays).toEqual([2000])
      expect(calls).toHaveLength(2)
    })
  })

  describe('when the first attempt answers 503', () => {
    it('should retry and return the next response', async () => {
      const { client, delays } = harness([new Response('busy', { status: 503 }), ok()])

      await expect(client.get('/v1/x')).resolves.toEqual({ ok: true })
      expect(delays).toEqual([2000])
    })
  })

  describe('when the connection drops while the body streams', () => {
    it('should retry the whole request', async () => {
      const { client, delays } = harness([brokenBody(), ok()])

      await expect(client.get('/v1/x')).resolves.toEqual({ ok: true })
      expect(delays).toEqual([2000])
    })
  })

  describe('when a network error repeats on every attempt', () => {
    it('should give up after 4 attempts with the path and the last cause', async () => {
      const { client, delays, calls } = harness([
        timeout(),
        timeout(),
        timeout(),
        new TypeError('fetch failed')
      ])

      const error = await client.get<never>('/v1/x').catch((caught: unknown) => caught as Error)
      expect(error.message).toBe(
        'Figma request failed after 4 attempts on /v1/x: TypeError: fetch failed'
      )
      expect(error.cause).toBeInstanceOf(TypeError)
      expect(calls).toHaveLength(4)
      expect(delays).toEqual([2000, 4000, 8000])
    })
  })

  describe('when an authenticated request exhausts its retries', () => {
    it('should never carry the token in diagnostics, the error or its causes', async () => {
      const { client, lines } = harness([1, 2, 3, 4].map(() => new TypeError('fetch failed')))

      const error = await client.get<never>('/v1/x').catch((caught: unknown) => caught as Error)
      const chain: string[] = []
      for (let current: unknown = error; current instanceof Error; current = current.cause)
        chain.push(current.message, String(current.stack))
      expect([...lines, ...chain].join('\n')).not.toContain('figd_secret_token')
    })
  })

  describe('when a 5xx repeats on every attempt', () => {
    it('should throw a FigmaApiError that says it gave up', async () => {
      const { client, calls } = harness(
        [1, 2, 3, 4].map(() => new Response('bad gateway', { status: 502 }))
      )

      await expect(client.get('/v1/x')).rejects.toMatchObject({
        status: 502,
        message: 'Figma API 502 on /v1/x: bad gateway; gave up after 4 attempts'
      })
      expect(calls).toHaveLength(4)
    })
  })

  describe.each([400, 401, 403, 404])('when the API answers %i', (status) => {
    it('should fail at once without retrying', async () => {
      const { client, delays, calls } = harness([new Response('no', { status })])

      await expect(client.get('/v1/x')).rejects.toBeInstanceOf(FigmaApiError)
      expect(calls).toHaveLength(1)
      expect(delays).toEqual([])
    })
  })

  describe('when a 4xx body cannot be read', () => {
    it('should still fail at once with the status', async () => {
      const broken = brokenBody()
      Object.defineProperty(broken, 'status', { value: 404 })
      Object.defineProperty(broken, 'ok', { value: false })
      const { client, calls } = harness([broken])

      await expect(client.get('/v1/x')).rejects.toMatchObject({ status: 404 })
      expect(calls).toHaveLength(1)
    })
  })

  describe('when a 200 body is not JSON', () => {
    it('should fail at once', async () => {
      const { client, calls } = harness([new Response('<html>', { status: 200 })])

      await expect(client.get('/v1/x')).rejects.toBeInstanceOf(SyntaxError)
      expect(calls).toHaveLength(1)
    })
  })

  describe('when a rate limit and a 5xx share the retry budget', () => {
    it('should wait Retry-After, then back off by the overall retry index', async () => {
      const { client, delays } = harness([
        new Response('', { status: 429, headers: { 'retry-after': '5' } }),
        new Response('', { status: 503 }),
        ok()
      ])

      await expect(client.get('/v1/x')).resolves.toEqual({ ok: true })
      expect(delays).toEqual([5000, 4000])
    })

    it('should stop after 4 attempts in total', async () => {
      const { client, calls } = harness([
        new Response('', { status: 429, headers: { 'retry-after': '0' } }),
        timeout(),
        new Response('', { status: 429, headers: { 'retry-after': '0' } }),
        new Response('slow down', { status: 429, headers: { 'retry-after': '0' } })
      ])

      await expect(client.get('/v1/x')).rejects.toMatchObject({ status: 429 })
      expect(calls).toHaveLength(4)
    })
  })

  describe('when attempts are logged', () => {
    it('should name the path, status, time and bytes, never the token or the body', async () => {
      let now = 0
      const { client, lines } = harness([new Response('secret body', { status: 503 }), ok()], {
        now: () => (now += 25)
      })

      await client.get('/v1/files/key?depth=2')
      expect(lines).toEqual([
        '[figma] GET /v1/files/key?depth=2 attempt 1/4 status 503 25ms',
        '[figma] GET /v1/files/key?depth=2 attempt 2/4 status 200 11B 25ms'
      ])
      expect(lines.join('\n')).not.toMatch(/figd_secret_token|secret body/)
    })

    it('should log a failed attempt with its cause', async () => {
      const { client, lines } = harness([timeout(), ok()])

      await client.get('/v1/x')
      expect(lines[0]).toMatch(
        /^\[figma\] GET \/v1\/x attempt 1\/4 failed: TimeoutError: The operation was aborted due to timeout \d+ms$/
      )
    })
  })

  describe('when a call sets its own timeout', () => {
    // Resolves after `ms` unless the request signal aborts first.
    const slow = (ms: number) => (signal: AbortSignal) =>
      new Promise<Response>((resolve, reject) => {
        const timer = setTimeout(() => resolve(ok()), ms)
        signal.addEventListener('abort', () => {
          clearTimeout(timer)
          reject(signal.reason)
        })
      })

    it('should let a slow response finish within the longer timeout', async () => {
      const { client } = harness([slow(120)], { timeoutMs: 60 })

      await expect(client.get('/v1/x', { timeoutMs: 180 })).resolves.toEqual({ ok: true })
    })

    it('should abort the same response under the default timeout', async () => {
      const { client, lines } = harness([slow(120), slow(120), slow(120), slow(120)], {
        timeoutMs: 60
      })

      await expect(client.get('/v1/x')).rejects.toThrow(
        'Figma request failed after 4 attempts on /v1/x: TimeoutError'
      )
      expect(lines).toHaveLength(4)
    })
  })
})

describe('FigmaClient.download', () => {
  const url = new URL('https://s3.amazonaws.com/export.svg?X-Amz-Signature=signed')

  it('should retry a timeout, send no token and log no signed query', async () => {
    const { client, calls, lines } = harness([timeout(), new Response('<svg/>')])

    await expect(client.download(url, '1:2', 1024)).resolves.toEqual(Buffer.from('<svg/>'))
    expect(calls).toHaveLength(2)
    expect(calls[1]!.init.headers).toBeUndefined()
    expect(calls[1]!.init.redirect).toBe('error')
    expect(lines.join('\n')).not.toContain('X-Amz-Signature')
  })

  describe('when fetch errors embed the signed URL', () => {
    const signed =
      'https://s3.amazonaws.com/exports/secret-path-token/export.svg?X-Amz-Signature=sig123'
    const leaky = () => {
      const socket = Object.assign(new Error(`other side closed ${signed}`), {
        code: 'UND_ERR_SOCKET'
      })
      return new TypeError(`fetch failed for ${signed}`, { cause: socket })
    }
    // Every text a thrown error carries: messages and names along its cause chain, without stacks.
    const texts = (error: unknown): string[] => {
      const out: string[] = []
      for (let current = error; current instanceof Error; current = current.cause)
        out.push(current.name, current.message)
      return out
    }
    const leaks = (text: string) => /X-Amz-Signature|sig123|secret-path-token|exports\//.test(text)

    it('should keep the URL out of diagnostics and the exhausted error', async () => {
      const { client, lines } = harness([leaky(), leaky(), leaky(), leaky()])

      const error = await client.download(url, '1:2', 1024).catch((caught: unknown) => caught)
      expect(error).toBeInstanceOf(Error)
      expect((error as Error).message).toBe(
        'Figma request failed after 4 attempts on s3.amazonaws.com 1:2: TypeError Error UND_ERR_SOCKET'
      )
      expect((error as Error).cause).toBeUndefined()
      expect([...lines, ...texts(error)].filter(leaks)).toEqual([])
      expect(lines[0]).toMatch(/attempt 1\/4 failed: TypeError Error UND_ERR_SOCKET \d+ms$/)
    })

    it('should render unknown error names as Error and drop oversized codes', async () => {
      const named = () => {
        const inner = Object.assign(new Error('closed'), { name: signed, code: 'X'.repeat(65) })
        return Object.assign(new TypeError('fetch failed', { cause: inner }), { name: signed })
      }
      const { client, lines } = harness([named(), named(), named(), named()])

      const error = await client.download(url, '1:2', 1024).catch((caught: unknown) => caught)
      expect((error as Error).message).toBe(
        'Figma request failed after 4 attempts on s3.amazonaws.com 1:2: Error'
      )
      expect([...lines, ...texts(error)].filter(leaks)).toEqual([])
    })

    it('should render an unknown name as Error in a non-retryable error', async () => {
      const fatal = Object.assign(new RangeError('bad'), {
        name: signed,
        cause: Object.assign(new Error('x'), { name: signed })
      })
      const { client, lines } = harness([fatal])

      const error = await client.download(url, '1:2', 1024).catch((caught: unknown) => caught)
      expect((error as Error).message).toBe('Request failed on s3.amazonaws.com 1:2: Error')
      expect([...lines, ...texts(error)].filter(leaks)).toEqual([])
    })

    it('should keep the URL out of a non-retryable error', async () => {
      const fatal = new RangeError(`bad request to ${signed}`)
      const { client, lines, calls } = harness([fatal])

      const error = await client.download(url, '1:2', 1024).catch((caught: unknown) => caught)
      expect((error as Error).message).toBe('Request failed on s3.amazonaws.com 1:2: RangeError')
      expect([...lines, ...texts(error)].filter(leaks)).toEqual([])
      expect(calls).toHaveLength(1)
    })
  })

  it('should not retry an expired signed URL', async () => {
    const { client, calls } = harness([new Response('', { status: 403 })])

    await expect(client.download(url, '1:2', 1024)).rejects.toThrow(
      'Export download failed for 1:2: 403'
    )
    expect(calls).toHaveLength(1)
  })

  it('should not retry an export over the size limit', async () => {
    const { client, calls } = harness([new Response('x'.repeat(2048))])

    await expect(client.download(url, '1:2', 1024)).rejects.toThrow('Export exceeds')
    expect(calls).toHaveLength(1)
  })
})

describe('when a call sets no timeout', () => {
  afterEach(() => vi.restoreAllMocks())

  it('should give each attempt 60s', async () => {
    const timeouts = vi.spyOn(AbortSignal, 'timeout')
    const client = new FigmaClient(
      { getToken: async () => 'token' },
      (async () => new Response('{}')) as unknown as typeof fetch,
      { log: () => {} }
    )

    await client.get('/v1/files/key/variables/local')
    expect(timeouts.mock.calls).toEqual([[60000]])
  })
})

describe('when an attempt fails', () => {
  it('should write its diagnostic line before the backoff and the next attempt start', async () => {
    const events: string[] = []
    const answers = [new Response('', { status: 503 }), new Response('{}')]
    const client = new FigmaClient(
      { getToken: async () => 'token' },
      (async () => {
        events.push('fetch')
        return answers.shift()!
      }) as unknown as typeof fetch,
      {
        sleep: async () => {
          events.push('sleep')
        },
        log: (line) => events.push(line.includes('status 503') ? 'log 503' : 'log 200')
      }
    )

    await client.get('/v1/x')
    expect(events).toEqual(['fetch', 'log 503', 'sleep', 'fetch', 'log 200'])
  })
})

function oversizedError(status: number, chunkBytes = 64 * 1024) {
  let bytes = 0
  let canceled = false
  const response = new Response(
    new ReadableStream<Uint8Array>({
      pull(controller) {
        if (bytes >= 20 * 1024 * 1024) return controller.close()
        const chunk = new Uint8Array(chunkBytes).fill('x'.charCodeAt(0))
        bytes += chunk.length
        controller.enqueue(chunk)
      },
      cancel() {
        canceled = true
      }
    }),
    { status, headers: { 'retry-after': '0' } }
  )
  return { response, readBytes: () => bytes, canceled: () => canceled }
}

describe.each([404, 429, 503])('when an oversized HTTP %i error body streams', (status) => {
  it('should bound and cancel the API error body while preserving status and retries', async () => {
    const body = oversizedError(status)
    const { client, calls, delays } = harness([body.response, ok()])

    if (status === 404) {
      await expect(client.get('/v1/x')).rejects.toMatchObject({
        status,
        message: `Figma API 404 on /v1/x: ${'x'.repeat(200)}`
      })
    } else await expect(client.get('/v1/x')).resolves.toEqual({ ok: true })

    expect(body.canceled()).toBe(true)
    expect(body.readBytes()).toBeLessThan(20 * 1024 * 1024)
    expect(body.response.body?.locked).toBe(false)
    expect(calls).toHaveLength(status === 404 ? 1 : 2)
    expect(delays).toEqual(status === 404 ? [] : [status === 429 ? 0 : 2000])
  })

  it('should bound and cancel export error bodies while preserving status and retries', async () => {
    const body = oversizedError(status)
    const { client, calls, delays } = harness([body.response, new Response('<svg/>')])
    const downloaded = client.download(
      new URL('https://images.figma.com/export'),
      'icon',
      16 * 1024 * 1024
    )

    if (status === 404)
      await expect(downloaded).rejects.toThrow('Export download failed for icon: 404')
    else await expect(downloaded).resolves.toEqual(Buffer.from('<svg/>'))

    expect(body.canceled()).toBe(true)
    expect(body.readBytes()).toBeLessThan(20 * 1024 * 1024)
    expect(body.response.body?.locked).toBe(false)
    expect(calls).toHaveLength(status === 404 ? 1 : 2)
    expect(delays).toEqual(status === 404 ? [] : [status === 429 ? 0 : 2000])
  })
})

describe('when an error stream reaches the excerpt budget exactly', () => {
  it('should stop reading at the budget instead of waiting for the next chunk', async () => {
    const body = oversizedError(404, 1024)
    const { client } = harness([body.response])

    await expect(client.get('/v1/x')).rejects.toMatchObject({ status: 404 })
    expect(body.canceled()).toBe(true)
    expect(body.readBytes()).toBeLessThanOrEqual(2048)
  })
})
