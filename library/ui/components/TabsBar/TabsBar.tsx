import type { AnchorHTMLAttributes, ComponentType, ReactNode } from 'react'
import { Tabs } from '../Tabs/Tabs'
import styles from './TabsBar.module.css'

export type TabsBarItem = Readonly<{
  id: string
  label: string
  href: string
  /** Hide inactive routes on narrow layouts; the controlled current route remains visible. */
  hiddenOnMobile?: boolean
}>
export type TabsBarLinkProps = AnchorHTMLAttributes<HTMLAnchorElement> & {
  href: string
  /** Intent-prefetch hint for the app adapter; consume it before forwarding native anchor props. */
  prefetch?: 'intent'
  'data-label'?: string
}
type SharedProps = {
  /** Validated route configuration. Invalid entries throw; validate remote data at the app boundary. */
  items: readonly TabsBarItem[]
  activeId: string
  label?: string
  onSelect?: (id: string) => void
  /** The app adapter forwards anchor attributes and owns client routing and intent prefetch. */
  LinkComponent?: ComponentType<TabsBarLinkProps>
  className?: string
}
export type TabsBarProps = SharedProps &
  (
    | { page?: 'create'; search?: never; actions?: never }
    | {
        page: 'shop' | 'learn'
        /** Explicit caller-owned content, including empty or loading states. */
        search: ReactNode
        actions: ReactNode
      }
  )

/** Navigation precedes caller-owned controls in visual and keyboard order at every size. */
export function TabsBar({
  page = 'create',
  items,
  activeId,
  label = 'Sections',
  onSelect,
  LinkComponent,
  className,
  search,
  actions
}: TabsBarProps) {
  if (!['create', 'shop', 'learn'].includes(page)) throw new Error('Unsupported TabsBar page')
  const ids = new Set<string>()
  for (const item of items) {
    if (typeof item.id !== 'string' || !item.id.trim() || ids.has(item.id))
      throw new Error('TabsBar item IDs must be unique non-empty strings')
    if (
      typeof item.label !== 'string' ||
      !item.label.trim() ||
      typeof item.href !== 'string' ||
      !item.href.trim()
    )
      throw new Error('TabsBar routes require a non-empty label and href')
    ids.add(item.id)
  }
  return (
    <nav
      aria-label={label.trim() || 'Sections'}
      data-page={page}
      className={[styles.root, className].filter(Boolean).join(' ')}
    >
      <div className={styles.navigation} data-region="navigation">
        <ul className={styles.list}>
          {items.map((item) => {
            const select: AnchorHTMLAttributes<HTMLAnchorElement>['onClick'] = (event) => {
              if (
                !event.defaultPrevented &&
                event.button === 0 &&
                !event.ctrlKey &&
                !event.metaKey &&
                !event.altKey &&
                !event.shiftKey
              )
                onSelect?.(item.id)
            }
            return (
              <li
                key={item.id}
                className={styles.item}
                data-mobile-hidden={(item.hiddenOnMobile && item.id !== activeId) || undefined}
              >
                {LinkComponent ? (
                  <LinkComponent
                    href={item.href}
                    prefetch="intent"
                    data-label={item.label}
                    className={styles.link}
                    aria-current={item.id === activeId ? 'page' : undefined}
                    onClick={select}
                  >
                    <span className={styles.text}>{item.label}</span>
                  </LinkComponent>
                ) : (
                  <Tabs
                    href={item.href}
                    label={item.label}
                    selected={item.id === activeId}
                    className={styles.link}
                    onClick={select}
                  />
                )}
              </li>
            )
          })}
        </ul>
      </div>
      {page !== 'create' ? (
        <div className={styles.controls} data-region="controls">
          <div className={styles.search} data-region="search">
            {search}
          </div>
          <div className={styles.actions} data-region="actions">
            {actions}
          </div>
        </div>
      ) : null}
    </nav>
  )
}
