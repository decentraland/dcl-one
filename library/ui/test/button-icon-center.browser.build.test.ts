// @vitest-environment node
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { waitForFixture } from './fixture-state.helpers'
import { productionBrowser } from './production-browser.helpers'

const b = productionBrowser('test/consumer/button-icon-center.tsx')

type Offset = { name: string; width: number; height: number; dx: number; dy: number }

async function ready() {
  const errorStart = b.session.runtimeErrors.length
  await b.session.navigate(b.origin)
  await waitForFixture(
    'Icon-only Button fixture',
    () =>
      b.evaluate<boolean>(
        `Boolean(document.querySelector('[data-text]')?.getBoundingClientRect().width && document.fonts.status === 'loaded')`
      ),
    Boolean,
    () => b.evaluate('new Promise(resolve=>requestAnimationFrame(()=>resolve(null)))'),
    () => b.session.runtimeErrors.slice(errorStart)
  )
}

function offsets() {
  return b.evaluate<Offset[]>(
    `Array.from(document.querySelectorAll('[data-case]'), button => {
      const r = button.getBoundingClientRect(), i = button.querySelector('[data-glyph]').getBoundingClientRect()
      return {
        name: button.dataset.case,
        width: r.width,
        height: r.height,
        dx: i.left + i.width / 2 - (r.left + r.width / 2),
        dy: i.top + i.height / 2 - (r.top + r.height / 2)
      }
    })`
  )
}

describe('when an icon-only Button holds an inline icon', () => {
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
    'should center the icon in its box for every variant, size and viewport at %ipx',
    async (width, height) => {
      await b.session.send('Emulation.setDeviceMetricsOverride', {
        width,
        height,
        deviceScaleFactor: 1,
        mobile: false
      })
      await ready()
      const measured = await offsets()
      expect(measured).toHaveLength(2 * 4 * (10 + 4 + 10) + 1)
      const offCenter = measured.filter((m) => Math.abs(m.dx) > 0.5 || Math.abs(m.dy) > 0.5)
      expect(offCenter).toEqual([])
      for (const m of measured) expect(m.height).toBeGreaterThanOrEqual(48)
    }
  )

  it('should keep a text Button label on its text baseline', async () => {
    await ready()
    expect(
      await b.evaluate(
        `getComputedStyle(document.querySelector('[data-text] [data-button-label]')).display`
      )
    ).toBe('block')
  })
})
