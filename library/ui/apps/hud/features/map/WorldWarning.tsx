// Shown over the map while the player is in a world: the map is still Genesis City's.

import { MaskIcon } from '../../design/MaskIcon'
import { PANEL_ART } from './mapArt'
import styles from './MapNotices.module.css'

export const WORLDS_WARNING = 'This is the Genesis City map. If you jump into any of this places you will leave the world you are currently visiting.'

export function WorldWarning({ onClose }: { onClose: () => void }): React.JSX.Element {
  return (
    <div className={styles.warning} role="status">
      <MaskIcon className={styles.warningIcon} src={PANEL_ART.warning} size={24} />
      <p className={styles.warningText}>{WORLDS_WARNING}</p>
      <button type="button" className={styles.warningClose} aria-label="Dismiss" onClick={onClose}>
        <MaskIcon src={PANEL_ART.closeThin} size={8} />
      </button>
    </div>
  )
}
