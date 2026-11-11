import type { NavBarProps, NavBarSection } from '@dcl/one-ui'

const sections: NavBarSection[] = [
  { label: 'Discover', items: [{ label: 'Jump in', href: '/play/' }] },
  { label: 'Shop', href: '/shop', active: true }
]
const signedOut: NavBarProps = { sections, onSignIn: () => {} }
const signedIn: NavBarProps = {
  sections,
  account: { address: '0x0000000000000000000000000000000000000000' },
  balances: { credits: 1 },
  notificationCount: 1,
  onNotificationsClick: () => {},
  accountActions: [
    { label: 'Profile', href: '/profile' },
    { label: 'Sign out', onSelect: () => {} }
  ]
}
// @ts-expect-error Sections are required: the library holds no navigation data of its own.
const missingSections: NavBarProps = {}
// @ts-expect-error A section is a link or a dropdown, never neither.
const emptySection: NavBarSection = { label: 'Empty' }
// @ts-expect-error A section is a link or a dropdown, never both.
const bothSection: NavBarSection = { label: 'Both', href: '/both', items: [] }
// @ts-expect-error An account is identified by its address.
const anonymous: NavBarProps = { sections, account: { name: 'Ada' } }
// @ts-expect-error The viewport is chosen by CSS media queries, not a prop.
const viewport: NavBarProps = { sections, viewport: 'mobile' }
// @ts-expect-error An account action has an href or an onSelect.
const inertAction: NavBarProps = { sections, accountActions: [{ label: 'Nothing' }] }

void [
  signedOut,
  signedIn,
  missingSections,
  emptySection,
  bothSection,
  anonymous,
  viewport,
  inertAction
]

// Foundation exports remain available through the package entry.
export { theme } from '@dcl/one-ui'
export { NavBar } from '@dcl/one-ui'
export type { NavBarProps } from '@dcl/one-ui'
// @ts-expect-error The package export map does not expose component deep imports.
export { NavBar as DeepNavBar } from '@dcl/one-ui/components/NavBar/NavBar'

// Foundations have their own public entry without components.
export { ShopCart, theme as foundationTheme } from '@dcl/one-ui/foundations'
// @ts-expect-error Components are not exported by the foundations entry.
export { NavBar as FoundationNavBar } from '@dcl/one-ui/foundations'
