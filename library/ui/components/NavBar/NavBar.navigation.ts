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

type PublicNavigationItem = DecentralandNavigationItem & { marketplaceOnly?: true }

const publicNavigation: readonly (Omit<DecentralandNavigationEntry, 'items'> & {
  items: readonly PublicNavigationItem[]
})[] = [
  {
    id: 'discover',
    label: 'Discover',
    href: '/events',
    items: [
      { label: 'Events', href: '/events' },
      { label: 'Places', href: '/places' }
    ]
  },
  {
    id: 'shop',
    label: 'Shop',
    href: '/shop/overview',
    items: [
      { label: 'Shop All', href: '/shop/overview' },
      { label: 'Wearables', href: '/shop/items?category=wearable' },
      { label: 'Emotes', href: '/shop/items?category=emote' },
      { label: 'NAMEs', href: '/shop/names' },
      { label: 'LAND', href: '/shop/lands', marketplaceOnly: true },
      { label: 'Merch', href: 'https://store.decentraland.org/', target: '_blank' }
    ]
  },
  {
    id: 'create',
    label: 'Create',
    href: '/create',
    items: [
      { label: 'Create in Decentraland', href: '/create' },
      { label: 'Publish Wearables & Emotes', href: '/create/wearables' },
      { label: 'Publish LAND', href: '/download/creator-hub' }
    ]
  },
  {
    id: 'learn',
    label: 'Learn',
    href: '/blog',
    items: [
      { label: 'Get Started', href: '/docs/player/' },
      { label: 'Start Creating', href: '/docs/creator/' },
      { label: 'See What’s New', href: '/blog' }
    ]
  }
]

/** Navigation for hosts that use a single link for all sections except Discover. */
export const decentralandNavigation: readonly DecentralandNavigationEntry[] = publicNavigation.map(
  ({ id, label, href, items }) =>
    id === 'discover' ? { id, label, href, items } : { id, label, href }
)

const SECTION_OF_AREA: Record<
  DecentralandNavigationArea,
  DecentralandNavigationSectionId | undefined
> = {
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
  for (const { id, label, href, items } of publicNavigation) {
    const active = id === current
    const visible = items.filter((item) => !item.marketplaceOnly || marketplaceEligible)
    const match = active ? matchingPublicItem(visible, path, params) : undefined
    const section = {
      label,
      active,
      items: visible.map((item) => ({
        label: item.label,
        href: item.href,
        ...(item.target ? { target: item.target } : {}),
        active: item === match
      }))
    }
    desktop.push(id === 'learn' ? { label, href, active } : section)
    mobile.push(section)
  }
  return { desktop, mobile }
}
