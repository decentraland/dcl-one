// Smart wrapper for the world profile card: resolves a user by address from the profile store and renders
// the presentational card. Opened as a popup via openProfileCard() (see the avatarClick handler in
// useEngineSession); because <PopupHost/> is mounted inside Hud's <SessionProvider>, the popup can
// read the session with useSession() even though it renders through a portal.
import { openPopup } from '../../design'
import { relationshipOf } from '@explorer-core/lib/relationship'
import { useSession } from '../session/SessionContext'
import { useProfile } from '../session/profileStore'
import { openPassport } from '../profile/Passport'
import { ProfileCardPresentation, type CardAction, type ChatUser, type MenuContext } from '../chat/ProfileCardPresentation'
import { confirmBlock, confirmUnfriend, reportUser } from '../friends/friendDialogs'
import { openFriendRequest } from '../friends/FriendRequestPopup'
import { splitName } from '@explorer-core/lib/identity'
import { startCallWith } from '../voiceChat/callGate'
import { offersInvite, useInviteToCommunityItems } from '../communities/InviteToCommunity'

export function ProfileCard({
  userId,
  x,
  y,
  above,
  context,
  communityActions,
  invite = true,
  onMessage,
  onCall,
  onClose
}: {
  userId: string
  x: number
  y: number
  above?: boolean
  context?: MenuContext
  communityActions?: CardAction[]
  /** Offer Invite to Community (the voice panel's menu does not). */
  invite?: boolean
  /** Replaces opening the DM (a card opened over another popup closes that one first). */
  onMessage?: (address: string) => void
  /** Replaces starting the call, as `onMessage` does the DM. */
  onCall?: (address: string) => void
  onClose: () => void
}): React.JSX.Element {
  const session = useSession()
  const known = useProfile(userId)
  const user: ChatUser = { address: userId, name: known?.name ?? userId, picture: known?.picture, claimed: known?.hasClaimedName, nameColor: known?.nameColor }
  const act = session.friends.act
  // the user menus and the member menus offer it; the friend request and blocked lists' menus do not
  const isMe = session.profile.data?.address?.toLowerCase() === userId.toLowerCase()
  const inviteItems = useInviteToCommunityItems(userId, invite && offersInvite(isMe, context))
  return (
    <ProfileCardPresentation
      user={user}
      x={x}
      y={y}
      above={above}
      context={context}
      communityActions={communityActions}
      inviteItems={inviteItems}
      me={session.profile.data}
      relationship={relationshipOf(session.friends, userId)}
      blockedBy={session.friends.blockedBy.includes(userId.toLowerCase())}
      onAddFriend={(u) => openFriendRequest('send', u)}
      onUnfriend={(u) => void confirmUnfriend(u).then((ok) => ok && act('delete', u.address))}
      onCancelRequest={(u) => act('cancel', u.address)}
      onAcceptRequest={(u) => openFriendRequest('accept', u)}
      onBlock={(u) => void confirmBlock(splitName(u.name).base).then((ok) => ok && act('block', u.address))}
      onReport={(u) => void reportUser(session.profile.data?.address, u.address)}
      onMention={session.chat.mention}
      onMessage={(u) => (onMessage != null ? onMessage(u.address) : session.chat.openConversation(u.address))}
      onCall={(u) => (onCall != null ? onCall(u.address) : startCallWith(session, u.address))}
      hushed={session.nearbyMutes.muted.has(userId.toLowerCase())}
      onSetHushed={(u, hushed) => session.nearbyMutes.set(u.address, hushed)}
      onViewProfile={() => openPassport(userId)}
      onClose={onClose}
    />
  )
}

export interface ProfileCardOptions {
  /** (x, y) is the card's bottom-left; it grows upward (a row's menu button). */
  above?: boolean
  context?: MenuContext
  /** 'community': the owner's or moderator's actions on the member. */
  communityActions?: CardAction[]
  /** Offer Invite to Community; on unless false. */
  invite?: boolean
  /** Replaces opening the DM with the user. */
  onMessage?: (address: string) => void
  /** Replaces starting a call with the user. */
  onCall?: (address: string) => void
  /** Runs when the card closes by any path. */
  onClose?: () => void
}

/** Open the world profile card as a popup, anchored at the given screen coords. */
export function openProfileCard(userId: string, x: number, y: number, opts: ProfileCardOptions = {}): () => void {
  return openPopup(
    (close) => <ProfileCard userId={userId} x={x} y={y} above={opts.above} context={opts.context} communityActions={opts.communityActions} invite={opts.invite} onMessage={opts.onMessage} onCall={opts.onCall} onClose={close} />,
    { dim: false, onClose: opts.onClose }
  ) // anchored popover, no scrim dim
}
