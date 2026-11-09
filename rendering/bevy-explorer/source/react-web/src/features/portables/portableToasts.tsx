import { showToast } from '../../design'
import type { PortableInfo, PortableState } from '../../engine/generated'

// The states the player hears about when a portable enters them.
const NOTICE: Partial<Record<PortableState, string>> = {
  running: 'is running',
  blocked: 'is paused in this scene',
  stopped: 'stopped',
  failed: 'failed to run'
}

/** A portable's identity across lists: its kind and pid (a stopped smart wearable has no hash). */
export function portableKey(p: PortableInfo): string {
  return `${p.kind}:${p.pid}`
}

/** Toast each portable that entered a state with a notice since `before` (portableKey → state). */
export function toastPortableChanges(before: ReadonlyMap<string, PortableState>, list: PortableInfo[]): void {
  for (const p of list) {
    if (before.get(portableKey(p)) === p.state) continue
    const notice = NOTICE[p.state]
    if (notice == null) continue
    showToast(
      <>
        <b>{p.name}</b> {notice}
      </>
    )
  }
}
