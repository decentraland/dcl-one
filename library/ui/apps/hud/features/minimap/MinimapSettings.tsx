// The menu behind the minimap panel's three dots. First the scene's options: Set as Home, a
// separator, Copy Link and Reload Scene. Then, below a separator, "Minimap
// settings", which opens the three sections the SDK7 HUD's minimap settings had: rotation mode,
// visualisation style, and which place categories get markers.
//
// Markers are modelled as the full category list even though the menu only offers three
// presets, so per-category toggles stay a UI change rather than a data change.

import { useState } from 'react'
import { Check, ChevronDown, ChevronRight, Toggle } from '../../design'
import { MaskIcon } from '../../design/MaskIcon'
import reloadIcon from '../../assets/minimap/reload.png'
import { ALL_MARKER_CATEGORIES } from '@explorer-core/features/minimap/minimapPrefs'
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

type MarkerPreset = 'all' | 'poi' | 'none'

function presetOf(markers: string[]): MarkerPreset | null {
  if (markers.length === 0) return 'none'
  if (markers.length === 1 && markers[0] === 'poi') return 'poi'
  if (markers.length === ALL_MARKER_CATEGORIES.length) return 'all'
  return null
}

const MARKER_PRESETS: Array<{ value: MarkerPreset; label: string; categories: string[] }> = [
  { value: 'all', label: 'All', categories: ALL_MARKER_CATEGORIES },
  { value: 'poi', label: 'Only POI', categories: ['poi'] },
  { value: 'none', label: 'None', categories: [] }
]

export function MinimapSettings({
  rotation,
  onRotation,
  style,
  onStyle,
  hideStyle,
  markers,
  onMarkers,
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
  markers: string[]
  onMarkers: (categories: string[]) => void
  /** Set as Home: on where the player stands in their home. Home can be set, not cleared, so the
   *  switch only turns on (turning it off does nothing, as the place panel's Home); absent where the place has no link (a preview, a local realm). */
  home?: { on: boolean; onSet: () => void }
  /** Copy the link to the place the player stands in; absent where the place has none. */
  onCopyLink?: () => void
  onReloadScene: () => void
}): React.JSX.Element {
  const preset = presetOf(markers)
  const [settingsOpen, setSettingsOpen] = useState(false)

  return (
    <div className={styles.menu} role="menu" aria-label="Minimap settings">
      {home != null && (
        <>
          {/* the switch is the one control; the row's label only gives it a bigger target */}
          <label className={`${styles.entry} ${styles.toggleEntry}`}>
            Set as Home
            <Toggle size="sm" checked={home.on} onChange={(on) => on && home.onSet()} aria-label="Set as Home" />
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

          <p className={styles.section}>Markers</p>
          {MARKER_PRESETS.map((o) => (
            <Option
              key={o.value}
              label={o.label}
              selected={preset === o.value}
              onSelect={() => onMarkers([...o.categories])}
            />
          ))}
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
