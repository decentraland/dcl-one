import type { ComponentProps } from 'react'
import { Tabs, type TabsNavigationProps } from '../components/Tabs/Tabs'
import { TabsBar, type TabsBarNavigationProps } from '../components/TabsBar/TabsBar'
const items = [{ id: 'a', label: 'A', href: '/a' }]
export const mainPanel = (
  <TabsBar label="Panels">
    <Tabs label="A" selected />
    <Tabs label="B" href="#b" />
  </TabsBar>
)
export const route = <Tabs mode="navigation" label="Route" href="/route" selected />
export const routeBar = (
  <TabsBar
    mode="navigation"
    items={items}
    activeId="a"
    page="shop"
    search={<input />}
    actions={<button />}
  />
)
// @ts-expect-error Navigation tabs require native destinations.
const missingHref: TabsNavigationProps = { mode: 'navigation', label: 'A' }
const implicitRouteCandidate = { label: 'Tabs', children: null, items, activeId: 'a' }
// @ts-expect-error Complete panel props still cannot silently accept route data.
const implicitRoutes: ComponentProps<typeof TabsBar> = implicitRouteCandidate
const mixed: TabsBarNavigationProps = {
  mode: 'navigation',
  items,
  activeId: 'a',
  // @ts-expect-error A route bar cannot mix opaque panel children.
  children: 'tabs'
}
// @ts-expect-error Shop parent requires caller-owned search and actions.
const missingControls: TabsBarNavigationProps = {
  mode: 'navigation',
  items,
  activeId: 'a',
  page: 'shop'
}
const mixedTab: TabsNavigationProps = {
  mode: 'navigation',
  href: '/a',
  label: 'A',
  // @ts-expect-error Navigation tabs do not accept panel-presentation variants.
  variant: 'pill'
}
export const expectedFailures = {
  missingHref,
  implicitRoutes,
  mixed,
  missingControls,
  mixedTab
}
