// The map's controls, bottom-left: zoom, centre on the player, centre on home, and the Layers panel.

import { MAX_ZOOM_LEVEL } from '@explorer-core/features/map/mapZoom'
import { MaskIcon } from '../../design/MaskIcon'
import { MAP_ICONS } from './mapArt'
import styles from './MapChrome.module.css'

export function MapToolbar({
  level,
  layersOpen,
  onZoom,
  onCenterPlayer,
  onCenterHome,
  onToggleLayers
}: {
  level: number
  layersOpen: boolean
  onZoom: (zoomIn: boolean) => void
  onCenterPlayer: () => void
  onCenterHome: () => void
  onToggleLayers: () => void
}): React.JSX.Element {
  return (
    <div className={styles.toolbar} role="toolbar" aria-label="Map controls">
      <button type="button" className={`${styles.tool} ${styles.toolLayers}`} aria-label="Layers" aria-expanded={layersOpen} onClick={onToggleLayers}>
        <MaskIcon src={MAP_ICONS.layers} size={22} />
      </button>
      <button type="button" className={`${styles.tool} ${styles.toolHome}`} aria-label="Center on home" onClick={onCenterHome}>
        <MaskIcon src={MAP_ICONS.centerHome} size={22} />
      </button>
      <button type="button" className={`${styles.tool} ${styles.toolPlayer}`} aria-label="Center on me" onClick={onCenterPlayer}>
        <MaskIcon src={MAP_ICONS.centerPlayer} size={22} />
      </button>
      <div className={styles.zoom}>
        <button type="button" className={styles.zoomButton} aria-label="Zoom in" disabled={level >= MAX_ZOOM_LEVEL} onClick={() => onZoom(true)}>
          <MaskIcon src={MAP_ICONS.plus} size={14} />
        </button>
        <button type="button" className={styles.zoomButton} aria-label="Zoom out" disabled={level <= 0} onClick={() => onZoom(false)}>
          <span className={styles.minus} />
        </button>
      </div>
    </div>
  )
}
