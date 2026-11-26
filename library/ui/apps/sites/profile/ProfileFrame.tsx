import { useEffect, useId, type ComponentType, type MouseEvent, type ReactNode } from 'react'
import { Tabs, TabsBar, type TabsLinkProps } from '../../../index'
import styles from './ProfileFrame.module.css'
import { revealTab } from './reveal-tab'

export type ProfileShellTab = { id: string; label: string; href: string }

/** A modifier or middle click opens a new browser tab; this page's tab does not change. */
function isPlainClick(event: MouseEvent<HTMLAnchorElement>): boolean {
  return event.button === 0 && !event.metaKey && !event.ctrlKey && !event.shiftKey && !event.altKey
}

type ProfileFrameProps = {
  header: ReactNode
  tabs: ProfileShellTab[]
  active: string
  onTab: (id: string) => void
  linkComponent: ComponentType<TabsLinkProps>
  aside?: ReactNode
  children: ReactNode
}

export function ProfileFrame({
  header,
  tabs,
  active,
  onTab,
  linkComponent,
  aside,
  children
}: ProfileFrameProps) {
  const ids = useId()
  const tabId = (id: string) => `${ids}-tab-${id}`
  // On a narrow screen the bar scrolls sideways and opens at its start, where a late tab is out of sight.
  const activeTabId = tabId(active)
  useEffect(() => {
    const reveal = () => revealTab(document.getElementById(activeTabId))
    reveal()
    // A turned phone or a resized window can put the tab out of sight again.
    window.addEventListener('resize', reveal)
    return () => window.removeEventListener('resize', reveal)
  }, [activeTabId])
  const panelId = `${ids}-panel`

  return (
    <div className={styles.root}>
      <div className={styles.content}>
        {header}
        <TabsBar label="Profile sections" variant="compact" inset="content" scrollable>
          {tabs.map((tab) => (
            <Tabs
              key={tab.id}
              variant="compact"
              id={tabId(tab.id)}
              label={tab.label}
              href={tab.href}
              linkComponent={linkComponent}
              selected={tab.id === active}
              aria-controls={tab.id === active ? panelId : undefined}
              onClick={(event) => {
                if (isPlainClick(event)) onTab(tab.id)
              }}
            />
          ))}
        </TabsBar>
        <div
          role="tabpanel"
          id={panelId}
          aria-labelledby={tabId(active)}
          className={aside ? `${styles.body} ${styles.withAside}` : styles.body}
        >
          {aside ? <aside className={styles.aside}>{aside}</aside> : null}
          <div className={styles.main}>{children}</div>
        </div>
      </div>
    </div>
  )
}
