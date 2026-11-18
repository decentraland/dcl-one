import assert from 'node:assert/strict'
import { mkdtemp, rm } from 'node:fs/promises'
import { createServer } from 'node:http'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { describe, test } from 'node:test'
import { activeEntitiesKey, retryDelayMs, serveCatalystMirror } from '../driver/catalyst-mirror.ts'
import { listen } from '../driver/serve.ts'

/** An upstream that counts what it is asked and answers each path with its own name. */
async function upstream(): Promise<{ url: string; asked: string[]; close: () => Promise<void> }> {
  const asked: string[] = []
  const server = createServer((request, response) => {
    asked.push(`${request.method} ${request.url}`)
    request.resume()
    request.on('end', () => response.writeHead(200, { 'content-type': 'text/plain' }).end(`upstream ${request.url}`))
  })
  const bound = await listen(server, 0)
  return { url: bound.url.replace(/\/$/, ''), asked, close: bound.close }
}

describe('the catalyst mirror', () => {
  test('asks the upstream for a content file once and keeps it', async () => {
    const directory = await mkdtemp(path.join(tmpdir(), 'mirror-'))
    const real = await upstream()
    const mirror = await serveCatalystMirror(directory, 0, real.url)
    try {
      const file = `${mirror.url}/content/contents/bafkreiabcdefghijklmnopqrstuvwxyz234567`
      const [first, second] = await Promise.all([fetch(file), fetch(file)])
      assert.equal(await first.text(), await second.text())
      assert.equal(first.headers.get('access-control-allow-origin'), '*')
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
    } finally {
      await mirror.close()
      await real.close()
      await rm(directory, { recursive: true, force: true })
    }
  })

  test('names a lookup by what it asks', () => {
    assert.equal(activeEntitiesKey('{"pointers":["a","B"]}'), activeEntitiesKey('{"pointers":["b","a","a"]}'))
    assert.notEqual(activeEntitiesKey('{"pointers":["a"]}'), activeEntitiesKey('{"ids":["a"]}'))
    assert.equal(activeEntitiesKey('{"pointers":"a"}'), undefined)
    assert.equal(activeEntitiesKey('not json'), undefined)
  })

  test('waits as long as the upstream asks, or longer each time, up to a minute', () => {
    assert.equal(retryDelayMs(1, '7'), 7_000)
    assert.equal(retryDelayMs(1, null), 2_000)
    assert.equal(retryDelayMs(3, null), 8_000)
    assert.equal(retryDelayMs(9, null), 60_000)
    assert.equal(retryDelayMs(1, '3600'), 60_000)
  })
})
