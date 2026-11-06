// Notifications: the notifications list, persisting "mark as read", and a poll for new ones.
//   from: notifications service via BevyApi.kernelFetch (signed GET list / PUT read).
//   The service has no push channel, so like unity-explorer this scene polls every 5 s for unread
//   notifications newer than the newest it has seen, and pushes them as `notificationsNew`.
import { getPlayer } from '@dcl/sdk/players'
import { serviceBase, signed } from '../http'
import { identity } from '../identity'
import type { Ctx } from '../bridge'
import type { AppNotification } from '../../../src/engine/protocol'
import { notificationTimeMs } from '../../../src/lib/notificationTime'

/** unity-explorer's NotificationsRequestController polls at the same rate. */
export const POLL_INTERVAL_S = 5
/** A failing service is retried with doubling waits up to this cap, not every 5 s. */
export const POLL_BACKOFF_MAX_S = 60

type NotificationRaw = {
  id: string
  type: string
  timestamp: string
  read: boolean
  metadata?: Record<string, unknown>
}

async function base(): Promise<string> {
  return `${await serviceBase('notifications')}/notifications`
}

function toApp(n: NotificationRaw): AppNotification {
  return { id: n.id, type: n.type, timestamp: n.timestamp, read: n.read, metadata: n.metadata ?? {} }
}

async function fetchList(query: string): Promise<AppNotification[]> {
  const res = await signed<{ notifications?: NotificationRaw[] }>(`${await base()}?${query}`)
  return (res?.notifications ?? []).map(toApp)
}

export function registerNotifications(ctx: Ctx): void {
  // `from` is the server's own `occurred_at > from` bound, so the cursor is the newest timestamp
  // the server has sent us, never the local clock. Polling starts once the page has fetched the
  // list (world entry), so the first poll never re-announces what the panel already holds. If
  // that fetch fails, polling starts anyway with no cursor.
  let armed = false
  let cursor: number | undefined
  let inFlight = false
  let waitS = POLL_INTERVAL_S
  let elapsedS = 0
  // Bumped on every account switch or disarm: an answer to a request sent under an older
  // generation belongs to the previous account and is dropped.
  let generation = 0

  const advance = (list: AppNotification[]): void => {
    for (const n of list) {
      const t = notificationTimeMs(n.timestamp)
      if (t != null && (cursor == null || t > cursor)) cursor = t
    }
  }

  const disarm = (): void => {
    generation++
    armed = false
    cursor = undefined
    waitS = POLL_INTERVAL_S
    elapsedS = 0
  }

  identity.onChange(disarm)

  ctx.on('getNotifications', async () => {
    const gen = generation
    let notifications: AppNotification[]
    try {
      notifications = await fetchList('limit=50&from=0')
    } catch (e) {
      // Poll anyway, from no cursor: the backed-off poll keeps retrying, so a failed fetch at
      // world entry doesn't leave toasts off for the session.
      if (gen === generation) armed = true
      throw e
    }
    if (gen !== generation) return
    advance(notifications)
    armed = true
    ctx.send({ kind: 'notifications', notifications })
  })

  ctx.on('markNotificationsRead', (msg) => {
    if (msg.ids.length === 0) return
    void (async () => {
      await signed(`${await base()}/read`, 'PUT', { notificationIds: msg.ids })
    })().catch((e: unknown) => {
      console.error('[notifications] markRead failed', e)
    })
  })

  const poll = async (gen: number): Promise<void> => {
    const query = `onlyUnread=true&limit=50${cursor != null ? `&from=${cursor}` : ''}`
    const fresh = (await fetchList(query)).filter((n) => !n.read)
    // An account switch while the request was out: its answer belongs to the old account.
    if (gen !== generation || !armed) return
    advance(fresh)
    if (fresh.length > 0) ctx.send({ kind: 'notificationsNew', notifications: fresh })
  }

  ctx.push((dt) => {
    const player = getPlayer()
    if (player == null || player.isGuest) {
      // Logged out (the engine clears the player) or a guest, who has no notifications.
      if (armed) disarm()
      return
    }
    if (!armed || inFlight) return
    elapsedS += dt
    if (elapsedS < waitS) return
    elapsedS = 0
    inFlight = true
    const gen = generation
    poll(gen)
      .then(() => {
        if (gen === generation) waitS = POLL_INTERVAL_S
      })
      .catch((e: unknown) => {
        if (gen === generation) waitS = Math.min(waitS * 2, POLL_BACKOFF_MAX_S)
        console.error('[notifications] poll failed', e)
      })
      .finally(() => {
        inFlight = false
      })
  })
}
