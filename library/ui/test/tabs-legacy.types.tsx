import { Tabs, type TabsProps, type TabsNavigationProps } from '../components/Tabs/Tabs'
export function ExistingPillTab(props: TabsProps) {
  return <Tabs {...props} variant="pill" />
}
export const routeProps: TabsNavigationProps = {
  mode: 'navigation',
  href: '/route',
  label: 'Route'
}
export const routeTab = <Tabs {...routeProps} />
// @ts-expect-error Explicit route tabs require a destination.
export const missingRouteHref: TabsNavigationProps = { mode: 'navigation', label: 'Route' }
