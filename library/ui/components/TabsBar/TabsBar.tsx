import type { HTMLAttributes, KeyboardEvent, ReactNode } from 'react'
import { useLayoutEffect, useRef, type AnchorHTMLAttributes, type ComponentType } from 'react'
import { Tabs } from '../Tabs/Tabs'
import styles from './TabsBar.module.css'
import { assertNavigationEntries, isPlainLinkClick } from '../../link.helpers'

type PanelTabsBarProps = {
  label: string
  children: ReactNode
  /** `compact` is the transparent underline bar. `pill` spaces rounded filter tabs. */
  variant?: 'underline' | 'compact' | 'pill'
  /**
   * The bar's inline padding. Without it the variant's own (the Figma 64px for `underline`, none for
   * `pill`); `content` lines the first tab up with a page's content column: 16px, 32px from 1024px.
   */
  inset?: 'content'
  /** Scrolls sideways instead of overflowing, with room inside the scroller for the tabs' focus ring. */
  scrollable?: boolean
} & Omit<HTMLAttributes<HTMLDivElement>, 'children' | 'role' | 'aria-label'>

const KEYS = new Set(['ArrowLeft', 'ArrowRight', 'Home', 'End'])

/** A tab list: arrows, Home and End move focus between its tabs; a button tab is also activated. */
function PanelTabsBar({
  label,
  children,
  variant = 'underline',
  inset,
  scrollable = false,
  className,
  onKeyDown,
  ...rest
}: PanelTabsBarProps) {
  const handleKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    onKeyDown?.(event)
    if (event.defaultPrevented || !KEYS.has(event.key)) return
    const tabs = Array.from(event.currentTarget.querySelectorAll<HTMLElement>('[role="tab"]'))
    const current = tabs.indexOf(event.target as HTMLElement)
    if (current < 0) return
    event.preventDefault()
    const last = tabs.length - 1
    const next =
      event.key === 'Home'
        ? 0
        : event.key === 'End'
          ? last
          : (current + (event.key === 'ArrowRight' ? 1 : -1) + tabs.length) % tabs.length
    const tab = tabs[next]!
    tab.focus()
    if (tab instanceof HTMLButtonElement) tab.click()
  }
  return (
    <div
      {...rest}
      role="tablist"
      aria-label={label}
      className={[
        styles.root,
        styles[variant],
        inset === 'content' && styles.insetContent,
        scrollable && styles.scrollable,
        className
      ]
        .filter(Boolean)
        .join(' ')}
      onKeyDown={handleKeyDown}
    >
      {children}
    </div>
  )
}

export type TabsBarItem = Readonly<{
  id: string
  label: string
  href: string
  /** Hide inactive routes on narrow layouts; the controlled current route remains visible. */
  hiddenOnMobile?: boolean
  /** A shortcut shown only on screens up to 768px wide. */
  mobileOnly?: boolean
  /** Apply the campaign gradient to the label while retaining the standard underline and focus ring. */
  accent?: 'campaign'
}>
/** Render one native anchor and forward children, className, data-label, aria-current and events. */
export type TabsBarLinkProps = AnchorHTMLAttributes<HTMLAnchorElement> & {
  href: string
  /** Intent-prefetch hint for the app adapter; consume it before forwarding native anchor props. */
  prefetch?: 'intent'
  'data-label'?: string
}
type SharedProps = {
  /** Caller-owned route configuration. IDs, labels and destinations must be non-empty. */
  items: readonly TabsBarItem[]
  activeId: string | null
  /** Describes the active route link from the current page. */
  current?: 'page' | 'true'
  label?: string
  onSelect?: (id: string) => void
  /** The app adapter forwards anchor attributes and owns client routing and intent prefetch. */
  /** Keep adapter identity stable across renders to preserve native DOM and scroll state. */
  LinkComponent?: ComponentType<TabsBarLinkProps>
  className?: string
}
export type TabsBarNavigationProps = SharedProps & {
  mode: 'navigation'
  children?: never
  variant?: never
} & (
    | { page?: 'create'; layout?: never; search?: never; actions?: never }
    | { page: 'shop'; layout: 'links'; search?: never; actions?: never }
    | { page: 'blog'; layout?: never; search: ReactNode; actions?: never }
    | {
        page: 'shop' | 'learn'
        layout?: never
        /** Explicit caller-owned content, including empty or loading states. */
        search: ReactNode
        actions: ReactNode
      }
  )

/** Navigation precedes caller-owned controls in visual and keyboard order at every size. */
function NavigationTabsBar({
  page = 'create',
  layout,
  items,
  activeId,
  current = 'page',
  label = 'Sections',
  onSelect,
  LinkComponent,
  className,
  search,
  actions
}: TabsBarNavigationProps) {
  if (!['create', 'shop', 'learn', 'blog'].includes(page)) throw new Error('Unsupported TabsBar page')
  assertNavigationEntries(items, (href) => typeof href === 'string' && !!href.trim(), {
    id: 'TabsBar item IDs must be unique non-empty strings',
    entry: 'TabsBar routes require a non-empty label and href'
  })
  const listRef = useRef<HTMLUListElement>(null)
  const routeLayout = JSON.stringify(
    items.map((item) => [item.id, item.label, item.href, !!item.hiddenOnMobile, !!item.mobileOnly])
  )
  useLayoutEffect(() => {
    const list = listRef.current
    if (!list) return
    let manualScroll = false
    let expectedScroll = list.scrollLeft
    let viewportWidth = list.ownerDocument.defaultView?.innerWidth
    /* Mark the edges with more routes beyond them; CSS fades those edges. */
    const updateFades = () => {
      const scrolled = Math.abs(list.scrollLeft)
      const hidden = list.scrollWidth - list.clientWidth
      list.toggleAttribute('data-fade-start', scrolled > 1)
      list.toggleAttribute('data-fade-end', hidden - scrolled > 1)
      list.toggleAttribute('data-rtl', getComputedStyle(list).direction === 'rtl')
    }
    /* Keep the current route clear of the fades: the scroll padding is the fade width. */
    const revealCurrent = () => {
      if (!list.isConnected) return
      const current = list.querySelector<HTMLElement>('[aria-current]')
      if (!current) return
      const frame = list.getBoundingClientRect()
      const route = current.getBoundingClientRect()
      if (frame.width <= 0) return
      const inset = Math.min(
        parseFloat(getComputedStyle(list).scrollPaddingInlineStart) || 0,
        Math.max(0, (frame.width - route.width) / 2)
      )
      const left = frame.left + inset
      const right = frame.right - inset
      const offset =
        route.width > right - left
          ? getComputedStyle(list).direction === 'rtl'
            ? route.right - frame.right
            : route.left - frame.left
          : route.left < left
            ? route.left - left
            : route.right > right
              ? route.right - right
              : 0
      if (Math.abs(offset) > 1) list.scrollLeft += offset
      expectedScroll = list.scrollLeft
    }
    revealCurrent()
    updateFades()
    const onScroll = () => {
      if (Math.abs(list.scrollLeft - expectedScroll) > 1) manualScroll = true
      updateFades()
    }
    list.addEventListener('scroll', onScroll, { passive: true })
    if (typeof ResizeObserver === 'undefined')
      return () => list.removeEventListener('scroll', onScroll)
    const observer = new ResizeObserver(() => {
      const width = list.ownerDocument.defaultView?.innerWidth
      if (width !== viewportWidth) {
        viewportWidth = width
        manualScroll = false
      }
      if (!manualScroll) revealCurrent()
      updateFades()
    })
    observer.observe(list)
    for (const item of list.children) observer.observe(item)
    return () => {
      observer.disconnect()
      list.removeEventListener('scroll', onScroll)
    }
  }, [activeId, routeLayout, page, LinkComponent])
  return (
    <nav
      aria-label={label.trim() || 'Sections'}
      data-page={page}
      data-layout={layout}
      className={[styles.routeRoot, className].filter(Boolean).join(' ')}
    >
      <div className={styles.routeNavigation} data-region="navigation">
        <ul ref={listRef} className={styles.routeList}>
          {items.map((item) => {
            const select: AnchorHTMLAttributes<HTMLAnchorElement>['onClick'] = (event) => {
              if (isPlainLinkClick(event)) onSelect?.(item.id)
            }
            return (
              <li
                key={item.id}
                className={styles.routeItem}
                data-mobile-only={item.mobileOnly || undefined}
                data-accent={item.accent}
                data-mobile-hidden={(item.hiddenOnMobile && item.id !== activeId) || undefined}
              >
                {LinkComponent ? (
                  <LinkComponent
                    href={item.href}
                    prefetch="intent"
                    data-label={item.label}
                    className={styles.routeLink}
                    aria-current={item.id === activeId ? current : undefined}
                    onClick={select}
                  >
                    <span className={styles.routeText}>{item.label}</span>
                  </LinkComponent>
                ) : (
                  <Tabs
                    mode="navigation"
                    href={item.href}
                    label={item.label}
                    selected={item.id === activeId}
                    current={current}
                    className={styles.routeLink}
                    onClick={select}
                  />
                )}
              </li>
            )
          })}
        </ul>
      </div>
      {page !== 'create' && layout !== 'links' ? (
        <div className={styles.routeControls} data-region="controls">
          <div className={styles.routeSearch} data-region="search">
            {search}
          </div>
          {page !== 'blog' && (
            <div className={styles.routeActions} data-region="actions">
              {actions}
            </div>
          )}
        </div>
      ) : null}
    </nav>
  )
}

export type TabsBarProps = PanelTabsBarProps
type TabsBarComponentProps =
  | (TabsBarProps & {
      mode?: 'panel'
      items?: never
      activeId?: never
      current?: never
      page?: never
      search?: never
      actions?: never
      LinkComponent?: never
    })
  | TabsBarNavigationProps
/** Choose panel-tab behavior or explicitly controlled route navigation. */
export function TabsBar(props: TabsBarComponentProps) {
  if (props.mode === 'navigation') return <NavigationTabsBar {...props} />
  const { mode, ...panel } = props
  return <PanelTabsBar {...panel} data-mode={mode} />
}
