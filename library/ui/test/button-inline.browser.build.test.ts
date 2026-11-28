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
  decoration: string
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
        decoration: cs.textDecorationLine,
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

  it('should underline only while hovered', async () => {
    await ready()
    const [body, byline] = await measure()
    expect(body!.decoration).toBe('none')
    expect(byline!.decoration).toBe('underline')
  })
})
