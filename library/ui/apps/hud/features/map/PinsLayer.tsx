// The pins scenes put on the map. A pin wears the scene's own icon when it has one, the quest
// pin otherwise; it pulses until it is pointed at, and opens the place it stands on.

import { memo, useState, type CSSProperties } from 'react'
import type { ScenePin } from '@explorer-core/features/map/scenePins'
import { MARKER_ART } from './mapArt'
import styles from './MapCanvas.module.css'

/** The side, in image px, of the corner of a scene's icon the pin shows. */
const ICON_CROP = 36

const Pin = memo(function Pin({ pin, onClick }: { pin: ScenePin; onClick: (pin: ScenePin) => void }): React.JSX.Element {
  // a scene icon is shown from its bottom-left corner, so it is sized by its own pixels
  const [size, setSize] = useState<{ w: number; h: number } | null>(null)
  const style = { '--x': pin.position.x, '--y': pin.position.y } as CSSProperties
  return (
    <button
      type="button"
      className={`${styles.marker} ${styles.scenePin}`}
      style={style}
      data-marker="pins"
      aria-label={pin.title !== '' ? pin.title : 'Map pin'}
      onClick={(e) => {
        e.stopPropagation()
        onClick(pin)
      }}
    >
      <span className={styles.scenePinPulse}>
        <span className={styles.scenePinScaler}>
          <img className={styles.scenePinOutline} src={MARKER_ART.pinOutline} alt="" draggable={false} />
          <span className={styles.scenePinDisc} />
          {pin.icon != null && size != null ? (
            <span
              className={styles.scenePinIcon}
              style={{ backgroundImage: `url("${pin.icon}")`, '--iw': size.w / ICON_CROP, '--ih': size.h / ICON_CROP } as CSSProperties}
            />
          ) : (
            <img className={styles.scenePinDefault} src={MARKER_ART.quest} alt="" draggable={false} />
          )}
          {pin.icon != null && size == null && (
            <img hidden src={pin.icon} alt="" onLoad={(e) => setSize({ w: e.currentTarget.naturalWidth, h: e.currentTarget.naturalHeight })} />
          )}
        </span>
      </span>
    </button>
  )
})

export function PinsLayer({ pins, onPick }: { pins: readonly ScenePin[]; onPick: (pin: ScenePin) => void }): React.JSX.Element {
  return (
    <div className={styles.layer}>
      {pins.map((pin) => (
        <Pin key={pin.id} pin={pin} onClick={onPick} />
      ))}
    </div>
  )
}
