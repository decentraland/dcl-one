import { createElement, createRef } from 'react'
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
// App-owned controls retain their own native semantics through the public props type.
const composition: NavBarProps = {
  sections,
  signIn: { href: '/sign-in' },
  balanceAction: createElement('a', { href: '/credits' }, 'Get credits'),
  accountControl: createElement('button', { type: 'button' }, 'Connect wallet'),
  mobileSecondaryNavigation: ({ closeDrawer }) =>
    createElement('a', { href: '/profile', onClick: closeDrawer }, 'Profile')
}
const callbackSlot: NavBarProps = {
  sections,
  signIn: { href: '/sign-in' },
  // @ts-expect-error Controls are React nodes, not render callbacks.
  balanceAction: () => 'Credits'
}
// @ts-expect-error The generic balance action does not introduce a credits-only alias.
const creditsAlias: NavBarProps = { sections, signIn: { href: '/sign-in' }, creditsAction: null }
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
  composition,
  callbackSlot,
  creditsAlias,
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

const button: import('@dcl/one-ui').ButtonProps = {
  variant: 'tertiary',
  size: 'md',
  children: 'Jump',
  leadingIcon: null,
  'aria-pressed': true,
  'aria-disabled': true,
  onClick: (event) => {
    event.preventDefault()
    event.currentTarget.disabled = true
  }
}
const link: import('@dcl/one-ui').ButtonProps = {
  variant: 'inverse',
  href: '/jump',
  children: 'Jump',
  target: '_blank',
  disabled: true,
  onClick: (event) => {
    event.preventDefault()
    event.currentTarget.href = '/next'
  }
}
// @ts-expect-error A navigation link cannot have a native button submit type.
const invalidLink: import('@dcl/one-ui').ButtonProps = { href: '/jump', type: 'submit' }
// @ts-expect-error Only supported button variants are public.
const invalidVariant: import('@dcl/one-ui').ButtonProps = { variant: 'unknown' }
void [button, link, invalidLink, invalidVariant]

const nativeButtonRef: import('@dcl/one-ui').ButtonProps = {
  children: 'Save',
  ref: createRef<HTMLButtonElement>()
}
const nativeLinkRef: import('@dcl/one-ui').ButtonProps = {
  href: '/save',
  children: 'Save',
  ref: createRef<HTMLAnchorElement>()
}
// @ts-expect-error Icon-only variants require an explicit accessible name.
const unnamedIcon: import('@dcl/one-ui').ButtonProps = { variant: 'iconOnly', children: null }
// @ts-expect-error The iconOnly flag also requires an explicit accessible name.
const unnamedFlag: import('@dcl/one-ui').ButtonProps = { iconOnly: true, children: null }
// @ts-expect-error A public control needs explicit content or an accessible name.
const unnamedControl: import('@dcl/one-ui').ButtonProps = { variant: 'primary' }
// @ts-expect-error A native button cannot receive an anchor ref.
const invalidRef: import('@dcl/one-ui').ButtonProps = {
  children: 'Save',
  ref: createRef<HTMLAnchorElement>()
}
void [nativeButtonRef, nativeLinkRef, unnamedIcon, unnamedFlag, unnamedControl, invalidRef]

// @ts-expect-error An absent content value needs an explicit accessible name.
const unnamedUndefinedButton: import('@dcl/one-ui').ButtonProps = { children: undefined }
// @ts-expect-error Decorative null content needs an explicit accessible name.
const unnamedNullButton: import('@dcl/one-ui').ButtonProps = { children: null }
void [unnamedUndefinedButton, unnamedNullButton]

const rendererAppearanceButton: import('@dcl/one-ui').ButtonProps = {
  children: 'Join',
  // @ts-expect-error Appearance is controlled through the public variant, not renderer internals.
  appearance: 'inverse'
}
void rendererAppearanceButton
