import type { AnchorHTMLAttributes, ButtonHTMLAttributes, ReactNode } from 'react'
import styles from './Tabs.module.css'
import { secureRel } from '../../link.helpers'

type TabsBaseProps = {
  label: ReactNode
  /** The current tab: `aria-selected`, and the only one in the tab order (TabsBar moves focus with the arrows). */
  selected?: boolean
  /** `underline` is the Figma tab; `pill` is the rounded tab decentraland.org shows on the events page. */
  variant?: 'underline' | 'pill'
  /** Fixes the Figma device size; without it the tab follows the viewport. */
  device?: 'mobile' | 'desktop'
}

type TabsLinkProps = TabsBaseProps & { href: string } & Omit<
    AnchorHTMLAttributes<HTMLAnchorElement>,
    'children' | 'href' | 'role'
  >

type TabsButtonProps = TabsBaseProps & { href?: undefined } & Omit<
    ButtonHTMLAttributes<HTMLButtonElement>,
    'children' | 'type' | 'role'
  >

type PanelTabsProps = TabsLinkProps | TabsButtonProps

/** One tab of a TabsBar: a link when it has `href` (works without script), otherwise a button. */
function PanelTabs(props: PanelTabsProps) {
  const {
    label,
    selected = false,
    variant = 'underline',
    device,
    className,
    tabIndex,
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
    return (
      <a {...(rest as AnchorHTMLAttributes<HTMLAnchorElement>)} {...shared}>
        {label}
      </a>
    )
  }
  return (
    <button type="button" {...(rest as ButtonHTMLAttributes<HTMLButtonElement>)} {...shared}>
      {label}
    </button>
  )
}

type NavigationTabsProps = Omit<
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
export type TabsProps = (PanelTabsProps & { mode?: 'panel' }) | NavigationTabsProps
export function Tabs(props: TabsProps) {
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
