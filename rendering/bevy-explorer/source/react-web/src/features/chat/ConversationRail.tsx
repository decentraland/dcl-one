// The conversation rail beside the chat: the chat's close button, Nearby pinned first, then the
// open DM tabs, then the open community chats. Each tab carries its unread count; a community's
// also whether one of those lines @-mentions the local user.

import type { ChatState, CommunityConversation, Conversation } from '../session/useEngineSession'
import { Avatar, ControlButton, DclLogo, MaskIcon, Tooltip } from '../../design'
import { splitName, usePartner } from './chatIdentity'
import closeIcon from '../../assets/chat/close-thin.png'
import styles from './Chat.module.css'

function badge(count: number): React.JSX.Element | null {
  return count > 0 ? <span className={styles.railBadge}>{count > 9 ? '9+' : count}</span> : null
}

/** One DM tab on the rail: face, online dot, unread count; faded while the partner is unavailable. */
function RailTab({ conversation, active, onSelect, onClose }: { conversation: Conversation; active: boolean; onSelect: () => void; onClose: () => void }): React.JSX.Element {
  const { name, picture, color } = usePartner(conversation.address)
  const { base } = splitName(name)
  return (
    <Tooltip label={base} side="left" variant="rail">
      <div className={`${styles.railTab} ${active ? styles.railActive : ''} ${conversation.online ? '' : styles.railOff}`.trim()}>
        <button type="button" className={styles.railBtn} aria-label={`Chat with ${base}`} aria-pressed={active} onClick={onSelect}>
          <Avatar src={picture} name={name} color={color} size={32} status={conversation.online ? 'online' : 'offline'} dotPosition="top" />
          {badge(conversation.unread)}
        </button>
        <button type="button" className={styles.railClose} aria-label={`Close chat with ${base}`} onClick={onClose}>
          <MaskIcon src={closeIcon} size={8} />
        </button>
      </div>
    </Tooltip>
  )
}

/** One community chat on the rail: its thumbnail (the initial while it has none), unread count and
 *  mention mark. */
function CommunityRailTab({ community, active, onSelect, onClose }: { community: CommunityConversation; active: boolean; onSelect: () => void; onClose: () => void }): React.JSX.Element {
  return (
    <Tooltip label={community.name} side="left" variant="rail">
      <div className={`${styles.railTab} ${active ? styles.railActive : ''}`.trim()}>
        <button type="button" className={styles.railBtn} aria-label={`Chat in ${community.name}`} aria-pressed={active} onClick={onSelect}>
          <Avatar src={community.thumbnail || undefined} name={community.name} size={32} />
          {community.mention && <span className={styles.railMention} role="img" aria-label="Mentions you" />}
          {badge(community.unread)}
        </button>
        <button type="button" className={styles.railClose} aria-label={`Close chat in ${community.name}`} onClick={onClose}>
          <MaskIcon src={closeIcon} size={8} />
        </button>
      </div>
    </Tooltip>
  )
}

export function ConversationRail({ chat, onClose }: { chat: ChatState; onClose: () => void }): React.JSX.Element {
  return (
    <nav className={styles.rail} aria-label="Conversations">
      <div className={styles.railHead}>
        <ControlButton variant="dark" aria-label="Close chat" onClick={onClose}>
          <MaskIcon src={closeIcon} size={10} />
        </ControlButton>
      </div>
      <Tooltip label="Nearby" side="left" variant="rail">
        <button
          type="button"
          className={`${styles.railBtn} ${styles.railNearby} ${chat.channel === 'Nearby' ? styles.railActive : ''}`.trim()}
          aria-label="Nearby chat"
          aria-pressed={chat.channel === 'Nearby'}
          onClick={() => chat.select('Nearby')}
        >
          <DclLogo size={28} />
          {badge(chat.nearbyUnread)}
        </button>
      </Tooltip>
      {chat.conversations.map((c) => (
        <RailTab key={c.address} conversation={c} active={chat.channel === c.address} onSelect={() => chat.select(c.address)} onClose={() => chat.closeConversation(c.address)} />
      ))}
      {chat.communities.map((c) => (
        <CommunityRailTab key={c.id} community={c} active={chat.channel === c.key} onSelect={() => chat.select(c.key)} onClose={() => chat.closeCommunity(c.id)} />
      ))}
    </nav>
  )
}
