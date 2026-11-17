import { describe, expect, it } from 'vitest'
import {
  decentralandNavigation,
  decentralandNavigationSection,
  decentralandNavSections
} from './NavBar.navigation'

describe('when building the default Decentraland navigation', () => {
  it('should list Discover, Shop, Create and Learn with their Sites destinations', () => {
    expect(decentralandNavSections(undefined, '/')).toEqual([
      {
        label: 'Discover',
        active: false,
        items: [
          { label: 'Events', href: '/events', active: false },
          { label: 'Places', href: '/places', active: false }
        ]
      },
      { label: 'Shop', href: '/shop', active: false },
      { label: 'Create', href: '/create', active: false },
      { label: 'Learn', href: '/blog', active: false }
    ])
  })

  it('should give every section a destination for hosts that render it as one link', () => {
    expect(decentralandNavigation.map(({ href }) => href)).toEqual([
      '/events',
      '/shop',
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

  it.each([['social'], [''], [undefined]] as const)(
    'should select no section for %s',
    (area) => {
      expect(decentralandNavigationSection(area)).toBeUndefined()
      expect(decentralandNavSections(area, '/social/').some(({ active }) => active)).toBe(false)
    }
  )

  it.each([
    ['/events', [true, false]],
    ['/events/new-event', [true, false]],
    ['/eventsx', [false, false]],
    ['/places', [false, true]],
    ['/places/world/foo.dcl.eth', [false, true]],
    ['/placesx', [false, false]],
    ['/', [false, false]]
  ] as const)('should select the Discover item matching %s', (pathname, expected) => {
    const [discover] = decentralandNavSections('explore', pathname)
    expect(discover?.active).toBe(true)
    expect(discover?.items?.map(({ active }) => active)).toEqual(expected)
  })

  it('should not select a Discover item while another section is current', () => {
    const [discover, shop] = decentralandNavSections('shop', '/places')
    expect(discover?.items?.every(({ active }) => !active)).toBe(true)
    expect(shop?.active).toBe(true)
  })
})
