// The conversation options menu on a chat title bar: the
// "Notification Ping" submenu every conversation has (Default / All Messages / Mentions Only /
// None), which only decides whether a line landing there sounds; the auto-translate switch; and
// the caller's entries under a rule. The menu opens over the title bar, from the kebab's left,
// in a layer of its own: the chat panel clips what it paints.

import { createContext, useCallback, useContext, useEffect, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
import { registerCancelLayer } from '@explorer-core/lib/cancelLayers'
import { autoTranslates, setAutoTranslate } from '@explorer-core/features/chat/translation'
import { GeneralLeftArrow, MenusDelete, MenusNotifications, MenusTranslate } from '@dcl/one-ui'
import { ContextMenu, type ContextMenuItem } from '../../design'
import { pressedWith, releasePointerFocus } from '../../lib/pressedWith'
import { KebabDots } from './ChatIcons'
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

/** The width of the chat's menus, as the design draws the conversation menu. */
export const CHAT_MENU_WIDTH = 260

/** Marks the menu's layer, so the chat knows that focus moving into it has not left the chat. */
export const MENU_LAYER = 'data-chat-menu'

/** Told whether one of the chat's menus is open: an open menu keeps the chat active, though one the
 *  pointer opened holds no focus. */
export const ChatMenuOpen = createContext<(open: boolean) => void>(() => {})

/** A menu anchored to the button that opens it, painted in its own layer (the chat panel clips
 *  what it paints). `anchor` places the menu from the button's rectangle and the HUD's scale; the
 *  open menu follows the button when the window or the HUD's scale changes. A press outside or
 *  Escape closes it. Opened from the keyboard (Enter or Space, a click with `detail` 0), the focus
 *  moves into the menu and closing from inside gives it back to the button; opened with the
 *  pointer, neither the button nor the menu keeps the focus, so a later key press (the game's)
 *  rings nothing. */
export function useAnchoredMenu(anchor: (button: DOMRect, scale: number) => { left: number; top: number }, onDismiss?: () => void): {
  open: boolean
  at: { left: number; top: number } | null
  button: React.RefObject<HTMLButtonElement | null>
  menu: React.RefObject<HTMLDivElement | null>
  toggle: (event: React.MouseEvent<HTMLElement>) => void
  close: () => void
  /** Whether the keyboard opened the menu. */
  byKeyboard: React.RefObject<boolean>
} {
  const [open, setOpen] = useState(false)
  const [at, setAt] = useState<{ left: number; top: number } | null>(null)
  const button = useRef<HTMLButtonElement>(null)
  const menu = useRef<HTMLDivElement>(null)
  const byKeyboard = useRef(false)
  const reportOpen = useContext(ChatMenuOpen)
  const anchorRef = useRef(anchor)
  anchorRef.current = anchor
  const dismissRef = useRef(onDismiss)
  dismissRef.current = onDismiss
  const dismiss = useCallback(() => {
    setOpen(false)
    dismissRef.current?.()
  }, [])
  const close = useCallback(() => {
    if (byKeyboard.current && menu.current?.contains(document.activeElement)) button.current?.focus()
    dismiss()
  }, [dismiss])
  const place = useCallback(() => {
    const r = button.current?.getBoundingClientRect()
    setAt(r != null ? anchorRef.current(r, getHudScale()) : { left: 0, top: 0 })
  }, [])
  const toggle = (event: React.MouseEvent<HTMLElement>): void => {
    releasePointerFocus(event)
    if (open) return close()
    byKeyboard.current = pressedWith(event) === 'keyboard'
    place()
    setOpen(true)
  }
  useEffect(() => {
    if (!open) return
    window.addEventListener('resize', place)
    const off = subscribeHudScale(place)
    return () => {
      window.removeEventListener('resize', place)
      off()
    }
  }, [open, place])
  useEffect(() => {
    if (!open) return
    reportOpen(true)
    return () => reportOpen(false)
  }, [open, reportOpen])
  // keyboard focus moves into the menu when it opens
  useEffect(() => {
    if (!open || !byKeyboard.current) return
    menu.current?.querySelector<HTMLElement>('[role^="menuitem"]')?.focus()
  }, [open])
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
  return { open, at, button, menu, toggle, close, byKeyboard }
}

// 46px left of the kebab and 13px above it, in the HUD's scaled pixels
const besideKebab = (r: DOMRect, scale: number): { left: number; top: number } => ({ left: r.left - 45 * scale, top: r.top - 14 * scale })

export function ConversationMenu({ channel, entries = [] }: { channel: string; entries?: ConversationMenuEntry[] }): React.JSX.Element {
  const [page, setPage] = useState<'root' | 'ping'>('root')
  const prefs = usePingPrefs()
  const translating = autoTranslates(useTranslationPrefs(), channel)
  const { open, at, button, menu, toggle, close, byKeyboard } = useAnchoredMenu(besideKebab, () => setPage('root'))
  // keyboard focus moves into the menu when its page changes
  useEffect(() => {
    if (!open || !byKeyboard.current) return
    menu.current?.querySelector<HTMLElement>('[role^="menuitem"]')?.focus()
  }, [open, page, menu, byKeyboard])
  // the conversation's own setting; none means the default from the Chat settings tab applies
  const override = prefs.channels[channel]
  const items: ContextMenuItem[] =
    page === 'root'
      ? [
          { kind: 'submenu', label: PING_LABEL, icon: <MenusNotifications size={20} />, onClick: () => setPage('ping') },
          { kind: 'toggle', label: AUTO_TRANSLATE_LABEL, icon: <MenusTranslate size={20} />, checked: translating, onChange: (on) => setAutoTranslate(channel, on) },
          ...(entries.length > 0 ? [{ kind: 'separator' } as ContextMenuItem] : []),
          ...entries.map((e): ContextMenuItem => ({ label: e.label, danger: e.danger, icon: e.danger ? <MenusDelete size={20} /> : undefined, onClick: () => { close(); e.onClick() } }))
        ]
      : [
          { kind: 'title', label: PING_LABEL },
          { kind: 'option', label: 'Default', checked: override == null, onClick: () => { clearPingMode(channel); close() } },
          ...PING_MODES.map((m): ContextMenuItem => ({ kind: 'option', label: m.label, checked: override === m.mode, onClick: () => { setPingMode(channel, m.mode); close() } })),
          { kind: 'separator' },
          // an icon, as every row of the design has, keeps its label in line with the options'
          { label: 'Back', icon: <GeneralLeftArrow size={20} />, onClick: () => setPage('root') }
        ]
  return (
    <>
      <button ref={button} type="button" className={styles.kebab} aria-label="Conversation options" aria-haspopup="menu" aria-expanded={open} onClick={toggle}>
        <KebabDots />
      </button>
      {open && at != null && createPortal(
        <div ref={menu} className={styles.menuLayer} style={at} {...{ [MENU_LAYER]: '' }}>
          <ContextMenu items={items} width={CHAT_MENU_WIDTH} />
        </div>,
        document.body
      )}
    </>
  )
}
