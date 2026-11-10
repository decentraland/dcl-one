import { test } from 'node:test'
import assert from 'node:assert/strict'
import { createServer } from 'node:http'
import { readFile } from 'node:fs/promises'
import { chromium } from '@playwright/test'

const engine = new URL('../../deploy/web/engine/', import.meta.url)

// Observe dispatch before Chromium applies its own per-origin connection limit.
function observeTransport() {
  const send = self.fetch.bind(self)
  self.sends = []
  self.peak = 0
  let active = 0
  self.fetch = async (input, init) => {
    self.sends.push({ url: input.url, addressSpace: init?.targetAddressSpace })
    self.peak = Math.max(self.peak, ++active)
    try {
      return await send(input, init)
    } finally {
      active--
    }
  }
}

// Chrome's cold start on a CI runner has taken 1 s in some runs and over 30 s in others. The
// budget covers it; the diagnostics and DEBUG=pw:browser in CI say where the time went.
test('scene Fetch isolates workers and preserves the browser request', { timeout: 120_000 }, async t => {
  const started = performance.now()
  const elapsed = () => `${((performance.now() - started) / 1000).toFixed(1)}s`
  const gates = new Map()
  let cachedHits = 0
  // Cookie headers the server saw, by path and query.
  const cookies = []
  const server = createServer(async (req, res) => {
    res.setHeader('Cross-Origin-Opener-Policy', 'same-origin')
    res.setHeader('Cross-Origin-Embedder-Policy', 'require-corp')
    const url = new URL(req.url, 'http://localhost')
    const path = url.pathname
    if (path === '/') {
      res.setHeader('Set-Cookie', 'page=1; Path=/')
      return res.end('<!doctype html>')
    }
    if (path === '/cookie') {
      cookies.push([req.url, req.headers.cookie ?? ''])
      res.setHeader('Set-Cookie', 'scene=1; Path=/')
      return res.end(req.headers.cookie ?? '')
    }
    if (path === '/cached') {
      cachedHits++
      res.setHeader('Cache-Control', 'max-age=600')
      return res.end(String(cachedHits))
    }
    if (path === '/batch') {
      const gate = gates.get(url.searchParams.get('scene'))
      if (gate) await gate.promise
      return res.end(url.searchParams.get('i'))
    }
    if (path === '/stall') return
    if (path === '/stream') return res.write('partial body')
    if (path === '/echo') return res.end(req.headers.referer ?? '')
    if (path === '/ipfs-header') return res.end(req.headers['x-ipfs'] ?? '')
    if (path === '/upload') return req.pipe(res)
    if (path.endsWith('.wasm')) {
      res.setHeader('Content-Type', 'application/wasm')
      return res.end(Buffer.from([0,97,115,109,1,0,0,0]))
    }
    res.setHeader('Content-Type', 'text/javascript')
    if (path.endsWith('/transport.js')) return res.end(`(${observeTransport})();`)
    if (path.endsWith('/sandbox_worker.bundle.js')) return res.end(`
      import '/transport.js'
      import '/engine/sandbox_worker.js'
      import { scrubSceneGlobal, createSceneScope } from '/engine/scene_global.js'
      self.scrubSceneGlobal = scrubSceneGlobal
      self.createSceneScope = createSceneScope
      self.onmessage = ({data}) => postMessage({type:'INIT_COMPLETE', killToken:data.payload.killToken})
    `)
    if (path.endsWith('/dcl_scene_wasm.js')) return res.end('export default () => {}')
    const modules = ['/engine/sandbox_host.js', '/engine/sandbox_worker.js', '/engine/relay_copy.js', '/engine/scene_fetch.js', '/engine/scene_global.js']
    if (!modules.includes(path)) return res.writeHead(404).end()
    res.end(await readFile(new URL(path.slice('/engine/'.length), engine)))
  })
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve))
  t.after(() => { server.closeAllConnections(); return new Promise(resolve => server.close(resolve)) })
  const browser = await chromium.launch({ executablePath: process.env.CHROME_BIN, timeout: 90_000 })
  t.after(() => browser.close())
  t.diagnostic(`browser launched at ${elapsed()}`)
  const page = await browser.newPage()
  await page.goto(`http://127.0.0.1:${server.address().port}`)
  await page.evaluate(async () => {
    const { createSandboxHost } = await import('/engine/sandbox_host.js')
    const onWorkerCrash = event => { event.preventDefault(); window.workerError = event.message }
    window.client = createSandboxHost({ onWorkerCrash })
    window.server = createSandboxHost({ onWorkerCrash, role: 'server' })
  })
  async function spawn(role) {
    const ready = page.waitForEvent('worker')
    await page.evaluate(role => window[role].spawn(), role)
    return ready
  }
  t.diagnostic(`page ready at ${elapsed()}`)

  const ids = Array.from({ length: 40 }, (_, i) => String(i))
  for (const role of ['client', 'server']) {
    await t.test(`two ${role} scenes complete independent batches of 40 in FIFO order`, async t => {
      const gate = Promise.withResolvers()
      gates.set(role, gate)
      t.after(() => gate.resolve())
      const blocked = await spawn(role)
      const other = await spawn(role)
      for (const [worker, scene] of [[blocked, role], [other, 'other']]) {
        const started = await worker.evaluate(scene => {
          self.batch = Promise.all(Array.from({ length: 40 }, async (_, i) =>
            (await fetch(`/batch?scene=${scene}&i=${i}`)).text()))
          self.batch.catch(() => {}) // observed below, after the other worker starts
          return sends.length
        }, scene)
        assert.equal(started, 5)
      }
      const result = worker => worker.evaluate(async () => ({
        bodies: await self.batch,
        order: sends.map(({ url }) => new URL(url).searchParams.get('i')),
        peak,
      }))
      assert.deepEqual(await result(other), { bodies: ids, order: ids, peak: 5 })
      assert.equal(await blocked.evaluate(() => sends.length), 5)
      // An uncaught callback error can be reported while its worker continues running.
      const errorMessage = `recoverable ${role} exception`
      await blocked.evaluate(message => { setTimeout(() => { throw new Error(message) }) }, errorMessage)
      await page.waitForFunction(message => window.workerError?.includes(message), errorMessage)
      gate.resolve()
      assert.deepEqual(await result(blocked), { bodies: ids, order: ids, peak: 5 })
    })
  }

  await t.test('queued calls snapshot their options and honour caller cancellation', async () => {
    const worker = await spawn('client')
    const result = await worker.evaluate(async () => {
      const blocked = new AbortController()
      const busy = Array.from({ length: 4 }, () => fetch('/stall', { signal: blocked.signal }).catch(e => e.name))
      const active = new AbortController()
      const fifth = fetch('/stall', { signal: active.signal }).catch(e => e.name)

      const queued = new AbortController()
      let cancelled
      const body = new ReadableStream({ cancel(reason) { cancelled = reason } })
      const upload = fetch('/upload', { method: 'POST', body, duplex: 'half', signal: queued.signal }).catch(e => e)
      const timeout = fetch('/queued-timeout', { signal: AbortSignal.timeout(20) }).catch(e => e.name)
      const init = { targetAddressSpace: 'loopback', referrer: location.origin + '/chosen', referrerPolicy: 'no-referrer' }
      const echo = fetch('/echo', init)
      init.targetAddressSpace = 'local'
      init.referrerPolicy = 'unsafe-url'
      const request = new Request('/upload', { method: 'POST', body: 'scene payload' })
      const post = fetch(request)
      const used = request.bodyUsed

      queued.abort('unsent upload')
      const timeoutName = await timeout
      active.abort()
      const referrer = await (await echo).text()
      const posted = await (await post).text()
      blocked.abort()
      return {
        timeoutName, upload: await upload, cancelled, active: await fifth, busy: await Promise.all(busy),
        referrer, posted, used,
        paths: sends.map(({ url }) => new URL(url).pathname),
        addressSpace: sends.find(({ url }) => url.endsWith('/echo')).addressSpace,
      }
    })
    assert.deepEqual(result, {
      timeoutName: 'TimeoutError', upload: 'unsent upload', cancelled: 'unsent upload',
      active: 'AbortError', busy: Array(4).fill('AbortError'),
      referrer: '', posted: 'scene payload', used: true,
      paths: [...Array(5).fill('/stall'), '/echo', '/upload'], addressSpace: 'loopback',
    })
    const referrer = await worker.evaluate(async () => (await fetch('/echo', {referrer:location.origin + '/chosen'})).text())
    assert.equal(referrer, new URL('/chosen', page.url()).href)
  })

  await t.test('a scene has no persistent storage and no request path around its fetch', async () => {
    const worker = await spawn('client')
    const result = await worker.evaluate(async () => {
      // Bare names resolve to the worker global, as they do in scene code.
      const reachable = new Function(`return {
        indexedDB: typeof indexedDB, caches: typeof caches, storage: typeof navigator.storage,
        storageBuckets: typeof navigator.storageBuckets, locks: typeof navigator.locks,
        serviceWorker: typeof navigator.serviceWorker, XMLHttpRequest: typeof XMLHttpRequest,
        EventSource: typeof EventSource, importScripts: typeof importScripts,
        usb: typeof navigator.usb, hid: typeof navigator.hid, serial: typeof navigator.serial,
      }`)
      const before = reachable()
      scrubSceneGlobal()
      const after = reachable()
      const cookie = await (await fetch('/cookie', { credentials: 'include' })).text()
      const cached = [
        await (await fetch('/cached')).text(),
        await (await fetch('/cached', { cache: 'force-cache' })).text(),
      ]
      // Removed when the sandbox loads: the native fetch up the chain, and the other request paths.
      let native = 'none'
      for (let o = Object.getPrototypeOf(self); o != null; o = Object.getPrototypeOf(o)) {
        if (Object.prototype.hasOwnProperty.call(o, 'fetch')) {
          native = await (await o.fetch.call(self, '/cookie?via=prototype', { credentials: 'include' })).text()
        }
      }
      const others = new Function('return [typeof WebTransport, typeof FontFace, typeof fonts]')()
      return { before, after, cookie, cached, native, others }
    })
    // Chromium gives a dedicated worker no navigator.serviceWorker; the rest are there to remove.
    for (const [name, type] of Object.entries(result.before)) {
      if (name !== 'serviceWorker') assert.notEqual(type, 'undefined', `${name} before the scrub`)
    }
    assert.deepEqual(result, {
      before: result.before,
      after: Object.fromEntries(Object.keys(result.before).map(name => [name, 'undefined'])),
      cookie: '',
      cached: ['1', '2'],
      native: 'none',
      others: ['undefined', 'undefined', 'undefined'],
    })
    assert.deepEqual(cookies.filter(([url]) => url.includes('via=prototype')), [])
    // The response's cookie was not stored either.
    assert.equal(await page.evaluate(() => document.cookie), 'page=1')
    // The page's own fetch does send its cookie and use the cache.
    const page_ = await page.evaluate(async () => ({
      cookie: await (await fetch('/cookie')).text(),
      cached: [await (await fetch('/cached')).text(), await (await fetch('/cached')).text()],
    }))
    assert.deepEqual(page_, { cookie: 'page=1', cached: ['3', '3'] })
  })

  await t.test('a scene fetch drops the content cache header, even through a replaced getter', async () => {
    const worker = await spawn('client')
    const sent = await worker.evaluate(async () => {
      const headers = Object.getOwnPropertyDescriptor(Request.prototype, 'headers')
      const read = async init => (await fetch('/ipfs-header', init)).text()
      const plain = await read({ headers: { 'x-IpFs': '1' } })
      Object.defineProperty(Request.prototype, 'headers', { get: () => new Headers(), configurable: true })
      const replaced = await read({ headers: { 'X-IPFS': '1' } })
      Object.defineProperty(Request.prototype, 'headers', headers)
      return [plain, replaced]
    })
    assert.deepEqual(sent, ['', ''])
  })

  await t.test('replaced Set and EventTarget methods leave the five-send limit in place', async () => {
    const worker = await spawn('client')
    await worker.evaluate(() => {
      Set.prototype.add = function (f) { if (typeof f === 'function') f(); return this }
      Set.prototype.values = function () { return [][Symbol.iterator]() }
      EventTarget.prototype.addEventListener = () => {}
      EventTarget.prototype.removeEventListener = () => {}
      self.stalls = new AbortController()
      for (let i = 0; i < 20; i++) fetch(`/stall?i=${i}`, { signal: stalls.signal }).catch(() => {})
    })
    await new Promise(resolve => setTimeout(resolve, 300))
    assert.equal(await worker.evaluate(() => sends.length), 5)
    // the browser's per-host connection limit is shared with the cases after this one
    await worker.evaluate(() => stalls.abort())
  })

  await t.test('scene code calls the timers as globals or as methods of globalThis', async () => {
    const worker = await spawn('client')
    const result = await worker.evaluate(async () => {
      const { proxy, preamble } = createSceneScope(Object.create(null))
      // Scene code runs as Function code with the scene's globalThis (sandbox_worker.js runWithScope).
      const scene = new Function('globalThis', `${preamble}
        return (async () => ({
          same: globalThis.setTimeout === setTimeout && globalThis.clearTimeout === clearTimeout &&
            globalThis.setInterval === setInterval && globalThis.clearInterval === clearInterval,
          fired: await new Promise(resolve => globalThis.setTimeout(() => resolve(true), 1)),
          cleared: await new Promise(resolve => {
            globalThis.clearTimeout(globalThis.setTimeout(() => resolve(false), 1))
            setTimeout(() => resolve(true), 20)
          }),
          ticked: await new Promise(resolve => {
            const id = globalThis.setInterval(() => { globalThis.clearInterval(id); resolve(true) }, 1)
          }),
        }))()`)
      return scene.call(proxy, proxy)
    })
    assert.deepEqual(result, { same: true, fired: true, cleared: true, ticked: true })
  })

  await t.test('headers release the slot and the caller can abort the response body', async () => {
    const worker = await spawn('server')
    const result = await worker.evaluate(async () => {
      const controller = new AbortController()
      const responses = await Promise.all(Array.from({ length: 5 }, () => fetch('/stream', { signal: controller.signal })))
      const timeout = await fetch('/stall', { signal: AbortSignal.timeout(50) }).catch(e => e.name)
      controller.abort()
      return { timeout, bodies: await Promise.all(responses.map(r => r.text().catch(e => e.name))), sent: sends.length }
    })
    assert.deepEqual(result, { timeout: 'TimeoutError', bodies: Array(5).fill('AbortError'), sent: 6 })
  })
})
