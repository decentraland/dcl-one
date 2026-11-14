// React Communities — a full-screen page inside the shared MainMenuShell (matches the
// Unity Communities screen): a left column with "My Communities" and a main browse grid
// of community cards. The grid reads 20 communities a page from the service,
// the next page read when the grid's end scrolls into view, and the search run on the server
// (a second after typing stops, or at once on Enter). Each card shows Public or Private and
// offers Join, Request to Join or Cancel (or Accept and Reject for an invite), the user's friends in
// it, and its description on hover. The sidebar's Invites & Requests opens the user's invites, the
// join requests their communities received and their own requests.

import { useEffect, useRef, useState } from 'react'
import { Avatar, Button, Chip, CountBadge, EmptyState, SearchField, Spinner, SwapLabel, Tooltip } from '../../design'
import { nameColor } from '@explorer-core/lib/identity'
import { MainMenuShell } from '../menu/MainMenuShell'
import { confirmRejectInvite, MemberRow, openCommunityModal, SECTION_LOAD_ERROR } from './CommunityModal'
import { RequestRowActions, type RequestAnswer } from './RequestRowActions'
import { openProfileCard } from '../profileCard/ProfileCard'
import { openPassport } from '../profile/Passport'
import { openCommunityCreateModal } from './CommunityCreateModal'
import type { Community, CommunityInvitesMessage, CommunityRequestMember, CommunityRequestsGroup } from '@explorer-core/engine/protocol'
import type { CommunitiesState, ProfileState, VoiceChatSession } from '../session/useEngineSession'
import { streamLive } from '../voiceChat/callGate'
import styles from './CommunitiesPage.module.css'
import { LoadMore } from './LoadMore'

/** The wait after the last keystroke before searching, so typing sends one read. */
export const SEARCH_DELAY_MS = 1000

function isMember(role: string): boolean {
  return role === 'owner' || role === 'moderator' || role === 'member'
}

function Globe(): React.JSX.Element {
  return (
    <svg viewBox="0 0 24 24" width="13" height="13" fill="none" aria-hidden="true">
      <circle cx="12" cy="12" r="9" stroke="currentColor" strokeWidth="1.7" />
      <path d="M3 12h18M12 3c2.5 2.5 2.5 15 0 18M12 3c-2.5 2.5-2.5 15 0 18" stroke="currentColor" strokeWidth="1.7" />
    </svg>
  )
}

function Lock(): React.JSX.Element {
  return (
    <svg viewBox="0 0 24 24" width="13" height="13" fill="none" aria-hidden="true">
      <rect x="5" y="11" width="14" height="9" rx="2" stroke="currentColor" strokeWidth="1.8" />
      <path d="M8 11V8a4 4 0 0 1 8 0v3" stroke="currentColor" strokeWidth="1.8" />
    </svg>
  )
}

interface CardActions {
  onJoin: (id: string) => void
  onRequestToJoin: (id: string) => void
  onCancelRequest: (id: string, requestId: string) => void
  onAcceptInvite: (id: string, inviteId: string) => void
  onRejectInvite: (id: string, inviteId: string) => void
}

// A member views; anyone invited accepts or rejects (asked first); anyone may join a public
// community; a private one takes a request, which can be cancelled while it waits.
function CardButton({ c, onOpen, onJoin, onRequestToJoin, onCancelRequest, onAcceptInvite, onRejectInvite }: CardActions & { c: Community; onOpen: () => void }): React.JSX.Element {
  const act = (run: () => void) => (e: React.MouseEvent): void => {
    e.stopPropagation()
    run()
  }
  const inviteId = !isMember(c.role) ? c.pendingInviteId : undefined
  if (inviteId != null) {
    return (
      <span className={styles.cardBtns}>
        <Button size="sm" className={styles.cardBtn} aria-label="Accept invite" onClick={act(() => onAcceptInvite(c.id, inviteId))}>Accept</Button>
        <Button
          size="sm"
          variant="secondary"
          className={styles.cardBtn}
          aria-label="Reject invite"
          onClick={act(() => void confirmRejectInvite(c).then((yes) => yes && onRejectInvite(c.id, inviteId)))}
        >
          Reject
        </Button>
      </span>
    )
  }
  if (isMember(c.role)) {
    return <Button size="sm" variant="ghost" className={styles.cardBtn} onClick={act(onOpen)}>View</Button>
  }
  if (c.privacy !== 'private') {
    return <Button size="sm" className={styles.cardBtn} onClick={act(() => onJoin(c.id))}>Join</Button>
  }
  const requestId = c.pendingRequestId
  if (requestId != null) {
    return (
      <Button size="sm" variant="ghost" className={styles.cardBtn} aria-label="Cancel join request" onClick={act(() => onCancelRequest(c.id, requestId))}>
        <SwapLabel idle="Request Sent" hover="Cancel Request" />
      </Button>
    )
  }
  return <Button size="sm" className={styles.cardBtn} onClick={act(() => onRequestToJoin(c.id))}>Request to Join</Button>
}

function CommunityCard({ c, live, onOpen, ...actions }: CardActions & { c: Community; live: boolean; onOpen: () => void }): React.JSX.Element {
  const [failed, setFailed] = useState(false)
  const showImg = c.thumbnail != null && c.thumbnail !== '' && !failed
  const isPrivate = c.privacy === 'private'
  const friends = c.friends ?? []
  return (
    // named by its whole text, as the e2e suite reads it (apps/bevy-explorer/e2e/specs/communities.test.ts)
    <div className={styles.card} onClick={onOpen} role="button" tabIndex={0}>
      <div className={styles.cover} style={{ background: showImg ? undefined : nameColor(c.id) }}>
        {live && <span className={styles.live} role="img" aria-label="Live stream">● LIVE</span>}
        {showImg ? (
          <img className={styles.coverImg} src={c.thumbnail} alt="" onError={() => setFailed(true)} />
        ) : (
          <span className={styles.coverInitial}>{c.name.charAt(0).toUpperCase()}</span>
        )}
      </div>
      <div className={styles.cardBody}>
        <span className={styles.cardName} title={c.name}>{c.name}</span>
        <div className={styles.ownerRow}>
          <span className={styles.cardOwner}>{c.ownerName}</span>
          {friends.length > 0 && (
            <span className={styles.friends} aria-label={`Friends in this community: ${friends.map((f) => f.name).join(', ')}`}>
              {friends.map((f) => (
                <Tooltip key={f.address} label={f.name} side="top" className={styles.friend}>
                  <Avatar src={f.picture} name={f.name} color={nameColor(f.address)} size={20} />
                </Tooltip>
              ))}
            </span>
          )}
        </div>
        {c.description !== '' && <p className={styles.cardDesc}>{c.description}</p>}
        <span className={styles.cardMeta}>
          {isPrivate ? <Lock /> : <Globe />} {isPrivate ? 'Private' : 'Public'} · {c.membersCount} Members
        </span>
        <CardButton c={c} onOpen={onOpen} {...actions} />
      </div>
    </div>
  )
}

/** The ⋮ menu on a request's row: the member menu without owner actions (View Profile, Chat,
 *  Call, Block, Report, Invite to Community). */
function openRequestMenu(member: CommunityRequestMember, anchor: HTMLElement, onClose: () => void): void {
  const r = anchor.getBoundingClientRect()
  openProfileCard(member.address, r.left + r.width / 2 + 5, r.top + r.height / 2 - 10, { above: true, context: 'community', onClose })
}

function RequestsGroup({
  group,
  me,
  onOpen,
  onManage
}: {
  group: CommunityRequestsGroup
  me: string | undefined
  onOpen: () => void
  onManage: (member: CommunityRequestMember, intention: RequestAnswer) => void
}): React.JSX.Element {
  const [acting, setActing] = useState<ReadonlySet<string>>(new Set())
  const [menuFor, setMenuFor] = useState<string | null>(null)
  // released when the group is read again (after any answer)
  useEffect(() => setActing(new Set()), [group])
  const manage = (m: CommunityRequestMember, intention: RequestAnswer): void => {
    setActing((prev) => new Set(prev).add(m.address))
    onManage(m, intention)
  }
  const n = group.total
  return (
    <div className={styles.requestsGroup}>
      <button type="button" className={styles.requestsGroupHead} onClick={onOpen}>
        <Avatar src={group.community.thumbnail} name={group.community.name} color={nameColor(group.community.id)} size={36} />
        <span className={styles.requestsGroupName}>{group.community.name}</span>
        <span className={styles.requestsGroupCount}>{`${n} Request${n > 1 ? 's' : ''} Received`}</span>
      </button>
      <div className={styles.requestsGrid}>
        {group.requests.map((m) => (
          <MemberRow
            key={m.address}
            member={m}
            self={me == null || me.toLowerCase() === m.address.toLowerCase()}
            busy={acting.has(m.address)}
            menuOpen={menuFor === m.address}
            actions={<RequestRowActions section="requests" busy={acting.has(m.address)} onAnswer={(intention) => manage(m, intention)} />}
            onView={() => openPassport(m.address)}
            onMenu={(anchor) => {
              setMenuFor(m.address)
              openRequestMenu(m, anchor, () => setMenuFor(null))
            }}
          />
        ))}
      </div>
    </div>
  )
}

/** The browser's Invites & Requests section: Invites, Requests Received (for owners and moderators) and Requests Sent, each titled with its
 *  count, or one line when all three are empty. */
function InvitesAndRequests({
  invites,
  canManage,
  me,
  live,
  actions,
  onOpen,
  onManage,
  onRetry
}: {
  invites: CommunityInvitesMessage | null
  /** The user owns or moderates a community: Requests Received is shown. */
  canManage: boolean
  me: string | undefined
  live: (c: Community) => boolean
  actions: CardActions
  onOpen: (id: string) => void
  onManage: (communityId: string, member: CommunityRequestMember, intention: RequestAnswer) => void
  /** Read the section again (a list could not be read). */
  onRetry: () => void
}): React.JSX.Element {
  if (invites == null) return <div className={styles.loading}><Spinner /></div>
  // a list that could not be read says so, rather than claiming nothing is pending
  const failed = (list: CommunityInvitesMessage['failed'][number]): boolean => invites.failed.includes(list)
  const unread = <EmptyState variant="inline" tone="error" title={SECTION_LOAD_ERROR} actions={[{ label: 'RETRY', variant: 'outline', onClick: onRetry }]} />
  if (failed('invites') && failed('requestsReceived') && failed('requestsSent')) return unread
  // the service's totals, as the sidebar's counter counts them
  const received = invites.requestsReceived.reduce((n, g) => n + g.total, 0)
  if (invites.failed.length === 0 && invites.invites.length === 0 && received === 0 && invites.requestsSent.length === 0) {
    return <EmptyState variant="inline" title="No pending invites or requests." />
  }
  const cards = (list: Community[]): React.JSX.Element => (
    <div className={styles.grid}>
      {list.map((c) => <CommunityCard key={c.id} c={c} live={live(c)} onOpen={() => onOpen(c.id)} {...actions} />)}
    </div>
  )
  return (
    <div className={styles.invitesSection}>
      <h2 className={styles.invitesHead}>{`Invites (${invites.invites.length})`}</h2>
      {failed('invites') ? unread : invites.invites.length > 0 ? cards(invites.invites) : <EmptyState variant="inline" title="No Invites" />}
      {canManage && (
        <>
          <h2 className={styles.invitesHead}>{`Requests Received (${received})`}</h2>
          {failed('requestsReceived') ? unread : received > 0 ? (
            invites.requestsReceived.map((g) => (
              <RequestsGroup key={g.community.id} group={g} me={me} onOpen={() => onOpen(g.community.id)} onManage={(m, intention) => onManage(g.community.id, m, intention)} />
            ))
          ) : (
            <EmptyState variant="inline" title="No Requests" />
          )}
        </>
      )}
      <h2 className={styles.invitesHead}>{`Requests Sent (${invites.requestsSent.length})`}</h2>
      {failed('requestsSent') ? unread : invites.requestsSent.length > 0 ? cards(invites.requestsSent) : <EmptyState variant="inline" title="No Requests" />}
    </div>
  )
}

export function CommunitiesPage({
  communities,
  profile,
  voiceChat,
  onNavigate
}: {
  communities: CommunitiesState
  profile: ProfileState
  /** The live streams; without them the communities' own snapshot says what is live. */
  voiceChat?: VoiceChatSession
  onNavigate: (page: string) => void
}): React.JSX.Element | null {
  // The search field's text; the grid shows the search last run (communities.browse.search).
  const [query, setQuery] = useState('')
  const [openId, setOpenId] = useState<string | null>(null)
  // Main area shows the browse grid, all of "My Communities" (via VIEW ALL), or Invites & Requests.
  const [tab, setTab] = useState<'browse' | 'mine' | 'invites'>('browse')
  // The "Streaming" filter: only the communities with a live voice stream.
  const [streaming, setStreaming] = useState(false)
  const searchTimer = useRef<ReturnType<typeof setTimeout> | null>(null)
  const { loadBrowse, loadMoreBrowse, browse, loadInvites, loadInvitesCount } = communities
  // what a delayed search compares with: the grid as it is when the delay ends
  const shown = useRef({ search: browse.search, tab })
  shown.current = { search: browse.search, tab }

  // The community detail modal lives in the HUD-wide popup layer (openCommunityModal), not this page's
  // own JSX — CommunityModalContainer reads live data itself. Re-opens on a different selection; closes
  // when the page closes (communities.open → false) or a selection clears, via the effect cleanup.
  const wantOpen = communities.open ? openId : null
  useEffect(() => {
    if (wantOpen == null) return
    // Only clear if `wantOpen` (captured here) is still the current selection: when switching directly
    // from one community to another, this same callback fires for the OLD id as its cleanup — by then
    // `openId` already holds the NEW id, so the identity check keeps it from clobbering the new popup.
    return openCommunityModal(wantOpen, () => setOpenId((id) => (id === wantOpen ? null : id)))
  }, [wantOpen])

  const cancelSearch = (): void => {
    if (searchTimer.current != null) clearTimeout(searchTimer.current)
    searchTimer.current = null
  }
  // Every opening shows every community, with the search cleared.
  useEffect(() => {
    if (!communities.open) return
    setQuery('')
    setTab('browse')
    loadBrowse('')
    // the sidebar's counter alone: the section is read when it opens
    loadInvitesCount()
    return cancelSearch
  }, [communities.open, loadBrowse, loadInvitesCount])

  // The same text again does nothing; empty text lists
  // every community; a search always shows the grid, even from My Communities.
  const runSearch = (text: string): void => {
    cancelSearch()
    if (text === shown.current.search && shown.current.tab === 'browse') return
    setTab('browse')
    loadBrowse(text)
  }
  const onQuery = (text: string): void => {
    setQuery(text)
    cancelSearch()
    searchTimer.current = setTimeout(() => runSearch(text), SEARCH_DELAY_MS)
  }
  const clearSearch = (): void => {
    cancelSearch()
    setQuery('')
    setTab('browse')
    loadBrowse('')
  }

  if (!communities.open) return null

  const mine = communities.list.filter((c) => isMember(c.role))
  const showingInvites = tab === 'invites'
  const showingMine = tab === 'mine'
  const searching = !showingMine && browse.search !== ''
  const live = (c: Community): boolean => streamLive(voiceChat, c.id, c.voiceChatStatus?.isActive)
  // The Streaming filter narrows the communities already read to the live ones
  const gridList = (showingMine ? mine : browse.items).filter((c) => !streaming || live(c))
  const actions: CardActions = {
    onJoin: communities.join,
    onRequestToJoin: communities.requestToJoin,
    onCancelRequest: communities.cancelRequest,
    onAcceptInvite: (id, inviteId) => communities.manageRequest(id, inviteId, 'accepted'),
    onRejectInvite: (id, inviteId) => communities.manageRequest(id, inviteId, 'rejected')
  }
  const pendingCount = communities.pendingCount ?? 0
  const openInvites = (): void => {
    cancelSearch()
    setTab('invites')
    loadInvites()
  }
  // The count is the service's total, blank while the first page loads; filtered, the cards shown
  const title = showingMine ? 'My Communities' : searching ? `Results for '${browse.search}'` : 'Browse Communities'
  const count = showingMine || streaming ? `(${gridList.length})` : browse.loading || browse.failed === true ? '' : `(${browse.total})`

  const p = profile.data
  return (
    <MainMenuShell
      active="communities"
      profileName={p?.name}
      profilePicture={p?.picture}
      profileAddress={p?.address}
      profileClaimed={p?.hasClaimedName}
      onNavigate={onNavigate}
      onClose={communities.toggle}
    >
      <div className={styles.head}>
        <h1 className={styles.title}>Communities</h1>
        <div className={styles.search}>
          <SearchField value={query} onChange={onQuery} onSubmit={runSearch} clearable onClear={clearSearch} placeholder="Search" />
        </div>
        <Chip label="Streaming" selected={streaming} onClick={() => setStreaming((s) => !s)} />
      </div>

      <div className={styles.layout}>
        <aside className={styles.side}>
          <button
            type="button"
            className={styles.createBtn}
            onClick={() => openCommunityCreateModal(!!p && p.hasClaimedName && !p.isGuest, (id) => id !== '' && setOpenId(id))}
          >
            + CREATE A COMMUNITY
          </button>
          <button
            type="button"
            className={`${styles.sideSection} ${styles.invitesBtn}`}
            aria-pressed={showingInvites}
            aria-label={pendingCount > 0 ? `Invites & Requests, ${pendingCount} pending` : 'Invites & Requests'}
            onClick={openInvites}
          >
            <span className={styles.invitesLabel}>Invites &amp; Requests</span>
            <CountBadge count={pendingCount} />
          </button>
          <div className={styles.myHead}>
            <span className={styles.sideHead}>My Communities</span>
            {mine.length > 0 && (
              <button type="button" className={styles.viewAll} onClick={() => setTab(showingMine ? 'browse' : 'mine')}>
                {showingMine ? '‹ BACK' : 'VIEW ALL ›'}
              </button>
            )}
          </div>
          {mine.length === 0 ? (
            <span className={styles.sideEmpty}>You haven't joined any communities yet.</span>
          ) : (
            mine.map((c) => (
              <div key={c.id} className={styles.sideRow} onClick={() => setOpenId(c.id)} role="button" tabIndex={0}>
                <Avatar src={c.thumbnail} name={c.name} color={nameColor(c.id)} size={36} />
                <span className={styles.sideName} title={c.name}>{c.name}</span>
              </div>
            ))
          )}
        </aside>

        <section className={styles.main}>
          {showingInvites ? (
            <>
              <div className={styles.browseHead}>
                <button type="button" className={styles.back} aria-label="Back to Browse Communities" onClick={() => setTab('browse')}>‹</button>
                <span>Invites &amp; Requests</span>
              </div>
              <InvitesAndRequests
                invites={communities.invites}
                canManage={mine.some((c) => c.role === 'owner' || c.role === 'moderator')}
                me={p?.address}
                live={live}
                actions={actions}
                onOpen={setOpenId}
                onManage={(id, m, intention) => communities.manageRequest(id, m.requestId, intention, m.address)}
                onRetry={loadInvites}
              />
            </>
          ) : (
          <>
          <div className={styles.browseHead}>
            {searching && (
              <button type="button" className={styles.back} aria-label="Back to Browse Communities" onClick={clearSearch}>‹</button>
            )}
            <span>{title}</span>
            {count !== '' && <span className={styles.count}>{count}</span>}
          </div>
          {!showingMine && browse.loading ? (
            <div className={styles.loading}><Spinner /></div>
          ) : !showingMine && browse.failed === true ? null : gridList.length === 0 ? (
            <div className={styles.empty}>{showingMine ? 'You haven’t joined any communities yet.' : 'No Communities match your search terms.'}</div>
          ) : (
            <>
              <div className={styles.grid}>
                {gridList.map((c) => (
                  <CommunityCard key={c.id} c={c} live={live(c)} onOpen={() => setOpenId(c.id)} {...actions} />
                ))}
              </div>
              {!showingMine && browse.items.length < browse.total && <LoadMore count={browse.items.length} onVisible={loadMoreBrowse} />}
              {!showingMine && browse.loadingMore && <div className={styles.loadingMore}><Spinner size={22} /></div>}
            </>
          )}
          </>
          )}
        </section>
      </div>
    </MainMenuShell>
  )
}
