import { Button, CopyFeedback, JumpInCTA, Spinner, Tag } from '../../../index'
import {
  GeneralCheck,
  GeneralCopy,
  GeneralPlayers,
  NavbarCommunities
} from '../../../foundations/icons'

import { EmptyMessage } from './EmptyMessage'
import { useBrokenImage } from '../useBrokenImage'
import styles from './ProfileCommunities.module.css'

const OWNER_EMPTY_TITLE = 'No communities yet'
const MEMBER_EMPTY = 'This member has not joined any communities yet.'

function countLabel(shown: number, total: number): string {
  const noun = (n: number) => (n === 1 ? '1 community' : `${n} communities`)
  return total > shown ? `Showing ${shown} of ${noun(total)}` : noun(shown)
}

type CommunityCardInfo = {
  id: string
  picture: string
  href: string
  name: string
  joined: boolean
  role?: string
  privacy?: 'private' | 'public'
  members?: string
}

function CommunityCard({ community }: { community: CommunityCardInfo }) {
  const { href, picture, joined } = community
  const { broken, ref, onError } = useBrokenImage(picture)
  const origin = typeof window === 'undefined' ? '' : window.location.origin
  return (
    <li className={styles.card}>
      <div className={styles.media}>
        {broken ? (
          <NavbarCommunities size={48} aria-hidden="true" />
        ) : (
          <img
            ref={ref}
            className={styles.picture}
            src={picture}
            alt=""
            loading="lazy"
            decoding="async"
            onError={onError}
          />
        )}
        {community.role ? (
          <div className={styles.role}>
            <Tag variant="role" appearance="emphasized">
              {community.role}
            </Tag>
          </div>
        ) : null}
        {community.members !== undefined ? (
          <span className={styles.members}>
            <GeneralPlayers size={16} aria-hidden="true" />
            {community.members}
          </span>
        ) : null}
      </div>
      <div className={styles.body}>
        <h3 className={styles.name}>{community.name}</h3>
        {community.privacy ? (
          <span className={styles.visuallyHidden}>
            {community.privacy === 'private' ? 'Private' : 'Public'}
          </span>
        ) : null}
        <div className={styles.actions}>
          <Button
            variant="outline"
            size="sm"
            href={href}
            aria-label={`${joined ? 'Joined' : 'View'} ${community.name}`}
            leadingIcon={joined ? <GeneralCheck size={16} aria-hidden="true" /> : undefined}
          >
            {joined ? 'Joined' : 'View'}
          </Button>
          <CopyFeedback
            value={`${origin}${href}`}
            successAnnouncement="Community link copied."
            errorAnnouncement="Couldn't copy the community link."
          >
            {(copy, state) => (
              <Button
                variant="iconOnly"
                size="sm"
                aria-label={`Copy link to ${community.name}`}
                onClick={copy}
              >
                {state === 'copied' ? (
                  <GeneralCheck size={16} aria-hidden="true" />
                ) : (
                  <GeneralCopy size={16} aria-hidden="true" />
                )}
              </Button>
            )}
          </CopyFeedback>
        </div>
      </div>
    </li>
  )
}

export function ProfileCommunities({
  own,
  communities,
  total,
  failed,
  retry,
  exploreHref
}: {
  own: boolean
  communities?: readonly CommunityCardInfo[]
  total?: number
  failed: boolean
  retry: () => void
  exploreHref: string
}) {
  const empty = !failed && communities?.length === 0
  return (
    <div className={styles.root}>
      {/* One live region, always mounted, so a screen reader hears how loading ended. */}
      <p className={communities?.length ? styles.count : styles.status} role="status">
        {failed ? (
          'Communities are unavailable right now.'
        ) : !communities ? (
          <>
            <Spinner variant="light" className={styles.spinner} aria-hidden="true" />
            <span className={styles.visuallyHidden}>Loading communities</span>
          </>
        ) : communities.length > 0 ? (
          countLabel(communities.length, total ?? communities.length)
        ) : own ? (
          // The empty message below shows this title; the region only announces it.
          <span className={styles.visuallyHidden}>{OWNER_EMPTY_TITLE}</span>
        ) : (
          <span className={styles.note}>{MEMBER_EMPTY}</span>
        )}
      </p>
      {failed ? (
        <div className={styles.errorAction}>
          <Button variant="secondary" size="md" onClick={retry}>
            Try again
          </Button>
        </div>
      ) : null}
      {empty && own ? (
        <EmptyMessage
          icon={<NavbarCommunities size={56} />}
          title={OWNER_EMPTY_TITLE}
          subtitle="Jump into Decentraland and join the community that better aligns with you. You can also create your own!"
          action={<JumpInCTA href={exploreHref} size="md" label="Explore communities" />}
        />
      ) : null}
      {communities?.length ? (
        <ul className={styles.grid}>
          {communities.map((community) => (
            <CommunityCard key={community.id} community={community} />
          ))}
        </ul>
      ) : null}
    </div>
  )
}
