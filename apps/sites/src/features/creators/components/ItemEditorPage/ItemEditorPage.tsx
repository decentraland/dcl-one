import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { useNavigate, useSearchParams, Link } from 'react-router'
import { useQueryClient } from '@tanstack/react-query'
import { BodyShape, type ContractNetwork } from '@dcl/schemas'
import { ArrowBackIosNewIcon, PersonOutlineIcon } from '../Icons'
import { Panel, PanelGroup, PanelResizeHandle } from 'react-resizable-panels'
import { AvatarPreview } from '../AvatarPreview'
import { Button } from '../Button'
import { AddItemsModal } from '../CollectionDetailPage/AddItemsModal'
import { CollectionNameModal } from '../CollectionNameModal'
import { CollectionStatusPill } from '../CollectionStatusPill'
import { CurationStatePill } from '../CurationStatePill'
import { ConfirmModal } from '../ConfirmModal'
import { ZoomControls } from '../ZoomControls'
import { useBaseWearables } from '../../hooks/useBaseWearables'
import { useBeforeUnloadGuard } from '../../hooks/useBeforeUnloadGuard'
import {
  allCollectionItemsKey,
  useAllCollectionItems,
  useCollection,
  useCollectionStatus,
  useSaveCollection
} from '../../hooks/useCollection'
import { useCommittee } from '../../hooks/useCommittee'
import { useCollectionCuration } from '../../hooks/useCuration'
import { useFeatureFlag } from '../../hooks/useFeatureFlag'
import { FeatureFlag } from '@/data/lib/creators/featureFlags'
import { useMediaQuery } from '../../hooks/useMediaQuery'
import { track } from '@/data/lib/creators/analytics'
import { modelValidationKey, useModelValidation } from '../../hooks/useModelValidation'
import { useRerunValidation } from '../../hooks/useRerunValidation'
import { useRepairedItemMetrics } from '../../hooks/useRepairedItemMetrics'
import { thumbnailValidationKey, useThumbnailValidation } from '../../hooks/useThumbnailValidation'
import { useCatalystItemSizes } from '../../hooks/useItemSize'
import { SaveInProgressError, useExclusiveSave } from '../../hooks/useExclusiveSave'
import { fetchStoredThumbnail, useSaveItem, type SaveItemVariables } from '../../hooks/useSaveItem'
import { captureError } from '@/data/lib/creators/monitoring'
import { springBonesFetcher } from '../../hooks/useSpringBones'
import { useTranslation } from '@/data/lib/creators/intl'
import type { AvatarAttributes } from '@/data/lib/creators/avatar'
import { BuilderServerError, COLLECTION_LOCKED_STATUS, getContentsStorageUrl } from '@/data/lib/creators/builder'
import {
  canManageCollectionItems,
  canViewCollection,
  hasCollectionRole,
  isCollectionLocked,
  isLinkedCollection,
  type Collection
} from '@/data/lib/creators/collections'
import { parseUuidParam } from '@/data/lib/creators/ids'
import { ReviewStage, getCurationState, getReviewStage, curationListUrl } from '@/data/lib/creators/curation'
import { useCollectionEvents } from '../../hooks/useCollectionEvents'
import { getAutomatedIssuesByItem } from '@/data/lib/creators/events'
import { toFileChangeSaveable, toPreviewItem, toSaveableItem, type ItemFileChange } from '@/data/lib/creators/itemDraft'
import { getEditorMode, pickDressedItems, resolveSelectedItem } from '@/data/lib/creators/itemEditor'
import { pickFiles } from '@/data/lib/creators/filePicker'
import { IMAGE_PATH, ITEM_EXTENSIONS } from '@/data/lib/creators/itemFiles'
import { getItemSizeLimit, isItemTooBigError, type ItemSize } from '@/data/lib/creators/itemSize'
import { ItemType, canDeleteItem, canEditItemDetails, type Item } from '@/data/lib/creators/items'
import { isLinkedItemPublished } from '@/data/lib/creators/linkedItemStatus'
import {
  LINKED_COLLECTION_NAME_MAX_LENGTH,
  canViewLinkedCollection,
  isThirdPartyManager,
  type LinkedContract
} from '@/data/lib/creators/thirdParty'
import { useItemCurations, useThirdParty } from '../../hooks/useLinkedCollection'
import { usePendingLinkedItems } from '../../hooks/useLinkedReview'
import { useNotifications } from '@/data/lib/creators/notifications'
import type { AvatarPreviewSource } from '@/data/lib/creators/preview'
import {
  carrySpringBonesForSave,
  getShapesMissingSpringBones,
  mergeSpringBonesIntoItem,
  type SpringBoneParamsByName
} from '@/data/lib/creators/springBones'
import { selectAvatarAttributes, useAvatarPreview } from '../../store/avatarPreview'
import { useWallet } from '../../store/wallet'
import { media } from '@/data/lib/creators/theme'
import { AvatarCustomizerDrawer, AvatarCustomizerToggle } from './AvatarCustomizer'
import { CollectionPicker } from './CollectionPicker'
import { ItemsSidebar, type EditorBack } from './ItemsSidebar'
import { MobilePreview } from './MobilePreview'
import { SceneBoundariesToggle } from './SceneBoundariesToggle'
import { PlaybackBar } from './PlaybackBar'
import { PropertiesPanel, type SpringBonesFormProps } from './PropertiesPanel'
import { EditorPreviewLaunch } from './EditorPreviewLaunch'
import { ReviewBar } from './ReviewBar'
import { LinkedReviewBar } from './ReviewBar/LinkedReviewBar'
import { ValidationBadge } from '../ValidationBadge'
import { getValidationStatus, itemValidationContext, type ValidationSource } from '@/data/lib/creators/validation'
import { useEditorLayout } from './useEditorLayout'
import { useItemForm } from './useItemForm'
import styles from './ItemEditorPage.module.css'
import type { WearablePreviewController } from '../WearablePreview'

const PREVIEW_ID = 'item-editor-preview'
const NOT_FOUND_STATUSES = [401, 403, 404]
const EMPTY_ITEMS: Item[] = []
const SPRING_BONES_PUSH_DELAY_MS = 500

// Panel sizes as react-resizable-panels wants them, percentages of the group. The sidebar sits
// outside the group with a fixed, animated width.
const CENTER_MIN_PCT = 25
const RIGHT_DEFAULT_PCT = 26
const RIGHT_MIN_PCT = 20

const isBodyShape = (value: string): value is BodyShape => BodyShape.validate(value)

/** A navigation held back by unsaved changes: another item, or out of the editor entirely. */
type PendingNavigation = { kind: 'item'; item: Item } | { kind: 'away'; to: string }

const ItemEditorPage = () => {
  const { t } = useTranslation()
  const navigate = useNavigate()
  const queryClient = useQueryClient()
  const [searchParams, setSearchParams] = useSearchParams()
  const collectionParam = searchParams.get('collection')
  const collectionId = parseUuidParam(collectionParam) ?? null
  const itemParam = parseUuidParam(searchParams.get('item')) ?? null
  const { session, restored, signIn } = useWallet()
  const address = session?.address
  const committee = useCommittee(address)
  const isCurator = committee.isCurator
  const mode = useMemo(() => getEditorMode(searchParams, isCurator), [searchParams, isCurator])
  // Until the committee answers, a review link can't tell a curator from someone with no access.
  const isResolvingReviewer = searchParams.get('reviewing') === 'true' && !!address && committee.isLoading
  const showToast = useNotifications(state => state.showToast)
  const isMobile = useMediaQuery(media.maxWidth('mobile'))

  const collectionQuery = useCollection(address, collectionId ?? undefined)
  // Linked collections belong to a third party: its managers, not the collection's roles, may edit them.
  const isLinked = !!collectionQuery.data && isLinkedCollection(collectionQuery.data)
  const thirdParty = useThirdParty(address, isLinked ? collectionQuery.data : undefined)
  const isManager = isLinked && !!thirdParty.data && isThirdPartyManager(thirdParty.data, address)
  // builder-server serves item curations to managers only; readers never delete, which is all they would decide.
  const linkedCurations = useItemCurations(address, isManager ? collectionQuery.data : undefined)
  const linkedContract = useMemo<LinkedContract | null>(() => {
    const target = collectionQuery.data
    return target?.linkedContractNetwork && target.linkedContractAddress
      ? { network: target.linkedContractNetwork as ContractNetwork, address: target.linkedContractAddress }
      : null
  }, [collectionQuery.data])
  // builder-server serves a collection's curation to its owner, collaborators and the committee only.
  const canReadCuration =
    !!collectionQuery.data &&
    !isLinked &&
    (canManageCollectionItems(collectionQuery.data, address) || committee.isCurator)
  const collectionStatus = useCollectionStatus(canReadCuration ? address : undefined, collectionQuery.data)
  const itemsQuery = useAllCollectionItems(address, collectionId ?? undefined)
  const baseWearables = useBaseWearables()
  const saveItem = useSaveItem(address)
  const saveItemAsync = saveItem.mutateAsync
  const { isSaving, run: runSave } = useExclusiveSave()
  const saveCollection = useSaveCollection(address)
  const [isRenameOpen, setRenameOpen] = useState(false)

  const collection = collectionQuery.data
  // Linked collections have no owner: their third party's managers are who works on them.
  const hasRole = !!collection && (isLinked ? isManager : hasCollectionRole(collection, address))
  // The automated review locks the items it judges and dresses them with its findings.
  const autoCuration = useFeatureFlag(FeatureFlag.AUTO_CURATION_REVIEW_FLOW).enabled
  const reviewCuration = useCollectionCuration(
    mode === 'review' || (canReadCuration && autoCuration) ? address : undefined,
    collection
  )
  const { events: reviewEvents } = useCollectionEvents(canReadCuration ? address : undefined, collection)
  const reviewStage = useMemo(
    () => (collection ? getReviewStage(collection, reviewCuration.data ?? null, reviewEvents) : null),
    [collection, reviewCuration.data, reviewEvents]
  )
  const reviewLocked = reviewStage === ReviewStage.AUTOMATED_REVIEWING
  const curatorRequest = useMemo(() => {
    const request = reviewCuration.data
    if (mode === 'review' || reviewStage !== ReviewStage.REJECTED_BY_CURATOR || !request) return undefined
    return { reasons: request.rejectionReasons ?? [], message: request.rejectionMessage }
  }, [mode, reviewStage, reviewCuration.data])
  const automatedIssues = useMemo(
    () => getAutomatedIssuesByItem(reviewEvents ?? [], itemsQuery.data ?? EMPTY_ITEMS),
    [reviewEvents, itemsQuery.data]
  )
  const items = itemsQuery.data ?? EMPTY_ITEMS
  // A linked collection under review: the sidebar sets apart the items this review is about.
  const pendingLinked = usePendingLinkedItems(address, mode === 'review' && isLinked ? collection : undefined)
  const [reviewedIds, setReviewedIds] = useState<ReadonlySet<string>>(() => new Set())
  const sidebarReview = useMemo(
    () =>
      pendingLinked.data ? { pendingIds: new Set(pendingLinked.data.map(item => item.id)), reviewedIds } : undefined,
    [pendingLinked.data, reviewedIds]
  )
  const resolved = useMemo(() => resolveSelectedItem(items, itemParam), [items, itemParam])
  const repairedMetrics = useRepairedItemMetrics(resolved)
  const selected = useMemo(
    () => (resolved && repairedMetrics ? { ...resolved, metrics: repairedMetrics } : resolved),
    [resolved, repairedMetrics]
  )

  // Avatar state (session-only store).
  const bodyShape = useAvatarPreview(state => state.bodyShape)
  const skin = useAvatarPreview(state => state.skin)
  const eyes = useAvatarPreview(state => state.eyes)
  const hair = useAvatarPreview(state => state.hair)
  const baseSelection = useAvatarPreview(state => state.baseWearables)
  const dressedItemIds = useAvatarPreview(state => state.dressedItemIds)
  const emote = useAvatarPreview(state => state.emote)
  const isPlaying = useAvatarPreview(state => state.isPlaying)
  const dress = useAvatarPreview(state => state.dress)
  const toggleDressed = useAvatarPreview(state => state.toggleDressed)
  const clearDressed = useAvatarPreview(state => state.clearDressed)
  const setBodyShape = useAvatarPreview(state => state.setBodyShape)
  const seedBaseWearables = useAvatarPreview(state => state.seedBaseWearables)
  // The store's own mapping, memoized on the fields it reads: subscribing to the selector directly
  // would hand the bridge a new object on every store change and rebuild the preview.
  const avatar = useMemo<AvatarAttributes>(
    () => selectAvatarAttributes({ bodyShape, skin, eyes, hair, baseWearables: baseSelection }),
    [bodyShape, skin, eyes, hair, baseSelection]
  )

  useEffect(() => clearDressed(), [collectionId, clearDressed])
  // The mannequin's outfit is picked the moment the catalog arrives, whether or not the customizer is open.
  useEffect(() => {
    if (baseWearables.data) seedBaseWearables(baseWearables.data)
  }, [baseWearables.data, seedBaseWearables])
  const selectedId = selected?.id ?? null
  // Read inside async work (a save in flight) where the closed-over selection may already be stale.
  const selectedIdRef = useRef(selectedId)
  selectedIdRef.current = selectedId
  const selectedType = selected?.type ?? null
  const selectedCategory = selected?.data.category
  useEffect(() => {
    if (selectedId && selectedType) dress({ id: selectedId, type: selectedType, category: selectedCategory })
  }, [selectedId, selectedType, selectedCategory, dress])

  // Form state.
  const form = useItemForm(selected)
  const { springBones } = form
  const [activeSpringHash, setActiveSpringHash] = useState<string | null>(null)
  useBeforeUnloadGuard(form.isDirty)

  const previewSelected = useMemo(() => (selected ? toPreviewItem(selected, form.draft) : null), [selected, form.draft])
  const previewItems = useMemo(() => {
    // A wearable whose new file dropped its category has no slot to render in until one is picked.
    const unplaceable = previewSelected?.type === ItemType.WEARABLE && form.draft.category === null
    const withDraft = previewSelected
      ? items.flatMap(item => {
          if (item.id !== previewSelected.id) return [item]
          return unplaceable ? [] : [previewSelected]
        })
      : items
    return pickDressedItems(withDraft, dressedItemIds, bodyShape)
  }, [items, previewSelected, form.draft.category, dressedItemIds, bodyShape])
  const source = useMemo<AvatarPreviewSource>(() => ({ kind: 'items', items: previewItems }), [previewItems])
  const collectionEmotes = useMemo(() => items.filter(item => item.type === ItemType.EMOTE), [items])
  const subjectEmote = useMemo(() => previewItems.find(item => item.type === ItemType.EMOTE) ?? null, [previewItems])
  const previewedWearables = useMemo(() => previewItems.filter(item => item.type === ItemType.WEARABLE), [previewItems])

  // Phones validate nothing.
  const validationSource = useMemo<ValidationSource | null>(
    () => (previewSelected && !isMobile ? { kind: 'item', item: previewSelected } : null),
    [previewSelected, isMobile]
  )
  const validationCtx = useMemo(
    () => (previewSelected ? itemValidationContext(previewSelected) : { type: ItemType.WEARABLE }),
    [previewSelected]
  )
  const validation = useModelValidation(validationSource, validationCtx)
  const thumbnailSource = useMemo(() => {
    if (isMobile) return null
    return selected?.contents[selected.thumbnail] ? ({ kind: 'item', item: selected } as const) : null
  }, [selected, isMobile])
  const thumbnailValidation = useThumbnailValidation(thumbnailSource)
  const validationSubject = useMemo(() => {
    if (!previewSelected) return undefined
    const storedThumbnail = previewSelected.contents[previewSelected.thumbnail]
    return {
      name: previewSelected.name,
      type: previewSelected.type,
      category: previewSelected.data.category,
      rarity: previewSelected.rarity,
      thumbnail: storedThumbnail ? getContentsStorageUrl(storedThumbnail) : null
    }
  }, [previewSelected])
  const validationIssues = useMemo(() => {
    const automated = selectedId ? automatedIssues.get(selectedId) : undefined
    if (!validation.data && !thumbnailValidation.data && !automated) return undefined
    return [...(validation.data?.issues ?? []), ...(thumbnailValidation.data?.issues ?? []), ...(automated ?? [])]
  }, [validation.data, thumbnailValidation.data, automatedIssues, selectedId])
  const validationStatus = useMemo(
    () => getValidationStatus(validationIssues, validation.isLoading || thumbnailValidation.isLoading),
    [validationIssues, validation.isLoading, thumbnailValidation.isLoading]
  )
  // The stored files' size, against the limit of the category being edited (skins get 9 MB). A new thumbnail or
  // rarity image is measured exactly when it is saved.
  const sizeItems = useMemo(() => (selected ? [selected] : []), [selected])
  const storedSizes = useCatalystItemSizes(sizeItems)
  const storedBytes = selected ? storedSizes.sizes.get(selected.id)?.bytes : undefined
  const sizeCategory = form.draft.category ?? selected?.data.category
  const itemSize = useMemo<ItemSize | null>(
    () =>
      selected && storedBytes !== undefined
        ? { bytes: storedBytes, limit: getItemSizeLimit(selected.type, sizeCategory) }
        : null,
    [selected, storedBytes, sizeCategory]
  )
  const rerunValidation = useRerunValidation()
  const rerunSelectedValidation = useCallback(async () => {
    const keys: (readonly unknown[])[] = []
    if (validationSource) keys.push(modelValidationKey(validationSource, validationCtx))
    if (thumbnailSource) keys.push(thumbnailValidationKey(thumbnailSource))
    const issues = await rerunValidation(keys)
    track('bd_item_validation_rerun', {
      item_id: selectedId,
      trigger: 'details',
      previous_status: validationStatus,
      new_status: getValidationStatus(issues, false)
    })
  }, [rerunValidation, validationSource, validationCtx, thumbnailSource, selectedId, validationStatus])
  const onValidationOpen = useCallback(
    () =>
      track('bd_item_validation_details_opened', { item_id: selectedId, status: validationStatus, trigger: 'editor' }),
    [selectedId, validationStatus]
  )

  // Preview controller: spring bones are pushed on edits (debounced), on every load and on play.
  const [controller, setController] = useState<WearablePreviewController | null>(null)
  const [loadCount, setLoadCount] = useState(0)
  const onPreviewLoad = useCallback(() => setLoadCount(count => count + 1), [])
  // Reported for diagnostics only: the iframe shows its own error state.
  const onPreviewError = useCallback((error: Error) => console.error('Avatar preview failed:', error), [])
  const currentSpringHash = useMemo(() => {
    const byShape = springBones.models.find(model => model.bodyShapes.includes(bodyShape))
    return byShape?.hash ?? springBones.models[0]?.hash ?? null
  }, [springBones.models, bodyShape])
  const springParamsForPreview = currentSpringHash ? form.springBoneParams[currentSpringHash] : undefined
  const pushSpringBones = useCallback(() => {
    if (!controller || !selectedId || !springParamsForPreview) return
    controller.physics.setSpringBonesParams(selectedId, springParamsForPreview).catch(() => undefined)
  }, [controller, selectedId, springParamsForPreview])
  const pushSpringBonesRef = useRef(pushSpringBones)
  pushSpringBonesRef.current = pushSpringBones
  useEffect(() => {
    const timer = window.setTimeout(pushSpringBones, SPRING_BONES_PUSH_DELAY_MS)
    return () => window.clearTimeout(timer)
  }, [pushSpringBones])
  // A reloaded scene and a restarted emote both come up with no chains, so they get the params
  // again. Through the ref: this fires on the load/play edge only, with whatever is current then.
  useEffect(() => {
    if (loadCount > 0 || isPlaying) pushSpringBonesRef.current()
  }, [loadCount, isPlaying])

  // Selection: the URL is the only source of truth; a dirty draft asks before switching.
  const [pending, setPending] = useState<PendingNavigation | null>(null)
  const navigateToItem = useCallback(
    (item: Item | null) => {
      setSearchParams(prev => {
        const params = new URLSearchParams(prev)
        if (item) params.set('item', item.id)
        else params.delete('item')
        return params
      })
    },
    [setSearchParams]
  )
  const selectItem = useCallback(
    (item: Item) => {
      if (item.id === selectedId) return
      if (form.isDirty) setPending({ kind: 'item', item })
      else navigateToItem(item)
    },
    [selectedId, form.isDirty, navigateToItem]
  )
  const toggleEmotePlay = useCallback(() => {
    if (!controller) return
    void (isPlaying ? controller.emote.pause() : controller.emote.play()).catch(() => undefined)
  }, [controller, isPlaying])

  // Add items: the modal's new item becomes the selection once the list refetches.
  const [addItemsFiles, setAddItemsFiles] = useState<File[] | null>(null)
  const knownIdsRef = useRef<Set<string> | null>(null)
  const [awaitingNewItem, setAwaitingNewItem] = useState(false)
  async function addItems() {
    const files = await pickFiles({ accept: ITEM_EXTENSIONS.join(','), multiple: true })
    if (files.length === 0) return
    // Snapshot the list before the upload so the item the modal creates can be told apart afterwards.
    knownIdsRef.current = new Set(items.map(item => item.id))
    setAddItemsFiles(files)
  }
  const closeAddItems = useCallback(() => {
    setAddItemsFiles(null)
    setAwaitingNewItem(true)
  }, [])
  useEffect(() => {
    if (!awaitingNewItem || !knownIdsRef.current || itemsQuery.isFetching) return
    const known = knownIdsRef.current
    // Several files can be uploaded at once: the last one created is the one to open.
    const added = items
      .filter(item => !known.has(item.id))
      .reduce<Item | null>((latest, item) => (!latest || item.createdAt > latest.createdAt ? item : latest), null)
    // The refetch that follows the upload may not have landed yet; keep waiting for it.
    if (!added) return
    setAwaitingNewItem(false)
    knownIdsRef.current = null
    navigateToItem(added)
  }, [awaitingNewItem, items, itemsQuery.isFetching, navigateToItem])

  // Save / revert.
  const [springWarning, setSpringWarning] = useState(false)
  const isDraftCollection = !!collection && !collection.isPublished && !isCollectionLocked(collection)
  // A linked collection keeps taking items after publishing; only its name and URN freeze.
  const canGrow = isLinked ? isManager && !!collection && !isCollectionLocked(collection) : isDraftCollection
  const canRename = isLinked ? canGrow && !collection?.isPublished : isDraftCollection
  const editable =
    mode === 'edit' &&
    !!collection &&
    !!selected &&
    !!address &&
    !reviewLocked &&
    (isLinked ? isManager && !isCollectionLocked(collection) : canEditItemDetails(collection, selected, address))
  const canDelete =
    editable &&
    !!collection &&
    (isLinked
      ? linkedCurations.isSuccess && !isLinkedItemPublished(selected, linkedCurations.data.get(selected.id) ?? null)
      : canDeleteItem(collection, address))

  // Metrics are part of the Catalyst entity hash, so nothing published is rewritten behind its owner's back: there
  // the corrected numbers are shown and ride along with the owner's next save.
  const repairAttemptedRef = useRef(new Set<string>())
  const isLinkedItemDraft =
    !isLinked ||
    (!!selected &&
      linkedCurations.isSuccess &&
      !isLinkedItemPublished(selected, linkedCurations.data.get(selected.id) ?? null))
  // A linked item's status is its content hash, so on one sent for review the corrected metrics would ask for a push.
  const saveBase = isLinkedItemDraft ? selected : resolved
  const isResolvingRepairGate =
    isResolvingReviewer || isSaving || (isLinked && (thirdParty.isLoading || linkedCurations.isLoading))
  useEffect(() => {
    if (!selected || !repairedMetrics || repairAttemptedRef.current.has(selected.id)) return
    if (!collection || isResolvingRepairGate) return
    repairAttemptedRef.current.add(selected.id)
    const itemId = selected.id
    if (!(editable && isDraftCollection && !selected.isPublished && isLinkedItemDraft && !form.isDirty)) {
      track('bd_item_metrics_repaired', { item_id: itemId, saved: false })
      return
    }
    // A failed save is reported by the query client's mutation handler.
    runSave(() => saveItemAsync({ item: selected, blobs: {}, background: true })).then(
      saved => {
        queryClient.setQueryData<Item[]>(allCollectionItemsKey(address, collectionId ?? undefined), current =>
          current?.map(candidate => (candidate.id === saved.id ? saved : candidate))
        )
        track('bd_item_metrics_repaired', { item_id: itemId, saved: true })
      },
      () => track('bd_item_metrics_repaired', { item_id: itemId, saved: false })
    )
  }, [
    selected,
    repairedMetrics,
    collection,
    isResolvingRepairGate,
    editable,
    isDraftCollection,
    isLinkedItemDraft,
    form.isDirty,
    runSave,
    saveItemAsync,
    queryClient,
    address,
    collectionId
  ])

  async function persist(item: Item) {
    const built = await toSaveableItem(item, form.draft, {
      springBones:
        springBones.models.length > 0 ? (mergeSpringBonesIntoItem(form.springBoneParams) ?? null) : undefined,
      fetchThumbnail: fetchStoredThumbnail
    })
    if (form.draft.rarity !== item.rarity && !built.blobs[IMAGE_PATH]) {
      showToast(t('item_editor.image_refresh_skipped'), { type: 'warn' })
    }
    const saved = await commit(item, built)
    // The selection may have moved on while the save was in flight; resetting then would wipe the
    // form of whichever item is on screen now with this item's data.
    if (selectedIdRef.current === saved.id) form.reset(saved)
  }

  /** PUTs the built item, refreshes the cached list and toasts. */
  async function commit(item: Item, save: SaveItemVariables): Promise<Item> {
    const saved = await saveItemAsync({ ...save, previous: item, previousVideo: item.video })
    queryClient.setQueryData<Item[]>(allCollectionItemsKey(address, collectionId ?? undefined), current =>
      current?.map(candidate => (candidate.id === saved.id ? saved : candidate))
    )
    showToast(t('item_editor.save_success', { name: saved.name }))
    return saved
  }

  // A locked collection is the creator's to wait out, an item over its size limit the creator's to slim down;
  // anything else is ours to look into.
  function reportSaveError(error: unknown, flow: 'item_save' | 'item_file_change') {
    if (isItemTooBigError(error)) {
      showToast(t('add_items_modal.file_error.size_exceeded', error.messageParams), { type: 'error' })
      return
    }
    console.error('Item save failed:', error)
    const locked = error instanceof BuilderServerError && error.status === COLLECTION_LOCKED_STATUS
    if (!locked) captureError(error, { flow, itemId: selectedIdRef.current ?? undefined })
    showToast(t(locked ? 'item_editor.save_error_locked' : 'item_editor.save_error'), { type: 'error' })
  }

  // The change-file modal saves on confirm, like the legacy builder's: the file change alone, built
  // from the saved item. Spring-bone params ride along as they stand in the form (chains are keyed by
  // model, so they move with the file); every other unsaved panel edit stays pending, except what the
  // file change decided (category, thumbnail).
  async function saveFileChange(change: ItemFileChange): Promise<void> {
    if (!saveBase) throw new Error('No item selected')
    const base = saveBase
    await runSave(() => applyFileChange(base, change))
  }

  async function applyFileChange(base: Item, change: ItemFileChange): Promise<void> {
    try {
      const springBonesData = await carrySpringBonesForSave(
        springBonesFetcher(queryClient),
        base,
        change.update,
        form.springBoneParams
      )
      const built = await toFileChangeSaveable(base, change, {
        springBones: springBonesData,
        fetchThumbnail: fetchStoredThumbnail
      })
      const saved = await commit(base, built)
      if (selectedIdRef.current === saved.id) {
        if (change.category) {
          form.dispatch({ type: 'setCategory', category: change.category })
          form.dispatch({ type: 'setHides', hides: saved.data.hides ?? [] })
        }
      }
    } catch (error) {
      // The modal shows an oversized result itself, next to the file that caused it.
      if (!isItemTooBigError(error)) reportSaveError(error, 'item_file_change')
      throw error
    }
  }

  // A new thumbnail saves on confirm, like a file change, so the size the panel shows is always the stored one.
  // The modal shows a failure itself and stays open.
  async function saveThumbnail(thumbnail: Blob): Promise<void> {
    if (!selected || !saveBase) return
    try {
      await runSave(() => commit(saveBase, { item: saveBase, thumbnail }))
    } catch (error) {
      const expected =
        isItemTooBigError(error) ||
        error instanceof SaveInProgressError ||
        (error instanceof BuilderServerError && error.status === COLLECTION_LOCKED_STATUS)
      if (!expected) captureError(error, { flow: 'item_save', itemId: selected.id })
      throw error
    }
  }

  function save() {
    if (!selected || !saveBase) return
    const missing = getShapesMissingSpringBones(form.springBoneParams, springBones.bonesByHash)
    if (missing.length > 0 && !springWarning) {
      setSpringWarning(true)
      return
    }
    setSpringWarning(false)
    runSave(() => persist(saveBase)).catch((error: unknown) => {
      if (!(error instanceof SaveInProgressError)) reportSaveError(error, 'item_save')
    })
  }

  // Read off `form` before the memo: `form` itself is a fresh object every render, so depending on it
  // would defeat the caching and re-render the properties panel on every keystroke.
  const { springBoneParams, setSpringBoneParams } = form
  const springBonesForm = useMemo<SpringBonesFormProps | null>(() => {
    if (!selected || springBones.models.length === 0) return null
    const activeHash =
      activeSpringHash && springBones.models.some(model => model.hash === activeSpringHash)
        ? activeSpringHash
        : (currentSpringHash ?? springBones.models[0].hash)
    return {
      models: springBones.models,
      params: springBoneParams[activeHash] ?? {},
      activeHash,
      onActiveHashChange: hash => {
        setActiveSpringHash(hash)
        // The tab pair doubles as a body-shape switch so the preview shows the edited model.
        const model = springBones.models.find(candidate => candidate.hash === hash)
        const shape = model?.bodyShapes.find(isBodyShape)
        if (shape) setBodyShape(shape)
      },
      onChange: (params: SpringBoneParamsByName) => setSpringBoneParams({ ...springBoneParams, [activeHash]: params })
    }
  }, [selected, springBones, activeSpringHash, currentSpringHash, springBoneParams, setSpringBoneParams, setBodyShape])

  const closeRename = useCallback(() => {
    setRenameOpen(false)
    saveCollection.reset()
  }, [saveCollection])

  // Layout.
  const { sidebarCollapsed, toggleSidebar, panelStorage } = useEditorLayout()
  const [isCustomizerOpen, setCustomizerOpen] = useState(false)
  const closeCustomizer = useCallback(() => setCustomizerOpen(false), [])

  // States.
  const isLoading =
    !restored ||
    isResolvingReviewer ||
    (!!address && !!collectionId && (collectionQuery.isLoading || itemsQuery.isLoading || thirdParty.isLoading)) ||
    // Whether a viewer without a role gets a back link depends on the committee, so wait for it.
    (mode === 'edit' && !!collection && !hasRole && committee.isLoading)
  const isThirdPartyNotFound =
    thirdParty.isError &&
    thirdParty.error instanceof BuilderServerError &&
    NOT_FOUND_STATUSES.includes(thirdParty.error.status)
  const isNotFound =
    (!!collectionParam && !collectionId) ||
    (!!collectionId &&
      ((collectionQuery.isError &&
        collectionQuery.error instanceof BuilderServerError &&
        NOT_FOUND_STATUSES.includes(collectionQuery.error.status)) ||
        // Drafts are private to their roles; published collections, linked ones included, open read-only here to any
        // signer, the editor being their only public view (curators review through mode === 'review', which skips
        // the gate).
        (!!collection &&
          mode === 'edit' &&
          (isLinked
            ? isThirdPartyNotFound ||
              (!!thirdParty.data && !canViewLinkedCollection(collection, thirdParty.data, address))
            : !canViewCollection(collection, address)))))
  const isError =
    !isNotFound &&
    !!collectionId &&
    (collectionQuery.isError || itemsQuery.isError || (thirdParty.isError && !isThirdPartyNotFound))

  const preview = (
    <AvatarPreview
      id={PREVIEW_ID}
      source={source}
      avatar={avatar}
      emote={emote}
      onLoad={onPreviewLoad}
      onError={onPreviewError}
      onController={setController}
    >
      <PlaybackBar
        previewId={PREVIEW_ID}
        controller={controller}
        collectionEmotes={collectionEmotes}
        previewedWearables={previewedWearables}
        subjectEmoteId={subjectEmote?.id ?? null}
      />
      <AvatarCustomizerToggle open={isCustomizerOpen} onToggle={() => setCustomizerOpen(open => !open)} />
      <SceneBoundariesToggle trigger="editor" />
      {selected && (
        <ValidationBadge
          status={validationStatus}
          issues={validationIssues ?? []}
          subject={validationSubject}
          onRerun={rerunSelectedValidation}
          onOpen={onValidationOpen}
        />
      )}
      {collection && (mode === 'review' || (mode === 'edit' && hasRole)) && (
        <EditorPreviewLaunch collectionId={collection.id} itemCount={items.length} reviewing={mode === 'review'} />
      )}
    </AvatarPreview>
  )

  if (restored && !session) {
    return (
      <div className={styles.workspace} data-testid="item-editor-page">
        <section className={styles.statePanel} data-testid="sign-in-panel">
          <PersonOutlineIcon />
          <h2 className={styles.stateTitle}>{t('collection_detail_page.sign_in.title')}</h2>
          <Button type="button" variant="primary" data-testid="sign-in" onClick={() => signIn()}>
            {t('collection_detail_page.sign_in.action')}
          </Button>
        </section>
      </div>
    )
  }

  if (isNotFound) {
    return (
      <div className={styles.workspace} data-testid="item-editor-page">
        <section className={styles.statePanel} data-testid="collection-not-found">
          <h2 className={styles.stateTitle}>{t('collection_detail_page.not_found.title')}</h2>
          <p className={styles.stateText}>{t('collection_detail_page.not_found.description')}</p>
          <Button type="button" variant="secondary" onClick={() => navigate('/create/collections')}>
            {t('collection_detail_page.not_found.back')}
          </Button>
        </section>
      </div>
    )
  }

  if (isError) {
    return (
      <div className={styles.workspace} data-testid="item-editor-page">
        <section className={styles.statePanel} data-testid="item-editor-error">
          <h2 className={styles.stateTitle}>{t('collection_detail_page.error.title')}</h2>
          <p className={styles.stateText}>{t('collection_detail_page.error.description')}</p>
          <Button
            type="button"
            variant="secondary"
            onClick={() => {
              void collectionQuery.refetch()
              void itemsQuery.refetch()
            }}
          >
            {t('collection_detail_page.error.retry')}
          </Button>
        </section>
      </div>
    )
  }

  if (isLoading || !address) {
    return (
      <div className={styles.workspace} data-testid="item-editor-page" aria-busy="true">
        <div className={styles.previewEmpty} data-testid="item-editor-loading">
          <span className="spinner" aria-hidden />
        </div>
      </div>
    )
  }

  const reviewBar =
    mode === 'review' && collection && session ? (
      isLinked ? (
        thirdParty.data && (
          <LinkedReviewBar
            key={collection.id}
            session={session}
            collection={collection}
            thirdParty={thirdParty.data}
            selectedId={selectedId}
            onSelectItem={selectItem}
            onReviewedChange={setReviewedIds}
          />
        )
      ) : (
        <ReviewBar
          key={collection.id}
          session={session}
          collection={collection}
          items={items}
          onSelectItem={selectItem}
        />
      )
    ) : null
  const properties =
    collection && selected ? (
      <PropertiesPanel
        key={selected.id}
        item={selected}
        address={address}
        editable={editable}
        showReadOnlyNote={mode !== 'review'}
        curatorRequest={curatorRequest}
        readOnlyReason={
          isCollectionLocked(collection) ? 'lock' : reviewLocked ? 'reviewing' : isLinked ? 'linked' : 'role'
        }
        canDelete={canDelete}
        canDownload={canManageCollectionItems(collection, address) || isManager || isCurator}
        draft={form.draft}
        dispatch={form.dispatch}
        isDirty={form.isDirty}
        isSaving={isSaving}
        size={itemSize}
        springBones={springBonesForm}
        linkedContract={isLinked ? linkedContract : undefined}
        onSave={save}
        onSaveFile={saveFileChange}
        onSaveThumbnail={saveThumbnail}
        onRevert={() => form.reset(selected)}
        onDeleted={() => navigateToItem(null)}
      />
    ) : null

  // The detail page is for the collection's roles; a curator with none goes back to the curation list, and anyone
  // else reached a published collection by link and has nowhere to go back to.
  const editBack: EditorBack | undefined = collection
    ? hasRole
      ? { to: `/create/collections/${collection.id}`, label: t('item_editor.sidebar.back') }
      : isCurator
        ? { to: curationListUrl(), label: t('item_editor.review.back') }
        : undefined
    : undefined
  const mobileBack: EditorBack | undefined =
    mode === 'review' ? { to: curationListUrl(), label: t('item_editor.review.back') } : editBack
  const leaveEditor = (to: string) => {
    if (!form.isDirty) return true
    setPending({ kind: 'away', to })
    return false
  }
  // Phones get no sidebar and no review bar, so this strip is the only way back out of the editor.
  const mobileHeader = collection ? (
    <div className={styles.mobileEditorHeader} data-testid="mobile-editor-header">
      {mobileBack && (
        <Link
          className={styles.mobileBackLink}
          to={mobileBack.to}
          aria-label={mobileBack.label}
          data-testid="mobile-editor-back"
          onClick={event => {
            if (mode === 'edit' && !leaveEditor(mobileBack.to)) event.preventDefault()
          }}
        >
          <ArrowBackIosNewIcon fontSize="small" />
        </Link>
      )}
      <h1 className={styles.mobileCollectionName} title={collection.name} data-testid="mobile-editor-collection">
        {collection.name}
      </h1>
      {mode === 'review'
        ? !reviewCuration.isLoading &&
          !reviewCuration.isError && (
            <CurationStatePill state={getCurationState(collection, reviewCuration.data ?? null)} />
          )
        : collectionStatus && <CollectionStatusPill collection={collection} status={collectionStatus} />}
    </div>
  ) : null

  if (isMobile) {
    return (
      <MobilePreview
        header={mobileHeader}
        readOnly={mode === 'review'}
        items={items}
        selectedId={selectedId}
        dressedIds={dressedItemIds}
        bodyShape={bodyShape}
        // The strip is the phone's only item control, so a tap does what the sidebar's row and dress
        // button do together: put the item on the avatar and edit it, or take it back off.
        onTap={item => {
          if (!dressedItemIds.includes(item.id)) navigateToItem(item)
          toggleDressed({ id: item.id, type: item.type, category: item.data.category })
        }}
      >
        {collection ? preview : <div className={styles.previewEmpty}>{t('item_editor.pick_collection')}</div>}
        {collection && isCustomizerOpen && (
          <AvatarCustomizerDrawer catalog={baseWearables.data} onClose={closeCustomizer} />
        )}
      </MobilePreview>
    )
  }

  const sidebar = collection ? (
    <ItemsSidebar
      collection={collection}
      collectionStatus={collectionStatus}
      items={items}
      isLoading={itemsQuery.isLoading}
      selectedId={selectedId}
      dressedIds={dressedItemIds}
      bodyShape={bodyShape}
      isPlaying={isPlaying}
      mode={mode}
      collapsed={sidebarCollapsed}
      onToggleCollapsed={toggleSidebar}
      review={sidebarReview}
      canAddItems={isLinked ? canGrow : isDraftCollection && canManageCollectionItems(collection, address)}
      onRename={
        (isLinked ? canRename : isDraftCollection && canManageCollectionItems(collection, address))
          ? () => setRenameOpen(true)
          : undefined
      }
      onAddItems={() => {
        addItems().catch((error: unknown) => console.error('Add items failed:', error))
      }}
      onSelect={selectItem}
      back={editBack}
      onLeave={leaveEditor}
      onToggleDressed={item => toggleDressed({ id: item.id, type: item.type, category: item.data.category })}
      onToggleEmotePlay={toggleEmotePlay}
    />
  ) : (
    <CollectionPicker address={address} onPick={(picked: Collection) => setSearchParams({ collection: picked.id })} />
  )

  return (
    <div className={styles.workspace} data-testid="item-editor-page" data-mode={mode}>
      {reviewBar}
      <div className={styles.columns}>
        {collection ? sidebar : <div className={styles.pickerColumn}>{sidebar}</div>}
        <PanelGroup
          direction="horizontal"
          autoSaveId="item-editor"
          storage={panelStorage}
          style={{ flex: 1, minWidth: 0 }}
        >
          <Panel id="editor-center" minSize={CENTER_MIN_PCT} order={2}>
            <div className={styles.centerPanel} data-testid="editor-center">
              {collection ? (
                <>
                  <div className={styles.previewArea}>
                    {preview}
                    {loadCount > 0 && controller && <ZoomControls controller={controller} />}
                  </div>
                  {isCustomizerOpen && (
                    <AvatarCustomizerDrawer catalog={baseWearables.data} onClose={closeCustomizer} />
                  )}
                </>
              ) : (
                <div className={styles.previewEmpty} data-testid="editor-center-empty">
                  {t('item_editor.pick_collection')}
                </div>
              )}
            </div>
          </Panel>
          {collection && selected && (
            <>
              <PanelResizeHandle>
                <div className={styles.handle} data-testid="resize-handle-right" />
              </PanelResizeHandle>
              <Panel id="editor-properties" defaultSize={RIGHT_DEFAULT_PCT} minSize={RIGHT_MIN_PCT} order={3}>
                {properties}
              </Panel>
            </>
          )}
        </PanelGroup>
      </div>

      {addItemsFiles && collection && (
        <AddItemsModal collection={collection} address={address} files={addItemsFiles} onClose={closeAddItems} />
      )}
      {isRenameOpen && collection && (
        <CollectionNameModal
          variant="rename"
          initialName={collection.name}
          nameMaxLength={isLinked ? LINKED_COLLECTION_NAME_MAX_LENGTH : undefined}
          isPending={saveCollection.isPending}
          error={saveCollection.error?.message ?? null}
          onSubmit={name => {
            if (name === collection.name) {
              closeRename()
              return
            }
            saveCollection.mutate({ ...collection, name }, { onSuccess: closeRename })
          }}
          onClose={closeRename}
        />
      )}
      {pending && (
        <ConfirmModal
          title={t('item_editor.discard.title')}
          description={t('item_editor.discard.description')}
          onClose={() => setPending(null)}
          cancel={{
            label: t('item_editor.discard.stay'),
            onClick: () => setPending(null),
            testId: 'discard-stay'
          }}
          confirm={{
            label: t('item_editor.discard.confirm'),
            onClick: () => {
              const next = pending
              setPending(null)
              if (next.kind === 'item') navigateToItem(next.item)
              else navigate(next.to)
            },
            testId: 'discard-confirm'
          }}
          testId="discard-changes"
        />
      )}
      {springWarning && selected && (
        <ConfirmModal
          title={t('item_editor.spring_bones.warning_title', { name: selected.name })}
          description={t('item_editor.spring_bones.warning_description')}
          onClose={() => setSpringWarning(false)}
          cancel={{
            label: t('item_editor.spring_bones.warning_back'),
            onClick: () => setSpringWarning(false),
            testId: 'spring-warning-back'
          }}
          confirm={{ label: t('item_editor.spring_bones.warning_save'), onClick: save, testId: 'spring-warning-save' }}
          testId="spring-bones-warning"
        />
      )}
    </div>
  )
}

export { ItemEditorPage }
