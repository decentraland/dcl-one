// Smart wearables popover beside the sidebar's smart wearables button: each smart wearable the engine
// runs for the player, what it is doing, and Stop or Restart (the engine console's
// `/smart_wearable kill|restart <urn>`). The button shows only while one is running, held back or
// waiting for the player's answer.

import { Button } from '../../design'
import type { SmartWearableEntryInfo, SmartWearableState, SmartWearableStateInfo } from '../../engine/generated'
import { useRailPopover } from '../sidebar/useRailPopover'
import styles from './SmartWearablesPopover.module.css'

const STATE_LABEL: Record<SmartWearableState, string> = {
  loading: 'Loading',
  awaitingAuthorization: 'Waiting for your answer',
  running: 'Running',
  blocked: 'Disabled in this scene',
  killed: 'Stopped',
  denied: 'Not allowed',
  notSmart: 'Not a smart wearable',
  failed: 'Failed'
}

// The states that put the button on the rail.
const RAIL_STATES: ReadonlySet<SmartWearableState> = new Set(['running', 'blocked', 'killed', 'denied', 'awaitingAuthorization'])
const STOPPABLE: ReadonlySet<SmartWearableState> = new Set(['loading', 'running', 'blocked'])
const RESTARTABLE: ReadonlySet<SmartWearableState> = new Set(['killed', 'denied', 'failed'])

export const DISABLED_IN_SCENE = 'Smart wearables are disabled in this scene'

/** Whether the rail shows the smart wearables button. */
export function showSmartWearables(entries: SmartWearableEntryInfo[]): boolean {
  return entries.some((e) => RAIL_STATES.has(e.state))
}

/** The rail button's tooltip: the scene's veto, else how many run and how many do not. */
export function smartWearablesTooltip(info: SmartWearableStateInfo): string {
  if (!info.currentSceneAllows) return DISABLED_IN_SCENE
  const listed = info.entries.filter((e) => e.state !== 'notSmart')
  const active = listed.filter((e) => e.state === 'running').length
  return `${active} active / ${listed.length - active} inactive`
}

export function SmartWearablesPopover({
  anchor,
  info,
  onStop,
  onRestart,
  onClose
}: {
  /** The sidebar smart wearables button the popover sits beside. */
  anchor: HTMLElement | null
  info: SmartWearableStateInfo
  onStop: (urn: string) => void
  onRestart: (urn: string) => void
  onClose: () => void
}): React.JSX.Element {
  const { ref, style } = useRailPopover(anchor, onClose)
  const entries = info.entries.filter((e) => e.state !== 'notSmart')
  const blocked = !info.currentSceneAllows || entries.some((e) => e.state === 'blocked')

  return (
    <div ref={ref} className={styles.panel} role="dialog" aria-labelledby="smart-wearables-title" style={style}>
      <h2 id="smart-wearables-title" className={styles.title}>
        Smart wearables
      </h2>
      <div className={styles.divider} />
      {blocked && <div className={styles.note}>{DISABLED_IN_SCENE}</div>}
      {entries.map((e) => (
        <div key={e.urn} className={styles.row}>
          <div className={styles.text}>
            <span className={styles.name}>{e.name}</span>
            <span className={styles.state} data-state={e.state}>
              {STATE_LABEL[e.state]}
              {e.reason != null && e.reason !== '' ? `: ${e.reason}` : ''}
            </span>
          </div>
          {STOPPABLE.has(e.state) && (
            <Button size="sm" variant="ghost" aria-label={`Stop ${e.name}`} onClick={() => onStop(e.urn)}>
              Stop
            </Button>
          )}
          {RESTARTABLE.has(e.state) && (
            <Button size="sm" variant="ghost" aria-label={`Restart ${e.name}`} onClick={() => onRestart(e.urn)}>
              Restart
            </Button>
          )}
        </div>
      ))}
    </div>
  )
}
