// Community detail modal — matches Unity's CommunityCardView: a wide card with a header
// (thumbnail, name, public/private, members, description, open-chat, Join/Joined, ⋮),
// ANNOUNCEMENTS / MEMBERS / PLACES / PHOTOS tabs, and an Upcoming Events sidebar. Data
// (members/posts/places/events) arrives via the bridge `communityDetail` relay.

import { useEffect, useState } from 'react'
import { Avatar, BanIcon, Button, ControlButton, EmptyState, SwapLabel, DemoteModeratorIcon, Kebab, MaskIcon, Pencil, Plus, PromoteModeratorIcon, RemoveMemberIcon, Tabs, TransferIcon, Trash, openPopup, showConfirm, type TabItem } from '../../design'
import joinStreamIcon from '../../assets/voice/join-stream.png'
import streamIcon from '../../assets/voice/stream-small.png'
import endStreamIcon from '../../assets/voice/end-stream.png'
import { nameColor, splitName } from '@explorer-core/lib/identity'
import { relationshipOf, type Relationship } from '@explorer-core/lib/relationship'
import { knownUserColor } from '../session/profileStore'
import { isMemberRole, memberActions, roleLabel } from '@explorer-core/features/communities/roles'
import { useSession } from '../session/SessionContext'
import { openCommunityEditModal } from './CommunityCreateModal'
import { RequestRowActions } from './RequestRowActions'
import { openPassport } from '../profile/Passport'
import { openProfileCard } from '../profileCard/ProfileCard'
import { confirmUnblock } from '../friends/friendDialogs'
import { openFriendRequest } from '../friends/FriendRequestPopup'
import type { CardAction } from '../chat/ProfileCardPresentation'
import type {
  Community,
  CommunityDetailMessage,
  CommunityEvent,
  CommunityMember,
  CommunityMemberAction,
  CommunityMemberSection,
  CommunityMemberSectionMessage,
  CommunityRequestMember,
  CommunityPhoto,
  CommunityPlace,
  CommunityPost
} from '@explorer-core/engine/protocol'
import { inCommunityStream, startCallWith, streamAct, streamLive, streamPending } from '../voiceChat/callGate'
import styles from './CommunityModal.module.css'
import { siteUrl } from '@hud-host'

type Tab = 'announcements' | 'members' | 'places' | 'photos'
const TABS: TabItem<Tab>[] = [
  { id: 'announcements', label: 'ANNOUNCEMENTS' },
  { id: 'members', label: 'MEMBERS' },
  { id: 'places', label: 'PLACES' },
  { id: 'photos', label: 'PHOTOS' }
]

function compact(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`
  if (n >= 1000) return `${(n / 1000).toFixed(n >= 10_000 ? 0 : 1)}k`
  return String(n)
}
function postDate(ts: number): string {
  return new Date(ts).toLocaleDateString([], { month: 'short', day: 'numeric' })
}
function eventDate(ts: number): string {
  const d = new Date(ts)
  const wd = d.toLocaleDateString([], { weekday: 'short' }).toUpperCase()
  const md = d.toLocaleDateString([], { month: 'short', day: 'numeric' }).toUpperCase()
  const t = d.toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' })
  return `${wd}, ${md} @ ${t}`
}

function GlobeIcon(): React.JSX.Element {
  return (
    <svg viewBox="0 0 24 24" width="14" height="14" fill="none" aria-hidden="true">
      <circle cx="12" cy="12" r="9" stroke="currentColor" strokeWidth="1.7" />
      <path d="M3 12h18M12 3c2.5 2.5 2.5 15 0 18M12 3c-2.5 2.5-2.5 15 0 18" stroke="currentColor" strokeWidth="1.7" />
    </svg>
  )
}
function LockIcon(): React.JSX.Element {
  return (
    <svg viewBox="0 0 24 24" width="13" height="13" fill="none" aria-hidden="true">
      <rect x="5" y="11" width="14" height="9" rx="2" stroke="currentColor" strokeWidth="1.8" />
      <path d="M8 11V8a4 4 0 0 1 8 0v3" stroke="currentColor" strokeWidth="1.8" />
    </svg>
  )
}
function ChatIcon(): React.JSX.Element {
  return (
    <svg viewBox="0 0 24 24" width="18" height="18" fill="none" aria-hidden="true">
      <path d="M20 2H4c-1.1 0-2 .9-2 2v12c0 1.1.9 2 2 2h4l4 4 4-4h4c1.1 0 2-.9 2-2V4c0-1.1-.9-2-2-2z" stroke="currentColor" strokeWidth="1.8" strokeLinejoin="round" />
    </svg>
  )
}
function HeartIcon(): React.JSX.Element {
  return (
    <svg viewBox="0 0 24 24" width="15" height="15" fill="none" aria-hidden="true">
      <path d="M12 20s-7-4.5-9.5-9C1 8 2.5 4.5 6 4.5c2 0 3.2 1.2 4 2.5.8-1.3 2-2.5 4-2.5 3.5 0 5 3.5 3.5 6.5C19 15.5 12 20 12 20z" stroke="currentColor" strokeWidth="1.7" strokeLinejoin="round" />
    </svg>
  )
}
function AddFriendIcon(): React.JSX.Element {
  return (
    <svg viewBox="0 0 24 24" width="15" height="15" fill="none" aria-hidden="true">
      <circle cx="9" cy="8" r="3.2" stroke="currentColor" strokeWidth="1.7" />
      <path d="M3.5 19c0-3 2.5-4.6 5.5-4.6s5.5 1.6 5.5 4.6" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" />
      <path d="M19 8v6M16 11h6" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" />
    </svg>
  )
}
function PinIcon(): React.JSX.Element {
  return (
    <svg viewBox="0 0 24 24" width="13" height="13" fill="none" aria-hidden="true">
      <path d="M12 21s7-5.7 7-11a7 7 0 1 0-14 0c0 5.3 7 11 7 11z" stroke="currentColor" strokeWidth="1.7" strokeLinejoin="round" />
      <circle cx="12" cy="10" r="2.4" stroke="currentColor" strokeWidth="1.7" />
    </svg>
  )
}
function Verified(): React.JSX.Element {
  return (
    <svg viewBox="0 0 24 24" width="13" height="13" aria-hidden="true">
      <path d="M12 2l2.4 1.8 3-.3 1 2.8 2.6 1.5-.9 2.9.9 2.9-2.6 1.5-1 2.8-3-.3L12 22l-2.4-1.8-3 .3-1-2.8L3 16.2l.9-2.9L3 10.4l2.6-1.5 1-2.8 3 .3L12 2z" fill="#4d8dff" />
      <path d="M8.5 12l2.2 2.2 4.5-4.5" stroke="#fff" strokeWidth="1.8" fill="none" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  )
}

function PostRow({ post }: { post: CommunityPost }): React.JSX.Element {
  return (
    <div className={styles.post}>
      <Avatar src={post.authorPicture} name={post.author} color={knownUserColor(post.authorAddress, post.author)} size={36} />
      <div className={styles.postBody}>
        <div className={styles.postHead}>
          <span className={styles.postAuthor}>{post.author}</span>
          <span className={styles.postDate}>· {postDate(post.timestamp)}</span>
          <span className={styles.postLikes}><HeartIcon /> {post.likes}</span>
        </div>
        <p className={styles.postText}>{post.text}</p>
      </div>
    </div>
  )
}

/** The row's friendship buttons, as unity-explorer's member row: Add Friend, Accept Friend, Request
 *  Sent (Cancel Request on hover) and Blocked (Unblock on hover). Friends get none. */
export type FriendAction = 'add' | 'accept' | 'cancel' | 'unblock'

function FriendButton({ relationship, onAction }: { relationship: Relationship; onAction: (action: FriendAction) => void }): React.JSX.Element | null {
  if (relationship === 'none') {
    return (
      <button type="button" className={styles.addFriend} onClick={() => onAction('add')}>
        <AddFriendIcon /> ADD FRIEND
      </button>
    )
  }
  if (relationship === 'incoming') {
    return (
      <button type="button" className={styles.addFriend} onClick={() => onAction('accept')}>
        ACCEPT FRIEND
      </button>
    )
  }
  if (relationship === 'requested') {
    return (
      <button type="button" className={`${styles.requested} ${styles.swap}`} aria-label="Request Sent, Cancel Request" onClick={() => onAction('cancel')}>
        <span className={styles.swapIdle}>REQUEST SENT</span>
        <span className={styles.swapHover}>CANCEL REQUEST</span>
      </button>
    )
  }
  if (relationship === 'blocked') {
    return (
      <button type="button" className={`${styles.requested} ${styles.swap}`} aria-label="Blocked, Unblock" onClick={() => onAction('unblock')}>
        <span className={styles.swapIdle}>BLOCKED</span>
        <span className={styles.swapHover}>UNBLOCK</span>
      </button>
    )
  }
  return null
}

export function MemberRow({
  member,
  self,
  relationship = 'none',
  busy,
  menuOpen,
  actions,
  onView,
  onFriend = () => undefined,
  onMenu
}: {
  member: CommunityMember
  /** The signed-in user's own row, or the user is not known yet: no friendship button and no menu. */
  self: boolean
  relationship?: Relationship
  /** An action on this member is running. */
  busy: boolean
  menuOpen: boolean
  /** The row's own buttons (a request's ACCEPT and REJECT, an invite's CANCEL, a ban's UNBAN),
   *  shown instead of the friendship button. */
  actions?: React.ReactNode
  onView: () => void
  onFriend?: (action: FriendAction) => void
  onMenu: (anchor: HTMLElement) => void
}): React.JSX.Element {
  const label = roleLabel(member.role)
  const color = knownUserColor(member.address, member.name)
  const { base, tag } = splitName(member.name)
  return (
    <div className={`${styles.member} ${menuOpen ? styles.memberActive : ''}`.trim()}>
      <button type="button" className={styles.memberMain} aria-label={`View ${base}'s profile`} onClick={onView}>
        <Avatar src={member.picture} name={base} color={color} size={40} />
        <span className={styles.memberInfo}>
          <span className={styles.memberName} style={{ color }}>
            {base}
            {member.hasClaimedName === true ? <Verified /> : <span className={styles.memberTag}>{tag !== '' ? tag : `#${member.address.slice(-4)}`}</span>}
          </span>
          {label && <span className={styles.memberRole}>{label}</span>}
        </span>
      </button>
      {actions != null ? <span className={styles.rowActions}>{actions}</span> : !self && <FriendButton relationship={relationship} onAction={onFriend} />}
      {!self && (
        <ControlButton
          shape="circle"
          size="lg"
          className={styles.memberMenu}
          aria-label={`More options for ${base}`}
          aria-haspopup="menu"
          aria-expanded={menuOpen}
          aria-pressed={undefined}
          active={menuOpen}
          disabled={busy}
          onClick={(e) => onMenu(e.currentTarget)}
        >
          <Kebab vertical />
        </ControlButton>
      )}
    </div>
  )
}

function MemberTabs({ tabs, value, requests, onChange }: { tabs: MemberTab[]; value: MemberTab; requests: number; onChange: (tab: MemberTab) => void }): React.JSX.Element {
  const items: TabItem<MemberTab>[] = tabs.map((t) => ({ id: t, label: SECTION_LABEL[t], ...(t === 'requests' && requests > 0 ? { badge: requests } : {}) }))
  return <Tabs variant="subtab" className={styles.memberTabs} items={items} value={value} onChange={onChange} aria-label="Member sections" />
}

function PlaceCard({ place, onDelete, deleting = false }: { place: CommunityPlace; onDelete?: () => void; deleting?: boolean }): React.JSX.Element {
  return (
    <div className={styles.place}>
      <div className={styles.placeCover} style={{ background: place.thumbnail ? undefined : nameColor(place.id) }}>
        {place.thumbnail && <img src={place.thumbnail} alt="" />}
        {onDelete != null && (
          <button type="button" className={styles.placeDelete} aria-label={`Delete ${place.title}`} disabled={deleting} onClick={onDelete}>
            <Trash size={14} />
          </button>
        )}
      </div>
      <span className={styles.placeName} title={place.title}>{place.title}</span>
      <div className={styles.placeMeta}>
        {place.likeRate != null && <span className={styles.placeLike}>👍 {Math.round(place.likeRate * 100)}%</span>}
        {place.positions && <span className={styles.placePos}><PinIcon /> {place.positions}</span>}
      </div>
    </div>
  )
}

/** unity-explorer's first tile in the card's Places grid for owners and moderators: opens the
 *  Edit Community form, where places are linked. */
function AddPlaceTile({ onClick }: { onClick: () => void }): React.JSX.Element {
  return (
    <button type="button" className={styles.addPlace} onClick={onClick}>
      <Plus size={22} />
      <span>ADD PLACE</span>
    </button>
  )
}

function PhotoTile({ photo }: { photo: CommunityPhoto }): React.JSX.Element {
  return (
    <a className={styles.photo} href={photo.url} target="_blank" rel="noreferrer">
      <img src={photo.thumbnail ?? photo.url} alt="" loading="lazy" />
    </a>
  )
}

function EventRow({ event }: { event: CommunityEvent }): React.JSX.Element {
  return (
    <div className={styles.event}>
      <div className={styles.eventThumb} style={{ background: event.thumbnail ? undefined : nameColor(event.id) }}>
        {event.thumbnail && <img src={event.thumbnail} alt="" />}
      </div>
      <div className={styles.eventInfo}>
        <span className={styles.eventDate}>{eventDate(event.startsAt)}</span>
        <span className={styles.eventName} title={event.name}>{event.name}</span>
      </div>
    </div>
  )
}

// unity-explorer's member menu (CommunityMemberListContextMenuSettings, MembersListView)
const MEMBER_ACTION_LABEL: Record<CommunityMemberAction, string> = {
  demote: 'Demote Moderator',
  promote: 'Promote Moderator',
  transfer: 'Make Community Owner',
  kick: 'Remove Member',
  ban: 'Ban'
}
const MEMBER_ACTION_ICON: Record<CommunityMemberAction, React.ReactNode> = {
  demote: <DemoteModeratorIcon />,
  promote: <PromoteModeratorIcon />,
  transfer: <TransferIcon />,
  kick: <RemoveMemberIcon />,
  ban: <BanIcon />
}
const TRANSFER_NEEDS_NAME = 'Community ownership can only be transferred to users who own a NAME.'

// The Members tab's sections for owners and moderators, Join Requests for private communities
// only, each with its empty text
type MemberTab = 'members' | CommunityMemberSection
const SECTION_LABEL: Record<MemberTab, string> = { members: 'MEMBERS', banned: 'BANNED', requests: 'JOIN REQUESTS', invites: 'INVITED' }
const SECTION_EMPTY: Record<CommunityMemberSection, { title: string; body?: string }> = {
  banned: { title: 'No Banned Accounts', body: "Banned accounts won't be able to find your Community." },
  requests: { title: 'No Requests' },
  invites: { title: 'No Invites' }
}
export const SECTION_LOAD_ERROR = 'There was an error loading this list. Please try again.'

// one object, so a card without sections keeps the same props between renders
const NO_SECTIONS: Partial<Record<CommunityMemberSection, CommunityMemberSectionMessage>> = {}

/** The question before rejecting an invite, on the card and on a browse card. */
export function confirmRejectInvite(community: Community): Promise<boolean> {
  return showConfirm({
    title: `Are you sure you want to delete your invitation to the [${community.name}] Community?`,
    body: community.thumbnail != null && community.thumbnail !== '' ? <img className={styles.confirmThumb} src={community.thumbnail} alt="" /> : undefined,
    confirmLabel: 'YES',
    cancelLabel: 'NO'
  })
}

export function CommunityModal({
  community,
  detail,
  error = null,
  me = null,
  relationship = () => 'none',
  onJoin,
  onRequestToJoin,
  onCancelRequest,
  onLeave,
  onFriend = () => undefined,
  onViewProfile = () => undefined,
  onMemberMenu = () => undefined,
  onMemberAction,
  onOpenChat,
  stream,
  onEdit,
  onDelete,
  onRemovePlace,
  onAcceptInvite,
  onRejectInvite,
  sections = NO_SECTIONS,
  onLoadSection,
  onManageRequest,
  onUnban,
  onClose
}: {
  community: Community
  detail: CommunityDetailMessage | null
  /** Why the last join/request/leave for this community failed. */
  error?: string | null
  onJoin: (id: string) => void
  onRequestToJoin: (id: string) => void
  onCancelRequest: (id: string, requestId: string) => void
  onLeave: (id: string) => void
  /** The signed-in user: their own row has no menu, and a transfer shows them handing over. */
  me?: { address: string; name: string; picture?: string } | null
  /** The signed-in user's friendship with a member, for the row's button. */
  relationship?: (address: string) => Relationship
  /** A member row's friendship button. */
  onFriend?: (member: CommunityMember, action: FriendAction) => void
  /** A member row's click: their passport. */
  onViewProfile?: (address: string) => void
  /** Open a member's menu at its button, with the owner's or moderator's actions; `onClose` runs
   *  when it closes. */
  onMemberMenu?: (member: CommunityMember, anchor: HTMLElement, actions: CardAction[], onClose: () => void) => void
  /** Run an owner's or moderator's action on a member (asked first where unity-explorer asks). */
  onMemberAction?: (id: string, address: string, action: CommunityMemberAction) => void
  onOpenChat: () => void
  /** The community's voice stream: live, whether the local user is in it, and whether they may
   *  open one (owner or moderator). */
  stream?: { live: boolean; inStream: boolean; canStart: boolean; pending: boolean; participants: number; onStart: () => void; onJoin: () => void; onLeave: () => void }
  /** Open the Edit Community form (owners and moderators). */
  onEdit?: () => void
  /** Delete the community (owner only; asked first). */
  onDelete?: (id: string) => void
  /** Unlink a place the user owns (owners and moderators; asked first). */
  onRemovePlace?: (id: string, placeId: string) => void
  /** Accept or reject the user's pending invite (a reject is asked first). */
  onAcceptInvite?: (id: string, inviteId: string) => void
  onRejectInvite?: (id: string, inviteId: string) => void
  /** The Members tab's other sections, for owners and moderators, as last read. */
  sections?: Partial<Record<CommunityMemberSection, CommunityMemberSectionMessage>>
  /** Read one of those sections (on opening it, and Join Requests on opening a private card). */
  onLoadSection?: (section: CommunityMemberSection) => void
  /** Answer a member's join request, or cancel an invite sent to them. */
  onManageRequest?: (member: CommunityRequestMember, intention: 'accepted' | 'rejected' | 'cancelled') => void
  onUnban?: (member: CommunityRequestMember) => void
  onClose: () => void
}): React.JSX.Element {
  const [tab, setTab] = useState<Tab>('announcements')
  const [coverFailed, setCoverFailed] = useState(false)
  const [menuOpen, setMenuOpen] = useState(false)
  // the places being deleted: their delete waits for the card's places (or an error) to come back
  const [removing, setRemoving] = useState<ReadonlySet<string>>(new Set())
  // the members acted on: their menu waits for the card's members (or an error) to come back
  const [acting, setActing] = useState<ReadonlySet<string>>(new Set())
  // released when the card's data comes back, or when an error arrives (not when one is cleared:
  // a new action clears the last error while it is still running)
  useEffect(() => {
    setRemoving(new Set())
    setActing(new Set())
  }, [detail, sections])
  useEffect(() => {
    if (error == null) return
    setRemoving(new Set())
    setActing(new Set())
  }, [error])
  const [menuFor, setMenuFor] = useState<string | null>(null)
  const member = isMemberRole(community.role)
  const isPrivate = community.privacy === 'private'
  // a pending invite replaces the join, chat and edit buttons
  const inviteId = !member ? community.pendingInviteId : undefined
  const [memberTab, setMemberTab] = useState<MemberTab>('members')
  const manager = community.role === 'owner' || community.role === 'moderator'
  const memberTabs: MemberTab[] = manager ? ['members', 'banned', ...(isPrivate ? (['requests'] as const) : []), 'invites'] : ['members']
  const shownTab: MemberTab = memberTabs.includes(memberTab) ? memberTab : 'members'
  const requestsCount = sections.requests?.total ?? 0
  // the Join Requests counter is read when a private card opens for its owner or moderator
  const wantsRequestsCount = manager && isPrivate
  useEffect(() => {
    if (wantsRequestsCount) onLoadSection?.('requests')
  }, [wantsRequestsCount, community.id]) // eslint-disable-line react-hooks/exhaustive-deps
  const openMemberTab = (tab: MemberTab): void => {
    setMemberTab(tab)
    if (tab !== 'members') onLoadSection?.(tab)
  }
  const runRowAction = (row: CommunityRequestMember, run: () => void): void => {
    setActing((prev) => new Set(prev).add(row.address))
    run()
  }
  const rowActions = (section: CommunityMemberSection, row: CommunityRequestMember): React.ReactNode => (
    <RequestRowActions
      section={section}
      busy={acting.has(row.address)}
      onAnswer={(intention) => runRowAction(row, () => onManageRequest?.(row, intention))}
      onUnban={() => runRowAction(row, () => onUnban?.(row))}
    />
  )
  const showCover = community.thumbnail != null && !coverFailed
  const loading = detail == null

  // until the user's own profile is known, no row offers controls: any of them could be theirs
  const noControls = (address: string): boolean => me == null || me.address.toLowerCase() === address.toLowerCase()
  const runMemberAction = (member: CommunityMember, action: CommunityMemberAction): void => {
    setActing((prev) => new Set(prev).add(member.address))
    onMemberAction?.(community.id, member.address, action)
  }
  // unity-explorer asks before a transfer, a kick or a ban; promoting and demoting run at once
  const askMemberAction = (member: CommunityMember, action: CommunityMemberAction): void => {
    const name = splitName(member.name).base
    const face = <Avatar src={member.picture} name={name} color={knownUserColor(member.address, member.name)} size={64} />
    const confirm =
      action === 'transfer'
        ? showConfirm({
            title: `Transferring Community Ownership to ${name}`,
            body: (
              <div className={styles.confirmTransfer}>
                <div className={styles.confirmFaces}>
                  {me != null && <Avatar src={me.picture} name={splitName(me.name).base} color={knownUserColor(me.address, me.name)} size={64} />}
                  <TransferIcon size={24} />
                  {face}
                </div>
                <p>Once you transfer ownership of a Community, you will be demoted from Owner to Moderator. This action cannot be undone by you.</p>
              </div>
            ),
            confirmLabel: 'TRANSFER',
            cancelLabel: 'CANCEL'
          })
        : action === 'kick'
          ? showConfirm({ title: `Are you sure you want to remove [${name}] from the [${community.name}] Community?`, body: face, confirmLabel: 'KICK', cancelLabel: 'CANCEL' })
          : action === 'ban'
            ? showConfirm({ title: `Are you sure you want to ban [${name}] from the [${community.name}] Community?`, body: face, confirmLabel: 'BAN', cancelLabel: 'CANCEL' })
            : Promise.resolve(true)
    void confirm.then((yes) => {
      if (yes) runMemberAction(member, action)
    })
  }
  const openMemberMenu = (member: CommunityMember, anchor: HTMLElement): void => {
    const actions: CardAction[] =
      onMemberAction == null
        ? []
        : memberActions(community.role, member).map(({ action, needsName }) => ({
            label: MEMBER_ACTION_LABEL[action],
            icon: MEMBER_ACTION_ICON[action],
            onClick: () => askMemberAction(member, action),
            ...(needsName === true ? { disabledReason: TRANSFER_NEEDS_NAME } : {})
          }))
    setMenuFor(member.address)
    onMemberMenu(member, anchor, actions, () => setMenuFor(null))
  }
  // unity-explorer's confirmation, with the community's picture
  const confirmDelete = (): void => {
    setMenuOpen(false)
    void showConfirm({
      title: `Are you sure you want to delete [${community.name}] Community?`,
      body: showCover ? <img className={styles.confirmThumb} src={community.thumbnail} alt="" /> : undefined,
      confirmLabel: 'CONTINUE',
      cancelLabel: 'CANCEL'
    }).then((yes) => {
      if (yes) onDelete?.(community.id)
    })
  }
  // as unity-explorer: owners and moderators get the ADD PLACE tile, and delete on the places they own
  const canModify = community.role === 'owner' || community.role === 'moderator'
  const confirmRemovePlace = (place: CommunityPlace): void => {
    void showConfirm({
      title: `Are you sure you want to delete [${place.title}] from [${community.name}]'s Places?`,
      body: "Photos and events associated with this Place won't appear on the Community's profile anymore.",
      confirmLabel: 'DELETE',
      cancelLabel: 'CANCEL'
    }).then((yes) => {
      if (!yes) return
      setRemoving((prev) => new Set(prev).add(place.id))
      onRemovePlace?.(community.id, place.id)
    })
  }
  const copyLink = (): void => {
    void navigator.clipboard?.writeText(siteUrl(`/communities/${community.id}`)).catch(() => undefined)
    setMenuOpen(false)
  }

  return (
    // Just the card: the popup layer (PopupHost .dim/.pop) draws the scrim, DPI-scales it and animates
    // it in. stopPropagation keeps a click on the card from reaching the scrim (and closing it).
    <div className={styles.modal} onClick={(e) => e.stopPropagation()}>
      <div className={styles.main}>
        <header className={styles.header}>
          <div className={styles.thumb} style={{ background: showCover ? undefined : nameColor(community.id) }}>
            {showCover ? (
              <img src={community.thumbnail} alt="" onError={() => setCoverFailed(true)} />
            ) : (
              <span>{community.name.charAt(0).toUpperCase()}</span>
            )}
          </div>
          <div className={styles.headInfo}>
            <h2 className={styles.name} title={community.name}>{community.name}</h2>
            <div className={styles.meta}>
              <span className={styles.privacy}>{isPrivate ? <><LockIcon /> Private</> : <><GlobeIcon /> Public</>}</span>
              <span className={styles.bar}>|</span>
              <span><b>{compact(community.membersCount)}</b> Members</span>
            </div>
            {community.description && <p className={styles.desc}>{community.description}</p>}
            {error && <p className={styles.error} role="alert">{error}</p>}
          </div>
          <div className={styles.headActions}>
            {stream != null && stream.inStream ? (
              <Button size="sm" variant="secondary" onClick={stream.onLeave}><MaskIcon src={endStreamIcon} size={14} /> Leave stream</Button>
            ) : stream != null && stream.live ? (
              <Button size="sm" variant="accent" aria-label="Join stream" disabled={stream.pending} onClick={stream.onJoin}><MaskIcon src={joinStreamIcon} size={14} /> Live · Join stream</Button>
            ) : stream != null && stream.canStart ? (
              <Button size="sm" variant="secondary" disabled={stream.pending} onClick={stream.onStart}><MaskIcon src={streamIcon} size={14} /> Start stream</Button>
            ) : null}
            {inviteId != null && (
              <>
                <Button size="sm" onClick={() => onAcceptInvite?.(community.id, inviteId)}>ACCEPT INVITE</Button>
                <Button size="sm" variant="secondary" onClick={() => void confirmRejectInvite(community).then((yes) => yes && onRejectInvite?.(community.id, inviteId))}>
                  REJECT INVITE
                </Button>
              </>
            )}
            {(community.role === 'owner' || community.role === 'moderator') && onEdit != null && (
              <button type="button" className={styles.iconBtn} aria-label="Edit Community" onClick={onEdit}><Pencil size={16} /></button>
            )}
            {member && (
              <button type="button" className={styles.iconBtn} aria-label="Open chat" onClick={onOpenChat}><ChatIcon /></button>
            )}
            {inviteId != null ? null : member ? (
              <Button size="sm" variant="ghost" className={styles.joined} disabled>✓ Joined</Button>
            ) : isPrivate && community.pendingRequestId != null ? (
              <Button size="sm" variant="ghost" aria-label="Cancel join request" onClick={() => onCancelRequest(community.id, community.pendingRequestId!)}>
                <SwapLabel idle="Request Sent" hover="Cancel Request" />
              </Button>
            ) : isPrivate ? (
              <Button size="sm" onClick={() => onRequestToJoin(community.id)}>Request to Join</Button>
            ) : (
              <Button size="sm" onClick={() => onJoin(community.id)}>Join</Button>
            )}
            <div className={styles.kebabWrap}>
              <button type="button" className={styles.kebab} aria-label="More" onClick={() => setMenuOpen((o) => !o)}>⋮</button>
              {menuOpen && (
                <div className={styles.menu}>
                  <button type="button" className={styles.menuItem} onClick={copyLink}>Copy link</button>
                  {/* an owner cannot leave their community, only delete it (unity-explorer) */}
                  {member && community.role !== 'owner' && (
                    <button type="button" className={`${styles.menuItem} ${styles.menuDanger}`} onClick={() => { setMenuOpen(false); onLeave(community.id); onClose() }}>
                      Leave community
                    </button>
                  )}
                  {community.role === 'owner' && onDelete != null && (
                    <button type="button" className={`${styles.menuItem} ${styles.menuDanger}`} onClick={confirmDelete}>
                      Delete Community
                    </button>
                  )}
                </div>
              )}
            </div>
          </div>
        </header>

        <Tabs variant="underline" className={styles.tabs} items={TABS} value={tab} onChange={setTab} aria-label="Community sections" />

        <div className={styles.tabBody}>
          {loading ? (
            <div className={styles.empty}>Loading…</div>
          ) : tab === 'announcements' ? (
            detail.posts.length > 0 ? detail.posts.map((p) => <PostRow key={p.id} post={p} />) : <div className={styles.empty}>No announcements yet.</div>
          ) : tab === 'members' && memberTabs.length > 1 && shownTab !== 'members' ? (
            <>
              <MemberTabs tabs={memberTabs} value={shownTab} requests={requestsCount} onChange={openMemberTab} />
              {(() => {
                const read = sections[shownTab]
                if (read == null) return <div className={styles.empty}>Loading…</div>
                // a list that could not be read says so, rather than claiming there is nothing in it
                if (read.failed === true) {
                  return (
                    <EmptyState
                      variant="inline"
                      tone="error"
                      title={SECTION_LOAD_ERROR}
                      actions={[{ label: 'RETRY', variant: 'outline', onClick: () => onLoadSection?.(shownTab) }]}
                    />
                  )
                }
                if (read.members.length === 0) {
                  const empty = SECTION_EMPTY[shownTab]
                  return <EmptyState variant="inline" title={empty.title} subtitle={empty.body} />
                }
                return (
                  <div className={`${styles.memberGrid} ${styles.sectionGrid}`}>
                    {read.members.map((m) => (
                      <MemberRow
                        key={m.address}
                        member={m}
                        self={noControls(m.address)}
                        busy={acting.has(m.address)}
                        menuOpen={menuFor === m.address}
                        actions={rowActions(shownTab, m)}
                        onView={() => onViewProfile(m.address)}
                        onMenu={(anchor) => {
                          setMenuFor(m.address)
                          onMemberMenu(m, anchor, [], () => setMenuFor(null))
                        }}
                      />
                    ))}
                  </div>
                )
              })()}
            </>
          ) : tab === 'members' ? (
            <>
            {memberTabs.length > 1 && <MemberTabs tabs={memberTabs} value={shownTab} requests={requestsCount} onChange={openMemberTab} />}
            {detail.members.length > 0 ? (
              <div className={styles.memberGrid}>
                {detail.members.map((m) => (
                  <MemberRow
                    key={m.address}
                    member={m}
                    self={noControls(m.address)}
                    relationship={relationship(m.address)}
                    busy={acting.has(m.address)}
                    menuOpen={menuFor === m.address}
                    onView={() => onViewProfile(m.address)}
                    onFriend={(action) => onFriend(m, action)}
                    onMenu={(anchor) => openMemberMenu(m, anchor)}
                  />
                ))}
              </div>
            ) : <div className={styles.empty}>No members to show.</div>}
            </>
          ) : tab === 'places' ? (
            detail.places.length > 0 || (canModify && onEdit != null) ? (
              <div className={styles.placeGrid}>
                {canModify && onEdit != null && <AddPlaceTile onClick={onEdit} />}
                {detail.places.map((p) => (
                  <PlaceCard
                    key={p.id}
                    place={p}
                    onDelete={canModify && p.isMine === true && onRemovePlace != null ? () => confirmRemovePlace(p) : undefined}
                    deleting={removing.has(p.id)}
                  />
                ))}
              </div>
            ) : <div className={styles.empty}>No places shared yet.</div>
          ) : detail.photos.length > 0 ? (
            <div className={styles.photoGrid}>{detail.photos.map((ph) => <PhotoTile key={ph.id} photo={ph} />)}</div>
          ) : (
            <div className={styles.empty}>No photos shared yet.</div>
          )}
        </div>
      </div>

      <aside className={styles.events}>
        <div className={styles.eventsHead}>Upcoming Events</div>
        <div className={styles.eventsList}>
          {loading ? (
            <div className={styles.empty}>Loading…</div>
          ) : detail.events.length > 0 ? (
            detail.events.map((e) => <EventRow key={e.id} event={e} />)
          ) : (
            <div className={styles.empty}>No upcoming events.</div>
          )}
        </div>
      </aside>

      <button type="button" className={styles.close} aria-label="Close" onClick={onClose}>×</button>
    </div>
  )
}

/** Reads live community data + the pending detail from the session and renders the card. Mirrors
 *  ProfileCard/Passport: the popup closure captures only the static id; everything reactive (detail
 *  arriving async, membership/role changing) comes from useSession(), so the popup content re-renders
 *  in place — never a frozen snapshot from when it was opened. */
function CommunityModalContainer({ communityId, onClose }: { communityId: string; onClose: () => void }): React.JSX.Element | null {
  const session = useSession()
  const { communities, friends, chat, voiceChat } = session
  const { loadDetail } = communities

  const { fetch } = communities
  // My Communities and the browse grid only hold what the Communities page read; a card opened
  // from elsewhere (the chat's "View Community") has its community fetched by id. Every opening
  // reads the community again, with the user's pending invite and request: one listed earlier can
  // have changed since (an invite arrived, a request was accepted).
  const community =
    communities.list.find((c) => c.id === communityId) ??
    communities.browse.items.find((c) => c.id === communityId) ??
    communities.cards[communityId]
  useEffect(() => {
    fetch(communityId)
  }, [fetch, communityId])
  const { cardOpen } = communities
  useEffect(() => {
    cardOpen(communityId)
    return () => cardOpen(null)
  }, [cardOpen, communityId])
  useEffect(() => {
    loadDetail(communityId)
  }, [loadDetail, communityId])

  const error = communities.error?.id === communityId ? communities.error.message : null
  // deleted: the card goes with it
  const gone = communities.deleted === communityId
  useEffect(() => {
    if (gone) onClose()
  }, [gone, onClose])
  if (!community) {
    // not fetched yet, or gone from the list while open; a failed fetch says so instead of nothing
    if (error == null) return null
    return (
      <div className={styles.modal} onClick={(e) => e.stopPropagation()}>
        <p className={styles.error} role="alert">{error}</p>
        <button type="button" className={styles.close} aria-label="Close" onClick={onClose}>×</button>
      </div>
    )
  }

  const detail = communities.detail != null && communities.detail.id === communityId ? communities.detail : null
  const live = voiceChat.active.find((c) => c.communityId === communityId)
  const stream = {
    live: streamLive(voiceChat, communityId, community.voiceChatStatus?.isActive),
    inStream: inCommunityStream(voiceChat.state, communityId),
    canStart: community.role === 'owner' || community.role === 'moderator',
    pending: streamPending(voiceChat.state),
    participants: live?.participantCount ?? community.voiceChatStatus?.participantCount ?? 0,
    onStart: () => streamAct(voiceChat, { op: 'startCommunityCall', communityId }),
    onJoin: () => streamAct(voiceChat, { op: 'joinCommunityCall', communityId }),
    onLeave: () => voiceChat.act({ op: 'leaveCall' })
  }

  return (
    <CommunityModal
      stream={stream}
      community={community}
      detail={detail}
      error={error}
      onJoin={communities.join}
      onRequestToJoin={communities.requestToJoin}
      onCancelRequest={communities.cancelRequest}
      onLeave={communities.leave}
      me={session.profile.data ?? null}
      relationship={(address) => relationshipOf(friends, address)}
      onFriend={(member, action) => {
        const user = { address: member.address, name: member.name, picture: member.picture }
        if (action === 'add') openFriendRequest('send', user)
        else if (action === 'accept') openFriendRequest('accept', user)
        else if (action === 'cancel') friends.act('cancel', member.address)
        else void confirmUnblock(splitName(member.name).base).then((ok) => ok && friends.act('unblock', member.address))
      }}
      onViewProfile={(address) => openPassport(address)}
      onMemberMenu={(member, anchor, actions, closed) => {
        // as the friends list's menu: up from the button, 5px right of and 10px above its centre
        const r = anchor.getBoundingClientRect()
        openProfileCard(member.address, r.left + r.width / 2 + 5, r.top + r.height / 2 - 10, {
          above: true,
          context: 'community',
          communityActions: actions,
          // as unity-explorer, a chat closes the card first
          onMessage: (address) => {
            onClose()
            chat.openConversation(address)
          },
          // a call opens the DM too, so it closes the card first as well
          onCall: (address) => {
            onClose()
            startCallWith(session, address)
          },
          onClose: closed
        })
      }}
      onMemberAction={communities.memberAction}
      onEdit={() => openCommunityEditModal(community)}
      onDelete={communities.remove}
      onRemovePlace={communities.removePlace}
      onAcceptInvite={(id, inviteId) => communities.manageRequest(id, inviteId, 'accepted')}
      onRejectInvite={(id, inviteId) => communities.manageRequest(id, inviteId, 'rejected')}
      sections={communities.memberSections}
      onLoadSection={(section) => communities.loadMemberSection(communityId, section)}
      onManageRequest={(m, intention) => communities.manageRequest(communityId, m.requestId, intention, m.address)}
      onUnban={(m) => communities.unban(communityId, m.address)}
      onOpenChat={() => {
        onClose()
        chat.openCommunity({ id: community.id, name: community.name, thumbnail: community.thumbnail ?? '', role: community.role })
      }}
      onClose={onClose}
    />
  )
}

/** Open a community's detail modal as a popup; returns the close handle. `onClose` fires exactly once
 *  whichever way the popup goes away (× button, Leave, Escape, scrim-click) — the caller (CommunitiesPage)
 *  uses it to clear its own "which community is open" state, so re-selecting the same community after
 *  closing it still opens (a same-value setState wouldn't otherwise re-trigger the opening effect). */
export function openCommunityModal(communityId: string, onClose: () => void): () => void {
  return openPopup((close) => <CommunityModalContainer communityId={communityId} onClose={close} />, { onClose })
}
