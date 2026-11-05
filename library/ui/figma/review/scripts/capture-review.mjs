import { captureScope, preserveFrozenCases } from './capture-scope.mjs'
import process from 'node:process'
import { Buffer } from 'node:buffer'
import { setTimeout, clearTimeout } from 'node:timers'
import { URL } from 'node:url'
const { fetch, WebSocket, console } = globalThis
import { spawn } from 'node:child_process'
import { reviewSignature } from './review-signature.mjs'
import {
  CaptureTimeout,
  assertNodeVersion,
  captureWithRetry,
  createRetryBudget,
  recoverInterruptedCommit,
  withStagedOutput
} from './capture-retry.mjs'
import { createServer } from 'node:http'
import { readFile, writeFile, mkdtemp, rm, access } from 'node:fs/promises'
import path from 'node:path'
import os from 'node:os'

// Capture the actual React output. Figma references must already exist; this never syncs Figma.
const root = path.resolve(import.meta.dirname, '..')
// Upstream pins Node in a repository .node-version; here the library pins it in library/ui/package.json
// (engines.node, ">=24"), two levels above the Review.
assertNodeVersion(
  String(
    JSON.parse(await readFile(path.join(root, '../../package.json'), 'utf8')).engines.node
  ).replace(/^>=\s*/, ''),
  process.versions.node
)
const output = path.join(root, 'public/dcl-ui-review')
// An earlier run may have stopped mid-publication. Finish it before reading any evidence it was replacing.
await recoverInterruptedCommit({ output, scratch: root })
const catalog = JSON.parse(await readFile(path.join(root, 'src/review/catalog.json'), 'utf8'))
const references = JSON.parse(await readFile(path.join(root, 'src/review/references.json'), 'utf8'))
const requestedPages = process.env.REVIEW_CAPTURE_PAGE_IDS
const scope = captureScope(
  catalog,
  requestedPages === undefined
    ? {}
    : JSON.parse(await readFile(path.join(root, '../figma.config.json'), 'utf8')),
  requestedPages
)
let previous
if (scope.frozen.length)
  previous = JSON.parse(await readFile(path.join(output, 'fidelity.json'), 'utf8'))
const frozenCases = preserveFrozenCases(scope.frozen, previous)
for (const evidence of Object.values(frozenCases))
  await readFile(path.join(root, 'public', evidence.image))
const staticRoot = path.join(root, 'review-dist')
const built = JSON.parse(
  await readFile(path.join(staticRoot, 'dcl-ui-review/source-signature.json'), 'utf8')
)
if (built.signature !== (await reviewSignature(root)))
  throw new Error('Build the current review before capturing pixel evidence.')
const mime = {
  '.html': 'text/html',
  '.js': 'text/javascript',
  '.css': 'text/css',
  '.json': 'application/json',
  '.png': 'image/png',
  '.svg': 'image/svg+xml',
  '.woff2': 'font/woff2',
  '.woff': 'font/woff'
}
const server = createServer(async (request, response) => {
  try {
    const pathname = decodeURIComponent(new URL(request.url, 'http://localhost').pathname)
    const file = path.resolve(
      staticRoot,
      '.' + pathname + (pathname.endsWith('/') ? 'index.html' : '')
    )
    if (!file.startsWith(staticRoot + path.sep)) throw new Error('Invalid path')
    response.setHeader('Content-Type', mime[path.extname(file)] ?? 'application/octet-stream')
    response.end(await readFile(file))
  } catch {
    response.writeHead(404)
    response.end('Not found')
  }
})
await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve))
const origin = `http://127.0.0.1:${server.address().port}`
let executable
for (const candidate of [
  process.env.BROWSER_PATH,
  process.env.CHROME_PATH,
  '/Applications/Brave Browser.app/Contents/MacOS/Brave Browser',
  '/usr/bin/brave-browser',
  '/usr/bin/brave',
  '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
  '/usr/bin/google-chrome',
  '/usr/bin/chromium',
  '/usr/bin/chromium-browser'
].filter(Boolean)) {
  try {
    await access(candidate)
    executable = candidate
    break
  } catch {
    /* Try the next installed browser. */
  }
}
if (!executable) {
  server.close()
  throw new Error('A Chromium browser is required. Set BROWSER_PATH to the Brave executable.')
}
const delay = (ms) => new Promise((resolve) => setTimeout(resolve, ms))

// One browser with one DevTools tab. A stalled browser is closed and replaced by a fresh session.
async function openSession() {
  const profile = await mkdtemp(path.join(os.tmpdir(), 'dcl-visual-'))
  const browser = spawn(
    executable,
    [
      '--headless=new',
      '--no-sandbox',
      '--disable-extensions',
      '--disable-background-networking',
      '--force-color-profile=srgb',
      '--remote-debugging-port=0',
      '--user-data-dir=' + profile
    ],
    { stdio: 'ignore' }
  )
  let socket
  const close = async () => {
    socket?.close()
    browser.kill('SIGTERM')
    // A hung browser can ignore SIGTERM; never let teardown hide the capture error behind the job timeout.
    await new Promise((resolve) => {
      if (browser.exitCode !== null || browser.signalCode !== null) return resolve()
      const kill = setTimeout(() => browser.kill('SIGKILL'), 5000)
      browser.once('exit', () => {
        clearTimeout(kill)
        resolve()
      })
    })
    // Browser helper processes can still write to the profile for a moment after the browser exits.
    await rm(profile, { recursive: true, force: true, maxRetries: 10, retryDelay: 200 })
  }
  try {
    let port
    for (let i = 0; i < 100; i++) {
      try {
        port = (await readFile(path.join(profile, 'DevToolsActivePort'), 'utf8')).split('\n')[0]
        break
      } catch {
        await delay(100)
      }
    }
    if (!port) throw new Error('Capture browser did not start')
    const tab = await fetch(`http://127.0.0.1:${port}/json/new?about:blank`, {
      method: 'PUT'
    }).then((response) => response.json())
    socket = new WebSocket(tab.webSocketDebuggerUrl)
    await new Promise((resolve, reject) => {
      socket.addEventListener('open', resolve, { once: true })
      socket.addEventListener('error', reject, { once: true })
    })
    let serial = 0
    const pending = new Map()
    const session = { runtimeErrors: [], close }
    socket.addEventListener('message', (event) => {
      const message = JSON.parse(event.data)
      if (message.id) {
        pending.get(message.id)?.(message)
        pending.delete(message.id)
      } else if (message.method === 'Runtime.exceptionThrown')
        session.runtimeErrors.push(
          message.params.exceptionDetails.exception?.description ??
            message.params.exceptionDetails.text
        )
    })
    session.send = (method, params = {}) =>
      new Promise((resolve, reject) => {
        const id = ++serial
        const timeout = setTimeout(() => {
          pending.delete(id)
          reject(new CaptureTimeout('CDP timeout: ' + method))
        }, 30000)
        pending.set(id, (message) => {
          clearTimeout(timeout)
          if (message.error) reject(new Error(JSON.stringify(message.error)))
          else resolve(message.result)
        })
        socket.send(JSON.stringify({ id, method, params }))
      })
    session.evaluate = async (expression) => {
      const result = await session.send('Runtime.evaluate', {
        expression,
        awaitPromise: true,
        returnByValue: true
      })
      if (result.exceptionDetails)
        throw new Error(
          result.exceptionDetails.exception?.description ?? result.exceptionDetails.text
        )
      return result.result?.value
    }
    await session.send('Page.enable')
    await session.send('Runtime.enable')
    await session.send('Emulation.setDefaultBackgroundColorOverride', {
      color: { r: 0, g: 0, b: 0, a: 0 }
    })
    return session
  } catch (error) {
    await close()
    throw error
  }
}

async function captureVariant(session, staging, component, index, variant, key, reference) {
  const { send, evaluate } = session
  session.runtimeErrors = []
  const runtimeErrors = session.runtimeErrors
  await send('Emulation.setDeviceMetricsOverride', {
    width: Math.ceil(Math.max(1000, reference.width + 600)),
    height: Math.ceil(Math.max(900, reference.height + 600)),
    deviceScaleFactor: 1,
    mobile: false
  })
  await send('Page.navigate', {
    url: `${origin}/dcl-ui-review/?audit=1&component=${encodeURIComponent(component.name)}&variant=${index}`
  })
  let ready = false
  for (let attempt = 0; attempt < 150; attempt++) {
    ready = await evaluate('window.__AUDIT_READY__===true')
    if (ready) break
    await delay(100)
  }
  if (!ready) throw new CaptureTimeout('Preview did not become ready')
  const box = await evaluate(
    `(async()=>{await document.fonts.ready;await Promise.all([...document.images].map(image=>image.decode()));for(const element of [document.documentElement,document.body,document.querySelector('#audit-stage')]){element.style.background='transparent';element.style.overflow='visible'}const stage=document.querySelector('#audit-stage');stage.style.padding='160px';stage.style.width='max-content';stage.style.maxWidth='none';await new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)));const root=document.querySelector('#audit-render')?.firstElementChild;if(!root)throw Error('Rendered root missing');const b=root.getBoundingClientRect();return{x:b.x,y:b.y,width:b.width,height:b.height}})()`
  )
  if (runtimeErrors.length) throw new Error(`${key}: ${runtimeErrors.join('\n')}`)
  if (Math.abs(box.width - variant.width) > 1 || Math.abs(box.height - variant.height) > 1)
    throw new Error(
      `${key}: expected ${variant.width}x${variant.height}, rendered ${box.width}x${box.height}`
    )
  const screenshot = await send('Page.captureScreenshot', {
    format: 'png',
    captureBeyondViewport: true,
    clip: {
      x: box.x + (reference.offsetX ?? 0),
      y: box.y + (reference.offsetY ?? 0),
      width: reference.width,
      height: reference.height,
      scale: 1
    }
  })
  await writeFile(
    path.join(staging, 'renders', key + '.png'),
    Buffer.from(screenshot.data, 'base64')
  )
  const measurement = await evaluate(
    `(async()=>{const images=await Promise.all([${JSON.stringify(reference.file)},${JSON.stringify('data:image/png;base64,' + screenshot.data)}].map(async src=>{const image=new Image();image.src=src;await image.decode();return image}));const[w,h]=[images[0].naturalWidth,images[0].naturalHeight];if(images[1].naturalWidth!==w||images[1].naturalHeight!==h)throw Error('Image dimensions differ');const canvas=document.createElement('canvas');canvas.width=w;canvas.height=h;const context=canvas.getContext('2d');const arrays=images.map(image=>{context.fillStyle='#1e1e1e';context.fillRect(0,0,w,h);context.drawImage(image,0,0);return context.getImageData(0,0,w,h).data});let differentPixels=0,significantPixels=0,error=0;for(let i=0;i<arrays[0].length;i+=4){let max=0;for(let c=0;c<3;c++){const delta=Math.abs(arrays[0][i+c]-arrays[1][i+c]);max=Math.max(max,delta);error+=delta}if(max>0)differentPixels++;if(max>16)significantPixels++}return{differentPixels,totalPixels:w*h,differencePercent:differentPixels/(w*h)*100,significantPercent:significantPixels/(w*h)*100,meanChannelError:error/(w*h*3)}})()`
  )
  return {
    sourceId: variant.id,
    width: reference.width,
    height: reference.height,
    ...measurement,
    image: `/dcl-ui-review/renders/${key}.png`,
    status: measurement.differentPixels ? 'different' : 'exact'
  }
}

const report = {
  generatedAt: new Date().toISOString(),
  signature: await reviewSignature(root),
  renderer: `${/brave/i.test(executable) ? 'Brave' : 'Chromium'}, sRGB, transparent capture composited on #1e1e1e`,
  scale: 1,
  cases: frozenCases
}
let session
try {
  if (previous && (previous.renderer !== report.renderer || previous.scale !== report.scale))
    throw new Error('Frozen evidence uses a different renderer or scale')
  session = await openSession()
  const budget = createRetryBudget()
  const restart = async () => {
    await session.close()
    session = undefined
    session = await openSession()
  }
  await withStagedOutput({ output, scratch: root }, async (staging) => {
    for (const component of scope.selected) {
      for (const [index, variant] of component.variants.entries()) {
        const key = `${component.name}-${index}`
        const reference = references[key]
        if (!reference) throw new Error('Missing original Figma reference: ' + key)
        report.cases[key] = await captureWithRetry(
          key,
          () => captureVariant(session, staging, component, index, variant, key, reference),
          { budget, restart, log: (line) => console.log(line) }
        )
      }
      console.log(
        component.name + ': ' + component.variants.length + ' source comparisons captured'
      )
    }
    await writeFile(path.join(staging, 'fidelity.json'), JSON.stringify(report, null, 2) + '\n')
  })
  console.log(
    JSON.stringify({
      cases: Object.keys(report.cases).length,
      exact: Object.values(report.cases).filter((item) => item.status === 'exact').length,
      different: Object.values(report.cases).filter((item) => item.status === 'different').length
    })
  )
} finally {
  await session?.close()
  server.close()
  // Keep-alive connections from the browser would otherwise keep the process running.
  server.closeAllConnections()
}
