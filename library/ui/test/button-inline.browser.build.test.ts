// @vitest-environment node
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { waitForFixture } from './fixture-state.helpers'
import { productionBrowser } from './production-browser.helpers'

const b = productionBrowser('test/consumer/button-inline.tsx')

type Measured = {
  name: string
  font: [string, string, string, string]
  parentFont: [string, string, string, string]
  color: string
  parentColor: string
  letterSpacing: string
  textTransform: string
  padding: string
  borderWidth: string
  background: string
  height: number
  lineTop: number
  wordTop: number
}

async function ready() {
  const errorStart = b.session.runtimeErrors.length
  await b.session.navigate(b.origin)
  await waitForFixture(
    'Inline Button fixture',
    () =>
      b.evaluate<boolean>(
        `Boolean(document.querySelector('[data-case] button')?.getBoundingClientRect().width && document.fonts.status === 'loaded')`
      ),
    Boolean,
    () => b.evaluate('new Promise(resolve=>requestAnimationFrame(()=>resolve(null)))'),
    () => b.session.runtimeErrors.slice(errorStart)
  )
}

const measure = () =>
  b.evaluate<Measured[]>(
    `Array.from(document.querySelectorAll('[data-case]'), p => {
      const button = p.querySelector('button'), cs = getComputedStyle(button), ps = getComputedStyle(p)
      const range = document.createRange(); range.selectNodeContents(button.querySelector('[data-button-label]'))
      const word = p.querySelector('[data-before]').getBoundingClientRect()
      const font = s => [s.fontFamily, s.fontSize, s.fontWeight, s.lineHeight]
      return {
        name: p.dataset.case,
        font: font(cs),
        parentFont: font(ps),
        color: cs.color,
        parentColor: ps.color,
        letterSpacing: cs.letterSpacing,
        textTransform: cs.textTransform,
        padding: cs.padding,
        borderWidth: cs.borderTopWidth,
        background: cs.backgroundColor,
        height: button.getBoundingClientRect().height,
        lineTop: range.getBoundingClientRect().top,
        wordTop: word.top
      }
    })`
  )

describe('when a Button is set inline in running text', () => {
  let caseErrorStart = 0
  beforeEach(() => {
    caseErrorStart = b.session.runtimeErrors.length
  })
  afterEach(async () => {
    expect(b.session.runtimeErrors.slice(caseErrorStart)).toEqual([])
    await b.session.send('Emulation.clearDeviceMetricsOverride')
  })

  it.each([
    [1440, 900],
    [390, 844]
  ])(
    "should take the text's font and colour, draw no box and sit on its line at %ipx",
    async (width, height) => {
      await b.session.send('Emulation.setDeviceMetricsOverride', {
        width,
        height,
        deviceScaleFactor: 1,
        mobile: false
      })
      await ready()
      for (const m of await measure()) {
        expect(m.font, m.name).toEqual(m.parentFont)
        expect(m.color, m.name).toBe(m.parentColor)
        expect(m.letterSpacing, m.name).toBe('normal')
        expect(m.textTransform, m.name).toBe('none')
        expect(m.padding, m.name).toBe('0px')
        expect(m.borderWidth, m.name).toBe('0px')
        expect(m.background, m.name).toBe('rgba(0, 0, 0, 0)')
        expect(m.height, m.name).toBe(Number.parseFloat(m.parentFont[3]))
        expect(Math.abs(m.lineTop - m.wordTop), m.name).toBeLessThanOrEqual(0.5)
      }
    }
  )

  it('should draw an underline under the label only while the pointer is over it', async () => {
    await ready()
    const box = await b.evaluate<{
      x: number
      y: number
      width: number
      height: number
      cx: number
      cy: number
    }>(
      `(() => {
        const button = document.querySelector('[data-case="byline"] button')
        const range = document.createRange(); range.selectNodeContents(button.querySelector('[data-button-label]'))
        const r = range.getBoundingClientRect(), c = button.getBoundingClientRect()
        return { x: r.left, y: r.top, width: r.width, height: r.height, cx: c.left + c.width / 2, cy: c.top + c.height / 2 }
      })()`
    )
    const clip = { x: box.x, y: box.y, width: box.width, height: box.height, scale: 1 }
    const shot = async () =>
      (
        (await b.session.send('Page.captureScreenshot', { format: 'png', clip })) as {
          data: string
        }
      ).data
    const decoration = () =>
      b.evaluate<string>(
        `getComputedStyle(document.querySelector('[data-case="byline"] [data-button-label]')).textDecorationLine`
      )
    await b.session.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: 1, y: 1 })
    const atRest = await shot()
    expect(await decoration()).toBe('none')
    await b.session.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: box.cx, y: box.cy })
    await b.evaluate(
      'new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(()=>resolve(null))))'
    )
    expect(await decoration()).toBe('underline')
    expect(await shot()).not.toBe(atRest)
    await b.session.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: 1, y: 1 })
    await b.evaluate(
      'new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(()=>resolve(null))))'
    )
    expect(await shot()).toBe(atRest)
  })

  it('should draw a 2px primary text ring 2px out when focused by keyboard', async () => {
    await ready()
    await b.evaluate('document.activeElement?.blur()')
    await b.session.send('Input.dispatchKeyEvent', {
      type: 'keyDown',
      key: 'Tab',
      code: 'Tab',
      windowsVirtualKeyCode: 9
    })
    await b.session.send('Input.dispatchKeyEvent', {
      type: 'keyUp',
      key: 'Tab',
      code: 'Tab',
      windowsVirtualKeyCode: 9
    })
    const ring = await b.evaluate<{ focused: boolean; ring: string }>(
      `(() => {
        const button = document.querySelector('[data-case="body"] button'), cs = getComputedStyle(button)
        return { focused: document.activeElement === button, ring: [cs.outlineStyle, cs.outlineWidth, cs.outlineColor, cs.outlineOffset].join(' ') }
      })()`
    )
    expect(ring).toEqual({ focused: true, ring: 'solid 2px rgb(252, 252, 252) 2px' })
  })
})
