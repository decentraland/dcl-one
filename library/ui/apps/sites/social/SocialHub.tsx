import {
  useId,
  useState,
  type AnchorHTMLAttributes,
  type ComponentType,
  type ReactNode
} from 'react'

import { Button, CommunityCard, Modal, SearchBar, Spinner } from '../../../index'
import { MenusAdd } from '../../../foundations/icons'

import { useBrokenImage } from '../useBrokenImage'
import { capitalized, isImageUrl, membersCount as members, shortAddress } from './hub-format'
import styles from './SocialHub.module.css'

/** A community as the hub lists it. */
export type HubCommunityView = {
  id: string
  name: string
  description?: string | null
  thumbnailUrl?: string | null
  privacy: 'public' | 'private'
  membersCount: number
  /** The reader's role; null when they are not a member. */
  role?: string | null
  ownerName?: string | null
}

/** The router's link, passed in so the hub's community links navigate under any router. */
export type HubLinkComponent = ComponentType<
  AnchorHTMLAttributes<HTMLAnchorElement> & { href: string }
>

function Anchor(props: AnchorHTMLAttributes<HTMLAnchorElement> & { href: string }) {
  return <a {...props} />
}

const THUMB_SIZES = { nav: styles.thumbNav, list: styles.thumbList, banner: styles.thumbBanner }

/** The community's image, or its initials when it has none or it fails to load. */
export function CommunityThumb({
  community,
  size
}: {
  community: Pick<HubCommunityView, 'name' | 'thumbnailUrl'>
  size: 'nav' | 'list' | 'banner'
}) {
  const src = isImageUrl(community.thumbnailUrl) ? community.thumbnailUrl : undefined
  const { broken, ref, onError } = useBrokenImage(src)
  return (
    <span className={`${styles.thumb} ${THUMB_SIZES[size]}`}>
      {src && !broken ? (
        <img ref={ref} src={src} alt="" loading="lazy" onError={onError} />
      ) : (
        <span aria-hidden="true">{community.name.trim().slice(0, 2).toUpperCase()}</span>
      )}
    </span>
  )
}

/** A count beside a navigation label, such as a channel's members. */
export function NavCount({ children }: { children: ReactNode }) {
  return <span className={styles.navCount}>{children}</span>
}

/** A navigation label: icon or image, text and an optional count. */
export function NavLabel({
  icon,
  children,
  count
}: {
  icon: ReactNode
  children: ReactNode
  count?: ReactNode
}) {
  return (
    <>
      {icon}
      <span className={styles.navText}>{children}</span>
      {count != null ? <NavCount>{count}</NavCount> : null}
    </>
  )
}

/** The hub's main column: its heading bar, an optional search, then the section or channel. */
export function SocialPanel({
  label,
  heading,
  subheading,
  icon,
  search,
  children
}: {
  /** The region's accessible name. */
  label: string
  heading: string
  subheading?: string | null
  icon: ReactNode
  search?: { label: string; value: string; onChange: (value: string) => void }
  children: ReactNode
}) {
  return (
    <section className={styles.panel} aria-label={label}>
      <header className={styles.bar}>
        <span className={styles.barIcon} aria-hidden="true">
          {icon}
        </span>
        <div className={styles.barText}>
          <strong>{heading}</strong>
          {subheading ? <small>{subheading}</small> : null}
        </div>
        {search ? (
          <SearchBar
            className={styles.barSearch}
            label={search.label}
            placeholder={search.label}
            value={search.value}
            onValueChange={search.onChange}
            onClear={() => search.onChange('')}
          />
        ) : null}
      </header>
      <div className={styles.body}>{children}</div>
    </section>
  )
}

/** A failed list with its retry. */
export function HubRetry({ message, onRetry }: { message: string; onRetry?: () => void }) {
  return (
    <div className={styles.retry} role="alert">
      <p>{message}</p>
      {onRetry ? (
        <Button variant="outline" size="sm" onClick={onRetry}>
          Try again
        </Button>
      ) : null}
    </div>
  )
}

/** A loading line, announced politely. */
export function HubLoading({ label }: { label: string }) {
  return (
    <div className={styles.loading}>
      <Spinner variant="light" size={20} label={label} />
      <span>{label}</span>
    </div>
  )
}

function CommunityRow({
  community,
  href,
  link: Link,
  detail
}: {
  community: HubCommunityView
  href: string
  link: HubLinkComponent
  detail: string
}) {
  return (
    <Link className={styles.row} href={href}>
      <CommunityThumb community={community} size="list" />
      <span className={styles.rowText}>
        <strong>{community.name}</strong>
        <small>{detail}</small>
        {community.description ? (
          <span className={styles.clamp}>{community.description}</span>
        ) : null}
      </span>
    </Link>
  )
}

export type SocialHomeProps = {
  /** The communities the reader belongs to; the hub lists them before any other. */
  mine: HubCommunityView[]
  communities: HubCommunityView[]
  pending: boolean
  failed: boolean
  onRetry: () => void
  requestsCount: number
  onRequests: () => void
  onExplore: () => void
  onCreate: () => void
  communityHref: (id: string) => string
  linkComponent?: HubLinkComponent
}

/** The hub's front page: the reader's communities (or some to discover), invitations and the way to start one. */
export function SocialHome({
  mine,
  communities,
  pending,
  failed,
  onRetry,
  requestsCount,
  onRequests,
  onExplore,
  onCreate,
  communityHref,
  linkComponent = Anchor
}: SocialHomeProps) {
  const titleId = useId()
  const rows = (mine.length ? mine : communities).slice(0, 4)
  return (
    <div className={styles.page}>
      <header className={styles.pageHead}>
        <div>
          <h1>Communities</h1>
          <p>The communities you belong to, and new ones to join.</p>
        </div>
        <Button
          variant="primary"
          size="md"
          leadingIcon={<MenusAdd size={18} aria-hidden="true" />}
          onClick={onCreate}
        >
          Create a community
        </Button>
      </header>
      <section className={styles.card} aria-labelledby={titleId}>
        <header className={styles.cardHead}>
          <h2 id={titleId}>{mine.length ? 'Your communities' : 'Discover communities'}</h2>
          {requestsCount > 0 ? (
            <Button variant="text" size="sm" onClick={onRequests}>
              Invitations ({requestsCount})
            </Button>
          ) : null}
          <Button variant="text" size="sm" onClick={onExplore}>
            Discover
          </Button>
        </header>
        {pending && !rows.length ? <HubLoading label="Loading communities" /> : null}
        {failed && !rows.length ? (
          <HubRetry message="Couldn't load communities." onRetry={onRetry} />
        ) : null}
        {rows.length ? (
          <ul className={styles.rows}>
            {rows.map((community) => (
              <li key={community.id}>
                <CommunityRow
                  community={community}
                  href={communityHref(community.id)}
                  link={linkComponent}
                  detail={`${members(community.membersCount)}${community.privacy === 'private' ? ' · Private' : ''}`}
                />
              </li>
            ))}
          </ul>
        ) : null}
        {!pending && !failed && !rows.length ? (
          <div className={styles.empty}>
            <h3>No communities yet</h3>
            <p>Start a community and bring your people together.</p>
          </div>
        ) : null}
      </section>
    </div>
  )
}

export type SocialExploreProps = {
  mine: HubCommunityView[]
  communities: HubCommunityView[]
  /** What the reader searched; the reader's own communities hide while they search. */
  query: string
  pending: boolean
  failed: boolean
  onRetry: () => void
  onLoadMore?: () => void
  loadingMore: boolean
  communityHref: (id: string) => string
  linkComponent?: HubLinkComponent
}

/** Every community the reader can discover, with theirs first, a page at a time. */
export function SocialExplore({
  mine,
  communities,
  query,
  pending,
  failed,
  onRetry,
  onLoadMore,
  loadingMore,
  communityHref,
  linkComponent = Anchor
}: SocialExploreProps) {
  const mineId = useId()
  const allId = useId()
  const searching = query.trim() !== ''
  return (
    <div className={styles.page}>
      {!searching && mine.length > 0 ? (
        <section aria-labelledby={mineId}>
          <h2 className={styles.heading} id={mineId}>
            Your communities
          </h2>
          <ul className={styles.mine}>
            {mine.map((community) => (
              <li key={community.id}>
                <CommunityRow
                  community={community}
                  href={communityHref(community.id)}
                  link={linkComponent}
                  detail={`${members(community.membersCount)} · ${capitalized(community.role || 'member')}`}
                />
              </li>
            ))}
          </ul>
        </section>
      ) : null}
      <section aria-labelledby={allId} aria-busy={pending || undefined}>
        <h2 className={styles.heading} id={allId}>
          Explore communities
        </h2>
        {pending ? (
          <ul className={styles.cards}>
            {Array.from({ length: 6 }, (_, index) => (
              <li key={index}>
                <CommunityCard loading loadingLabel="Loading communities" />
              </li>
            ))}
          </ul>
        ) : failed ? (
          <HubRetry message="Couldn't load communities." onRetry={onRetry} />
        ) : communities.length === 0 ? (
          <p className={styles.muted}>
            {searching ? 'No communities match your search.' : 'No communities to show.'}
          </p>
        ) : (
          <ul className={styles.cards}>
            {communities.map((community, index) => (
              <li key={community.id}>
                <CommunityCard
                  name={community.name}
                  href={communityHref(community.id)}
                  image={community.thumbnailUrl}
                  priority={index < 3}
                  ownerName={community.ownerName}
                  privacy={community.privacy}
                  membersLabel={`${members(community.membersCount)}${community.role ? ' · Joined' : ''}`}
                  description={community.description}
                  linkComponent={linkComponent}
                />
              </li>
            ))}
          </ul>
        )}
        {!pending && !failed && onLoadMore ? (
          <div className={styles.more}>
            <Button
              variant="outline"
              size="md"
              disabled={loadingMore}
              loading={loadingMore}
              onClick={onLoadMore}
            >
              {loadingMore ? 'Loading communities…' : 'Load more communities'}
            </Button>
          </div>
        ) : null}
      </section>
    </div>
  )
}

/** An invitation to a community or a request to join one. */
export type HubRequestView = {
  id: string
  communityId: string
  memberAddress: string
  type: 'invite' | 'request_to_join'
  /** The community's name for the member's own requests, the member's for a community's. */
  name?: string | null
}

export type HubDecision = 'accepted' | 'rejected' | 'cancelled'

export type SocialRequestsProps = {
  /** A community's requests, decided by its owner or moderators, instead of the member's own. */
  managing?: boolean
  /** Pending requests only. */
  rows: HubRequestView[]
  pending: boolean
  error?: string
  /** An answer or an invitation is on its way; every control waits for it. */
  busy: boolean
  onRetry: () => void
  onDecide: (request: HubRequestView, decision: HubDecision) => void
  /** Present for a community's managers: invites a wallet by address. */
  onInvite?: (address: string) => void
  /** Opens the community a request is about. */
  onOpen: (communityId: string) => void
}

const WALLET = /^0x[0-9a-fA-F]{40}$/

/** The member's invitations and requests, or a community's requests and its invitation form. */
export function SocialRequests({
  managing = false,
  rows,
  pending,
  error,
  busy,
  onRetry,
  onDecide,
  onInvite,
  onOpen
}: SocialRequestsProps) {
  const [address, setAddress] = useState('')
  const inputId = useId()
  const valid = WALLET.test(address.trim())
  return (
    <div className={styles.page}>
      <header className={styles.pageHead}>
        <div>
          <h1>{managing ? 'Membership requests' : 'Invitations & requests'}</h1>
        </div>
      </header>
      {error ? <HubRetry message={error} onRetry={onRetry} /> : null}
      {onInvite ? (
        <form
          className={styles.invite}
          onSubmit={(event) => {
            event.preventDefault()
            if (!busy && valid) onInvite(address.trim())
          }}
        >
          <label className={styles.field} htmlFor={inputId}>
            Member wallet address
            <input
              id={inputId}
              className={styles.input}
              value={address}
              onChange={(event) => setAddress(event.target.value)}
              placeholder="0x…"
              required
              autoComplete="off"
              spellCheck={false}
            />
          </label>
          <Button type="submit" variant="primary" size="md" disabled={busy || !valid}>
            Send invitation
          </Button>
        </form>
      ) : null}
      {pending ? (
        <HubLoading label="Loading membership requests" />
      ) : (
        <>
          {!rows.length && !error ? (
            <p className={styles.muted}>No pending invitations or requests.</p>
          ) : null}
          <ul className={styles.list}>
            {rows.map((row) => {
              const answer = managing ? row.type === 'request_to_join' : row.type === 'invite'
              return (
                <li
                  key={row.id}
                  className={styles.request}
                  aria-label={`${managing ? row.memberAddress : row.name || row.communityId}: ${row.type === 'invite' ? 'invitation' : 'join request'}`}
                >
                  <span className={styles.rowText}>
                    <strong>{row.name || shortAddress(row.memberAddress)}</strong>
                    <small title={row.memberAddress}>
                      {managing
                        ? row.memberAddress
                        : row.type === 'invite'
                          ? 'You are invited to join'
                          : 'Waiting for approval'}
                    </small>
                  </span>
                  <span className={styles.actions}>
                    {!managing ? (
                      <Button variant="outline" size="sm" onClick={() => onOpen(row.communityId)}>
                        Open community
                      </Button>
                    ) : null}
                    {answer ? (
                      <>
                        <Button
                          variant="primary"
                          size="sm"
                          disabled={busy}
                          onClick={() => onDecide(row, 'accepted')}
                        >
                          {managing ? 'Approve' : 'Accept invitation'}
                        </Button>
                        <Button
                          variant="outline"
                          size="sm"
                          disabled={busy}
                          onClick={() => onDecide(row, 'rejected')}
                        >
                          {managing ? 'Reject' : 'Decline invitation'}
                        </Button>
                      </>
                    ) : (
                      <Button
                        variant="outline"
                        size="sm"
                        disabled={busy}
                        onClick={() => onDecide(row, 'cancelled')}
                      >
                        {managing ? 'Cancel invitation' : 'Cancel request'}
                      </Button>
                    )}
                  </span>
                </li>
              )
            })}
          </ul>
        </>
      )}
    </div>
  )
}

/** A question before a change the member cannot take back by accident. */
export function HubConfirm({
  title,
  body,
  confirmLabel,
  cancelLabel = 'Cancel',
  danger = false,
  busy = false,
  error,
  onConfirm,
  onCancel
}: {
  title: string
  body: ReactNode
  confirmLabel: string
  cancelLabel?: string
  danger?: boolean
  /** The change is on its way: the dialog stays open and its controls wait. */
  busy?: boolean
  error?: string
  onConfirm: () => void
  onCancel: () => void
}) {
  return (
    <Modal
      title={title}
      onClose={onCancel}
      dismissible={!busy}
      closeButton={false}
      actions={
        <>
          <Button variant="outline" size="md" disabled={busy} onClick={onCancel}>
            {cancelLabel}
          </Button>
          <Button
            variant="primary"
            size="md"
            {...(danger ? { tone: 'danger' as const } : {})}
            disabled={busy}
            loading={busy}
            onClick={onConfirm}
          >
            {confirmLabel}
          </Button>
        </>
      }
    >
      <div className={styles.confirm}>{error ? <p role="alert">{error}</p> : <p>{body}</p>}</div>
    </Modal>
  )
}
