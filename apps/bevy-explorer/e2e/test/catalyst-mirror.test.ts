import assert from 'node:assert/strict'
import { mkdtemp, rm } from 'node:fs/promises'
import { createServer } from 'node:http'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { describe, test } from 'node:test'
import { activeEntitiesLookup, retryDelayMs, serveCatalystMirror } from '../driver/catalyst-mirror.ts'
import { listen } from '../driver/serve.ts'

const PAGE = 'http://localhost:8765'

/** An upstream that counts what it is asked, keeps the bodies, and answers each path with its
 *  own name. */
async function upstream(): Promise<{ url: string; asked: string[]; bodies: string[]; close: () => Promise<void> }> {
  const asked: string[] = []
  const bodies: string[] = []
  const server = createServer((request, response) => {
    asked.push(`${request.method} ${request.url}`)
    const chunks: Buffer[] = []
    request.on('data', (chunk: Buffer) => chunks.push(chunk))
    request.on('end', () => {
      if (chunks.length > 0) bodies.push(Buffer.concat(chunks).toString('utf8'))
      response.writeHead(200, { 'content-type': 'text/plain' }).end(`upstream ${request.url}`)
    })
  })
  const bound = await listen(server, 0)
  return { url: bound.url.replace(/\/$/, ''), asked, bodies, close: bound.close }
}

describe('the catalyst mirror', () => {
  test('asks the upstream for a content file once and keeps it', async () => {
    const directory = await mkdtemp(path.join(tmpdir(), 'mirror-'))
    const real = await upstream()
    const mirror = await serveCatalystMirror(directory, 0, real.url, PAGE)
    try {
      const file = `${mirror.url}/content/contents/bafkreiabcdefghijklmnopqrstuvwxyz234567`
      const [first, second] = await Promise.all([fetch(file, { headers: { origin: PAGE } }), fetch(file)])
      assert.equal(await first.text(), await second.text())
      assert.equal(first.headers.get('access-control-allow-origin'), PAGE)
      const again = await fetch(file)
      assert.equal(await again.text(), 'upstream /content/contents/bafkreiabcdefghijklmnopqrstuvwxyz234567')
      assert.equal(real.asked.length, 1)
      assert.deepEqual(mirror.stats, { kept: 1, fetched: 1, passed: 0, refused: 0 })
    } finally {
      await mirror.close()
      await real.close()
      await rm(directory, { recursive: true, force: true })
    }
  })

  test('keeps an active-entities lookup whatever the order of its pointers, and passes the rest through', async () => {
    const directory = await mkdtemp(path.join(tmpdir(), 'mirror-'))
    const real = await upstream()
    const mirror = await serveCatalystMirror(directory, 0, real.url)
    try {
      const lookup = (pointers: string[]): Promise<Response> =>
        fetch(`${mirror.url}/content/entities/active`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ pointers }) })
      await (await lookup(['a', 'b'])).text()
      await (await lookup(['b', 'a'])).text()
      await (await fetch(`${mirror.url}/about`)).text()
      await (await fetch(`${mirror.url}/about`)).text()
      assert.deepEqual(real.asked, ['POST /content/entities/active', 'GET /about', 'GET /about'])
      // the upstream is asked what the kept answer is named by
      assert.deepEqual(real.bodies, ['{"pointers":["a","b"]}'])
    } finally {
      await mirror.close()
      await real.close()
      await rm(directory, { recursive: true, force: true })
    }
  })

  test('answers only its own page, on loopback, and reads no long body', async () => {
    const directory = await mkdtemp(path.join(tmpdir(), 'mirror-'))
    const real = await upstream()
    const mirror = await serveCatalystMirror(directory, 0, real.url, PAGE)
    try {
      const file = `${mirror.url}/content/contents/bafkreiabcdefghijklmnopqrstuvwxyz234567`
      const elsewhere = await fetch(file, { headers: { origin: 'https://example.com' } })
      assert.equal(elsewhere.status, 403)
      assert.equal(elsewhere.headers.get('access-control-allow-origin'), null)
      const preflight = await fetch(file, { method: 'OPTIONS', headers: { origin: 'https://example.com' } })
      assert.equal(preflight.status, 403)
      const renamed = await fetch(file, { headers: { host: 'mirror.example.com' } })
      assert.equal(renamed.status, 403)
      const long = await fetch(`${mirror.url}/content/entities/active`, { method: 'POST', body: 'a'.repeat(2 * 1024 * 1024) }).then(
        (reply) => reply.status,
        () => 413 // the connection may end before the whole body is sent
      )
      assert.equal(long, 413)
      assert.deepEqual(real.asked, [])
    } finally {
      await mirror.close()
      await real.close()
      await rm(directory, { recursive: true, force: true })
    }
  })

  test('names a lookup by what it asks', () => {
    const key = (body: string): string | undefined => activeEntitiesLookup(body)?.key
    assert.equal(key('{"pointers":["a","B"]}'), key('{"pointers":["b","a","a"]}'))
    assert.notEqual(key('{"pointers":["a"]}'), key('{"ids":["a"]}'))
    // a content id's case is part of it
    assert.notEqual(key('{"ids":["QmAbc"]}'), key('{"ids":["qmabc"]}'))
    assert.deepEqual(activeEntitiesLookup('{"ids":["QmB","QmA"],"pointers":["X"]}')?.body, '{"pointers":["x"],"ids":["QmA","QmB"]}')
    assert.equal(key('{"pointers":"a"}'), undefined)
    assert.equal(key('not json'), undefined)
  })

  test('waits as long as the upstream asks, or longer each time, up to a minute', () => {
    assert.equal(retryDelayMs(1, '7'), 7_000)
    assert.equal(retryDelayMs(1, null), 2_000)
    assert.equal(retryDelayMs(3, null), 8_000)
    assert.equal(retryDelayMs(9, null), 60_000)
    assert.equal(retryDelayMs(1, '3600'), 60_000)
  })
})
