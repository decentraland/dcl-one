import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { useNavigate, useSearchParams } from 'react-router'
import type { ContractNetwork } from '@dcl/schemas'
import { useTranslation } from '@/data/lib/creators/intl'
import { track } from '@/data/lib/creators/analytics'
import { previewCollectionFromDetail } from '@/data/lib/creators/explorer'
import { boundRefusal, isRefusedRequest } from '@/data/lib/creators/builder'
import { captureError } from '@/data/lib/creators/monitoring'
import { isSocialLogin, sendContractTransaction, settleTransaction, type Session } from '@/data/lib/creators/auth'
import { shortenAddress } from '@/data/lib/creators/address'
import { copyToClipboard } from '@/data/lib/creators/clipboard'
import { isCollectionLocked, type Collection } from '@/data/lib/creators/collections'
import { ITEM_EXTENSIONS } from '@/data/lib/creators/itemFiles'
import type { Item } from '@/data/lib/creators/items'
import {
  LinkedItemFilter,
  getLinkedItemStatus,
  getLinkedPublishAction,
  isLinkedItemSelectable,
  parseLinkedItemFilter,
  splitLinkedSelection,
  type ItemCuration
} from '@/data/lib/creators/linkedItemStatus'
import { isContractInUseError } from '@/data/lib/creators/linkedContract'
import type { Mappings } from '@/data/lib/creators/mappings'
import { getMaticChainId } from '@/data/lib/creators/publishCollection'
import {
  RelinkCancelledError,
  loadRelinkFrom,
  relinkCollection,
  type RelinkStep
} from '@/data/lib/creators/relinkCollection'
import { isWalletRejection, isWalletStateError } from '@/data/lib/creators/walletErrors'
import errorArt from '../../../assets/modal-error.png'
import { ConfirmModal } from '../../ConfirmModal'
import { PendingModal } from '../../PendingModal'
import { SuccessModal } from '../../SuccessModal'
import { fetchRegisteredContracts } from '@/data/lib/creators/thirdPartyRegistry'
import { useNotifications } from '@/data/lib/creators/notifications'
import { pageRangeLabel } from '@/data/lib/creators/pagination'
import { CurationStatus } from '@/data/lib/creators/collections'
import {
  LINKED_COLLECTION_NAME_MAX_LENGTH,
  REGISTRY_NETWORKS,
  decodeLinkedUrn,
  withCollectionUrnSuffix,
  type LinkedContract,
  type ThirdParty
} from '@/data/lib/creators/thirdParty'
import { useQueryClient } from '@tanstack/react-query'
import { useSaveCollection } from '../../../hooks/useCollection'
import { invalidateCollectionItems } from '../../../hooks/usePublishCollection'
import {
  LINKED_ITEMS_PAGE_SIZE,
  thirdPartyKey,
  useFetchLinkedItems,
  useHasLinkedWork,
  useItemCurations,
  useLinkedCollectionItems
} from '../../../hooks/useLinkedCollection'
import { useMediaQuery } from '../../../hooks/useMediaQuery'
import { useBeforeUnloadGuard } from '../../../hooks/useBeforeUnloadGuard'
import { useSaveItem } from '../../../hooks/useSaveItem'
import { useRefusalText } from '../../../hooks/useRefusalText'
import { createItemSaveQueue } from '@/data/lib/creators/itemSaveQueue'
import { media } from '@/data/lib/creators/theme'
import { Button } from '../../Button'
import { CollectionActionsMenu } from '../../CollectionActionsMenu'
import { CollectionNameModal } from '../../CollectionNameModal'
import { CollectionStatusPill } from '../../CollectionStatusPill'
import { EditContractModal } from './EditContractModal'
import { EditUrnModal } from './EditUrnModal'
import {
  OpenEditorIcon,
  AddIcon,
  ArrowBackIosNewIcon as ArrowBackIcon,
  ContentCopyIcon,
  EditIcon,
  ReportProblemOutlinedIcon as WarningIcon
} from '../../Icons'
import { Pagination } from '../../Pagination'
import { Tooltip } from '../../Tooltip'
import addItemsArt from '../../../assets/add-items.png'
import { AddItemsModal } from '../AddItemsModal'
import detail from '../CollectionDetailPage.module.css'
import styles from './LinkedCollectionView.module.css'
import shared from '../../../styles/shared.module.css'
import { LinkedItemActionsMenu } from './LinkedItemActionsMenu'
import { LinkedItemRow, nextMappingStates } from './LinkedItemRow'
import { PublishLinkedItemsModal } from './PublishLinkedItemsModal'
import { PushLinkedChangesModal } from './PushLinkedChangesModal'

type RelinkView =
  | { kind: 'form'; last?: LinkedContract }
  | { kind: 'running'; step: RelinkStep; to: LinkedContract }
  | { kind: 'success'; linked: number }
  | { kind: 'error'; inUse: boolean; wallet: boolean; to: LinkedContract }

// In the order an item goes through them.
const BASE_FILTERS = [
  LinkedItemFilter.ALL,
  LinkedItemFilter.DRAFT,
  LinkedItemFilter.UNDER_REVIEW,
  LinkedItemFilter.PUBLISHED,
  LinkedItemFilter.READY_TO_PUSH
]
const MIGRATION_FILTERS = [LinkedItemFilter.PENDING_MAPPING, LinkedItemFilter.PENDING_MIGRATION]

type Props = {
  collection: Collection
  thirdParty: ThirdParty | undefined
  session: Session
  page: number
  onBack: () => void
  onPageChange: (page: number) => void
}

/** A linked (third-party) collection: its contract, its items with their review status and token mapping, and the
 * actions to grow, publish and push it. */
export function LinkedCollectionView({ collection, thirdParty, session, page, onBack, onPageChange }: Props) {
  const { t } = useTranslation()
  const refusalText = useRefusalText()
  const navigate = useNavigate()
  const { address } = session
  const showToast = useNotifications(state => state.showToast)
  const [searchParams, setSearchParams] = useSearchParams()
  const filter = parseLinkedItemFilter(searchParams.get('filter'))
  const compact = useMediaQuery(media.noActions)
  const filesInputRef = useRef<HTMLInputElement>(null)

  const itemsPage = useLinkedCollectionItems(address, collection, page, filter)
  const items = useMemo(() => itemsPage.data?.results ?? [], [itemsPage.data])
  // Every item's latest curation: the rows read theirs, and a pending one anywhere blocks publishing.
  const curations = useItemCurations(address, collection)
  const saveCollection = useSaveCollection(address)
  const queryClient = useQueryClient()
  const saveItem = useSaveItem(address)
  const fetchLinkedItems = useFetchLinkedItems(address, collection.id)
  const saveItemRef = useRef(saveItem)
  saveItemRef.current = saveItem
  const saveItemPatch = useMemo(
    () => createItemSaveQueue(item => saveItemRef.current.mutateAsync({ item })),
    // One queue per collection URN: a URN change rebuilds every item's URN, which the queue's copies would undo.
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [collection.id, collection.urn]
  )

  const [isRenameOpen, setRenameOpen] = useState(false)
  const [isUrnOpen, setUrnOpen] = useState(false)
  // The contract change: `from` is what the collection had when the dialog opened (a retry after a partial run still
  // knows what to replace), `view` which of its screens is up.
  const [relink, setRelink] = useState<{ from: LinkedContract | null; view: RelinkView } | null>(null)
  const runningView = relink?.view.kind === 'running' ? relink.view : null
  // Bumped when the user backs out of the wallet prompt: that run stops once the wallet answers, and is not shown.
  const relinkAttempt = useRef(0)
  // A run the user backed out of is still waiting on its wallet prompt; no other run may start until it settles.
  const [relinkInFlight, setRelinkInFlight] = useState(false)
  const errorView = relink?.view.kind === 'error' ? relink.view : null
  const [addItemsFiles, setAddItemsFiles] = useState<File[] | null>(null)
  const [bulkEditFiles, setBulkEditFiles] = useState<File[] | null>(null)
  const bulkInputRef = useRef<HTMLInputElement>(null)
  // Selected items by id, kept across pages: "Select all" can reach items that are not on screen.
  const [selected, setSelected] = useState<Map<string, Item>>(() => new Map())
  const [isSelectingAll, setSelectingAll] = useState(false)
  const [allAcrossSelected, setAllAcrossSelected] = useState(false)
  // What the wizard opened with: the live selection shifts as a partial failure refreshes the list, while the cheque
  // was signed for this set.
  const [publishView, setPublishView] = useState<
    { kind: 'closed' } | { kind: 'publish' | 'push'; toPublish: Item[]; toPush: Item[] }
  >({ kind: 'closed' })
  // Rows with a mapping edit not saved yet, by item, with the generation of the row that reported it last: publishing
  // now would send the old mapping, and a row already gone can't overrule a newer one.
  const [mappingStates, setMappingStates] = useState<ReadonlyMap<string, { dirty: boolean; generation: number }>>(
    () => new Map()
  )
  const unsavedMappings = useMemo(
    () => new Set([...mappingStates].filter(([, state]) => state.dirty).map(([itemId]) => itemId)),
    [mappingStates]
  )
  // Leaving the page within a mapping's save delay would drop the edit; leaving mid-relink leaves items behind.
  useBeforeUnloadGuard(unsavedMappings.size > 0 || relinkInFlight)
  const onMappingDirtyChange = useCallback((itemId: string, dirty: boolean, generation: number) => {
    setMappingStates(current => nextMappingStates(current, itemId, dirty, generation))
  }, [])

  const network = collection.linkedContractNetwork
  const contractAddress = collection.linkedContractAddress
  const contract = useMemo<LinkedContract | null>(
    () => (network && contractAddress ? { network: network as ContractNetwork, address: contractAddress } : null),
    [network, contractAddress]
  )
  const canEdit = !compact && !isCollectionLocked(collection)
  const canEditCollection = canEdit && !collection.isPublished
  const total = itemsPage.data?.total ?? 0
  const pages = itemsPage.data?.pages ?? 0
  const decodedUrn = decodeLinkedUrn(collection.urn)

  const tracked = useRef(false)
  useEffect(() => {
    if (tracked.current || !itemsPage.data) return
    tracked.current = true
    track('bd_linked_collection_viewed', {
      collection_id: collection.id,
      item_count: itemsPage.data.total,
      network
    })
  }, [collection.id, itemsPage.data, network])

  // A filter shows another set of items: a selection made under the previous one would publish things off screen.
  const filterRef = useRef(filter)
  filterRef.current = filter
  useEffect(() => {
    setSelected(new Map())
    setAllAcrossSelected(false)
  }, [filter])

  const statuses = useMemo(
    () => new Map(items.map(item => [item.id, getLinkedItemStatus(item, curations.data?.get(item.id) ?? null)])),
    [items, curations.data]
  )
  const curationMap = useMemo(() => curations.data ?? new Map<string, ItemCuration>(), [curations.data])
  const selectableIds = useMemo(
    () =>
      curations.data
        ? items.filter(item => isLinkedItemSelectable(item, curationMap.get(item.id) ?? null)).map(item => item.id)
        : [],
    [items, curations.data, curationMap]
  )
  const selectableSet = useMemo(() => new Set(selectableIds), [selectableIds])
  // Selected items are kept as copies: refresh the ones on this page, so an edit since selecting is what publishes,
  // and drop the ones that can no longer be published or pushed.
  useEffect(() => {
    if (!curations.data) return
    setSelected(current => {
      let changed = false
      const next = new Map(current)
      for (const item of items) {
        if (!next.has(item.id)) continue
        if (!selectableSet.has(item.id)) next.delete(item.id)
        else if (next.get(item.id) !== item) next.set(item.id, item)
        else continue
        changed = true
      }
      return changed ? next : current
    })
  }, [items, selectableSet, curations.data])
  const selection = useMemo(() => splitLinkedSelection(selected.values(), curationMap), [selected, curationMap])
  const selectedCount = selection.toPublish.length + selection.toPush.length
  const publishAction = getLinkedPublishAction(selection)
  // Nothing anywhere to publish or push: the button would only ever be disabled, so it isn't shown.
  const hasWork = useHasLinkedWork(address, collection, curations.data)
  const hasPendingReview = useMemo(
    () => [...(curations.data?.values() ?? [])].some(curation => curation.status === CurationStatus.PENDING),
    [curations.data]
  )
  const publishBlocker = !publishAction
    ? 'no_selection'
    : publishAction !== 'push' && hasPendingReview
      ? 'under_review'
      : unsavedMappings.size > 0
        ? 'saving_mappings'
        : null
  const pageSelected = useMemo(() => selectableIds.filter(id => selected.has(id)).length, [selectableIds, selected])
  const allPageSelected = selectableIds.length > 0 && pageSelected === selectableIds.length
  const headerCheckbox = useRef<HTMLInputElement>(null)
  useEffect(() => {
    if (headerCheckbox.current) headerCheckbox.current.indeterminate = pageSelected > 0 && !allPageSelected
  }, [pageSelected, allPageSelected])
  const canSelectAcross = allPageSelected && !allAcrossSelected && pages > 1

  const filters = collection.isMappingComplete === false ? [...BASE_FILTERS, ...MIGRATION_FILTERS] : BASE_FILTERS

  function changeFilter(next: LinkedItemFilter) {
    setSearchParams(
      prev => {
        const params = new URLSearchParams(prev)
        if (next === LinkedItemFilter.ALL) params.delete('filter')
        else params.set('filter', next)
        params.delete('page')
        return params
      },
      { replace: true }
    )
  }

  function toggleSelected(item: Item, isSelected: boolean) {
    setAllAcrossSelected(false)
    setSelected(current => {
      const next = new Map(current)
      if (isSelected) next.set(item.id, item)
      else next.delete(item.id)
      return next
    })
  }

  function togglePage(isSelected: boolean) {
    setAllAcrossSelected(false)
    setSelected(current => {
      const next = new Map(current)
      for (const item of items) {
        if (!selectableSet.has(item.id)) continue
        if (isSelected) next.set(item.id, item)
        else next.delete(item.id)
      }
      return next
    })
  }

  function clearSelection() {
    setAllAcrossSelected(false)
    setSelected(new Map())
  }

  async function selectAllAcross() {
    const startedWith = filter
    setSelectingAll(true)
    try {
      const all = await fetchLinkedItems(filter)
      // The filter changed while this loaded: these items belong to the previous one.
      if (filterRef.current !== startedWith) return
      const selectable = all.filter(item => isLinkedItemSelectable(item, curationMap.get(item.id) ?? null))
      setSelected(new Map(selectable.map(item => [item.id, item])))
      setAllAcrossSelected(true)
      track('bd_linked_items_all_selected', { collection_id: collection.id, filter, count: selectable.length })
    } catch (error) {
      captureError(error, { flow: 'linked_select_all', collectionId: collection.id })
      showToast(t('linked_collection.select_all_error'), { type: 'error' })
    } finally {
      setSelectingAll(false)
    }
  }

  async function copyContract() {
    if (!contractAddress) return
    track('bd_linked_contract_copied', { collection_id: collection.id, network })
    if (await copyToClipboard(contractAddress)) showToast(t('linked_collection.contract_copied'))
    else showToast(t('collection_detail_page.actions.copy_failed'), { type: 'error' })
  }

  function closeRename() {
    setRenameOpen(false)
    saveCollection.reset()
  }

  function openContract() {
    if (contract) setRelink({ from: loadRelinkFrom(collection.id) ?? contract, view: { kind: 'form' } })
  }

  function closeContract() {
    setRelink(null)
  }

  async function relinkContract(next: LinkedContract) {
    if (!thirdParty || !relink) return
    const { from } = relink
    const attempt = ++relinkAttempt.current
    const show = (view: RelinkView) => {
      if (relinkAttempt.current === attempt) setRelink({ from, view })
    }
    setRelinkInFlight(true)
    try {
      const chainId = getMaticChainId()
      const { linked } = await relinkCollection(
        {
          saveCollection: saveCollection.mutateAsync,
          fetchRegisteredContracts: () => fetchRegisteredContracts(chainId, thirdParty.id),
          sendTransaction: call => sendContractTransaction(session, call),
          settleTransaction: txHash => settleTransaction(chainId, txHash),
          fetchItems: () => fetchLinkedItems(),
          saveItem: saveItemPatch,
          onStep: step => show({ kind: 'running', step, to: next }),
          isCancelled: () => relinkAttempt.current !== attempt
        },
        { chainId, collection, thirdParty, from, to: next }
      )
      track('bd_linked_collection_contract_edited', {
        collection_id: collection.id,
        from_network: from?.network ?? null,
        from_address: from?.address ?? null,
        to_network: next.network,
        to_address: next.address,
        items: linked
      })
      void curations.refetch()
      show({ kind: 'success', linked })
    } catch (error) {
      if (error instanceof RelinkCancelledError) return
      if (isWalletRejection(error)) {
        show({ kind: 'form', last: next })
        return
      }
      const inUse = isContractInUseError(error)
      const wallet = isWalletStateError(error)
      if (!inUse && !wallet) captureError(error, { flow: 'linked_collection_contract', collectionId: collection.id })
      show({ kind: 'error', inUse, wallet, to: next })
    } finally {
      // A run that stopped halfway has already changed the collection, the registry or some items.
      void queryClient.invalidateQueries({ queryKey: thirdPartyKey(address, thirdParty.id) })
      invalidateCollectionItems(queryClient, collection.id)
      setRelinkInFlight(false)
    }
  }

  function closeUrn() {
    setUrnOpen(false)
    saveCollection.reset()
  }

  function saveUrn(suffix: string) {
    const next = withCollectionUrnSuffix(collection, suffix)
    saveCollection.mutate(next, {
      onSuccess: () => {
        track('bd_linked_collection_urn_changed', { old_urn: collection.urn, new_urn: next.urn })
        // builder-server rebuilds every item URN from the collection's, so the cached items carry the old one.
        invalidateCollectionItems(queryClient, collection.id)
        closeUrn()
        showToast(t('linked_collection.urn_saved'))
      },
      onError: error => {
        if (!isRefusedRequest(error))
          captureError(error, { flow: 'linked_collection_urn', collectionId: collection.id })
      }
    })
  }

  const saveMappings = useCallback(
    (item: Item, mappings: Mappings) =>
      saveItemPatch(item, { mappings })
        .then(saved => {
          track('bd_linked_item_mapping_edited', {
            item_id: item.id,
            mapping_type: Object.values(mappings)[0]?.[contractAddress?.toLowerCase() ?? '']?.[0]?.type
          })
          return saved
        })
        .catch((error: unknown) => {
          captureError(error, { flow: 'linked_mapping_save', collectionId: collection.id, itemId: item.id })
          showToast(t('linked_collection.mapping_error', { name: item.name }), { type: 'error' })
          throw error
        }),
    [saveItemPatch, showToast, t, contractAddress, collection.id]
  )

  function startAddItems(files: FileList | File[] | null) {
    const list = files ? Array.from(files) : []
    if (list.length > 0) setAddItemsFiles(list)
  }

  const closePublish = useCallback(() => setPublishView({ kind: 'closed' }), [])
  const onPublished = useCallback(() => {
    setSelected(new Map())
    setAllAcrossSelected(false)
    void curations.refetch()
    void itemsPage.refetch()
  }, [curations, itemsPage])

  // A failed load has no total: it must not read as an empty collection.
  const loadFailed = itemsPage.isError && !itemsPage.data
  const isEmpty = !itemsPage.isLoading && !loadFailed && total === 0

  return (
    <>
      <div className={detail.header}>
        <div className={detail.headerLeft}>
          <button
            className={detail.backLink}
            type="button"
            aria-label={t('collection_detail_page.back')}
            data-testid="back-to-collections"
            onClick={onBack}
          >
            <ArrowBackIcon />
          </button>
          <div className={detail.titleGroup}>
            <h1 className={detail.title} title={collection.name}>
              {collection.name}
            </h1>
            {canEditCollection && (
              <button
                className={detail.renameButton}
                type="button"
                aria-label={t('collection_detail_page.rename')}
                data-testid="rename-collection"
                onClick={() => setRenameOpen(true)}
              >
                <EditIcon fontSize="small" />
              </button>
            )}
          </div>
          <CollectionStatusPill collection={collection} />
        </div>
        <div className={detail.headerActions}>
          {hasWork && (
            <Tooltip
              content={publishBlocker ? t(`linked_collection.publish_blocker.${publishBlocker}`) : null}
              placement="bottom"
              asChild
              testId="linked-publish-blocker"
            >
              <Button
                type="button"
                variant="primary"
                data-desktop-only
                data-testid="publish-linked-items"
                aria-disabled={publishBlocker ? true : undefined}
                onClick={() => {
                  if (publishBlocker || !publishAction) return
                  setPublishView({
                    kind: publishAction === 'push' ? 'push' : 'publish',
                    toPublish: selection.toPublish,
                    toPush: selection.toPush
                  })
                }}
              >
                {t(`linked_collection.publish_action.${publishAction ?? 'publish'}`, { count: selectedCount })}
              </Button>
            </Tooltip>
          )}
          <CollectionActionsMenu
            collection={collection}
            address={address}
            showRoles={false}
            onPreviewInWorld={total > 0 ? () => previewCollectionFromDetail(collection.id, total) : undefined}
            onPreviewItems={() => navigate(`/create/collections/editor?collection=${collection.id}`)}
            onEditUrn={canEditCollection ? () => setUrnOpen(true) : undefined}
            onEditContract={
              // A mapping edit still waiting to save would write the old contract back after the change.
              canEdit && contract && thirdParty && unsavedMappings.size === 0 ? openContract : undefined
            }
            onEditInBulk={canEdit && total > 0 ? () => bulkInputRef.current?.click() : undefined}
            onDeleted={() => navigate('/create/collections', { replace: true })}
          />
        </div>
      </div>

      <div className={styles.contractRow} data-testid="linked-contract">
        {thirdParty?.name && (
          <div className={styles.contractField}>
            <span className={styles.contractLabel}>{t('linked_collection.third_party')}</span>
            <span className={styles.contractValue} data-testid="linked-third-party">
              {thirdParty.name}
            </span>
          </div>
        )}
        {/* A virtual third party has no kind yet: it is chosen in its first publish. */}
        {thirdParty && (thirdParty.published || thirdParty.isProgrammatic) && (
          <div className={styles.contractField}>
            <span className={styles.contractLabel}>{t('linked_collection.kind_label')}</span>
            <span className={styles.contractValue} data-testid="third-party-kind">
              {t(`linked_collection.kind.${thirdParty.isProgrammatic ? 'programmatic' : 'standard'}`)}
            </span>
          </div>
        )}
        {network && (
          <div className={styles.contractField}>
            <span className={styles.contractLabel}>{t('linked_collection.network')}</span>
            <span className={styles.contractValue} data-testid="linked-network">
              {REGISTRY_NETWORKS.has(network) ? t(`linked_collection.networks.${network}`) : network}
            </span>
          </div>
        )}
        {contractAddress && (
          <div className={styles.contractField}>
            <span className={styles.contractLabel}>{t('linked_collection.contract')}</span>
            <span className={styles.contractValue} data-testid="linked-contract-address" title={contractAddress}>
              {shortenAddress(contractAddress)}
              <button
                className={styles.copyButton}
                type="button"
                aria-label={t('linked_collection.copy_contract')}
                data-testid="copy-linked-contract"
                onClick={() => void copyContract()}
              >
                <ContentCopyIcon fontSize="small" />
              </button>
            </span>
          </div>
        )}
      </div>

      {collection.isMappingComplete === false && (
        <div className={styles.banner} data-testid="linked-migration-banner">
          <WarningIcon fontSize="small" />
          {t('linked_collection.migration_banner')}
        </div>
      )}

      <div className={detail.subHeader}>
        <div className={detail.filterChips} data-testid="linked-filters">
          {filters.map(value => (
            <button
              className={detail.filterChip}
              key={value}
              type="button"
              data-active={filter === value || undefined}
              data-testid={`linked-filter-${value}`}
              onClick={() => changeFilter(value)}
            >
              {t(`linked_collection.filter.${value}`)}
            </button>
          ))}
        </div>
        <div className={detail.subActions}>
          <Button
            variant="secondary"
            type="button"
            data-testid="open-editor"
            onClick={() => navigate(`/create/collections/editor?collection=${collection.id}`)}
          >
            <OpenEditorIcon />
            {t('collection_detail_page.open_editor')}
          </Button>
          {canEdit && (
            <Button
              variant="secondary"
              type="button"
              data-testid="add-items"
              onClick={() => filesInputRef.current?.click()}
            >
              <AddIcon fontSize="small" />
              {t('collection_detail_page.add_items')}
            </Button>
          )}
        </div>
      </div>

      {loadFailed ? (
        <section className={shared.panel} data-testid="linked-items-error">
          <h2 className={shared.panelTitle}>{t('linked_collection.load_error.title')}</h2>
          <p className={shared.panelText}>
            {refusalText(boundRefusal(itemsPage.error)) ?? t('linked_collection.load_error.description')}
          </p>
          <Button
            type="button"
            variant="secondary"
            data-testid="linked-items-retry"
            onClick={() => void itemsPage.refetch()}
          >
            {t('linked_collection.load_error.retry')}
          </Button>
        </section>
      ) : isEmpty && filter === LinkedItemFilter.ALL ? (
        <section className={shared.panel} data-testid="collection-empty">
          <img className={detail.dropArt} src={addItemsArt} alt="" />
          <h2 className={shared.panelTitle}>{t('linked_collection.empty.title')}</h2>
          <p className={shared.panelText}>{t('linked_collection.empty.description')}</p>
          {canEdit && (
            <Button
              type="button"
              variant="primary"
              data-testid="add-first-items"
              onClick={() => filesInputRef.current?.click()}
            >
              <AddIcon fontSize="small" />
              {t('collection_detail_page.add_items')}
            </Button>
          )}
        </section>
      ) : isEmpty ? (
        <section className={shared.panel} data-testid="collection-no-items">
          <h2 className={shared.panelTitle}>{t('linked_collection.no_results.title')}</h2>
          <p className={shared.panelText}>{t('linked_collection.no_results.description')}</p>
        </section>
      ) : (
        <>
          <div className={styles.list} data-testid="items-list" aria-busy={itemsPage.isFetching || undefined}>
            <div className={styles.listHeader} data-selecting={selectedCount > 0 || undefined}>
              <span className={styles.headerSelect}>
                <input
                  ref={headerCheckbox}
                  type="checkbox"
                  checked={allPageSelected}
                  disabled={selectableIds.length === 0 || isSelectingAll}
                  aria-label={t('linked_collection.select_all')}
                  data-testid="linked-select-all"
                  onChange={event => togglePage(event.target.checked)}
                />
              </span>
              {selectedCount > 0 ? (
                <div className={styles.selectionSummary} data-testid="linked-selection">
                  <span className={styles.selectionCount} data-testid="linked-selection-count">
                    {t(allAcrossSelected ? 'linked_collection.selected_all' : 'linked_collection.selected', {
                      count: selectedCount
                    })}
                  </span>
                  <div className={styles.selectionActions}>
                    {canSelectAcross && (
                      <Button
                        type="button"
                        size="sm"
                        variant="secondary"
                        loading={isSelectingAll}
                        data-testid="linked-select-across"
                        onClick={() => void selectAllAcross()}
                      >
                        {t('linked_collection.select_across', { total })}
                      </Button>
                    )}
                    <Button
                      type="button"
                      size="sm"
                      variant="dark"
                      disabled={isSelectingAll}
                      data-testid="linked-clear-selection"
                      onClick={clearSelection}
                    >
                      {t('linked_collection.clear_selection')}
                    </Button>
                  </div>
                </div>
              ) : (
                <>
                  <span data-name>{t('collection_detail_page.list.item')}</span>
                  <span>{t('collection_detail_page.list.category')}</span>
                  <span>
                    {t('linked_collection.list.status')}
                    {curations.isError && (
                      <button
                        className={styles.statusRetry}
                        type="button"
                        data-testid="linked-item-status-retry"
                        disabled={curations.isFetching}
                        onClick={() => void curations.refetch()}
                      >
                        {t('linked_collection.item_status.retry')}
                      </button>
                    )}
                  </span>
                  <span>{t('linked_collection.list.mapping')}</span>
                  <span>{t('collection_detail_page.list.actions')}</span>
                </>
              )}
            </div>
            {items.map(item => (
              <LinkedItemRow
                key={item.id}
                item={item}
                contract={contract}
                status={curations.isSuccess ? statuses.get(item.id) : undefined}
                selectable={selectableSet.has(item.id)}
                selected={selected.has(item.id)}
                onSelectedChange={isSelected => toggleSelected(item, isSelected)}
                editable={canEdit}
                onSaveMappings={saveMappings}
                onMappingDirtyChange={onMappingDirtyChange}
                actions={
                  <LinkedItemActionsMenu
                    onDeleted={() => toggleSelected(item, false)}
                    item={item}
                    collection={collection}
                    address={address}
                    curation={curations.isSuccess ? (curations.data.get(item.id) ?? null) : undefined}
                    canEdit={canEdit}
                    saveItemPatch={saveItemPatch}
                  />
                }
              />
            ))}
          </div>
          <div className={shared.footerRow}>
            <span className={shared.showingCount} data-testid="items-showing">
              {t('collection_detail_page.showing', {
                range: pageRangeLabel(page, LINKED_ITEMS_PAGE_SIZE, items.length),
                total
              })}
            </span>
            {pages > 1 && <Pagination page={page} pages={pages} onPageChange={onPageChange} />}
          </div>
        </>
      )}

      <input
        ref={filesInputRef}
        type="file"
        multiple
        accept={ITEM_EXTENSIONS.join(',')}
        hidden
        data-testid="add-items-input"
        onChange={event => {
          startAddItems(event.target.files)
          event.target.value = ''
        }}
      />

      <input
        ref={bulkInputRef}
        type="file"
        multiple
        accept=".zip"
        hidden
        data-testid="edit-in-bulk-input"
        onChange={event => {
          const list = event.target.files ? Array.from(event.target.files) : []
          if (list.length > 0) setBulkEditFiles(list)
          event.target.value = ''
        }}
      />
      {bulkEditFiles && (
        <AddItemsModal
          collection={collection}
          address={address}
          files={bulkEditFiles}
          updateOnly
          onClose={() => setBulkEditFiles(null)}
        />
      )}
      {addItemsFiles && (
        <AddItemsModal
          collection={collection}
          address={address}
          files={addItemsFiles}
          onClose={() => setAddItemsFiles(null)}
        />
      )}
      {isRenameOpen && (
        <CollectionNameModal
          variant="rename"
          initialName={collection.name}
          nameMaxLength={LINKED_COLLECTION_NAME_MAX_LENGTH}
          isPending={saveCollection.isPending}
          error={saveCollection.error?.message ?? null}
          onSubmit={name => {
            if (name === collection.name) return closeRename()
            saveCollection.mutate({ ...collection, name }, { onSuccess: closeRename })
          }}
          onClose={closeRename}
        />
      )}
      {isUrnOpen && decodedUrn && (
        <EditUrnModal
          kind="collection"
          urnPrefix={decodedUrn.thirdPartyId}
          initialSuffix={decodedUrn.collectionSuffix}
          isPending={saveCollection.isPending}
          error={saveCollection.isError ? t('linked_collection.urn_error') : null}
          onSubmit={saveUrn}
          onClose={closeUrn}
        />
      )}
      {relink && (relink.view.kind === 'form' || (relink.view.kind === 'running' && isSocialLogin(session))) && (
        <EditContractModal
          initial={relink.view.kind === 'form' ? relink.view.last : relink.view.to}
          isPending={relink.view.kind === 'running' || relinkInFlight}
          onSubmit={next => void relinkContract(next)}
          onClose={closeContract}
        />
      )}
      {relink && runningView && !isSocialLogin(session) && (
        <PendingModal
          label={t(
            runningView.step === 'signing' ? 'sell_item_modal.confirm_in_wallet' : 'edit_contract_modal.running.working'
          )}
          onCancel={
            runningView.step === 'signing'
              ? () => {
                  relinkAttempt.current++
                  setRelink({ from: relink.from, view: { kind: 'form', last: runningView.to } })
                }
              : undefined
          }
          testId="edit-contract-running"
        />
      )}
      {relink?.view.kind === 'success' && (
        <SuccessModal
          title={t('edit_contract_modal.success.title')}
          description={t('edit_contract_modal.success.body', { count: relink.view.linked })}
          onDone={closeContract}
        />
      )}
      {relink && errorView && (
        <ConfirmModal
          title={t('edit_contract_modal.error.title')}
          description={t(
            errorView.inUse
              ? 'create_linked_collection_modal.error_contract_in_use'
              : errorView.wallet
                ? 'wallet_error.wrong_account'
                : 'edit_contract_modal.error.body'
          )}
          art={{ src: errorArt }}
          onClose={closeContract}
          cancel={{
            label: t('publish_collection_modal.error.cancel'),
            onClick: closeContract,
            testId: 'edit-contract-error-cancel'
          }}
          confirm={{
            label: t('publish_collection_modal.error.try_again'),
            onClick: () => setRelink({ from: relink.from, view: { kind: 'form', last: errorView.to } }),
            testId: 'edit-contract-error-retry'
          }}
          testId="edit-contract-error"
        />
      )}
      {publishView.kind === 'publish' && thirdParty && (
        <PublishLinkedItemsModal
          collection={collection}
          thirdParty={thirdParty}
          session={session}
          itemsToPublish={publishView.toPublish}
          itemsToPush={publishView.toPush}
          onClose={closePublish}
          onPublished={onPublished}
        />
      )}
      {publishView.kind === 'push' && (
        <PushLinkedChangesModal
          collection={collection}
          session={session}
          items={publishView.toPush}
          onClose={closePublish}
          onPushed={onPublished}
        />
      )}
    </>
  )
}
