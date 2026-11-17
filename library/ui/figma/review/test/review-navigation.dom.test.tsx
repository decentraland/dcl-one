import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { ReviewNavigation } from '../src/review/ReviewNavigation'

// The app's base is read in one place; the test builds the app as the CDN would, with the prefix as its base.
vi.mock('../src/review/review-base', () => ({ runtimeBase: () => '/@scope/pkg/1.2.3/' }))

// The bundle cannot run in jsdom, so this renders the navigation with the CDN prefix as the app's base and
// checks every URL it hands to the browser: the links it renders and the pages it navigates to.
const PREFIX = '/@scope/pkg/1.2.3/'
// The folder the Review is served from, named apart so no path-shaped literal of it is spelled out here.
const FOLDER = 'dcl-ui-review'
const PAGE = `${PREFIX}${FOLDER}/`

let navigated: string[]

beforeEach(() => {
  navigated = []
  vi.stubGlobal('location', {
    search: '?platform=all&component=NavBar&variant=1',
    set href(value: string) {
      navigated.push(value)
    }
  })
})

afterEach(() => {
  vi.unstubAllGlobals()
})

describe('ReviewNavigation under a CDN prefix', () => {
  describe('when listing components', () => {
    it('should point every component link at a gallery page under the prefix', () => {
      render(<ReviewNavigation selected="NavBar" foundation={null} />)

      const links = screen
        .getAllByRole('link', { hidden: true })
        .map((link) => link.getAttribute('href')!)

      expect(links.length).toBeGreaterThan(10)
      for (const href of links) expect(href.startsWith(`${PAGE}?`), href).toBe(true)
      const navBar = links.find(
        (href) => new URLSearchParams(href.split('?')[1]).get('component') === 'NavBar'
      )!
      expect(new URLSearchParams(navBar.split('?')[1]).get('platform')).toBe('all')
    })
  })

  describe('when listing foundations', () => {
    it('should point every foundation link under the prefix', () => {
      render(<ReviewNavigation selected="NavBar" foundation="tokens" />)

      const links = screen.getAllByRole('link').map((link) => link.getAttribute('href')!)

      expect(links.length).toBeGreaterThan(2)
      for (const href of links) {
        expect(href.startsWith(`${PAGE}?`), href).toBe(true)
        expect(new URLSearchParams(href.split('?')[1]).has('foundations')).toBe(true)
      }
    })
  })

  describe('when switching between components and foundations', () => {
    it('should navigate to a page under the prefix, keeping the variant', async () => {
      render(<ReviewNavigation selected="NavBar" foundation={null} />)

      await userEvent.setup().selectOptions(screen.getByLabelText('Browse'), 'foundations')

      expect(navigated).toHaveLength(1)
      expect(navigated[0]!.startsWith(`${PAGE}?`)).toBe(true)
      const params = new URLSearchParams(navigated[0]!.split('?')[1])
      expect(params.get('foundations')).toBe('tokens')
      expect(params.get('variant')).toBe('1')
    })
  })

  describe('when choosing a platform', () => {
    it('should navigate to a page under the prefix for that platform', async () => {
      render(<ReviewNavigation selected="NavBar" foundation={null} />)

      await userEvent.setup().selectOptions(screen.getByLabelText('Platform'), 'web')

      expect(navigated).toHaveLength(1)
      expect(navigated[0]!.startsWith(`${PAGE}?`)).toBe(true)
      expect(new URLSearchParams(navigated[0]!.split('?')[1]).get('platform')).toBe('web')
    })
  })
})
