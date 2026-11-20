// A community chat's title bar: the community (its thumbnail and name, which open a one-item menu
// with View Community, as unity-explorer's title bar does), how many of its members are online
// (opens the members list), and the conversation menu: Notification Ping, Auto-Translate, View
// Community, Delete Chat History, Close.

import { createPortal } from 'react-dom'
import type { CommunityConversation } from '../session/useEngineSession'
import { Avatar, ContextMenu, ControlButton, MaskIcon } from '../../design'
import { openCommunityModal } from '../communities/CommunityModal'
import { ConversationMenu, MENU_LAYER, useAnchoredMenu } from './ConversationMenu'
import playersIcon from '../../assets/chat/players.png'
import styles from './Chat.module.css'

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
      <CommunityTitle community={community} />
      <div className={styles.navRight}>
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
            { label: VIEW_COMMUNITY, onClick: () => openCommunityModal(community.id, () => undefined) },
            { label: 'Delete Chat History', danger: true, onClick: onDelete },
            { label: 'Close conversation', onClick: onClose }
          ]}
        />
      </div>
    </header>
  )
}

const VIEW_COMMUNITY = 'View Community'
const underTitle = (r: DOMRect): { left: number; top: number } => ({ left: r.left, top: r.bottom })

/** The thumbnail and name: a press opens a menu under them with View Community. */
function CommunityTitle({ community }: { community: CommunityConversation }): React.JSX.Element {
  const { open, at, button, menu, toggle, close } = useAnchoredMenu(underTitle)
  return (
    <>
      <button
        ref={button}
        type="button"
        className={`${styles.navLeft} ${styles.communityTitle}`}
        aria-label={`${community.name} options`}
        aria-haspopup="menu"
        aria-expanded={open}
        onClick={toggle}
      >
        <Avatar src={community.thumbnail || undefined} name={community.name} size={28} framed className={styles.channelIcon} />
        <span className={styles.navTitle} title={community.name}>{community.name}</span>
      </button>
      {open && at != null &&
        createPortal(
          <div ref={menu} className={styles.menuLayer} style={at} {...{ [MENU_LAYER]: '' }}>
            <ContextMenu
              variant="sheet"
              items={[
                {
                  label: VIEW_COMMUNITY,
                  onClick: () => {
                    close()
                    openCommunityModal(community.id, () => undefined)
                  }
                }
              ]}
            />
          </div>,
          document.body
        )}
    </>
  )
}
