import type { ViteDevServer } from 'vite'
import { afterAll, beforeAll, describe, expect, it } from 'vitest'
import { findBrowser, openSession } from '../figma/review/scripts/capture-browser.mjs'
import { serveConsumerPage } from './consumer-page.helpers'

let server: ViteDevServer
let disruptions: string[] = []
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
  /** Scrolled to the end of the page, where the sticky banner rests in its own place. */
  contentBottom: number
  bannerTop: number
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

async function open(width: number, height: number) {
  await session.send('Emulation.setDeviceMetricsOverride', {
    width,
    height,
    deviceScaleFactor: 1,
    mobile: false
  })
  await session.navigate(`${server.resolvedUrls!.local[0]}`)
  await evaluate(`(async()=>{
    for(let n=0;!document.querySelector('[data-questions]');n++) {
      if(n===100)throw new Error('Help layout fixture did not render')
      await new Promise(resolve=>setTimeout(resolve,50))
    }
    await document.fonts.ready
    // The selected pill tab pops in with a scale animation; measure it at rest (a cancelled one rejects, so settle).
    await Promise.allSettled([...document.querySelectorAll('[role="tab"]')].flatMap((tab)=>tab.getAnimations()).map((animation)=>animation.finished))
    await new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)))
  })()`)
}

async function layoutAt(width: number, height = 1024): Promise<Geometry> {
  await open(width, height)
  return evaluate<Geometry>(`(async()=>{
    const box=(element)=>{const b=element.getBoundingClientRect();return {left:b.left,top:b.top,right:b.right,bottom:b.bottom,width:b.width}}
    const status=box(document.querySelector('main [aria-controls]').parentElement)
    window.scrollTo(0,document.documentElement.scrollHeight)
    await new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)))
    const section=document.querySelector('[role="tabpanel"] section')
    const contentBottom=Math.max(...[...section.children].map((part)=>part.getBoundingClientRect().bottom))
    const bannerTop=document.querySelector('main aside').getBoundingClientRect().top
    window.scrollTo(0,0)
    await new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)))
    return {
      contentBottom,
      bannerTop,
      status,
      viewport:document.documentElement.clientWidth,
      scrollWidth:document.documentElement.scrollWidth,
      title:box(document.querySelector('main h1')),
      tabs:box(document.querySelector('[role="tablist"]')),
      panel:box(document.querySelector('[role="tabpanel"]')),
      tabTops:[...document.querySelectorAll('[role="tab"]')].map((tab)=>Math.round(tab.getBoundingClientRect().top))
    }
  })()`)
}

beforeAll(async () => {
  ;({ server, disruptions } = await serveConsumerPage(
    'test/consumer/help-layout-browser.tsx',
    '<meta name="viewport" content="width=device-width,initial-scale=1"><style>body{margin:0}</style>'
  ))
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
  describe('and the viewport has any supported width', () => {
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
  })

  describe('and the viewport is a tablet', () => {
    it.each([768, 1023])(
      'should stack the section under one row of tabs at %ipx',
      async (width) => {
        const geometry = await layoutAt(width)
        expect(new Set(geometry.tabTops).size).toBe(1)
        expect(geometry.panel.top).toBeGreaterThanOrEqual(geometry.tabs.bottom)
        expect(geometry.panel.left).toBeCloseTo(geometry.title.left, 0)
        expect(geometry.status.top).toBeLessThan(geometry.panel.top)
        expect(geometry.status.right).toBeGreaterThan(geometry.panel.right - 1)
      }
    )
  })

  describe('and the viewport is a narrow desktop', () => {
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
  })

  describe('and the viewport is a wide desktop', () => {
    it.each([1280, 1440])('should keep the status in its own column at %ipx', async (width) => {
      const geometry = await layoutAt(width)
      expect(geometry.status.left).toBeGreaterThanOrEqual(geometry.panel.right)
      expect(geometry.status.top).toBeLessThan(geometry.panel.top)
    })
  })

  describe('and the page is scrolled to its end', () => {
    it.each([
      [844, 390],
      [932, 430],
      [1024, 640],
      [1280, 640],
      [768, 1024],
      [1440, 900]
    ])('should end the section above the chat banner at %ix%i', async (width, height) => {
      const geometry = await layoutAt(width, height)
      expect(geometry.contentBottom).toBeLessThanOrEqual(geometry.bannerTop)
    })
  })

  describe('and the system status is opened, then closed with Escape', () => {
    it.each([390, 768, 1032, 1440])(
      'should open the status inside the viewport and return focus to its toggle at %ipx',
      async (width) => {
        await open(width, 1024)
        const result = await evaluate<{
          left: number
          right: number
          viewport: number
          focused: boolean
          closed: boolean
        }>(`(async()=>{
        const toggle=document.querySelector('main [aria-controls]')
        toggle.click()
        await new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)))
        const popover=document.getElementById(toggle.getAttribute('aria-controls')).getBoundingClientRect()
        document.dispatchEvent(new KeyboardEvent('keydown',{key:'Escape',bubbles:true}))
        await new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)))
        return {left:popover.left,right:popover.right,viewport:document.documentElement.clientWidth,focused:document.activeElement===toggle,closed:toggle.getAttribute('aria-expanded')==='false'}
      })()`)
        expect(result.left).toBeGreaterThanOrEqual(0)
        expect(result.right).toBeLessThanOrEqual(result.viewport)
        expect(result.closed).toBe(true)
        expect(result.focused).toBe(true)
      }
    )
  })

  describe('and the viewport is a 390px phone', () => {
    it('should keep the status beside the title', async () => {
      const geometry = await layoutAt(390)
      expect(geometry.status.top).toBeLessThan(geometry.tabs.top)
      expect(geometry.status.left).toBeGreaterThan(geometry.title.left)
      expect(geometry.panel.top).toBeGreaterThanOrEqual(geometry.tabs.bottom)
    })
  })
})

describe('when the Help page is served', () => {
  // A failed dependency scan or a re-optimization reloads the page under the tests above, which then fail with
  // "Inspected target navigated or closed".
  it('should load it without a dependency scan error or a dependency reload', () => {
    expect(disruptions).toEqual([])
  })
})
