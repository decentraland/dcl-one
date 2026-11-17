// The route under way, named at the bottom of the map, with the way to end it.

import type { Destination } from '@explorer-core/features/map/navigation'
import { MaskIcon } from '../../design/MaskIcon'
import { PANEL_ART } from './mapArt'
import styles from './MapNotices.module.css'

export function DestinationBanner({ destination, onExit }: { destination: Destination; onExit: () => void }): React.JSX.Element {
  return (
    <div className={styles.destination} role="status">
      <MaskIcon className={styles.destinationIcon} src={PANEL_ART.pinOutline} size={26} />
      <span className={styles.destinationLabel}>DESTINATION</span>
      <span className={styles.destinationTitle}>{destination.title}</span>
      <button type="button" className={styles.destinationExit} onClick={onExit}>
        Exit
      </button>
    </div>
  )
}
