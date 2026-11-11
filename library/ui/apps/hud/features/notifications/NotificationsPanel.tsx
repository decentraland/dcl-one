// React notifications panel — the list the bridge scene fetches on world entry and on open, plus
// the new ones its poll pushes (`notificationsNew`). Title/body/image come from `summary.ts`.
// Mark-all-read clears the badge and persists through the bridge's signed PUT.

import { ControlButton } from '../../design'
import type { AppNotification } from '@explorer-core/engine/protocol'
import type { ChatState, FriendsState, NotificationsState, VoiceChatSession } from '../session/useEngineSession'
import { ago, friendshipTarget, streamTarget, summarize } from './summary'
import styles from './NotificationsPanel.module.css'

function NotificationRow({ n, friends, voiceChat, chat }: { n: AppNotification; friends?: FriendsState; voiceChat?: VoiceChatSession; chat?: ChatState }): React.JSX.Element {
  const { title, body, image } = summarize(n)
  const onOpen = friendshipTarget(n, friends) ?? streamTarget(n, voiceChat, chat)
  return (
    <div
      className={`${styles.row} ${n.read ? '' : styles.unread} ${onOpen ? styles.clickable : ''}`.trim()}
      role={onOpen ? 'button' : undefined}
      tabIndex={onOpen ? 0 : undefined}
      onClick={onOpen}
      onKeyDown={onOpen ? (e) => { if (e.key === 'Enter') onOpen() } : undefined}
    >
      <div className={styles.thumb}>
        {image ? <img className={styles.img} src={image} alt="" /> : <span className={styles.dot} />}
      </div>
      <div className={styles.info}>
        <span className={styles.title}>{title}</span>
        {body && <span className={styles.bodyText}>{body}</span>}
      </div>
      <span className={styles.time}>{ago(n.timestamp)}</span>
      {!n.read && <span className={styles.unreadDot} aria-label="unread" />}
    </div>
  )
}

export function NotificationsPanel({
  notifications,
  friends,
  voiceChat,
  chat,
  anchor = 'rail'
}: {
  notifications: NotificationsState
  /** Docked by the sidebar in-world; dropped under the lobby's bell (left of a Close in-world). */
  anchor?: 'rail' | 'lobby' | 'lobbyInWorld'
  /** Makes friendship notifications open the request, the friends panel or the passport. */
  friends?: FriendsState
  /** Makes a community stream notification join the stream. */
  voiceChat?: VoiceChatSession
  /** A stream notification opens the community's chat. */
  chat?: ChatState
}): React.JSX.Element | null {
  if (!notifications.open) return null
  return (
    <div className={`${styles.root} ${anchor === 'rail' ? '' : styles[anchor]}`.trim()}>
      <header className={styles.head}>
        <span className={styles.heading}>Notifications</span>
        {notifications.unread > 0 && (
          <button type="button" className={styles.markRead} onClick={notifications.markAllRead}>
            Mark all read
          </button>
        )}
        <ControlButton variant="solid" className={styles.closeGlyph} aria-label="Close notifications" onClick={notifications.toggle}>
          ×
        </ControlButton>
      </header>
      <div className={styles.body}>
        {notifications.list.length === 0 ? (
          <div className={styles.empty}>You’re all caught up.</div>
        ) : (
          notifications.list.map((n) => <NotificationRow key={n.id} n={n} friends={friends} voiceChat={voiceChat} chat={chat} />)
        )}
      </div>
    </div>
  )
}
