// The notification ping: whether a chat line landing in a conversation makes a sound, the
// per-conversation "Notification Ping" (All Messages / Mentions Only / None). It only gates the
// sound: unread counts and mention markers are not affected. A default (the Chat settings tab)
// applies to every conversation not set by hand ('Nearby', a lowercase wallet, or
// `community:<id>`); closing a tab leaves its setting alone.

import { useSyncExternalStore } from 'react'
import { PREF, getPref, setPref } from '../../lib/prefs'

export type PingMode = 'all' | 'mentions' | 'none'

export const PING_MODES: readonly { mode: PingMode; label: string }[] = [
  { mode: 'all', label: 'All Messages' },
  { mode: 'mentions', label: 'Mentions Only' },
  { mode: 'none', label: 'None' }
]

export interface PingPrefs {
  /** The mode of a conversation not set by hand. */
  default: PingMode
  /** Conversations set by hand. */
  channels: Readonly<Record<string, PingMode>>
}

function isMode(value: unknown): value is PingMode {
  return value === 'all' || value === 'mentions' || value === 'none'
}

let prefs: PingPrefs | null = null
const listeners = new Set<() => void>()

function readPrefs(): PingPrefs {
  let channels: Record<string, PingMode> = {}
  try {
    const parsed: unknown = JSON.parse(getPref(PREF.chatPingChannels) ?? '{}')
    if (parsed != null && typeof parsed === 'object') {
      channels = Object.fromEntries(Object.entries(parsed).filter((e): e is [string, PingMode] => isMode(e[1])))
    }
  } catch {}
  const stored = getPref(PREF.chatPingDefault)
  return { default: isMode(stored) ? stored : 'all', channels }
}

function getPrefs(): PingPrefs {
  // read on first use: the stored values are only loaded when the HUD is about to render
  prefs ??= readPrefs()
  return prefs
}

function updatePrefs(next: PingPrefs): void {
  prefs = next
  listeners.forEach((l) => l())
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener)
  return () => listeners.delete(listener)
}

export function usePingPrefs(): PingPrefs {
  return useSyncExternalStore(subscribe, getPrefs)
}

export function pingMode(p: PingPrefs, channel: string): PingMode {
  return p.channels[channel] ?? p.default
}

/** The mode of a conversation now, outside React (the message handler). */
export function pingModeFor(channel: string): PingMode {
  return pingMode(getPrefs(), channel)
}

export function setPingDefault(mode: PingMode): void {
  setPref(PREF.chatPingDefault, mode)
  updatePrefs({ ...getPrefs(), default: mode })
}

export function setPingMode(channel: string, mode: PingMode): void {
  const channels = { ...getPrefs().channels, [channel]: mode }
  setPref(PREF.chatPingChannels, JSON.stringify(channels))
  updatePrefs({ ...getPrefs(), channels })
}

/** Back to the default for a conversation (chosen in its menu, its history deleted, the user no
 *  longer in it), or for every conversation (the settings tab reset). */
export function clearPingMode(channel?: string): void {
  const channels = channel == null ? {} : Object.fromEntries(Object.entries(getPrefs().channels).filter(([c]) => c !== channel))
  setPref(PREF.chatPingChannels, JSON.stringify(channels))
  updatePrefs({ ...getPrefs(), channels })
}

/** Whether a line that landed in `channel` makes a sound: never the user's own or a system line. */
export function shouldPing(channel: string, mentioned: boolean): boolean {
  const mode = pingModeFor(channel)
  return mode === 'all' || (mode === 'mentions' && mentioned)
}

/** Forget everything: for tests. */
export function resetPingPrefs(): void {
  prefs = null
}
