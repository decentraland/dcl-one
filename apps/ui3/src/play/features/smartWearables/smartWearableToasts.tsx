import { showToast } from '../../design'
import type { SmartWearableEntryInfo, SmartWearableState } from '../../engine/generated'

// The states the player hears about when a smart wearable enters them.
const NOTICE: Partial<Record<SmartWearableState, string>> = {
  running: 'is running',
  blocked: 'is disabled in this scene',
  killed: 'stopped',
  denied: 'was not allowed to run'
}

/** Toast each smart wearable that entered a state with a notice since `before` (urn → state). */
export function toastSmartWearableChanges(before: ReadonlyMap<string, SmartWearableState>, entries: SmartWearableEntryInfo[]): void {
  for (const entry of entries) {
    if (before.get(entry.urn) === entry.state) continue
    const notice = NOTICE[entry.state]
    if (notice == null) continue
    showToast(
      <>
        <b>{entry.name}</b> {notice}
      </>
    )
  }
}
