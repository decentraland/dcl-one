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
  })) as {
    result: { value: T }
    exceptionDetails?: { text: string; exception?: { description?: string } }
  }
  if (result.exceptionDetails)
    throw new Error(result.exceptionDetails.exception?.description ?? result.exceptionDetails.text)
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
        '<!doctype html><meta name="viewport" content="width=device-width,initial-scale=1"><style>body{margin:0;background:#21102f}main{margin:0 auto;padding:0 32px;max-width:1280px}[data-day]{width:300px}</style><div id="root"></div><script type="module" src="/test/consumer/events-cards-browser.tsx"></script>'
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
  await session.navigate(server.resolvedUrls!.local[0]!)
  await evaluate(
    `(async()=>{for(let n=0;n<100 && document.querySelectorAll('article').length!==5;n++)await new Promise(r=>setTimeout(r,100));await document.fonts.ready})()`
  )
}, 30_000)

afterAll(async () => {
  try {
    await session?.close()
  } finally {
    await server?.close()
  }
})

const rect = (selector: string) =>
  evaluate<{ width: number; height: number; x: number; y: number }>(
    `document.querySelector(${JSON.stringify(selector)}).getBoundingClientRect().toJSON()`
  )
const hover = async (selector: string) => {
  await evaluate(
    `document.querySelector(${JSON.stringify(selector)}).scrollIntoView({block:'center'})`
  )
  const box = await rect(selector)
  await session.send('Input.dispatchMouseEvent', {
    type: 'mouseMoved',
    x: box.x + box.width / 2,
    y: box.y + box.height / 2
  })
  await evaluate(`new Promise(r=>setTimeout(r,350))`)
}
describe('when Events cards reveal their controls', () => {
  it.each([390, 1440])(
    'should keep Live Now and the next section stable at %ipx',
    async (width) => {
      await session.send('Emulation.setDeviceMetricsOverride', {
        width,
        height: 1000,
        deviceScaleFactor: 1,
        mobile: false
      })
      await evaluate(`scrollTo(0,0);document.activeElement?.blur()`)
      await session.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: 0, y: 0 })
      const selector = 'article[class*="liveCard"]'
      const before = await rect(selector)
      const sectionBefore = await rect('section[aria-labelledby]:nth-of-type(2)')
      await hover(selector)
      const after = await rect(selector)
      const sectionAfter = await rect('section[aria-labelledby]:nth-of-type(2)')
      expect(after.width).toBeCloseTo(before.width, 1)
      expect(after.height).toBeCloseTo(before.height, 1)
      expect(sectionAfter.y - after.y).toBeCloseTo(sectionBefore.y - before.y, 1)
    }
  )
  it.each(['smallCard', 'dayCard'])(
    'should swap time for controls without resizing the %s on hover and focus',
    async (variant) => {
      await session.send('Emulation.setDeviceMetricsOverride', {
        width: 1440,
        height: 1000,
        deviceScaleFactor: 1,
        mobile: false
      })
      await session.send('Emulation.setTouchEmulationEnabled', {
        enabled: false,
        configuration: 'desktop'
      })
      await evaluate(`document.activeElement?.blur()`)
      expect(await evaluate(`matchMedia('(hover: hover)').matches`)).toBe(true)
      await session.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: 0, y: 0 })
      const selector = `article[class*="${variant}"]`
      const before = await rect(selector)
      await hover(selector)
      const after = await rect(selector)
      expect(after.width).toBeCloseTo(before.width, 1)
      expect(after.height).toBeCloseTo(before.height, 1)
      const state = await evaluate<{ opacity: string; time: string; buttons: number }>(
        `(()=>{const card=document.querySelector(${JSON.stringify(selector)});return {opacity:getComputedStyle(card.querySelector('[class*="actions"]')).opacity,time:getComputedStyle(card.querySelector('[class*="time"]')).opacity,buttons:card.querySelectorAll('button,a[aria-label*="calendar"]').length}})()`
      )
      expect(state).toEqual({ opacity: '1', time: '0', buttons: 3 })
      await session.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: 0, y: 0 })
      await evaluate(
        `document.querySelector(${JSON.stringify(selector)}).querySelector('h3 a').focus()`
      )
      expect(
        await evaluate(
          `getComputedStyle(document.querySelector(${JSON.stringify(selector)}).querySelector('[class*="actions"]')).opacity`
        )
      ).toBe('1')
      expect((await rect(selector)).height).toBeCloseTo(before.height, 1)
    }
  )
  it.each([390, 768, 1024, 1440])(
    'should retain two complete title lines with and without a creator at %ipx',
    async (width) => {
      await session.send('Emulation.setDeviceMetricsOverride', {
        width,
        height: 1000,
        deviceScaleFactor: 1,
        mobile: width === 390
      })
      await session.send('Emulation.setTouchEmulationEnabled', {
        enabled: width === 390,
        maxTouchPoints: 1
      })
      const titles = await evaluate<
        { height: number; lineHeight: number; footerInside: boolean; clamp: string }[]
      >(
        `(()=>[...document.querySelectorAll('article[class*="smallCard"]')].map(card=>{const title=card.querySelector('h3'),style=getComputedStyle(title),footer=card.querySelector('[class*="cardFooter"]');return {height:title.getBoundingClientRect().height,lineHeight:parseFloat(style.lineHeight),clamp:style.webkitLineClamp,footerInside:footer.getBoundingClientRect().bottom<=card.getBoundingClientRect().bottom}}))()`
      )
      expect(titles).toHaveLength(2)
      for (const title of titles) {
        expect(title.height).toBeGreaterThanOrEqual(title.lineHeight * 2 - 0.5)
        expect(title.clamp).toBe('2')
        expect(title.footerInside).toBe(true)
      }
    }
  )
  it('should keep desktop controls discoverable before hover and reachable by reverse Tab', async () => {
    await session.send('Emulation.setDeviceMetricsOverride', {
      width: 1440,
      height: 1000,
      deviceScaleFactor: 1,
      mobile: false
    })
    await session.send('Emulation.setTouchEmulationEnabled', {
      enabled: false,
      configuration: 'desktop'
    })
    await session.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: 0, y: 0 })
    await evaluate(`document.activeElement?.blur()`)
    const tree = (await session.send('Accessibility.getFullAXTree')) as {
      nodes: { ignored: boolean; role?: { value: string }; name?: { value: string } }[]
    }
    expect(
      tree.nodes.some(
        (node) =>
          !node.ignored &&
          node.role?.value === 'button' &&
          node.name?.value.startsWith('Remind me:')
      )
    ).toBe(true)
    await evaluate(
      `document.querySelectorAll('article[class*="smallCard"]')[1].querySelector('h3 a').focus()`
    )
    await session.send('Input.dispatchKeyEvent', {
      type: 'keyDown',
      key: 'Tab',
      code: 'Tab',
      modifiers: 8
    })
    await session.send('Input.dispatchKeyEvent', {
      type: 'keyUp',
      key: 'Tab',
      code: 'Tab',
      modifiers: 8
    })
    const focused = await evaluate<{ label: string; first: boolean; opacity: string }>(
      `(()=>{const card=document.querySelector('article[class*="smallCard"]');return {label:document.activeElement.getAttribute('aria-label'),first:card.contains(document.activeElement),opacity:getComputedStyle(card.querySelector('[class*="actions"]')).opacity}})()`
    )
    expect(focused.label).toContain('Copy link:')
    expect(focused.first).toBe(true)
    await evaluate(`new Promise(r=>setTimeout(r,250))`)
    expect(
      await evaluate(
        `getComputedStyle(document.querySelector('article[class*="smallCard"] [class*="actions"]')).opacity`
      )
    ).toBe('1')
  })
  it('should let uncovered footer space open the card instead of intercepting the pointer', async () => {
    await hover('article[class*="smallCard"]')
    const hit = await evaluate<boolean>(
      `(()=>{const card=document.querySelector('article[class*="smallCard"]'),footer=card.querySelector('[class*="cardFooter"]').getBoundingClientRect(),target=document.elementFromPoint(footer.right-1,footer.top+footer.height/2);return target?.closest('a')===card.querySelector('h3 a')})()`
    )
    expect(hit).toBe(true)
  })
  it('should keep every compact action inside its card at tablet width', async () => {
    await session.send('Emulation.setDeviceMetricsOverride', {
      width: 1024,
      height: 1000,
      deviceScaleFactor: 1,
      mobile: false
    })
    await hover('article[class*="smallCard"]')
    const contained = await evaluate<boolean>(
      `(()=>{const card=document.querySelector('article[class*="smallCard"]'),r=card.getBoundingClientRect();return [...card.querySelectorAll('button,a[aria-label*="calendar"]')].every(el=>{const b=el.getBoundingClientRect();return b.left>=r.left&&b.right<=r.right&&b.top>=r.top&&b.bottom<=r.bottom})})()`
    )
    expect(contained).toBe(true)
  })
  it('should expose usable action targets without hover on a touch viewport', async () => {
    await session.send('Emulation.setDeviceMetricsOverride', {
      width: 390,
      height: 900,
      deviceScaleFactor: 1,
      mobile: true
    })
    await session.send('Emulation.setTouchEmulationEnabled', { enabled: true, maxTouchPoints: 1 })
    await evaluate(`document.activeElement?.blur()`)
    const state = await evaluate<{ hover: boolean; visible: string; sizes: number[] }>(
      `(()=>{const card=document.querySelector('article[class*="smallCard"]');return {hover:matchMedia('(hover: hover)').matches,visible:getComputedStyle(card.querySelector('[class*="actions"]')).visibility,sizes:[...card.querySelectorAll('button,a[aria-label*="calendar"]')].map(el=>el.getBoundingClientRect().height)}})()`
    )
    expect(state.hover).toBe(false)
    expect(state.visible).toBe('visible')
    expect(state.sizes).toEqual([44, 44, 44])
    expect(
      await evaluate(
        `getComputedStyle(document.querySelector('article[class*="smallCard"] [class*="time"]')).opacity`
      )
    ).toBe('1')
  })
})
