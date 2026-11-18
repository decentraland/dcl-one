import { useEffect, useMemo, useState } from 'react'
import { CategorySelect } from '@/features/creators/components/CategorySelect'
import {
  ExpandMoreIcon,
  LoopIcon,
  PlayOnceIcon,
  ErrorOutlineIcon as HintIcon,
  InfoOutlinedIcon as InfoIcon,
  PlayArrowIcon as PlayIcon,
  VideocamOutlinedIcon as VideoIcon
} from '@/features/creators/components/Icons'
import { RaritySelect } from '@/features/creators/components/RaritySelect'
import { InfoTooltip, Tooltip } from '@/features/creators/components/Tooltip'
import { VideoDropzone } from '@/features/creators/components/VideoModal'
import { useObjectURL } from '@/features/creators/hooks/useObjectURL'
import { useTranslation } from '@/data/lib/creators/intl'
import { EmotePlayMode, ITEM_NAME_MAX_LENGTH, isValidItemName } from '@/data/lib/creators/itemFactory'
import { formatItemSize, isOverSizeLimit, type ItemSize } from '@/data/lib/creators/itemSize'
import { toMB } from '@/data/lib/creators/itemFiles'
import { BodyShapeType, ItemType, VIDEO_PATH, type Item } from '@/data/lib/creators/items'
import { type Mapping } from '@/data/lib/creators/mappings'
import { decodeLinkedUrn, type LinkedContract } from '@/data/lib/creators/thirdParty'
import { MappingEditor } from '@/features/creators/components/MappingEditor'
import { getCategoryOptions, getVariantTargets, type ItemDraft } from './AddItemsModal.state'
import { BodyShapePicker, DraftPreviewPane } from '@/features/creators/components/ItemDraftModal'
import draftModal from '@/features/creators/components/ItemDraftModal/ItemDraftModal.module.css'
import styles from './AddItemsModal.module.css'

type Props = {
  draft: ItemDraft
  drafts: ItemDraft[]
  collectionItems: Item[]
  onUpdate: (id: string, patch: Partial<ItemDraft>) => void
  onOpenThumbnail: () => void
  /** Smart wearables only: opens the preview video overlay. */
  onOpenVideo: () => void
  /** Smart wearables only: a picked file replaces the preview video. */
  onVideoChange: (video: File) => void
  /** Linked collections: no rarity, and the mapping field when the collection has a linked contract. */
  linked?: { contract: LinkedContract | null; updating?: boolean }
  /** The item this draft will be saved as, measured the catalyst way; null while unknown. */
  size?: ItemSize | null
}

export function DraftForm({
  draft,
  drafts,
  collectionItems,
  onUpdate,
  onOpenThumbnail,
  onOpenVideo,
  onVideoChange,
  linked,
  size = null
}: Props) {
  const { t } = useTranslation()

  const isEmote = draft.type === ItemType.EMOTE
  const isWearable = draft.type === ItemType.WEARABLE
  const isSmart = isWearable && draft.isSmart
  const bodyShapeHint = isEmote
    ? 'add_items_modal.body_shape_emote_hint'
    : isSmart
      ? 'add_items_modal.body_shape_smart_hint'
      : null
  const isSingleShape = isWearable && draft.bodyShape !== BodyShapeType.BOTH
  const variantTargets = useMemo(
    () => (isSingleShape ? getVariantTargets(draft, drafts, collectionItems) : []),
    [isSingleShape, draft, drafts, collectionItems]
  )
  // Edit in bulk: the item this zip's wearable.json names, once the zip has been read.
  const updatedItem = useMemo(
    () =>
      linked?.updating && draft.urnSuffix
        ? collectionItems.find(item => decodeLinkedUrn(item.urn)?.itemSuffix === draft.urnSuffix)
        : undefined,
    [linked?.updating, draft.urnSuffix, collectionItems]
  )
  const variantTargetShape = draft.bodyShape === BodyShapeType.MALE ? BodyShapeType.FEMALE : BodyShapeType.MALE
  const showItemFields = !draft.isVariant
  const sizeText = useMemo(() => (size ? formatItemSize(size) : null), [size])
  const sizeExceeded = !!size && isOverSizeLimit(size)
  const nameInvalid = draft.name.length > 0 && !isValidItemName(draft.name)
  const categories = useMemo(() => getCategoryOptions(draft), [draft])

  const video = draft.contents[VIDEO_PATH]
  const videoUrl = useObjectURL(video)
  const [videoDuration, setVideoDuration] = useState<number | null>(null)
  // A new video (or draft) must not flash the previous one's duration until its metadata loads.
  useEffect(() => setVideoDuration(null), [videoUrl])

  return (
    <section className={draftModal.content} data-testid="draft-form">
      <DraftPreviewPane draft={draft} thumbnailLocked={draft.isVariant} onOpenThumbnail={onOpenThumbnail} />

      <div className={draftModal.formPane}>
        <h3 className={draftModal.formHeading}>
          {t(linked?.updating ? 'add_items_modal.bulk_edit.form_heading' : 'add_items_modal.form_heading')}
        </h3>
        {updatedItem?.urn && (
          <span className={draftModal.fieldHint} data-testid="bulk-edit-target" title={updatedItem.urn}>
            <InfoIcon />
            {t('add_items_modal.bulk_edit.target', {
              name: updatedItem.name,
              urn: decodeLinkedUrn(updatedItem.urn)?.itemSuffix ?? updatedItem.urn
            })}
          </span>
        )}

        {showItemFields && (
          <label className={draftModal.field}>
            {t('add_items_modal.item_name')}
            <div className={styles.textInputBox} data-invalid={nameInvalid || undefined}>
              <input
                value={draft.name}
                maxLength={ITEM_NAME_MAX_LENGTH}
                placeholder={t('add_items_modal.item_name_placeholder')}
                data-testid="item-name"
                data-invalid={nameInvalid || undefined}
                onChange={event => onUpdate(draft.id, { name: event.target.value })}
              />
              <span className={styles.charCount} data-testid="item-name-count">
                {t('add_items_modal.char_count', { count: draft.name.length, max: ITEM_NAME_MAX_LENGTH })}
              </span>
            </div>
            {nameInvalid && <p className={draftModal.errorText}>{t('add_items_modal.invalid_name')}</p>}
          </label>
        )}

        {(isWearable || isEmote) && (
          <div className={draftModal.field} data-hinted={bodyShapeHint ? true : undefined}>
            {bodyShapeHint ? (
              <span className={draftModal.fieldLabel}>
                {t('add_items_modal.body_shape')}
                <span className={draftModal.fieldHint} data-testid="body-shape-hint">
                  <HintIcon />
                  {t(bodyShapeHint)}
                </span>
              </span>
            ) : (
              t('add_items_modal.body_shape')
            )}
            <BodyShapePicker
              value={draft.bodyShape}
              locked={draft.bodyShapeLocked}
              lockedHint={bodyShapeHint ? t(bodyShapeHint) : null}
              onChange={value =>
                onUpdate(draft.id, {
                  bodyShape: value,
                  // Going back to BOTH clears the variant answer entirely.
                  ...(value === BodyShapeType.BOTH
                    ? { isVariant: false, variantTargetId: null }
                    : { variantTargetId: null })
                })
              }
            />
          </div>
        )}

        {isSingleShape && (
          <div className={draftModal.field}>
            {t('add_items_modal.variant_question')}
            <div
              className={draftModal.segmented}
              data-joined
              role="radiogroup"
              aria-label={t('add_items_modal.variant_question')}
            >
              {[true, false].map(answer => (
                <button
                  className={draftModal.segmentButton}
                  key={String(answer)}
                  type="button"
                  role="radio"
                  aria-checked={draft.isVariant === answer}
                  data-selected={draft.isVariant === answer || undefined}
                  data-testid={`variant-${answer ? 'yes' : 'no'}`}
                  onClick={() => onUpdate(draft.id, { isVariant: answer, variantTargetId: null })}
                >
                  {t(answer ? 'add_items_modal.yes' : 'add_items_modal.no')}
                </button>
              ))}
            </div>
          </div>
        )}

        {draft.isVariant && variantTargets.length === 0 && (
          <div className={draftModal.warningCard} data-testid="variant-target-empty">
            <InfoIcon />
            {t('add_items_modal.no_variant_targets', {
              bodyShape: t(`add_items_modal.body_shape_option.${variantTargetShape}`)
            })}
          </div>
        )}

        {draft.isVariant && variantTargets.length > 0 && (
          <div className={draftModal.field}>
            <div className={styles.selectField}>
              <select
                className={styles.select}
                value={draft.variantTargetId ?? ''}
                aria-label={t('add_items_modal.select_item')}
                data-testid="variant-target"
                onChange={event => onUpdate(draft.id, { variantTargetId: event.target.value || null })}
              >
                <option value="">{t('add_items_modal.select_item')}</option>
                {variantTargets.map(target => (
                  <option key={target.id} value={target.id}>
                    {t('add_items_modal.variant_target_label', {
                      name: target.label,
                      bodyShape: t(`add_items_modal.body_shape_option.${target.bodyShape}`)
                    })}
                  </option>
                ))}
              </select>
              <ExpandMoreIcon className={styles.selectChevron} />
            </div>
          </div>
        )}

        {isEmote && showItemFields && (
          <div className={draftModal.field}>
            {t('add_items_modal.play_mode')}
            <div className={draftModal.segmented} role="radiogroup" aria-label={t('add_items_modal.play_mode')}>
              {[EmotePlayMode.LOOP, EmotePlayMode.SIMPLE].map(mode => (
                <Tooltip
                  key={mode}
                  content={t(`add_items_modal.play_mode_tooltip.${mode}`)}
                  asChild
                  testId={`play-mode-${mode}-tooltip`}
                >
                  <button
                    className={draftModal.segmentButton}
                    type="button"
                    role="radio"
                    aria-checked={draft.playMode === mode}
                    data-selected={draft.playMode === mode || undefined}
                    data-testid={`play-mode-${mode}`}
                    onClick={() => onUpdate(draft.id, { playMode: mode })}
                  >
                    {mode === EmotePlayMode.LOOP ? <LoopIcon /> : <PlayOnceIcon />}
                    {t(`add_items_modal.play_mode_option.${mode}`)}
                  </button>
                </Tooltip>
              ))}
            </div>
          </div>
        )}

        {showItemFields && (
          <div className={styles.fieldRow}>
            <label className={draftModal.field}>
              {t('add_items_modal.category')}
              <CategorySelect
                value={draft.category}
                categories={categories}
                testId="item-category"
                onChange={category => onUpdate(draft.id, { category })}
              />
              {isWearable && draft.suggestedCategory !== null && draft.category === draft.suggestedCategory && (
                <span className={draftModal.fieldHint} data-testid="suggested-category">
                  <HintIcon />
                  {t('add_items_modal.suggested_category', {
                    category: t(`collection_detail_page.category.${draft.suggestedCategory}`)
                  })}
                </span>
              )}
            </label>
            {!linked && (
              <label className={draftModal.field}>
                <span className={styles.fieldLabelRow}>
                  {t('add_items_modal.rarity')}
                  <InfoTooltip content={t('add_items_modal.rarity_tooltip')} testId="rarity-tooltip" />
                </span>
                <RaritySelect
                  value={draft.rarity}
                  testId="item-rarity"
                  onChange={rarity => onUpdate(draft.id, { rarity })}
                />
              </label>
            )}
          </div>
        )}

        {showItemFields && linked?.contract && (
          <div className={draftModal.field} data-testid="draft-mapping-field">
            <span className={draftModal.fieldLabel}>
              {t('mapping_editor.label')}
              <span className={draftModal.fieldHint}>{t('add_items_modal.mapping_hint')}</span>
            </span>
            <MappingEditor
              key={draft.id}
              value={draft.mapping}
              testId="draft-mapping"
              onChange={(mapping: Mapping | null) => onUpdate(draft.id, { mapping })}
            />
          </div>
        )}

        {isSmart && (
          <div className={draftModal.field} data-testid="draft-video-field">
            <span className={draftModal.fieldLabel}>
              {t('add_items_modal.video.label')}
              <span className={draftModal.fieldHint}>{t('add_items_modal.video.field_hint')}</span>
            </span>
            <VideoDropzone compact testId="draft-video" onPick={onVideoChange}>
              {video && videoUrl && (
                <>
                  <button
                    className={styles.videoPoster}
                    type="button"
                    aria-label={t('add_items_modal.video.edit')}
                    data-testid="draft-video-preview"
                    onClick={onOpenVideo}
                  >
                    <video
                      key={videoUrl}
                      src={videoUrl}
                      preload="metadata"
                      muted
                      playsInline
                      onLoadedMetadata={event => {
                        const { duration } = event.currentTarget
                        setVideoDuration(Number.isFinite(duration) ? duration : null)
                      }}
                    />
                    <span className={styles.videoPlay}>
                      <PlayIcon />
                    </span>
                    <span className={styles.videoPosterOverlay} data-video-overlay>
                      <VideoIcon />
                    </span>
                  </button>
                  <span className={styles.videoInfo}>
                    <span className={styles.videoName}>{video instanceof File ? video.name : VIDEO_PATH}</span>
                    <span className={styles.videoMeta} data-testid="draft-video-meta">
                      {videoDuration !== null && (
                        <span>{t('add_items_modal.video.duration', { seconds: Math.round(videoDuration) })}</span>
                      )}
                      <span>{t('add_items_modal.video.size', { size: toMB(video.size) })}</span>
                    </span>
                  </span>
                </>
              )}
            </VideoDropzone>
          </div>
        )}

        {sizeText && (
          <p className={draftModal.sizeLine} data-testid="draft-size" data-exceeded={sizeExceeded || undefined}>
            {t('item_size.label')}
            <strong>{t('item_size.value', sizeText)}</strong>
          </p>
        )}
        {sizeExceeded && sizeText && (
          <p className={draftModal.errorText} role="alert" data-testid="size-error">
            {t('add_items_modal.file_error.size_exceeded', sizeText)}
          </p>
        )}
      </div>
    </section>
  )
}
