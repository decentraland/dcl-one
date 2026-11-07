import { createHash } from 'node:crypto'
import { readFile, mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises'
import { createServer, type Server } from 'node:http'
import type { AddressInfo } from 'node:net'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
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
  await new Promise((done) => server.close(done))
  await rm(store, { recursive: true, force: true })
})
const run = (objects: { oid: string; size: number }[]) =>
  prefetch({ store, endpoint, authorization: AUTHORIZATION, objects })

describe('when prefetching listed LFS objects', () => {
  it('should download each missing object into the store, verified, through one batch request', async () => {
    const a = object('alpha')
    const b = object('beta')
    payloads.set(a.oid, 'alpha').set(b.oid, 'beta')

    expect(await run([a, b])).toEqual({ local: 0, downloaded: 2, bytes: 9, failures: [] })
    expect(await readFile(stored(store, a.oid), 'utf8')).toBe('alpha')
    expect(await readFile(stored(store, b.oid), 'utf8')).toBe('beta')
    const batch = requests.filter((item) => item.url === '/lfs/objects/batch')
    expect(batch).toHaveLength(1)
    expect(batch[0]!.authorization).toBe(AUTHORIZATION)
    expect(JSON.parse(batch[0]!.body!)).toMatchObject({ operation: 'download', objects: [a, b] })
  })

  it('should not request an object the store already holds intact', async () => {
    const a = object('alpha')
    await mkdir(path.dirname(stored(store, a.oid)), { recursive: true })
    await writeFile(stored(store, a.oid), 'alpha')

    expect(await run([a])).toEqual({ local: 1, downloaded: 0, bytes: 0, failures: [] })
    expect(requests).toEqual([])
  })

  it('should replace a stored object whose bytes do not match', async () => {
    const a = object('alpha')
    payloads.set(a.oid, 'alpha')
    await mkdir(path.dirname(stored(store, a.oid)), { recursive: true })
    await writeFile(stored(store, a.oid), 'stale')

    expect((await run([a])).downloaded).toBe(1)
    expect(await readFile(stored(store, a.oid), 'utf8')).toBe('alpha')
  })

  it('should report an object the server does not have, and download the rest', async () => {
    const a = object('alpha')
    const gone = object('gone')
    payloads.set(a.oid, 'alpha')

    const result = await run([a, gone])
    expect(result.downloaded).toBe(1)
    expect(result.failures).toEqual([{ oid: gone.oid, reason: 'the server refused it (404)' }])
    await expect(readFile(stored(store, gone.oid))).rejects.toThrow()
  })

  it('should reject a payload whose hash does not match, writing nothing', async () => {
    const a = object('alpha')
    payloads.set(a.oid, 'alpha')
    override.payload = 'omega'

    expect((await run([a])).failures).toEqual([
      { oid: a.oid, reason: 'the payload hash does not match the pointer' }
    ])
    await expect(readFile(stored(store, a.oid))).rejects.toThrow()
  })

  it('should reject a payload whose size does not match', async () => {
    const a = object('alpha')
    payloads.set(a.oid, 'alpha')
    override.payload = 'alphabet'

    expect((await run([a])).failures[0]!.reason).toBe('the payload size does not match the pointer')
  })

  it('should refuse a download action pointing at a plain http host', async () => {
    const a = object('alpha')
    payloads.set(a.oid, 'alpha')
    override.href = 'http://example.com/objects/alpha'

    expect((await run([a])).failures[0]!.reason).toBe('no usable download action')
    expect(requests.some((item) => item.url.startsWith('/objects/'))).toBe(false)
  })

  it('should fail as a whole when the batch request is refused, without echoing the response', async () => {
    const a = object('alpha')
    override.batchStatus = 403

    await expect(run([a])).rejects.toThrow('the LFS batch request answered 403')
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

  it('should succeed with nothing to do on an empty list', async () => {
    expect(await run([])).toEqual({ local: 0, downloaded: 0, bytes: 0, failures: [] })
    expect(requests).toEqual([])
  })
})

describe('when reading the object list', () => {
  it('should keep distinct, well-formed objects and skip blank lines', () => {
    const a = object('alpha')
    expect(parseObjects(`${JSON.stringify(a)}\n\n${JSON.stringify(a)}\n`)).toEqual([a])
  })

  it.each([
    ['a short oid', JSON.stringify({ oid: 'abc', size: 1 })],
    ['a negative size', JSON.stringify({ oid: 'a'.repeat(64), size: -1 })],
    ['a fractional size', JSON.stringify({ oid: 'a'.repeat(64), size: 1.5 })],
    ['a line that is not JSON', 'oid sha256:abc']
  ])('should reject %s', (_, line) => {
    expect(() => parseObjects(line)).toThrow()
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
  it('should refuse to start without LFS_AUTHORIZATION', async () => {
    const stderr = vi.spyOn(process.stderr, 'write').mockReturnValue(true)
    try {
      expect(await main(['--endpoint', endpoint, '--objects', '/dev/null'], {})).toBe(2)
    } finally {
      stderr.mockRestore()
    }
  })

  it('should exit 1 and name each missing object, never the authorization', async () => {
    const repo = await mkdtemp(path.join(tmpdir(), 'lfs-repo-'))
    const list = path.join(repo, 'objects.jsonl')
    const gone = object('gone')
    await writeFile(list, JSON.stringify(gone))
    const { execFileSync } = await import('node:child_process')
    execFileSync('git', ['init', '-q'], { cwd: repo })
    const written: string[] = []
    const stdout = vi.spyOn(process.stdout, 'write').mockImplementation((chunk) => {
      written.push(String(chunk))
      return true
    })
    try {
      expect(
        await main(['--root', repo, '--endpoint', endpoint, '--objects', list], {
          LFS_AUTHORIZATION: AUTHORIZATION
        })
      ).toBe(1)
    } finally {
      stdout.mockRestore()
      await rm(repo, { recursive: true, force: true })
    }
    expect(written.join('')).toContain(`MISSING ${gone.oid}`)
    expect(written.join('')).not.toContain(AUTHORIZATION)
  })
})
