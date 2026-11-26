import {
  TabsBar,
  type TabsBarItem,
  type TabsBarNavigationProps,
  type TabsBarLinkProps
} from '@dcl/one-ui'
const items: readonly TabsBarItem[] = [{ id: 'news', label: 'News', href: '/news' }] as const
const complete: TabsBarNavigationProps = {
  mode: 'navigation',
  items,
  activeId: 'news',
  onSelect: (id) => {
    id.toUpperCase()
  },
  LinkComponent: ({ prefetch, ...props }: TabsBarLinkProps) => (
    <a {...props} data-prefetch={prefetch} />
  )
}
const missingHref: TabsBarNavigationProps = {
  mode: 'navigation',
  // @ts-expect-error Routes require hrefs instead of activation-only button content.
  items: [{ id: 'news', label: 'News' }],
  activeId: 'news'
}
// @ts-expect-error The current route is explicitly controlled.
const missingActive: TabsBarNavigationProps = { mode: 'navigation', items }
const invalidHref: TabsBarNavigationProps = {
  mode: 'navigation',
  // @ts-expect-error Route destinations must be strings.
  items: [{ id: 'news', label: 'News', href: 1 }],
  activeId: 'news'
}
const invalidSelection: TabsBarNavigationProps = {
  mode: 'navigation',
  items,
  activeId: 'news',
  // @ts-expect-error Selection callbacks receive the route ID.
  onSelect: (item: TabsBarItem) => {
    void item
  }
}
export const fixtures = {
  complete,
  missingHref,
  missingActive,
  invalidHref,
  invalidSelection
}
export const rendered = <TabsBar {...complete} />
export const panelWithCurrent = (
  // @ts-expect-error Only route links accept a current marker.
  <TabsBar label="Sections" current="true">
    {null}
  </TabsBar>
)
const shop: TabsBarNavigationProps = {
  mode: 'navigation',
  page: 'shop',
  items,
  activeId: 'news',
  search: <input aria-label="Search" />,
  actions: <button>Cart</button>
}
// @ts-expect-error Shop requires explicit keys; their contents may be empty or loading.
const missingShopControls: TabsBarNavigationProps = {
  mode: 'navigation',
  page: 'shop',
  items,
  activeId: 'news'
}
// @ts-expect-error Create does not have a captured search/actions row.
const unexpectedCreateControl: TabsBarNavigationProps = {
  mode: 'navigation',
  page: 'create',
  items,
  activeId: 'news',
  search: <input />
}
const inventedPage: TabsBarNavigationProps = {
  mode: 'navigation',
  // @ts-expect-error Only supported parent pages are accepted.
  page: 'profile',
  items,
  activeId: 'news'
}
export const pageFixtures = { shop, missingShopControls, unexpectedCreateControl, inventedPage }

export const emptyShop: TabsBarNavigationProps = {
  mode: 'navigation',
  page: 'shop',
  items,
  activeId: 'news',
  search: undefined,
  actions: null
}
