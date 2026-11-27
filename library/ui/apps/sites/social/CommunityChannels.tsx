import { useId, useState, type ReactNode } from 'react'

import { Button, CopyFeedback, ProfilePic, SearchBar } from '../../../index'
import { GeneralVerified, MenusLink } from '../../../foundations/icons'

import { useBrokenImage } from '../useBrokenImage'
import {
  capitalized,
  fullDate,
  isImageUrl,
  relativeDate,
  shortAddress,
  type HubPostView
} from './hub-format'
import { PostMessage } from './CommunityAnnouncements'
import { CommunityThumb, HubLoading, type HubCommunityView } from './SocialHub'
import styles from './CommunityChannels.module.css'

/** A place a community shares. */
export type HubPlaceView = {
  id: string
  title?: string | null
  description?: string | null
  image?: string | null
  location?: string | null
  /** Who shared it and when, where the API says. */
  addedBy?: string | null
  addedAt?: string | null
}

/** A member as the community's directory lists them. */
export type HubMemberView = {
  memberAddress: string
  name?: string | null
  role: string
  profilePictureUrl?: string | null
  hasClaimedName?: boolean
}

/** Where the place is, who shared it and when, as far as each is known: "-66,-56 · Added by 0x12…ab · 2 months ago". */
function placeLine(place: HubPlaceView, now?: number): string {
  return [
    place.location,
    place.addedBy ? `Added by ${shortAddress(place.addedBy)}` : null,
    place.addedAt ? relativeDate(place.addedAt, now) : null
  ]
    .filter(Boolean)
    .join(' · ')
}

function PlaceArt({ place, className }: { place: HubPlaceView; className: string | undefined }) {
  const src = isImageUrl(place.image) ? place.image : undefined
  const { broken, ref, onError } = useBrokenImage(src)
  return (
    <span className={className}>
      {src && !broken ? <img ref={ref} src={src} alt="" loading="lazy" onError={onError} /> : null}
    </span>
  )
}

function Card({
  title,
  action,
  children
}: {
  title: string
  action: ReactNode
  children: ReactNode
}) {
  const titleId = useId()
  return (
    <section className={styles.card} aria-labelledby={titleId}>
      <header className={styles.cardHead}>
        <h2 id={titleId}>{title}</h2>
        {action}
      </header>
      {children}
    </section>
  )
}

export type CommunityOverviewProps = {
  community: HubCommunityView & { visibility?: string | null }
  /** The reader is a member. */
  joined: boolean
  /** The reader's call to action: sign in, join, ask to join, withdraw, leave. */
  membershipAction: ReactNode
  /** Why the last membership change failed. */
  membershipError?: string
  /** The community's link, copied by Copy link. */
  link: string
  /** The reader may not read announcements until they join. */
  postsLocked: boolean
  latestPost: HubPostView | null
  latestPending: boolean
  /** Hidden when the community shares no places and the reader does not manage it. */
  showPlaces: boolean
  places: HubPlaceView[]
  placesPending: boolean
  onAnnouncements: () => void
  onHangouts: () => void
  now?: number
}

/** The community's overview: its banner and words, the member's action, its facts, latest announcement and places. */
export function CommunityOverview({
  community,
  joined,
  membershipAction,
  membershipError,
  link,
  postsLocked,
  latestPost,
  latestPending,
  showPlaces,
  places,
  placesPending,
  onAnnouncements,
  onHangouts,
  now
}: CommunityOverviewProps) {
  const visibility =
    community.visibility === 'unlisted' ? 'Unlisted' : capitalized(community.privacy)
  return (
    <div className={styles.page}>
      <CommunityThumb community={community} size="banner" />
      <div className={styles.summary}>
        <div>
          <h1>{community.name}</h1>
          {community.description ? <p>{community.description}</p> : null}
        </div>
        <div className={styles.summaryActions}>
          <div className={styles.membership}>
            {membershipAction}
            {membershipError ? (
              <span className={styles.error} role="alert">
                {membershipError}
              </span>
            ) : null}
          </div>
          <CopyFeedback
            value={link}
            showTooltip={false}
            successAnnouncement="Community link copied."
            errorAnnouncement="Couldn't copy the link."
          >
            {(copy, state) => (
              <Button
                variant="outline"
                size="md"
                leadingIcon={<MenusLink size={18} aria-hidden="true" />}
                onClick={copy}
              >
                {state === 'copied' ? 'Copied!' : state === 'failed' ? 'Copy failed' : 'Copy link'}
              </Button>
            )}
          </CopyFeedback>
        </div>
      </div>
      <dl className={styles.facts}>
        <div>
          <dt>Members</dt>
          <dd>{community.membersCount.toLocaleString('en-US')}</dd>
        </div>
        <div>
          <dt>Privacy</dt>
          <dd>{visibility}</dd>
        </div>
        <div>
          <dt>Your role</dt>
          <dd>{joined ? capitalized(community.role || 'member') : 'Not a member'}</dd>
        </div>
      </dl>
      <div className={showPlaces ? styles.columns : styles.column}>
        <Card
          title="Latest announcement"
          action={
            <Button variant="text" size="sm" onClick={onAnnouncements}>
              All announcements
            </Button>
          }
        >
          {postsLocked ? (
            <p className={styles.muted}>Join this community to read its announcements.</p>
          ) : latestPending ? (
            <p className={styles.muted} role="status">
              Loading announcements…
            </p>
          ) : latestPost ? (
            <PostMessage post={latestPost} now={now} />
          ) : (
            <p className={styles.muted}>No announcements yet.</p>
          )}
        </Card>
        {showPlaces ? (
          <Card
            title="Its places"
            action={
              <Button variant="text" size="sm" onClick={onHangouts}>
                All hangouts
              </Button>
            }
          >
            {placesPending ? (
              <p className={styles.muted} role="status">
                Loading shared places…
              </p>
            ) : places.length === 0 ? (
              <p className={styles.muted}>No places shared yet.</p>
            ) : (
              <ul className={styles.list}>
                {places.slice(0, 3).map((place) => (
                  <li key={place.id}>
                    <button type="button" className={styles.placeRow} onClick={onHangouts}>
                      <PlaceArt place={place} className={styles.placeThumb} />
                      <span className={styles.rowText}>
                        <strong>{place.title || place.id}</strong>
                        {placeLine(place) ? <small>{placeLine(place)}</small> : null}
                      </span>
                    </button>
                  </li>
                ))}
              </ul>
            )}
          </Card>
        ) : null}
      </div>
    </div>
  )
}

/** The places a community shares, with who added each and when. */
export function CommunityHangouts({
  name,
  places,
  pending,
  now
}: {
  name: string
  places: HubPlaceView[]
  pending: boolean
  now?: number
}) {
  return (
    <div className={styles.page}>
      <header className={styles.pageHead}>
        <h1>Hangouts</h1>
        <p>Places shared by {name}.</p>
      </header>
      {pending ? (
        <HubLoading label="Loading shared places" />
      ) : places.length === 0 ? (
        <p className={styles.muted}>No places shared yet.</p>
      ) : (
        <ul className={styles.hangouts}>
          {places.map((place) => (
            <li key={place.id} className={styles.hangout}>
              <PlaceArt place={place} className={styles.hangoutArt} />
              <span className={styles.rowText}>
                <strong title={place.title || place.id}>{place.title || place.id}</strong>
                {place.description ? (
                  <span className={styles.clamp}>{place.description}</span>
                ) : null}
                {placeLine(place, now) ? (
                  <small title={fullDate(place.addedAt)}>{placeLine(place, now)}</small>
                ) : null}
              </span>
            </li>
          ))}
        </ul>
      )}
    </div>
  )
}

/** Everyone in the community, searchable by name or address, with the managers' actions on each. */
export function CommunityDirectory({
  name,
  members,
  pending,
  viewer,
  actions
}: {
  name: string
  members: HubMemberView[]
  pending: boolean
  viewer: string | null
  /** A manager's actions on one member; nothing when they cannot change that member. */
  actions?: (member: HubMemberView) => ReactNode
}) {
  const [query, setQuery] = useState('')
  const term = query.trim().toLowerCase()
  const shown = members.filter(
    (member) =>
      !term ||
      (member.name ?? '').toLowerCase().includes(term) ||
      member.memberAddress.toLowerCase().includes(term)
  )
  return (
    <div className={styles.page}>
      <header className={styles.pageHead}>
        <div>
          <h1>Members</h1>
          <p>Everyone in {name}.</p>
        </div>
        <SearchBar
          className={styles.search}
          label="Search members"
          placeholder="Search member or name"
          value={query}
          onValueChange={setQuery}
          onClear={() => setQuery('')}
        />
      </header>
      {pending ? (
        <HubLoading label="Loading members" />
      ) : shown.length === 0 ? (
        <p className={styles.muted}>
          {term ? 'No members match your search.' : 'No members to show.'}
        </p>
      ) : (
        <ul className={styles.members}>
          {shown.map((member) => {
            const label = member.name || shortAddress(member.memberAddress)
            const self = !!viewer && member.memberAddress.toLowerCase() === viewer.toLowerCase()
            return (
              <li
                key={member.memberAddress}
                className={styles.member}
                aria-label={`${member.name || 'Member'} ${member.memberAddress}`}
              >
                <ProfilePic
                  name={label}
                  src={isImageUrl(member.profilePictureUrl) ? member.profilePictureUrl : undefined}
                  alt=""
                  size={40}
                />
                <span className={styles.rowText}>
                  <strong>
                    <span className={styles.truncate}>{label}</span>
                    {member.hasClaimedName ? (
                      <GeneralVerified size={14} aria-label="verified" />
                    ) : null}
                  </strong>
                  <small>{capitalized(member.role)}</small>
                </span>
                {self ? <span className={styles.you}>You</span> : actions?.(member)}
              </li>
            )
          })}
        </ul>
      )}
    </div>
  )
}

/** A channel a private community keeps for its members. */
export function CommunityLockedChannel({ channel }: { channel: string }) {
  return (
    <div className={styles.page}>
      <header className={styles.pageHead}>
        <h1>{channel}</h1>
      </header>
      <p className={styles.muted}>Join this community to view its {channel.toLowerCase()}.</p>
    </div>
  )
}

/** The community could not be opened, or is still opening. */
export function CommunityUnavailable({ pending }: { pending: boolean }) {
  return (
    <div className={styles.page}>
      {pending ? (
        <HubLoading label="Opening community" />
      ) : (
        <p className={styles.muted}>Couldn&apos;t load this community.</p>
      )}
    </div>
  )
}
