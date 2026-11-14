import path from 'node:path'
import { createServer, type ViteDevServer } from 'vite'
import { afterAll, beforeAll, describe, expect, it } from 'vitest'
import { findBrowser, openSession } from '../figma/review/scripts/capture-browser.mjs'

let server: ViteDevServer | undefined
let session: Awaited<ReturnType<typeof openSession>> | undefined

async function evaluate<T>(expression: string): Promise<T> {
  const response = (await session!.send('Runtime.evaluate', {
    expression,
    awaitPromise: true,
    returnByValue: true
  })) as { result: { value: T }; exceptionDetails?: unknown }
  if (response.exceptionDetails) throw new Error(JSON.stringify(response.exceptionDetails))
  return response.result.value
}

beforeAll(async () => {
  try {
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
        await server!.transformIndexHtml(
          '/',
          '<!doctype html><meta name="viewport" content="width=device-width,initial-scale=1"><style>body{margin:0}</style><div id="root"></div><script type="module" src="/test/consumer/store-badges-browser.tsx"></script>'
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
  } catch (error) {
    await server?.close()
    throw error
  }
}, 60_000)

afterAll(async () => {
  try {
    await session?.close()
  } finally {
    await server?.close()
  }
})

describe('when the home sections offer a mobile store download', () => {
  for (const store of ['google_play', 'app_store']) {
    for (const width of [320, 360, 390, 768]) {
      it(`should contain and center the ${store} badge in both links at ${width}px`, async () => {
        await session!.send('Emulation.setDeviceMetricsOverride', {
          width,
          height: 844,
          deviceScaleFactor: 1,
          mobile: true
        })
        await session!.navigate(`${server!.resolvedUrls!.local[0]}?store=${store}`)
        await evaluate(`(async () => {
          for (let n=0;n<100 && document.querySelectorAll('a[target="_blank"]').length<2;n++) await new Promise(r=>setTimeout(r,20));
          await document.fonts.ready;
          await Promise.all([...document.querySelectorAll('a[target="_blank"] img')].map(image=>image.decode()));
        })()`)
        const measurements = await evaluate<{
          overflow: boolean
          sections: {
            contains: boolean
            centered: boolean
            imageRatio: number
            height: number
            name: string
            href: string
            focusable: boolean
          }[]
        }>(`(() => ({
          overflow:document.documentElement.scrollWidth>innerWidth,
          sections:['hero','closing'].map(id=>{
            const link=document.querySelector('#'+id+' a[target="_blank"]');
            const image=link.querySelector('img');const a=link.getBoundingClientRect(),i=image.getBoundingClientRect();
            link.focus();
            return {contains:i.left>=a.left-.5&&i.right<=a.right+.5&&i.top>=a.top-.5&&i.bottom<=a.bottom+.5,
              centered:Math.abs((a.left+a.right)/2-innerWidth/2)<.5&&Math.abs((i.left+i.right-a.left-a.right)/2)<.5,
              imageRatio:i.width/i.height,height:a.height,name:link.getAttribute('aria-label'),href:link.href,focusable:document.activeElement===link};
          })
        }))()`)
        expect(measurements.overflow).toBe(false)
        for (const section of measurements.sections) {
          expect(section.contains).toBe(true)
          expect(section.centered).toBe(true)
          expect(section.height).toBe(64)
          expect(section.imageRatio).toBeCloseTo(store === 'google_play' ? 163 / 41 : 140 / 36, 2)
          expect(section.name).toBe(
            store === 'google_play' ? 'Get it on Google Play' : 'Download on the App Store'
          )
          expect(section.href).toContain(
            store === 'google_play' ? 'play.google.com/' : 'apps.apple.com/'
          )
          expect(section.focusable).toBe(true)
        }
      }, 30_000)
    }
  }

  it('should keep mobile store links hidden and desktop downloads available at 1440px', async () => {
    await session!.send('Emulation.setDeviceMetricsOverride', {
      width: 1440,
      height: 900,
      deviceScaleFactor: 1,
      mobile: false
    })
    await session!.navigate(server!.resolvedUrls!.local[0]!)
    const visible = await evaluate<{ stores: number; downloads: number }>(`(async () => {
      for(let n=0;n<100&&document.querySelectorAll('a').length<4;n++)await new Promise(r=>setTimeout(r,20));
      return {stores:[...document.querySelectorAll('a[target="_blank"]')].filter(a=>a.getClientRects().length).length,
        downloads:[...document.querySelectorAll('a[href^="/download_success"]')].filter(a=>a.getClientRects().length).length};
    })()`)
    expect(visible).toEqual({ stores: 0, downloads: 2 })
  }, 30_000)
})
