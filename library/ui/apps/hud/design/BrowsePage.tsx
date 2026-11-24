// BrowsePage — the frame shared by the menu's browse pages (Places, Events, Shop): a toolbar with
// section tabs on the left and search/sort controls on the right, over a purple grid panel
// (decentraland-ui2 GridPanel).

import type { ReactNode } from 'react'
import { Spinner } from './Spinner'
import styles from './BrowsePage.module.css'

export function BrowseToolbar({ tabs, children }: { tabs: ReactNode; children?: ReactNode }): React.JSX.Element {
  return (
    <div className={styles.toolbar}>
      {tabs}
      {children != null && <div className={styles.controls}>{children}</div>}
    </div>
  )
}

/** A fixed-width slot in the toolbar's controls, so the row doesn't reflow as values change. */
export function BrowseControl({ size, children }: { size: 'search' | 'select'; children: ReactNode }): React.JSX.Element {
  return <div className={styles[size]}>{children}</div>
}

/** The results panel. `label` names it as a region; `busy` marks it loading (`aria-busy`) while the
 *  results it shows do not yet answer the current query. */
export function BrowsePanel({ children, label, busy }: { children: ReactNode; label?: string; busy?: boolean }): React.JSX.Element {
  return (
    <div className={styles.panel} role={label != null ? 'region' : undefined} aria-label={label} aria-busy={busy}>
      {children}
    </div>
  )
}

export function BrowseLoading(): React.JSX.Element {
  return (
    <div className={styles.center}>
      <Spinner size={34} />
    </div>
  )
}
