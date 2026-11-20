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
async function openCase(
  id: string,
  width: number,
  publicNavigation = false,
  height = 900,
  profile: '' | 'balances' | false = false
) {
  await session.send('Emulation.setDeviceMetricsOverride', {
    width,
    height,
    deviceScaleFactor: 1,
    mobile: false
  })
  const params = new URLSearchParams({ case: id })
  if (publicNavigation) params.set('public', '')
  if (profile !== false) params.set('profile', profile)
  await session.navigate(`${origin}${page}?${params}`)
  await evaluate(
    `new Promise((resolve,reject)=>{const start=Date.now();const poll=()=>{if(!document.querySelector('#native-bounds-root>div>nav'))return Date.now()-start>15000?reject(Error('Native NavBar missing')):setTimeout(poll,25);document.fonts.ready.then(()=>requestAnimationFrame(()=>requestAnimationFrame(resolve)))};poll()})`
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
      text: value === 'Enter' && type === 'keyDown' ? '\r' : undefined,
      windowsVirtualKeyCode: code
    })
  await evaluate('new Promise(resolve=>requestAnimationFrame(resolve))')
}
async function waitForExit() {
  await evaluate(
    `Promise.all(Array.from(document.querySelectorAll('[data-exiting]')).flatMap(panel=>panel.getAnimations().map(animation=>animation.finished.catch(()=>{}))))`
  )
  await evaluate('new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)))')
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
        const result = await bounds('#native-bounds-root>div>nav')
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
        const result = await bounds('[role="dialog"] [data-testid="navbar-panel"]', false)
        expect(result.overflow).toEqual([])
        expect(
          await evaluate(
            `(()=>{const b=document.querySelector('[role="dialog"] [data-testid="navbar-panel"]').getBoundingClientRect();return b.left>=-1&&b.top>=-1&&b.right<=innerWidth+1&&b.bottom<=innerHeight+1})()`
          )
        ).toBe(true)
        let panelStops = 0
        for (let i = 0; i < 12; i++) {
          await key('Tab', 9)
          expect(await evaluate('!!document.activeElement?.closest(\'[role="dialog"]\')')).toBe(
            true
          )
          if (
            await evaluate<boolean>(
              '!!document.activeElement?.closest(\'[data-testid="navbar-panel"]\')'
            )
          )
            panelStops++
        }
        expect(panelStops).toBeGreaterThan(0)
        await key('Escape', 27)
        await waitForExit()
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

describe('when the public Sites navigation runs with browser input', () => {
  it('should hold Discover open across its visual gap and on pointer press', async () => {
    await openCase('out-solid-0', 1440, true)
    const points = await evaluate<{ x: number; y: number; bottom: number }>(
      `(()=>{const button=[...document.querySelectorAll('nav button')].find(el=>el.textContent==='Discover');const b=button.getBoundingClientRect();return{x:b.x+b.width/2,y:b.y+b.height/2,bottom:b.bottom}})()`
    )
    for (const y of [points.y, points.bottom + 4, points.bottom + 24]) {
      await session.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: points.x, y })
      expect(
        await evaluate(`!document.querySelector('nav a[href="/places"]').closest('ul').hidden`)
      ).toBe(true)
    }
    await click('nav button[aria-expanded="true"]:not([aria-label])')
    expect(
      await evaluate(`!document.querySelector('nav a[href="/places"]').closest('ul').hidden`)
    ).toBe(true)
    await key('Escape', 27)
    expect(
      await evaluate(
        `document.activeElement.textContent==='Discover'&&document.activeElement.getAttribute('aria-expanded')==='false'`
      )
    ).toBe(true)
  }, 60_000)

  it('should expand Discover on touch and keep the drawer within the viewport', async () => {
    await openCase('out-solid-0', 390, true)
    await click('button[aria-label="Open menu"]')
    const control = await evaluate<{ x: number; y: number }>(
      `(()=>{const b=[...document.querySelectorAll('[data-testid="navbar-panel"] button')].find(el=>el.textContent==='Discover').getBoundingClientRect();return{x:Math.round(b.x+b.width/2),y:Math.round(b.y+b.height/2)}})()`
    )
    await evaluate(
      `Promise.all(document.querySelector('[data-testid="navbar-panel"]').getAnimations().map(animation=>animation.finished))`
    )
    await session.send('Input.dispatchTouchEvent', {
      type: 'touchStart',
      touchPoints: [control]
    })
    await session.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] })
    expect(
      await evaluate(
        `(()=>{const panel=document.querySelector('[data-testid="navbar-panel"]');const link=panel.querySelector('a[href="/places"]');return !link.closest('[inert]')&&panel.getBoundingClientRect().bottom<=innerHeight})()`
      )
    ).toBe(true)
  }, 60_000)

  it.each([390, 485])(
    'should keep Nav Profile anchored to the header above the open drawer at %ipx',
    async (width) => {
      await openCase('in-solid-3', width, true, 900, '')
      const trigger = 'button[aria-label="Account: Ada"]'
      const profileBounds = `(() => {
        const button = document.querySelector(${JSON.stringify(trigger)})
        const panel = document.getElementById(button.getAttribute('aria-controls'))
        const { left, right, top, width } = panel.getBoundingClientRect()
        const action = panel.querySelector('a[href="/profile"]')
        const actionBox = action.getBoundingClientRect()
        const target = document.elementFromPoint(
          actionBox.x + actionBox.width / 2,
          actionBox.y + actionBox.height / 2
        )
        return { left, right, top, width, actionReachable: action.contains(target) }
      })()`
      await click(trigger)
      const closedDrawer = await evaluate<{ width: number; actionReachable: boolean }>(
        profileBounds
      )
      expect(closedDrawer.width).toBeGreaterThan(300)
      expect(closedDrawer.actionReachable).toBe(true)
      await click(trigger)

      await click('button[aria-label="Open menu"]')
      await click(trigger)
      expect(await evaluate('!!document.querySelector(\'[role="dialog"]\')')).toBe(true)
      expect(await evaluate(profileBounds)).toEqual(closedDrawer)
    }
  )

  it('should keep keyboard focus on Places while the pointer crosses Shop', async () => {
    await openCase('out-solid-0', 1440, true)
    await evaluate(`[...document.querySelectorAll('nav button')]
      .find(button => button.textContent === 'Discover').focus()`)
    await key('Enter', 13)
    await key('Tab', 9)
    await key('Tab', 9)
    expect(await evaluate('document.activeElement?.textContent')).toBe('Places')
    const shop = await evaluate<{ x: number; y: number }>(
      `(() => {
        const { x, y, width, height } = [...document.querySelectorAll('nav button')]
          .find(button => button.textContent === 'Shop').getBoundingClientRect()
        return { x: x + width / 2, y: y + height / 2 }
      })()`
    )
    await session.send('Input.dispatchMouseEvent', { type: 'mouseMoved', ...shop })
    expect(await evaluate('document.activeElement?.textContent')).toBe('Places')
    await key('Escape', 27)
    expect(await evaluate('document.activeElement?.textContent')).toBe('Discover')
  })

  it('should show desktop pressed colors while holding the mouse and highlight the current item', async () => {
    await openCase('out-solid-0', 1440, true)
    for (const selector of [
      'nav button[aria-controls]:not([aria-label])',
      'nav a[href="/events"]'
    ]) {
      const point = await evaluate<{ x: number; y: number }>(
        `(() => {
          const { x, y, width, height } = document.querySelector(${JSON.stringify(selector)}).getBoundingClientRect()
          return { x: x + width / 2, y: y + height / 2 }
        })()`
      )
      await session.send('Input.dispatchMouseEvent', { type: 'mouseMoved', ...point })
      await session.send('Input.dispatchMouseEvent', {
        type: 'mousePressed',
        button: 'left',
        clickCount: 1,
        ...point
      })
      const colors = await evaluate<{
        actual: string
        pressed: string
        color: string
        primary: string
      }>(
        `(() => {
          const element = document.querySelector(${JSON.stringify(selector)})
          const probe = document.createElement('span')
          probe.style.backgroundColor = 'var(--dcl-bg-pressed)'
          probe.style.color = 'var(--dcl-text-primary)'
          element.closest('nav').append(probe)
          const expected = getComputedStyle(probe)
          const actual = getComputedStyle(element)
          const values = { actual: actual.backgroundColor, pressed: expected.backgroundColor, color: actual.color, primary: expected.color }
          probe.remove()
          return values
        })()`
      )
      // A release off the control keeps this test on the fixture.
      await session.send('Input.dispatchMouseEvent', {
        type: 'mouseReleased',
        button: 'left',
        clickCount: 1,
        x: 1,
        y: 1
      })
      expect(colors.actual).toBe(colors.pressed)
      if (selector.includes('href')) expect(colors.color).toBe(colors.primary)
      // Hover the trigger again if the release dismissed the panel.
      if (!selector.includes('href')) {
        await session.send('Input.dispatchMouseEvent', { type: 'mouseMoved', ...point })
      }
    }
  })

  it.each([390, 1440])(
    'should keep profile actions in place when MANA first loads at %ipx',
    async (width) => {
      await openCase('in-solid-3', width, true, 900, 'balances')
      await click('button[aria-label="Account: Ada"]')
      const actionTop =
        'document.querySelector(\'nav a[href="/profile"]\').getBoundingClientRect().top'
      const before = await evaluate<number>(actionTop)
      await evaluate(`(async () => {
        window.dispatchEvent(new Event('fixture:load-profile-balances'))
        await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))
      })()`)
      expect(
        await evaluate(`(() => {
        const account = document.querySelector('[aria-label="Account: Ada"]')
        const panel = document.getElementById(account.getAttribute('aria-controls'))
        return !!panel.querySelector('[data-balance="polygonMana"]')
      })()`)
      ).toBe(true)
      expect(await evaluate<number>(actionTop)).toBeCloseTo(before, 0)
    }
  )

  for (const preference of ['no-preference', 'reduce']) {
    it(`should fill the Pixel viewport below the header and honor ${preference} motion`, async () => {
      await session.send('Emulation.setEmulatedMedia', {
        features: [{ name: 'prefers-reduced-motion', value: preference }]
      })
      await session.send('Animation.enable')
      await session.send('Animation.setPlaybackRate', { playbackRate: 0 })
      try {
        await openCase('out-solid-0', 485, true, 926)
        await click('button[aria-label="Open menu"]')
        const reduced = preference === 'reduce'
        expect(
          await evaluate(
            `(()=>{const p=document.querySelector('[data-testid="navbar-panel"]'),b=p.getBoundingClientRect();return{left:b.left,right:b.right,top:b.top,bottom:b.bottom,opacity:Number(getComputedStyle(p).opacity),animations:p.getAnimations().length}})()`
          )
        ).toEqual({
          left: 0,
          right: 485,
          top: 64,
          bottom: 926,
          opacity: reduced ? 1 : 0,
          animations: reduced ? 0 : 1
        })
        await evaluate(
          `document.querySelector('[data-testid="navbar-panel"]').getAnimations().forEach(a=>a.finish())`
        )
        await click('[data-testid="navbar-panel"] button[aria-expanded="false"]')
        const disclosure = `(()=>{const p=document.querySelector('[data-testid="navbar-panel"]'),btn=p.querySelector('button');return document.getElementById(btn.getAttribute('aria-controls'))})()`
        expect(await evaluate(`${disclosure}.getAnimations().length>0`)).toBe(!reduced)
        await evaluate(`${disclosure}.getAnimations().forEach(a=>a.finish())`)
        expect(
          await evaluate(
            `(()=>{const p=document.querySelector('[data-testid="navbar-panel"]'),link=p.querySelector('a[href="/events"]'),b=link.getBoundingClientRect(),list=${disclosure};return{inset:b.left-p.getBoundingClientRect().left,active:link.getAttribute('aria-current'),height:b.height,clipped:list.getBoundingClientRect().height===0}})()`
          )
        ).toEqual({ inset: 28, active: 'page', height: 48, clipped: false })
        await click('[data-testid="navbar-panel"] button[aria-expanded="true"]')
        expect(await evaluate(`${disclosure}.inert`)).toBe(true)
        expect(await evaluate(`${disclosure}.getAnimations().length>0`)).toBe(!reduced)
        await evaluate(`${disclosure}.getAnimations().forEach(a=>a.finish())`)
        expect(await evaluate(`${disclosure}.getBoundingClientRect().height`)).toBe(0)
        await click('button[aria-label="Close menu"]')
        if (!reduced) {
          expect(
            await evaluate(
              `(()=>{const p=document.querySelector('[data-testid="navbar-panel"]'),a=p.getAnimations()[0];a.currentTime=140;return p.inert&&Number(getComputedStyle(p).opacity)<1})()`
            )
          ).toBe(true)
          await evaluate(
            `document.querySelector('[data-testid="navbar-panel"]').getAnimations().forEach(a=>a.finish());new Promise(r=>requestAnimationFrame(r))`
          )
        }
        expect(await evaluate(`document.activeElement.getAttribute('aria-label')`)).toBe(
          'Open menu'
        )
      } finally {
        await session.send('Animation.setPlaybackRate', { playbackRate: 1 })
        await session.send('Emulation.setEmulatedMedia', { features: [] })
      }
    }, 60_000)
  }

  it('should keep all four section titles aligned while switching between Discover and Shop', async () => {
    await openCase('out-solid-0', 485, true, 926)
    const panel = '[data-testid="navbar-panel"]'
    const finishAnimations = () =>
      evaluate(
        `Promise.all(document.querySelector('${panel}').getAnimations({subtree:true}).map(a=>a.finished))`
      )
    const titleGeometry = () =>
      evaluate<number[][]>(
        `Array.from(document.querySelectorAll('${panel} > ul > li > button'),button=>{const b=button.getBoundingClientRect(),label=document.createRange();label.selectNode(button.firstChild);return[b.height,label.getBoundingClientRect().top-b.top]})`
      )
    await click('button[aria-label="Open menu"]')
    await finishAnimations()
    const before = await titleGeometry()
    expect(before).toHaveLength(4)
    expect(before.every((row) => row.join() === before[0]?.join())).toBe(true)
    for (const section of [1, 2, 1]) {
      const toggle = `${panel} > ul > li:nth-child(${section}) > button`
      await click(toggle)
      await finishAnimations()
      expect(
        await evaluate(`document.querySelector('${toggle}').getAttribute('aria-expanded')`)
      ).toBe('true')
      expect(await titleGeometry()).toEqual(before)
    }
  }, 60_000)

  it('should scroll all the way to the final row inside a short viewport', async () => {
    await openCase('out-solid-0', 485, true, 320)
    const panel = '[data-testid="navbar-panel"]'
    const finishAnimations = () =>
      evaluate(
        `Promise.all(document.querySelector('${panel}').getAnimations({subtree:true}).map(a=>a.finished))`
      )
    await click('button[aria-label="Open menu"]')
    await finishAnimations()
    await click(`${panel} > ul > li:nth-child(2) > button`)
    await finishAnimations()
    expect(
      await evaluate(
        `(()=>{const p=document.querySelector('${panel}'),last=[...p.querySelectorAll('button')].at(-1);p.scrollTop=p.scrollHeight;const b=p.getBoundingClientRect();return{reachedEnd:p.scrollTop>0&&last.getBoundingClientRect().bottom<=b.bottom,viewportBottom:b.bottom,documentScroll:scrollY}})()`
      )
    ).toEqual({ reachedEnd: true, viewportBottom: 320, documentScroll: 0 })
  }, 60_000)
})
