import { useEffect, useMemo, useRef, useState } from 'react'
import { useSearchParams } from 'react-router'
import { PersonOutlineIcon, SearchIcon } from '../Icons'
import { useTranslation } from '@/data/lib/creators/intl'
import { Button } from '../Button'
import { AssignCuratorModal } from '../AssignCuratorModal'
import NotFoundPage from '../../../not-found/NotFoundPage'
import { Pagination } from '../Pagination'
import { MultiSelect } from '../MultiSelect'
import { Select, type SelectOption } from '../Select'
import { Switch } from '../Switch'
import { useCampaign } from '../../hooks/useCampaign'
import { useCommittee } from '../../hooks/useCommittee'
import { useCurationCollections, useCurationsByCollection, useCuratorOptions } from '../../hooks/useCuration'
import { useFeatureFlag } from '../../hooks/useFeatureFlag'
import { shortenAddress } from '@/data/lib/creators/address'
import { track } from '@/data/lib/creators/analytics'
import type { Collection } from '@/data/lib/creators/collections'
import { FeatureFlag } from '@/data/lib/creators/featureFlags'
import {
  CURATION_PAGE_SIZE,
  CURATION_SORTS,
  CurationTypeFilter,
  DEFAULT_STATUS_FILTERS,
  ReviewType,
  STATUS_FILTERS,
  UNASSIGNED,
  defaultAssignees,
  parseCurationFilters,
  rememberCurationSearch,
  toMultiParam,
  type CollectionCuration,
  CurationStatusFilter
} from '@/data/lib/creators/curation'
import { pageRangeLabel } from '@/data/lib/creators/pagination'
import { useWallet } from '../../store/wallet'
import { CurationRow } from './CurationRow'
import { cx } from '@/data/lib/creators/cx'
import shared from '../../styles/shared.module.css'
import styles from './CurationPage.module.css'

// The chip that clears the review type, and what a cleared status or assignee picker reports to analytics.
const ALL = 'all'
const REVIEW_TYPE_FILTERS = [ALL, ReviewType.NEW, ReviewType.UPDATES] as const
const SEARCH_DEBOUNCE_MS = 500

type Assigning = { collection: Collection; curation: CollectionCuration | null; mode: 'self' | 'edit' }

const CurationPage = () => {
  const { t } = useTranslation()
  const { session, restored, signIn } = useWallet()
  const address = session?.address
  const committee = useCommittee(address)
  const campaign = useCampaign()

  const [searchParams, setSearchParams] = useSearchParams()
  const autoCuration = useFeatureFlag(FeatureFlag.AUTO_CURATION_REVIEW_FLOW).enabled
  // Only a server with the automated review knows the auto-reviewing status; a shared link may still carry it.
  const filters = useMemo(() => {
    const parsed = parseCurationFilters(searchParams, address)
    return autoCuration
      ? parsed
      : { ...parsed, statuses: parsed.statuses.filter(status => status !== CurationStatusFilter.AUTO_REVIEWING) }
  }, [searchParams, address, autoCuration])
  const assigneeDefaults = useMemo(() => defaultAssignees(address), [address])
  const collections = useCurationCollections(address, filters, committee.isCurator)
  const curations = useCurationsByCollection(address, committee.isCurator)
  const [assigning, setAssigning] = useState<Assigning | null>(null)

  const [searchInput, setSearchInput] = useState(filters.search)
  const searchTimer = useRef<ReturnType<typeof setTimeout>>(undefined)
  // The URL holds the trimmed query; writing it back verbatim would eat the space the user is typing.
  useEffect(
    () => setSearchInput(current => (current.trim() === filters.search ? current : filters.search)),
    [filters.search]
  )
  useEffect(() => rememberCurationSearch(searchParams.size > 0 ? `?${searchParams}` : ''), [searchParams])
  useEffect(() => () => clearTimeout(searchTimer.current), [])

  const unassigned = useMemo(() => ({ value: UNASSIGNED, label: t('curation_page.filter.unassigned') }), [t])
  const curatorOptions = useCuratorOptions(address, unassigned)
  // A shared link may name a curator who left the committee, or the committee may still be loading: the picked
  // address stays listed so the trigger counts it and it can be unticked.
  const assigneeOptions = useMemo(() => {
    const known = new Set(curatorOptions.map(option => option.value))
    const extra = filters.assignees.filter(value => !known.has(value))
    return extra.length
      ? [...curatorOptions, ...extra.map(value => ({ value, label: shortenAddress(value) }))]
      : curatorOptions
  }, [curatorOptions, filters.assignees])
  const statusOptions = useMemo(
    () =>
      STATUS_FILTERS.filter(status => autoCuration || status !== CurationStatusFilter.AUTO_REVIEWING).map(status => ({
        value: status,
        label: t(`curation_page.filter.${status}`)
      })),
    [t, autoCuration]
  )
  const sortOptions = useMemo<SelectOption<string>[]>(
    () => CURATION_SORTS.map(sort => ({ value: sort, label: t(`curation_page.sort.${sort}`) })),
    [t]
  )
  const typeOptions = useMemo<SelectOption<CurationTypeFilter>[]>(
    () => Object.values(CurationTypeFilter).map(type => ({ value: type, label: t(`curation_page.type.${type}`) })),
    [t]
  )

  // Filters live in the URL so a review link can be shared and Back restores the list.
  function changeParams(changes: Record<string, string | string[] | null>) {
    setSearchParams(
      prev => {
        const next = new URLSearchParams(prev)
        next.delete('page')
        for (const [key, value] of Object.entries(changes)) {
          next.delete(key)
          if (Array.isArray(value)) value.forEach(entry => next.append(key, entry))
          else if (value) next.set(key, value)
        }
        return next
      },
      { replace: true }
    )
  }

  function changeFilter(filter: string, value: string | null) {
    track('bd_curation_filter_changed', { filter, value: value ?? ALL })
    changeParams({ [filter]: value })
  }

  // The defaults are left out of the URL, so a plain /curation link always opens the standard view.
  function changeMultiFilter(filter: 'status' | 'assignee', values: string[], defaults: string[]) {
    track('bd_curation_filter_changed', { filter, value: values.join(',') || ALL })
    changeParams({ [filter]: toMultiParam(values, defaults) })
  }

  function onSearchChange(value: string) {
    setSearchInput(value)
    if (searchTimer.current) clearTimeout(searchTimer.current)
    searchTimer.current = setTimeout(() => changeParams({ q: value.trim() || null }), SEARCH_DEBOUNCE_MS)
  }

  function goToPage(next: number) {
    changeParams({ page: next > 1 ? String(next) : null })
    window.scrollTo({ top: 0 })
  }

  if (!restored || (session && committee.isLoading)) {
    return (
      <div className="page-loading" aria-busy="true" data-testid="curation-loading">
        <span className="spinner" aria-hidden />
      </div>
    )
  }

  if (!session) {
    return (
      <div className={shared.page} data-testid="curation-page">
        <section className={shared.panel} data-testid="sign-in-panel">
          <div className={shared.signInIcon} aria-hidden>
            <PersonOutlineIcon />
          </div>
          <h2 className={shared.panelTitle}>{t('curation_page.sign_in')}</h2>
          <Button type="button" variant="primary" data-testid="sign-in" onClick={() => signIn()}>
            {t('collections_page.sign_in.action')}
          </Button>
        </section>
      </div>
    )
  }

  if (!committee.isCurator) return <NotFoundPage />

  const data = collections.data
  const total = data?.total ?? 0
  const shown = data?.results.length ?? 0
  // The default pickers are the curator's queue, not a filter: an empty queue reads as empty, not as no match.
  const hasActiveFilters =
    !!filters.search ||
    toMultiParam(filters.statuses, DEFAULT_STATUS_FILTERS) !== null ||
    toMultiParam(filters.assignees, assigneeDefaults) !== null ||
    !!filters.reviewType ||
    filters.type !== CurationTypeFilter.ALL ||
    !!filters.tag
  // Rows can't tell "never requested" from "not loaded yet": without the curations their actions would open new requests.
  const isLoading = collections.isLoading || (collections.isFetching && !data) || curations.isLoading
  // A failed background refetch keeps its cached data: only a failure with nothing to show blanks the list.
  const isError = (collections.isError && !data) || (curations.isError && !curations.data)
  const isSearching = searchInput.trim() !== filters.search || (!!filters.search && collections.isFetching)

  return (
    <div className={shared.page} data-testid="curation-page">
      <div className={shared.header}>
        <h1 className={shared.title}>{t('curation_page.title')}</h1>
        <div className={shared.searchBox}>
          <SearchIcon fontSize="small" />
          <input
            value={searchInput}
            placeholder={t('curation_page.search_placeholder')}
            aria-label={t('curation_page.search_placeholder')}
            data-testid="curation-search"
            onChange={e => onSearchChange(e.target.value)}
          />
          {isSearching && <span className={shared.spinner} aria-hidden />}
        </div>
      </div>

      <div className={styles.filterRow}>
        <div className={shared.chips} data-testid="curation-review-type-filters">
          {REVIEW_TYPE_FILTERS.map(type => (
            <button
              className={shared.chip}
              key={type}
              type="button"
              aria-pressed={(filters.reviewType ?? ALL) === type}
              data-active={(filters.reviewType ?? ALL) === type || undefined}
              data-testid={`curation-review-type-${type}`}
              onClick={() => changeFilter('review_type', type === ALL ? null : type)}
            >
              {t(`curation_page.filter.review_type.${type}`)}
            </button>
          ))}
        </div>
        <div className={styles.controls}>
          {campaign && (
            <label className={styles.campaignToggle}>
              <Switch
                checked={filters.tag === campaign.mainTag}
                onChange={checked => changeFilter('tag', checked ? campaign.mainTag : null)}
                label={campaign.name}
                testId="curation-campaign-filter"
              />
              <span>{campaign.name}</span>
            </label>
          )}
          <div className={styles.selects}>
            <Select
              value={filters.type}
              options={typeOptions}
              onChange={value => changeFilter('type', value === CurationTypeFilter.ALL ? null : value)}
              ariaLabel={t('curation_page.type.label')}
              variant="compact"
              testId="curation-type-filter"
            />
            <MultiSelect<CurationStatusFilter>
              values={filters.statuses}
              options={statusOptions}
              onChange={values => changeMultiFilter('status', values, DEFAULT_STATUS_FILTERS)}
              placeholder={t('curation_page.filter.all_statuses')}
              ariaLabel={t('curation_page.filter.status')}
              variant="compact"
              testId="curation-status-filter"
            />
            <MultiSelect
              values={filters.assignees}
              options={assigneeOptions}
              onChange={values => changeMultiFilter('assignee', values, assigneeDefaults)}
              placeholder={t('curation_page.filter.all_assignees')}
              ariaLabel={t('curation_page.filter.assignee')}
              variant="compact"
              testId="curation-assignee-filter"
            />
            <Select
              value={filters.sort}
              options={sortOptions}
              onChange={value => changeFilter('sort', value)}
              ariaLabel={t('curation_page.filter.sort')}
              variant="compact"
              testId="curation-sort"
            />
          </div>
        </div>
      </div>

      {isLoading ? (
        <div className={styles.list} data-testid="curation-loading-rows" aria-hidden>
          {Array.from({ length: 6 }, (_, i) => (
            <div className={cx(styles.skeletonRow, 'skeleton')} key={i} />
          ))}
        </div>
      ) : isError ? (
        <section className={shared.panel} data-testid="curation-error">
          <h2 className={shared.panelTitle}>{t('curation_page.error.title')}</h2>
          <p className={shared.panelText}>{t('curation_page.error.description')}</p>
          <Button
            data-testid="curation-retry"
            type="button"
            variant="secondary"
            onClick={() => {
              if (collections.isError) void collections.refetch()
              if (curations.isError) void curations.refetch()
            }}
          >
            {t('curation_page.error.retry')}
          </Button>
        </section>
      ) : total === 0 ? (
        <section className={shared.panel} data-testid="curation-empty">
          <h2 className={shared.panelTitle}>
            {t(hasActiveFilters ? 'curation_page.no_results' : 'curation_page.empty')}
          </h2>
        </section>
      ) : (
        <>
          <div className={styles.list} data-testid="curation-list">
            <div className={styles.listHeader}>
              <span>{t('curation_page.list.collection')}</span>
              <span>{t('curation_page.list.owner')}</span>
              <span>{t('curation_page.list.date')}</span>
              <span>{t('curation_page.list.type')}</span>
              <span>{t('curation_page.list.status')}</span>
              <span>{t('curation_page.list.discussion')}</span>
              <span>{t('curation_page.list.assignee')}</span>
            </div>
            {data?.results.map(collection => (
              <CurationRow
                key={collection.id}
                collection={collection}
                curation={curations.byCollection.get(collection.id) ?? null}
                address={session.address}
                onAssign={(target, curation, mode) => setAssigning({ collection: target, curation, mode })}
              />
            ))}
          </div>
          <div className={shared.footerRow}>
            <span className={shared.showingCount} data-testid="curation-count">
              {t('curation_page.showing', { range: pageRangeLabel(filters.page, CURATION_PAGE_SIZE, shown), total })}
            </span>
            {(data?.pages ?? 0) > 1 && <Pagination page={filters.page} pages={data!.pages} onPageChange={goToPage} />}
          </div>
        </>
      )}

      {assigning && (
        <AssignCuratorModal
          collection={assigning.collection}
          curation={assigning.curation}
          address={session.address}
          mode={assigning.mode}
          onClose={() => setAssigning(null)}
        />
      )}
    </div>
  )
}

export { CurationPage }
