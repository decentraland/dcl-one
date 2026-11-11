import path from 'node:path'
import { createServer, type ViteDevServer } from 'vite'
import { afterAll, beforeAll, describe, expect, it } from 'vitest'
import { findBrowser, openSession } from '../figma/review/scripts/capture-browser.mjs'

type Measurement = {
  width: number
  color: string
  labelColor: string
  iconColor: string
  background: string
}
let server: ViteDevServer
let session: Awaited<ReturnType<typeof openSession>>

async function evaluate<T = unknown>(expression: string): Promise<T> {
  const result = (await session.send('Runtime.evaluate', {
    expression,
    awaitPromise: true,
    returnByValue: true
  })) as {
    result: { value: T }
    exceptionDetails?: { text: string }
  }
  if (result.exceptionDetails) throw new Error(result.exceptionDetails.text)
  return result.result.value
}

beforeAll(async () => {
  const root = path.resolve(import.meta.dirname, '..')
  server = await createServer({
    configFile: false,
    root,
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
        '<!doctype html><style>body{padding:20px;background:white}main{display:flex;flex-direction:column;align-items:flex-start;gap:24px}</style><div id="root"></div><script type="module" src="/test/consumer/button-browser.tsx"></script>'
      )
    )
  })
  await server.listen()
  session = await openSession(
    await findBrowser([
      process.env.BROWSER_PATH,
      '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
      '/usr/bin/google-chrome',
      '/usr/bin/chromium'
    ])
  )
  await session.send('Page.navigate', { url: server.resolvedUrls!.local[0] })
  await evaluate(
    `(async()=>{for(let n=0;n<100 && document.querySelectorAll('button').length!==4;n++)await new Promise(r=>setTimeout(r,100));await document.fonts.ready})()`
  )
}, 30_000)

afterAll(async () => {
  try {
    await session?.close()
  } finally {
    await server?.close()
  }
})

const measure = (control: string): Promise<Measurement> =>
  evaluate<Measurement>(
    `(()=>{const root=document.querySelector('[data-control="${control}"]'),label=root.querySelector('[data-n]'),icon=root.querySelector('svg');return {width:root.getBoundingClientRect().width,color:getComputedStyle(root).color,labelColor:label&&getComputedStyle(label).color,iconColor:getComputedStyle(icon).color,background:getComputedStyle(root).backgroundColor}})()`
  )

describe('when a public Button enters a captured interaction state', () => {
  it.each([390, 1440])(
    'should keep label geometry and foreground through all interaction states at %ipx',
    async (width) => {
      await session.send('Emulation.setDeviceMetricsOverride', {
        width,
        height: 900,
        deviceScaleFactor: 1,
        mobile: false
      })
      await session.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: 0, y: 0 })
      const base = await measure('base')
      const point = await evaluate<{ x: number; y: number }>(
        `(()=>{const r=document.querySelector('[data-control="base"]').getBoundingClientRect();return {x:r.x+r.width/2,y:r.y+r.height/2}})()`
      )
      await session.send('Input.dispatchMouseEvent', { type: 'mouseMoved', ...point })
      const hover = await measure('base')
      await session.send('Input.dispatchMouseEvent', {
        type: 'mousePressed',
        button: 'left',
        clickCount: 1,
        ...point
      })
      const active = await measure('base')
      await session.send('Input.dispatchMouseEvent', {
        type: 'mouseReleased',
        button: 'left',
        clickCount: 1,
        ...point
      })
      const forcedHover = await measure('hover')
      const forcedPressed = await measure('pressed')
      for (const state of [base, hover, active, forcedHover, forcedPressed]) {
        expect(state.width).toBeCloseTo(base.width, 1)
        expect(state.iconColor).toBe(state.labelColor)
        expect(state.color).toBe(state.labelColor)
      }
      expect(hover.labelColor).not.toBe(base.labelColor)
      expect(forcedHover.labelColor).toBe(hover.labelColor)
      expect(forcedPressed.labelColor).toBe(active.labelColor)
      expect(session.runtimeErrors).toEqual([])
    }
  )

  it('should keep an icon-only pressed appearance neutral', async () => {
    const icon = await measure('icon')
    const neutral = await evaluate<string>(
      `(()=>{const probe=document.createElement('span');probe.style.backgroundColor='var(--dcl-bg-hover)';document.body.append(probe);const color=getComputedStyle(probe).backgroundColor;probe.remove();return color})()`
    )
    expect(icon.background).toBe(neutral)
    expect(icon.width).toBeGreaterThanOrEqual(48)
  })
})
