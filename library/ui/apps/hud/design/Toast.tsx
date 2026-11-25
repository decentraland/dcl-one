// Toast — a transient notice at the top of the screen (friend blocked, "X is online", a failed
// action). Imperative like the popups: showToast() from anywhere, <ToastHost/> mounted once. Unlike a
// popup it holds no input focus and never counts as an open popup.
//
// A keyed toast is one notice that changes: showing it again under the same key replaces it in
// place (a progress count), and dismissToast(key) takes it down. One can stay until dismissed
// (`duration: null`), offer one action and a Dismiss button; only those toasts take pointer input.

import { useSyncExternalStore, type ReactNode } from 'react'
import { Button } from './Button'
import { ControlButton } from './ControlButton'
import { Close } from './Glyphs'
import styles from './Toast.module.css'

export interface ToastAction {
  label: string
  onClick: () => void
  /** Accessible name when the label alone is ambiguous out of context (default: the label). */
  ariaLabel?: string
}

export interface ToastOptions {
  /** Leading visual, e.g. an Avatar. */
  icon?: ReactNode
  tone?: 'default' | 'error'
  /** ms before it goes away (default 5000); null keeps it until it is dismissed or replaced. */
  duration?: number | null
  /** Showing a toast under a key already shown replaces that toast in place. */
  key?: string
  /** One button after the text; clicking it dismisses the toast and runs `onClick`. */
  action?: ToastAction
  /** A Dismiss button. */
  dismissible?: boolean
  /** The text is still changing (a progress count): assistive tech waits for the settled text. */
  busy?: boolean
}

interface ToastItem extends ToastOptions {
  id: number
  content: ReactNode
}

let items: ToastItem[] = []
let nextId = 0
const timers = new Map<number, ReturnType<typeof setTimeout>>()
const listeners = new Set<() => void>()
const emit = (): void => listeners.forEach((l) => l())

function remove(id: number): void {
  const timer = timers.get(id)
  if (timer != null) clearTimeout(timer)
  timers.delete(id)
  const next = items.filter((t) => t.id !== id)
  if (next.length === items.length) return
  items = next
  emit()
}

export function showToast(content: ReactNode, options: ToastOptions = {}): void {
  const existing = options.key != null ? items.find((t) => t.key === options.key) : undefined
  const id = existing?.id ?? ++nextId
  const timer = timers.get(id)
  if (timer != null) clearTimeout(timer)
  timers.delete(id)
  const item: ToastItem = { id, content, ...options }
  items = existing != null ? items.map((t) => (t.id === id ? item : t)) : [...items, item]
  emit()
  const duration = options.duration === undefined ? 5000 : options.duration
  if (duration != null) timers.set(id, setTimeout(() => remove(id), duration))
}

/** Take down the toast shown under `key`, if any. */
export function dismissToast(key: string): void {
  const item = items.find((t) => t.key === key)
  if (item != null) remove(item.id)
}

const subscribe = (cb: () => void): (() => void) => {
  listeners.add(cb)
  return () => listeners.delete(cb)
}
const getSnapshot = (): ToastItem[] => items

export function ToastHost(): React.JSX.Element {
  const list = useSyncExternalStore(subscribe, getSnapshot)
  return (
    <div className={styles.host} aria-live="polite">
      {list.map((t) => {
        const interactive = t.action != null || t.dismissible === true
        const cls = [styles.toast, t.tone === 'error' ? styles.error : '', interactive ? styles.interactive : ''].filter(Boolean).join(' ')
        return (
          <div key={t.id} className={cls} role="status" aria-busy={t.busy === true ? true : undefined}>
            {t.icon}
            <span className={styles.text}>{t.content}</span>
            {t.action != null && (
              <Button
                variant="secondary"
                size="sm"
                aria-label={t.action.ariaLabel}
                onClick={() => {
                  remove(t.id)
                  t.action?.onClick()
                }}
              >
                {t.action.label}
              </Button>
            )}
            {t.dismissible === true && (
              <ControlButton size="sm" shape="circle" aria-label="Dismiss" aria-pressed={undefined} onClick={() => remove(t.id)}>
                <Close size={14} />
              </ControlButton>
            )}
          </div>
        )
      })}
    </div>
  )
}
