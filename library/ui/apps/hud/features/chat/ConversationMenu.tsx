// The conversation options menu on a chat title bar: the caller's entries, then the
// "Notification Ping" submenu every conversation has (Default / All Messages / Mentions Only /
// None), which only decides whether a line landing there sounds.

import { useState } from 'react'
import { KebabMenu, type ContextMenuItem } from '../../design'
import { PING_MODES, clearPingMode, setPingMode, usePingPrefs } from './ping'

export const PING_LABEL = 'Notification Ping'

/** The menu's own entries: label, optional danger, and what a click does (the menu closes first). */
export interface ConversationMenuEntry {
  label: string
  danger?: boolean
  onClick: () => void
}

export function ConversationMenu({ channel, entries = [] }: { channel: string; entries?: ConversationMenuEntry[] }): React.JSX.Element {
  const [page, setPage] = useState<'root' | 'ping'>('root')
  const prefs = usePingPrefs()
  // the conversation's own setting; none means the default from the Chat settings tab applies
  const override = prefs.channels[channel]
  const items = (close: () => void): ContextMenuItem[] =>
    page === 'root'
      ? [
          ...entries.map((e): ContextMenuItem => ({ label: e.label, danger: e.danger, onClick: () => { close(); e.onClick() } })),
          { kind: 'submenu', label: PING_LABEL, onClick: () => setPage('ping') }
        ]
      : [
          { kind: 'title', label: PING_LABEL },
          { kind: 'option', label: 'Default', checked: override == null, onClick: () => { clearPingMode(channel); close() } },
          ...PING_MODES.map((m): ContextMenuItem => ({ kind: 'option', label: m.label, checked: override === m.mode, onClick: () => { setPingMode(channel, m.mode); close() } })),
          { kind: 'separator' },
          { label: 'Back', onClick: () => setPage('root') }
        ]
  return <KebabMenu label="Conversation options" items={items} onClosed={() => setPage('root')} />
}
