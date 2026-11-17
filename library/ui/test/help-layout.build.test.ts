import path from 'node:path'
import { createServer, type ViteDevServer } from 'vite'
import { afterAll, beforeAll, describe, expect, it } from 'vitest'
import { findBrowser, openSession } from '../figma/review/scripts/capture-browser.mjs'

let server: ViteDevServer
let session: Awaited<ReturnType<typeof openSession>>

type Box = { left: number; top: number; right: number; bottom: number; width: number }
type Geometry = {
  viewport: number
  scrollWidth: number
  title: Box
  status: Box
  tabs: Box
  panel: Box
  tabTops: number[]
}

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

async function layoutAt(width: number): Promise<Geometry> {
  await session.send('Emulation.setDeviceMetricsOverride', {
    width,
    height: 1024,
    deviceScaleFactor: 1,
    mobile: false
  })
  await session.navigate(`${server.resolvedUrls!.local[0]}`)
  return evaluate<Geometry>(`(async()=>{
    for(let n=0;!document.querySelector('[data-questions]');n++) {
      if(n===100)throw new Error('Help layout fixture did not render')
      await new Promise(resolve=>setTimeout(resolve,50))
    }
    await document.fonts.ready
    await new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)))
    const box=(element)=>{const b=element.getBoundingClientRect();return {left:b.left,top:b.top,right:b.right,bottom:b.bottom,width:b.width}}
    return {
      viewport:document.documentElement.clientWidth,
      scrollWidth:document.documentElement.scrollWidth,
      title:box(document.querySelector('main h1')),
      status:box(document.querySelector('[data-status]')),
      tabs:box(document.querySelector('[role="tablist"]')),
      panel:box(document.querySelector('[role="tabpanel"]')),
      tabTops:[...document.querySelectorAll('[role="tab"]')].map((tab)=>Math.round(tab.getBoundingClientRect().top))
    }
  })()`)
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
    if (new URL(request.url ?? '/', 'http://localhost').pathname !== '/') return next()
    response.setHeader('Content-Type', 'text/html')
    response.end(
      await server.transformIndexHtml(
        '/',
        '<!doctype html><meta name="viewport" content="width=device-width,initial-scale=1"><style>body{margin:0}</style><div id="root"></div><script type="module" src="/test/consumer/help-layout-browser.tsx"></script>'
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
      '/usr/bin/chromium',
      '/usr/bin/chromium-browser'
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

describe('when the help center renders', () => {
  it.each([390, 768, 1023, 1024, 1032, 1279, 1280, 1440])(
    'should keep every part inside the viewport at %ipx',
    async (width) => {
      const geometry = await layoutAt(width)
      expect(geometry.scrollWidth).toBe(geometry.viewport)
      for (const part of [geometry.title, geometry.status, geometry.tabs, geometry.panel]) {
        expect(part.left).toBeGreaterThanOrEqual(0)
        expect(part.right).toBeLessThanOrEqual(geometry.viewport)
      }
    }
  )

  it.each([768, 1023])(
    'should stack the section under one row of tabs on a %ipx tablet',
    async (width) => {
      const geometry = await layoutAt(width)
      expect(new Set(geometry.tabTops).size).toBe(1)
      expect(geometry.panel.top).toBeGreaterThanOrEqual(geometry.tabs.bottom)
      expect(geometry.panel.left).toBeCloseTo(geometry.title.left, 0)
      expect(geometry.status.top).toBeLessThan(geometry.panel.top)
      expect(geometry.status.right).toBeGreaterThan(geometry.panel.right - 1)
    }
  )

  it.each([1024, 1032, 1279])(
    'should give the section the width the status column took, with the status above it at %ipx',
    async (width) => {
      const geometry = await layoutAt(width)
      expect(geometry.panel.left).toBeGreaterThan(geometry.tabs.right)
      expect(geometry.status.bottom).toBeLessThanOrEqual(geometry.panel.top)
      expect(geometry.status.right).toBeCloseTo(geometry.panel.right, 0)
      expect(geometry.panel.width).toBeGreaterThan(500)
    }
  )

  it.each([1280, 1440])('should keep the status in its own column at %ipx', async (width) => {
    const geometry = await layoutAt(width)
    expect(geometry.status.left).toBeGreaterThanOrEqual(geometry.panel.right)
    expect(geometry.status.top).toBeLessThan(geometry.panel.top)
  })

  it('should keep the status beside the title on a 390px phone', async () => {
    const geometry = await layoutAt(390)
    expect(geometry.status.top).toBeLessThan(geometry.tabs.top)
    expect(geometry.status.left).toBeGreaterThan(geometry.title.left)
    expect(geometry.panel.top).toBeGreaterThanOrEqual(geometry.tabs.bottom)
  })
})
