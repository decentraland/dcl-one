import { createHash } from 'node:crypto'
import { readFile, mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises'
import { createServer, type Server } from 'node:http'
import type { AddressInfo } from 'node:net'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { afterEach, beforeEach, describe, expect, it, vi, type Mock } from 'vitest'
import { allowedUrl, main, parseObjects, prefetch } from './lfs-prefetch.mjs'

const AUTHORIZATION = 'Basic ZmFrZS10b2tlbg=='
const object = (content: string) => ({
  oid: createHash('sha256').update(content).digest('hex'),
  size: Buffer.byteLength(content)
})
const stored = (store: string, oid: string) =>
  path.join(store, oid.slice(0, 2), oid.slice(2, 4), oid)

// A fake LFS server: the batch endpoint answers from `payloads`, each object downloads from /objects/<oid>.
let server: Server
let endpoint: string
let store: string
let payloads: Map<string, string>
let requests: { url: string; authorization?: string; body?: string }[]
let override: Partial<Record<string, unknown>>
beforeEach(async () => {
  store = await mkdtemp(path.join(tmpdir(), 'lfs-store-'))
  payloads = new Map()
  requests = []
  override = {}
  server = createServer((request, response) => {
    let body = ''
    request.on('data', (chunk) => (body += chunk))
    request.on('end', () => {
      requests.push({ url: request.url!, authorization: request.headers.authorization, body })
      const base = `http://127.0.0.1:${(server.address() as AddressInfo).port}`
      if (request.url === '/lfs/objects/batch') {
        if (override.batchStatus) {
          response.writeHead(override.batchStatus as number).end('server detail')
          return
        }
        const asked = JSON.parse(body).objects as { oid: string; size: number }[]
        response.writeHead(200, { 'Content-Type': 'application/vnd.git-lfs+json' }).end(
          JSON.stringify({
            objects: asked.map(({ oid, size }) =>
              payloads.has(oid)
                ? {
                    oid,
                    size,
                    actions: {
                      download: {
                        href: (override.href as string) ?? `${base}/objects/${oid}`,
                        header: { 'X-Download': 'yes' }
                      }
                    }
                  }
                : { oid, size, error: { code: 404, message: 'Object does not exist' } }
            )
          })
        )
      } else if (request.url?.startsWith('/objects/')) {
        const oid = request.url.slice('/objects/'.length)
        const payload = (override.payload as string) ?? payloads.get(oid)
        if (payload === undefined) response.writeHead(404).end()
        else response.writeHead(200).end(payload)
      } else response.writeHead(404).end()
    })
  })
  await new Promise<void>((done) => server.listen(0, '127.0.0.1', done))
  endpoint = `http://127.0.0.1:${(server.address() as AddressInfo).port}/lfs`
})
afterEach(async () => {
  vi.resetAllMocks()
  vi.restoreAllMocks()
  await new Promise((done) => server.close(done))
  await rm(store, { recursive: true, force: true })
})
const run = (objects: { oid: string; size: number }[]) =>
  prefetch({ store, endpoint, authorization: AUTHORIZATION, objects })

// An error response whose body is never read keeps its connection busy; each one must be released.
const errorResponse = (status: number, cancel: () => Promise<void>) => {
  const body = new ReadableStream({ cancel })
  return new Response(body, { status })
}

describe('when prefetching listed LFS objects', () => {
  let a: { oid: string; size: number }

  beforeEach(() => {
    a = object('alpha')
  })

  describe('and every object is missing from the store', () => {
    let b: { oid: string; size: number }

    beforeEach(() => {
      b = object('beta')
      payloads.set(a.oid, 'alpha').set(b.oid, 'beta')
    })

    it('should download each missing object into the store, verified, through one batch request', async () => {
      expect(await run([a, b])).toEqual({ local: 0, downloaded: 2, bytes: 9, failures: [] })
      expect(await readFile(stored(store, a.oid), 'utf8')).toBe('alpha')
      expect(await readFile(stored(store, b.oid), 'utf8')).toBe('beta')
      const batch = requests.filter((item) => item.url === '/lfs/objects/batch')
      expect(batch).toHaveLength(1)
      expect(batch[0]!.authorization).toBe(AUTHORIZATION)
      expect(JSON.parse(batch[0]!.body!)).toMatchObject({ operation: 'download', objects: [a, b] })
    })
  })

  describe('and the store already holds the object intact', () => {
    beforeEach(async () => {
      await mkdir(path.dirname(stored(store, a.oid)), { recursive: true })
      await writeFile(stored(store, a.oid), 'alpha')
    })

    it('should not request the object', async () => {
      expect(await run([a])).toEqual({ local: 1, downloaded: 0, bytes: 0, failures: [] })
      expect(requests).toEqual([])
    })
  })

  describe('and a stored object has bytes that do not match', () => {
    beforeEach(async () => {
      payloads.set(a.oid, 'alpha')
      await mkdir(path.dirname(stored(store, a.oid)), { recursive: true })
      await writeFile(stored(store, a.oid), 'stale')
    })

    it('should replace the stored object', async () => {
      expect((await run([a])).downloaded).toBe(1)
      expect(await readFile(stored(store, a.oid), 'utf8')).toBe('alpha')
    })
  })

  describe('and the server does not have one of the objects', () => {
    let gone: { oid: string; size: number }

    beforeEach(() => {
      gone = object('gone')
      payloads.set(a.oid, 'alpha')
    })

    it('should report the missing object, and download the rest', async () => {
      const result = await run([a, gone])
      expect(result.downloaded).toBe(1)
      expect(result.failures).toEqual([{ oid: gone.oid, reason: 'the server refused it (404)' }])
      await expect(readFile(stored(store, gone.oid))).rejects.toThrow('ENOENT')
    })
  })

  describe('and the payload hash does not match', () => {
    beforeEach(() => {
      payloads.set(a.oid, 'alpha')
      override.payload = 'omega'
    })

    it('should reject the payload, writing nothing', async () => {
      expect((await run([a])).failures).toEqual([
        { oid: a.oid, reason: 'the payload hash does not match the pointer' }
      ])
      await expect(readFile(stored(store, a.oid))).rejects.toThrow('ENOENT')
    })
  })

  describe('and the batch response fails', () => {
    let cancel: Mock<() => Promise<void>>
    let fetchFn: Mock<(url: string) => Promise<Response>>

    beforeEach(() => {
      cancel = vi.fn(() => Promise.resolve())
      fetchFn = vi.fn(async () => errorResponse(503, cancel))
    })

    it('should release its body and report its status', async () => {
      await expect(
        prefetch({ store, endpoint, authorization: AUTHORIZATION, objects: [object('a')], fetchFn })
      ).rejects.toThrow('the LFS batch request answered 503')
      expect(cancel).toHaveBeenCalledTimes(1)
    })
  })

  describe('and every download fails', () => {
    let listed: { oid: string; size: number }[]
    let cancel: Mock<() => Promise<void>>
    let fetchFn: Mock<(url: string) => Promise<Response>>

    beforeEach(() => {
      listed = ['a', 'b', 'c', 'd', 'e'].map(object)
      cancel = vi.fn(() => Promise.resolve())
      fetchFn = vi.fn(async (url: string) =>
        url.endsWith('/objects/batch')
          ? Response.json({
              objects: listed.map(({ oid, size }) => ({
                oid,
                size,
                actions: { download: { href: `${endpoint}/download/${oid}` } }
              }))
            })
          : errorResponse(500, cancel)
      )
    })

    it('should release the body of every failed download and still try the remaining objects', async () => {
      const result = await prefetch({
        store,
        endpoint,
        authorization: AUTHORIZATION,
        objects: listed,
        fetchFn
      })
      expect(result.failures).toEqual(
        listed.map(({ oid }) => ({ oid, reason: 'the download answered 500' }))
      )
      expect(cancel).toHaveBeenCalledTimes(listed.length)
    })
  })

  describe('and releasing the body of a failed download itself fails', () => {
    let cancel: Mock<() => Promise<void>>
    let fetchFn: Mock<(url: string) => Promise<Response>>

    beforeEach(() => {
      a = object('a')
      cancel = vi.fn(() => Promise.reject(new Error('stream already closed')))
      fetchFn = vi.fn(async (url: string) =>
        url.endsWith('/objects/batch')
          ? Response.json({
              objects: [{ ...a, actions: { download: { href: `${endpoint}/download/${a.oid}` } } }]
            })
          : errorResponse(502, cancel)
      )
    })

    it('should keep the status as the failure', async () => {
      expect(
        (await prefetch({ store, endpoint, authorization: AUTHORIZATION, objects: [a], fetchFn }))
          .failures
      ).toEqual([{ oid: a.oid, reason: 'the download answered 502' }])
      expect(cancel).toHaveBeenCalledTimes(1)
    })
  })

  describe('and the payload size does not match', () => {
    beforeEach(() => {
      payloads.set(a.oid, 'alpha')
      override.payload = 'alphabet'
    })

    it('should reject the payload', async () => {
      expect((await run([a])).failures[0]!.reason).toBe(
        'the payload size does not match the pointer'
      )
    })
  })

  describe('and the download action points at a plain http host', () => {
    beforeEach(() => {
      payloads.set(a.oid, 'alpha')
      override.href = 'http://example.com/objects/alpha'
    })

    it('should refuse the download action', async () => {
      expect((await run([a])).failures[0]!.reason).toBe('no usable download action')
      expect(requests.some((item) => item.url.startsWith('/objects/'))).toBe(false)
    })
  })

  describe('and the batch request is refused', () => {
    beforeEach(() => {
      override.batchStatus = 403
    })

    it('should fail as a whole without echoing the response', async () => {
      await expect(run([a])).rejects.toThrow('the LFS batch request answered 403')
    })
  })

  it.each([
    ['a plain http endpoint', 'http://example.com/lfs'],
    ['an endpoint carrying credentials', 'https://user:secret@example.com/lfs'],
    ['something that is not a URL', 'not a url']
  ])('should refuse %s before any request', async (_, value) => {
    await expect(
      prefetch({ store, endpoint: value, authorization: AUTHORIZATION, objects: [object('a')] })
    ).rejects.toThrow('the LFS endpoint must be an https URL without credentials')
    expect(requests).toEqual([])
  })

  describe('and the list is empty', () => {
    it('should succeed with nothing to do', async () => {
      expect(await run([])).toEqual({ local: 0, downloaded: 0, bytes: 0, failures: [] })
      expect(requests).toEqual([])
    })
  })
})

describe('when reading the object list', () => {
  describe('and it repeats an object between blank lines', () => {
    let a: { oid: string; size: number }

    beforeEach(() => {
      a = object('alpha')
    })

    it('should keep distinct, well-formed objects and skip blank lines', () => {
      expect(parseObjects(`${JSON.stringify(a)}\n\n${JSON.stringify(a)}\n`)).toEqual([a])
    })
  })

  it.each([
    ['a short oid', JSON.stringify({ oid: 'abc', size: 1 }), 'malformed LFS object line'],
    [
      'a negative size',
      JSON.stringify({ oid: 'a'.repeat(64), size: -1 }),
      'malformed LFS object line'
    ],
    [
      'a fractional size',
      JSON.stringify({ oid: 'a'.repeat(64), size: 1.5 }),
      'malformed LFS object line'
    ],
    ['a line that is not JSON', 'oid sha256:abc', 'is not valid JSON']
  ])('should reject %s', (_, line, message) => {
    expect(() => parseObjects(line)).toThrow(message)
  })
})

describe('when checking a URL', () => {
  it.each([
    ['https', 'https://github.com/example/repo.git/info/lfs', true],
    ['loopback http', 'http://127.0.0.1:8080/lfs', true],
    ['other http', 'http://github.com/lfs', false],
    ['credentials', 'https://token@github.com/lfs', false],
    ['another scheme', 'file:///tmp/lfs', false]
  ])('should judge %s', (_, value, expected) => {
    expect(allowedUrl(value)).toBe(expected)
  })
})

describe('when run from the command line', () => {
  describe('and LFS_AUTHORIZATION is missing', () => {
    beforeEach(() => {
      vi.spyOn(process.stderr, 'write').mockReturnValue(true)
    })

    it('should refuse to start', async () => {
      expect(await main(['--endpoint', endpoint, '--objects', '/dev/null'], {})).toBe(2)
    })
  })

  describe('and an object is missing from the server', () => {
    let repo: string
    let list: string
    let gone: { oid: string; size: number }
    let written: string[]

    beforeEach(async () => {
      repo = await mkdtemp(path.join(tmpdir(), 'lfs-repo-'))
      list = path.join(repo, 'objects.jsonl')
      gone = object('gone')
      await writeFile(list, JSON.stringify(gone))
      const { execFileSync } = await import('node:child_process')
      execFileSync('git', ['init', '-q'], { cwd: repo })
      written = []
      vi.spyOn(process.stdout, 'write').mockImplementation((chunk) => {
        written.push(String(chunk))
        return true
      })
    })

    afterEach(async () => {
      vi.restoreAllMocks()
      await rm(repo, { recursive: true, force: true })
    })

    it('should exit 1 and name each missing object, never the authorization', async () => {
      expect(
        await main(['--root', repo, '--endpoint', endpoint, '--objects', list], {
          LFS_AUTHORIZATION: AUTHORIZATION
        })
      ).toBe(1)
      expect(written.join('')).toContain(`MISSING ${gone.oid}`)
      expect(written.join('')).not.toContain(AUTHORIZATION)
    })
  })
})
