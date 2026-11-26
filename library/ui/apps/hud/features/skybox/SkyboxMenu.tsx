// Skybox menu — the sidebar's NIGHT/DAY panel: "Auto" lets the engine's day cycle
// run; the "Custom" time-of-day slider is locked meanwhile. Where the scene sets the time of day
// both are off and a line says which setting gives them back.

import { useEffect, useId } from 'react'
import { Panel, Slider, Toggle, hasOpenPopup } from '../../design'
import type { SkyboxState } from '../session/useEngineSession'
import styles from './SkyboxMenu.module.css'

export function formatHours(hours: number): string {
  const total = Math.round(hours * 60) % (24 * 60)
  return `${String(Math.floor(total / 60)).padStart(2, '0')}:${String(total % 60).padStart(2, '0')}`
}

export function SkyboxMenu({ skybox }: { skybox: SkyboxState }): React.JSX.Element | null {
  // Clicking anywhere outside (except the rail, whose Skybox button toggles it) closes it.
  const { open, toggle } = skybox
  const noteId = useId()
  const describedBy = skybox.sceneLocked ? noteId : undefined
  useEffect(() => {
    if (!open) return
    const onDown = (e: PointerEvent): void => {
      if (hasOpenPopup()) return
      if (e.target instanceof Element && e.target.closest('[role="dialog"][aria-label="Skybox"], nav[aria-label="Main navigation"]')) return
      toggle()
    }
    document.addEventListener('pointerdown', onDown, true)
    return () => document.removeEventListener('pointerdown', onDown, true)
  }, [open, toggle])
  if (!open) return null
  return (
    <Panel className={styles.root} role="dialog" aria-label="Skybox">
      <div className={styles.title}>NIGHT/DAY</div>
      <div className={styles.divider} />
      <label className={styles.row}>
        <span>Auto</span>
        <Toggle size="sm" checked={skybox.progressing} onChange={skybox.setProgressing} disabled={skybox.sceneLocked} aria-describedby={describedBy} />
      </label>
      <div className={styles.divider} />
      <div className={styles.custom} data-locked={skybox.progressing || skybox.sceneLocked}>
        <div className={styles.head}>
          <span>Custom</span>
          <span className={styles.time}>{formatHours(skybox.hours)}</span>
        </div>
        <Slider variant="thick" arrows value={skybox.hours} min={0} max={24} step={0.25} onChange={skybox.setHours} disabled={skybox.progressing || skybox.sceneLocked} aria-describedby={describedBy} />
      </div>
      {skybox.sceneLocked && (
        <p id={noteId} className={styles.locked} role="note">
          This scene sets the time of day. To change it, turn on “Allow overriding scene controls” in Settings.
        </p>
      )}
    </Panel>
  )
}
