import path from 'node:path'
import { createServer, type ViteDevServer } from 'vite'
import { afterAll, beforeAll, describe, expect, it } from 'vitest'
import { findBrowser, openSession } from '../figma/review/scripts/capture-browser.mjs'

let server: ViteDevServer
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
        '<!doctype html><meta name="viewport" content="width=device-width,initial-scale=1"><style>body{margin:0}</style><div id="root"></div><script type="module" src="/test/consumer/places-spacing-browser.tsx"></script>'
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

describe.each([390, 1440])('when Places renders at %ipx', (width) => {
  it.each([false, true])(
    'should reserve the section gap only for visible Live Now (%s)',
    async (live) => {
      await session.send('Emulation.setDeviceMetricsOverride', {
        width,
        height: 844,
        deviceScaleFactor: 1,
        mobile: false
      })
      await session.navigate(`${server.resolvedUrls!.local[0]}${live ? '?live=1' : ''}`)
      await evaluate(`(async()=>{
      for(let n=0;!document.querySelector('main [role="group"]');n++) {
        if(n===100)throw new Error('Places spacing fixture did not render')
        await new Promise(resolve=>setTimeout(resolve,50))
      }
      await document.fonts.ready
      await new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)))
    })()`)
      const geometry = await evaluate<{
        headerBottom: number
        mainTop: number
        bandTop: number
        contentTop: number
        liveBottom: number | null
      }>(`(()=>{
      const main=document.querySelector('main'),tabs=main.querySelector('[role="group"]'),content=tabs.parentElement.parentElement,band=content.parentElement,live=document.querySelector('[data-live]')
      return {headerBottom:document.querySelector('nav').getBoundingClientRect().bottom,mainTop:main.getBoundingClientRect().top+parseFloat(getComputedStyle(main).paddingTop),bandTop:band.getBoundingClientRect().top,contentTop:content.getBoundingClientRect().top,liveBottom:live?.getBoundingClientRect().bottom??null}
    })()`)
      // The page starts under the fixed bar; its content starts at the bar's bottom, 4px lower from
      // 992px as on decentraland.org.
      const pageTop = geometry.headerBottom + (width >= 992 ? 4 : 0)
      expect(geometry.mainTop).toBeCloseTo(pageTop, 1)
      expect(geometry.bandTop).toBeCloseTo(live ? geometry.liveBottom! + 48 : pageTop, 1)
      expect(geometry.contentTop - geometry.bandTop).toBeCloseTo(48, 1)
    }
  )
})
