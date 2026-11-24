// Whether the web shell asks before the page goes while a session runs (apps/ui3 shell/mount.ts):
// off until the player turns it on in Settings, where the host can ask (CAN_ASK_BEFORE_LEAVING).

import { useSyncExternalStore } from 'react'
import { PREF, getPref, setPref } from '@explorer-core/lib/prefs'

let ask: boolean | null = null
const listeners = new Set<() => void>()

function getAsk(): boolean {
  // read on first use: the stored values are only loaded when the HUD is about to render
  ask ??= getPref(PREF.askBeforeLeaving) === '1'
  return ask
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener)
  return () => listeners.delete(listener)
}

export function useAskBeforeLeaving(): boolean {
  return useSyncExternalStore(subscribe, getAsk)
}

export function setAskBeforeLeaving(on: boolean): void {
  setPref(PREF.askBeforeLeaving, on ? '1' : '0')
  ask = on
  listeners.forEach((l) => l())
}
