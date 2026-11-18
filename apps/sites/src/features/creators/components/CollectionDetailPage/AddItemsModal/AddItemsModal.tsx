import { useEffect, useMemo, useReducer, useRef, useState } from 'react'
import { useQueryClient } from '@tanstack/react-query'
import { Button } from '~/components/Button'
import { Modal } from '~/components/Modal'
import { useTranslation } from '~/intl'
import { allCollectionItemsKey, useAllCollectionItems } from '~/hooks/useCollection'
import { itemCurationsKey, linkedItemsKey } from '~/hooks/useLinkedCollection'
import { useThumbnailEditor } from '~/hooks/useThumbnailEditor'
import { useBeforeUnloadGuard } from '~/hooks/useBeforeUnloadGuard'
import { useItemSizeDeps, useMeasuredBytes } from '~/hooks/useItemSize'
import { installBackGuard } from '~/lib/backGuard'
import { ItemFileError, VIDEO_PATH } from '~/lib/itemFiles'
import { type EmotePlayMode, type ItemDraftPayload } from '~/lib/itemFactory'
import { isLinkedCollection, type Collection } from '~/lib/collections'
import { areMappingsEqual, buildItemMappings, getItemMapping, type Mappings } from '~/lib/mappings'
import { buildLinkedItemUrn, getDefaultUrnSuffix, type LinkedContract } from '~/lib/thirdParty'
import { type ContractNetwork } from '@dcl/schemas'
import { ItemType } from '~/lib/items'
import { FeatureFlag } from '~/lib/featureFlags'
import { useFeatureFlag } from '~/hooks/useFeatureFlag'
import { hidesForCategory } from '~/lib/wearableCategories'
import { getValidator } from '~/lib/validation'
import { useNotifications } from '~/lib/notifications'
import { type SpringBoneParamsByName } from '~/lib/springBones'
import { errorCode, track } from '~/lib/analytics'
import { captureError } from '~/lib/monitoring'
import { type ItemSize } from '~/lib/itemSize'
import { executeUpload, findDuplicateUrns, planDraftUpload, planUpload, type UploadDraft } from '~/lib/uploadItems'
import {
  addItemsReducer,
  createDraft,
  createInitialState,
  draftSizeInputs,
  getDraftSizeLimit,
  isDraftComplete,
  isValidationStale,
  type ItemDraft
} from './AddItemsModal.state'
import { imageThumbnailPatch } from '~/lib/draftPreview'
import { isImageThumbnailStale, processDraftFile, pickPreviewDraft, type LinkedDraftTarget } from './processDraft'
import { DraftList } from './DraftList'
import { DraftForm } from './DraftForm'
import { DraftProcessor } from '~/components/ItemDraftModal'
import { VideoModal } from '~/components/VideoModal'
import { LeaveConfirmModal } from './LeaveConfirmModal'
import { UploadErrorModal } from './UploadErrorModal'
import draftModal from '~/components/ItemDraftModal/ItemDraftModal.module.css'
import styles from './AddItemsModal.module.css'

/** Values decided before the modal opened (the Blender live preview's tuning), applied to every draft. */
export type AddItemsPrefill = {
  category?: string
  hides?: string[]
  springBoneParams?: SpringBoneParamsByName
  /** Emotes only. */
  playMode?: EmotePlayMode
}

/**
 * The live preview's tuning on top of what the file itself said: category for wearables, play mode for
 * emotes. A prefilled category is the creator's own pick, so the analysis suggestion hint is dropped.
 */
function applyPrefill(patch: Partial<ItemDraft>, prefill: AddItemsPrefill | undefined): Partial<ItemDraft> {
  if (!prefill) return patch
  if (patch.type === ItemType.WEARABLE && prefill.category) {
    const hides = patch.hides && hidesForCategory(prefill.category, patch.hides)
    return { ...patch, category: prefill.category, suggestedCategory: null, ...(hides ? { hides } : {}) }
  }
  if (patch.type === ItemType.EMOTE && prefill.playMode) return { ...patch, playMode: prefill.playMode }
  return patch
}

type Props = {
  collection: Collection
  address: string
  /** The files picked or dropped on the collection page; processing starts immediately. */
  files: File[]
  prefill?: AddItemsPrefill
  /** Linked collections only: every file must update an existing item, matched by the URN in its wearable.json. */
  updateOnly?: boolean
  onClose: () => void
}

export function AddItemsModal({ collection, address, files, prefill, onClose, updateOnly = false }: Props) {
  const { t } = useTranslation()
  const queryClient = useQueryClient()
  // The editor only shows utility behind this flag, so an import must not save one the creator can't see.
  const utilityFlag = useFeatureFlag(FeatureFlag.WEARABLE_UTILITY)
  const showToast = useNotifications(state => state.showToast)

  // Linked collections take plain wearables only, with no rarity and a token mapping per item.
  const isLinked = isLinkedCollection(collection)
  const linkedContract = useMemo<LinkedContract | null>(
    () =>
      collection.linkedContractNetwork && collection.linkedContractAddress
        ? { network: collection.linkedContractNetwork as ContractNetwork, address: collection.linkedContractAddress }
        : null,
    [collection.linkedContractNetwork, collection.linkedContractAddress]
  )
  const draftRules = useMemo(
    () => ({ linked: isLinked, requiresMapping: isLinked && linkedContract !== null }),
    [isLinked, linkedContract]
  )

  const initialDraftsRef = useRef<Array<{ draft: ItemDraft; file: File }> | null>(null)
  if (initialDraftsRef.current === null) {
    // The live preview streams a placeholder file name, not one worth suggesting as the item name.
    initialDraftsRef.current = files.map(file => ({ draft: createDraft(file, { nameFromFile: !prefill }), file }))
  }
  const [state, dispatch] = useReducer(
    addItemsReducer,
    initialDraftsRef.current.map(entry => entry.draft),
    createInitialState
  )

  const [isLeaveConfirmOpen, setLeaveConfirmOpen] = useState(false)
  const thumbnailEditor = useThumbnailEditor((patch, subject) => {
    if (subject.kind === 'files') dispatch({ type: 'draftUpdated', id: subject.id, patch })
  })
  const [isVideoOpen, setVideoOpen] = useState(false)

  // Variant targets need every unpublished wearable of the collection, not just the loaded page.
  const collectionItemsQuery = useAllCollectionItems(address, collection.id)
  const collectionItems = useMemo(() => collectionItemsQuery.data ?? [], [collectionItemsQuery.data])
  const linkedTarget = useMemo<LinkedDraftTarget | undefined>(
    () =>
      isLinked
        ? {
            collectionUrn: collection.urn,
            contract: linkedContract,
            ...(updateOnly
              ? { updating: new Map(collectionItems.flatMap(item => (item.urn ? [[item.urn, item] as const] : []))) }
              : {})
          }
        : undefined,
    [isLinked, collection.urn, linkedContract, updateOnly, collectionItems]
  )
  // Matching a zip to the item it updates needs the collection's items first. Adding never waits for them (a linked
  // collection can be large): a URN another item has is refused by builder-server on upload.
  const itemsLoaded = collectionItemsQuery.data !== undefined
  const canProcess = !updateOnly || itemsLoaded
  const canUpload = !updateOnly || itemsLoaded
  // Without them neither of the above can go on: say so and offer a retry instead of waiting forever.
  const itemsFailed = updateOnly && collectionItemsQuery.isError && !itemsLoaded

  // Load + analyze each dropped file once. The prefill is what it was when the modal opened.
  const prefillRef = useRef(prefill)
  const linkedTargetRef = useRef(linkedTarget)
  linkedTargetRef.current = linkedTarget
  const processed = useRef(false)
  useEffect(() => {
    if (!canProcess || processed.current) return
    processed.current = true
    for (const { draft, file } of initialDraftsRef.current ?? []) {
      void processDraftFile(file, prefillRef.current?.hides, linkedTargetRef.current)
        .then(patch =>
          dispatch({
            type: 'draftAnalyzed',
            id: draft.id,
            patch: applyPrefill(patch, prefillRef.current)
          })
        )
        .catch((error: unknown) => {
          if (error instanceof ItemFileError) {
            dispatch({
              type: 'draftFailed',
              id: draft.id,
              errorKey: error.messageKey,
              errorParams: error.messageParams
            })
          } else {
            console.error('Draft processing failed:', error)
            dispatch({ type: 'draftFailed', id: draft.id, errorKey: 'invalid_model_file' })
          }
        })
    }
  }, [canProcess])

  const { drafts, selectedId, view, failureReason, isUploading } = state
  const selected = useMemo(
    () => drafts.find(draft => draft.id === selectedId) ?? drafts[0] ?? null,
    [drafts, selectedId]
  )
  const processingIdRef = useRef<string | null>(null)
  const previewDraft = useMemo(
    () => pickPreviewDraft(drafts, selected?.id ?? null, processingIdRef.current),
    [drafts, selected?.id]
  )
  const previewDraftId = previewDraft?.id ?? null
  useEffect(() => {
    // Kept once the queue drains, so the next file finishes the type the iframe is mounted for.
    if (previewDraftId) processingIdRef.current = previewDraftId
  }, [previewDraftId])

  // Image wearables are padded per category, so a category change re-pads our auto thumbnail.
  const repaddingIdsRef = useRef(new Set<string>())
  useEffect(() => {
    const stale = drafts.find(draft => isImageThumbnailStale(draft) && !repaddingIdsRef.current.has(draft.id))
    if (!stale) return
    repaddingIdsRef.current.add(stale.id)
    void imageThumbnailPatch(stale.contents, stale.model, stale.category)
      .then(patch =>
        dispatch({ type: 'draftAnalyzed', id: stale.id, patch: { ...patch, autoThumbnailCategory: stale.category } })
      )
      .catch((error: unknown) => console.error('Thumbnail re-padding failed:', error))
      .finally(() => repaddingIdsRef.current.delete(stale.id))
  }, [drafts])

  // Category-dependent limits re-run the validator on every category pick. A pick made mid-run leaves
  // the draft stale for the category it just got, so the next pass validates it again.
  const validatingIdsRef = useRef(new Set<string>())
  useEffect(() => {
    const stale = drafts.find(draft => isValidationStale(draft) && !validatingIdsRef.current.has(draft.id))
    if (!stale) return
    validatingIdsRef.current.add(stale.id)
    void getValidator()
      .validate(
        { kind: 'blob', contents: stale.contents, mainFile: stale.model },
        { type: ItemType.WEARABLE, category: stale.category ?? undefined, hides: stale.hides }
      )
      .then(({ issues }) => issues)
      // Advisory: a failure clears the previous category's issues and marks the draft done, so edits don't retry it.
      .catch((error: unknown) => {
        captureError(error, { flow: 'add_items', step: 'revalidate' })
        return []
      })
      .then(issues =>
        dispatch({
          type: 'draftAnalyzed',
          id: stale.id,
          patch: { validationIssues: issues, validatedCategory: stale.category }
        })
      )
      .finally(() => validatingIdsRef.current.delete(stale.id))
  }, [drafts])

  // The item the selected draft will be saved as, built exactly as the upload builds it (thumbnail quantized, rarity
  // image rendered, a variant merged into its target) and measured the catalyst way (lib/itemSize).
  const sizeDeps = useItemSizeDeps()
  const sizeInputs = useMemo(
    () => [selected?.id ?? null, collectionItems, ...draftSizeInputs(drafts)],
    [selected?.id, collectionItems, drafts]
  )
  const measuredBytes = useMeasuredBytes(
    selected?.status === 'ready' && selected.type
      ? async () => (await planDraftUpload(selected.id, drafts.map(toPayload), collectionItems))?.built ?? null
      : null,
    sizeInputs,
    'add_items'
  )
  const selectedSize = useMemo<ItemSize | null>(
    () =>
      selected && measuredBytes !== null
        ? { bytes: measuredBytes, limit: getDraftSizeLimit(selected, drafts, collectionItems) }
        : null,
    [selected, measuredBytes, drafts, collectionItems]
  )

  const isSelectedComplete = useMemo(
    () => !!selected && isDraftComplete(selected, drafts, collectionItems, draftRules, selectedSize),
    [selected, drafts, collectionItems, draftRules, selectedSize]
  )
  const othersChecked = useMemo(
    () => !!selected && drafts.every(draft => draft.id === selected.id || draft.checked),
    [selected, drafts]
  )
  const selectedIndex = useMemo(
    () => (selected ? drafts.findIndex(draft => draft.id === selected.id) : -1),
    [selected, drafts]
  )
  const hasCheckedDrafts = useMemo(() => drafts.some(draft => draft.checked), [drafts])

  // Everything deleted → nothing left to review, close silently.
  useEffect(() => {
    if (drafts.length === 0 && !isUploading) onClose()
  }, [drafts.length, isUploading, onClose])

  useBeforeUnloadGuard(drafts.length > 0)

  // Browser back would silently unmount the modal; the back guard absorbs it and shows the same
  // leave confirmation instead.
  useEffect(() => installBackGuard(() => setLeaveConfirmOpen(true)), [])

  function toItemMappings(draft: ItemDraft): Mappings | undefined {
    if (!linkedContract || !draft.mapping) return undefined
    const mappings = buildItemMappings(draft.mapping, linkedContract)
    const kept = draft.keptMappings ? getItemMapping(draft.keptMappings, linkedContract) : null
    // Untouched in the form: the item keeps every rule it had, not just the one the form can show.
    if (draft.keptMappings && kept && areMappingsEqual(mappings, buildItemMappings(kept, linkedContract))) {
      return draft.keptMappings
    }
    return mappings
  }

  function toPayload(draft: ItemDraft): UploadDraft {
    return {
      id: draft.id,
      name: draft.name.trim(),
      type: draft.type ?? ItemType.WEARABLE,
      bodyShape: draft.bodyShape,
      category: draft.category ?? '',
      // Linked items are never minted: the legacy builder fixes their rarity to unique.
      rarity: isLinked ? 'unique' : draft.rarity,
      playMode: draft.playMode,
      requiredPermissions: draft.isSmart ? draft.requiredPermissions : undefined,
      description: draft.description,
      utility: utilityFlag.enabled ? draft.utility : undefined,
      tags: draft.tags,
      blockVrmExport: draft.blockVrmExport,
      hides: draft.type === ItemType.WEARABLE ? draft.hides : undefined,
      springBoneParams: draft.type === ItemType.WEARABLE ? prefill?.springBoneParams : undefined,
      contents: draft.contents,
      model: draft.model,
      metrics: draft.metrics ?? {},
      owner: address,
      collectionId: collection.id,
      ...(isLinked
        ? {
            urn: buildLinkedItemUrn(collection.urn, draft.urnSuffix ?? getDefaultUrnSuffix(draft.name.trim())),
            mappings: toItemMappings(draft)
          }
        : {}),
      variantTargetId: draft.isVariant && draft.variantTargetId ? draft.variantTargetId : undefined
    } satisfies ItemDraftPayload & { variantTargetId?: string }
  }

  async function upload(draftsToUpload: ItemDraft[]) {
    if (!canUpload) return
    dispatch({ type: 'uploadStarted' })
    const allIds = draftsToUpload.map(draft => draft.id)
    const payloads = draftsToUpload.map(toPayload)
    if (findDuplicateUrns(payloads).length > 0) {
      track('Add items error', { collectionId: collection.id, item_count: allIds.length, error: 'duplicate_urn' })
      return dispatch({ type: 'uploadFailed', failureReason: 'duplicate_urn', failedDraftIds: allIds })
    }
    try {
      const operations = await planUpload(payloads, collectionItems, { replaceByUrn: updateOnly })
      const result = await executeUpload(address, operations, sizeDeps)

      // One event per upload rather than per item: the legacy builder's per-item `Save item` on creation
      // reads here as `item_count` (see docs/TRACKING_SPEC.md).
      if (updateOnly) {
        // Legacy event name, so bulk edits line up with the old builder's.
        track('Edit TP Items', {
          collectionId: collection.id,
          item_count: result.savedDraftIds.length,
          failed_count: result.failedDraftIds.length
        })
      } else
        track('Add items', {
          collectionId: collection.id,
          origin: prefill ? 'live_preview' : 'collection',
          item_count: result.savedDraftIds.length,
          failed_count: result.failedDraftIds.length,
          error: result.failedDraftIds.length > 0 ? (result.failureReason ?? 'generic') : undefined
        })

      if (result.savedDraftIds.length > 0) {
        showToast(
          t(updateOnly ? 'add_items_modal.bulk_edit.success_toast' : 'add_items_modal.success_toast', {
            count: result.savedDraftIds.length
          })
        )
        await Promise.all([
          queryClient.invalidateQueries({ queryKey: allCollectionItemsKey(address, collection.id) }),
          queryClient.invalidateQueries({ queryKey: ['collection', address, collection.id] }),
          queryClient.invalidateQueries({ queryKey: ['collection-preview', address, collection.id] }),
          queryClient.invalidateQueries({ queryKey: ['collections'] }),
          // Linked collections page their items and curations server-side under their own keys.
          queryClient.invalidateQueries({ queryKey: linkedItemsKey(address, collection.id) }),
          queryClient.invalidateQueries({ queryKey: itemCurationsKey(address, collection.id) })
        ])
      }

      if (result.failedDraftIds.length > 0) {
        dispatch({
          type: 'uploadFailed',
          failureReason: result.failureReason ?? 'generic',
          failedDraftIds: result.failedDraftIds
        })
      } else {
        onClose()
      }
    } catch (error) {
      track('Add items error', { collectionId: collection.id, item_count: allIds.length, error: errorCode(error) })
      dispatch({ type: 'uploadFailed', failureReason: 'generic', failedDraftIds: allIds })
    }
  }

  function handleFinish() {
    if (!selected) return
    // FINISH also confirms the item being viewed; every other draft is already checked, so the
    // selection stays put and the list shows the last green check before the upload starts.
    dispatch({ type: 'draftChecked', id: selected.id })
    const finalDrafts = drafts.map(draft => (draft.id === selected.id ? { ...draft, checked: true } : draft))
    void upload(finalDrafts)
  }

  function handleVideoChange(video: File) {
    if (!selected) return
    dispatch({
      type: 'draftUpdated',
      id: selected.id,
      patch: { contents: { ...selected.contents, [VIDEO_PATH]: video } }
    })
  }

  function handleSaveChanges() {
    if (!canUpload) {
      setLeaveConfirmOpen(false)
      showToast(t(itemsFailed ? 'add_items_modal.items_error' : 'add_items_modal.items_loading'), {
        type: itemsFailed ? 'error' : 'warn'
      })
      return
    }
    const checked = drafts.filter(draft => draft.checked)
    // A checked variant whose base draft is still unreviewed can't be saved on its own: point the
    // creator at that base draft instead of silently dropping the variant.
    const checkedIds = new Set(checked.map(draft => draft.id))
    // Variants of existing collection items are fine on their own.
    const orphan = checked.find(
      draft =>
        draft.isVariant &&
        !checkedIds.has(draft.variantTargetId ?? '') &&
        drafts.some(candidate => candidate.id === draft.variantTargetId)
    )
    const orphanTarget = orphan && drafts.find(draft => draft.id === orphan.variantTargetId)
    setLeaveConfirmOpen(false)
    if (orphan && orphanTarget) {
      dispatch({ type: 'draftSelected', id: orphanTarget.id })
      showToast(t('add_items_modal.leave.review_target_first', { target: orphanTarget.name, variant: orphan.name }), {
        type: 'error'
      })
      return
    }
    void upload(checked)
  }

  function requestClose() {
    if (isUploading || isLeaveConfirmOpen || thumbnailEditor.isOpen || isVideoOpen) return
    // Nothing salvageable to lose: every file failed to import.
    if (drafts.every(draft => draft.status === 'failed')) {
      onClose()
      return
    }
    setLeaveConfirmOpen(true)
  }

  if (view === 'error') {
    return (
      <UploadErrorModal
        reason={failureReason ?? 'generic'}
        onCancel={onClose}
        onRetry={() => dispatch({ type: 'retryRequested' })}
      />
    )
  }

  const isSingle = drafts.length === 1
  const showFinish = isSingle || othersChecked

  return (
    <>
      <Modal
        title={
          updateOnly
            ? t('add_items_modal.bulk_edit.title', { count: drafts.length })
            : t('add_items_modal.title', { count: drafts.length })
        }
        onClose={requestClose}
        size="wide"
        flush={!isSingle}
        closeDisabled={isUploading}
        testId="add-items-modal"
      >
        {/* `inert` isn't in React 18's typings, so it goes through the spread. */}
        <div
          className={draftModal.layout}
          data-flush={!isSingle || undefined}
          data-uploading={isUploading || undefined}
          aria-busy={isUploading || undefined}
          {...(isUploading ? { inert: '' } : {})}
        >
          {/* A single upload gets the sidebar-less layout from the design. */}
          {!isSingle && (
            <DraftList
              drafts={drafts}
              selectedId={selected?.id ?? null}
              onSelect={id => dispatch({ type: 'draftSelected', id })}
              onRemove={id => dispatch({ type: 'draftRemoved', id })}
            />
          )}
          <div className={draftModal.main}>
            {selected && selected.status === 'ready' ? (
              <DraftForm
                draft={selected}
                drafts={drafts}
                collectionItems={collectionItems}
                onUpdate={(id, patch) => dispatch({ type: 'draftUpdated', id, patch })}
                onOpenThumbnail={() =>
                  thumbnailEditor.edit({
                    kind: 'files',
                    id: selected.id,
                    type: selected.type,
                    contents: selected.contents
                  })
                }
                onOpenVideo={() => setVideoOpen(true)}
                onVideoChange={handleVideoChange}
                size={selectedSize}
                linked={isLinked ? { contract: linkedContract, updating: updateOnly } : undefined}
              />
            ) : (
              <div
                className={draftModal.processingPane}
                data-testid="draft-processing"
                data-error={selected?.status === 'failed' ? (selected.errorKey ?? 'invalid_model_file') : undefined}
              >
                {selected?.status === 'failed' ? (
                  t(`add_items_modal.file_error.${selected.errorKey ?? 'invalid_model_file'}`, selected.errorParams)
                ) : (
                  <>
                    <span className={draftModal.processingSpinner} data-testid="draft-processing-spinner" aria-hidden />
                    {t('add_items_modal.processing')}
                  </>
                )}
              </div>
            )}
            {itemsFailed && (
              <div className={styles.itemsError} role="alert" data-testid="add-items-items-error">
                <p className={draftModal.errorText}>{t('add_items_modal.items_error')}</p>
                <Button
                  type="button"
                  size="sm"
                  variant="secondary"
                  data-testid="add-items-items-retry"
                  onClick={() => void collectionItemsQuery.refetch()}
                >
                  {t('add_items_modal.error.try_again')}
                </Button>
              </div>
            )}
            <div className={draftModal.footer}>
              <Button
                type="button"
                variant="secondary"
                data-testid="add-items-cancel"
                disabled={isUploading}
                onClick={requestClose}
              >
                {t('add_items_modal.cancel')}
              </Button>
              <div className={draftModal.footerRight}>
                {!isSingle && (
                  <Button
                    type="button"
                    variant="secondary"
                    data-testid="add-items-previous"
                    disabled={isUploading || selectedIndex <= 0}
                    onClick={() => {
                      const previous = drafts[selectedIndex - 1]
                      if (previous) dispatch({ type: 'draftSelected', id: previous.id })
                    }}
                  >
                    {t('add_items_modal.previous')}
                  </Button>
                )}
                {showFinish ? (
                  <Button
                    type="button"
                    variant="primary"
                    data-testid="add-items-finish"
                    disabled={!isSelectedComplete || !canUpload}
                    loading={isUploading}
                    onClick={handleFinish}
                  >
                    {isSingle ? t('add_items_modal.save') : t('add_items_modal.finish')}
                  </Button>
                ) : (
                  <Button
                    type="button"
                    variant="primary"
                    data-testid="add-items-save-next"
                    disabled={!isSelectedComplete || isUploading}
                    onClick={() => selected && dispatch({ type: 'draftChecked', id: selected.id })}
                  >
                    {t('add_items_modal.save_next')}
                  </Button>
                )}
              </div>
            </div>
          </div>
        </div>
        {previewDraft && (
          <DraftProcessor
            draft={previewDraft}
            flow="add_items"
            batchSize={drafts.length}
            onResult={(id, result) => dispatch({ type: 'draftPreviewed', id, result })}
            onError={(id, reason) =>
              dispatch({
                type: 'draftFailed',
                id,
                errorKey: reason === 'timeout' ? 'preview_timeout' : 'invalid_model_file'
              })
            }
          />
        )}
      </Modal>

      {thumbnailEditor.modal}

      {isVideoOpen && selected && selected.status === 'ready' && (
        <VideoModal
          video={selected.contents[VIDEO_PATH] ?? null}
          onChange={handleVideoChange}
          onClose={() => setVideoOpen(false)}
        />
      )}

      {isLeaveConfirmOpen && (
        <LeaveConfirmModal
          hasCheckedDrafts={hasCheckedDrafts}
          isSaving={isUploading}
          onLeave={onClose}
          onSaveChanges={handleSaveChanges}
          onStay={() => setLeaveConfirmOpen(false)}
        />
      )}
    </>
  )
}
