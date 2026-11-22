import { afterEach, beforeEach, describe, expect, it, vi, type MockInstance } from 'vitest'
import { EnvTokenProvider, FigmaApiError, FigmaClient, type ClientOptions } from './client.js'

describe('EnvTokenProvider', () => {
  let provider: EnvTokenProvider

  describe('when FIGMA_TOKEN is set', () => {
    beforeEach(() => {
      provider = new EnvTokenProvider({ FIGMA_TOKEN: 'figd_example' })
    })

    it('should return the token', async () => {
      await expect(provider.getToken()).resolves.toBe('figd_example')
    })
  })

  describe('when FIGMA_TOKEN is the placeholder', () => {
    beforeEach(() => {
      provider = new EnvTokenProvider({ FIGMA_TOKEN: '<FIGMA_TOKEN>' })
    })

    it('should reject', async () => {
      await expect(provider.getToken()).rejects.toThrow('FIGMA_TOKEN is not set')
    })
  })

  describe('when FIGMA_TOKEN is missing', () => {
    beforeEach(() => {
      provider = new EnvTokenProvider({})
    })

    it('should reject instead of reading a token from any file', async () => {
      await expect(provider.getToken()).rejects.toThrow('FIGMA_TOKEN is not set')
    })
  })
})

describe('FigmaClient', () => {
  let client: FigmaClient

  describe('when the response is rate limited once', () => {
    beforeEach(() => {
      const responses = [
        new Response('', { status: 429, headers: { 'retry-after': '0' } }),
        new Response('{"ok":true}', { status: 200 })
      ]
      const fetchFn = (async () => responses.shift()!) as unknown as typeof fetch
      client = new FigmaClient({ getToken: async () => 'token' }, fetchFn)
    })

    it('should retry and return the second response', async () => {
      await expect(client.get('/v1/x')).resolves.toEqual({ ok: true })
    })
  })

  describe('when the API answers with an error status', () => {
    beforeEach(() => {
      const fetchFn = (async () => new Response('nope', { status: 403 })) as unknown as typeof fetch
      client = new FigmaClient({ getToken: async () => 'token' }, fetchFn)
    })

    it('should throw a FigmaApiError carrying the status', async () => {
      await expect(client.get('/v1/x')).rejects.toMatchObject({ status: 403 })
      await expect(client.get('/v1/x')).rejects.toBeInstanceOf(FigmaApiError)
    })

    it('should name the status, path and body in the error message', async () => {
      await expect(client.get('/v1/x')).rejects.toThrow('Figma API 403 on /v1/x: nope')
    })
  })

  describe('when the failing path contains the configured file key', () => {
    let error: Error
    let lines: string[]

    beforeEach(() => {
      vi.stubEnv('FIGMA_FILE_KEY', 'FAKEKEY000example000000')
    })

    afterEach(() => vi.unstubAllEnvs())

    describe('and the API answers 404', () => {
      beforeEach(async () => {
        const fetchFn = (async () =>
          new Response('file FAKEKEY000example000000 not found', {
            status: 404
          })) as unknown as typeof fetch
        client = new FigmaClient({ getToken: async () => 'token' }, fetchFn)

        error = (await client
          .get('/v1/files/FAKEKEY000example000000?depth=1')
          .catch((e: unknown) => e)) as Error
      })

      it('should never put the key in the error message', () => {
        expect(error.message).not.toContain('FAKEKEY000example000000')
      })

      it('should put the redacted path in the error message', () => {
        expect(error.message).toContain('/v1/files/<DCL_FIGMA_FILE>?depth=1')
      })
    })

    describe('and the retries run out', () => {
      beforeEach(async () => {
        const run = harness([timeout(), timeout(), timeout(), timeout()])
        lines = run.lines

        error = (await run.client
          .get('/v1/files/FAKEKEY000example000000/nodes?ids=1')
          .catch((e: unknown) => e)) as Error
      })

      it('should name the attempts and the redacted path in the error', () => {
        expect(error.message).toContain('after 4 attempts on /v1/files/<DCL_FIGMA_FILE>/nodes')
      })

      it('should keep the key out of the error', () => {
        expect(error.message).not.toContain('FAKEKEY000example000000')
      })

      it('should log one diagnostic per attempt', () => {
        expect(lines).toHaveLength(4)
      })

      it('should keep the key out of the retry diagnostics', () => {
        expect(lines.join('\n')).not.toContain('FAKEKEY000example000000')
      })
    })

    describe('and a non-retryable fetch error quotes the url', () => {
      beforeEach(async () => {
        const run = harness([
          new Error(
            'Failed to parse URL from https://api.figma.com/v1/files/FAKEKEY000example000000'
          )
        ])
        lines = run.lines

        error = (await run.client
          .get('/v1/files/FAKEKEY000example000000')
          .catch((e: unknown) => e)) as Error
      })

      it('should redact the key in the error', () => {
        expect(error.message).toBe(
          'Failed to parse URL from https://api.figma.com/v1/files/<DCL_FIGMA_FILE>'
        )
      })

      it('should keep the key out of the diagnostics', () => {
        expect(lines.join('\n')).not.toContain('FAKEKEY000example000000')
      })
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

  it('should say the rate limit requires a later run', async () => {
    await expect(client.get('/v1/x')).rejects.toThrow(
      'Figma API 429 on /v1/x: Rate limit requires a later run; no immediate retries were made.'
    )
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
  let run: ReturnType<typeof harness>

  describe('when the first attempt times out', () => {
    beforeEach(() => {
      run = harness([timeout(), ok()])
    })

    it('should retry after 2s and return the next response', async () => {
      await expect(run.client.get('/v1/x')).resolves.toEqual({ ok: true })
      expect(run.delays).toEqual([2000])
      expect(run.calls).toHaveLength(2)
    })
  })

  describe('when the first attempt answers 503', () => {
    beforeEach(() => {
      run = harness([new Response('busy', { status: 503 }), ok()])
    })

    it('should retry and return the next response', async () => {
      await expect(run.client.get('/v1/x')).resolves.toEqual({ ok: true })
      expect(run.delays).toEqual([2000])
    })
  })

  describe('when the connection drops while the body streams', () => {
    beforeEach(() => {
      run = harness([brokenBody(), ok()])
    })

    it('should retry the whole request', async () => {
      await expect(run.client.get('/v1/x')).resolves.toEqual({ ok: true })
      expect(run.delays).toEqual([2000])
    })
  })

  describe('when a network error repeats on every attempt', () => {
    let error: Error

    beforeEach(async () => {
      run = harness([timeout(), timeout(), timeout(), new TypeError('fetch failed')])

      error = await run.client.get<never>('/v1/x').catch((caught: unknown) => caught as Error)
    })

    it('should give up with the path and the last cause in the message', () => {
      expect(error.message).toBe(
        'Figma request failed after 4 attempts on /v1/x: TypeError: fetch failed'
      )
    })

    it('should keep the last cause', () => {
      expect(error.cause).toBeInstanceOf(TypeError)
    })

    it('should give up after 4 attempts', () => {
      expect(run.calls).toHaveLength(4)
    })

    it('should back off exponentially between attempts', () => {
      expect(run.delays).toEqual([2000, 4000, 8000])
    })
  })

  describe('when an authenticated request exhausts its retries', () => {
    let chain: string[]

    beforeEach(async () => {
      run = harness([1, 2, 3, 4].map(() => new TypeError('fetch failed')))

      const error = await run.client.get<never>('/v1/x').catch((caught: unknown) => caught as Error)
      chain = []
      for (let current: unknown = error; current instanceof Error; current = current.cause)
        chain.push(current.message, String(current.stack))
    })

    it('should never carry the token in diagnostics, the error or its causes', () => {
      expect([...run.lines, ...chain].join('\n')).not.toContain('figd_secret_token')
    })
  })

  describe('when a 5xx repeats on every attempt', () => {
    beforeEach(() => {
      run = harness([1, 2, 3, 4].map(() => new Response('bad gateway', { status: 502 })))
    })

    it('should throw a FigmaApiError that says it gave up', async () => {
      await expect(run.client.get('/v1/x')).rejects.toMatchObject({
        status: 502,
        message: 'Figma API 502 on /v1/x: bad gateway; gave up after 4 attempts'
      })
      expect(run.calls).toHaveLength(4)
    })
  })

  describe.each([400, 401, 403, 404])('when the API answers %i', (status) => {
    beforeEach(() => {
      run = harness([new Response('no', { status })])
    })

    it('should fail at once without retrying', async () => {
      await expect(run.client.get('/v1/x')).rejects.toBeInstanceOf(FigmaApiError)
      expect(run.calls).toHaveLength(1)
      expect(run.delays).toEqual([])
    })

    it('should name the status and body in the error message', async () => {
      await expect(run.client.get('/v1/x')).rejects.toThrow(`Figma API ${status} on /v1/x: no`)
    })
  })

  describe('when a 4xx body cannot be read', () => {
    beforeEach(() => {
      const broken = brokenBody()
      Object.defineProperty(broken, 'status', { value: 404 })
      Object.defineProperty(broken, 'ok', { value: false })
      run = harness([broken])
    })

    it('should still fail at once with the status', async () => {
      await expect(run.client.get('/v1/x')).rejects.toMatchObject({ status: 404 })
      expect(run.calls).toHaveLength(1)
    })

    it('should name the status and path in the error message', async () => {
      await expect(run.client.get('/v1/x')).rejects.toThrow(/^Figma API 404 on \/v1\/x: /)
    })
  })

  describe('when a 200 body is not JSON', () => {
    beforeEach(() => {
      run = harness([new Response('<html>', { status: 200 })])
    })

    it('should fail at once', async () => {
      await expect(run.client.get('/v1/x')).rejects.toBeInstanceOf(SyntaxError)
      expect(run.calls).toHaveLength(1)
    })

    it('should surface the JSON parse error message', async () => {
      await expect(run.client.get('/v1/x')).rejects.toThrow(/is not valid JSON/)
    })
  })

  describe('when a rate limit and a 5xx share the retry budget', () => {
    describe('and the next attempt succeeds', () => {
      beforeEach(() => {
        run = harness([
          new Response('', { status: 429, headers: { 'retry-after': '5' } }),
          new Response('', { status: 503 }),
          ok()
        ])
      })

      it('should wait Retry-After, then back off by the overall retry index', async () => {
        await expect(run.client.get('/v1/x')).resolves.toEqual({ ok: true })
        expect(run.delays).toEqual([5000, 4000])
      })
    })

    describe('and every attempt fails', () => {
      beforeEach(() => {
        run = harness([
          new Response('', { status: 429, headers: { 'retry-after': '0' } }),
          timeout(),
          new Response('', { status: 429, headers: { 'retry-after': '0' } }),
          new Response('slow down', { status: 429, headers: { 'retry-after': '0' } })
        ])
      })

      it('should stop after 4 attempts in total', async () => {
        await expect(run.client.get('/v1/x')).rejects.toMatchObject({ status: 429 })
        expect(run.calls).toHaveLength(4)
      })

      it('should report the last rate-limit body in the error message', async () => {
        await expect(run.client.get('/v1/x')).rejects.toThrow('Figma API 429 on /v1/x: slow down')
      })
    })
  })

  describe('when attempts are logged', () => {
    describe('and an attempt answers 503 before a success', () => {
      beforeEach(async () => {
        let now = 0
        run = harness([new Response('secret body', { status: 503 }), ok()], {
          now: () => (now += 25)
        })

        await run.client.get('/v1/files/key?depth=2')
      })

      it('should name the path, status, time and bytes', () => {
        expect(run.lines).toEqual([
          '[figma] GET /v1/files/key?depth=2 attempt 1/4 status 503 25ms',
          '[figma] GET /v1/files/key?depth=2 attempt 2/4 status 200 11B 25ms'
        ])
      })

      it('should never log the token or the body', () => {
        expect(run.lines.join('\n')).not.toMatch(/figd_secret_token|secret body/)
      })
    })

    describe('and an attempt fails', () => {
      beforeEach(async () => {
        run = harness([timeout(), ok()])

        await run.client.get('/v1/x')
      })

      it('should log the failed attempt with its cause', () => {
        expect(run.lines[0]).toMatch(
          /^\[figma\] GET \/v1\/x attempt 1\/4 failed: TimeoutError: The operation was aborted due to timeout \d+ms$/
        )
      })
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

    describe('and the timeout is longer than the response', () => {
      beforeEach(() => {
        run = harness([slow(120)], { timeoutMs: 60 })
      })

      it('should let the slow response finish', async () => {
        await expect(run.client.get('/v1/x', { timeoutMs: 180 })).resolves.toEqual({ ok: true })
      })
    })

    describe('and the call keeps the shorter default timeout', () => {
      beforeEach(() => {
        run = harness([slow(120), slow(120), slow(120), slow(120)], {
          timeoutMs: 60
        })
      })

      it('should abort the same response', async () => {
        await expect(run.client.get('/v1/x')).rejects.toThrow(
          'Figma request failed after 4 attempts on /v1/x: TimeoutError'
        )
        expect(run.lines).toHaveLength(4)
      })
    })
  })
})

describe('FigmaClient.download', () => {
  const url = new URL('https://s3.amazonaws.com/export.svg?X-Amz-Signature=signed')
  let run: ReturnType<typeof harness>

  describe('when the first attempt times out', () => {
    beforeEach(() => {
      run = harness([timeout(), new Response('<svg/>')])
    })

    it('should retry a timeout, send no token and log no signed query', async () => {
      await expect(run.client.download(url, '1:2', 1024)).resolves.toEqual(Buffer.from('<svg/>'))
      expect(run.calls).toHaveLength(2)
      expect(run.calls[1]!.init.headers).toBeUndefined()
      expect(run.calls[1]!.init.redirect).toBe('error')
      expect(run.lines.join('\n')).not.toContain('X-Amz-Signature')
    })
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
    let error: unknown

    describe('and the retries run out', () => {
      beforeEach(async () => {
        run = harness([leaky(), leaky(), leaky(), leaky()])

        error = await run.client.download(url, '1:2', 1024).catch((caught: unknown) => caught)
      })

      it('should keep the URL out of diagnostics and the exhausted error', () => {
        expect(error).toBeInstanceOf(Error)
        expect((error as Error).message).toBe(
          'Figma request failed after 4 attempts on s3.amazonaws.com 1:2: TypeError Error UND_ERR_SOCKET'
        )
        expect((error as Error).cause).toBeUndefined()
        expect([...run.lines, ...texts(error)].filter(leaks)).toEqual([])
        expect(run.lines[0]).toMatch(/attempt 1\/4 failed: TypeError Error UND_ERR_SOCKET \d+ms$/)
      })
    })

    describe('and the errors carry unknown names and oversized codes', () => {
      beforeEach(async () => {
        const named = () => {
          const inner = Object.assign(new Error('closed'), { name: signed, code: 'X'.repeat(65) })
          return Object.assign(new TypeError('fetch failed', { cause: inner }), { name: signed })
        }
        run = harness([named(), named(), named(), named()])

        error = await run.client.download(url, '1:2', 1024).catch((caught: unknown) => caught)
      })

      it('should render unknown error names as Error and drop oversized codes', () => {
        expect((error as Error).message).toBe(
          'Figma request failed after 4 attempts on s3.amazonaws.com 1:2: Error'
        )
      })

      it('should keep the URL out of diagnostics and the error', () => {
        expect([...run.lines, ...texts(error)].filter(leaks)).toEqual([])
      })
    })

    describe('and a non-retryable error carries an unknown name', () => {
      beforeEach(async () => {
        const fatal = Object.assign(new RangeError('bad'), {
          name: signed,
          cause: Object.assign(new Error('x'), { name: signed })
        })
        run = harness([fatal])

        error = await run.client.download(url, '1:2', 1024).catch((caught: unknown) => caught)
      })

      it('should render the unknown name as Error', () => {
        expect((error as Error).message).toBe('Request failed on s3.amazonaws.com 1:2: Error')
      })

      it('should keep the URL out of diagnostics and the error', () => {
        expect([...run.lines, ...texts(error)].filter(leaks)).toEqual([])
      })
    })

    describe('and a non-retryable error quotes the URL', () => {
      beforeEach(async () => {
        run = harness([new RangeError(`bad request to ${signed}`)])

        error = await run.client.download(url, '1:2', 1024).catch((caught: unknown) => caught)
      })

      it('should replace the message with the host, label and error name', () => {
        expect((error as Error).message).toBe('Request failed on s3.amazonaws.com 1:2: RangeError')
      })

      it('should keep the URL out of diagnostics and the error', () => {
        expect([...run.lines, ...texts(error)].filter(leaks)).toEqual([])
      })

      it('should not retry', () => {
        expect(run.calls).toHaveLength(1)
      })
    })
  })

  describe('when the signed URL has expired', () => {
    beforeEach(() => {
      run = harness([new Response('', { status: 403 })])
    })

    it('should not retry it', async () => {
      await expect(run.client.download(url, '1:2', 1024)).rejects.toThrow(
        'Export download failed for 1:2: 403'
      )
      expect(run.calls).toHaveLength(1)
    })
  })

  describe('when the export is over the size limit', () => {
    beforeEach(() => {
      run = harness([new Response('x'.repeat(2048))])
    })

    it('should not retry it', async () => {
      await expect(run.client.download(url, '1:2', 1024)).rejects.toThrow(
        'Export exceeds 0.0009765625 MiB: 1:2'
      )
      expect(run.calls).toHaveLength(1)
    })
  })
})

describe('when a call sets no timeout', () => {
  let timeouts: MockInstance<typeof AbortSignal.timeout>
  let client: FigmaClient

  beforeEach(() => {
    timeouts = vi.spyOn(AbortSignal, 'timeout')
    client = new FigmaClient(
      { getToken: async () => 'token' },
      (async () => new Response('{}')) as unknown as typeof fetch,
      { log: () => {} }
    )
  })

  afterEach(() => vi.restoreAllMocks())

  it('should give each attempt 60s', async () => {
    await client.get('/v1/files/key/variables/local')
    expect(timeouts.mock.calls).toEqual([[60000]])
  })
})

describe('when an attempt fails', () => {
  let events: string[]
  let client: FigmaClient

  beforeEach(() => {
    events = []
    const answers = [new Response('', { status: 503 }), new Response('{}')]
    client = new FigmaClient(
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
  })

  it('should write its diagnostic line before the backoff and the next attempt start', async () => {
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
  let body: ReturnType<typeof oversizedError>

  describe('and it answers an API call', () => {
    let run: ReturnType<typeof harness>

    beforeEach(() => {
      body = oversizedError(status)
      run = harness([body.response, ok()])
    })

    it('should bound and cancel the API error body while preserving status and retries', async () => {
      if (status === 404) {
        await expect(run.client.get('/v1/x')).rejects.toMatchObject({
          status,
          message: `Figma API 404 on /v1/x: ${'x'.repeat(200)}`
        })
      } else await expect(run.client.get('/v1/x')).resolves.toEqual({ ok: true })

      expect(body.canceled()).toBe(true)
      expect(body.readBytes()).toBeLessThan(20 * 1024 * 1024)
      expect(body.response.body?.locked).toBe(false)
      expect(run.calls).toHaveLength(status === 404 ? 1 : 2)
      expect(run.delays).toEqual(status === 404 ? [] : [status === 429 ? 0 : 2000])
    })
  })

  describe('and it answers an export download', () => {
    let run: ReturnType<typeof harness>

    beforeEach(() => {
      body = oversizedError(status)
      run = harness([body.response, new Response('<svg/>')])
    })

    it('should bound and cancel export error bodies while preserving status and retries', async () => {
      const downloaded = run.client.download(
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
      expect(run.calls).toHaveLength(status === 404 ? 1 : 2)
      expect(run.delays).toEqual(status === 404 ? [] : [status === 429 ? 0 : 2000])
    })
  })
})

describe('when an error stream reaches the excerpt budget exactly', () => {
  let body: ReturnType<typeof oversizedError>
  let run: ReturnType<typeof harness>

  beforeEach(() => {
    body = oversizedError(404, 1024)
    run = harness([body.response])
  })

  it('should stop reading at the budget instead of waiting for the next chunk', async () => {
    await expect(run.client.get('/v1/x')).rejects.toMatchObject({ status: 404 })
    expect(body.canceled()).toBe(true)
    expect(body.readBytes()).toBeLessThanOrEqual(2048)
  })
})
