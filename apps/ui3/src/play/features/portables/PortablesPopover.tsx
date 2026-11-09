// Portable experiences popover beside the sidebar's portables button: each portable the engine
// runs beside the scene (smart wearables and scene-spawned portables, never the HUD's own scene or a
// startup portable), what it is doing, and Stop (the engine console's `/stop_portable <hash>`) or,
// for a stopped smart wearable, Restart (`/smart_wearable restart <urn>`). The button shows while the
// engine lists any.

import { Button } from '../../design'
import type { PortableInfo, PortableInfoKind, PortableState } from '../../engine/generated'
import { useRailPopover } from '../sidebar/useRailPopover'
import styles from './PortablesPopover.module.css'

const STATE_LABEL: Record<PortableState, string> = {
  loading: 'Loading',
  running: 'Running',
  failed: 'Failed',
  blocked: 'Paused in this scene',
  stopped: 'Stopped'
}

const KIND_LABEL: Record<PortableInfoKind, string> = {
  scene: 'Portable experience',
  smartWearable: 'Smart wearable'
}

/** Whether the rail shows the portables button. */
export function showPortables(list: PortableInfo[]): boolean {
  return list.length > 0
}

/** The rail button's tooltip: how many run, and how many this scene pauses. */
export function portablesTooltip(list: PortableInfo[]): string {
  const running = list.filter((p) => p.state === 'running').length
  const paused = list.filter((p) => p.state === 'blocked').length
  return paused > 0 ? `${running} running, ${paused} paused in this scene` : `${running} running`
}

function stateText(p: PortableInfo): string {
  const label = STATE_LABEL[p.state]
  return p.state === 'failed' && p.reason != null && p.reason !== '' ? `${label}: ${p.reason}` : label
}

export function PortablesPopover({
  anchor,
  list,
  onStop,
  onRestart,
  onClose
}: {
  /** The sidebar portables button the popover sits beside. */
  anchor: HTMLElement | null
  list: PortableInfo[]
  onStop: (hash: string) => void
  onRestart: (urn: string) => void
  onClose: () => void
}): React.JSX.Element {
  const { ref, style } = useRailPopover(anchor, onClose)

  return (
    <div ref={ref} className={styles.panel} role="dialog" aria-labelledby="portables-title" style={style}>
      <h2 id="portables-title" className={styles.title}>
        Portable experiences
      </h2>
      <div className={styles.divider} />
      {list.map((p) => {
        const { hash } = p
        return (
          <div key={`${p.kind}:${p.pid}`} className={styles.row}>
            <div className={styles.text}>
              <span className={styles.name}>{p.name}</span>
              <span className={styles.kind}>{KIND_LABEL[p.kind]}</span>
              <span className={styles.state} data-state={p.state}>
                {stateText(p)}
              </span>
            </div>
            {p.kind === 'smartWearable' && p.state === 'stopped' ? (
              <Button size="sm" variant="ghost" aria-label={`Restart ${p.name}`} onClick={() => onRestart(p.pid)}>
                Restart
              </Button>
            ) : (
              hash != null && (
                <Button size="sm" variant="ghost" aria-label={`Stop ${p.name}`} onClick={() => onStop(hash)}>
                  Stop
                </Button>
              )
            )}
          </div>
        )
      })}
    </div>
  )
}
