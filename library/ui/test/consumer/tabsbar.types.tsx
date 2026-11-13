import { TabsBar, type TabsBarItem, type TabsBarProps, type TabsBarLinkProps } from '@dcl/one-ui'
const items: readonly TabsBarItem[] = [{ id: 'news', label: 'News', href: '/news' }] as const
const complete: TabsBarProps = {
  items,
  activeId: 'news',
  onSelect: (id) => {
    id.toUpperCase()
  },
  LinkComponent: ({ prefetch, ...props }: TabsBarLinkProps) => (
    <a {...props} data-prefetch={prefetch} />
  )
}
const missingHref: TabsBarProps = {
  // @ts-expect-error Routes require hrefs instead of activation-only button content.
  items: [{ id: 'news', label: 'News' }],
  activeId: 'news'
}
// @ts-expect-error The current route is explicitly controlled.
const missingActive: TabsBarProps = { items }
const invalidHref: TabsBarProps = {
  // @ts-expect-error Route destinations must be strings.
  items: [{ id: 'news', label: 'News', href: 1 }],
  activeId: 'news'
}
const invalidSelection: TabsBarProps = {
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
const shop: TabsBarProps = {
  page: 'shop',
  items,
  activeId: 'news',
  search: <input aria-label="Search" />,
  actions: <button>Cart</button>
}
// @ts-expect-error Shop requires explicit keys; their contents may be empty or loading.
const missingShopControls: TabsBarProps = { page: 'shop', items, activeId: 'news' }
// @ts-expect-error Create does not have a captured search/actions row.
const unexpectedCreateControl: TabsBarProps = {
  page: 'create',
  items,
  activeId: 'news',
  search: <input />
}
// @ts-expect-error Source page choices are explicit.
const inventedPage: TabsBarProps = { page: 'profile', items, activeId: 'news' }
export const pageFixtures = { shop, missingShopControls, unexpectedCreateControl, inventedPage }

export const emptyShop: TabsBarProps = {
  page: 'shop',
  items,
  activeId: 'news',
  search: undefined,
  actions: null
}
