import { useId, type ReactNode } from 'react'
import { HelpTabs, type HelpTab } from './HelpTabs'
import styles from './HelpPage.module.css'

type HelpPageProps = {
  title: string
  description: string
  tabs: readonly HelpTab[]
  activeTab: string
  onTabChange: (id: string) => void
  /** The system status control: beside the title on phones, at the right of the content on wider screens. */
  status: ReactNode
  /** The active tab's section. */
  children: ReactNode
  /** The chat banner under the content. */
  banner?: ReactNode
}

/**
 * The help center frame: the title, intro and section tabs in a left column, the active section beside them and
 * the system status at the right. Phones stack it, with the status beside the title and the tabs wrapping onto rows.
 */
function HelpPage({
  title,
  description,
  tabs,
  activeTab,
  onTabChange,
  status,
  children,
  banner
}: HelpPageProps) {
  const prefix = useId()
  const tabId = (id: string) => `${prefix}-tab-${id}`
  const panelId = `${prefix}-panel`

  return (
    <main className={styles.root}>
      <div className={styles.layout}>
        <h1 className={styles.title}>{title}</h1>
        <div className={styles.status}>{status}</div>
        <p className={styles.description}>{description}</p>
        <div className={styles.tabs}>
          <HelpTabs
            tabs={tabs}
            active={activeTab}
            onChange={onTabChange}
            label={title}
            tabId={tabId}
            panelId={panelId}
          />
        </div>
        <div
          className={styles.panel}
          id={panelId}
          role="tabpanel"
          aria-labelledby={tabId(activeTab)}
        >
          {children}
        </div>
      </div>
      {banner}
    </main>
  )
}

export { HelpPage }
export type { HelpPageProps }
