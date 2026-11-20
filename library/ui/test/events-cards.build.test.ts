import path from 'node:path'
import { createServer, type ViteDevServer } from 'vite'
import { afterAll, beforeAll, beforeEach, describe, expect, it } from 'vitest'
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
        '<!doctype html><meta name="viewport" content="width=device-width,initial-scale=1"><style>body{margin:0;background:#21102f}main{margin:0 auto;padding:0 32px;max-width:1280px}[data-day],[data-live-day],[data-live-small],[data-calendar]{width:300px}</style><div id="root"></div><script type="module" src="/test/consumer/events-cards-browser.tsx"></script>'
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

// Turning touch emulation off gives the open document the host's own hover and pointer media; only the next
// document gets the capture browser's desktop defaults. Tests reset input here, before navigating, never after.
beforeEach(async () => {
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
  await session.navigate(server.resolvedUrls!.local[0]!)
  await evaluate(
    `(async()=>{for(let n=0;n<100 && (document.querySelectorAll('article').length!==13||document.querySelectorAll('[class*="liveCard"]').length!==2);n++)await new Promise(r=>setTimeout(r,100));await document.fonts.ready})()`
  )
  await evaluate(`document.activeElement?.blur();scrollTo(0,0)`)
  await session.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: 0, y: 0 })
})

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
const UPCOMING = 'article[class*="small"]:not([class*="isLive"])'

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
      const selector = 'div[class*="liveCard"]'
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
  it('should swap time for controls without resizing the day card on hover and focus', async () => {
    const variant = 'day'
    await session.send('Emulation.setDeviceMetricsOverride', {
      width: 1440,
      height: 1000,
      deviceScaleFactor: 1,
      mobile: false
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
    // Leaving the card starts the fade-out; focus reverses it from wherever it is, so read it once it ends.
    expect(
      await evaluate(
        `(async()=>{const actions=document.querySelector(${JSON.stringify(selector)}).querySelector('[class*="actions"]');await Promise.all(actions.getAnimations().map(a=>a.finished));return getComputedStyle(actions).opacity})()`
      )
    ).toBe('1')
    expect((await rect(selector)).height).toBeCloseTo(before.height, 1)
  })
  describe('when a live event is in the desktop day column', () => {
    it('should grow only the cover on hover and keep Jump In visible at rest', async () => {
      const card = '[data-live-day] article'
      const cover = card + ' [class*="media"]'
      const before = await rect(card)
      const mediaBefore = await rect(cover)
      await hover(card)
      const after = await rect(card)
      expect(after.width).toBe(before.width)
      expect((await rect(cover)).height).toBeGreaterThan(mediaBefore.height)
      expect(await evaluate(`getComputedStyle(document.querySelector('${card}')).boxShadow`)).toBe(
        'none'
      )
      await evaluate(`document.querySelector('${card}').scrollIntoView({block:"center"})`)
      await session.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: 0, y: 0 })
      expect(
        await evaluate(`(() => {
          const link = document.querySelector('${card} a[href^="/play"]')
          const r = link.getBoundingClientRect()
          return document.elementFromPoint(r.x + r.width / 2, r.y + r.height / 2)?.closest('a') === link
        })()`)
      ).toBe(true)
    })
  })

  describe('when All Events cards sit inside a scrolling day column', () => {
    const column = '[data-calendar] ul[aria-label="Events for Today"]'
    const live = column + ' li:first-child article'
    const future = column + ' li:nth-child(2) article'

    it('should keep the live accent and hover stable when the first cover expands above a stationary pointer', async () => {
      await evaluate(`document.querySelector('${column}').scrollIntoView({block:'center'})`)
      const box = await rect(live)
      const cover = live + ' [class*="media"]'
      const initial = (await rect(cover)).height
      await session.send('Input.dispatchMouseEvent', {
        type: 'mouseMoved',
        x: box.x + box.width / 2,
        y: box.y + initial / 2
      })
      const result = await evaluate<{
        heights: number[]
        accent: string
        shadow: string
      }>(`(async () => {
        const card = document.querySelector('${live}')
        const cover = document.querySelector('${cover}')
        const heights = []
        for (let i = 0; i < 8; i++) {
          await new Promise(resolve => setTimeout(resolve, 55))
          heights.push(cover.getBoundingClientRect().height)
        }
        return { heights, accent: getComputedStyle(card, '::before').width, shadow: getComputedStyle(card).boxShadow }
      })()`)
      expect(result.accent).toBe('5px')
      expect(result.shadow).toBe('none')
      expect(result.heights.at(-1)).toBeGreaterThan(initial)
      expect(result.heights.at(-1)).toBe(result.heights.at(-2))
      expect(
        result.heights.every((height, index) => index === 0 || height >= result.heights[index - 1]!)
      ).toBe(true)
      expect((await rect(live)).x).toBe(box.x)
    })

    it('should keep the future hover highlight inset with the card partly clipped by the column', async () => {
      await evaluate(`(() => {
        const column = document.querySelector('${column}')
        column.scrollIntoView({block:'center'})
        const card = document.querySelector('${future}')
        column.scrollTop = card.getBoundingClientRect().top - column.getBoundingClientRect().top + 12
      })()`)
      const before = await rect(future)
      const bounds = await rect(column)
      expect(before.y).toBeLessThan(bounds.y)
      await session.send('Input.dispatchMouseEvent', {
        type: 'mouseMoved',
        x: before.x + before.width / 2,
        y: bounds.y + 20
      })
      await evaluate(`new Promise(resolve => setTimeout(resolve, 250))`)
      expect(await rect(future)).toEqual(before)
      expect(
        await evaluate(`getComputedStyle(document.querySelector('${future}')).boxShadow`)
      ).toContain('inset')
      expect(
        await evaluate(`getComputedStyle(document.querySelector('${column}')).overflowY`)
      ).toBe('auto')
    })

    it.each(['hover', 'keyboard focus'])(
      'should fit the reminder label before the calendar action on %s',
      async (interaction) => {
        if (interaction === 'hover') {
          await hover(future)
        } else {
          await evaluate(`document.querySelector('${future} h3 a').focus()`)
          for (let i = 0; i < 2; i++) {
            await session.send('Input.dispatchKeyEvent', {
              type: 'rawKeyDown',
              key: 'Tab',
              code: 'Tab'
            })
            await session.send('Input.dispatchKeyEvent', { type: 'keyUp', key: 'Tab', code: 'Tab' })
          }
          expect(await evaluate(`document.activeElement.getAttribute('aria-label')`)).toBe(
            'Remind me: Future event 1'
          )
        }

        const layout = await evaluate(`(async () => {
          const card = document.querySelector('${future}')
          const button = card.querySelector('button[aria-label^="Remind me:"]')
          const row = button.parentElement
          await Promise.all(row.getAnimations().map(animation => animation.finished))
          const label = button.lastElementChild.getBoundingClientRect()
          const bounds = button.getBoundingClientRect()
          const calendar = card.querySelector('a[aria-label^="Add to calendar:"]').getBoundingClientRect()
          return {
            visible: getComputedStyle(row).opacity === '1',
            fitsButton: label.left >= bounds.left && label.right <= bounds.right,
            beforeCalendar: label.right <= calendar.left
          }
        })()`)
        expect(layout).toEqual({ visible: true, fitsButton: true, beforeCalendar: true })
      }
    )

    it('should keep all three future actions inside a narrow desktop day card', async () => {
      await evaluate(`document.querySelector('[data-calendar]').style.width = '212px'`)
      await hover(future)
      const fits = await evaluate(`(() => {
        const card = document.querySelector('${future}')
        const button = card.querySelector('button[aria-pressed]')
        const row = button.parentElement
        const label = button.lastElementChild
        const copy = row.lastElementChild
        return label.clientWidth === label.scrollWidth &&
          copy.getBoundingClientRect().right <= card.getBoundingClientRect().right
      })()`)
      expect(fits).toBe(true)
    })

    it('should reach the live card and Jump In by keyboard with a visible focus outline', async () => {
      const cardLink = live + ' h3 a'
      await evaluate(`document.querySelector('${cardLink}').focus()`)
      await session.send('Input.dispatchKeyEvent', { type: 'rawKeyDown', key: 'Tab', code: 'Tab' })
      await session.send('Input.dispatchKeyEvent', { type: 'keyUp', key: 'Tab', code: 'Tab' })
      await session.send('Input.dispatchKeyEvent', { type: 'rawKeyDown', key: 'Tab', code: 'Tab' })
      await session.send('Input.dispatchKeyEvent', { type: 'keyUp', key: 'Tab', code: 'Tab' })
      expect(await evaluate(`document.activeElement.getAttribute('href')`)).toBe(
        '/play?realm=gallery.dcl.eth'
      )
      expect(await evaluate(`getComputedStyle(document.activeElement).outlineStyle`)).not.toBe(
        'none'
      )
    })
    it('should skip the cover transition with reduced motion enabled', async () => {
      await session.send('Emulation.setEmulatedMedia', {
        features: [{ name: 'prefers-reduced-motion', value: 'reduce' }]
      })
      try {
        await hover(live)
        const cover = live + ' [class*="media"]'
        expect(
          await evaluate(`getComputedStyle(document.querySelector('${cover}')).transitionDuration`)
        ).toBe('0s')
      } finally {
        await session.send('Emulation.setEmulatedMedia', { features: [] })
      }
    })

    it('should use the existing live small card and accessible Jump In at 390px on touch', async () => {
      await session.send('Emulation.setDeviceMetricsOverride', {
        width: 390,
        height: 900,
        deviceScaleFactor: 1,
        mobile: true
      })
      await session.send('Emulation.setTouchEmulationEnabled', { enabled: true })
      await session.navigate(server.resolvedUrls!.local[0]!)
      const state = await evaluate(`(async () => {
        for (let i = 0; i < 100 && !document.querySelector('${live}'); i++) await new Promise(resolve => setTimeout(resolve, 25))
        const card = document.querySelector('${live}')
        const image = card.querySelector('[class*="media"]').getBoundingClientRect()
        const launch = card.querySelector('a[href^="/play"]')
        return {
          small: card.className.includes('small'),
          hover: matchMedia('(hover: hover)').matches,
          media: [image.width, image.height],
          action: launch.getAttribute('href'),
          visible: launch.getBoundingClientRect().width > 0 && launch.getBoundingClientRect().height > 0
        }
      })()`)
      expect(state).toEqual({
        small: true,
        hover: false,
        media: [135, 135],
        action: '/play?realm=gallery.dcl.eth',
        visible: true
      })
    })
  })

  describe('when the card link is copied', () => {
    it('should visibly announce success and let the user retry after a denial', async () => {
      await hover('[data-day] article')
      expect(
        await evaluate(`(async () => {
          const card = document.querySelector('[data-day] article')
          const copy = card.querySelector('button[aria-label^="Copy link"]')
          const notice = card.querySelector('[role="status"]')
          let url
          Object.defineProperty(navigator, 'clipboard', {
            configurable: true,
            value: { writeText: async (text) => { url = text; throw new DOMException('Denied', 'NotAllowedError') } }
          })
          copy.click()
          await new Promise(requestAnimationFrame)
          const failure = notice.textContent
          navigator.clipboard.writeText = async (text) => { url = text }
          copy.click()
          await new Promise(requestAnimationFrame)
          return { failure, url: new URL(url).pathname + new URL(url).search, success: notice.textContent, visible: notice.getBoundingClientRect().height > 1 }
        })()`)
      ).toEqual({
        failure: "Couldn't copy the link. Try again.",
        url: '/events?id=event',
        success: 'Copied',
        visible: true
      })
    })
  })

  describe('when activating an event reminder', () => {
    it.each([
      ['no-preference', true],
      ['reduce', false]
    ])('should respect prefers-reduced-motion: %s', async (value, animated) => {
      await session.send('Emulation.setEmulatedMedia', {
        features: [{ name: 'prefers-reduced-motion', value }]
      })
      try {
        await hover('[data-day] article')
        expect(
          await evaluate(`(() => {
            const button = document.querySelector('[data-day] button[aria-pressed]')
            button.click()
            return new Promise(resolve => requestAnimationFrame(() => resolve({
              pressed: button.getAttribute('aria-pressed'),
              animated: button.querySelector('svg').getAnimations().length > 0
            })))
          })()`)
        ).toEqual({ pressed: 'true', animated })
      } finally {
        await session.send('Emulation.setEmulatedMedia', { features: [] })
      }
    })
  })

  it('should fold the host and open the small card controls under the time without resizing it', async () => {
    await evaluate(`document.activeElement?.blur()`)
    expect(await evaluate(`matchMedia('(hover: hover)').matches`)).toBe(true)
    const selector = UPCOMING
    const before = await rect(selector)
    await hover(selector)
    const after = await rect(selector)
    expect(after.width).toBeCloseTo(before.width, 1)
    expect(after.height).toBeCloseTo(before.height, 1)
    const state = await evaluate<{ actions: string; host: string; time: string; buttons: number }>(
      `(()=>{const card=document.querySelector(${JSON.stringify(selector)});return {actions:getComputedStyle(card.querySelector('[class*="showOnHover"]')).opacity,host:getComputedStyle(card.querySelector('[class*="hideOnHover"]')).opacity,time:getComputedStyle(card.querySelector('[class*="smallTime"]').closest('[class*="reveal"]')).opacity,buttons:card.querySelectorAll('button,a[aria-label*="calendar"]').length}})()`
    )
    expect(state).toEqual({ actions: '1', host: '0', time: '1', buttons: 3 })
    // The host folds away under the pointer, so it holds no link a pointer could not reach.
    expect(
      await evaluate(
        `document.querySelector(${JSON.stringify(selector)}).querySelector('[class*="hideOnHover"] a')`
      )
    ).toBeNull()
    await session.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: 0, y: 0 })
    await evaluate(
      `document.querySelector(${JSON.stringify(selector)}).querySelector('h3 a').focus()`
    )
    expect(
      await evaluate(
        `(async()=>{const row=document.querySelector(${JSON.stringify(selector)}).querySelector('[class*="showOnHover"]');await Promise.all(row.getAnimations().map(a=>a.finished));return getComputedStyle(row).opacity})()`
      )
    ).toBe('1')
    expect((await rect(selector)).height).toBeCloseTo(before.height, 1)
  })
  it('should swap a live small card host and time for JUMP IN and share on hover', async () => {
    const selector = '[data-live-small] article'
    const before = await rect(selector)
    await hover(selector)
    const after = await rect(selector)
    expect(after.height).toBeCloseTo(before.height, 1)
    const state = await evaluate<{
      folded: string[]
      jump: boolean
      share: boolean
      inside: boolean
    }>(
      `(()=>{const card=document.querySelector(${JSON.stringify(selector)}),r=card.getBoundingClientRect(),jump=card.querySelector('a[aria-label^="Jump in"]'),b=jump.getBoundingClientRect();return {folded:[...card.querySelectorAll('[class*="hideOnHover"]')].map(el=>getComputedStyle(el).opacity),jump:getComputedStyle(jump.closest('[class*="showOnHover"]')).opacity==='1',share:Boolean(card.querySelector('button[aria-label^="Copy link"]')),inside:b.left>=r.left&&b.right<=r.right&&b.top>=r.top&&b.bottom<=r.bottom}})()`
    )
    expect(state).toEqual({ folded: ['0', '0'], jump: true, share: true, inside: true })
  })
  it.each([390, 768, 1024, 1440])(
    'should retain two complete title lines with and without a creator at %ipx',
    async (width) => {
      await session.send('Emulation.setDeviceMetricsOverride', {
        width,
        height: 1000,
        deviceScaleFactor: 1,
        mobile: width === 390
      })
      if (width === 390)
        await session.send('Emulation.setTouchEmulationEnabled', {
          enabled: true,
          maxTouchPoints: 1
        })
      const titles = await evaluate<
        { height: number; lineHeight: number; footerInside: boolean; clamp: string }[]
      >(
        `(()=>[...document.querySelectorAll(${JSON.stringify(UPCOMING)})].map(card=>{const title=card.querySelector('h3'),style=getComputedStyle(title),footer=card.querySelector('[class*="actionsRow"]');return {height:title.getBoundingClientRect().height,lineHeight:parseFloat(style.lineHeight),clamp:style.webkitLineClamp,footerInside:footer.getBoundingClientRect().bottom<=card.getBoundingClientRect().bottom}}))()`
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
      `document.querySelectorAll(${JSON.stringify(UPCOMING)})[1].querySelector('h3 a').focus()`
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
      `(()=>{const card=document.querySelector(${JSON.stringify(UPCOMING)});return {label:document.activeElement.getAttribute('aria-label'),first:card.contains(document.activeElement),opacity:getComputedStyle(card.querySelector('[class*="showOnHover"]')).opacity}})()`
    )
    expect(focused.label).toContain('Copy link:')
    expect(focused.first).toBe(true)
    await evaluate(`new Promise(r=>setTimeout(r,250))`)
    expect(
      await evaluate(
        `getComputedStyle(document.querySelector(${JSON.stringify(UPCOMING)}).querySelector('[class*="showOnHover"]')).opacity`
      )
    ).toBe('1')
  })
  it('should let uncovered action row space open the card instead of intercepting the pointer', async () => {
    await hover(UPCOMING)
    const hit = await evaluate<boolean>(
      `(()=>{const card=document.querySelector(${JSON.stringify(UPCOMING)}),footer=card.querySelector('[class*="actionsRow"]').getBoundingClientRect(),target=document.elementFromPoint(footer.right-1,footer.top+footer.height/2);return target?.closest('a')===card.querySelector('h3 a')})()`
    )
    expect(hit).toBe(true)
  })
  it('should keep every small card action inside its card at tablet width', async () => {
    await session.send('Emulation.setDeviceMetricsOverride', {
      width: 1024,
      height: 1000,
      deviceScaleFactor: 1,
      mobile: false
    })
    await hover(UPCOMING)
    const contained = await evaluate<boolean>(
      `(()=>{const card=document.querySelector(${JSON.stringify(UPCOMING)}),r=card.getBoundingClientRect();return [...card.querySelectorAll('button,a[aria-label*="calendar"]')].every(el=>{const b=el.getBoundingClientRect();return b.left>=r.left&&b.right<=r.right&&b.top>=r.top&&b.bottom<=r.bottom})})()`
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
    const state = await evaluate<{ hover: boolean; visible: boolean; sizes: number[] }>(
      `(()=>{const card=document.querySelector(${JSON.stringify(UPCOMING)});return {hover:matchMedia('(hover: hover)').matches,visible:card.querySelector('[class*="showOnHover"]').getBoundingClientRect().height>0&&getComputedStyle(card.querySelector('[class*="showOnHover"]')).opacity==='1',sizes:[...card.querySelectorAll('button,a[aria-label*="calendar"]')].map(el=>el.getBoundingClientRect().height)}})()`
    )
    expect(state.hover).toBe(false)
    expect(state.visible).toBe(true)
    expect(state.sizes).toEqual([44, 44, 44])
    const layout = await evaluate<{
      cover: number[]
      card: number
      host: string
      hostShown: boolean
    }>(
      `(()=>{const card=document.querySelector(${JSON.stringify(UPCOMING)}),cover=card.querySelector('[class*="media"]').getBoundingClientRect(),host=card.querySelector('[class*="hideOnHover"]');return {cover:[cover.width,cover.height],card:card.getBoundingClientRect().height,host:getComputedStyle(host).opacity,hostShown:host.getBoundingClientRect().height>0}})()`
    )
    expect(layout.cover).toEqual([135, 135])
    expect(layout.card).toBeGreaterThan(135)
    expect(layout).toMatchObject({ host: '1', hostShown: true })
    const live = await evaluate<{ jump: boolean; share: boolean; time: boolean }>(
      `(()=>{const card=document.querySelector('[data-live-small] article');return {jump:Boolean(card.querySelector('a[aria-label^="Jump in"]')?.getBoundingClientRect().height),share:Boolean(card.querySelector('button[aria-label^="Copy link"]')?.getBoundingClientRect().height),time:Boolean(card.querySelector('time')?.getBoundingClientRect().height)}})()`
    )
    expect(live).toEqual({ jump: true, share: true, time: true })
    expect(
      await evaluate(
        `getComputedStyle(document.querySelector(${JSON.stringify(UPCOMING)}).querySelector('[class*="smallTime"]')).opacity`
      )
    ).toBe('1')
  })
})
