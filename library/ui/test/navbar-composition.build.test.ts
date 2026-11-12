import path from 'node:path'
import { createServer, type ViteDevServer } from 'vite'
import { afterAll, beforeAll, describe, expect, it } from 'vitest'
import { findBrowser, openSession } from '../figma/review/scripts/capture-browser.mjs'

let server: ViteDevServer
let session: Awaited<ReturnType<typeof openSession>>

async function evaluate<T = unknown>(expression: string): Promise<T> {
  const result = (await session.send('Runtime.evaluate', {
    expression,
    awaitPromise: true,
    returnByValue: true
  })) as { result: { value: T }; exceptionDetails?: { text: string } }
  if (result.exceptionDetails) throw new Error(result.exceptionDetails.text)
  return result.result.value
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

beforeAll(async () => {
  server = await createServer({
    configFile: false,
    root: path.resolve(import.meta.dirname, '..'),
    appType: 'custom',
    logLevel: 'error',
    resolve: { dedupe: ['react', 'react-dom'] },
    server: { host: '127.0.0.1', port: 0 }
  })
  server.middlewares.use(async (request, response, next) => {
    if (new URL(request.url ?? '/', 'http://fixture.local').pathname !== '/') return next()
    response.setHeader('Content-Type', 'text/html')
    response.end(
      await server.transformIndexHtml(
        '/',
        '<!doctype html><style>body{margin:0;font-family:Inter,sans-serif;background:var(--dcl-neutrals-shadow);color:var(--dcl-text-primary)}</style><div id="root"></div><script type="module" src="/test/consumer/navbar-composition-browser.tsx"></script>'
      )
    )
  })
  await server.listen()
  session = await openSession(
    await findBrowser([
      process.env.BROWSER_PATH,
      process.env.CHROME_PATH,
      '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
      '/usr/bin/google-chrome',
      '/usr/bin/chromium'
    ])
  )
}, 30_000)

afterAll(async () => {
  try {
    await session?.close()
  } finally {
    await server?.close()
  }
})

describe('when a public NavBar composes app-owned controls in Chromium', () => {
  for (const width of [390, 768, 1440]) {
    it(`should show one control instance and usable drawer focus at ${width}px`, async () => {
      await session.send('Emulation.setDeviceMetricsOverride', {
        width,
        height: 900,
        deviceScaleFactor: 1,
        mobile: false
      })
      await session.navigate(server.resolvedUrls!.local[0]!)
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
  for (const width of [390, 1440]) {
    it(`should keep overlay geometry, foreground contrast and opaque popups at ${width}px`, async () => {
      await session.send('Emulation.setDeviceMetricsOverride', {
        width,
        height: 900,
        deviceScaleFactor: 1,
        mobile: false
      })
      const measure = `(() => { const nav=document.querySelector('nav[aria-label="Main"]'); const rect=nav.getBoundingClientRect(); return { width:rect.width,height:rect.height,image:getComputedStyle(nav).backgroundImage,position:getComputedStyle(nav).position }; })()`
      await session.navigate(server.resolvedUrls!.local[0]!)
      await evaluate(
        `(async()=>{for(let n=0;n<100&&!document.querySelector('#balance-action');n++)await new Promise(r=>setTimeout(r,50));await document.fonts.ready})()`
      )
      const standard = await evaluate<{
        width: number
        height: number
        image: string
        position: string
      }>(measure)
      expect(standard.image).toBe('none')
      await session.navigate(server.resolvedUrls!.local[0]! + '?surface=overlay')
      await evaluate(
        `(async()=>{for(let n=0;n<100&&!document.querySelector('#balance-action');n++)await new Promise(r=>setTimeout(r,50));await document.fonts.ready;document.body.style.background='white'})()`
      )
      const overlay = await evaluate<{
        width: number
        height: number
        image: string
        position: string
      }>(measure)
      expect(overlay).toMatchObject({
        width: standard.width,
        height: standard.height,
        position: standard.position
      })
      expect(overlay.image).toContain('linear-gradient')
      const foreground = await evaluate<{ inBlackBand: boolean; contrast: number }>(`(() => {
        const nav=document.querySelector('nav[aria-label="Main"]');const r=nav.getBoundingClientRect();
        const control=innerWidth>=1024?nav.querySelector('a[href="/discover"]'):nav.querySelector('button[aria-label="Open menu"]');const c=control.getBoundingClientRect();
        const rgb=getComputedStyle(control).color.match(/[0-9.]+/g).slice(0,3).map(Number).map(v=>{v/=255;return v<=0.04045?v/12.92:((v+0.055)/1.055)**2.4});
        return {inBlackBand:c.y+c.height/2-r.y<=r.height*0.7,contrast:(0.2126*rgb[0]+0.7152*rgb[1]+0.0722*rgb[2]+0.05)/0.05};
      })()`)
      expect(foreground.inBlackBand).toBe(true)
      expect(foreground.contrast).toBeGreaterThanOrEqual(4.5)
      if (width < 1024) {
        await click('button[aria-label="Open menu"]')
        const popup = await evaluate<{ image: string; color: string }>(
          `(() => {const style=getComputedStyle(document.querySelector('[role=dialog]'));return {image:style.backgroundImage,color:style.backgroundColor}})()`
        )
        expect(popup.image).toBe('none')
        expect(popup.color).toMatch(/^rgb\(/)
      } else {
        await click('button[aria-controls]:not([aria-label])')
        const popup = await evaluate<{ image: string; color: string }>(
          `(() => {const trigger=document.querySelector('button[aria-controls]:not([aria-label])');const style=getComputedStyle(document.getElementById(trigger.getAttribute('aria-controls')));return {image:style.backgroundImage,color:style.backgroundColor}})()`
        )
        expect(popup.image).toBe('none')
        expect(popup.color).toMatch(/^rgb\(/)
      }
    }, 30000)
  }

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
          await session.navigate(
            server.resolvedUrls!.local[0]! + '?geometry&surface=' + surface + '&account=' + account
          )
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
