import { createContext, useContext, type ReactNode } from 'react'
import { useChromeNav } from './chrome-nav'
import styles from './SiteLayout.module.css'

type SiteLayoutNavigation = { sections?: (closeMenu: () => void) => ReactNode }
const SiteLayoutContext = createContext<SiteLayoutNavigation | null>(null)

export const useSiteLayout = () => useContext(SiteLayoutContext)

export type SiteLayoutProps = SiteLayoutNavigation & {
  navigation?: ReactNode
  overlayNavigation?: boolean
  children?: ReactNode
  className?: string
}

export default function SiteLayout({
  navigation,
  sections,
  children,
  className,
  overlayNavigation = false
}: SiteLayoutProps) {
  const { headerLayout } = useChromeNav()
  return (
    <SiteLayoutContext.Provider value={{ sections }}>
      <div
        data-site-layout=""
        data-header-layout={headerLayout}
        data-header-overlay={(headerLayout === 'native' && overlayNavigation) || undefined}
        data-site-layout-sections={sections ? '' : undefined}
        className={className ? `${styles.root} ${className}` : styles.root}
      >
        {navigation ? (
          <div className={styles.navigation} data-site-layout-navigation="">
            {navigation}
          </div>
        ) : null}
        {sections ? (
          <aside
            className={styles.sidebar}
            data-site-layout-sidebar=""
            aria-label="Secondary navigation"
          >
            {sections(() => {})}
          </aside>
        ) : null}
        <div className={styles.content} data-site-layout-content="">
          {children}
        </div>
      </div>
    </SiteLayoutContext.Provider>
  )
}
