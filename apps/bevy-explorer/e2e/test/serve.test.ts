import assert from 'node:assert/strict'
import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { after, before, describe, test } from 'node:test'
import { WEB_APP_HEADERS, resolveFile, serveWebApp, type StaticServer } from '../driver/serve.ts'

describe('serveWebApp', () => {
  let dir: string
  let server: StaticServer

  before(async () => {
    dir = await mkdtemp(path.join(tmpdir(), 'e2e-serve-'))
    await mkdir(path.join(dir, 'engine'))
    await writeFile(path.join(dir, 'index.html'), '<!doctype html><title>shell</title>')
    await writeFile(path.join(dir, 'app.html'), '<!doctype html><title>app</title>')
    await writeFile(path.join(dir, 'engine', 'engine.js'), 'export {}')
    server = await serveWebApp(dir, 0)
  })

  after(async () => {
    await server.close()
    await rm(dir, { recursive: true, force: true })
  })

  test("the benchmark's headers are on every response", async () => {
    for (const route of ['/', '/app.html', '/engine/engine.js', '/missing']) {
      const response = await fetch(server.url + route.slice(1))
      for (const [name, value] of Object.entries(WEB_APP_HEADERS)) assert.equal(response.headers.get(name), value, `${name} on ${route}`)
    }
  })

  test('/ is index.html with its type', async () => {
    const response = await fetch(server.url)
    assert.equal(response.status, 200)
    assert.equal(response.headers.get('content-type'), 'text/html; charset=utf-8')
    assert.match(await response.text(), /shell/)
    assert.equal((await fetch(`${server.url}engine/engine.js`)).headers.get('content-type'), 'text/javascript; charset=utf-8')
  })

  test('missing files are 404, other methods 405, bad paths 400', async () => {
    assert.equal((await fetch(`${server.url}nope.js`)).status, 404)
    assert.equal((await fetch(server.url, { method: 'POST' })).status, 405)
    assert.equal((await fetch(`${server.url}a%5Cb`)).status, 400)
    assert.equal((await fetch(`${server.url}..%2Fx`)).status, 400)
  })

  test('HEAD carries the length and no body', async () => {
    const response = await fetch(`${server.url}app.html`, { method: 'HEAD' })
    assert.equal(response.status, 200)
    assert.equal(response.headers.get('content-length'), String(Buffer.byteLength('<!doctype html><title>app</title>')))
    assert.equal(await response.text(), '')
  })
})

describe('resolveFile', () => {
  const root = path.join(path.sep, 'srv', 'app')
  test('maps a path under the root and the index', () => {
    assert.equal(resolveFile(root, '/engine/pkg/a.wasm?x=1'), path.join(root, 'engine', 'pkg', 'a.wasm'))
    assert.equal(resolveFile(root, '/'), path.join(root, 'index.html'))
    assert.equal(resolveFile(root, '/sub/'), path.join(root, 'sub', 'index.html'))
  })
  test('refuses traversal, backslashes, colons and control characters', () => {
    assert.equal(resolveFile(root, '/../etc'), undefined)
    assert.equal(resolveFile(root, '/a\\b'), undefined)
    assert.equal(resolveFile(root, '/c:/x'), undefined)
    assert.equal(resolveFile(root, '/a%0Ab'), undefined)
  })
})
