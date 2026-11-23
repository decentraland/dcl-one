import { useId, useState, type ReactNode } from 'react'

import { Button, Skeleton, Spinner, Tabs, TabsBar } from '../../../index'
import {
  GeneralCheck,
  GeneralLock,
  GeneralPlayers,
  GeneralPrivate,
  GeneralPublic
} from '../../../foundations/icons'

import { useBrokenImage } from '../useBrokenImage'
import styles from './CommunityPage.module.css'

/** The page's brand gradient and column, under the Sites top bar. */
export function CommunityFrame({ children }: { children: ReactNode }) {
  return (
    <div className={styles.root}>
      <div className={styles.content}>{children}</div>
    </div>
  )
}

export function CommunityLoading() {
  return (
    <div className={styles.info} aria-busy="true" aria-label="Loading community">
      <Skeleton tone="neutral" className={styles.thumbnail} />
    </div>
  )
}

/** A face, or the name's initial when there is none or it fails to load. */
function Face({
  src,
  name,
  className
}: {
  src: string | undefined
  name: string
  className: string | undefined
}) {
  const { broken, ref, onError } = useBrokenImage(src)
  return src && !broken ? (
    <img ref={ref} className={className} src={src} alt="" loading="lazy" onError={onError} />
  ) : (
    <span className={className} aria-hidden="true">
      {name.charAt(0).toUpperCase()}
    </span>
  )
}

export type CommunityInfoCardProps = {
  name: string
  privacy: 'public' | 'private'
  /** "1.3K Members". */
  membersLabel: string
  thumbnailSrc: string
  owner: { name: string; face?: string | undefined }
  /** The member's call to action. */
  action: ReactNode
  /** Why the last membership change failed. */
  error?: string
  /** Shown only to whoever may see the community's content. */
  description?: string | null
}

/**
 * The community's card: image, name, privacy, members, owner, the call to action and the
 * description. A grid, so the description moves under the image row on a tablet, as source.
 */
export function CommunityInfoCard({
  name,
  privacy,
  membersLabel,
  thumbnailSrc,
  owner,
  action,
  error,
  description
}: CommunityInfoCardProps) {
  const titleId = useId()
  return (
    <section className={styles.info} aria-labelledby={titleId}>
      <CommunityThumbnail src={thumbnailSrc} name={name} />
      <div className={styles.heading}>
        <p className={styles.eyebrow}>Decentraland Community</p>
        <div className={styles.titleBlock}>
          <h1 id={titleId} className={styles.title}>
            {name}
          </h1>
          <p className={styles.meta}>
            <span className={styles.privacy}>
              {privacy === 'private' ? (
                <GeneralPrivate size={14} aria-hidden="true" />
              ) : (
                <GeneralPublic size={14} aria-hidden="true" />
              )}
              {privacy === 'private' ? 'Private' : 'Public'}
            </span>
            <span className={styles.divider} aria-hidden="true" />
            <span>{membersLabel}</span>
          </p>
        </div>
        <p className={styles.owner}>
          <Face className={styles.ownerAvatar} src={owner.face} name={owner.name} />
          By <span className={styles.ownerName}>{owner.name}</span>
        </p>
        <div className={styles.actions}>{action}</div>
        {error ? (
          <p className={styles.error} role="alert">
            {error}
          </p>
        ) : null}
      </div>
      {description ? <p className={styles.description}>{description}</p> : null}
    </section>
  )
}

function CommunityThumbnail({ src, name }: { src: string; name: string }) {
  const { broken, ref, onError } = useBrokenImage(src)
  return (
    <div className={styles.thumbnail}>
      {broken ? (
        <GeneralPlayers className={styles.thumbnailFallback} size={96} aria-hidden="true" />
      ) : (
        <img ref={ref} className={styles.thumbnailImage} src={src} alt={name} onError={onError} />
      )}
    </div>
  )
}

export type CommunityMembershipState =
  'joined' | 'signed-out' | 'loading' | 'error' | 'join' | 'request' | 'cancel'

export type CommunityMembershipActionProps = {
  state: CommunityMembershipState
  /** A join, request or cancel is on its way. */
  pending: boolean
  onSignIn: () => void
  onRetry: () => void
  onJoin: () => void
  onRequest: () => void
  onCancel: () => void
}

/** What the visitor can do here: sign in, join, ask to join, withdraw the request, or nothing. */
export function CommunityMembershipAction({
  state,
  pending,
  onSignIn,
  onRetry,
  onJoin,
  onRequest,
  onCancel
}: CommunityMembershipActionProps) {
  switch (state) {
    case 'joined':
      return (
        <Button
          className={styles.cta}
          variant="outline"
          size="md"
          disabled
          startIcon={<GeneralCheck size={16} aria-hidden="true" />}
        >
          Joined
        </Button>
      )
    case 'signed-out':
      return (
        <Button className={styles.cta} size="md" onClick={onSignIn}>
          Sign in to join
        </Button>
      )
    case 'loading':
      return (
        <Button className={styles.cta} variant="secondary" size="md" disabled loading>
          Loading...
        </Button>
      )
    case 'error':
      return (
        <>
          <Button className={styles.cta} variant="secondary" size="md" onClick={onRetry}>
            Try again
          </Button>
          <p className={styles.error} role="alert">
            We couldn&apos;t check your membership right now.
          </p>
        </>
      )
    case 'join':
      return (
        <Button
          className={styles.cta}
          size="md"
          onClick={onJoin}
          disabled={pending}
          loading={pending}
        >
          Join
        </Button>
      )
    case 'request':
      return (
        <Button
          className={styles.cta}
          variant="secondary"
          size="md"
          onClick={onRequest}
          disabled={pending}
          loading={pending}
        >
          Request to join
        </Button>
      )
    case 'cancel':
      return (
        <Button
          className={styles.cta}
          variant="secondary"
          size="md"
          onClick={onCancel}
          disabled={pending}
          loading={pending}
        >
          Cancel request
        </Button>
      )
  }
}

/** A private community waits for the member's own read before deciding they cannot see it. */
export function CommunityDeciding() {
  return (
    <div className={styles.deciding}>
      <Spinner variant="light" label="Loading community" />
    </div>
  )
}

export function CommunityPrivateNotice() {
  const titleId = useId()
  return (
    <section className={styles.private} aria-labelledby={titleId}>
      <GeneralLock className={styles.privateIcon} size={72} aria-hidden="true" />
      <h2 id={titleId} className={styles.privateTitle}>
        This is a Private Community
      </h2>
      <p className={styles.privateText}>
        You must be a member of this Community to see its profile.
      </p>
    </section>
  )
}

type Section = 'members' | 'events'

/** Members and upcoming events side by side from 991px; below that, tabs pick which one shows. */
export function CommunitySections({ members, events }: { members: ReactNode; events: ReactNode }) {
  const ids = useId()
  const [section, setSection] = useState<Section>('members')
  const tabId = (value: Section) => `${ids}-tab-${value}`
  const panelId = (value: Section) => `${ids}-panel-${value}`
  const tab = (value: Section, label: string) => (
    <Tabs
      id={tabId(value)}
      label={label}
      selected={section === value}
      device="desktop"
      aria-controls={panelId(value)}
      onClick={() => setSection(value)}
    />
  )
  const panelClass = (value: Section) =>
    [styles.panel, styles[value], section === value ? undefined : styles.inactive]
      .filter(Boolean)
      .join(' ')
  return (
    <div className={styles.sections}>
      <TabsBar label="Community sections" inset="content" className={styles.tabs}>
        {tab('members', 'Members')}
        {tab('events', 'Upcoming Events')}
      </TabsBar>
      <div className={styles.columns}>
        <div
          id={panelId('members')}
          role="tabpanel"
          className={panelClass('members')}
          aria-labelledby={tabId('members')}
        >
          {members}
        </div>
        <div
          id={panelId('events')}
          role="tabpanel"
          className={panelClass('events')}
          aria-labelledby={tabId('events')}
        >
          {events}
        </div>
      </div>
    </div>
  )
}
