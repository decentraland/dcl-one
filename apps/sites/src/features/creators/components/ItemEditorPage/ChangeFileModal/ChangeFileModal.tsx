import { useEffect, useMemo, useRef, useState, type DragEvent } from 'react'
import { ErrorOutlineIcon as HintIcon, UploadFileOutlinedIcon as UploadIcon } from '@/features/creators/components/Icons'
import { Button } from '@/features/creators/components/Button'
import { CategorySelect } from '@/features/creators/components/CategorySelect'
import { Checkbox } from '@/features/creators/components/Checkbox'
import { ConfirmModal } from '@/features/creators/components/ConfirmModal'
import { BodyShapePicker, DraftPreviewPane, DraftProcessor } from '@/features/creators/components/ItemDraftModal'
import { Modal } from '@/features/creators/components/Modal'
import { useMeasuredBytes } from '@/features/creators/hooks/useItemSize'
import { useThumbnailEditor } from '@/features/creators/hooks/useThumbnailEditor'
import { useTranslation } from '@/data/lib/creators/intl'
import { errorCode, track } from '@/data/lib/creators/analytics'
import { applyPreviewResult, imageThumbnailPatch, needsPreviewData, type PreviewDraft } from '@/data/lib/creators/draftPreview'
import { pickFile } from '@/data/lib/creators/filePicker'
import { toFileChangeSaveable, type ItemFileChange } from '@/data/lib/creators/itemDraft'
import { type BuiltItem } from '@/data/lib/creators/itemFactory'
import { ITEM_EXTENSIONS, ItemFileError, THUMBNAIL_PATH, getExtension, isImageFile } from '@/data/lib/creators/itemFiles'
import {
  buildModelUpdate,
  getDefaultBodyShape,
  getModelUpdateError,
  getModelUpdateMode,
  narrowToShape,
  isBodyShapeLocked,
  loadModelForItem,
  needsNewCategory,
  type LoadedItemModel
} from '@/data/lib/creators/itemModelImport'
import { formatItemSize, getItemSizeLimit, isItemTooBigError, isOverSizeLimit, type ItemSize } from '@/data/lib/creators/itemSize'
import {
  BodyShapeType,
  IMAGE_WEARABLE_METRICS,
  ItemType,
  getItemBodyShapeType,
  isSmartWearable,
  type Item
} from '@/data/lib/creators/items'
import { captureError } from '@/data/lib/creators/monitoring'
import { DEFAULT_RARITY } from '@/data/lib/creators/rarities'
import { getWearableCategoryOptions } from '@/data/lib/creators/wearableCategories'
import draftModal from '@/features/creators/components/ItemDraftModal/ItemDraftModal.module.css'
import styles from './ChangeFileModal.module.css'

type Props = {
  item: Item
  /** "Add {shape} representation": the pick is fixed to the shape the item lacks. */
  lockedBodyShape?: BodyShapeType.MALE | BodyShapeType.FEMALE
  /** Items of a linked collection take no smart zip, as add items enforces. */
  linked?: boolean
  /** Persists the change; the modal closes when it resolves and stays open when it rejects (already reported). */
  onSave: (change: ItemFileChange) => Promise<void>
  onClose: () => void
}

/** The preview draft for a file: a 3D model leaves metrics and render to the DraftProcessor, a texture has fixed metrics. */
function makeDraft(file: LoadedItemModel, item: Item): PreviewDraft {
  return {
    id: crypto.randomUUID(),
    type: file.type,
    contents: file.contents,
    model: file.model,
    category: item.data.category ?? null,
    rarity: item.rarity ?? DEFAULT_RARITY,
    isSmart: file.isSmart,
    requiredPermissions: file.requiredPermissions,
    metrics: isImageFile(file.model) ? IMAGE_WEARABLE_METRICS : file.metrics,
    thumbnail: null,
    isAutoThumbnail: false,
    autoThumbnailCategory: null,
    validationIssues: file.validationIssues
  }
}

/**
 * Loads a new model for a saved item and lets the creator pick which body shape it goes to; Save
 * persists the item at once, as the legacy builder's modal did.
 */
export function ChangeFileModal({ item, lockedBodyShape, linked = false, onSave, onClose }: Props) {
  const { t, rich } = useTranslation()
  const [picked, setPicked] = useState<File | null>(null)
  const [loaded, setLoaded] = useState<LoadedItemModel | null>(null)
  const [draft, setDraft] = useState<PreviewDraft | null>(null)
  const [isLoading, setLoading] = useState(false)
  const [isDragging, setDragging] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [bodyShape, setBodyShape] = useState<BodyShapeType>(lockedBodyShape ?? BodyShapeType.BOTH)
  const [keepThumbnail, setKeepThumbnail] = useState(true)
  const [category, setCategory] = useState<string | null>(null)
  const [isSaving, setSaving] = useState(false)
  const [isDiscardOpen, setDiscardOpen] = useState(false)
  // A file picked after the modal closed must not land in state; a texture thumbnail padded for a pick
  // that was changed again meanwhile must not either.
  const mountedRef = useRef(true)
  const pickRef = useRef(0)
  useEffect(() => {
    mountedRef.current = true
    return () => {
      mountedRef.current = false
    }
  }, [])

  const thumbnailEditor = useThumbnailEditor(patch => {
    setDraft(
      current => current && { ...current, thumbnail: patch.thumbnail, contents: patch.contents, isAutoThumbnail: false }
    )
  })

  const locked = lockedBodyShape !== undefined || isBodyShapeLocked(item, loaded)
  const lockHint =
    item.type === ItemType.EMOTE
      ? t('add_items_modal.body_shape_emote_hint')
      : isSmartWearable(item) || loaded?.isSmart
        ? t('add_items_modal.body_shape_smart_hint')
        : null
  // A two-folder zip is seen through the pick: one folder, as the save will take it.
  const file = useMemo(() => (loaded ? narrowToShape(loaded, bodyShape) : null), [loaded, bodyShape])
  const mode = useMemo(() => getModelUpdateMode(item, bodyShape), [item, bodyShape])
  const pickError = useMemo(() => (file ? getModelUpdateError(file, item, bodyShape) : null), [file, item, bodyShape])
  const needsCategory = useMemo(() => !!file && needsNewCategory(file, item, bodyShape), [file, item, bodyShape])
  const categories = useMemo(() => (file ? getWearableCategoryOptions(file.contents) : []), [file])
  const shapeLabel = t(`add_items_modal.body_shape_option.${bodyShape}`)
  // The item as Save would leave it, measured the catalyst way: the new files plus everything it keeps (the other
  // body shape's files, the thumbnail, new or kept, and the rarity image).
  const newThumbnail = keepThumbnail ? (draft?.contents[THUMBNAIL_PATH] ?? null) : null
  const measuredBytes = useMeasuredBytes(
    file && !pickError
      ? async () =>
          toFileChangeSaveable(item, {
            update: await buildModelUpdate(file, item, bodyShape, {}),
            thumbnail: newThumbnail,
            category: null
          })
      : null,
    [item, file, bodyShape, newThumbnail],
    'item_file_change'
  )
  const sizeCategory = (needsCategory ? category : null) ?? item.data.category
  const size = useMemo<ItemSize | null>(
    () => (measuredBytes !== null ? { bytes: measuredBytes, limit: getItemSizeLimit(item.type, sizeCategory) } : null),
    [measuredBytes, item.type, sizeCategory]
  )
  const sizeText = useMemo(() => (size ? formatItemSize(size) : null), [size])
  const sizeExceeded = !!size && isOverSizeLimit(size)
  const isReady = !!draft && !!draft.metrics && !!draft.thumbnail
  const canSave = isReady && !pickError && (!needsCategory || category !== null) && !sizeExceeded && !isSaving

  function reportError(err: unknown) {
    const key = err instanceof ItemFileError ? err.messageKey : 'invalid_model_file'
    const params = err instanceof ItemFileError ? err.messageParams : undefined
    if (!(err instanceof ItemFileError)) captureError(err, { flow: 'item_file_change' })
    track('Change item file error', { itemId: item.id, error: err instanceof ItemFileError ? key : errorCode(err) })
    setError(t(`add_items_modal.file_error.${key}`, params))
  }

  /** Shows a file as picked: a fresh draft, and for a texture its padded thumbnail once rendered. */
  function showDraft(next: LoadedItemModel) {
    const pick = ++pickRef.current
    const draft = makeDraft(next, item)
    setDraft(draft)
    if (!isImageFile(next.model)) return
    imageThumbnailPatch(next.contents, next.model, draft.category)
      .then(patch => {
        if (!mountedRef.current || pickRef.current !== pick) return
        setDraft(
          current => current && { ...current, ...patch, isAutoThumbnail: true, autoThumbnailCategory: draft.category }
        )
      })
      .catch((err: unknown) => {
        if (!mountedRef.current || pickRef.current !== pick) return
        // Without a thumbnail there is nothing to save: back to the drop area, with the reason.
        setDraft(null)
        setLoaded(null)
        reportError(err)
      })
  }

  async function handleFile(picked: File | null | undefined) {
    if (!picked) return
    setDragging(false)
    setError(null)
    setLoading(true)
    try {
      const next = await loadModelForItem(picked, item, { linked })
      if (!mountedRef.current) return
      const shape = lockedBodyShape ?? getDefaultBodyShape(item, next)
      setPicked(picked)
      setLoaded(next)
      setBodyShape(shape)
      setCategory(null)
      showDraft(narrowToShape(next, shape))
    } catch (err) {
      if (mountedRef.current) reportError(err)
    } finally {
      if (mountedRef.current) setLoading(false)
    }
  }

  function pickShape(value: BodyShapeType) {
    if (locked || !loaded || value === bodyShape) return
    setBodyShape(value)
    // What the last pick said or chose no longer applies: a size error, a category for another kind of file.
    setError(null)
    setCategory(null)
    // Another folder of a two-folder zip is another model: a new draft, measured and rendered again.
    const next = narrowToShape(loaded, value)
    if (next !== file) showDraft(next)
  }

  function handleDrop(event: DragEvent) {
    event.preventDefault()
    if (isLoading) return
    handleFile(event.dataTransfer.files[0]).catch(() => undefined)
  }

  async function browse() {
    await handleFile(await pickFile({ accept: ITEM_EXTENSIONS.join(',') }))
  }

  async function save() {
    if (!file || !draft || !canSave) return
    setSaving(true)
    let update: BuiltItem
    try {
      update = await buildModelUpdate(file, item, bodyShape, draft.metrics ?? {})
    } catch (err) {
      reportError(err)
      setSaving(false)
      return
    }
    const thumbnail = keepThumbnail ? (draft.contents[THUMBNAIL_PATH] ?? null) : null
    try {
      await onSave({ update, thumbnail, category: needsCategory ? category : null })
    } catch (err) {
      // The editor reported any other failed save; the loaded file stays for another try.
      if (mountedRef.current) {
        if (isItemTooBigError(err)) reportError(err)
        setSaving(false)
      }
      return
    }
    track('Change item file', {
      itemId: item.id,
      bodyShape,
      mode,
      fileKind: (picked && getExtension(picked.name)?.slice(1)) ?? 'unknown',
      thumbnailRegenerated: thumbnail !== null
    })
    onClose()
  }

  function requestClose() {
    if (isSaving || thumbnailEditor.isOpen) return
    if (draft) setDiscardOpen(true)
    else onClose()
  }

  const title = lockedBodyShape
    ? t('item_editor.details.add_representation', { shape: t(`add_items_modal.body_shape_option.${lockedBodyShape}`) })
    : t('item_editor.details.change_file')

  return (
    <>
      <Modal
        title={title}
        onClose={requestClose}
        size={draft && loaded ? 'wide' : 'default'}
        closeDisabled={isSaving}
        testId="change-file-modal"
      >
        {draft && loaded ? (
          <div className={draftModal.layout}>
            <div className={draftModal.main}>
              <section className={draftModal.content} data-testid="change-file-form">
                <DraftPreviewPane
                  draft={draft}
                  onOpenThumbnail={() =>
                    thumbnailEditor.edit({ kind: 'files', id: draft.id, type: draft.type, contents: draft.contents })
                  }
                >
                  <div className={styles.thumbnailToggle}>
                    <Checkbox checked={keepThumbnail} onChange={setKeepThumbnail} testId="change-file-keep-thumbnail">
                      {t('item_editor.change_file.use_thumbnail')}
                    </Checkbox>
                  </div>
                </DraftPreviewPane>
                <div className={draftModal.formPane}>
                  <h3 className={draftModal.formHeading}>{t('item_editor.change_file.heading')}</h3>
                  <div className={draftModal.field} data-hinted>
                    <span className={draftModal.fieldLabel}>
                      {t('add_items_modal.body_shape')}
                      <span className={draftModal.fieldHint} data-testid="change-file-effect" data-mode={mode}>
                        <HintIcon />
                        {lockHint ??
                          t(
                            mode === 'replace_shape' && getItemBodyShapeType(item) !== BodyShapeType.BOTH
                              ? 'item_editor.change_file.effect.replace_only_shape'
                              : `item_editor.change_file.effect.${mode}`,
                            { shape: shapeLabel }
                          )}
                      </span>
                    </span>
                    <BodyShapePicker
                      value={bodyShape}
                      locked={locked}
                      lockedHint={lockHint ?? t('item_editor.change_file.locked_hint')}
                      onChange={pickShape}
                    />
                    {pickError && (
                      <p className={draftModal.errorText} data-testid="change-file-pick-error">
                        {t(`add_items_modal.file_error.${pickError}`)}
                      </p>
                    )}
                  </div>
                  {needsCategory && (
                    <div className={draftModal.field}>
                      <span className={draftModal.fieldLabel}>
                        {t('add_items_modal.category')}
                        <span className={draftModal.fieldHint}>
                          <HintIcon />
                          {t('item_editor.change_file.category_hint')}
                        </span>
                      </span>
                      <CategorySelect
                        value={category}
                        categories={categories}
                        testId="change-file-category"
                        onChange={setCategory}
                      />
                    </div>
                  )}
                  {sizeText && (
                    <p
                      className={draftModal.sizeLine}
                      data-testid="change-file-size"
                      data-exceeded={sizeExceeded || undefined}
                    >
                      {t('item_size.label')}
                      <strong>{t('item_size.value', sizeText)}</strong>
                    </p>
                  )}
                  {sizeExceeded && sizeText && (
                    <p className={draftModal.errorText} role="alert" data-testid="change-file-size-error">
                      {t('add_items_modal.file_error.size_exceeded', sizeText)}
                    </p>
                  )}
                  {error && (
                    <p className={draftModal.errorText} data-testid="change-file-error">
                      {error}
                    </p>
                  )}
                </div>
              </section>
              <div className={draftModal.footer}>
                <Button
                  type="button"
                  variant="secondary"
                  data-testid="change-file-cancel"
                  disabled={isSaving}
                  onClick={requestClose}
                >
                  {t('add_items_modal.cancel')}
                </Button>
                <div className={draftModal.footerRight}>
                  <Button
                    type="button"
                    variant="primary"
                    data-testid="change-file-save"
                    disabled={!canSave}
                    loading={isSaving}
                    onClick={() => void save()}
                  >
                    {t('add_items_modal.save')}
                  </Button>
                </div>
              </div>
            </div>
          </div>
        ) : (
          <div className={styles.pickStep}>
            <div
              className={styles.dropzone}
              data-testid="change-file-dropzone"
              data-dragging={isDragging || undefined}
              data-busy={isLoading || undefined}
              onDragOver={event => {
                event.preventDefault()
                if (!isLoading) setDragging(true)
              }}
              onDragLeave={() => setDragging(false)}
              onDrop={handleDrop}
            >
              {isLoading && (
                <span className={styles.spinner} aria-hidden data-spinner data-testid="change-file-processing" />
              )}
              <span className={styles.dropIcon}>
                <UploadIcon />
              </span>
              <span className={styles.dropText}>
                {rich('item_editor.change_file.drop_description', {
                  browse: chunks => (
                    <button
                      className={styles.browseLink}
                      type="button"
                      data-testid="change-file-browse"
                      disabled={isLoading}
                      onClick={() => void browse()}
                    >
                      {chunks}
                    </button>
                  )
                })}
              </span>
              <span className={styles.dropFormats}>{t('collection_detail_page.empty.formats')}</span>
            </div>
            {error && (
              <p className={styles.dropError} data-testid="change-file-error">
                {error}
              </p>
            )}
            <div className={styles.pickActions}>
              <Button type="button" variant="secondary" data-testid="change-file-cancel" onClick={requestClose}>
                {t('add_items_modal.cancel')}
              </Button>
            </div>
          </div>
        )}
        {draft && needsPreviewData(draft) && (
          <DraftProcessor
            draft={draft}
            flow="item_file_change"
            onResult={(id, result) =>
              setDraft(current => (current && current.id === id ? applyPreviewResult(current, result) : current))
            }
            onError={(_id, reason) => {
              setDraft(null)
              setLoaded(null)
              // A timeout is already reported to Sentry by the processor.
              reportError(
                reason === 'timeout'
                  ? new ItemFileError('preview_timeout')
                  : new Error('The preview could not process the model')
              )
            }}
          />
        )}
      </Modal>

      {thumbnailEditor.modal}

      {isDiscardOpen && (
        <ConfirmModal
          title={t('item_editor.change_file.discard_title')}
          description={t('item_editor.change_file.discard_description')}
          onClose={() => setDiscardOpen(false)}
          cancel={{ label: t('item_editor.change_file.discard'), onClick: onClose, testId: 'change-file-discard' }}
          confirm={{
            label: t('add_items_modal.leave.keep_editing'),
            onClick: () => setDiscardOpen(false),
            testId: 'change-file-keep-editing'
          }}
          testId="change-file-discard-modal"
        />
      )}
    </>
  )
}
