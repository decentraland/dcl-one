import type { NavBarProps, NavBarSection } from '@dcl/one-ui'

const sections: NavBarSection[] = [
  { label: 'Discover', items: [{ label: 'Jump in', href: '/play/' }] },
  { label: 'Shop', href: '/shop', active: true }
]
const signedOut: NavBarProps = { sections, signIn: { onSelect: () => {} } }
const signedOutLink: NavBarProps = { sections, signIn: { href: '/sign-in' } }
const signedIn: NavBarProps = {
  sections,
  account: { address: '0x0000000000000000000000000000000000000000' },
  balances: { credits: 1 },
  notifications: { count: 1, onSelect: () => {} },
  accountActions: [
    { label: 'Profile', href: '/profile' },
    { label: 'Sign out', onSelect: () => {} }
  ]
}
// @ts-expect-error Sections are required: the library holds no navigation data of its own.
const missingSections: NavBarProps = { signIn: { href: '/sign-in' } }
// @ts-expect-error Signed out, the bar needs a Sign in action.
const noSignIn: NavBarProps = { sections }
// @ts-expect-error A Sign in action is a link or a callback, never both.
const bothSignIn: NavBarProps = { sections, signIn: { href: '/sign-in', onSelect: () => {} } }
const countOnly: NavBarProps = {
  sections,
  account: { address: '0x0' },
  // @ts-expect-error A notification count needs the action that opens the notifications.
  notifications: { count: 3 }
}
// @ts-expect-error A section is a link or a dropdown, never neither.
const emptySection: NavBarSection = { label: 'Empty' }
// @ts-expect-error A section is a link or a dropdown, never both.
const bothSection: NavBarSection = { label: 'Both', href: '/both', items: [] }
// @ts-expect-error An account is identified by its address.
const anonymous: NavBarProps = { sections, signIn: { href: '/sign-in' }, account: { name: 'Ada' } }
// @ts-expect-error The viewport is chosen by CSS media queries, not a prop.
const viewport: NavBarProps = { sections, signIn: { href: '/sign-in' }, viewport: 'mobile' }
const inertAction: NavBarProps = {
  sections,
  signIn: { href: '/sign-in' },
  // @ts-expect-error An account action has an href or an onSelect.
  accountActions: [{ label: 'Nothing' }]
}

void [
  signedOut,
  signedOutLink,
  signedIn,
  missingSections,
  noSignIn,
  bothSignIn,
  countOnly,
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
