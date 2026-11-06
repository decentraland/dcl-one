// New-notification toast: the notifications the bridge's poll reports, shown one at a time for
// TOAST_MS each in arrival order, like unity-explorer's NewNotificationController. Imperative like
// design/Toast: queueNotificationToasts() from the session, <NotificationToastHost/> mounted once.
// Clicking a toast opens where it leads (a friend request, a passport) or the notifications panel.

import { useSyncExternalStore } from 'react'
import type { AppNotification } from '../../engine/protocol'
import type { FriendsState, NotificationsState } from '../session/useEngineSession'
import { friendshipTarget, summarize } from './summary'
import styles from './NotificationToast.module.css'

export const TOAST_MS = 5000

let queue: AppNotification[] = []
let current: AppNotification | null = null
let timer: ReturnType<typeof setTimeout> | undefined
const listeners = new Set<() => void>()
const emit = (): void => listeners.forEach((l) => l())

function showNext(): void {
  clearTimeout(timer)
  current = queue[0] ?? null
  queue = queue.slice(1)
  if (current != null) timer = setTimeout(showNext, TOAST_MS)
  emit()
}

/** Queue toasts in the order given (oldest first). */
export function queueNotificationToasts(list: AppNotification[]): void {
  if (list.length === 0) return
  queue = [...queue, ...list]
  if (current == null) showNext()
  else emit()
}

/** Drop the toast on screen and everything waiting (logout). */
export function clearNotificationToasts(): void {
  clearTimeout(timer)
  queue = []
  current = null
  emit()
}

const subscribe = (cb: () => void): (() => void) => {
  listeners.add(cb)
  return () => listeners.delete(cb)
}
const getSnapshot = (): AppNotification | null => current

export function NotificationToastHost({
  notifications,
  friends
}: {
  notifications: NotificationsState
  friends?: FriendsState
}): React.JSX.Element | null {
  const n = useSyncExternalStore(subscribe, getSnapshot)
  if (n == null) return null
  const { title, body, image } = summarize(n)
  const target = friendshipTarget(n, friends)
  const open = (): void => {
    showNext()
    if (target != null) target()
    else if (!notifications.open) notifications.toggle()
  }
  return (
    <div className={styles.host} aria-live="polite">
      <button
        key={n.id}
        type="button"
        className={styles.toast}
        aria-label={body != null ? `${title}. ${body}` : title}
        onClick={open}
      >
        <span className={styles.thumb}>{image ? <img className={styles.img} src={image} alt="" /> : <span className={styles.dot} />}</span>
        <span className={styles.info}>
          <span className={styles.title}>{title}</span>
          {body && <span className={styles.body}>{body}</span>}
        </span>
      </button>
    </div>
  )
}
