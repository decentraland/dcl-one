import type { ReactNode } from 'react'
import { Button, CopyFeedback, FriendCTA, Toast, type FriendCTAStatus } from '../../../index'
import {
  GeneralCheck,
  GeneralForbidden,
  GeneralPlayers,
  MenusShare
} from '../../../foundations/icons'

import styles from './ProfileMemberActionControls.module.css'
import { ProfileMoreActions, type MoreAction } from './ProfileMoreActions'

type Friendship =
  'none' | 'request_sent' | 'request_received' | 'friends' | 'blocked' | 'blocked_by'
type FriendshipAction = 'request' | 'cancel' | 'accept' | 'reject' | 'remove'
export type MemberActionNotice = { kind: 'status' | 'error'; text: string }

const ICON = { size: 18, 'aria-hidden': true } as const

type ProfileMemberActionProps = {
  signedIn: boolean
  state?: Friendship
  friendshipFailed: boolean
  blockingFailed: boolean
  blockListReady: boolean
  retrying: boolean
  mutual?: number
  pendingAction?: FriendshipAction
  blockingPending: boolean
  shareUrl: string
  notice: MemberActionNotice | null
  retry: () => void
  act: (action: FriendshipAction) => void
  setBlock: (next: boolean) => void
  onSignIn: () => void
  onDismissNotice: () => void
}

export function ProfileMemberActionControls({
  signedIn,
  state,
  friendshipFailed,
  blockingFailed,
  blockListReady,
  retrying,
  mutual,
  pendingAction,
  blockingPending,
  shareUrl,
  notice,
  retry,
  act,
  setBlock,
  onSignIn,
  onDismissNotice
}: ProfileMemberActionProps) {
  const blocked = state === 'blocked'
  const blockedBy = state === 'blocked_by'
  const busy = pendingAction !== undefined || blockingPending
  const tryAgain = (
    <Button variant="secondary" size="md" loading={retrying} onClick={retry}>
      Try again
    </Button>
  )

  /** Whether this friendship write is the one in progress; no second write starts meanwhile. */
  const running = (run: FriendshipAction) => pendingAction === run
  const cta = (status: FriendCTAStatus, run: FriendshipAction, name?: string) => (
    <FriendCTA
      status={status}
      aria-label={name}
      loading={running(run)}
      disabled={busy && !running(run)}
      onClick={() => act(run)}
    />
  )

  let friend: ReactNode
  if (!signedIn) {
    friend = <FriendCTA status="add" onClick={onSignIn} />
  } else if (state === undefined) {
    friend = friendshipFailed ? (
      <>
        <span className={styles.label}>Friendship unavailable</span>
        {tryAgain}
      </>
    ) : (
      <FriendCTA status="add" loading aria-label="Loading friendship" />
    )
  } else if (state === 'request_sent') {
    friend = cta('sent', 'cancel')
  } else if (state === 'request_received') {
    friend = (
      <>
        {/* The design system's friendship button has no Accept state. */}
        <Button
          size="md"
          leadingIcon={<GeneralCheck {...ICON} />}
          aria-label="Accept friend request"
          loading={running('accept')}
          disabled={busy && !running('accept')}
          onClick={() => act('accept')}
        >
          Accept
        </Button>
        {cta('reject', 'reject', 'Reject friend request')}
      </>
    )
  } else if (state === 'friends') {
    friend = cta('friend', 'remove')
  } else if (state === 'blocked') {
    // Unblock is the action of the blocked state; without the block lists it cannot run.
    friend = (
      <FriendCTA
        status="blocked"
        loading={blockingPending}
        disabled={pendingAction !== undefined || !blockListReady}
        onClick={() => setBlock(false)}
      />
    )
  } else if (state === 'blocked_by') {
    // A member who blocked the visitor takes no friend request.
    friend = null
  } else {
    friend = cta('add', 'request')
  }

  /** The menu's items. Without the block lists it cannot say Block or Unblock; a signed-out visitor has no block. */
  const more: MoreAction[] = [
    ...(signedIn && !blockingFailed
      ? [
          {
            label: blocked ? 'Unblock' : 'Block',
            icon: <GeneralForbidden {...ICON} />,
            disabled: busy || !blockListReady,
            onSelect: () => setBlock(!blocked)
          }
        ]
      : [])
  ]

  return (
    <div className={styles.root}>
      <div className={styles.controls}>
        {/* Members who blocked one another see no shared friends. */}
        {mutual !== undefined && mutual > 0 && !blocked && !blockedBy ? (
          <span className={styles.label}>
            <GeneralPlayers {...ICON} />
            {`${mutual} Mutual`}
          </span>
        ) : null}
        {friend}
        {/* One Try again serves both failed reads. */}
        {signedIn && blockingFailed && !friendshipFailed ? (
          <>
            <span className={styles.label}>Block unavailable</span>
            {tryAgain}
          </>
        ) : null}
        <CopyFeedback
          as="div"
          value={shareUrl}
          successAnnouncement="Profile link copied"
          errorAnnouncement="Couldn't copy the link"
        >
          {(copy) => (
            <ProfileMoreActions
              actions={[
                ...more,
                {
                  label: 'Share',
                  icon: <MenusShare {...ICON} />,
                  onSelect: () => {
                    onDismissNotice()
                    copy()
                  }
                }
              ]}
            />
          )}
        </CopyFeedback>
      </div>
      {/* The status region stays mounted, so a screen reader hears each result. An alert is announced when it appears, and stays until it is closed. */}
      <Toast onClose={onDismissNotice}>{notice?.kind === 'status' ? notice.text : null}</Toast>
      {notice?.kind === 'error' ? (
        <Toast role="alert" variant="error" duration={0} closeButton onClose={onDismissNotice}>
          {notice.text}
        </Toast>
      ) : null}
    </div>
  )
}
