// The menu behind the minimap panel's three dots. First the scene's options: Set as Home, a
// separator, Copy Link and Reload Scene. Then, below a separator, "Minimap
// settings", which opens the rotation mode and the visualisation style. Which markers show is the
// full map's Layers panel's.

import { useState } from 'react'
import { Check, ChevronDown, ChevronRight, Toggle } from '../../design'
import { MaskIcon } from '../../design/MaskIcon'
import reloadIcon from '../../assets/minimap/reload.png'
import type { MinimapRotation, MinimapStyle } from '@explorer-core/engine/protocol'
import { LinkMark } from './icons'
import styles from './MinimapSettings.module.css'

const ROTATION_LABELS: Array<{ value: MinimapRotation; label: string }> = [
  { value: 'camera', label: 'Rotate with camera' },
  { value: 'north', label: 'Fixed north' }
]

const STYLE_LABELS: Array<{ value: MinimapStyle; label: string }> = [
  { value: 'parcel', label: 'Parcel atlas' },
  { value: 'satellite', label: 'Satellite' },
  { value: 'imposters', label: 'Camera' }
]

export function MinimapSettings({
  rotation,
  onRotation,
  style,
  onStyle,
  hideStyle,
  home,
  onCopyLink,
  onReloadScene
}: {
  rotation: MinimapRotation
  onRotation: (r: MinimapRotation) => void
  style: MinimapStyle
  onStyle: (s: MinimapStyle) => void
  /** True outside Genesis City, where only the Camera style can render anything. */
  hideStyle: boolean
  /** Set as Home: on where the player stands in their home; turning it off takes the home back,
   *  as the place panel's Home button does. Absent where the place has no link (a preview, a
   *  local realm). */
  home?: { on: boolean; onSet: () => void; onClear: () => void }
  /** Copy the link to the place the player stands in; absent where the place has none. */
  onCopyLink?: () => void
  onReloadScene: () => void
}): React.JSX.Element {
  const [settingsOpen, setSettingsOpen] = useState(false)

  return (
    <div className={styles.menu} role="menu" aria-label="Minimap settings">
      {home != null && (
        <>
          {/* the switch is the one control; the row's label only gives it a bigger target */}
          <label className={`${styles.entry} ${styles.toggleEntry}`}>
            Set as Home
            <Toggle size="sm" checked={home.on} onChange={(on) => (on ? home.onSet() : home.onClear())} aria-label="Set as Home" />
          </label>
          <div className={styles.separator} />
        </>
      )}
      {onCopyLink != null && (
        <button type="button" className={styles.entry} role="menuitem" onClick={onCopyLink}>
          <LinkMark />
          Copy Link
        </button>
      )}
      <button type="button" className={styles.entry} role="menuitem" onClick={onReloadScene}>
        <MaskIcon src={reloadIcon} size={20} />
        Reload Scene
      </button>
      <div className={styles.separator} />
      <button type="button" className={styles.more} role="menuitem" aria-expanded={settingsOpen} onClick={() => setSettingsOpen((v) => !v)}>
        Minimap settings
        {settingsOpen ? <ChevronDown size={16} /> : <ChevronRight size={16} />}
      </button>

      {settingsOpen && (
        <>
          <p className={styles.section}>Rotation mode</p>
          {ROTATION_LABELS.map((o) => (
            <Option key={o.value} label={o.label} selected={rotation === o.value} onSelect={() => onRotation(o.value)} />
          ))}

          {!hideStyle && (
            <>
              <p className={styles.section}>Visualization style</p>
              {STYLE_LABELS.map((o) => (
                <Option key={o.value} label={o.label} selected={style === o.value} onSelect={() => onStyle(o.value)} />
              ))}
            </>
          )}
        </>
      )}
    </div>
  )
}

function Option({ label, selected, onSelect }: { label: string; selected: boolean; onSelect: () => void }): React.JSX.Element {
  return (
    <button type="button" className={styles.option} role="menuitemradio" aria-checked={selected} onClick={onSelect}>
      <span className={styles.check}>{selected && <Check size={13} />}</span>
      {label}
    </button>
  )
}
