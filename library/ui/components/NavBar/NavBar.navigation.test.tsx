import { describe, expect, it } from 'vitest'
import {
  decentralandNavigation,
  decentralandNavigationSection,
  decentralandPublicNavSections
} from './NavBar.navigation'

describe('when building the default Decentraland navigation', () => {
  it('should list Discover, Shop, Create and Learn with their Sites destinations', () => {
    expect(decentralandNavigation).toEqual([
      {
        label: 'Discover',
        id: 'discover',
        href: '/events',
        items: [
          { label: 'Events', href: '/events' },
          { label: 'Places', href: '/places' }
        ]
      },
      { id: 'shop', label: 'Shop', href: '/shop/overview' },
      { id: 'create', label: 'Create', href: '/create' },
      { id: 'learn', label: 'Learn', href: '/blog' }
    ])
  })

  it('should give every section a destination for hosts that render it as one link', () => {
    expect(decentralandNavigation.map(({ href }) => href)).toEqual([
      '/events',
      '/shop/overview',
      '/create',
      '/blog'
    ])
  })
})

describe('when a page reports its area', () => {
  it.each([
    ['explore', 'discover'],
    ['whatson', 'discover'],
    ['events', 'discover'],
    ['shop', 'shop'],
    ['create', 'create'],
    ['learn', 'learn']
  ] as const)('should map %s to the %s section', (area, section) => {
    expect(decentralandNavigationSection(area)).toBe(section)
  })

  it.each([['social'], [''], [undefined]] as const)('should select no section for %s', (area) => {
    expect(decentralandNavigationSection(area)).toBeUndefined()
    expect(
      decentralandPublicNavSections(area, '/social/').desktop.some(({ active }) => active)
    ).toBe(false)
  })

  it.each([
    ['/events', [true, false]],
    ['/events/new-event', [true, false]],
    ['/eventsx', [false, false]],
    ['/places', [false, true]],
    ['/places/world/foo.dcl.eth', [false, true]],
    ['/placesx', [false, false]],
    ['/', [false, false]]
  ] as const)('should select the Discover item matching %s', (pathname, expected) => {
    const [discover] = decentralandPublicNavSections('explore', pathname).desktop
    expect(discover?.active).toBe(true)
    expect(discover?.items?.map(({ active }) => active)).toEqual(expected)
  })

  it('should not select a Discover item while another section is current', () => {
    const [discover, shop] = decentralandPublicNavSections('shop', '/places').desktop
    expect(discover?.items?.every(({ active }) => !active)).toBe(true)
    expect(shop?.active).toBe(true)
  })
})

describe('when building the public Sites navigation', () => {
  it('should render desktop Learn as a link and mobile Learn as a disclosure', () => {
    const { desktop, mobile } = decentralandPublicNavSections('learn', '/docs/creator/')
    expect(desktop[3]).toEqual({ label: 'Learn', href: '/blog', active: true })
    expect(mobile[3]?.items).toEqual([
      { label: 'Get Started', href: '/docs/player/', active: false },
      { label: 'Start Creating', href: '/docs/creator/', active: true },
      { label: 'See What’s New', href: '/blog', active: false }
    ])
  })

  it('should offer the same Discover destinations on both viewports', () => {
    const { desktop, mobile } = decentralandPublicNavSections('events', '/events/new-event')
    expect(desktop[0]?.items).toEqual([
      { label: 'Events', href: '/events', active: true },
      { label: 'Places', href: '/places', active: false }
    ])
    expect(mobile[0]).toEqual(desktop[0])
  })

  describe('and Marketplace eligibility is not granted', () => {
    it.each([undefined, false])('should hide LAND for eligibility %s', (marketplaceEligible) => {
      const { desktop, mobile } = decentralandPublicNavSections('shop', '/shop/overview', {
        marketplaceEligible
      })
      expect(desktop[1]?.items?.map(({ href }) => href)).toEqual([
        '/shop/overview',
        '/shop/items?category=wearable',
        '/shop/items?category=emote',
        '/shop/names',
        'https://store.decentraland.org/'
      ])
      expect(mobile[1]).toEqual(desktop[1])
    })
  })

  it.each([
    ['/shop/items', '?category=wearable&sort=recent', 'Wearables'],
    ['/shop/items', '?category=emote', 'Emotes'],
    ['/shop/items', '?category=unknown', undefined],
    ['/shop/items/123', '?category=emote', undefined],
    ['/shop/lands', '', 'LAND'],
    ['/shop/overview', '', 'Shop All'],
    ['/shop/names', '?section=resale', 'NAMEs']
  ])('should select only the matching Shop item for %s%s', (path, search, label) => {
    const { desktop } = decentralandPublicNavSections('shop', path, {
      search,
      marketplaceEligible: true
    })
    expect(desktop[1]?.items?.filter(({ active }) => active).map((item) => item.label)).toEqual(
      label ? [label] : []
    )
  })

  it('should select the publishing destination under Create', () => {
    const { desktop } = decentralandPublicNavSections('create', '/create/collections/editor')
    expect(desktop[2]?.items).toEqual([
      { label: 'Create in Decentraland', href: '/create', active: false },
      { label: 'Publish Wearables & Emotes', href: '/create/collections', active: true },
      { label: 'Publish LAND', href: '/download/creator-hub', active: false }
    ])
  })
})
