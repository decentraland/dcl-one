import { test } from 'node:test'
import assert from 'node:assert/strict'
import { createServer } from 'node:http'
import { createHash } from 'node:crypto'
import { readFile } from 'node:fs/promises'
import { fileURLToPath } from 'node:url'
import { build } from 'esbuild'
import { chromium } from '@playwright/test'

const web = new URL('../../deploy/web/', import.meta.url)
const engine = new URL('engine/', web)

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

// The sandbox bundled as build-web.py does, so it loads under its own policy, with the
// transport observer first and the scene runtime and INIT stubbed.
async function bundleSandbox() {
  const stubs = {
    './transport.js': `(${observeTransport})();`,
    './pkg-scene/dcl_scene_wasm.js': 'export default () => {}',
  }
  const result = await build({
    stdin: {
      contents: `import './transport.js'
        import './sandbox_worker.js'
        self.onmessage = ({data}) => postMessage({type:'INIT_COMPLETE', killToken:data.payload.killToken})`,
      resolveDir: fileURLToPath(engine),
    },
    bundle: true, format: 'esm', write: false, logLevel: 'error',
    plugins: [{ name: 'stubs', setup(b) {
      b.onResolve({ filter: /^\.\/(transport|pkg-scene\/)/ }, args => ({ path: args.path, namespace: 'stub' }))
      b.onLoad({ filter: /.*/, namespace: 'stub' }, args => ({ contents: stubs[args.path], loader: 'js' }))
    } }],
  })
  return result.outputFiles[0].text
}

// Chrome's cold start on a CI runner has taken 1 s in some runs and over 30 s in others. The
// budget covers it; the diagnostics and DEBUG=pw:browser in CI say where the time went.
test('scene Fetch isolates workers and preserves the browser request', { timeout: 120_000 }, async t => {
  const started = performance.now()
  const elapsed = () => `${((performance.now() - started) / 1000).toFixed(1)}s`
  const gates = new Map()
  const sandbox = await bundleSandbox()
  // A cross-origin module host that would answer a scene's import().
  const imports = []
  const other = createServer((req, res) => {
    imports.push(req.url)
    res.setHeader('Access-Control-Allow-Origin', '*')
    res.setHeader('Cross-Origin-Resource-Policy', 'cross-origin')
    res.setHeader('Content-Type', 'text/javascript')
    res.end('export default 1')
  })
  // Answers the WebSocket handshake and nothing else.
  other.on('upgrade', (req, socket) => {
    const accept = createHash('sha1').update(req.headers['sec-websocket-key'] + '258EAFA5-E914-47DA-95CA-C5AB0DC85B11').digest('base64')
    socket.end(`HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: ${accept}\r\n\r\n`)
  })
  await new Promise(resolve => other.listen(0, '127.0.0.1', resolve))
  t.after(() => { other.closeAllConnections(); return new Promise(resolve => other.close(resolve)) })
  const otherOrigin = `http://127.0.0.1:${other.address().port}`
  const server = createServer(async (req, res) => {
    res.setHeader('Cross-Origin-Opener-Policy', 'same-origin')
    res.setHeader('Cross-Origin-Embedder-Policy', 'require-corp')
    const url = new URL(req.url, 'http://localhost')
    const path = url.pathname
    if (path === '/') return res.end('<!doctype html>')
    if (path === '/batch') {
      const gate = gates.get(url.searchParams.get('scene'))
      if (gate) await gate.promise
      return res.end(url.searchParams.get('i'))
    }
    if (path === '/stall') return
    if (path === '/stream') return res.write('partial body')
    if (path === '/echo') return res.end(req.headers.referer ?? '')
    if (path === '/upload') return req.pipe(res)
    if (path.endsWith('.wasm')) {
      res.setHeader('Content-Type', 'application/wasm')
      return res.end(Buffer.from([0,97,115,109,1,0,0,0]))
    }
    res.setHeader('Content-Type', 'text/javascript')
    if (path === '/engine/pkg/sandbox_worker.bundle.js') return res.end(sandbox)
    const modules = ['/engine/sandbox_host.js', '/service_worker.js', '/preview_realm.js']
    if (!modules.includes(path)) return res.writeHead(404).end()
    res.end(await readFile(new URL(path.slice(1), web)))
  })
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve))
  t.after(() => { server.closeAllConnections(); return new Promise(resolve => server.close(resolve)) })
  const browser = await chromium.launch({ executablePath: process.env.CHROME_BIN, timeout: 90_000 })
  t.after(() => browser.close())
  t.diagnostic(`browser launched at ${elapsed()}`)
  const origin = `http://127.0.0.1:${server.address().port}`
  async function openPage(page) {
    await page.goto(origin)
    await page.evaluate(async () => {
      const { createSandboxHost } = await import('/engine/sandbox_host.js')
      const onWorkerCrash = event => { event.preventDefault(); window.workerError = event.message }
      window.client = createSandboxHost({ onWorkerCrash })
      window.server = createSandboxHost({ onWorkerCrash, role: 'server' })
    })
    return page
  }
  // Without the service worker: the sandbox script carries no policy of its own.
  const page = await openPage(await browser.newPage())
  async function spawn(role, from = page) {
    const ready = from.waitForEvent('worker')
    await from.evaluate(role => window[role].spawn(), role)
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

  await t.test('the service worker keeps scene code from loading modules', async () => {
    const context = await browser.newContext()
    t.after(() => context.close())
    const controlled = await openPage(await context.newPage())
    await controlled.evaluate(async () => {
      if (navigator.serviceWorker.controller) return
      const changed = new Promise(resolve => navigator.serviceWorker.addEventListener('controllerchange', resolve, { once: true }))
      await navigator.serviceWorker.register('/service_worker.js')
      await changed
    })
    // Scene code runs as Function code in the worker realm (sandbox_worker.js runWithScope).
    const probe = (worker, url) => worker.evaluate(async ({ url, otherOrigin }) => {
      const scene = new Function('url', 'return import(url).then(() => "loaded", () => "blocked")')
      const socket = new WebSocket(otherOrigin.replace('http', 'ws'))
      const opened = await new Promise(resolve => { socket.onopen = () => resolve(true); socket.onerror = () => resolve(false) })
      socket.close()
      const blob = URL.createObjectURL(new Blob(['blob body']))
      const blobModule = URL.createObjectURL(new Blob(['export default 1'], { type: 'text/javascript' }))
      const owners = (start, name) => {
        let n = 0
        for (let o = start; o != null; o = Object.getPrototypeOf(o)) if (Object.prototype.hasOwnProperty.call(o, name)) n++
        return n
      }
      return {
        imported: await scene(url),
        sameOrigin: await scene('/engine/sandbox_host.js'),
        dataModule: await scene('data:text/javascript,export default 1'),
        blobModule: await scene(blobModule),
        wasm: await WebAssembly.instantiate(new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0])).then(() => 'compiled', e => e.name),
        fetched: await (await fetch('/echo', { referrer: location.origin + '/chosen' })).text(),
        data: await (await fetch('data:text/plain,data body')).text(),
        blob: await (await fetch(blob)).text(),
        opened,
        sent: sends.length,
        // the scene fetch is the global's own property; nothing up the chain holds another
        nativeFetch: owners(Object.getPrototypeOf(self), 'fetch'),
        WebTransport: owners(self, 'WebTransport'),
        url: location.pathname + location.search,
      }
    }, { url, otherOrigin })

    const open = await probe(await spawn('client'), `${otherOrigin}/open.js?scene=data`)
    assert.deepEqual([open.imported, open.dataModule, open.blobModule], ['loaded', 'loaded', 'loaded'])
    assert.deepEqual(imports, ['/open.js?scene=data'])

    for (const role of ['client', 'server']) {
      const result = await probe(await spawn(role, controlled), `${otherOrigin}/blocked.js?scene=data`)
      assert.deepEqual(result, {
        imported: 'blocked', sameOrigin: 'blocked', dataModule: 'blocked', blobModule: 'blocked', wasm: 'compiled',
        fetched: new URL('/chosen', origin).href, data: 'data body', blob: 'blob body', opened: true,
        sent: 3, nativeFetch: 0, WebTransport: 0,
        url: `/engine/pkg/sandbox_worker.bundle.js${role === 'server' ? '?server' : ''}`,
      })
    }
    assert.deepEqual(imports, ['/open.js?scene=data'])
  })
})
