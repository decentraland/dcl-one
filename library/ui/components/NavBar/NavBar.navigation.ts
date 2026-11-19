import type { NavBarLink, NavBarSection } from './NavBar'

export type DecentralandNavigationSectionId = 'discover' | 'shop' | 'create' | 'learn'

/** The area a page reports. Several areas belong to one section; `social` belongs to none. */
export type DecentralandNavigationArea =
  'explore' | 'whatson' | 'events' | 'social' | 'shop' | 'create' | 'learn'

export type DecentralandNavigationItem = Omit<NavBarLink, 'active'>

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
        ...item,
        active: active && underPath(pathname, item.href)
      }))
    }
  })
}

const publicSectionItems: Record<
  Exclude<DecentralandNavigationSectionId, 'discover'>,
  readonly DecentralandNavigationItem[]
> = {
  shop: [
    { label: 'Shop All', href: '/shop/overview' },
    { label: 'Wearables', href: '/shop/items?category=wearable' },
    { label: 'Emotes', href: '/shop/items?category=emote' },
    { label: 'NAMEs', href: '/shop/names' },
    { label: 'LAND', href: '/shop/lands' },
    { label: 'Merch', href: 'https://store.decentraland.org/', target: '_blank' }
  ],
  create: [
    { label: 'Create in Decentraland', href: '/create' },
    { label: 'Publish Wearables & Emotes', href: '/create/wearables' },
    { label: 'Publish LAND', href: '/create/land' }
  ],
  learn: [
    { label: 'Get Started', href: '/docs/player/' },
    { label: 'Start Creating', href: '/docs/creator/' },
    { label: 'See What’s New', href: '/blog' }
  ]
}

function matchingPublicItem(
  items: readonly DecentralandNavigationItem[],
  pathname: string,
  search: URLSearchParams
) {
  let match: DecentralandNavigationItem | undefined
  for (const item of items) {
    if (!item.href.startsWith('/')) continue
    const [path = item.href, query] = item.href.split('?')
    const itemPath = path.replace(/\/$/, '')
    const matches = query
      ? pathname === itemPath &&
        [...new URLSearchParams(query)].every(([key, value]) => search.get(key) === value)
      : underPath(pathname, itemPath)
    if (matches && (!match || item.href.length > match.href.length)) match = item
  }
  return match
}

/** Public Sites navigation. The host supplies resolved Marketplace eligibility. */
export function decentralandPublicNavSections(
  area: DecentralandNavigationArea | '' | undefined,
  pathname: string,
  { search, marketplaceEligible }: { search?: string; marketplaceEligible?: boolean } = {}
): { desktop: NavBarSection[]; mobile: NavBarSection[] } {
  const current = decentralandNavigationSection(area)
  const path = pathname.replace(/\/$/, '')
  const params = new URLSearchParams(search)
  const desktop: NavBarSection[] = []
  const mobile: NavBarSection[] = []
  for (const { id, label, href, items } of decentralandNavigation) {
    const active = id === current
    const extraItems = id === 'discover' ? items ?? [] : publicSectionItems[id]
    const visible =
      id === 'shop' && !marketplaceEligible
        ? extraItems.filter((item) => item.href !== '/shop/lands')
        : extraItems
    const match = active ? matchingPublicItem(visible, path, params) : undefined
    const section = {
      label,
      active,
      items: visible.map((item) => ({ ...item, active: item === match }))
    }
    desktop.push(id === 'learn' ? { label, href, active } : section)
    mobile.push(section)
  }
  return { desktop, mobile }
}
