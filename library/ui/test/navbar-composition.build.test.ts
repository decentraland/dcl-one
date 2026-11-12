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
    if (request.url !== '/') return next()
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
})
