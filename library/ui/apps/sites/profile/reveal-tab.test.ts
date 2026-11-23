// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it } from 'vitest'

import { revealTab } from './reveal-tab'

type Box = { left: number; right: number }

/** A tab bar 300px wide at the page's left edge, scrolled by `scrollLeft`, with one tab at `tab` in the bar's content. */
function barWith(tab: Box, scrollLeft: number) {
  const bar = document.createElement('div')
  bar.setAttribute('role', 'tablist')
  const link = document.createElement('a')
  bar.append(link)
  document.body.append(bar)
  bar.scrollLeft = scrollLeft
  bar.getBoundingClientRect = () => ({ left: 0, right: 300 }) as DOMRect
  link.getBoundingClientRect = () =>
    ({ left: tab.left - bar.scrollLeft, right: tab.right - bar.scrollLeft }) as DOMRect
  return { bar, link }
}

afterEach(() => document.body.replaceChildren())

describe('revealTab', () => {
  let bar: HTMLElement
  let link: HTMLElement

  describe('when the tab is past the end edge of the bar', () => {
    beforeEach(() => {
      ;({ bar, link } = barWith({ left: 420, right: 560 }, 0))
    })

    it('should scroll the bar until the tab is inside it, with the gutter beside it', () => {
      revealTab(link)

      expect(bar.scrollLeft).toBe(560 - 300 + 16)
    })
  })

  describe('when the tab is before the start edge of the bar', () => {
    beforeEach(() => {
      ;({ bar, link } = barWith({ left: 100, right: 180 }, 200))
    })

    it('should scroll the bar back to the tab, with the gutter beside it', () => {
      revealTab(link)

      expect(bar.scrollLeft).toBe(100 - 16)
    })
  })

  describe('when the tab is already inside the bar', () => {
    beforeEach(() => {
      ;({ bar, link } = barWith({ left: 40, right: 120 }, 0))
    })

    it('should leave the bar alone', () => {
      revealTab(link)

      expect(bar.scrollLeft).toBe(0)
    })

    describe('and there is no tab or the tab has no bar', () => {
      let orphan: HTMLAnchorElement

      beforeEach(() => {
        orphan = document.createElement('a')
      })

      it('should do nothing', () => {
        revealTab(null)
        revealTab(orphan)

        expect(bar.scrollLeft).toBe(0)
      })
    })
  })
})
