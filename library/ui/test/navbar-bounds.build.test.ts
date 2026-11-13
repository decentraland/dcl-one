import { mkdtemp, readFile, stat } from 'node:fs/promises'
import { createServer, type Server } from 'node:http'
import type { AddressInfo } from 'node:net'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { build } from 'vite'
import { afterAll, beforeAll, describe, expect, it } from 'vitest'
import { findBrowser, openSession } from '../figma/review/scripts/capture-browser.mjs'
import { releaseHarness } from './release-harness'
import { nativeNavBarCases } from './consumer/navbar-bounds-cases'

const root = path.resolve(import.meta.dirname, '..')
const page = '/test/consumer/navbar-bounds.html'
const tolerance = 1
const mime: Record<string, string> = {
  '.html': 'text/html',
  '.js': 'text/javascript',
  '.css': 'text/css',
  '.svg': 'image/svg+xml',
  '.woff2': 'font/woff2'
}
let scratch: string
let server: Server
let origin: string
let session: Awaited<ReturnType<typeof openSession>>
async function evaluate<T>(expression: string): Promise<T> {
  const result = (await session.send('Runtime.evaluate', {
    expression,
    awaitPromise: true,
    returnByValue: true
  })) as {
    result: { value: T }
    exceptionDetails?: { text: string; exception?: { description?: string } }
  }
  if (result.exceptionDetails)
    throw new Error(result.exceptionDetails.exception?.description ?? result.exceptionDetails.text)
  return result.result.value
}
beforeAll(async () => {
  scratch = await mkdtemp(path.join(tmpdir(), 'native-navbar-bounds-'))
  const outDir = path.join(scratch, 'out')
  await build({
    configFile: false,
    root,
    logLevel: 'silent',
    build: { outDir, emptyOutDir: true, rolldownOptions: { input: path.join(root, page) } }
  })
  server = createServer((request, response) => {
    void (async () => {
      const file = path.resolve(
        outDir,
        '.' + decodeURIComponent(new URL(request.url ?? '/', 'http://fixture.invalid').pathname)
      )
      if (file !== outDir && !file.startsWith(outDir + path.sep))
        return void response.writeHead(403).end()
      try {
        if (!(await stat(file)).isFile()) return void response.writeHead(404).end()
        response
          .writeHead(200, {
            'content-type': mime[path.extname(file)] ?? 'application/octet-stream'
          })
          .end(await readFile(file))
      } catch {
        response.writeHead(404).end()
      }
    })()
  })
  await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve))
  origin = `http://127.0.0.1:${(server.address() as AddressInfo).port}`
  session = await openSession(
    await findBrowser([
      process.env.BROWSER_PATH,
      process.env.CHROME_PATH,
      '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
      '/usr/bin/google-chrome',
      '/usr/bin/chromium-browser',
      '/usr/bin/chromium'
    ])
  )
  await session.send('Page.enable')
}, 300_000)
afterAll(() => releaseHarness({ session, server, scratch }))
async function openCase(id: string, width: number) {
  await session.send('Emulation.setDeviceMetricsOverride', {
    width,
    height: 900,
    deviceScaleFactor: 1,
    mobile: false
  })
  await session.navigate(`${origin}${page}?case=${encodeURIComponent(id)}`)
  await evaluate(
    `new Promise((resolve,reject)=>{const start=Date.now();const poll=()=>{if(!document.querySelector('#native-bounds-root>nav'))return Date.now()-start>15000?reject(Error('Native NavBar missing')):setTimeout(poll,25);document.fonts.ready.then(()=>requestAnimationFrame(()=>requestAnimationFrame(resolve)))};poll()})`
  )
}
async function click(selector: string) {
  const point = await evaluate<{ x: number; y: number }>(
    `(()=>{const el=document.querySelector(${JSON.stringify(selector)});if(!el)throw Error('Control missing');const b=el.getBoundingClientRect();return{x:b.x+b.width/2,y:b.y+b.height/2}})()`
  )
  for (const type of ['mousePressed', 'mouseReleased'])
    await session.send('Input.dispatchMouseEvent', {
      type,
      button: 'left',
      clickCount: 1,
      ...point
    })
  await evaluate('new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)))')
}
async function key(value: string, code: number) {
  for (const type of ['keyDown', 'keyUp'])
    await session.send('Input.dispatchKeyEvent', {
      type,
      key: value,
      code: value,
      windowsVirtualKeyCode: code
    })
  await evaluate('new Promise(resolve=>requestAnimationFrame(resolve))')
}
type Bounds = {
  width: number
  height: number
  overflow: { name: string; right: number; bottom: number }[]
}
async function bounds(selector: string, vertical = true): Promise<Bounds> {
  return evaluate(
    `(()=>{const root=document.querySelector(${JSON.stringify(selector)});if(!root)throw Error('Bounds root missing');const box=root.getBoundingClientRect();const overflow=[...root.querySelectorAll('*')].map(el=>{const b=el.getBoundingClientRect(),s=getComputedStyle(el);return{name:el.getAttribute('aria-label')||el.tagName,right:b.right-box.right,bottom:b.bottom-box.bottom,left:box.left-b.left,top:box.top-b.top,w:b.width,h:b.height,visible:s.visibility!=='hidden'&&s.display!=='none'}}).filter(el=>el.visible&&el.w>0&&el.h>0&&(el.right>${tolerance}||el.left>${tolerance}${vertical ? `||el.top>${tolerance}||el.bottom>${tolerance}` : ''}));return{width:box.width,height:box.height,overflow}})()`
  )
}
describe('native public NavBar bounds, independently of captured Review selection', () => {
  it('should cover every native auth, surface and balance mode plus app composition', () => {
    expect(nativeNavBarCases).toHaveLength(18)
    expect(new Set(nativeNavBarCases.map((value) => value.id)).size).toBe(18)
  })
  for (const width of [390, 768, 1024, 1440])
    it.each(nativeNavBarCases)(
      '$id keeps closed-header descendants inside their root at ' + width + 'px',
      async (example) => {
        await openCase(example.id, width)
        const result = await bounds('#native-bounds-root>nav')
        expect(result.width).toBeGreaterThan(0)
        expect(result.height).toBeGreaterThan(0)
        expect(result.overflow).toEqual([])
        expect(session.runtimeErrors).toEqual([])
      },
      60_000
    )
  for (const width of [390, 768])
    it(
      'should keep the composed drawer inside the viewport and preserve keyboard containment at ' +
        width +
        'px',
      async () => {
        await openCase('in-composition', width)
        await click('button[aria-label="Open menu"]')
        expect(await evaluate('!!document.activeElement?.closest(\'[role="dialog"]\')')).toBe(true)
        const result = await bounds('[role="dialog"]', false)
        expect(result.overflow).toEqual([])
        expect(
          await evaluate(
            `(()=>{const b=document.querySelector('[role="dialog"]').getBoundingClientRect();return b.left>=-1&&b.top>=-1&&b.right<=innerWidth+1&&b.bottom<=innerHeight+1})()`
          )
        ).toBe(true)
        for (let i = 0; i < 12; i++) {
          await key('Tab', 9)
          expect(await evaluate('!!document.activeElement?.closest(\'[role="dialog"]\')')).toBe(
            true
          )
        }
        await key('Escape', 27)
        expect(await evaluate('document.querySelector(\'[role="dialog"]\')===null')).toBe(true)
        expect(await evaluate('document.activeElement?.getAttribute("aria-label")')).toBe(
          'Open menu'
        )
        expect(session.runtimeErrors).toEqual([])
      },
      60_000
    )
  it('should bound native section and account popups by the viewport and return focus on Escape', async () => {
    await openCase('in-solid-3', 1440)
    for (const selector of [
      'button[aria-controls]:not([aria-label])',
      'button[aria-label="Account: Ada"]'
    ]) {
      await click(selector)
      expect(
        await evaluate(
          `(()=>{const el=document.querySelector(${JSON.stringify(selector)});const panel=document.getElementById(el.getAttribute('aria-controls'));if(!panel||panel.hidden)return false;const b=panel.getBoundingClientRect();return b.width>0&&b.left>=-1&&b.right<=innerWidth+1&&b.top>=-1&&b.bottom<=innerHeight+1})()`
        )
      ).toBe(true)
      await key('Escape', 27)
      expect(
        await evaluate(
          `document.activeElement===document.querySelector(${JSON.stringify(selector)})`
        )
      ).toBe(true)
    }
    expect(session.runtimeErrors).toEqual([])
  }, 60_000)
})
