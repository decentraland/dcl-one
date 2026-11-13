import path from 'node:path'
import { build } from 'vite'
import { mkdtemp, readFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { createServer, type Server } from 'node:http'
import type { AddressInfo } from 'node:net'
import { releaseHarness } from './release-harness'
import { afterAll, beforeAll, describe, expect, it } from 'vitest'
import { findBrowser, openSession } from '../figma/review/scripts/capture-browser.mjs'

let server: Server
let scratch: string
let origin: string
let session: Awaited<ReturnType<typeof openSession>>
let preparation: Promise<void> | undefined
let startup: ReturnType<typeof openSession> | undefined
const startupController = new AbortController()

async function evaluate<T = unknown>(expression: string): Promise<T> {
  const result = (await session.send('Runtime.evaluate', {
    expression,
    awaitPromise: true,
    returnByValue: true
  })) as { result: { value: T }; exceptionDetails?: { text: string } }
  if (result.exceptionDetails) throw new Error(result.exceptionDetails.text)
  return result.result.value
}

async function resolveWash(token: string): Promise<string> {
  return evaluate(
    `(()=>{const probe=document.createElement('span');probe.style.backgroundColor='var('+${JSON.stringify(token)}+')';document.body.append(probe);const color=getComputedStyle(probe).backgroundColor;probe.remove();return color})()`
  )
}

async function click(selector: string) {
  const point = await evaluate<{ x: number; y: number }>(`(() => {
    const element = document.querySelector(${JSON.stringify(selector)});
    const rect = element.getBoundingClientRect();
    return { x: rect.x + rect.width / 2, y: rect.y + rect.height / 2 };
  })()`)
  await session.send('Input.dispatchMouseEvent', {
    type: 'mousePressed',
    button: 'left',
    clickCount: 1,
    ...point
  })
  await session.send('Input.dispatchMouseEvent', {
    type: 'mouseReleased',
    button: 'left',
    clickCount: 1,
    ...point
  })
  await evaluate(
    'new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))'
  )
}

async function key(value: 'Tab' | 'Escape', shift = false) {
  for (const type of ['keyDown', 'keyUp'])
    await session.send('Input.dispatchKeyEvent', {
      type,
      key: value,
      code: value,
      windowsVirtualKeyCode: value === 'Tab' ? 9 : 27,
      modifiers: type === 'keyDown' && shift ? 8 : 0
    })
}

beforeAll(() => {
  preparation = prepareHarness()
  return preparation
}, 30_000)

async function prepareHarness() {
  const root = path.resolve(import.meta.dirname, '..')
  scratch = await mkdtemp(path.join(tmpdir(), 'navbar-composition-'))
  const environment = process.env.NODE_ENV
  process.env.NODE_ENV = 'production'
  try {
    await build({
      configFile: false,
      root,
      logLevel: 'silent',
      define: { 'process.env.NODE_ENV': JSON.stringify('production') },
      resolve: { dedupe: ['react', 'react-dom'] },
      build: {
        outDir: scratch,
        emptyOutDir: true,
        lib: {
          entry: path.join(root, 'test/consumer/navbar-composition-browser.tsx'),
          formats: ['es'],
          fileName: 'fixture',
          cssFileName: 'fixture'
        }
      }
    })
  } finally {
    if (environment === undefined) delete process.env.NODE_ENV
    else process.env.NODE_ENV = environment
  }
  startupController.signal.throwIfAborted()
  server = createServer((request, response) => {
    void (async () => {
      const pathname = new URL(request.url ?? '/', 'http://fixture.local').pathname
      if (pathname === '/') {
        response
          .writeHead(200, { 'content-type': 'text/html' })
          .end(
            '<!doctype html><meta name="viewport" content="width=device-width,initial-scale=1"><style>body{margin:0;font-family:Inter,sans-serif;background:var(--dcl-neutrals-shadow);color:var(--dcl-text-primary)}</style><link rel="stylesheet" href="/fixture.css"><div id="root"></div><script type="module" src="/fixture.js"></script>'
          )
        return
      }
      const type =
        pathname === '/fixture.js'
          ? 'text/javascript'
          : pathname === '/fixture.css'
            ? 'text/css'
            : undefined
      if (!type) return void response.writeHead(404).end()
      try {
        response
          .writeHead(200, { 'content-type': type })
          .end(await readFile(path.join(scratch, pathname.slice(1))))
      } catch {
        response.writeHead(404).end()
      }
    })()
  })
  await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve))
  origin = `http://127.0.0.1:${(server.address() as AddressInfo).port}/`
  startup = (async () =>
    openSession(
      await findBrowser([
        process.env.BROWSER_PATH,
        process.env.CHROME_PATH,
        '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
        '/usr/bin/google-chrome',
        '/usr/bin/chromium'
      ]),
      { signal: startupController.signal }
    ))()
  session = await startup
}

afterAll(async () => {
  startupController.abort(new Error('NavBar composition fixture disposed'))
  await preparation?.catch(() => undefined)
  const opened = await startup?.catch(() => undefined)
  await releaseHarness({ session: opened, server, scratch })
}, 30_000)

describe('when a public NavBar composes app-owned controls in Chromium', () => {
  for (const width of [390, 768, 1440]) {
    it(`should show one control instance and usable drawer focus at ${width}px`, async () => {
      await session.send('Emulation.setDeviceMetricsOverride', {
        width,
        height: 900,
        deviceScaleFactor: 1,
        mobile: false
      })
      await session.navigate(origin)
      await evaluate(`(async () => {
        for (let n = 0; n < 100 && !document.querySelector('#balance-action'); n++) await new Promise(resolve => setTimeout(resolve, 50));
        await document.fonts.ready;
      })()`)
      const resting = await evaluate<{
        balanceVisible: boolean
        accountVisible: boolean
        secondary: boolean
        count: number
        overflow: boolean
      }>(`(() => {
        const visible = selector => !!document.querySelector(selector).getClientRects().length;
        return { balanceVisible: visible('#balance-action'), accountVisible: visible('#account-control'), secondary: !!document.querySelector('#secondary-link'), count: document.querySelectorAll('#balance-action').length, overflow: document.documentElement.scrollWidth > innerWidth };
      })()`)
      expect(resting).toEqual({
        balanceVisible: width >= 768,
        accountVisible: width >= 1024,
        secondary: false,
        count: 1,
        overflow: false
      })
      if (width >= 1024) {
        await click('#account-control')
        expect(await evaluate("document.querySelector('[role=status]').textContent")).toBe(
          'Connections: 1'
        )
        return
      }
      await click('button[aria-label="Open menu"]')
      expect(
        await evaluate(`(() => {
        const dialog = document.querySelector('[role=dialog]');
        return { balance: dialog.querySelectorAll('#balance-action').length, account: dialog.querySelectorAll('#account-control').length, secondary: dialog.querySelectorAll('#secondary-link').length, totalBalance: document.querySelectorAll('#balance-action').length, nested: !!dialog.querySelector('a a,button button,button a,a button') };
      })()`)
      ).toEqual({ balance: 1, account: 1, secondary: 1, totalBalance: 1, nested: false })
      await click('#account-control')
      expect(await evaluate("document.querySelector('[role=status]').textContent")).toBe(
        'Connections: 1'
      )
      await session.send('Input.dispatchKeyEvent', {
        type: 'keyDown',
        key: 'Escape',
        code: 'Escape',
        windowsVirtualKeyCode: 27
      })
      expect(await evaluate('!!document.querySelector("[role=dialog]")')).toBe(true)
      expect(
        await evaluate('document.querySelector("#account-control").getAttribute("aria-expanded")')
      ).toBe('false')
      await session.send('Input.dispatchKeyEvent', {
        type: 'keyDown',
        key: 'Tab',
        code: 'Tab',
        windowsVirtualKeyCode: 9
      })
      await session.send('Input.dispatchKeyEvent', {
        type: 'keyUp',
        key: 'Tab',
        code: 'Tab',
        windowsVirtualKeyCode: 9
      })
      expect(await evaluate('document.activeElement.getAttribute("aria-label")')).toBe('Close menu')
      await session.send('Input.dispatchKeyEvent', {
        type: 'keyDown',
        key: 'Escape',
        code: 'Escape',
        windowsVirtualKeyCode: 27
      })
      expect(await evaluate('document.activeElement.getAttribute("aria-label")')).toBe('Open menu')
      expect(await evaluate('document.querySelectorAll("#balance-action").length')).toBe(1)
      await click('button[aria-label="Open menu"]')
      await click('#secondary-link')
      expect(await evaluate('!!document.querySelector("[role=dialog]")')).toBe(false)
      expect(await evaluate('document.activeElement.getAttribute("aria-label")')).toBe('Open menu')
    }, 30_000)
  }
  for (const width of [360, 390, 414, 768, 1023, 1024, 1440]) {
    for (const surface of ['solid', 'overlay']) {
      it(`should preserve the production surface and viewport overlays at ${width}px on ${surface}`, async () => {
        await session.send('Emulation.setDeviceMetricsOverride', {
          width,
          height: 844,
          deviceScaleFactor: 1,
          mobile: width < 1024
        })
        await session.send('Emulation.setTouchEmulationEnabled', { enabled: width < 1024 })
        const errorStart = session.runtimeErrors.length
        await session.navigate(origin + '?surface-proof&surface=' + surface)
        await evaluate(
          `(async()=>{for(let n=0;n<100&&!document.querySelector('#balance-action');n++)await new Promise(r=>setTimeout(r,50));await document.fonts.ready;document.documentElement.style.scrollbarGutter='stable'})()`
        )
        expect(session.runtimeErrors.slice(errorStart)).toEqual([])
        const minimal = width >= 1024 && surface === 'overlay'
        const expectedWash = minimal
          ? 'rgba(0, 0, 0, 0)'
          : await resolveWash(width >= 1024 ? '--dcl-bg-navbar' : '--dcl-bg-navbar-mobile')
        for (const scroll of [0, 50, 200, 400, ...(minimal ? [1000] : [])]) {
          await evaluate(
            `scrollTo(0,${scroll});new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)))`
          )
          const availableWidth = await evaluate<number>('document.documentElement.clientWidth')
          expect(await evaluate('innerWidth')).toBe(width)
          expect(await evaluate('document.documentElement.scrollWidth')).toBe(availableWidth)
          const measured = await evaluate(
            `(()=>{const nav=document.querySelector('nav[aria-label="Main"]'),root=getComputedStyle(nav),layer=getComputedStyle(nav,'::before'),r=nav.getBoundingClientRect();return{x:r.x,y:r.y,height:r.height,width:r.width,background:root.backgroundColor,image:root.backgroundImage,shadow:root.boxShadow,filter:root.backdropFilter,transform:root.transform,contain:root.contain,layerBackground:layer.backgroundColor,layerFilter:layer.backdropFilter,layerShadow:layer.boxShadow,layerPointer:layer.pointerEvents,opacity:layer.opacity}})()`
          )
          expect(measured).toMatchObject({
            x: 0,
            y: 0,
            height: width >= 1024 ? 80 : 64,
            width: availableWidth,
            background: 'rgba(0, 0, 0, 0)',
            image: 'none',
            shadow: 'none',
            filter: 'none',
            transform: 'none',
            contain: 'none',
            layerBackground: expectedWash,
            layerFilter: minimal ? 'none' : 'saturate(1.8) blur(20px)',
            layerShadow: 'none',
            layerPointer: 'none',
            opacity: '1'
          })
        }
        if (width < 1024) {
          await evaluate("document.documentElement.style.scrollbarGutter = 'auto'")
          await click('button[aria-label="Open menu"]')
          expect(await evaluate('document.documentElement.clientWidth')).toBe(width)
          const measure = await evaluate(
            `(()=>{const d=document.querySelector('[role=dialog]'),layer=d.parentElement,r=d.getBoundingClientRect(),l=layer.getBoundingClientRect();return{dialog:{x:r.x,y:r.y,width:r.width,height:r.height},layer:{x:l.x,y:l.y,width:l.width,height:l.height},background:getComputedStyle(d).backgroundColor,bottomInside:d.contains(document.elementFromPoint(innerWidth-1,innerHeight-1)),bottomBackdrop:document.elementFromPoint(1,innerHeight-1)===layer.firstElementChild}})()`
          )
          expect(measure).toMatchObject({
            dialog: { x: 0, y: 0, width, height: 844 },
            layer: { x: 0, y: 0, width, height: 844 },
            bottomInside: true,
            bottomBackdrop: false
          })
          expect((measure as { background: string }).background).toMatch(/^rgb\(/)
          await key('Tab', true)
          expect(await evaluate('document.activeElement.id')).toBe('account-control')
          await key('Tab')
          expect(await evaluate('document.activeElement.getAttribute("aria-label")')).toBe(
            'Close menu'
          )
          await evaluate(
            `document.querySelector('[role=dialog]').scrollTop=10000;document.querySelector('[role=dialog] a[href="#blog"]').scrollIntoView({block:'end'})`
          )
          await click('[role=dialog] a[href="#blog"]')
          expect(await evaluate('location.hash')).toBe('#blog')
          expect(await evaluate('!!document.querySelector("[role=dialog]")')).toBe(false)
          await click('button[aria-label="Open menu"]')
          await key('Escape')
          expect(await evaluate('document.activeElement.getAttribute("aria-label")')).toBe(
            'Open menu'
          )
          await click('button[aria-label="Open menu"]')
          await click('button[aria-label="Close menu"]')
          expect(await evaluate('!!document.querySelector("[role=dialog]")')).toBe(false)
          expect(await evaluate('document.activeElement.getAttribute("aria-label")')).toBe(
            'Open menu'
          )
        } else {
          expect(
            await evaluate(
              `document.querySelector('button[aria-label="Open menu"]').getClientRects().length`
            )
          ).toBe(0)
          expect(await evaluate('!!document.querySelector("[role=dialog]")')).toBe(false)
          const trigger = 'button[aria-controls]:not([aria-label])'
          await click(trigger)
          const popup = await evaluate(
            `(()=>{const t=document.querySelector(${JSON.stringify(trigger)}),p=document.getElementById(t.getAttribute('aria-controls')),r=p.getBoundingClientRect(),a=p.querySelector('a'),b=a.getBoundingClientRect();return{width:r.width,height:r.height,background:getComputedStyle(p).backgroundColor,hit:a.contains(document.elementFromPoint(b.x+b.width/2,b.y+b.height/2))}})()`
          )
          expect(popup).toMatchObject({ width: 200, height: 92, hit: true })
          expect((popup as { background: string }).background).toMatch(/^rgb\(/)
          await key('Escape')
          expect(
            await evaluate(
              `document.activeElement===document.querySelector(${JSON.stringify(trigger)})`
            )
          ).toBe(true)
        }
        expect(session.runtimeErrors.slice(errorStart)).toEqual([])
      }, 30_000)
    }
  }
  it('should keep the existing wash when backdrop filtering is unavailable', async () => {
    await session.send('Emulation.setDeviceMetricsOverride', {
      width: 390,
      height: 844,
      deviceScaleFactor: 1,
      mobile: false
    })
    await session.navigate(origin + '?surface-proof&surface=overlay')
    await evaluate(
      `(async()=>{for(let n=0;n<100&&!document.querySelector('nav');n++)await new Promise(r=>setTimeout(r,50));const style=document.createElement('style');style.textContent='nav[aria-label="Main"]::before{-webkit-backdrop-filter:none!important;backdrop-filter:none!important}';document.head.append(style)})()`
    )
    expect(
      await evaluate(
        `(()=>{const p=getComputedStyle(document.querySelector('nav[aria-label="Main"]'),'::before');return{wash:p.backgroundColor,filter:p.backdropFilter}})()`
      )
    ).toEqual({ wash: await resolveWash('--dcl-bg-navbar-mobile'), filter: 'none' })
  }, 30_000)

  for (const width of [991, 992, 1023, 1024, 1440]) {
    for (const surface of ['solid', 'overlay']) {
      it(`should keep account geometry stable across balance states at ${width}px on ${surface}`, async () => {
        await session.send('Emulation.setDeviceMetricsOverride', {
          width,
          height: 900,
          deviceScaleFactor: 1,
          mobile: false
        })
        for (const account of ['default', 'custom']) {
          await session.navigate(origin + '?geometry&surface=' + surface + '&account=' + account)
          await evaluate(
            `(async()=>{for(let n=0;n<100&&!document.querySelector('[data-chip-mode]');n++)await new Promise(r=>setTimeout(r,50));await document.fonts.ready})()`
          )
          expect(
            await evaluate(
              `getComputedStyle(document.querySelector('nav[aria-label="Main"]')).fontFamily`
            )
          ).toContain('Inter')
          expect(
            await evaluate(
              `Array.from(document.fonts).some(face => face.family.includes('Inter') && face.status === 'loaded')`
            )
          ).toBe(true)
          for (const mode of [
            'none',
            'loading',
            'zero',
            'positive',
            'iap',
            'loading',
            'positive'
          ]) {
            await click('[data-chip-mode="' + mode + '"]')
            const triggerSelector =
              account === 'default' ? 'button[aria-label="Account: Ada"]' : '#geometry-account'
            const hover = await evaluate<{ x: number; y: number } | null>(
              `(() => { const r=document.querySelector(${JSON.stringify(triggerSelector)}).getBoundingClientRect(); return r.height ? {x:r.x+r.width/2,y:r.y+r.height/2} : null })()`
            )
            if (hover)
              await session.send('Input.dispatchMouseEvent', { type: 'mouseMoved', ...hover })
            const bounds = await evaluate<{
              height: number
              width: number
              account: number
              trigger: number
              chip: number | null
              hit: boolean | null
              count: number
              inside: boolean
            }>(`(() => {
              const nav=document.querySelector('nav[aria-label="Main"]'),n=nav.getBoundingClientRect();
              const trigger=nav.querySelector(${JSON.stringify(triggerSelector)}),t=trigger.getBoundingClientRect();
              const chip=document.querySelector('#balance-action'),r=chip?.getBoundingClientRect();
              return {height:n.height,width:n.width,account:trigger.parentElement.getBoundingClientRect().height,trigger:t.height,chip:r?.height ?? null,hit:r ? chip.contains(document.elementFromPoint(r.right-1,r.bottom-1)) : null,count:document.querySelectorAll('#balance-action').length,inside:(!t.height || t.top>=n.top+2 && t.bottom<=n.bottom-2) && (!r || r.top>=n.top+2 && r.bottom<=n.bottom-2)};
            })()`)
            const context = account + '/' + mode
            expect(bounds.height, context).toBe(width >= 1024 ? 80 : 64)
            expect(bounds.width, context).toBe(width)
            const accountSize = width >= 1024 ? 48 : account === 'default' ? 40 : 0
            expect(bounds.account, context).toBe(accountSize)
            expect(bounds.trigger, context).toBe(accountSize)
            expect(bounds.inside, context).toBe(true)
            expect(bounds.count, context).toBe(mode === 'none' || mode === 'loading' ? 0 : 1)
            if (bounds.chip !== null) {
              expect(bounds.chip, context).toBe(44)
              expect(bounds.hit, context).toBe(true)
            }
            if (accountSize) {
              await evaluate(`document.querySelector(${JSON.stringify(triggerSelector)}).focus()`)
              expect(
                await evaluate(
                  `document.querySelector('nav[aria-label="Main"]').getBoundingClientRect().height`
                ),
                context
              ).toBe(width >= 1024 ? 80 : 64)
            }
          }
        }
      }, 30000)
    }
  }
})
