// @vitest-environment node
import { describe, expect, it } from 'vitest'
import { productionBrowser } from './production-browser.helpers'

const browser = productionBrowser('test/consumer/blogcard-browser.tsx')

type Box = { width: number; height: number; titleLines: number }

async function boxes(width: number, standard: number, related: number) {
  await browser.session.send('Emulation.setDeviceMetricsOverride', {
    width,
    height: 900,
    deviceScaleFactor: 1,
    mobile: width < 768
  })
  await browser.session.navigate(`${browser.origin}?standard=${standard}&related=${related}`)
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
  // The column widths the gallery's Blog grids give each variant at that viewport (measured on the BlogCard page).
  it.each([
    [390, 324, 324],
    [768, 332, 332],
    [1440, 421, 377]
  ])(
    'should keep the size of a loaded card with a two-line title and a 24px category at %ipx, in both variants',
    async (width, standard, related) => {
      const cards = await boxes(width, standard, related)
      // The loaded cards carry the content the skeleton is sized for.
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
