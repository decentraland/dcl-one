// A community chat's title bar: the community (its thumbnail and name), how many of its members
// are online (opens the members list), auto-translation, and the conversation menu: View
// Community, Notification Ping, Delete chat history, Close.

import type { CommunityConversation } from '../session/useEngineSession'
import { Avatar, ControlButton, MaskIcon } from '@hud/design'
import { openCommunityModal } from '../communities/CommunityModal'
import { AutoTranslateButton } from './AutoTranslateButton'
import { ConversationMenu } from './ConversationMenu'
import playersIcon from '@hud/assets/chat/players.png'
import styles from '@hud/features/chat/Chat.module.css'

export function CommunityHeader({
  community,
  showMembers,
  onToggleMembers,
  onDelete,
  onClose
}: {
  community: CommunityConversation
  showMembers: boolean
  onToggleMembers: () => void
  /** Delete the stored history of this community's chat. */
  onDelete: () => void
  /** Close this community's chat (hide its tab). */
  onClose: () => void
}): React.JSX.Element {
  return (
    <header className={styles.nav}>
      <div className={styles.navLeft}>
        <Avatar src={community.thumbnail || undefined} name={community.name} size={28} framed className={styles.channelIcon} />
        <span className={styles.navTitle} title={community.name}>{community.name}</span>
      </div>
      <div className={styles.navRight}>
        <AutoTranslateButton channel={community.key} />
        <ControlButton
          variant="faint"
          shape="pill"
          className={styles.membersBtn}
          active={showMembers}
          aria-label={`${community.online} online`}
          onClick={onToggleMembers}
        >
          <MaskIcon src={playersIcon} size={18} />
          {community.online}
        </ControlButton>
        <ConversationMenu
          channel={community.key}
          entries={[
            { label: 'View Community', onClick: () => openCommunityModal(community.id, () => undefined) },
            { label: 'Delete chat history', danger: true, onClick: onDelete },
            { label: 'Close conversation', onClick: onClose }
          ]}
        />
      </div>
    </header>
  )
}
