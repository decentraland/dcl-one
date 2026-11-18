import { useEffect, useMemo, useRef, useState } from 'react'
import { useNavigate, useSearchParams } from 'react-router-dom'
import {
  AddIcon,
  FormatListBulletedIcon,
  GridViewIcon,
  LinkIcon,
  PersonOutlineIcon,
  SearchIcon
} from '~/components/Icons'
import { useTranslation } from '~/intl'
import { clearTopUpResume, parseTopUpReturn, readTopUpResume, stripTopUpReturn } from '~/lib/creditsTopUp'
import { openExternal } from '~/lib/navigation'
import { pageRangeLabel } from '~/lib/pagination'
import { useWallet } from '~/store/wallet'
import { COLLECTIONS_PAGE_SIZE, useCollections, useCollectionsSummary } from '~/hooks/useCollections'
import { useSaveCollection } from '~/hooks/useCollection'
import { CONTRACT_PARAM, useCollectionByContractLink } from '~/hooks/useCollectionByContractLink'
import { track } from '~/lib/analytics'
import {
  CollectionSort,
  CollectionStatusFilter,
  LINKED_FILTER,
  isStatusFilterShown,
  type CollectionListFilter
} from '~/lib/collections'
import { buildNewCollection } from '~/lib/saveCollection'
import { isRefusedRequest } from '~/lib/builder'
import { captureError } from '~/lib/monitoring'
import {
  LINKED_WEARABLES_DOCS_PATH,
  buildNewLinkedCollection,
  getThirdPartyId,
  type LinkedContract
} from '~/lib/thirdParty'
import { config } from '~/config'
import { CollectionNameModal } from '~/components/CollectionNameModal'
import { CreateLinkedCollectionModal } from './CreateLinkedCollectionModal'
import { Pagination } from '~/components/Pagination'
import emptyCollectionsArt from '~/assets/empty-collections.png'
import { Button } from '~/components/Button'
import { CollectionCard } from './CollectionCard'
import { CollectionListRow } from './CollectionListRow'
import { cx } from '~/lib/cx'
import shared from '~/styles/shared.module.css'
import styles from './CollectionsPage.module.css'

const STATUS_FILTERS = [
  CollectionStatusFilter.ALL,
  CollectionStatusFilter.DRAFT,
  CollectionStatusFilter.PUBLISHED,
  CollectionStatusFilter.UNDER_REVIEW,
  CollectionStatusFilter.REJECTED
]
const SEARCH_DEBOUNCE_MS = 500
const LEARN_MORE_URL =
  'https://docs.decentraland.org/creator/wearables-and-emotes/manage-collections/creating-a-collection/'

type ViewMode = 'grid' | 'list'

function parseStatus(raw: string | null): CollectionListFilter {
  if (raw === LINKED_FILTER) return LINKED_FILTER
  return STATUS_FILTERS.includes(raw as CollectionStatusFilter)
    ? (raw as CollectionStatusFilter)
    : CollectionStatusFilter.ALL
}

const CollectionsPage = () => {
  const { t } = useTranslation()
  const navigate = useNavigate()
  const { session, restored, signIn } = useWallet()
  const address = session?.address

  const [searchParams, setSearchParams] = useSearchParams()
  const page = Math.max(1, Number(searchParams.get('page')) || 1)
  const search = searchParams.get('q') ?? ''
  const status = parseStatus(searchParams.get('status'))
  const [view, setView] = useState<ViewMode>('grid')
  const contractLink = useCollectionByContractLink(address)

  // Stripe returns every credits purchase to this route; the hand-off record says which collection's
  // publish wizard it belongs to. Without one there is nothing to resume, so the params are dropped.
  const topUpReturn = useMemo(() => parseTopUpReturn(searchParams), [searchParams])
  useEffect(() => {
    if (!topUpReturn) return
    const resume = readTopUpResume()
    if (resume && resume.orderId === topUpReturn.orderId) {
      const params = new URLSearchParams({ order: topUpReturn.orderId })
      if (topUpReturn.canceled) params.set('canceled', '1')
      navigate({ pathname: `/collections/${resume.collectionId}`, search: `?${params}` }, { replace: true })
      return
    }
    clearTopUpResume()
    setSearchParams(prev => stripTopUpReturn(prev), { replace: true })
  }, [topUpReturn, navigate, setSearchParams])

  // The box updates on every keystroke; the URL (and with it the query) lags behind.
  const [searchInput, setSearchInput] = useState(search)
  const searchTimer = useRef<ReturnType<typeof setTimeout>>()
  useEffect(() => setSearchInput(search), [search])
  useEffect(() => () => clearTimeout(searchTimer.current), [])

  const collections = useCollections(address, {
    page,
    search,
    status,
    sort: CollectionSort.LAST_ACTIVITY_DESC,
    includeLinked: true
  })
  const summary = useCollectionsSummary(address, search)
  const counts = summary.data?.counts
  const linkedTotal = summary.data?.linkedTotal ?? 0

  const [isCreateOpen, setCreateOpen] = useState(false)
  const saveCollection = useSaveCollection(address)
  // Under the Linked chip the page creates linked collections: same button, its own dialog.
  const isLinkedSection = status === LINKED_FILTER

  function closeCreateModal() {
    setCreateOpen(false)
    saveCollection.reset()
  }

  function onCreateSubmit(name: string) {
    if (!address) return
    saveCollection.mutate(buildNewCollection(name, address), {
      onSuccess: collection => navigate(`/collections/${collection.id}`)
    })
  }

  function onCreateLinkedSubmit(name: string, contract: LinkedContract) {
    if (!address) return
    const collection = buildNewLinkedCollection(name, address, contract)
    saveCollection.mutate(collection, {
      onSuccess: saved => {
        // Legacy event and prop names, so the funnel lines up with the old builder's.
        track('Create TP Collection', {
          collectionId: saved.id,
          thirdPartyId: getThirdPartyId(saved),
          linkedContractAddress: contract.address.toLowerCase(),
          linkedContractNetwork: contract.network,
          collectionName: name
        })
        navigate(`/collections/${saved.id}`)
      },
      onError: error => {
        if (!isRefusedRequest(error)) captureError(error, { flow: 'linked_create', network: contract.network })
      }
    })
  }

  // Filter/search/page changes all go through the URL, so back/forward and deep links just work.
  // Functional updater: the debounced search callback would otherwise apply a stale snapshot and
  // revert a filter clicked inside the debounce window.
  function changeParams(changes: Record<string, string | null>) {
    setSearchParams(
      prev => {
        const next = new URLSearchParams(prev)
        next.delete('page')
        for (const [key, value] of Object.entries(changes)) {
          if (value) next.set(key, value)
          else next.delete(key)
        }
        return next
      },
      { replace: true }
    )
  }

  // A chip whose last collection moved on disappears: fall back to All. While searching, the active
  // chip stays, showing (0) next to the no-results state. The Linked chip always stays: it is where
  // linked collections are created.
  const isActiveChipGone =
    !!counts &&
    !summary.isPlaceholderData &&
    !search &&
    status !== LINKED_FILTER &&
    !isStatusFilterShown(status, counts)
  // A contract link rewrites the URL itself; writing back from this render's params could revive it.
  const hasContractLink = searchParams.has(CONTRACT_PARAM)
  useEffect(() => {
    if (!isActiveChipGone || hasContractLink) return
    setSearchParams(
      prev => {
        const next = new URLSearchParams(prev)
        next.delete('status')
        next.delete('page')
        return next
      },
      { replace: true }
    )
  }, [isActiveChipGone, hasContractLink, setSearchParams])

  function onSearchChange(value: string) {
    setSearchInput(value)
    if (searchTimer.current) clearTimeout(searchTimer.current)
    searchTimer.current = setTimeout(() => changeParams({ q: value.trim() || null }), SEARCH_DEBOUNCE_MS)
  }

  function goToPage(next: number) {
    changeParams({ page: next > 1 ? String(next) : null })
    window.scrollTo({ top: 0 })
  }

  function onNewCollection() {
    if (!session) signIn()
    else setCreateOpen(true)
  }

  const data = collections.data
  const total = data?.total ?? 0
  const pages = data?.pages ?? 0
  const shown = data?.results.length ?? 0
  const hasActiveFilters = !!search || status !== CollectionStatusFilter.ALL
  const allCount = counts
    ? counts.published + counts.draft + counts.under_review + counts.rejected + counts.disabled + linkedTotal
    : 0
  // Counts ignore the status filter but follow the search, so only an unsearched list proves there are no collections.
  const isEmpty = !!counts && !search && allCount === 0
  const isLinkedEmpty = !!counts && !search && isLinkedSection && linkedTotal === 0
  const noResults = !!data && total === 0 && hasActiveFilters && !isEmpty && !isLinkedEmpty
  const isLoading =
    !restored || contractLink.isResolving || (!!address && (collections.isLoading || (collections.isFetching && !data)))
  // Debounce window (input ahead of the URL) or a refetch with a search applied.
  const isSearching = searchInput.trim() !== search || (!!search && collections.isFetching)

  return (
    <div className={shared.page} data-testid="collections-page">
      <div className={shared.header}>
        <h1 className={shared.title}>{t('collections_page.title')}</h1>
        <div className={styles.headerActions}>
          <div className={shared.searchBox}>
            <SearchIcon fontSize="small" />
            <input
              value={searchInput}
              placeholder={t('collections_page.search_placeholder')}
              aria-label={t('collections_page.search_placeholder')}
              data-testid="collections-search"
              onChange={e => onSearchChange(e.target.value)}
            />
            {isSearching && <span className={shared.spinner} data-testid="search-spinner" aria-hidden />}
          </div>
          <Button
            type="button"
            variant="primary"
            data-testid="new-collection"
            data-linked={isLinkedSection || undefined}
            onClick={onNewCollection}
          >
            <AddIcon fontSize="small" />
            {t(isLinkedSection ? 'collections_page.new_linked_collection' : 'collections_page.new_collection')}
          </Button>
        </div>
      </div>

      <div className={styles.filterRow}>
        <div className={shared.chips} data-testid="status-filters">
          {counts &&
            STATUS_FILTERS.filter(filter => filter === status || isStatusFilterShown(filter, counts)).map(filter => (
              <button
                className={shared.chip}
                key={filter}
                type="button"
                data-active={status === filter || undefined}
                data-testid={`status-filter-${filter}`}
                onClick={() => changeParams({ status: filter === CollectionStatusFilter.ALL ? null : filter })}
              >
                {t(`collections_page.filter.${filter}`, {
                  count: filter === CollectionStatusFilter.ALL ? allCount : counts[filter]
                })}
                {filter === CollectionStatusFilter.REJECTED && !!counts.rejected && (
                  <span className={styles.chipDot} data-testid="rejected-dot" aria-hidden />
                )}
              </button>
            ))}
          {counts && (
            <>
              <span className={styles.chipDivider} aria-hidden />
              <button
                className={shared.chip}
                type="button"
                data-active={status === LINKED_FILTER || undefined}
                data-testid={`status-filter-${LINKED_FILTER}`}
                onClick={() => {
                  track('Filter linked collections', { count: linkedTotal })
                  changeParams({ status: LINKED_FILTER })
                }}
              >
                <LinkIcon fontSize="small" />
                {t('collections_page.filter.linked', { count: linkedTotal })}
              </button>
            </>
          )}
        </div>
        <div className={styles.viewToggle} role="group" aria-label={t('collections_page.view_mode')}>
          <button
            className={styles.viewButton}
            type="button"
            data-active={view === 'grid' || undefined}
            aria-pressed={view === 'grid'}
            aria-label={t('collections_page.view_grid')}
            data-testid="view-grid"
            onClick={() => setView('grid')}
          >
            <GridViewIcon fontSize="small" />
          </button>
          <button
            className={styles.viewButton}
            type="button"
            data-active={view === 'list' || undefined}
            aria-pressed={view === 'list'}
            aria-label={t('collections_page.view_list')}
            data-testid="view-list"
            onClick={() => setView('list')}
          >
            <FormatListBulletedIcon fontSize="small" />
          </button>
        </div>
      </div>

      {restored && !session ? (
        <section className={shared.panel} data-testid="sign-in-panel">
          <div className={shared.signInIcon} aria-hidden>
            <PersonOutlineIcon />
          </div>
          <h2 className={shared.panelTitle}>{t('collections_page.sign_in.title')}</h2>
          <Button type="button" variant="primary" data-testid="sign-in" onClick={() => signIn()}>
            {t('collections_page.sign_in.action')}
          </Button>
        </section>
      ) : isLoading ? (
        <div className={styles.grid} data-testid="collections-loading" aria-hidden>
          {Array.from({ length: COLLECTIONS_PAGE_SIZE }, (_, i) => (
            <div className={cx(styles.skeletonCard, 'skeleton')} key={i} />
          ))}
        </div>
      ) : collections.isError ? (
        <section className={shared.panel} data-testid="collections-error">
          <h2 className={shared.panelTitle}>{t('collections_page.error.title')}</h2>
          <p className={shared.panelText}>{t('collections_page.error.description')}</p>
          <Button type="button" variant="secondary" onClick={() => void collections.refetch()}>
            {t('collections_page.error.retry')}
          </Button>
        </section>
      ) : isEmpty ? (
        <section className={shared.panel} data-testid="collections-empty">
          <img className={styles.emptyArt} src={emptyCollectionsArt} alt="" />
          <h2 className={shared.panelTitle}>{t('collections_page.empty.title')}</h2>
          <p className={shared.panelText} data-desktop>
            {t('collections_page.empty.description')}
          </p>
          <p className={shared.panelText} data-mobile>
            {t('collections_page.empty.description_mobile')}
          </p>
          <div className={styles.emptyActions}>
            <Button as="a" variant="secondary" href={LEARN_MORE_URL} target="_blank" rel="noopener noreferrer">
              {t('collections_page.empty.learn_more')}
            </Button>
            <Button type="button" variant="primary" data-testid="create-first-collection" onClick={onNewCollection}>
              <AddIcon fontSize="small" />
              {t('collections_page.empty.create_first')}
            </Button>
          </div>
        </section>
      ) : isLinkedEmpty ? (
        <section className={shared.panel} data-testid="linked-collections-empty">
          <img className={styles.emptyArt} src={emptyCollectionsArt} alt="" />
          <h2 className={shared.panelTitle}>{t('collections_page.linked_empty.title')}</h2>
          <p className={shared.panelText} data-desktop>
            {t('collections_page.linked_empty.description')}
          </p>
          <p className={shared.panelText} data-mobile>
            {t('collections_page.empty.description_mobile')}
          </p>
          <div className={styles.emptyActions}>
            <Button
              as="a"
              variant="secondary"
              href={`${config.get('DOCS_URL')}${LINKED_WEARABLES_DOCS_PATH}`}
              onClick={(event: React.MouseEvent) => {
                event.preventDefault()
                openExternal(`${config.get('DOCS_URL')}${LINKED_WEARABLES_DOCS_PATH}`)
              }}
            >
              {t('collections_page.empty.learn_more')}
            </Button>
            <Button
              type="button"
              variant="primary"
              data-testid="create-first-linked-collection"
              onClick={onNewCollection}
            >
              <AddIcon fontSize="small" />
              {t('collections_page.linked_empty.create_first')}
            </Button>
          </div>
        </section>
      ) : noResults ? (
        <section className={shared.panel} data-testid="collections-no-results">
          <img className={styles.emptyArt} src={emptyCollectionsArt} alt="" />
          <h2 className={shared.panelTitle}>{t('collections_page.no_results.title')}</h2>
          <p className={shared.panelText}>{t('collections_page.no_results.description')}</p>
        </section>
      ) : (
        <>
          {view === 'grid' ? (
            <div className={styles.grid} data-testid="collections-grid">
              {data?.results.map(collection => (
                <CollectionCard key={collection.id} collection={collection} />
              ))}
            </div>
          ) : (
            <div className={styles.list} data-testid="collections-list">
              <div className={styles.listHeader}>
                <span>{t('collections_page.list.collection')}</span>
                <span>{t('collections_page.list.items')}</span>
                <span>{t('collections_page.list.status')}</span>
                <span>{t('collections_page.list.last_updated')}</span>
                <span>{t('collections_page.list.created')}</span>
                <span className={styles.listHeaderActions}>{t('collections_page.list.actions')}</span>
              </div>
              {data?.results.map(collection => (
                <CollectionListRow key={collection.id} collection={collection} />
              ))}
            </div>
          )}
          <div className={shared.footerRow}>
            <span className={shared.showingCount} data-testid="collections-count">
              {t('collections_page.showing', { range: pageRangeLabel(page, COLLECTIONS_PAGE_SIZE, shown), total })}
            </span>
            {pages > 1 && <Pagination page={page} pages={pages} onPageChange={goToPage} />}
          </div>
        </>
      )}

      {isCreateOpen && !isLinkedSection && (
        <CollectionNameModal
          variant="create"
          isPending={saveCollection.isPending}
          error={saveCollection.error?.message ?? null}
          onSubmit={onCreateSubmit}
          onClose={closeCreateModal}
        />
      )}
      {isCreateOpen && isLinkedSection && (
        <CreateLinkedCollectionModal
          isPending={saveCollection.isPending}
          error={saveCollection.error}
          onSubmit={onCreateLinkedSubmit}
          onClose={closeCreateModal}
        />
      )}
    </div>
  )
}

export { CollectionsPage }
