import type { AnchorHTMLAttributes, ButtonHTMLAttributes, ComponentType, ReactNode } from 'react'
import styles from './Tabs.module.css'
import { secureRel } from '../../link.helpers'

/** The props a link tab hands its link component: the anchor's attributes, `href` included. */
export type TabsLinkProps = AnchorHTMLAttributes<HTMLAnchorElement> & { href: string }
/** The app's router link, so choosing a link tab is a client navigation instead of a page load. */
export type TabsLinkComponent = ComponentType<TabsLinkProps>

type TabsBaseProps = {
  label: ReactNode
  /** The current tab: `aria-selected`, and the only one in the tab order (TabsBar moves focus with the arrows). */
  selected?: boolean
  /** `compact` uses a shorter desktop underline; `pill` is the rounded filter tab. */
  variant?: 'underline' | 'compact' | 'pill'
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

type PanelTabsProps = TabsLinkTabProps | TabsButtonProps

/** One tab of a TabsBar: a link when it has `href` (works without script), otherwise a button. */
function PanelTabs(props: PanelTabsProps) {
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

export type TabsNavigationProps = Omit<
  AnchorHTMLAttributes<HTMLAnchorElement>,
  'children' | 'aria-current' | 'dangerouslySetInnerHTML' | 'role'
> & {
  mode: 'navigation'
  href: string
  label: string
  selected?: boolean
  variant?: never
  device?: never
}
/** Panel tabs remain the default; route navigation must declare its distinct intent. */
export type TabsProps = PanelTabsProps
export function Tabs(props: (TabsProps & { mode?: 'panel' }) | TabsNavigationProps) {
  if (props.mode === 'navigation') {
    const { mode, label, selected = false, className, target, rel, ...anchor } = props
    return (
      <a
        {...anchor}
        target={target}
        rel={secureRel(target, rel)}
        dangerouslySetInnerHTML={undefined}
        role={undefined}
        data-mode={mode}
        className={[styles.routeRoot, className].filter(Boolean).join(' ')}
        aria-current={selected ? 'page' : undefined}
        data-label={label}
      >
        <span className={styles.routeLabel}>{label}</span>
      </a>
    )
  }
  const { mode, ...panel } = props
  return <PanelTabs {...panel} data-mode={mode} />
}
