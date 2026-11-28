import { useCallback, useEffect, useMemo, useRef, useState, type DragEvent } from 'react'
import { useLocation, useNavigate, useParams, useSearchParams } from 'react-router'
import { useIntl } from '@/data/lib/creators/intl'
import { useTranslation } from '@/data/lib/creators/intl'
import { useWallet } from '../../store/wallet'
import {
  ITEMS_PAGE_SIZE,
  useAllCollectionItems,
  useCollection,
  useCollectionStatus,
  useSaveCollection
} from '../../hooks/useCollection'
import {
  BuilderServerError,
  boundRefusal,
  ValidationLimitError,
  isCollectionBusy,
  isValidationRunning
} from '@/data/lib/creators/builder'
import { isSocialLogin } from '@/data/lib/creators/auth'
import { FeatureFlag } from '@/data/lib/creators/featureFlags'
import { getValidationStatus } from '@/data/lib/creators/validation'
import {
  CollectionDisplayStatus,
  canManageCollectionItems,
  canSellCollectionItems,
  hasBeenApproved,
  hasCollectionRole,
  isCollectionLocked,
  isLinkedCollection
} from '@/data/lib/creators/collections'
import { parseUuidParam } from '@/data/lib/creators/ids'
import { track } from '@/data/lib/creators/analytics'
import { previewCollectionFromDetail } from '@/data/lib/creators/explorer'
import { isForumPostRelevant, openForumPost } from '@/data/lib/creators/forumPost'
import { cancelCreditsOrder } from '@/data/lib/creators/credits'
import { clearTopUpResume, parseTopUpReturn, readTopUpResume, stripTopUpReturn } from '@/data/lib/creators/creditsTopUp'
import { isCollectionOwner, type RoleKind } from '@/data/lib/creators/collectionRoles'
import { runningDiscountPpm } from '@/data/lib/creators/collectionDiscount'
import { toSaleableCollection } from '@/data/lib/creators/saleableCollection'
import { useManaUsdRate } from '../../hooks/useSales'
import type { CollectionDiscount } from '@/data/lib/creators/coupons'
import { getMaticChainId } from '@/data/lib/creators/publishCollection'
import { canSendCollectionItems } from '@/data/lib/creators/mint'
import { ItemType, canEditItemDetails, canEditItemPrice, type Item } from '@/data/lib/creators/items'
import {
  ITEM_TYPE_FILTERS,
  ItemTypeFilter,
  countItemsByType,
  filterItemsByType,
  paginateItems,
  parseItemTypeFilter
} from '@/data/lib/creators/itemFilters'
import { MAX_PUBLISH_ITEMS, getPublishBlocker } from '@/data/lib/creators/publishCollection'
import { useSaveItem } from '../../hooks/useSaveItem'
import { useRefusalText } from '../../hooks/useRefusalText'
import { useForumPostRecovery } from '../../hooks/useForumPost'
import { useLockedCollectionDeployed, useSyncPublishedItems } from '../../hooks/usePublishCollection'
import { usePublishInFlight } from '../../store/publishSync'
import { useThumbnailEditor } from '../../hooks/useThumbnailEditor'
import { useCollectionListings } from '../../hooks/useCollectionListings'
import { useBeforeUnloadGuard } from '../../hooks/useBeforeUnloadGuard'
import {
  DISCOUNT_OPENED_EVENT,
  useAnyDiscountInFlight,
  useCollectionDiscounts,
  useDiscountFlags
} from '../../hooks/useCollectionDiscounts'
import { useFeatureFlag } from '../../hooks/useFeatureFlag'
import { useMediaQuery } from '../../hooks/useMediaQuery'
import { useItemSyncs } from '../../hooks/useItemSync'
import {
  useCollectionValidation,
  useRerunItemValidation,
  type ItemValidation
} from '../../hooks/useCollectionValidation'
import {
  ItemSyncStatus,
  getItemRowStatus,
  hasPendingChanges,
  hasUpdatesSinceRejection,
  showsItemStatusColumn
} from '@/data/lib/creators/itemSync'
import {
  ReviewStage,
  canPushChanges,
  getReviewStage,
  hasReviewConversation,
  isRejectedStage
} from '@/data/lib/creators/curation'
import { useCommittee } from '../../hooks/useCommittee'
import { useCollectionCuration, usePushCuration } from '../../hooks/useCuration'
import { useCollectionEvents } from '../../hooks/useCollectionEvents'
import {
  NOTE_MAX_LENGTH,
  VALIDATION_ATTEMPTS_PER_DAY,
  getAutomatedIssuesByItem,
  countValidationAttempts,
  hasChangesSinceVerdict,
  startOfUtcDay
} from '@/data/lib/creators/events'
import { pageRangeLabel } from '@/data/lib/creators/pagination'
import { ITEM_EXTENSIONS, THUMBNAIL_PATH } from '@/data/lib/creators/itemFiles'
import type { ItemListing } from '@/data/lib/creators/listings'
import { useNotifications } from '@/data/lib/creators/notifications'
import { media } from '@/data/lib/creators/theme'
import { Button } from '../Button'
import { ConfirmModal } from '../ConfirmModal'
import { NoteField } from '../NoteField'
import { Tooltip } from '../Tooltip'
import {
  EmoteIcon,
  OpenEditorIcon,
  WearableIcon,
  AddIcon,
  ArrowBackIosNewIcon as ArrowBackIcon,
  EditIcon,
  ForumOutlinedIcon as ForumIcon,
  LocalOfferOutlinedIcon as DiscountIcon,
  PersonOutlineIcon,
  SyncIcon
} from '../Icons'
import { CollectionNameModal } from '../CollectionNameModal'
import { CollectionRolePill } from '../CollectionRolePill'
import { CollectionStatusPill } from '../CollectionStatusPill'
import { Pagination } from '../Pagination'
import { ValidationResultsModal, toValidationSubject } from '../ValidationBadge'
import addItemsArt from '../../assets/add-items.png'
import { CollectionActionsMenu } from '../CollectionActionsMenu'
import { useThirdParty } from '../../hooks/useLinkedCollection'
import { getThirdPartyId, isThirdPartyManager } from '@/data/lib/creators/thirdParty'
import { AddItemsModal } from './AddItemsModal'
import { LinkedCollectionView } from './LinkedCollectionView'
import { ReviewFlow, type ReviewFlowRequest } from './ReviewFlow'
import { ItemActionsMenu } from './ItemActionsMenu'
import { ItemListRow } from './ItemListRow'
import { PublishCollectionModal, PublishSuccessModal, type PublishResume } from './PublishCollectionModal'
import { ValidationGate } from './PublishCollectionModal/ValidationGate'
import { SellItemFlow, UpdatePriceFlow } from './SellItemFlow'
import { ManageRolesFlow } from './ManageRolesFlow'
import { TransferOwnershipFlow } from './TransferOwnershipFlow'
import { SendItemsFlow } from './SendItemsFlow'
import { CollectionDiscounts, DiscountFlow, EndDiscountFlow } from './DiscountFlow'
import { cx } from '@/data/lib/creators/cx'
import styles from './CollectionDetailPage.module.css'
import row from './ItemListRow/ItemListRow.module.css'
import shared from '../../styles/shared.module.css'

const NOT_FOUND_STATUSES = [401, 403, 404]
const NO_ITEMS: Item[] = []

const CollectionDetailPage = () => {
  const { t } = useTranslation()
  const refusalText = useRefusalText()
  const intl = useIntl()
  const navigate = useNavigate()
  const location = useLocation()
  const listState = location.state as { listSearch?: string } | null
  const { collectionId: collectionIdParam } = useParams()
  const collectionId = parseUuidParam(collectionIdParam)
  const { session, restored, signIn } = useWallet()
  const address = session?.address

  const [searchParams, setSearchParams] = useSearchParams()
  const page = Math.max(1, Number(searchParams.get('page')) || 1)
  const typeFilter = parseItemTypeFilter(searchParams.get('type'))

  const [isRenameOpen, setRenameOpen] = useState(false)
  const [publishView, setPublishView] = useState<'closed' | 'wizard' | 'success'>('closed')
  const [publishResume, setPublishResume] = useState<PublishResume | undefined>(undefined)
  const topUpReturn = useMemo(() => parseTopUpReturn(searchParams), [searchParams])
  const [addItemsFiles, setAddItemsFiles] = useState<File[] | null>(null)
  const [isDragging, setDragging] = useState(false)
  const [sellingItem, setSellingItem] = useState<Item | null>(null)
  // The price dialog keeps the listing it opened with: the flow rewrites the listings cache itself.
  const [priceEdit, setPriceEdit] = useState<{ item: Item; listing: ItemListing & { tradeId: string } } | null>(null)
  const filesInputRef = useRef<HTMLInputElement>(null)

  // Back from buying credits: reopen the wizard on the payment step the creator left. Waits for the
  // session, since a signed-out visitor is sent to sign in and comes back to this same URL.
  useEffect(() => {
    if (!topUpReturn || !restored || !session) return
    const resume = readTopUpResume()
    clearTopUpResume()
    setSearchParams(prev => stripTopUpReturn(prev), { replace: true })
    if (!resume || resume.collectionId !== collectionId || resume.orderId !== topUpReturn.orderId) return
    if (topUpReturn.canceled) {
      track('bd_credits_checkout_canceled', { collection_id: collectionId, order_id: topUpReturn.orderId })
      void cancelCreditsOrder(session.address, topUpReturn.orderId)
    }
    setPublishResume({
      paymentMethod: resume.paymentMethod,
      termsAccepted: resume.termsAccepted,
      email: resume.email,
      orderId: topUpReturn.canceled ? null : topUpReturn.orderId
    })
    setPublishView('wizard')
  }, [topUpReturn, restored, session, collectionId, setSearchParams])

  const collectionQuery = useCollection(address, collectionId)
  // Linked collections page their items server-side (they can hold thousands): the whole-list query starts with the
  // collection's, so standard pages load both in parallel, and stops once the collection turns out to be linked.
  const itemsQuery = useAllCollectionItems(
    address,
    collectionQuery.data && isLinkedCollection(collectionQuery.data) ? undefined : collectionId
  )
  const saveCollection = useSaveCollection(address)
  const updateItem = useSaveItem(address)
  const showToast = useNotifications(state => state.showToast)
  const thumbnailEditor = useThumbnailEditor(async (patch, subject) => {
    if (subject.kind !== 'item') return
    const { item } = subject
    await updateItem.mutateAsync({ item, thumbnail: patch.contents[THUMBNAIL_PATH] })
  })
  // Small screens are a viewer: the in-row edit shortcuts are desktop-only, like the menu's edit actions.
  const compact = useMediaQuery(media.noActions)

  const collection = collectionQuery.data
  // Linked collections are read-only here: none of the publishing, sales or sync machinery runs for them.
  const isLinked = !!collection && isLinkedCollection(collection)
  const standardCollection = isLinked ? undefined : collection
  const thirdParty = useThirdParty(address, isLinked ? collection : undefined)
  const isLinkedManager = !!thirdParty.data && isThirdPartyManager(thirdParty.data, address)
  const hasRole = !!collection && (isLinked ? isLinkedManager : hasCollectionRole(collection, address))
  const allItems = itemsQuery.data
  const total = allItems?.length ?? 0
  const counts = useMemo(() => countItemsByType(allItems ?? []), [allItems])
  const {
    results,
    total: filteredTotal,
    pages
  } = useMemo(
    () => paginateItems(filterItemsByType(allItems ?? [], typeFilter), page, ITEMS_PAGE_SIZE),
    [allItems, typeFilter, page]
  )
  // Play Mode is an emote-only attribute; the column exists only while the visible page has emotes.
  const withPlayMode = useMemo(() => results.some(item => item.type === ItemType.EMOTE), [results])
  const canManage = !!standardCollection && canManageCollectionItems(standardCollection, address)
  // builder-server serves a collection's curation to its owner, collaborators and the committee only.
  const committee = useCommittee(address)
  // Without a role the page is "not found", so curators only read it where they hold one.
  const curationAddress = canManage || (committee.isCurator && hasRole) ? address : undefined
  useSyncPublishedItems(canManage ? address : undefined, standardCollection, allItems ?? [])
  useForumPostRecovery(address, standardCollection, allItems ?? [])
  // Price, Sales and Sale Status exist once the collection is published; the owner can put items on sale
  // once it has been approved at least once, even if it is under review again.
  const withMarket = !!standardCollection?.isPublished && hasRole
  const status = useCollectionStatus(curationAddress, standardCollection)
  const showForumPost = !!standardCollection && isForumPostRelevant(standardCollection, status)
  // Sales here are off-chain public orders only: with the flag off there is no other way to list an item.
  const canListItems = useFeatureFlag(FeatureFlag.OFFCHAIN_PUBLIC_ITEM_ORDERS).enabled
  const isApprovedForSale = canListItems && !!collection && hasBeenApproved(collection)
  const isSeller = canListItems && !!collection && canSellCollectionItems(collection, address)
  const canSend = useMemo(
    () => !!standardCollection && canSendCollectionItems(standardCollection, address),
    [standardCollection, address]
  )
  const [isSending, setSending] = useState(false)
  const [managingRoles, setManagingRoles] = useState<RoleKind | null>(null)
  const [isTransferring, setTransferring] = useState(false)
  const listingsQuery = useCollectionListings(withMarket ? standardCollection.contractAddress : undefined)
  const listings = listingsQuery.data
  // `undefined` keeps the price cell blank while the catalog loads; a failed request shows no price rather than an error.
  const listingFor = (item: Item) =>
    listings ? (listings.get(item.tokenId ?? '') ?? null) : listingsQuery.isError ? null : undefined
  const discountFlags = useDiscountFlags()
  // Only the collection's creator can sign a discount over it: marketplace-server checks the on-chain creator.
  // While the Shop applies discounts, running ones stay listed and endable even with creation off here.
  const canManageDiscounts =
    isApprovedForSale &&
    discountFlags.shopApplies &&
    !!standardCollection?.contractAddress &&
    isCollectionOwner(standardCollection, address)
  // Offered once something is listed, even if none of it can take a discount: the modal explains what to change.
  const canCreateDiscountHere =
    canManageDiscounts && discountFlags.enabled && discountFlags.manaListings !== undefined && !!listings?.size
  const { discounts: openDiscounts, now: discountsNow } = useCollectionDiscounts(
    canManageDiscounts ? address : undefined,
    standardCollection?.contractAddress
  )
  const discountReachOptions = useMemo(
    () => ({ chainId: getMaticChainId(), manaListings: !!discountFlags.manaListings }),
    [discountFlags.manaListings]
  )
  const discountPpmFor = (listing: ItemListing | null | undefined) =>
    listing && openDiscounts
      ? runningDiscountPpm(listing, openDiscounts, discountReachOptions, discountsNow)
      : undefined
  const [discountOpen, setDiscountOpen] = useState(false)
  // A MANA listing the discount reaches is previewed in credits, at the live rate.
  const reachesMana = useMemo(
    () => !!discountFlags.manaListings && [...(listings?.values() ?? [])].some(listing => listing.currency === 'mana'),
    [listings, discountFlags.manaListings]
  )
  // Only once the modal is open: the rate is three contract reads, and only its preview needs it.
  const manaRate = useManaUsdRate(discountOpen && reachesMana).data
  const sale = useMemo(
    () =>
      listings && standardCollection && allItems
        ? toSaleableCollection(standardCollection, allItems, listings, { ...discountReachOptions, manaRate })
        : null,
    [listings, standardCollection, allItems, discountReachOptions, manaRate]
  )
  // Offered when something can take a discount, or is listed at a price the modal explains how to change. Listings
  // of sold-out items alone leave nothing a discount could reach.
  const canCreateDiscount = useMemo(
    () => canCreateDiscountHere && !!sale?.items.some(item => item.state === 'discounted' || item.state === 'classic'),
    [canCreateDiscountHere, sale]
  )
  // Also once its dialog is closed: a reload loses this tab's record of a cancellation that is still mining.
  useBeforeUnloadGuard(useAnyDiscountInFlight())
  const [endingDiscount, setEndingDiscount] = useState<CollectionDiscount | null>(null)
  const openDiscount = useCallback(
    (trigger: 'header' | 'menu') => {
      // Not `source`: that prop is reserved for the app discriminator and would overwrite this one.
      if (collection) track(DISCOUNT_OPENED_EVENT, { collection_id: collection.id, trigger })
      setDiscountOpen(true)
    },
    [collection]
  )
  const syncs = useItemSyncs(curationAddress, standardCollection, allItems ?? [])
  const curationQuery = useCollectionCuration(curationAddress, standardCollection)
  const curation = curationQuery.data ?? null
  const eventsQuery = useCollectionEvents(curationAddress, standardCollection)
  const { events } = eventsQuery
  const pushCuration = usePushCuration(address)
  // Sending changes for review runs the same item checks as publishing before its confirm modal.
  const [pushView, setPushView] = useState<'closed' | 'gate' | 'confirm'>('closed')
  const hasUnsyncedItems = useMemo(
    () => [...syncs.values()].some(sync => sync.status === ItemSyncStatus.UNSYNCED),
    [syncs]
  )
  const [pushNote, setPushNote] = useState('')
  const closePush = () => {
    setPushView('closed')
    setPushNote('')
    pushCuration.reset()
  }
  const stage = useMemo(
    () => (standardCollection ? getReviewStage(standardCollection, curation, events) : null),
    [standardCollection, curation, events]
  )
  const statusHint =
    status === CollectionDisplayStatus.REJECTED
      ? isRejectedStage(stage)
        ? t(`review_stage_hint.${stage}`)
        : t('collection_detail_page.review_notice.rejected')
      : status === CollectionDisplayStatus.UNDER_REVIEW
        ? t('collection_status.under_review_hint')
        : status === CollectionDisplayStatus.DISABLED
          ? t('collection_status.disabled_hint')
          : null
  // The validator judges a snapshot: nothing changes under it (Figma: "block edit settings during these 5 mins").
  const reviewLocked = stage === ReviewStage.AUTOMATED_REVIEWING
  const autoCuration = useFeatureFlag(FeatureFlag.AUTO_CURATION_REVIEW_FLOW).enabled
  const [reviewRequest, setReviewRequest] = useState<ReviewFlowRequest | null>(null)
  // Only a server with the review timeline keeps the note.
  const withPushNote = autoCuration && events !== null
  const clearReviewRequest = useCallback(() => setReviewRequest(null), [])
  // The validator's findings dress the rows and the details modal like the browser's checks do.
  const automatedIssues = useMemo(() => getAutomatedIssuesByItem(events ?? [], allItems ?? []), [events, allItems])
  // Until the request loads, a pending one looks like none and the push would duplicate it. An automated
  // rejection of a first review is never pushed: it is reviewed again or sent to a curator.
  const curationLoaded = curationQuery.isSuccess
  const showPushChanges = useMemo(
    () =>
      !!standardCollection &&
      curationLoaded &&
      !(stage === ReviewStage.REJECTED_BY_VALIDATOR && !standardCollection.isApproved) &&
      canPushChanges(standardCollection, curation, hasUnsyncedItems, canManage),
    [standardCollection, curationLoaded, stage, curation, hasUnsyncedItems, canManage]
  )
  // An approval ends the conversation for both sides, as the server does.
  const commentAs = !hasReviewConversation(curation)
    ? undefined
    : canManage
      ? ('creator' as const)
      : committee.isCurator
        ? ('curator' as const)
        : undefined
  const showReviewAgain = canManage && curationLoaded && stage === ReviewStage.REJECTED_BY_VALIDATOR
  // After a curator's request for fixes, reading the feedback is the main action until an item is updated.
  const awaitsFixes = useMemo(
    () => stage === ReviewStage.REJECTED_BY_CURATOR && !hasUpdatesSinceRejection(curation, allItems ?? []),
    [stage, curation, allItems]
  )
  // The same content always gets the same result, so another automated review waits for a saved or added item.
  const hasNewContent = useMemo(() => hasChangesSinceVerdict(events ?? [], allItems ?? []), [events, allItems])
  const today = startOfUtcDay(Date.now())
  const reviewsUsed = useMemo(() => countValidationAttempts(events ?? [], today), [events, today])
  // After a rejection the creator asks for a review again, whether it was the first one or a pushed update.
  const pushCopy = curation?.status === 'rejected' ? 'request_review' : 'push_changes'
  const rowStatuses = useMemo(
    () => new Map(results.map(item => [item.id, getItemRowStatus(item, syncs.get(item.id), curation)])),
    [results, syncs, curation]
  )
  const withStatus = useMemo(
    () => !!standardCollection && showsItemStatusColumn(standardCollection, rowStatuses.values()),
    [standardCollection, rowStatuses]
  )

  // Drafts check every item; published collections only the items with changes waiting for approval.
  // Only who can fix the findings checks. Small screens are a viewer and check nothing, unless a gated
  // modal is already open: crossing the breakpoint mid-check must not empty its results and let it through.
  const validates = canManage && (!compact || publishView === 'wizard' || pushView !== 'closed')
  const validationItems = useMemo(
    () =>
      !validates || !standardCollection || !allItems
        ? NO_ITEMS
        : allItems.filter(item => !standardCollection.isPublished || hasPendingChanges(syncs.get(item.id)?.status)),
    [validates, standardCollection, allItems, syncs]
  )
  const validation = useCollectionValidation(validationItems, validates)
  const blockOnErrors = useFeatureFlag(FeatureFlag.BLOCK_PUBLISH_ON_VALIDATION_ERRORS).enabled
  const publishValidation = useMemo(
    () => ({
      // A background refetch (back from saving in the editor) may bring new hashes the modal must wait for.
      isValidating: itemsQuery.isFetching || validation.isValidating,
      results: validationItems.map(item => ({ item, issues: validation.results.get(item.id)?.issues ?? [] }))
    }),
    [itemsQuery.isFetching, validation, validationItems]
  )
  const rerunItemValidation = useRerunItemValidation()
  const rowValidations = useMemo(() => {
    const merged = new Map<string, ItemValidation>(validation.results)
    for (const [itemId, automated] of automatedIssues) {
      const local = validation.results.get(itemId)
      const issues = [...(local?.issues ?? []), ...automated]
      merged.set(itemId, {
        status: local?.status === 'loading' ? 'loading' : getValidationStatus(issues, false),
        issues
      })
    }
    return merged
  }, [validation.results, automatedIssues])
  // Counted from what the rows show, so the push is blocked by the same errors the rows say block it.
  const invalidCount = useMemo(
    () => [...rowValidations.values()].filter(result => result.status === 'errors').length,
    [rowValidations]
  )
  const rowValidation = useCallback((item: Item) => rowValidations.get(item.id), [rowValidations])
  const [validationItem, setValidationItem] = useState<Item | null>(null)
  const showValidation = useCallback(
    (item: Item) => {
      setValidationItem(item)
      track('bd_item_validation_details_opened', {
        item_id: item.id,
        status: rowValidation(item)?.status ?? 'idle',
        trigger: 'row'
      })
    },
    [rowValidation]
  )

  const isLoading =
    !restored ||
    (!!address &&
      (collectionQuery.isLoading || (!isLinked && !!collection && itemsQuery.isLoading) || thirdParty.isLoading))
  const isThirdPartyNotFound =
    thirdParty.isError &&
    thirdParty.error instanceof BuilderServerError &&
    NOT_FOUND_STATUSES.includes(thirdParty.error.status)
  // The detail page is for the collection's roles only (a linked collection's are its third party's managers):
  // anyone else gets the same "not found" as a rejected request, even once published, when only the editor is public.
  // A finished transfer can drop the owner's role at once, so the page holds until its success modal is dismissed.
  const isNotFound =
    !collectionId ||
    (collectionQuery.isError &&
      collectionQuery.error instanceof BuilderServerError &&
      NOT_FOUND_STATUSES.includes(collectionQuery.error.status)) ||
    (!!collection &&
      (isLinked
        ? // A third-party request that failed otherwise is a retryable error, not a missing role.
          !getThirdPartyId(collection) || isThirdPartyNotFound || (!thirdParty.isError && !hasRole)
        : !hasRole && !isTransferring))
  const isError = !isNotFound && (collectionQuery.isError || (!isLinked && itemsQuery.isError) || thirdParty.isError)
  const isEmpty = filteredTotal === 0
  const hasItems = total > 0
  const canRename = !!collection && !collection.isPublished && !isCollectionLocked(collection)
  const canAddItems = canRename
  const hasInvalidItems = blockOnErrors && invalidCount > 0
  // A locked draft's last transaction may have reverted: publishing again is safe, as at most one can land.
  const isPublishRetry = !!collection && isCollectionLocked(collection)
  const publishInFlight = usePublishInFlight(collection?.id)
  // A retry is hidden once an earlier attempt is known to have landed; a failed read still offers it.
  const deployed = useLockedCollectionDeployed(isPublishRetry ? collection : undefined)
  const retryLanded = isPublishRetry && (deployed.isLoading || deployed.data === true)
  const publishBlocker =
    collection && !publishInFlight && !retryLanded
      ? getPublishBlocker(collection, total, allItems ?? [], { hasInvalidItems })
      : 'not_draft'
  const pushBlocked = hasInvalidItems || reviewLocked

  function openFileBrowser() {
    filesInputRef.current?.click()
  }

  function startAddItems(files: FileList | File[] | null) {
    const list = files ? Array.from(files) : []
    if (list.length > 0) setAddItemsFiles(list)
  }

  const closeAddItems = useCallback(() => setAddItemsFiles(null), [])

  function onDropzoneDrop(event: DragEvent) {
    event.preventDefault()
    setDragging(false)
    if (canAddItems) startAddItems(event.dataTransfer.files)
  }

  function goToPage(next: number) {
    setSearchParams(
      prev => {
        const params = new URLSearchParams(prev)
        if (next > 1) params.set('page', String(next))
        else params.delete('page')
        return params
      },
      // Keeps the list's query that the back arrow returns to.
      { replace: true, state: listState }
    )
    window.scrollTo({ top: 0 })
  }

  function changeTypeFilter(next: ItemTypeFilter) {
    setSearchParams(
      prev => {
        const params = new URLSearchParams(prev)
        if (next === ItemTypeFilter.ALL) params.delete('type')
        else params.set('type', next)
        params.delete('page')
        return params
      },
      { replace: true, state: listState }
    )
  }

  function renameItem(item: Item, name: string) {
    return updateItem.mutateAsync({ item: { ...item, name } }).catch((error: unknown) => {
      showToast(t('collection_detail_page.item_row.rename_error', { name: item.name }), { type: 'error' })
      throw error
    })
  }

  function openPriceEdit(item: Item) {
    if (!collection) return
    const listing = listingFor(item)
    if (canEditItemPrice(collection, item, listing, address)) setPriceEdit({ item, listing })
  }

  function closeRenameModal() {
    setRenameOpen(false)
    saveCollection.reset()
  }

  function onRenameSubmit(name: string) {
    if (!collection || name === collection.name) {
      closeRenameModal()
      return
    }
    saveCollection.mutate({ ...collection, name }, { onSuccess: closeRenameModal })
  }

  return (
    <div className={styles.page} data-testid="collection-detail-page">
      {restored && !session ? (
        <section className={shared.panel} data-testid="sign-in-panel">
          <div className={styles.signInIconWrap} aria-hidden>
            <PersonOutlineIcon />
          </div>
          <h2 className={shared.panelTitle}>{t('collection_detail_page.sign_in.title')}</h2>
          <Button type="button" variant="primary" data-testid="sign-in" onClick={() => signIn()}>
            {t('collection_detail_page.sign_in.action')}
          </Button>
        </section>
      ) : isLoading ? (
        <div className={styles.loading} aria-hidden data-testid="collection-detail-loading">
          <div className={styles.header}>
            <div className={styles.headerLeft}>
              <div className={cx(styles.skeletonTitle, 'skeleton')} />
            </div>
            <div className={styles.headerActions}>
              <div className={cx(styles.skeletonButton, 'skeleton')} data-desktop-only />
              <div className={cx(styles.skeletonButton, 'skeleton')} data-desktop-only />
              <div className={cx(styles.skeletonButton, 'skeleton')} data-icon />
            </div>
          </div>
          <div className={styles.subHeader}>
            <div className={styles.filterChips}>
              <div className={cx(styles.skeletonChip, 'skeleton')} />
              <div className={cx(styles.skeletonChip, 'skeleton')} />
              <div className={cx(styles.skeletonChip, 'skeleton')} />
            </div>
            <div className={styles.subActions}>
              <div className={cx(styles.skeletonButton, 'skeleton')} data-compact />
              <div className={cx(styles.skeletonButton, 'skeleton')} data-compact />
            </div>
          </div>
          <div className={styles.list}>
            <div className={cx(styles.skeletonListHeader, 'skeleton')} />
            {Array.from({ length: 5 }, (_, i) => (
              <div className={cx(styles.skeletonRow, 'skeleton')} key={i} />
            ))}
          </div>
        </div>
      ) : isNotFound ? (
        <section className={shared.panel} data-testid="collection-not-found">
          <h2 className={shared.panelTitle}>{t('collection_detail_page.not_found.title')}</h2>
          <p className={shared.panelText}>{t('collection_detail_page.not_found.description')}</p>
          <Button type="button" variant="secondary" onClick={() => navigate('/create/collections')}>
            {t('collection_detail_page.not_found.back')}
          </Button>
        </section>
      ) : isError || !collection ? (
        <section className={shared.panel} data-testid="collection-detail-error">
          <h2 className={shared.panelTitle}>{t('collection_detail_page.error.title')}</h2>
          <p className={shared.panelText}>
            {refusalText(boundRefusal(collectionQuery.error ?? itemsQuery.error ?? thirdParty.error)) ??
              t('collection_detail_page.error.description')}
          </p>
          <Button
            type="button"
            variant="secondary"
            onClick={() => {
              void collectionQuery.refetch()
              void itemsQuery.refetch()
              if (thirdParty.isError) void thirdParty.refetch()
            }}
          >
            {t('collection_detail_page.error.retry')}
          </Button>
        </section>
      ) : isLinked && session ? (
        <LinkedCollectionView
          collection={collection}
          thirdParty={thirdParty.data}
          session={session}
          page={page}
          onBack={() => navigate({ pathname: '/create/collections', search: listState?.listSearch })}
          onPageChange={goToPage}
        />
      ) : (
        <>
          <div className={styles.header}>
            <div className={styles.headerLeft}>
              <button
                className={styles.backLink}
                type="button"
                aria-label={t('collection_detail_page.back')}
                data-testid="back-to-collections"
                onClick={() =>
                  navigate({
                    pathname: '/create/collections',
                    search: listState?.listSearch
                  })
                }
              >
                <ArrowBackIcon />
              </button>
              <div className={styles.titleGroup}>
                <h1 className={styles.title} data-testid="collection-name" title={collection.name}>
                  {collection.name}
                </h1>
                {canRename && (
                  <button
                    className={styles.renameButton}
                    type="button"
                    aria-label={t('collection_detail_page.rename')}
                    data-testid="rename-collection"
                    onClick={() => setRenameOpen(true)}
                  >
                    <EditIcon fontSize="small" />
                  </button>
                )}
              </div>
              {status && <CollectionStatusPill collection={collection} status={status} hint={statusHint} />}
              {address && <CollectionRolePill collection={collection} address={address} />}
            </div>
            <div className={styles.headerActions}>
              {validation.isValidating && (
                <Tooltip content={t('item_validation.checking')} asChild testId="validation-spinner-tooltip">
                  <span
                    className={cx(shared.spinner, styles.validationSpinner)}
                    role="status"
                    aria-label={t('item_validation.checking')}
                    tabIndex={0}
                    data-desktop-only
                    data-testid="validation-spinner"
                  />
                </Tooltip>
              )}
              {showForumPost && !autoCuration && (
                <Button
                  type="button"
                  variant="dark"
                  data-desktop-only
                  data-testid="forum-post"
                  onClick={() => openForumPost(collection, 'detail')}
                >
                  <ForumIcon fontSize="small" />
                  {t('collection_detail_page.forum_post')}
                </Button>
              )}
              {publishBlocker !== 'not_draft' && (
                <Tooltip
                  content={
                    publishBlocker
                      ? t(`collection_detail_page.publish_blocker.${publishBlocker}`, {
                          max: MAX_PUBLISH_ITEMS,
                          count: invalidCount
                        })
                      : isPublishRetry
                        ? t('collection_detail_page.publish_again_hint')
                        : null
                  }
                  placement="bottom"
                  asChild
                  testId="publish-blocker"
                >
                  <Button
                    type="button"
                    variant="primary"
                    data-desktop-only
                    data-testid="publish-collection"
                    data-blocker={publishBlocker ?? undefined}
                    data-retry={isPublishRetry || undefined}
                    aria-disabled={publishBlocker ? true : undefined}
                    onClick={() => {
                      if (publishBlocker) return
                      setPublishResume(undefined)
                      setPublishView('wizard')
                    }}
                  >
                    {t(isPublishRetry ? 'collection_detail_page.publish_again' : 'collection_detail_page.publish')}
                  </Button>
                </Tooltip>
              )}
              {canSend && (
                <Button
                  type="button"
                  variant="dark"
                  disabled={!hasItems}
                  data-desktop-only
                  data-testid="send-items"
                  onClick={() => setSending(true)}
                >
                  {t('collection_detail_page.send_items')}
                </Button>
              )}
              {canCreateDiscount && (
                <Button
                  type="button"
                  variant="dark"
                  data-desktop-only
                  data-testid="create-discount"
                  onClick={() => openDiscount('header')}
                >
                  <DiscountIcon fontSize="small" />
                  {t('collection_discount.create')}
                </Button>
              )}
              {showPushChanges && (
                <Tooltip
                  content={
                    reviewLocked
                      ? t('collection_detail_page.push_blocker.reviewing')
                      : pushBlocked
                        ? t('collection_detail_page.push_blocker.has_invalid_items', { count: invalidCount })
                        : null
                  }
                  placement="bottom"
                  asChild
                  testId="push-changes-blocker"
                >
                  <Button
                    type="button"
                    variant={awaitsFixes ? 'dark' : 'primary'}
                    data-desktop-only
                    data-testid="push-changes"
                    aria-disabled={pushBlocked || undefined}
                    onClick={() => {
                      if (!pushBlocked) setPushView('gate')
                    }}
                  >
                    {t(`collection_detail_page.${pushCopy}.action`)}
                  </Button>
                </Tooltip>
              )}
              {showReviewAgain && (
                <Tooltip
                  content={
                    hasNewContent
                      ? t('review_flow.attempts', {
                          used: Math.min(reviewsUsed, VALIDATION_ATTEMPTS_PER_DAY),
                          max: VALIDATION_ATTEMPTS_PER_DAY
                        })
                      : t('review_flow.no_changes')
                  }
                  placement="bottom"
                  asChild
                  testId="validate-again-tooltip"
                >
                  <Button
                    type="button"
                    variant="primary"
                    data-desktop-only
                    data-testid="validate-again"
                    aria-disabled={!hasNewContent || undefined}
                    onClick={() => {
                      if (hasNewContent) setReviewRequest('validate')
                    }}
                  >
                    {t('review_flow.validate_again')}
                  </Button>
                </Tooltip>
              )}
              {address && (
                <CollectionActionsMenu
                  collection={collection}
                  address={address}
                  onPreviewInWorld={hasItems ? () => previewCollectionFromDetail(collection.id, total) : undefined}
                  items={allItems}
                  showActivity={autoCuration && !!curationAddress && !!standardCollection?.isPublished}
                  commentAs={commentAs}
                  forumInMenu={autoCuration}
                  onSendItems={canSend ? () => setSending(true) : undefined}
                  onCreateDiscount={canCreateDiscount ? () => openDiscount('menu') : undefined}
                  onPreviewItems={() => navigate(`/create/collections/editor?collection=${collection.id}`)}
                  forumLink={showForumPost ? collection.forumLink : undefined}
                  onManageRoles={setManagingRoles}
                  onTransferOwnership={() => setTransferring(true)}
                  onDeleted={() => navigate('/create/collections', { replace: true })}
                />
              )}
            </div>
          </div>
          {canManageDiscounts && openDiscounts && (
            <CollectionDiscounts
              discounts={openDiscounts}
              now={discountsNow}
              onEnd={session && !isSocialLogin(session) ? setEndingDiscount : undefined}
            />
          )}

          {address && standardCollection && (
            <ReviewFlow
              collection={standardCollection}
              address={address}
              curation={curation}
              events={events}
              items={allItems ?? []}
              canManage={canManage}
              request={reviewRequest}
              onRequestHandled={clearReviewRequest}
            />
          )}

          <div className={styles.subHeader}>
            <div className={styles.filterChips} data-testid="type-filters">
              {ITEM_TYPE_FILTERS.map(filter => (
                <button
                  className={styles.filterChip}
                  key={filter}
                  type="button"
                  data-active={typeFilter === filter || undefined}
                  data-testid={`type-filter-${filter}`}
                  onClick={() => changeTypeFilter(filter)}
                >
                  {filter === ItemTypeFilter.WEARABLE && <WearableIcon />}
                  {filter === ItemTypeFilter.EMOTE && <EmoteIcon />}
                  {t(`collection_detail_page.filter.${filter}`, { count: counts[filter] })}
                </button>
              ))}
            </div>
            <div className={styles.subActions}>
              <Button
                variant="secondary"
                type="button"
                data-testid="open-editor"
                onClick={() => navigate(`/create/collections/editor?collection=${collection.id}`)}
              >
                <OpenEditorIcon />
                {t(canManage ? 'collection_detail_page.open_editor' : 'collection_detail_page.open_preview')}
              </Button>
              {canAddItems && (
                <Button variant="secondary" type="button" data-testid="add-items" onClick={openFileBrowser}>
                  <AddIcon fontSize="small" />
                  {t('collection_detail_page.add_items')}
                </Button>
              )}
            </div>
          </div>

          {isEmpty && !canAddItems ? (
            <section className={shared.panel} data-testid="collection-no-items">
              <img className={styles.dropArt} src={addItemsArt} alt="" />
              <h2 className={shared.panelTitle}>{t(`collection_detail_page.no_items.${typeFilter}`)}</h2>
              <p className={shared.panelText}>{t('collection_detail_page.no_items.description')}</p>
            </section>
          ) : isEmpty ? (
            <>
              <section
                className={styles.dropzone}
                data-testid="collection-empty"
                data-dragging={isDragging || undefined}
                onDragOver={event => {
                  event.preventDefault()
                  if (canAddItems) setDragging(true)
                }}
                onDragLeave={() => setDragging(false)}
                onDrop={onDropzoneDrop}
              >
                <img className={styles.dropArt} src={addItemsArt} alt="" />
                <h3 className={styles.dropTitle} data-desktop>
                  {t('collection_detail_page.empty.title')}
                </h3>
                <h3 className={styles.dropTitle} data-mobile>
                  {t('collection_detail_page.empty.title_mobile')}
                </h3>
                <p className={styles.dropText} data-desktop>
                  {intl.formatMessage(
                    { id: 'collection_detail_page.empty.description' },
                    {
                      browse: chunks => (
                        <button
                          className={styles.browseLink}
                          type="button"
                          data-testid="browse-files"
                          onClick={openFileBrowser}
                        >
                          {chunks}
                        </button>
                      )
                    }
                  )}
                </p>
                <p className={styles.dropText} data-mobile>
                  {t('collection_detail_page.empty.description_mobile')}
                </p>
                <p className={styles.dropFormats}>{t('collection_detail_page.empty.formats')}</p>
              </section>
            </>
          ) : (
            <>
              <div className={styles.list} data-testid="items-list">
                <div
                  className={cx(row.columns, styles.listHeader)}
                  data-with-play-mode={withPlayMode || undefined}
                  data-with-market={withMarket || undefined}
                  data-with-status={withStatus || undefined}
                >
                  <span>{t('collection_detail_page.list.item')}</span>
                  <span>{t('collection_detail_page.list.body_shape')}</span>
                  <span>{t('collection_detail_page.list.category')}</span>
                  {withPlayMode && (
                    <span data-testid="list-header-play-mode">{t('collection_detail_page.list.play_mode')}</span>
                  )}
                  <span>{t('collection_detail_page.list.rarity')}</span>
                  {withMarket && (
                    <>
                      <span data-testid="list-header-price">{t('collection_detail_page.list.price')}</span>
                      <span data-testid="list-header-sales">{t('collection_detail_page.list.sales')}</span>
                      <span data-testid="list-header-sale-status">{t('collection_detail_page.list.sale_status')}</span>
                    </>
                  )}
                  {withStatus && (
                    <span data-testid="list-header-status">{t('collection_detail_page.list.status')}</span>
                  )}
                  <span className={styles.listHeaderActions}>{t('collection_detail_page.list.actions')}</span>
                </div>
                {results.map(item => (
                  <ItemListRow
                    key={item.id}
                    item={item}
                    withPlayMode={withPlayMode}
                    withMarket={withMarket}
                    listing={withMarket ? listingFor(item) : undefined}
                    discountPpm={withMarket ? discountPpmFor(listingFor(item)) : undefined}
                    withStatus={withStatus}
                    syncStatus={rowStatuses.get(item.id)}
                    canSell={isApprovedForSale && item.isPublished && !!item.tokenId}
                    onPutOnSale={isSeller ? setSellingItem : undefined}
                    onEditPrice={
                      canListItems &&
                      !compact &&
                      session &&
                      canEditItemPrice(collection, item, listingFor(item), address)
                        ? openPriceEdit
                        : undefined
                    }
                    editable={!compact && !reviewLocked && canEditItemDetails(collection, item, address)}
                    onRename={renameItem}
                    onEditThumbnail={item => thumbnailEditor.edit({ kind: 'item', item })}
                    contractAddress={collection.contractAddress}
                    validation={rowValidation(item)}
                    validationBlocks={blockOnErrors}
                    onShowValidation={showValidation}
                    actions={
                      address && (
                        <ItemActionsMenu
                          item={item}
                          collection={collection}
                          address={address}
                          sync={syncs.get(item.id)}
                          listing={withMarket ? listingFor(item) : undefined}
                          reviewLocked={reviewLocked}
                        />
                      )
                    }
                  />
                ))}
              </div>
              <div className={shared.footerRow}>
                <span className={shared.showingCount} data-testid="items-showing">
                  {t('collection_detail_page.showing', {
                    range: pageRangeLabel(page, ITEMS_PAGE_SIZE, results.length),
                    total: filteredTotal
                  })}
                </span>
                {pages > 1 && <Pagination page={page} pages={pages} onPageChange={goToPage} />}
              </div>
            </>
          )}

          {isRenameOpen && (
            <CollectionNameModal
              variant="rename"
              initialName={collection.name}
              isPending={saveCollection.isPending}
              error={saveCollection.error?.message ?? null}
              onSubmit={onRenameSubmit}
              onClose={closeRenameModal}
            />
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

          {addItemsFiles && address && (
            <AddItemsModal collection={collection} address={address} files={addItemsFiles} onClose={closeAddItems} />
          )}

          {publishView === 'wizard' && session && (
            <PublishCollectionModal
              collection={collection}
              session={session}
              resume={publishResume}
              validation={publishValidation}
              blockOnErrors={blockOnErrors}
              onClose={() => setPublishView('closed')}
              onPublished={() => {
                // With auto curation the review starts with the publication, so its modal is the confirmation.
                if (autoCuration) {
                  setPublishView('closed')
                  setReviewRequest('reviewing')
                } else setPublishView('success')
              }}
            />
          )}
          {publishView === 'success' && <PublishSuccessModal onDone={() => setPublishView('closed')} />}
          {pushView === 'gate' && (
            <ValidationGate
              collectionId={collection.id}
              flow="push_changes"
              validation={publishValidation}
              blockOnErrors={blockOnErrors}
              onPass={() => {
                // A curator who asked for fixes sees them next: the request skips the automated review.
                if (stage === ReviewStage.REJECTED_BY_CURATOR) {
                  closePush()
                  setReviewRequest('appeal')
                } else setPushView('confirm')
              }}
              onClose={closePush}
            />
          )}
          {pushView === 'confirm' && (
            <ConfirmModal
              icon={<SyncIcon />}
              title={t(`collection_detail_page.${pushCopy}.title`)}
              description={t(`collection_detail_page.${pushCopy}.description`)}
              error={pushCuration.isError ? t(`collection_detail_page.${pushCopy}.error`) : null}
              busy={pushCuration.isPending}
              onClose={closePush}
              cancel={{
                label: t(`collection_detail_page.${pushCopy}.cancel`),
                onClick: closePush,
                testId: 'push-changes-cancel'
              }}
              confirm={{
                label: t(`collection_detail_page.${pushCopy}.confirm`),
                onClick: () =>
                  pushCuration.mutate(
                    { collection, note: (withPushNote && pushNote.trim()) || undefined },
                    {
                      onSuccess: () => {
                        closePush()
                        showToast(t(`collection_detail_page.${pushCopy}.success`))
                      },
                      // The automated review answers for itself: the day's reviews are spent, or one is running.
                      onError: error => {
                        if (error instanceof ValidationLimitError) {
                          closePush()
                          setReviewRequest('limit')
                        } else if (isValidationRunning(error)) {
                          closePush()
                          showToast(t('review_flow.running'), { type: 'warn' })
                        } else if (isCollectionBusy(error)) {
                          closePush()
                          showToast(t('review_flow.busy'), { type: 'warn' })
                        }
                      }
                    }
                  ),
                testId: 'push-changes-confirm'
              }}
              testId="push-changes-modal"
            >
              {withPushNote && (
                <NoteField
                  label={t('collection_detail_page.push_note.label')}
                  value={pushNote}
                  onChange={setPushNote}
                  maxLength={NOTE_MAX_LENGTH}
                  placeholder={t('collection_detail_page.push_note.placeholder')}
                  disabled={pushCuration.isPending}
                  testId="push-changes-note"
                />
              )}
            </ConfirmModal>
          )}
          {isSending && session && (
            <SendItemsFlow
              collection={collection}
              items={allItems ?? []}
              session={session}
              onClose={() => setSending(false)}
            />
          )}
          {managingRoles && session && (
            <ManageRolesFlow
              collection={collection}
              kind={managingRoles}
              session={session}
              onClose={() => setManagingRoles(null)}
            />
          )}
          {isTransferring && session && (
            <TransferOwnershipFlow
              collection={collection}
              session={session}
              onDone={() => navigate('/create/collections', { replace: true })}
              onClose={() => setTransferring(false)}
            />
          )}
          {validationItem && (
            <ValidationResultsModal
              subject={toValidationSubject(validationItem)}
              issues={rowValidation(validationItem)?.issues ?? []}
              onRerun={() =>
                rerunItemValidation(
                  validationItem,
                  'details',
                  validation.results.get(validationItem.id)?.status ?? 'idle'
                )
              }
              onClose={() => setValidationItem(null)}
              testId="item-validation"
            />
          )}
          {thumbnailEditor.modal}
          {priceEdit && session && (
            <UpdatePriceFlow
              item={priceEdit.item}
              collection={collection}
              listing={priceEdit.listing}
              session={session}
              onClose={() => setPriceEdit(null)}
            />
          )}
          {discountOpen && session && sale && (
            <DiscountFlow
              collection={collection}
              session={session}
              sale={sale}
              onClose={() => setDiscountOpen(false)}
            />
          )}
          {endingDiscount && session && (
            <EndDiscountFlow
              discount={endingDiscount}
              collection={collection}
              session={session}
              onClose={() => setEndingDiscount(null)}
            />
          )}
          {sellingItem && session && (
            <SellItemFlow
              item={sellingItem}
              collection={collection}
              session={session}
              hasPendingChanges={hasPendingChanges(syncs.get(sellingItem.id)?.status)}
              onClose={() => setSellingItem(null)}
            />
          )}
        </>
      )}
    </div>
  )
}

export { CollectionDetailPage }
