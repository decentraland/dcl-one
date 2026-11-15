// The conversation options menu on a chat title bar: the
// "Notification Ping" submenu every conversation has (Default / All Messages / Mentions Only /
// None), which only decides whether a line landing there sounds; the auto-translate switch; and
// the caller's entries under a rule. The menu opens over the title bar, from the kebab's left,
// in a layer of its own: the chat panel clips what it paints.

import { useCallback, useEffect, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
import { registerCancelLayer } from '@explorer-core/lib/cancelLayers'
import { autoTranslates, setAutoTranslate } from '@explorer-core/features/chat/translation'
import { ContextMenu, type ContextMenuItem } from '../../design'
import { BellIcon, KebabDots, TranslateIcon, TrashOutlineIcon } from './ChatIcons'
import { PING_MODES, clearPingMode, setPingMode, usePingPrefs } from './ping'
import { useTranslationPrefs } from './useTranslation'
import { getHudScale, subscribeHudScale } from '@explorer-core/lib/hudScale'
import styles from './Chat.module.css'

export const PING_LABEL = 'Notification Ping'

/** The menu's own entries: label, optional danger, and what a click does (the menu closes first). */
export interface ConversationMenuEntry {
  label: string
  danger?: boolean
  onClick: () => void
}

export const AUTO_TRANSLATE_LABEL = 'Auto-Translate'

/** Marks the menu's layer, so the chat knows that focus moving into it has not left the chat. */
export const MENU_LAYER = 'data-chat-menu'

export function ConversationMenu({ channel, entries = [] }: { channel: string; entries?: ConversationMenuEntry[] }): React.JSX.Element {
  const [open, setOpen] = useState(false)
  const [at, setAt] = useState<{ left: number; top: number } | null>(null)
  const [page, setPage] = useState<'root' | 'ping'>('root')
  const prefs = usePingPrefs()
  const translating = autoTranslates(useTranslationPrefs(), channel)
  const button = useRef<HTMLButtonElement>(null)
  const menu = useRef<HTMLDivElement>(null)
  const dismiss = useCallback(() => {
    setOpen(false)
    setPage('root')
  }, [])
  // closed from inside (a row, Escape): focus goes back to the button the menu was opened from
  const close = useCallback(() => {
    if (menu.current?.contains(document.activeElement)) button.current?.focus()
    dismiss()
  }, [dismiss])
  // 46px left of the kebab and 13px above it, in the HUD's scaled pixels
  const place = useCallback(() => {
    const r = button.current?.getBoundingClientRect()
    const scale = getHudScale()
    setAt(r != null ? { left: r.left - 45 * scale, top: r.top - 14 * scale } : { left: 0, top: 0 })
  }, [])
  const toggle = (): void => {
    if (open) return close()
    place()
    setOpen(true)
  }
  // the kebab moves when the window or the HUD's scale changes: the open menu follows it
  useEffect(() => {
    if (!open) return
    window.addEventListener('resize', place)
    const off = subscribeHudScale(place)
    return () => {
      window.removeEventListener('resize', place)
      off()
    }
  }, [open, place])
  // keyboard focus moves into the menu when it opens and when its page changes
  useEffect(() => {
    if (!open) return
    menu.current?.querySelector<HTMLElement>('[role^="menuitem"]')?.focus()
  }, [open, page])
  useEffect(() => {
    if (!open) return
    const onDown = (e: PointerEvent): void => {
      const t = e.target as Node
      if (button.current?.contains(t) || menu.current?.contains(t)) return
      // a press elsewhere takes the focus with it
      dismiss()
    }
    document.addEventListener('pointerdown', onDown, true)
    const off = registerCancelLayer(close)
    return () => {
      document.removeEventListener('pointerdown', onDown, true)
      off()
    }
  }, [open, close, dismiss])
  // the conversation's own setting; none means the default from the Chat settings tab applies
  const override = prefs.channels[channel]
  const items: ContextMenuItem[] =
    page === 'root'
      ? [
          { kind: 'submenu', label: PING_LABEL, icon: <BellIcon />, onClick: () => setPage('ping') },
          { kind: 'toggle', label: AUTO_TRANSLATE_LABEL, icon: <TranslateIcon />, checked: translating, onChange: (on) => setAutoTranslate(channel, on) },
          ...(entries.length > 0 ? [{ kind: 'separator' } as ContextMenuItem] : []),
          ...entries.map((e): ContextMenuItem => ({ label: e.label, danger: e.danger, icon: e.danger ? <TrashOutlineIcon /> : undefined, onClick: () => { close(); e.onClick() } }))
        ]
      : [
          { kind: 'title', label: PING_LABEL },
          { kind: 'option', label: 'Default', checked: override == null, onClick: () => { clearPingMode(channel); close() } },
          ...PING_MODES.map((m): ContextMenuItem => ({ kind: 'option', label: m.label, checked: override === m.mode, onClick: () => { setPingMode(channel, m.mode); close() } })),
          { kind: 'separator' },
          { label: 'Back', onClick: () => setPage('root') }
        ]
  return (
    <>
      <button ref={button} type="button" className={styles.kebab} aria-label="Conversation options" aria-haspopup="menu" aria-expanded={open} onClick={toggle}>
        <KebabDots />
      </button>
      {open && at != null && createPortal(
        <div ref={menu} className={styles.menuLayer} style={at} {...{ [MENU_LAYER]: '' }}>
          <ContextMenu items={items} variant="sheet" />
        </div>,
        document.body
      )}
    </>
  )
}
