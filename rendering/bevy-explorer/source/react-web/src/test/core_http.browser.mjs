// CHROME_BIN optionally names an installed Chromium. Run with node --test src/test/core_http.browser.mjs from react-web.
import assert from 'node:assert/strict'
import test from 'node:test'
import { createServer } from 'node:http'
import { readFile } from 'node:fs/promises'
import { stripTypeScriptTypes } from 'node:module'
import { chromium } from '@playwright/test'

const engine = new URL('../../../deploy/web/engine/', import.meta.url)
const hud = new URL('../lib/http.ts', import.meta.url)

// Import and re-export the same synchronous functions as the engine's wasm-bindgen glue.
function wasmFixture() {
  const name = s => [s.length, ...Buffer.from(s)]
  const section = (tag, bytes) => [tag, bytes.length, ...bytes]
  return Buffer.from([0, 97, 115, 109, 1, 0, 0, 0,
    ...section(1, [1, 96, 1, 127, 0]),
    ...section(2, [2, ...name('http'), ...name('beginCoreHttp'), 0, 0, ...name('http'), ...name('endCoreHttp'), 0, 0]),
    ...section(7, [2, ...name('begin'), 0, 0, ...name('end'), 0, 1])])
}

async function workerMain() {
  const http = await import('/engine/core_http.js')
  const { instance } = await WebAssembly.instantiateStreaming(fetch('/hooks.wasm'), {http})
  onmessage = async ({data}) => {
    if (data === 'error') {
      setTimeout(() => { throw new Error('recoverable fixture error') })
    } else if (data === 'close') {
      self.close()
    } else if (data.wasm) {
      instance.exports.begin(1)
      try { await (await fetch(data.wasm)).text() } finally { instance.exports.end(1) }
    } else {
      postMessage(await http.coreFetch(data.url, undefined, r => r.text()))
    }
  }
  postMessage('ready')
}

test('page, HUD shell and trusted workers share one advisory snapshot across live transfers', {timeout: 30000}, async () => {
  const held = new Map()
  const errorClosed = Promise.withResolvers()
  const statusClosed = Promise.withResolvers()
  const servers = []
  let browser
  try {
    // Split slow transfers across ports to stay below Chromium's HTTP/1 socket cap per origin.
    for (let i = 0; i < 3; i++) {
      const server = createServer(async (req, res) => {
        res.setHeader('Access-Control-Allow-Origin', '*')
        res.setHeader('Cache-Control', 'no-store')
        if (req.url === '/slow-error' || req.url === '/slow-status') {
          res.writeHead(503)
          res.write('open body')
          res.on('close', req.url === '/slow-error' ? errorClosed.resolve : statusClosed.resolve)
        } else if (req.url.startsWith('/hold/')) {
          res.write('start')
          held.set(req.url, res)
        } else if (req.url === '/engine/core_http.js') {
          res.writeHead(200, {'Content-Type':'text/javascript'})
          res.end(await readFile(new URL('core_http.js', engine)))
        } else if (req.url === '/hud.js') {
          res.writeHead(200, {'Content-Type':'text/javascript'})
          res.end(stripTypeScriptTypes(await readFile(hud, 'utf8')).replaceAll('../../../deploy/web/engine/', '/engine/'))
        } else if (req.url === '/hooks.wasm') {
          res.writeHead(200, {'Content-Type':'application/wasm'})
          res.end(wasmFixture())
        } else if (req.url === '/worker.js') {
          res.writeHead(200, {'Content-Type':'text/javascript'})
          res.end('await (' + workerMain.toString() + ')()')
        } else {
          res.writeHead(200, {'Content-Type':'text/html'})
          res.end('<!doctype html><title>fixture</title>')
        }
      })
      await new Promise(resolve => server.listen(0, '127.0.0.1', resolve))
      servers.push(server)
    }
    const origins = servers.map(s => 'http://127.0.0.1:' + s.address().port)
    browser = await chromium.launch({executablePath: process.env.CHROME_BIN || undefined})
    const page = await browser.newPage()
    page.setDefaultTimeout(5000)
    await page.goto(origins[0])
    await page.evaluate(async () => {
      const shell = await import('/hud.js')
      const connected = new Promise(resolve => {
        addEventListener('message', e => {
          if (e.source !== frame.contentWindow || e.origin !== location.origin || e.data?.type !== 'bevy-shell:http') return
          shell.connectShellHttp(e.ports[0])
          resolve()
        })
      })
      const frame = document.createElement('iframe')
      frame.src = '/hud.html'
      const loaded = new Promise(resolve => frame.onload = resolve)
      document.body.append(frame)
      await loaded
      const hud = await frame.contentWindow.eval("import('/hud.js')")
      hud.installHudHttp(true)
      await connected
      window.app = frame.contentWindow
      window.http = await app.eval("import('/engine/core_http.js')")
      window.shell = shell
      window.startWorker = async () => {
        const worker = new Worker('/worker.js', {type:'module'})
        http.attachCoreWorker(worker)
        await new Promise(resolve => worker.addEventListener('message', e => { if (e.data === 'ready') resolve() }))
        return worker
      }
      window.worker = await startWorker()
    })
    await page.evaluate(origins => {
      for (let i = 0; i < 5; i++) {
        http.coreFetch(origins[0] + '/hold/page' + i, undefined, r => r.text())
        shell.hudFetch(origins[1] + '/hold/shell' + i, undefined, r => r.text())
        worker.postMessage({url: origins[2] + '/hold/worker' + i})
      }
    }, origins)
    await page.waitForFunction(() => app.__dclCoreHttp.snapshot().active === 15)
    // Remounting the HUD in the same live engine document must keep the shell's transfers.
    assert.equal(await page.evaluate(async () => {
      const previous = window.__dclCoreHttp
      const hud = await app.eval("import('/hud.js')")
      hud.installHudHttp(true)
      const delivered = new Promise(resolve => {
        const check = event => {
          if (event.data !== 'after-hud-mount') return
          removeEventListener('message', check)
          resolve()
        }
        addEventListener('message', check)
      })
      app.eval("parent.postMessage('after-hud-mount', location.origin)")
      await delivered
      return previous === window.__dclCoreHttp && app.__dclCoreHttp.snapshot().active === 15
    }), true)
    await page.evaluate(origin => worker.postMessage({wasm: origin + '/hold/sixteen'}), origins[2])
    await page.waitForFunction(() => app.__dclCoreHttp.snapshot().active === 16)
    const until = Date.now() + 4000
    while (held.size < 16 && Date.now() < until) await new Promise(r => setTimeout(r, 10))
    assert.equal(held.size, 16, 'all 16 reach the server before any body finishes')
    await page.evaluate(async () => {
      const reported = new Promise(resolve => worker.addEventListener('error', e => { e.preventDefault(); resolve() }, {once:true}))
      worker.postMessage('error')
      await reported
    })
    assert.equal(await page.evaluate(() => app.__dclCoreHttp.snapshot().active), 16)
    held.get('/hold/sixteen').end('wasm done')
    await page.waitForFunction(() => app.__dclCoreHttp.snapshot().active === 15)
    await page.evaluate(() => worker.terminate())
    await page.waitForFunction(() => app.__dclCoreHttp.snapshot().active === 10)
    // A worker can also close itself before its pending reads settle.
    const spawned = page.waitForEvent('worker')
    await page.evaluate(async origin => {
      window.worker = await startWorker()
      worker.postMessage({url: origin + '/hold/closing'})
    }, origins[2])
    await page.waitForFunction(() => app.__dclCoreHttp.snapshot().active === 11)
    const closed = (await spawned).waitForEvent('close')
    await page.evaluate(() => worker.postMessage('close'))
    await closed
    await page.waitForFunction(() => app.__dclCoreHttp.snapshot().active === 10)
    for (const response of held.values()) response.end()
    await page.waitForFunction(() => app.__dclCoreHttp.snapshot().active === 0)
    assert.deepEqual(await page.evaluate(() => app.__dclCoreHttp.snapshot()), { active: 0, highWater: 16, oldestActiveMs: 0, suspectedStarvation: false })

    await assert.rejects(page.evaluate(() => http.coreFetchJson('/slow-error')), /503/)
    await errorClosed.promise
    assert.equal(await page.evaluate(() => http.coreFetch('/slow-status', undefined, r => r.status)), 503)
    await statusClosed.promise
    assert.equal(await page.evaluate(() => app.__dclCoreHttp.snapshot().active), 0)

    // Transports with their own scheduler keep using fetch in the same document.
    await page.evaluate(async () => { window.rawBody = (await app.fetch('/hold/raw')).text() })
    assert.equal(await page.evaluate(() => app.__dclCoreHttp.snapshot().active), 0)
    held.get('/hold/raw').end(' done')
    assert.equal(await page.evaluate(() => rawBody), 'start done')

    // A worker whose owner never attaches a reporter still completes its real fetch.
    assert.equal(await page.evaluate(async () => {
      const unobserved = new Worker('/worker.js', {type:'module'})
      try {
        return await new Promise(resolve => {
          unobserved.onmessage = ({data}) => {
            if (data === 'ready') unobserved.postMessage({url: 'data:text/plain,ok'})
            if (data === 'ok') resolve(data)
          }
        })
      } finally { unobserved.terminate() }
    }), 'ok')
  } finally {
    await browser?.close()
    for (const server of servers) {
      server.closeAllConnections()
      await new Promise(resolve => server.close(resolve))
    }
  }
})
