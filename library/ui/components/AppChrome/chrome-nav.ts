import { createContext, useContext } from 'react'
import type { ComponentPropsWithRef, ComponentType, MouseEvent, ReactNode } from 'react'
import type { DecentralandNavigationArea } from '../NavBar/NavBar.navigation'

export type ChromeNavigate = (href: string) => boolean | void | Promise<void>

/** The props a host's top bar takes when it fills the chrome's `TopBar` slot. */
export type ChromeTopBarProps = {
  variant?: 'default' | 'sites'
  active?: DecentralandNavigationArea | ''
  signedIn?: boolean
  account?: string
  transparent?: boolean
  onSignIn?: () => void
  onSignOut?: () => void
  signInHref?: string
  fetchNotifications?: (signal?: AbortSignal) => Promise<unknown>
  onNavigate?: ChromeNavigate
  mobileNavigation?: (closeMenu: () => void) => ReactNode
  accountControl?: ReactNode
  /** Balance chips for a signed-in account, before the bell (the Shop's credits). */
  balances?: ReactNode
}

type ChromeNav = {
  TopBar?: ComponentType<ChromeTopBarProps>
  headerLayout?: 'native'
  navigate?: ChromeNavigate
  Link?: ComponentType<ComponentPropsWithRef<'a'>>
}

export const ChromeNavContext = createContext<ChromeNav>({ navigate: undefined })

export function useChromeNav(): ChromeNav {
  return useContext(ChromeNavContext)
}

export function isSameDocumentHref(href: string): boolean {
  return href.startsWith('/') && !href.startsWith('//')
}

export function chromeLinkClick(
  e: MouseEvent<HTMLAnchorElement>,
  href: string,
  navigate: ChromeNavigate | undefined
): void {
  if (!navigate || e.defaultPrevented) return
  if (e.currentTarget.hasAttribute('download')) return
  if (e.button !== 0 || e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return
  const target = e.currentTarget.getAttribute('target')
  if (target && target !== '_self') return
  if (!isSameDocumentHref(href)) return
  if (navigate(href) !== false) e.preventDefault()
}

export function chromeLinkProps(
  href: string,
  navigate: ChromeNavigate | undefined
): { href: string; onClick: (e: MouseEvent<HTMLAnchorElement>) => void; 'data-discover'?: 'true' } {
  return {
    href,
    onClick: (e) => chromeLinkClick(e, href, navigate),
    'data-discover': navigate && isSameDocumentHref(href) ? 'true' : undefined
  }
}
