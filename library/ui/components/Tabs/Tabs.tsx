import type { AnchorHTMLAttributes, ButtonHTMLAttributes, ComponentType, ReactNode } from 'react'
import styles from './Tabs.module.css'

/** The props a link tab hands its link component: the anchor's attributes, `href` included. */
export type TabsLinkProps = AnchorHTMLAttributes<HTMLAnchorElement> & { href: string }
/** The app's router link, so choosing a link tab is a client navigation instead of a page load. */
export type TabsLinkComponent = ComponentType<TabsLinkProps>

type TabsBaseProps = {
  label: ReactNode
  /** The current tab: `aria-selected`, and the only one in the tab order (TabsBar moves focus with the arrows). */
  selected?: boolean
  /** `underline` is the Figma tab; `pill` is the rounded tab decentraland.org shows on the events page. */
  variant?: 'underline' | 'pill'
  /** Fixes the Figma device size; without it the tab follows the viewport. */
  device?: 'mobile' | 'desktop'
}

type TabsLinkTabProps = TabsBaseProps & {
  href: string
  /** Renders the link tab; a plain anchor without it. */
  linkComponent?: TabsLinkComponent
} & Omit<AnchorHTMLAttributes<HTMLAnchorElement>, 'children' | 'href' | 'role'>

type TabsButtonProps = TabsBaseProps & { href?: undefined; linkComponent?: undefined } & Omit<
    ButtonHTMLAttributes<HTMLButtonElement>,
    'children' | 'type' | 'role'
  >

export type TabsProps = TabsLinkTabProps | TabsButtonProps

/** One tab of a TabsBar: a link when it has `href` (works without script), otherwise a button. */
export function Tabs(props: TabsProps) {
  const {
    label,
    selected = false,
    variant = 'underline',
    device,
    className,
    tabIndex,
    linkComponent: LinkComponent,
    ...rest
  } = props
  const classes = [styles.root, styles[variant], device && styles[device], className]
    .filter(Boolean)
    .join(' ')
  const shared = {
    role: 'tab',
    'aria-selected': selected,
    tabIndex: tabIndex ?? (selected ? 0 : -1),
    className: classes
  } as const
  if (rest.href !== undefined) {
    const anchor = { ...(rest as TabsLinkProps), ...shared }
    return LinkComponent ? <LinkComponent {...anchor}>{label}</LinkComponent> : <a {...anchor}>{label}</a>
  }
  return (
    <button type="button" {...(rest as ButtonHTMLAttributes<HTMLButtonElement>)} {...shared}>
      {label}
    </button>
  )
}
