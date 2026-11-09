// The conversation options menu on a chat title bar: the caller's entries, then the
// "Notification Ping" submenu every conversation has (Default / All Messages / Mentions Only /
// None), which only decides whether a line landing there sounds.

import { useEffect, useRef, useState } from 'react'
import { ContextMenu, ControlButton, Kebab, type ContextMenuItem } from '../../design'
import { registerCancelLayer } from '../../lib/cancelLayers'
import { PING_MODES, clearPingMode, setPingMode, usePingPrefs } from './ping'
import styles from './Chat.module.css'

export const PING_LABEL = 'Notification Ping'

/** The menu's own entries: label, optional danger, and what a click does (the menu closes first). */
export interface ConversationMenuEntry {
  label: string
  danger?: boolean
  onClick: () => void
}

export function ConversationMenu({ channel, entries = [] }: { channel: string; entries?: ConversationMenuEntry[] }): React.JSX.Element {
  const [menu, setMenu] = useState(false)
  const [page, setPage] = useState<'root' | 'ping'>('root')
  const menuRef = useRef<HTMLDivElement>(null)
  const prefs = usePingPrefs()
  // the conversation's own setting; none means the default from the Chat settings tab applies
  const override = prefs.channels[channel]
  const close = (): void => {
    setMenu(false)
    setPage('root')
  }
  useEffect(() => {
    if (!menu) return
    const onDown = (e: MouseEvent): void => {
      if (menuRef.current != null && !menuRef.current.contains(e.target as Node)) close()
    }
    document.addEventListener('mousedown', onDown)
    const off = registerCancelLayer(close)
    return () => {
      document.removeEventListener('mousedown', onDown)
      off()
    }
  }, [menu])
  const items: ContextMenuItem[] =
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
  return (
    <div ref={menuRef} className={styles.menuWrap}>
      <ControlButton variant="faint" aria-label="Conversation options" aria-expanded={menu} active={menu} onClick={() => (menu ? close() : setMenu(true))}>
        <Kebab vertical size={18} r={2} />
      </ControlButton>
      {menu && (
        <div className={styles.menu}>
          <ContextMenu items={items} />
        </div>
      )}
    </div>
  )
}
