import type { NavBarSection } from './NavBar'

export type DecentralandNavigationSectionId = 'discover' | 'shop' | 'create' | 'learn'

/** The area a page reports. Several areas belong to one section; `social` belongs to none. */
export type DecentralandNavigationArea =
  'explore' | 'whatson' | 'events' | 'social' | 'shop' | 'create' | 'learn'

export type DecentralandNavigationItem = {
  label: string
  href: string
}

export type DecentralandNavigationEntry = {
  id: DecentralandNavigationSectionId
  label: string
  /** Where the section leads when a host renders it as one link. */
  href: string
  items?: readonly DecentralandNavigationItem[]
}

/** The default Decentraland site navigation. Every destination is a Sites route. */
export const decentralandNavigation: readonly DecentralandNavigationEntry[] = [
  {
    id: 'discover',
    label: 'Discover',
    href: '/events',
    items: [
      { label: 'Events', href: '/events' },
      { label: 'Places', href: '/places' }
    ]
  },
  { id: 'shop', label: 'Shop', href: '/shop' },
  { id: 'create', label: 'Create', href: '/create' },
  { id: 'learn', label: 'Learn', href: '/blog' }
]

const SECTION_OF_AREA: Record<DecentralandNavigationArea, DecentralandNavigationSectionId | undefined> = {
  explore: 'discover',
  whatson: 'discover',
  events: 'discover',
  social: undefined,
  shop: 'shop',
  create: 'create',
  learn: 'learn'
}

export function decentralandNavigationSection(
  area: DecentralandNavigationArea | '' | undefined
): DecentralandNavigationSectionId | undefined {
  return area ? SECTION_OF_AREA[area] : undefined
}

const underPath = (pathname: string, path: string) =>
  pathname === path || pathname.startsWith(`${path}/`)

/** NavBar sections for the default navigation: the area selects a section, the pathname one of its items. */
export function decentralandNavSections(
  area: DecentralandNavigationArea | '' | undefined,
  pathname: string
): NavBarSection[] {
  const current = decentralandNavigationSection(area)
  return decentralandNavigation.map(({ id, label, href, items }) => {
    const active = id === current
    if (!items) return { label, href, active }
    return {
      label,
      active,
      items: items.map((item) => ({
        label: item.label,
        href: item.href,
        active: active && underPath(pathname, item.href)
      }))
    }
  })
}
