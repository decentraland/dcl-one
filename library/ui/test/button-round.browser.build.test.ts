// @vitest-environment node
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { waitForFixture } from './fixture-state.helpers'
import { productionBrowser } from './production-browser.helpers'

const b = productionBrowser('test/consumer/button-round.tsx')

type Box = {
  name: string
  round: number
  iconSize: number
  width: number
  height: number
  radius: string
  iconWidth: number
  iconHeight: number
  dx: number
  dy: number
}

async function ready() {
  const errorStart = b.session.runtimeErrors.length
  await b.session.navigate(b.origin)
  await waitForFixture(
    'Round Button fixture',
    () =>
      b.evaluate<boolean>(
        `Boolean(document.querySelector('[data-unsized-case]')?.getBoundingClientRect().width)`
      ),
    Boolean,
    () => b.evaluate('new Promise(resolve=>requestAnimationFrame(()=>resolve(null)))'),
    () => b.session.runtimeErrors.slice(errorStart)
  )
}

const measure = (selector: string) =>
  b.evaluate<Box[]>(
    `Array.from(document.querySelectorAll(${JSON.stringify(selector)}), button => {
      const r = button.getBoundingClientRect(), i = button.querySelector('[data-glyph]').getBoundingClientRect()
      return {
        name: button.dataset.case ?? '',
        round: Number(button.dataset.roundCase),
        iconSize: Number(button.dataset.iconCase ?? button.dataset.sizedCase),
        width: r.width,
        height: r.height,
        radius: getComputedStyle(button).borderRadius,
        iconWidth: i.width,
        iconHeight: i.height,
        dx: i.left + i.width / 2 - (r.left + r.width / 2),
        dy: i.top + i.height / 2 - (r.top + r.height / 2)
      }
    })`
  )

describe('when an icon-only Button is round or sets its icon size', () => {
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
    'should draw each round Button its diameter across, a circle with its icon centred, at %ipx',
    async (width, height) => {
      await b.session.send('Emulation.setDeviceMetricsOverride', {
        width,
        height,
        deviceScaleFactor: 1,
        mobile: false
      })
      await ready()
      const measured = await measure('[data-round-case]')
      expect(measured).toHaveLength(3 * 3 * 4 + 1)
      const wrong = measured.filter(
        (m) =>
          m.width !== m.round ||
          m.height !== m.round ||
          m.radius !== '9999px' ||
          m.iconWidth !== m.iconSize ||
          m.iconHeight !== m.iconSize ||
          Math.abs(m.dx) > 0.5 ||
          Math.abs(m.dy) > 0.5
      )
      expect(wrong).toEqual([])
    }
  )

  it('should force the icon size of a square icon-only Button and keep its box', async () => {
    await ready()
    const sized = await measure('[data-sized-case]')
    expect(sized.map((m) => [m.iconWidth, m.iconHeight])).toEqual([
      [14, 14],
      [16, 16],
      [20, 20],
      [24, 24]
    ])
    const [unsized] = await measure('[data-unsized-case]')
    expect(unsized!.iconWidth).toBe(30)
    for (const m of sized) {
      expect(m.width).toBe(unsized!.width)
      expect(m.height).toBe(unsized!.height)
    }
  })
})
