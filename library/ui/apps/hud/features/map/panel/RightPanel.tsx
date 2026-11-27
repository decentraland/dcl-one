// The map's right-hand column: the search bar on top, and under it a panel that slides in to
// show search results, a place or an event.

import type { NavmapSnapshot } from '@explorer-core/features/map/navmap'
import { PANEL_ART } from '../mapArt'
import styles from './MapPanel.module.css'
import { MaskIcon } from '@dcl/one-ui'

export function RightPanel({
  snapshot,
  searchBar,
  children,
  compact = false,
  onExpand,
  onCollapse
}: {
  snapshot: NavmapSnapshot
  searchBar: React.ReactNode
  children: React.ReactNode
  /** Hosted in the lobby's section modal. */
  compact?: boolean
  onExpand: () => void
  onCollapse: () => void
}): React.JSX.Element {
  const { expanded, handle } = snapshot
  return (
    <aside className={compact ? `${styles.panel} ${styles.compact}` : styles.panel} aria-label="Places and events">
      {handle != null && (
        <div className={`${styles.slide} ${styles.handleRail} ${expanded ? '' : styles.slideOut}`.trim()}>
          <button
            type="button"
            className={styles.handle}
            aria-label={handle === 'collapse' ? 'Collapse panel' : 'Expand panel'}
            onClick={handle === 'collapse' ? onCollapse : onExpand}
          >
            <MaskIcon src={handle === 'collapse' ? PANEL_ART.arrowRight : PANEL_ART.arrowLeft} size={14} />
            <MaskIcon src={handle === 'collapse' ? PANEL_ART.arrowRight : PANEL_ART.arrowLeft} size={14} />
          </button>
        </div>
      )}
      <div className={`${styles.slide} ${expanded ? '' : styles.slideOut}`.trim()} inert={!expanded}>
        <div className={styles.section}>{children}</div>
        <div className={styles.band} />
      </div>
      {searchBar}
    </aside>
  )
}
