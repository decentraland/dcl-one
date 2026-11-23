// @vitest-environment node
import { describe, expect, it } from 'vitest'
import { productionBrowser } from './production-browser.helpers'

const browser = productionBrowser('test/consumer/blogcard-browser.tsx')

type Box = { width: number; height: number; titleLines: number }

async function boxes(width: number) {
  await browser.session.send('Emulation.setDeviceMetricsOverride', {
    width,
    height: 900,
    deviceScaleFactor: 1,
    mobile: width < 768
  })
  await browser.session.navigate(browser.origin)
  return browser.evaluate<Record<string, Box>>(`(async () => {
    for (let n = 0; n < 100 && !document.querySelector('#related-loading article'); n++) await new Promise((r) => setTimeout(r, 20))
    await document.fonts.ready
    const out = {}
    for (const id of ['standard-loaded', 'standard-loading', 'related-loaded', 'related-loading']) {
      const card = document.querySelector('#' + id + ' article')
      const box = card.getBoundingClientRect()
      const heading = card.querySelector('h3')
      out[id] = {
        width: box.width,
        height: box.height,
        titleLines: Math.round(heading.getBoundingClientRect().height / parseFloat(getComputedStyle(heading).lineHeight))
      }
    }
    return out
  })()`)
}

describe('when a BlogCard loads', () => {
  it.each([390, 1440])(
    'should keep the loaded card size while loading at %ipx, in both variants',
    async (width) => {
      const cards = await boxes(width)
      // The loaded cards carry the content the skeleton stands for: a two-line title.
      expect(cards['standard-loaded']!.titleLines).toBe(2)
      expect(cards['related-loaded']!.titleLines).toBe(2)
      for (const variant of ['standard', 'related']) {
        const loaded = cards[`${variant}-loaded`]!
        const loading = cards[`${variant}-loading`]!
        expect(loading.width).toBeCloseTo(loaded.width, 1)
        expect(loading.height).toBeCloseTo(loaded.height, 1)
      }
    }
  )
})
