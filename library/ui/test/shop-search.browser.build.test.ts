// @vitest-environment node
import { describe, expect, it } from 'vitest'
import { productionBrowser } from './production-browser.helpers'
import { waitForFixture } from './fixture-state.helpers'

const browser = productionBrowser('test/consumer/shop-search.tsx')

describe('Shop search with only the shared foundations loaded', () => {
  it('should retain a visible panel and the responsive field dimensions', async () => {
    await browser.session.navigate(browser.origin)
    await waitForFixture(
      'Shop search',
      () => browser.evaluate("Boolean(document.querySelector('[role=combobox]'))"),
      Boolean,
      () => browser.evaluate('new Promise(resolve=>requestAnimationFrame(()=>resolve(null)))'),
      () => browser.session.runtimeErrors
    )
    for (const [width, touch, fontSize] of [
      [1440, false, '17px'],
      [768, false, '14px'],
      [390, false, '14px'],
      [390, true, '16px']
    ] as const) {
      await browser.session.send('Emulation.setTouchEmulationEnabled', { enabled: touch })
      await browser.session.send('Emulation.setDeviceMetricsOverride', {
        width,
        height: 900,
        deviceScaleFactor: 1,
        mobile: false
      })
      expect(
        await browser.evaluate(`(() => {
        const field = document.querySelector('[role=combobox]')
        const form = field.closest('form')
        const panel = document.querySelector('[data-testid=search-panel]')
        return [
          form.getBoundingClientRect().height,
          getComputedStyle(field).fontSize,
          getComputedStyle(panel).backgroundColor,
          getComputedStyle(panel).color
        ]
      })()`)
      ).toEqual([40, fontSize, 'rgb(60, 21, 86)', 'rgb(252, 252, 252)'])
    }
  })

  it('should keep both edges of the open panel aligned with its field in a narrow container', async () => {
    await browser.session.navigate(browser.origin)
    await waitForFixture(
      'open Shop search panel',
      () => browser.evaluate("Boolean(document.querySelector('[data-testid=search-panel]'))"),
      Boolean,
      () => browser.evaluate('new Promise(resolve=>requestAnimationFrame(()=>resolve(null)))'),
      () => browser.session.runtimeErrors
    )
    for (const width of [768, 390]) {
      await browser.session.send('Emulation.setDeviceMetricsOverride', {
        width,
        height: 900,
        deviceScaleFactor: 1,
        mobile: false
      })
      for (const parentWidth of ['100%', '250px']) {
        const bounds = await browser.evaluate<{ field: number[]; panel: number[] }>(`(() => {
          document.getElementById('root').style.width = ${JSON.stringify(parentWidth)}
          const rect = (element) => {
            const { left, right } = element.getBoundingClientRect()
            return [left, right]
          }
          return {
            field: rect(document.querySelector('form')),
            panel: rect(document.querySelector('[data-testid=search-panel]'))
          }
        })()`)
        expect(bounds.panel).toEqual(bounds.field)
        expect(bounds.panel[0]).toBeGreaterThanOrEqual(0)
        expect(bounds.panel[1]).toBeLessThanOrEqual(width)
      }
    }
  })
})
