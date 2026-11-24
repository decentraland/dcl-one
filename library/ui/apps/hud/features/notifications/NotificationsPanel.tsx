// React notifications panel — the list the bridge scene fetches on world entry and on open, plus
// the new ones its poll pushes (`notificationsNew`). Title/body/image come from `summary.ts`.
// Mark-all-read clears the badge and persists through the bridge's signed PUT.

import { ControlButton } from '../../design'
import type { AppNotification } from '@explorer-core/engine/protocol'
import type { ChatState, FriendsState, NotificationsState, VoiceChatSession } from '../session/useEngineSession'
import { ago, communityTarget, friendshipTarget, giftTarget, referralTarget, tipTarget, streamTarget, summarize } from './summary'
import styles from './NotificationsPanel.module.css'

// The kind badge on the thumbnail's corner, marking each notification: a
// rosette for badges and rewards, a person for friendships and communities, a bell for the rest.
function KindBadge({ type }: { type: string }): React.JSX.Element {
  const kind = /badge|reward|credit/.test(type) ? 'award' : /friend|communit|tip|referral/.test(type) ? 'person' : 'bell'
  return (
    <span className={styles.kind} aria-hidden="true">
      <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
        {kind === 'award' && (
          <>
            <circle cx="7" cy="5.2" r="3.4" />
            <path d="M4.9 8.2 4 12.6l3-1.5 3 1.5-.9-4.4" />
          </>
        )}
        {kind === 'person' && (
          <>
            <circle cx="7" cy="4.6" r="2.4" />
            <path d="M2.4 12.2c.5-2.4 2.3-3.7 4.6-3.7s4.1 1.3 4.6 3.7" />
          </>
        )}
        {kind === 'bell' && <path d="M3.4 9.8V6.3a3.6 3.6 0 0 1 7.2 0v3.5l1 1.2H2.4l1-1.2zM5.9 12.6h2.2" />}
      </svg>
    </span>
  )
}

function NotificationRow({ n, friends, voiceChat, chat }: { n: AppNotification; friends?: FriendsState; voiceChat?: VoiceChatSession; chat?: ChatState }): React.JSX.Element {
  const { title, body, image } = summarize(n)
  const onOpen = friendshipTarget(n, friends) ?? tipTarget(n) ?? giftTarget(n) ?? referralTarget(n) ?? streamTarget(n, voiceChat, chat) ?? communityTarget(n)
  return (
    <div
      className={`${styles.row} ${onOpen ? styles.clickable : ''}`.trim()}
      role={onOpen ? 'button' : undefined}
      tabIndex={onOpen ? 0 : undefined}
      onClick={onOpen}
      onKeyDown={onOpen ? (e) => { if (e.key === 'Enter') onOpen() } : undefined}
    >
      <div className={styles.thumb}>
        <span className={styles.thumbFrame}>{image ? <img className={styles.img} src={image} alt="" /> : <span className={styles.dot} />}</span>
        <KindBadge type={n.type} />
      </div>
      <div className={styles.info}>
        <span className={styles.title}>{title}</span>
        {body && <span className={styles.bodyText}>{body}</span>}
        <span className={styles.time}>{ago(n.timestamp)}</span>
      </div>
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
