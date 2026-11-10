// React Communities — a full-screen page inside the shared MainMenuShell (matches the
// Unity Communities screen): a left column with "My Communities" and a main browse grid
// of community cards. The grid reads 20 communities a page from the service,
// the next page read when the grid's end scrolls into view, and the search run on the server
// (a second after typing stops, or at once on Enter). Each card shows Public or Private and
// offers Join, Request to Join or Cancel, the user's friends in it, and its description on hover.

import { useEffect, useRef, useState } from 'react'
import { Avatar, Button, SearchField, Spinner, Tooltip } from '@hud/design'
import { nameColor } from '../../lib/identity'
import { MainMenuShell } from '../menu/MainMenuShell'
import { openCommunityModal } from './CommunityModal'
import { openCommunityCreateModal } from './CommunityCreateModal'
import type { Community } from '@hud/engine/protocol'
import type { CommunitiesState, ProfileState } from '../session/useEngineSession'
import styles from './CommunitiesPage.module.css'

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
}

// A member views; anyone may join a public community; a
// private one takes a request, which can be cancelled while it waits.
function CardButton({ c, onOpen, onJoin, onRequestToJoin, onCancelRequest }: CardActions & { c: Community; onOpen: () => void }): React.JSX.Element {
  const act = (run: () => void) => (e: React.MouseEvent): void => {
    e.stopPropagation()
    run()
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
        Requested
      </Button>
    )
  }
  return <Button size="sm" className={styles.cardBtn} onClick={act(() => onRequestToJoin(c.id))}>Request to Join</Button>
}

function CommunityCard({ c, onOpen, ...actions }: CardActions & { c: Community; onOpen: () => void }): React.JSX.Element {
  const [failed, setFailed] = useState(false)
  const showImg = c.thumbnail != null && c.thumbnail !== '' && !failed
  const isPrivate = c.privacy === 'private'
  const friends = c.friends ?? []
  return (
    // named by its whole text, as the e2e suite reads it (apps/bevy-explorer/e2e/specs/communities.test.ts)
    <div className={styles.card} onClick={onOpen} role="button" tabIndex={0}>
      <div className={styles.cover} style={{ background: showImg ? undefined : nameColor(c.id) }}>
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

// Reads the next page when the grid's end comes into view. The observer is made again whenever the
// grid grows, so a page that leaves the end still in view reads the next one too. jsdom has no
// IntersectionObserver: the grid then shows the first page only.
function LoadMore({ count, onVisible }: { count: number; onVisible: () => void }): React.JSX.Element {
  const ref = useRef<HTMLDivElement>(null)
  const visible = useRef(onVisible)
  visible.current = onVisible
  useEffect(() => {
    const el = ref.current
    if (el == null || typeof IntersectionObserver === 'undefined') return
    const io = new IntersectionObserver((entries) => {
      if (entries.some((e) => e.isIntersecting)) visible.current()
    }, { rootMargin: '0px 0px 200px 0px' })
    io.observe(el)
    return () => io.disconnect()
  }, [count])
  return <div ref={ref} className={styles.loadMore} aria-hidden="true" />
}

export function CommunitiesPage({
  communities,
  profile,
  onNavigate
}: {
  communities: CommunitiesState
  profile: ProfileState
  onNavigate: (page: string) => void
}): React.JSX.Element | null {
  // The search field's text; the grid shows the search last run (communities.browse.search).
  const [query, setQuery] = useState('')
  const [openId, setOpenId] = useState<string | null>(null)
  // Main area shows the browse grid, or all of "My Communities" (via VIEW ALL).
  const [tab, setTab] = useState<'browse' | 'mine'>('browse')
  const searchTimer = useRef<ReturnType<typeof setTimeout> | null>(null)
  const { loadBrowse, loadMoreBrowse, browse } = communities
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
    return cancelSearch
  }, [communities.open, loadBrowse])

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
  const showingMine = tab === 'mine'
  const searching = !showingMine && browse.search !== ''
  const gridList = showingMine ? mine : browse.items
  const actions: CardActions = { onJoin: communities.join, onRequestToJoin: communities.requestToJoin, onCancelRequest: communities.cancelRequest }
  // The count is the service's total, blank while the first page loads
  const title = showingMine ? 'My Communities' : searching ? `Results for '${browse.search}'` : 'Browse Communities'
  const count = showingMine ? `(${mine.length})` : browse.loading || browse.failed === true ? '' : `(${browse.total})`

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
          <div className={styles.sideSection}>
            <span className={styles.sideHead}>Invites &amp; Requests</span>
          </div>
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
                  <CommunityCard key={c.id} c={c} onOpen={() => setOpenId(c.id)} {...actions} />
                ))}
              </div>
              {!showingMine && browse.items.length < browse.total && <LoadMore count={browse.items.length} onVisible={loadMoreBrowse} />}
              {!showingMine && browse.loadingMore && <div className={styles.loadingMore}><Spinner size={22} /></div>}
            </>
          )}
        </section>
      </div>
    </MainMenuShell>
  )
}
