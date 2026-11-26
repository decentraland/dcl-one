import { Button, CopyFeedback, Modal, ProfilePic, Spinner } from '../../../index'
import { GeneralPlayers, MenusShare } from '../../../foundations/icons'

import type { ComponentType, MouseEvent, ReactNode } from 'react'
import styles from './ProfileOwnerActions.module.css'

type Friend = { key: string; href: string; name: string; picture?: string }
type FriendLink = ComponentType<{
  to: string
  className?: string
  onClick?: (event: MouseEvent<HTMLAnchorElement>) => void
  children?: ReactNode
}>

export function ProfileFriendsDialog({
  friends,
  pending,
  failed,
  loadingMore,
  showMore,
  moreFailed,
  close,
  onRetry,
  onMore,
  linkComponent: Link
}: {
  friends?: readonly Friend[]
  pending: boolean
  failed: boolean
  loadingMore: boolean
  showMore: boolean
  moreFailed: boolean
  close: () => void
  onRetry: () => void
  onMore: () => void
  linkComponent: FriendLink
}) {
  return (
    <Modal title="Friends" onClose={close}>
      {pending ? <Spinner label="Loading friends" /> : null}
      {failed ? (
        <div role="status">
          <p>Friends are unavailable right now.</p>
          <Button variant="outline" size="sm" onClick={onRetry}>
            Try again
          </Button>
        </div>
      ) : null}
      {friends?.length === 0 ? <p>You don't have any friends yet.</p> : null}
      {friends?.length ? (
        <ul className={styles.friends}>
          {friends.map((friend) => (
            <li key={friend.key}>
              <Link to={friend.href} className={styles.friend} onClick={close}>
                <ProfilePic name={friend.name} src={friend.picture} size={32} alt="" />
                {friend.name}
              </Link>
            </li>
          ))}
        </ul>
      ) : null}
      {showMore ? (
        <div className={styles.more}>
          {moreFailed ? <p role="status">More friends could not be loaded.</p> : null}
          <Button variant="outline" size="sm" loading={loadingMore} onClick={onMore}>
            {moreFailed ? 'Try again' : 'Load more'}
          </Button>
        </div>
      ) : null}
    </Modal>
  )
}

export function ProfileFriendsButton({ count, onOpen }: { count: number; onOpen: () => void }) {
  return (
    <Button
      variant="outline"
      size="md"
      leadingIcon={<GeneralPlayers size={18} aria-hidden="true" />}
      onClick={onOpen}
    >
      {count === 1 ? '1 Friend' : `${count} Friends`}
    </Button>
  )
}
export function ProfileInviteButton({ href }: { href: string }) {
  return (
    <CopyFeedback
      value={href}
      successAnnouncement="Invite link copied."
      errorAnnouncement="Couldn't copy the invite link."
      showTooltip={false}
    >
      {(copy, state) => (
        <Button
          variant="outline"
          size="md"
          leadingIcon={<MenusShare size={18} aria-hidden="true" />}
          onClick={copy}
        >
          {state === 'copied'
            ? 'Invite Copied'
            : state === 'failed'
              ? 'Copy failed'
              : 'Invite Friends'}
        </Button>
      )}
    </CopyFeedback>
  )
}
