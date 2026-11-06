import assert from 'node:assert/strict'
import { spawn } from 'node:child_process'
import { access, mkdtemp, readFile, rm } from 'node:fs/promises'
import { createServer } from 'node:http'
import os from 'node:os'
import path from 'node:path'
import { setTimeout as delay } from 'node:timers/promises'

const root = path.resolve(process.argv[2] ?? 'library/ui/figma/review/review-dist')
const prefix = '/@scope/package/0.1.0-preview/dcl-ui-review/'
const packagePrefix = prefix.slice(0, -'dcl-ui-review/'.length)
const mime = {
  '.html': 'text/html',
  '.js': 'text/javascript',
  '.css': 'text/css',
  '.json': 'application/json',
  '.png': 'image/png',
  '.svg': 'image/svg+xml',
  '.woff': 'font/woff',
  '.woff2': 'font/woff2'
}
const failures = []
const server = createServer(async (request, response) => {
  try {
    const pathname = new URL(request.url, 'http://localhost').pathname
    assert(pathname.startsWith(packagePrefix), `Request escaped the CDN prefix: ${pathname}`)
    const relative = decodeURIComponent(pathname.slice(packagePrefix.length))
    const file = path.resolve(root, relative + (pathname.endsWith('/') ? 'index.html' : ''))
    assert(file.startsWith(root + path.sep), 'Invalid static path')
    const data = await readFile(file)
    response.setHeader('Content-Type', mime[path.extname(file)] ?? 'application/octet-stream')
    response.end(data)
  } catch (error) {
    // Chromium probes this independently of application requests.
    if (request.url !== '/favicon.ico') failures.push(`${request.url}: ${error.message}`)
    response.writeHead(404)
    response.end('Not found')
  }
})
let browser
let socket
let profile
let browserExit
const pending = new Map()
let nextId = 0
const fetched = new Set()
const loaded = new Set()
const requestUrls = new Map()

async function waitFor(check, label, timeout = 15000) {
  const end = Date.now() + timeout
  while (Date.now() < end) {
    if (failures.length) throw new Error(failures.join('\n'))
    if (await check()) return
    await delay(100)
  }
  throw new Error(`Timed out: ${label}`)
}

function cdp(method, params = {}) {
  return new Promise((resolve, reject) => {
    const id = ++nextId
    const timer = setTimeout(() => {
      pending.delete(id)
      reject(new Error(`DevTools timeout: ${method}`))
    }, 10000)
    pending.set(id, { resolve, reject, timer })
    socket.send(JSON.stringify({ id, method, params }))
  })
}

async function evaluate(expression) {
  const result = await cdp('Runtime.evaluate', {
    expression,
    returnByValue: true,
    awaitPromise: true
  })
  assert(!result.exceptionDetails, JSON.stringify(result.exceptionDetails))
  return result.result.value
}

try {
  let executable
  for (const candidate of [
    process.env.BROWSER_PATH,
    process.env.CHROME_PATH,
    '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    '/Applications/Brave Browser.app/Contents/MacOS/Brave Browser',
    '/usr/bin/google-chrome',
    '/usr/bin/chromium',
    '/usr/bin/chromium-browser'
  ].filter(Boolean)) {
    try {
      await access(candidate)
      executable = candidate
      break
    } catch {
      /* Try the next browser. */
    }
  }
  assert(executable, 'A Chromium browser is required; set BROWSER_PATH')
  const fidelity = JSON.parse(
    await readFile(path.join(root, 'dcl-ui-review/fidelity.json'), 'utf8')
  )
  const sample = Object.entries(fidelity.cases).find(
    ([key, value]) => key.endsWith('-0') && value.status !== 'blocked' && value.image
  )
  assert(sample, 'The reviewer must include a captured component and render image')
  const component = sample[0].slice(0, -2)
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve))
  const origin = `http://127.0.0.1:${server.address().port}`
  profile = await mkdtemp(path.join(os.tmpdir(), 'web-preview-chrome-'))
  browser = spawn(
    executable,
    [
      '--headless=new',
      '--no-sandbox',
      '--disable-extensions',
      '--disable-background-networking',
      '--remote-debugging-port=0',
      '--user-data-dir=' + profile
    ],
    { stdio: 'ignore' }
  )
  browserExit = new Promise((resolve) => {
    browser.once('exit', resolve)
    browser.once('error', resolve)
  })
  browser.on('error', (error) => failures.push(error.message))
  let port
  await waitFor(async () => {
    try {
      port = (await readFile(path.join(profile, 'DevToolsActivePort'), 'utf8')).split('\n')[0]
    } catch {
      return false
    }
    return Boolean(port)
  }, 'Chromium startup')
  const tab = await fetch(`http://127.0.0.1:${port}/json/new?about:blank`, {
    method: 'PUT'
  }).then((response) => response.json())
  socket = new WebSocket(tab.webSocketDebuggerUrl)
  await new Promise((resolve, reject) => {
    socket.addEventListener('open', resolve, { once: true })
    socket.addEventListener('error', reject, { once: true })
  })
  socket.addEventListener('message', (event) => {
    const message = JSON.parse(event.data)
    if (message.id) {
      const callback = pending.get(message.id)
      if (!callback) return
      pending.delete(message.id)
      clearTimeout(callback.timer)
      if (message.error) callback.reject(new Error(JSON.stringify(message.error)))
      else callback.resolve(message.result)
    } else if (message.method === 'Network.requestWillBeSent') {
      const url = message.params.request.url
      requestUrls.set(message.params.requestId, url)
      if (/^https?:/.test(url) && url !== origin + '/favicon.ico') {
        fetched.add(url)
        if (!url.startsWith(origin + packagePrefix))
          failures.push(`Request escaped the CDN: ${url}`)
      }
    } else if (message.method === 'Network.responseReceived') {
      const response = message.params.response
      if (response.status >= 400 && !response.url.endsWith('/favicon.ico')) {
        failures.push(`HTTP ${response.status}: ${response.url}`)
      }
    } else if (message.method === 'Network.loadingFinished') {
      loaded.add(requestUrls.get(message.params.requestId))
    } else if (message.method === 'Runtime.exceptionThrown') {
      failures.push(JSON.stringify(message.params.exceptionDetails))
    }
  })
  await cdp('Page.enable')
  await cdp('Runtime.enable')
  await cdp('Network.enable')
  const entry = `${origin}${prefix}index.html?platform=all&component=${encodeURIComponent(component)}&variant=0`
  await cdp('Page.navigate', { url: entry })
  await waitFor(
    () =>
      evaluate(`document.querySelector('main h1')?.textContent === ${JSON.stringify(component)} &&
    [...document.images].some(image => image.alt.includes('original Figma variant') && image.naturalWidth > 0)`),
    'reviewer component and decoded Figma reference'
  )
  await waitFor(
    () => [...loaded].some((url) => url?.endsWith('/dcl-ui-review/fidelity.json')),
    'version-scoped fidelity fetch'
  )
  const links = await evaluate(`[...document.querySelectorAll('nav a')].map(link => link.href)`)
  assert(
    links.length > 0 && links.every((url) => url.startsWith(origin + prefix)),
    'Navigation must remain inside the versioned reviewer path'
  )
  await evaluate(`(() => {
    const view = document.querySelector('select[name="comparison-view"]');
    view.value = 'difference'; view.dispatchEvent(new Event('change', { bubbles: true }));
  })()`)
  await waitFor(
    async () =>
      [...loaded].some((url) => url?.includes('/dcl-ui-review/renders/')) &&
      (await evaluate(`(() => {
      const canvas = document.querySelector('canvas[aria-label="Pixel difference against the original Figma variant"]');
      return canvas && canvas.getContext('2d').getImageData(0, 0, 1, 1).data[3] > 0;
    })()`)),
    'pixel difference render'
  )
  const target = links.find((url) => new URL(url).searchParams.get('component') !== component)
  assert(target, 'The reviewer must offer component navigation')
  await evaluate(`document.querySelector('nav a[href]') &&
    [...document.querySelectorAll('nav a')].find(link => link.href === ${JSON.stringify(target)}).click()`)
  const targetName = new URL(target).searchParams.get('component')
  await waitFor(
    () =>
      evaluate(`location.href === ${JSON.stringify(target)} &&
    document.querySelector('main h1')?.textContent === ${JSON.stringify(targetName)}`),
    'component navigation after click'
  )
  assert.equal(failures.length, 0, failures.join('\n'))
  assert(
    [...fetched].some((url) => /\/assets\/.*\.js/.test(url)),
    'No bundled JavaScript requested'
  )
  console.log(
    'CDN path verified: entry, assets, navigation, fidelity, Figma reference and render image'
  )
} finally {
  for (const callback of pending.values()) {
    clearTimeout(callback.timer)
    callback.reject(new Error('Browser session closed'))
  }
  socket?.close()
  if (browser && browser.exitCode === null && browser.signalCode === null) {
    browser.kill('SIGTERM')
    const kill = setTimeout(() => browser.kill('SIGKILL'), 5000)
    await browserExit
    clearTimeout(kill)
  }
  server.closeAllConnections()
  if (server.listening) await new Promise((resolve) => server.close(resolve))
  if (profile)
    await rm(profile, {
      recursive: true,
      force: true,
      maxRetries: 10,
      retryDelay: 200
    })
}
