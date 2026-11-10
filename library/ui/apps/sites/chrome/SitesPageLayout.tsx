import { useId, type ReactNode } from 'react'
import styles from './SitesPageLayout.module.css'

type SitesPageLayoutProps = {
  /** The page's header, when it has one outside its own content. */
  header?: ReactNode
  footer: ReactNode
  /** The skip link's text, e.g. "Skip to content". */
  skipLabel: string
  children: ReactNode
}

/** The source-matching chrome around the Sites home and download pages: header, page, footer. */
function SitesPageLayout({ header, footer, skipLabel, children }: SitesPageLayoutProps) {
  const contentId = useId()
  return (
    <div className={styles.root}>
      <a className={styles.skip} href={`#${contentId}`}>
        {skipLabel}
      </a>
      {header}
      <div id={contentId} className={styles.content} tabIndex={-1}>
        {children}
      </div>
      {footer}
    </div>
  )
}

export { SitesPageLayout }
export type { SitesPageLayoutProps }
