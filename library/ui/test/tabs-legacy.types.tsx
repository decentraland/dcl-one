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

import {
  TabsBar,
  type TabsBarProps,
  type TabsBarNavigationProps
} from '../components/TabsBar/TabsBar'
export function ExistingPillBar(props: TabsBarProps) {
  return <TabsBar {...props} variant="pill" />
}
export interface ExistingPanelBar extends TabsBarProps {
  id?: string
}
export type ExistingBarVariant = TabsBarProps['variant']
export const routeBarProps: TabsBarNavigationProps = {
  mode: 'navigation',
  items: [{ id: 'a', label: 'A', href: '/a' }],
  activeId: 'a'
}
export const routeBar = <TabsBar {...routeBarProps} />
