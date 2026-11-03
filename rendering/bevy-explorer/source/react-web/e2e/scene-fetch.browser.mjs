import { test } from 'node:test'
import assert from 'node:assert/strict'
import { createServer } from 'node:http'
import { readFile } from 'node:fs/promises'
import { chromium } from 'playwright'

const engine = new URL('../../deploy/web/engine/', import.meta.url)

// Use the real sandbox bootstrap. Substitute only the WASM scene and transport so that
// Chromium's per-origin connection limit doesn't hide how many sends a worker admitted.
function installTransport() {
  const send = self.fetch.bind(self)
  self.held = []
  self.sent = []
  self.fetch = (input, init) => {
    const url = new URL(input instanceof Request ? input.url : input, location.href)
    if (url.pathname !== '/held') {
      self.sent.push(init?.targetAddressSpace)
      return send(input, init)
    }
    const req = new Request(input, init)
    return new Promise((resolve, reject) => {
      self.held.push(() => resolve(new Response('ok', { status: 206 })))
      req.signal.addEventListener('abort', () => reject(req.signal.reason), {once:true})
    })
  }
}

test('scene Fetch isolates workers and preserves the browser request', { timeout: 30_000 }, async t => {
  const server = createServer(async (req, res) => {
    res.setHeader('Cross-Origin-Opener-Policy', 'same-origin')
    res.setHeader('Cross-Origin-Embedder-Policy', 'require-corp')
    const path = new URL(req.url, 'http://localhost').pathname
    if (path === '/') return res.end('<!doctype html>')
    if (path === '/echo') return res.end(req.headers.referer ?? '')
    if (path === '/upload') return req.pipe(res)
    if (path.endsWith('.wasm')) {
      res.setHeader('Content-Type', 'application/wasm')
      return res.end(Buffer.from([0,97,115,109,1,0,0,0]))
    }
    res.setHeader('Content-Type', 'text/javascript')
    if (path.endsWith('/transport.js')) return res.end(`(${installTransport})();`)
    if (path.endsWith('/sandbox_worker.bundle.js')) return res.end(`
      import '/transport.js'
      import '/engine/sandbox_worker.js'
      self.onmessage = ({data}) => postMessage({type:'INIT_COMPLETE', killToken:data.payload.killToken})
    `)
    if (path.endsWith('/dcl_scene_wasm.js')) return res.end('export default () => {}')
    const modules = ['/engine/sandbox_host.js', '/engine/sandbox_worker.js', '/engine/relay_copy.js', '/engine/scene_fetch.js']
    if (!modules.includes(path)) return res.writeHead(404).end()
    res.end(await readFile(new URL(path.slice('/engine/'.length), engine)))
  })
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve))
  t.after(() => { server.closeAllConnections(); return new Promise(resolve => server.close(resolve)) })
  const browser = await chromium.launch({ executablePath: process.env.CHROME_BIN })
  t.after(() => browser.close())
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

  const a = await spawn('client')
  const b = await spawn('client')
  const globals = await a.evaluate(() => {
    const accessible = []
    for (let owner = self; owner; owner = Object.getPrototypeOf(owner)) {
      if (Object.hasOwn(owner, 'fetch')) accessible.push(owner.fetch === fetch)
    }
    return { accessible, xhr: typeof XMLHttpRequest, events: typeof EventSource, socket: typeof WebSocket }
  })
  assert.ok(globals.accessible.length && globals.accessible.every(Boolean))
  assert.deepEqual([globals.xhr, globals.events, globals.socket], ['undefined', 'undefined', 'function'])

  await t.test('client and server capacity, recoverable errors and request options', async () => {
    const serverScene = await spawn('server')
    for (const [worker, expected] of [[a, 5], [b, 5], [serverScene, 32]]) {
      const state = await worker.evaluate(async () => {
        self.requests = Array.from({length: 33}, () => fetch('/held').catch(e => e.name))
        return { admitted: held.length, excess: await Promise.race([requests[32], new Promise(r => setTimeout(() => r('pending'), 20))]) }
      })
      assert.deepEqual(state, { admitted: expected, excess: 'QuotaExceededError' })
    }
    await a.evaluate(() => { setTimeout(() => { throw new Error('recoverable exception') }) })
    await page.waitForFunction(() => window.workerError?.includes('recoverable exception'))
    assert.equal(await a.evaluate(() => held.length), 5)
    const afterError = await a.evaluate(async () => {
      held[0]()
      await requests[0]
      const init = {targetAddressSpace:'loopback', referrer:location.origin + '/chosen', referrerPolicy:'unsafe-url'}
      const echo = fetch('/echo', init)
      init.targetAddressSpace = 'local'
      init.referrer = ''
      // Release waves of headers without consuming their bodies.
      for (let i = 1; i < 32; i++) {
        held[i]()
        await requests[i]
      }
      const response = await echo
      return { referrer: await response.text(), addressSpace: sent.at(-1), body: await (await requests[0]).text() }
    })
    assert.deepEqual(afterError, { referrer: new URL('/chosen', page.url()).href, addressSpace:'loopback', body:'ok' })
    const upload = await a.evaluate(async () => {
      const request = new Request('/upload', {method:'POST', body:'scene payload'})
      const response = fetch(request)
      return { used:request.bodyUsed, body:await (await response).text() }
    })
    assert.deepEqual(upload, { used:true, body:'scene payload' })
    assert.equal(await b.evaluate(() => held.length), 5)
  })

  await t.test('the header deadline includes waiting for admission', async () => {
    // Exercise short deadlines in a real worker, including the time a browser transport
    // can take to report its abort. A queued request must expire without dispatching.
    const deadlines = await a.evaluate(async () => {
      const { createSceneFetch } = await import('/engine/scene_fetch.js')
      let transfers = 0
      const client = createSceneFetch((_request, {signal}) => new Promise((resolve, reject) => {
        transfers++
        signal.addEventListener('abort', () => setTimeout(() => reject(signal.reason), 50))
      }), false, 20)
      const batch = Array.from({length:6}, () => client.fetch('/slow').catch(e => e.name))
      const queued = await batch[5]
      return { queued, outcomes: await Promise.all(batch), transfers }
    })
    assert.deepEqual(deadlines, { queued:'TimeoutError', outcomes:Array(6).fill('TimeoutError'), transfers:5 })
  })

  await t.test('cancelling queued uploads and closing active transfers use captured browser operations', async () => {
    const cancels = await a.evaluate(async () => {
      const { createSceneFetch } = await import('/engine/scene_fetch.js')
      const stablePromise = Promise
      let transfers = 0
      const client = createSceneFetch((_request, {signal}) => new stablePromise((resolve, reject) => {
        transfers++
        signal.addEventListener('abort', () => reject(signal.reason))
      }), false)
      const held = Array.from({length:5}, () => client.fetch('/busy').catch(e => e.name))
      const originalAdd = Set.prototype.add
      const originalListen = EventTarget.prototype.addEventListener
      const controller = new AbortController()
      let queued, uploadCancelled = false
      const body = new ReadableStream({cancel() { uploadCancelled = true }})
      try {
        Set.prototype.add = function (start) { start() }
        EventTarget.prototype.addEventListener = function (_type, listener) { listener() }
        self.Promise = class extends stablePromise { constructor() { super(resolve => resolve()) } }
        queued = client.fetch('/queued', {method:'POST', body, duplex:'half', signal:controller.signal})
      } finally {
        self.Promise = stablePromise
        Set.prototype.add = originalAdd
        EventTarget.prototype.addEventListener = originalListen
      }
      controller.abort()
      const cancellation = await queued.catch(e => e.name)
      client.close()
      return { cancellation, uploadCancelled, active:await Promise.all(held), transfers, closed:await client.fetch('/after-close').catch(e => e.name) }
    })
    assert.deepEqual(cancels, { cancellation:'AbortError', uploadCancelled:true, active:Array(5).fill('AbortError'), transfers:5, closed:'AbortError' })
  })
})
