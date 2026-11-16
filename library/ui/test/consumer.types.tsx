import { Tabs, type TabsProps } from '../components/Tabs/Tabs'
import { TabsBar, type TabsBarProps } from '../components/TabsBar/TabsBar'
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
const missingHref: TabsProps = { mode: 'navigation', label: 'A' }
// @ts-expect-error Panel bars cannot silently accept route data.
const implicitRoutes: TabsBarProps = { items, activeId: 'a' }
// @ts-expect-error A route bar cannot mix opaque panel children.
const mixed: TabsBarProps = {
  mode: 'navigation',
  items,
  activeId: 'a',
  children: 'tabs'
}
// @ts-expect-error Shop parent requires caller-owned search and actions.
const missingControls: TabsBarProps = {
  mode: 'navigation',
  items,
  activeId: 'a',
  page: 'shop'
}
const mixedTab: TabsProps = {
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
