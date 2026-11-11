// Profile card (the old SDK7 "profile menu") — the popover shown when you click a
// sender's name/avatar, an @mention, or a nearby avatar in the world. Header with
// avatar / name+copy / address+copy, a relationship-driven friend CTA (Add / Accept +
// Reject / Requested), then the action list — mirroring bevy-ui-scene's profile-menu:
// View Passport · Mention · Block/Unblock. A community card's member row opens it as
// The member menu (context 'community'): View Profile · Chat · Call, the owner's and
// moderators' actions, then Block · Report, then Invite to Community.

import { useEffect, useId, useLayoutEffect, useRef, useState } from 'react'
import { Avatar, BlockUserIcon, MaskIcon, ReportFlag, Submenu, Tooltip, VerifiedBadge, type ContextMenuItem } from '../../design'
import { closingFirst, INVITE_TO_COMMUNITY, InviteToCommunityIcon } from '../communities/InviteToCommunity'
import callIcon from '../../assets/voice/call.png'
import { shortAddr, splitName, userColor } from '@explorer-core/lib/identity'
import { looksClaimed, mentionName } from '@explorer-core/engine/mention'
import { peekProfile } from '../session/profileStore'
import type { Relationship } from '@explorer-core/lib/relationship'
import styles from './ProfileCard.module.css'

export type { Relationship } from '@explorer-core/lib/relationship'

export interface ChatUser {
  address: string
  name: string
  picture?: string
  claimed?: boolean
  nameColor?: { r: number; g: number; b: number }
}

function CopyIcon(): React.JSX.Element {
  return (
    <svg viewBox="0 0 24 24" width="14" height="14" fill="none" aria-hidden="true">
      <rect x="9" y="9" width="11" height="11" rx="2" stroke="currentColor" strokeWidth="1.7" />
      <path d="M5 15V5a2 2 0 0 1 2-2h8" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" />
    </svg>
  )
}
function AddFriendIcon(): React.JSX.Element {
  return (
    <svg viewBox="0 0 24 24" width="18" height="18" fill="none" aria-hidden="true">
      <circle cx="9" cy="8" r="3.4" stroke="currentColor" strokeWidth="1.9" />
      <path d="M3.5 19c0-3.1 2.5-4.8 5.5-4.8" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" />
      <path d="M18 8v6M15 11h6" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" />
    </svg>
  )
}
function ViewProfileIcon(): React.JSX.Element {
  return (
    <svg viewBox="0 0 24 24" width="20" height="20" fill="none" aria-hidden="true">
      <rect x="3" y="5" width="18" height="14" rx="2.5" stroke="currentColor" strokeWidth="1.8" />
      <circle cx="8.5" cy="12" r="2.2" stroke="currentColor" strokeWidth="1.7" />
      <path d="M13.5 10.5h4M13.5 14h2.5" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" />
    </svg>
  )
}
function MessageIcon(): React.JSX.Element {
  return (
    <svg viewBox="0 0 24 24" width="20" height="20" fill="none" aria-hidden="true">
      <path d="M4 6.5A2.5 2.5 0 0 1 6.5 4h11A2.5 2.5 0 0 1 20 6.5v8a2.5 2.5 0 0 1-2.5 2.5H9l-4.2 3.2A.5.5 0 0 1 4 19.8V6.5Z" stroke="currentColor" strokeWidth="1.8" strokeLinejoin="round" />
    </svg>
  )
}
function MentionIcon(): React.JSX.Element {
  return (
    <svg viewBox="0 0 24 24" width="20" height="20" fill="none" aria-hidden="true">
      <circle cx="12" cy="12" r="4" stroke="currentColor" strokeWidth="1.8" />
      <path d="M16 8.5V13a2.5 2.5 0 0 0 5 0V12a9 9 0 1 0-3.6 7.2" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" />
    </svg>
  )
}

export type MenuContext = 'default' | 'friend' | 'request' | 'blocked' | 'community'

/** An extra row of the 'community' menu: a role action on the member. */
export interface CardAction {
  label: string
  icon: React.ReactNode
  onClick: () => void
  /** Shown greyed out with this explanation on hover, instead of running. */
  disabledReason?: string
}

export function ProfileCardPresentation({
  user,
  x,
  y,
  above = false,
  me,
  relationship = 'none',
  blockedBy = false,
  context = 'default',
  communityActions = [],
  inviteItems = [],
  onAddFriend,
  onUnfriend,
  onCancelRequest,
  onAcceptRequest,
  onBlock,
  onReport,
  onMention,
  onMessage,
  onCall,
  onViewProfile,
  onClose
}: {
  user: ChatUser
  x: number
  y: number
  /** Grow upward from (x, y) — anchored to a row's menu button — instead of down from a click. */
  above?: boolean
  me?: { address?: string } | null
  /** Relationship of the local user to this profile — drives the friendship button. */
  relationship?: Relationship
  /** This user blocked the local user ('community' hides Block then, as unity-explorer does). */
  blockedBy?: boolean
  /** Which list opened it: the item set follows the reference menu for that list. */
  context?: MenuContext
  /** 'community' only: the owner's or moderator's actions on this member. */
  communityActions?: CardAction[]
  /** The Invite to Community submenu's communities; the option shows only when there are some. */
  inviteItems?: ContextMenuItem[]
  onAddFriend?: (user: ChatUser) => void
  onUnfriend?: (user: ChatUser) => void
  onCancelRequest?: (user: ChatUser) => void
  onAcceptRequest?: (user: ChatUser) => void
  onBlock?: (user: ChatUser) => void
  onReport?: (user: ChatUser) => void
  onMention?: (name: string) => void
  /** Open a DM with the user. */
  onMessage?: (user: ChatUser) => void
  /** Ring the user (a private voice call). */
  onCall?: (user: ChatUser) => void
  onViewProfile?: (user: ChatUser) => void
  onClose: () => void
}): React.JSX.Element {
  const [copied, setCopied] = useState<'name' | 'address' | null>(null)
  // Reset the "copied" hint after a beat — in an effect so the timer is cleared on unmount.
  useEffect(() => {
    if (!copied) return
    const t = setTimeout(() => setCopied(null), 1200)
    return () => clearTimeout(t)
  }, [copied])
  const cardRef = useRef<HTMLDivElement>(null)
  const reasonId = useId()
  // Start at the anchor; once laid out, clamp to the viewport using the card's REAL size.
  const [pos, setPos] = useState({ left: x, top: y })
  // the Invite to Community row arrives after the card opens and makes it taller
  const hasInvite = inviteItems.length > 0
  useLayoutEffect(() => {
    const el = cardRef.current
    if (!el) return
    const r = el.getBoundingClientRect()
    const top = above ? y - r.height : y
    setPos({
      left: Math.max(8, Math.min(x, window.innerWidth - r.width - 8)),
      top: Math.max(8, Math.min(top, window.innerHeight - r.height - 8))
    })
  }, [x, y, above, relationship, hasInvite])
  const isMe = !!me?.address && !!user.address && me.address.toLowerCase() === user.address.toLowerCase()
  const { base, tag } = splitName(user.name)
  const color = userColor(user.address, user.name, user.claimed, user.nameColor)

  const copy = (text: string, which: 'name' | 'address'): void => {
    navigator.clipboard?.writeText(text).then(
      () => setCopied(which),
      () => {}
    )
  }
  const run = (fn?: (u: ChatUser) => void) => (): void => {
    onClose()
    fn?.(user)
  }

  const other = !isMe && !!user.address
  const community = context === 'community'
  // unity-explorer's member menu also hides Block when the member blocked you
  const canBlock = other && relationship !== 'blocked' && !(community && blockedBy) && !!onBlock
  const canReport = other && !!onReport
  const showMention = context !== 'request' && context !== 'blocked' && !community && !!onMention
  // unity-explorer's member menu offers Chat to every other member
  const showMessage = other && (community || (relationship !== 'blocked' && context !== 'blocked')) && !!onMessage
  const showCall = other && relationship !== 'blocked' && context !== 'blocked' && !!onCall

  let cta: React.ReactNode = null
  // the member menu's header button only shows a friendship (to remove it): the row carries the rest
  if (other && relationship !== 'blocked' && (!community || relationship === 'friend')) {
    if (relationship === 'friend' && onUnfriend) {
      cta = (
        <button type="button" className={`${styles.cta} ${styles.ctaFriend}`} onClick={run(onUnfriend)}>
          <span className={styles.ctaIdle}>Friend</span>
          <span className={styles.ctaHover}>Remove Friend</span>
        </button>
      )
    } else if (relationship === 'requested' && onCancelRequest) {
      cta = (
        <button type="button" className={`${styles.cta} ${styles.ctaSent}`} onClick={run(onCancelRequest)}>
          <span className={styles.ctaIdle}>Request Sent</span>
          <span className={styles.ctaHover}>Cancel Request</span>
        </button>
      )
    } else if (relationship === 'incoming' && onAcceptRequest) {
      cta = (
        <button type="button" className={`${styles.cta} ${styles.ctaPrimary}`} onClick={run(onAcceptRequest)}>
          Accept Friend
        </button>
      )
    } else if (relationship === 'none' && onAddFriend) {
      cta = (
        <button type="button" className={`${styles.cta} ${styles.ctaPrimary}`} onClick={run(onAddFriend)}>
          <AddFriendIcon /> Add Friend
        </button>
      )
    }
  }

  const viewProfile = onViewProfile && (
    <button type="button" className={styles.row} onClick={run(onViewProfile)}>
      <ViewProfileIcon />
      <span>View Profile</span>
    </button>
  )
  const block = canBlock && (
    <button type="button" className={`${styles.row} ${styles.danger}`} onClick={run(onBlock)}>
      <BlockUserIcon />
      <span>Block</span>
    </button>
  )
  const report = canReport && (
    <button type="button" className={`${styles.row} ${styles.danger}`} onClick={run(onReport)}>
      <ReportFlag />
      <span>Report</span>
    </button>
  )
  const hasDestructive = !!block || !!report
  // picking a community invites and closes the card, as any other row does
  const invite = other && inviteItems.length > 0 && (
    <Submenu
      className={styles.row}
      label={INVITE_TO_COMMUNITY}
      icon={<InviteToCommunityIcon />}
      items={closingFirst(inviteItems, onClose)}
    />
  )
  const call = showCall && (
    <button type="button" className={styles.row} onClick={run(onCall)}>
      <MaskIcon src={callIcon} size={16} />
      <span>Call</span>
    </button>
  )
  const hasMenu = !isMe && (!!viewProfile || showMention || showMessage || showCall || hasDestructive || communityActions.length > 0 || !!invite)
  const message = showMessage && (
    <button type="button" className={styles.row} onClick={run(onMessage)}>
      <MessageIcon />
      <span>Chat</span>
    </button>
  )

  // The backdrop (and click-outside-to-close) is owned by the popup layer (openPopup default
  // options); this just renders the positioned card. The stopPropagation keeps a click on the card
  // from reaching the backdrop.
  return (
    <div ref={cardRef} className={styles.card} style={{ left: pos.left, top: pos.top }} onClick={(e) => e.stopPropagation()} role="dialog" aria-label="Profile">
      <div className={styles.header}>
        <Avatar src={user.picture} name={base} color={color} size={72} status="online" />
        <button type="button" className={styles.copyRow} title="Copy name" onClick={() => copy(user.name, 'name')}>
          <span className={styles.name} style={{ color }}>
            {base}
            {tag && <span className={styles.tag}>{tag}</span>}
          </span>
          {looksClaimed(user.name) && <VerifiedBadge size={16} />}
          <CopyIcon />
        </button>
        {user.address && (
          <button type="button" className={`${styles.copyRow} ${styles.addrRow}`} title="Copy address" onClick={() => copy(user.address, 'address')}>
            <span className={styles.addr}>{shortAddr(user.address)}</span>
            <CopyIcon />
          </button>
        )}
        {copied && <span className={styles.copied}>Copied {copied}</span>}
        {context !== 'blocked' && cta}
      </div>

      {hasMenu && (
        <div className={styles.menu}>
          <div className={styles.divider} />
          {community ? (
            <>
              {viewProfile}
              {message}
              {call}
              {communityActions.length > 0 && <div className={styles.divider} />}
              {communityActions.map((a, i) =>
                a.disabledReason != null ? (
                  // aria-disabled, not disabled: it stays focusable, so the reason is reachable without a mouse
                  <Tooltip key={a.label} label={a.disabledReason} side="bottom" multiline className={styles.rowTip}>
                    <button type="button" className={styles.row} aria-disabled="true" aria-describedby={`${reasonId}-${i}`}>
                      {a.icon}
                      <span>{a.label}</span>
                    </button>
                    <span id={`${reasonId}-${i}`} className={styles.srOnly}>{a.disabledReason}</span>
                  </Tooltip>
                ) : (
                  <button type="button" key={a.label} className={styles.row} onClick={() => { onClose(); a.onClick() }}>
                    {a.icon}
                    <span>{a.label}</span>
                  </button>
                )
              )}
              {hasDestructive && <div className={styles.divider} />}
              {block}
              {report}
              {invite}
            </>
          ) : context === 'request' ? (
            <>
              {viewProfile}
              {hasDestructive && <div className={styles.divider} />}
              {block}
              {report}
            </>
          ) : context === 'blocked' ? (
            viewProfile
          ) : (
            <>
              {message}
              {call}
              {showMention && (
                <button type="button" className={styles.row} onClick={() => { onMention?.(mentionName(user.name, user.address, peekProfile(user.address)?.hasClaimedName)); onClose() }}>
                  <MentionIcon />
                  <span>Mention</span>
                </button>
              )}
              {viewProfile}
              {invite}
              {hasDestructive && <div className={styles.divider} />}
              {report}
              {block}
            </>
          )}
        </div>
      )}
    </div>
  )
}
