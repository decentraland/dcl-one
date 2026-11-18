import { useEffect, useMemo, useRef, useState, type Dispatch } from 'react'
import { ActionsMenu, ActionsMenuDivider, ActionsMenuItem } from '../../ActionsMenu'
import { Button } from '../../Button'
import { CategorySelect } from '../../CategorySelect'
import {
  ClockIcon,
  ImageIcon,
  LoopIcon,
  PlayOnceIcon,
  SmartIcon,
  CameraAltIcon as CameraIcon,
  ChangeHistoryIcon as TriangleIcon,
  CircleIcon as MaterialIcon,
  PlayArrowIcon as PlayIcon,
  ReportProblemOutlinedIcon as WarningIcon,
  SentimentSatisfiedAltIcon as SmileIcon,
  StorageIcon as SizeIcon,
  VideocamOutlinedIcon as VideoIcon,
  TextureIcon,
  ThirtyFpsSelectIcon as FpsIcon
} from '../../Icons'
import { ItemThumbnail } from '../../ItemThumbnail'
import { RaritySelect } from '../../RaritySelect'
import { RequiredPermissions } from '../../RequiredPermissions'
import { Switch } from '../../Switch'
import { InfoTooltip, Tooltip } from '../../Tooltip'
import { VideoDropzone, VideoModal } from '../../VideoModal'
import { useCampaign } from '../../../hooks/useCampaign'
import { useFeatureFlag } from '../../../hooks/useFeatureFlag'
import { useObjectURL } from '../../../hooks/useObjectURL'
import { useItemVideo } from '../../../hooks/useItem'
import { useDeleteItem } from '../../../hooks/usePublishCollection'
import { useThumbnailEditor } from '../../../hooks/useThumbnailEditor'
import { useTranslation } from '@/data/lib/creators/intl'
import { getContentsStorageUrl } from '@/data/lib/creators/builder'
import { FeatureFlag } from '@/data/lib/creators/featureFlags'
import {
  ITEM_DESCRIPTION_MAX_LENGTH,
  ITEM_UTILITY_MAX_LENGTH,
  isValidItemDescription,
  type ItemDraft,
  type ItemDraftAction,
  type ItemFileChange
} from '@/data/lib/creators/itemDraft'
import { downloadItem } from '@/data/lib/creators/itemDownload'
import { EmotePlayMode, ITEM_NAME_MAX_LENGTH, isValidItemName } from '@/data/lib/creators/itemFactory'
import { hasFacialExpressions, THUMBNAIL_PATH, VIDEO_PATH, toMB } from '@/data/lib/creators/itemFiles'
import { formatItemSize, isOverSizeLimit, type ItemSize } from '@/data/lib/creators/itemSize'
import {
  BodyShapeType,
  ItemType,
  getMissingBodyShapeType,
  isMissingSmartWearableVideo,
  isSmartWearable,
  type Item
} from '@/data/lib/creators/items'
import { useNotifications } from '@/data/lib/creators/notifications'
import { type SpringBoneParamsByName } from '@/data/lib/creators/springBones'
import { getEmoteCategoryOptions, getWearableCategoryOptions } from '@/data/lib/creators/wearableCategories'
import { buildItemMappings, getItemMapping } from '@/data/lib/creators/mappings'
import { type LinkedContract } from '@/data/lib/creators/thirdParty'
import { MappingEditor } from '../../MappingEditor'
import { DeleteItemModal } from '../../CollectionDetailPage/DeleteItemModal'
import { ChangeFileModal } from '../ChangeFileModal'
import { EditorSection } from '../EditorSection'
import { HidesEditor } from '../HidesEditor'
import { SpringBonesEditor, type SpringBonesModel } from '../SpringBonesEditor'
import { TagsInput } from './TagsInput'
import { type RejectReasonCode } from '@/data/lib/creators/events'
import styles from '../ItemEditorPage.module.css'

export type SpringBonesFormProps = {
  models: SpringBonesModel[]
  params: SpringBoneParamsByName
  activeHash: string
  onActiveHashChange: (hash: string) => void
  onChange: (params: SpringBoneParamsByName) => void
}

type Props = {
  item: Item
  address: string
  /** False in review mode, for viewers without edit rights, and while the collection is publish-locked. */
  editable: boolean
  /** Review mode hides the read-only note: curators know the editor is locked. */
  showReadOnlyNote?: boolean
  /** Why the viewer cannot edit: no manage role, or the collection's publish lock. */
  readOnlyReason?: 'role' | 'linked' | 'lock' | 'reviewing'
  /** The fixes a curator asked for on the collection, shown to the people who can make them. */
  curatorRequest?: { reasons: RejectReasonCode[]; message: string | null }
  /** Owner of a draft collection: the only one who may delete items here. */
  canDelete: boolean
  /** Only the people behind the collection and curators get the source files; anyone else could re-upload them as their own. */
  canDownload: boolean
  draft: ItemDraft
  dispatch: Dispatch<ItemDraftAction>
  isDirty: boolean
  isSaving: boolean
  /** The stored item's size against the limit of the category being edited; null while unknown. */
  size?: ItemSize | null
  springBones: SpringBonesFormProps | null
  /** Linked collections: the contract the item's mapping targets (null when the collection has none); undefined otherwise. */
  linkedContract?: LinkedContract | null
  onSave: () => void
  /** Persists a file change from the modal on its own; rejects when the save fails. */
  onSaveFile: (change: ItemFileChange) => Promise<void>
  /** Persists a new thumbnail on its own, as soon as it is confirmed; reports its own errors. */
  onSaveThumbnail: (thumbnail: Blob) => Promise<void>
  onRevert: () => void
  onDeleted: () => void
  testId?: string
}

export function PropertiesPanel({
  item,
  address,
  editable,
  showReadOnlyNote = true,
  readOnlyReason = 'role',
  curatorRequest,
  canDelete,
  canDownload,
  draft,
  dispatch,
  isDirty,
  isSaving,
  size = null,
  springBones,
  linkedContract,
  onSave,
  onSaveFile,
  onSaveThumbnail,
  onRevert,
  onDeleted,
  testId = 'properties-panel'
}: Props) {
  const { t, rich } = useTranslation()
  const isLinked = linkedContract !== undefined
  const [mappingInvalid, setMappingInvalid] = useState(false)
  // Bumped by Revert: the mapping editor keeps a half-typed draft of its own, which only a remount discards.
  const [revision, setRevision] = useState(0)
  const showToast = useNotifications(state => state.showToast)
  const [changeFile, setChangeFile] = useState<{ lockedBodyShape?: BodyShapeType.MALE | BodyShapeType.FEMALE } | null>(
    null
  )
  const [isVideoOpen, setVideoOpen] = useState(false)
  const [isDeleteOpen, setDeleteOpen] = useState(false)
  const [isDownloading, setDownloading] = useState(false)
  const tagsFieldRef = useRef<HTMLInputElement>(null)
  const deleteItem = useDeleteItem(address)
  const campaign = useCampaign()
  const utilityFlag = useFeatureFlag(FeatureFlag.WEARABLE_UTILITY)
  const vrmFlag = useFeatureFlag(FeatureFlag.VRM_OPTOUT)
  const thumbnailEditor = useThumbnailEditor(patch => onSaveThumbnail(patch.contents[THUMBNAIL_PATH]))

  const isEmote = item.type === ItemType.EMOTE
  const isWearable = item.type === ItemType.WEARABLE
  const isSmart = isSmartWearable(item)
  const disabled = !editable
  const contents = item.contents
  const categories = useMemo(
    () => (isEmote ? getEmoteCategoryOptions() : getWearableCategoryOptions(contents)),
    [isEmote, contents]
  )
  const missingShape = useMemo(
    () => (isWearable && !isSmart ? getMissingBodyShapeType(item) : null),
    [isWearable, isSmart, item]
  )
  const nameInvalid = draft.name.length > 0 && !isValidItemName(draft.name)
  const descriptionInvalid = !isValidItemDescription(draft.description)
  const thumbnailHash = item.contents[item.thumbnail]
  const videoHash = item.contents[VIDEO_PATH]
  const draftVideoUrl = useObjectURL(draft.video)
  const videoUrl = draftVideoUrl ?? (videoHash ? getContentsStorageUrl(videoHash) : null)
  const canEditVideo = editable
  const videoName = draft.video instanceof File ? draft.video.name : VIDEO_PATH
  const [videoDuration, setVideoDuration] = useState<number | null>(null)
  // A new video must not flash the previous one's duration until its metadata loads.
  useEffect(() => setVideoDuration(null), [videoUrl])
  const storedVideo = useItemVideo(videoHash, isVideoOpen && !draft.video)
  const metrics = item.metrics
  const sizeText = useMemo(() => (size ? formatItemSize(size) : null), [size])
  const sizeExceeded = !!size && isOverSizeLimit(size)

  async function download() {
    setDownloading(true)
    try {
      await downloadItem(item)
    } catch (error) {
      console.error('Item download failed:', error)
      showToast(t('item_editor.details.download_error'), { type: 'error' })
    } finally {
      setDownloading(false)
    }
  }

  function confirmDelete() {
    deleteItem.mutate(item, {
      onSuccess: () => {
        setDeleteOpen(false)
        showToast(t('collection_detail_page.item_actions.deleted', { name: item.name }))
        onDeleted()
      },
      onError: () => showToast(t('collection_detail_page.delete_item.error'), { type: 'error' })
    })
  }

  return (
    <div className={styles.panelBody} data-testid={testId} data-editable={editable || undefined}>
      <div className={styles.panelHeader} data-testid={`${testId}-header`}>
        <div className={styles.panelTitleRow}>
          <h2 className={styles.panelTitle} title={item.name}>
            {item.name}
          </h2>
          {isSmart && (
            <Tooltip content={t('collection_detail_page.smart_wearable')} asChild testId={`${testId}-smart-tooltip`}>
              <span className={styles.headerBadge} data-testid={`${testId}-smart`} tabIndex={0}>
                <SmartIcon />
              </span>
            </Tooltip>
          )}
          {isWearable && hasFacialExpressions(contents) && (
            <Tooltip
              content={t('add_items_modal.facial_expressions')}
              asChild
              testId={`${testId}-facial-expressions-tooltip`}
            >
              <span className={styles.headerBadge} data-testid={`${testId}-facial-expressions`} tabIndex={0}>
                <SmileIcon />
              </span>
            </Tooltip>
          )}
        </div>
        {(canDownload || editable) && (
          <ActionsMenu label={t('item_editor.actions.label')} variant="row" tone="dark" testId={`${testId}-actions`}>
            {canDownload && (
              <ActionsMenuItem testId={`${testId}-download`} disabled={isDownloading} onClick={() => void download()}>
                {t('item_editor.details.download')}
              </ActionsMenuItem>
            )}
            {editable && (
              <ActionsMenuItem testId={`${testId}-change-file`} disabled={isSaving} onClick={() => setChangeFile({})}>
                {t('item_editor.details.change_file')}
              </ActionsMenuItem>
            )}
            {editable && missingShape && (
              <ActionsMenuItem
                testId={`${testId}-add-representation`}
                disabled={isSaving}
                onClick={() => setChangeFile({ lockedBodyShape: missingShape })}
              >
                {t('item_editor.details.add_representation', {
                  shape: t(`add_items_modal.body_shape_option.${missingShape}`)
                })}
              </ActionsMenuItem>
            )}
            {canDelete && (
              <>
                <ActionsMenuDivider />
                <ActionsMenuItem testId={`${testId}-delete`} onClick={() => setDeleteOpen(true)}>
                  {t('item_editor.danger.delete')}
                </ActionsMenuItem>
              </>
            )}
          </ActionsMenu>
        )}
      </div>
      {!editable && showReadOnlyNote && (
        <p className={styles.readOnlyNote} data-testid={`${testId}-readonly`}>
          {t(
            readOnlyReason === 'lock'
              ? 'item_editor.read_only_locked'
              : readOnlyReason === 'reviewing'
                ? 'item_editor.read_only_reviewing'
                : readOnlyReason === 'linked'
                  ? 'item_editor.read_only_linked'
                  : 'item_editor.read_only'
          )}
        </p>
      )}
      {curatorRequest && (
        <div className={styles.curatorRequest} data-testid={`${testId}-curator-request`}>
          <strong>{t('item_editor.curator_request.title')}</strong>
          {curatorRequest.reasons.length > 0 && (
            <span>{curatorRequest.reasons.map(code => t(`reject_reason.${code}`)).join(' · ')}</span>
          )}
          {curatorRequest.message && <q>{curatorRequest.message}</q>}
        </div>
      )}

      <EditorSection title={t('item_editor.details.title')} testId={`${testId}-details`}>
        <div className={styles.detailsRow}>
          <button
            className={styles.thumbButton}
            type="button"
            aria-label={t('item_editor.details.edit_thumbnail')}
            disabled={disabled || isSaving}
            data-testid={`${testId}-thumbnail`}
            onClick={() => thumbnailEditor.edit({ kind: 'item', item })}
          >
            <ItemThumbnail src={thumbnailHash ? getContentsStorageUrl(thumbnailHash) : null} rarity={item.rarity} />
            {editable && (
              <span className={styles.thumbOverlay} aria-hidden>
                <CameraIcon />
              </span>
            )}
          </button>
          {(metrics || sizeText) && (
            <ul className={styles.metrics} data-testid={`${testId}-metrics`}>
              {!metrics ? null : isEmote ? (
                <>
                  <li className={styles.metric}>
                    <ClockIcon />
                    {t('add_items_modal.metrics.duration', { count: Math.round((metrics.duration ?? 0) * 10) / 10 })}
                  </li>
                  <li className={styles.metric}>
                    <ImageIcon />
                    {t('add_items_modal.metrics.frames', { count: metrics.frames ?? 0 })}
                  </li>
                  <li className={styles.metric}>
                    <FpsIcon />
                    {t('add_items_modal.metrics.fps', { count: Math.round((metrics.fps ?? 0) * 10) / 10 })}
                  </li>
                </>
              ) : (
                <>
                  <li className={styles.metric}>
                    <TriangleIcon />
                    {t('add_items_modal.metrics.triangles', { count: metrics.triangles ?? 0 })}
                  </li>
                  <li className={styles.metric}>
                    <MaterialIcon />
                    {t('add_items_modal.metrics.materials', { count: metrics.materials ?? 0 })}
                  </li>
                  <li className={styles.metric}>
                    <TextureIcon />
                    {t('add_items_modal.metrics.textures', { count: metrics.textures ?? 0 })}
                  </li>
                </>
              )}
              {sizeText && (
                <li className={styles.metric} data-testid={`${testId}-size`} data-exceeded={sizeExceeded || undefined}>
                  <SizeIcon />
                  {t('item_size.value', sizeText)}
                </li>
              )}
            </ul>
          )}
        </div>
        {sizeExceeded && (
          <span className={styles.errorText} role="alert" data-testid={`${testId}-size-error`}>
            {t('item_size.exceeded')}
          </span>
        )}
        {isSmart && (
          <div className={styles.videoPane} data-testid={`${testId}-video`}>
            <span className={styles.fieldLabel}>
              <span className={styles.labelText}>
                {t('add_items_modal.video.label')}
                <InfoTooltip content={t('item_editor.details.video_hint')} testId={`${testId}-video-hint`} />
              </span>
            </span>
            {videoUrl ? (
              <div className={styles.detailsRow}>
                <button
                  className={styles.thumbButton}
                  type="button"
                  aria-label={t(canEditVideo ? 'add_items_modal.video.edit' : 'item_editor.details.watch_video')}
                  data-testid={`${testId}-video-preview`}
                  onClick={() => setVideoOpen(true)}
                >
                  <video
                    className={styles.videoPoster}
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
                  {/* Storage streams the mp4 without ranges, so the first frame can take a while: mark the wait. */}
                  {videoDuration === null ? (
                    <span className={styles.posterPlaceholder} aria-hidden data-testid={`${testId}-video-loading`}>
                      <VideoIcon />
                    </span>
                  ) : (
                    <span className={styles.playBadge} aria-hidden>
                      <PlayIcon />
                    </span>
                  )}
                  {canEditVideo && (
                    <span className={styles.thumbOverlay} aria-hidden>
                      <VideoIcon />
                    </span>
                  )}
                </button>
                <ul className={styles.metrics} data-testid={`${testId}-video-meta`}>
                  <li className={styles.metric}>
                    <VideoIcon />
                    <span className={styles.metricText} title={videoName}>
                      {videoName}
                    </span>
                  </li>
                  {videoDuration !== null && (
                    <li className={styles.metric}>
                      <ClockIcon />
                      {t('add_items_modal.video.duration', { seconds: Math.round(videoDuration) })}
                    </li>
                  )}
                  {draft.video && (
                    <li className={styles.metric}>
                      <SizeIcon />
                      {t('add_items_modal.video.size', { size: toMB(draft.video.size) })}
                    </li>
                  )}
                </ul>
              </div>
            ) : canEditVideo ? (
              <VideoDropzone
                compact
                testId={`${testId}-video-dropzone`}
                onPick={video => dispatch({ type: 'setVideo', video })}
              />
            ) : null}
            {isMissingSmartWearableVideo(item) && !draft.video && (
              <div className={styles.warning} data-testid={`${testId}-video-missing`}>
                <WarningIcon fontSize="small" />
                {t('item_editor.details.video_required')}
              </div>
            )}
          </div>
        )}
      </EditorSection>

      <EditorSection title={t('item_editor.basics.title')} testId={`${testId}-basics`}>
        <label className={styles.field}>
          <span className={styles.fieldLabel}>
            {t('item_editor.basics.name')}
            <span className={styles.charCount}>
              {t('add_items_modal.char_count', { count: draft.name.length, max: ITEM_NAME_MAX_LENGTH })}
            </span>
          </span>
          <input
            className={styles.textInput}
            value={draft.name}
            maxLength={ITEM_NAME_MAX_LENGTH}
            disabled={disabled}
            data-invalid={nameInvalid || undefined}
            data-testid={`${testId}-name`}
            onChange={event => dispatch({ type: 'setText', field: 'name', value: event.target.value })}
          />
          {nameInvalid && (
            <span className={styles.errorText} data-testid={`${testId}-name-error`}>
              {t('add_items_modal.invalid_name')}
            </span>
          )}
        </label>
        <label className={styles.field}>
          <span className={styles.fieldLabel}>
            {t('item_editor.basics.description')}
            <span className={styles.charCount}>
              {t('add_items_modal.char_count', { count: draft.description.length, max: ITEM_DESCRIPTION_MAX_LENGTH })}
            </span>
          </span>
          <textarea
            className={styles.textArea}
            value={draft.description}
            maxLength={ITEM_DESCRIPTION_MAX_LENGTH}
            disabled={disabled}
            data-invalid={descriptionInvalid || undefined}
            data-testid={`${testId}-description`}
            onChange={event => dispatch({ type: 'setText', field: 'description', value: event.target.value })}
          />
          {descriptionInvalid && (
            <span className={styles.errorText} data-testid={`${testId}-description-error`}>
              {t('item_editor.basics.invalid_description')}
            </span>
          )}
        </label>
        {utilityFlag.enabled && (
          <label className={styles.field}>
            <span className={styles.fieldLabel}>
              <span className={styles.labelText}>
                {t('item_editor.basics.utility')}{' '}
                <InfoTooltip content={t('item_editor.basics.utility_hint')} testId={`${testId}-utility-hint`} />
              </span>
              <span className={styles.charCount}>
                {t('add_items_modal.char_count', { count: draft.utility.length, max: ITEM_UTILITY_MAX_LENGTH })}
              </span>
            </span>
            <input
              className={styles.textInput}
              value={draft.utility}
              maxLength={ITEM_UTILITY_MAX_LENGTH}
              disabled={disabled}
              data-testid={`${testId}-utility`}
              onChange={event => dispatch({ type: 'setText', field: 'utility', value: event.target.value })}
            />
          </label>
        )}
        <div className={styles.field}>
          <span className={styles.fieldLabel}>{t('item_editor.basics.category')}</span>
          <CategorySelect
            value={draft.category}
            categories={categories}
            disabled={disabled}
            tone="dark"
            testId={`${testId}-category`}
            onChange={category => dispatch({ type: 'setCategory', category })}
          />
        </div>
        {!isLinked && (
          <div className={styles.field}>
            <span className={styles.fieldLabel}>
              <span className={styles.labelText}>
                {t('item_editor.basics.rarity')}{' '}
                <InfoTooltip content={t('add_items_modal.rarity_tooltip')} testId={`${testId}-rarity-hint`} />
              </span>
            </span>
            <RaritySelect
              value={draft.rarity ?? ''}
              disabled={disabled || item.isPublished}
              tone="dark"
              testId={`${testId}-rarity`}
              onChange={rarity => dispatch({ type: 'setRarity', rarity })}
            />
          </div>
        )}
      </EditorSection>

      {isLinked && linkedContract && (
        <EditorSection title={t('mapping_editor.label')} testId={`${testId}-mapping`}>
          <MappingEditor
            key={`${item.id}:${revision}`}
            value={getItemMapping(draft.mappings, linkedContract)}
            disabled={disabled}
            tone="dark"
            testId={`${testId}-mapping-editor`}
            onChange={mapping => {
              setMappingInvalid(mapping === null)
              if (mapping) dispatch({ type: 'setMappings', mappings: buildItemMappings(mapping, linkedContract) })
            }}
          />
          <p className={styles.sectionNote}>{t('add_items_modal.mapping_hint')}</p>
        </EditorSection>
      )}

      {isWearable && (
        <EditorSection title={t('item_editor.overrides.title')} testId={`${testId}-overrides`}>
          <HidesEditor
            contents={contents}
            category={draft.category}
            hides={draft.hides}
            disabled={disabled}
            testId={`${testId}-hides`}
            onChange={hides => dispatch({ type: 'setHides', hides })}
          />
        </EditorSection>
      )}

      {isEmote && (
        <EditorSection title={t('item_editor.animation.title')} testId={`${testId}-animation`}>
          <div className={styles.segmented} role="radiogroup" aria-label={t('add_items_modal.play_mode')}>
            {[EmotePlayMode.LOOP, EmotePlayMode.SIMPLE].map(mode => {
              const selected = draft.loop === (mode === EmotePlayMode.LOOP)
              return (
                <button
                  className={styles.segmentButton}
                  key={mode}
                  type="button"
                  role="radio"
                  aria-checked={selected}
                  disabled={disabled}
                  data-selected={selected || undefined}
                  data-testid={`${testId}-play-mode-${mode}`}
                  onClick={() => dispatch({ type: 'setLoop', loop: mode === EmotePlayMode.LOOP })}
                >
                  {mode === EmotePlayMode.LOOP ? <LoopIcon /> : <PlayOnceIcon />}
                  {t(`add_items_modal.play_mode_option.${mode}`)}
                </button>
              )
            })}
          </div>
        </EditorSection>
      )}

      {springBones && springBones.models.length > 0 && (
        <EditorSection title={t('item_editor.spring_bones.title')} testId={`${testId}-spring-bones`}>
          <SpringBonesEditor
            models={springBones.models}
            activeHash={springBones.activeHash}
            onActiveHashChange={springBones.onActiveHashChange}
            params={springBones.params}
            onChange={springBones.onChange}
            disabled={disabled}
          />
        </EditorSection>
      )}

      {isSmart && (
        <EditorSection title={t('required_permissions.title')} testId={`${testId}-permissions`}>
          <RequiredPermissions
            permissions={item.data.requiredPermissions ?? []}
            testId={`${testId}-required-permissions`}
          />
        </EditorSection>
      )}

      <EditorSection title={t('item_editor.tags.title')} testId={`${testId}-tags`}>
        <TagsInput
          tags={draft.tags}
          disabled={disabled}
          inputRef={tagsFieldRef}
          onChange={tags => dispatch({ type: 'setTags', tags })}
        />
        {editable && campaign && (
          <p className={styles.sectionNote} data-testid={`${testId}-campaign-hint`}>
            {rich('item_editor.tags.campaign_hint', {
              tag: campaign.mainTag,
              name: campaign.name,
              hl: chunks =>
                draft.tags.includes(campaign.mainTag) ? (
                  <strong>{chunks}</strong>
                ) : (
                  <Tooltip content={t('item_editor.tags.campaign_add')} asChild describeChild>
                    <button
                      className={styles.campaignTag}
                      type="button"
                      data-testid={`${testId}-campaign-tag`}
                      onClick={() => {
                        dispatch({ type: 'setTags', tags: [...draft.tags, campaign.mainTag] })
                        // The button unmounts once the tag is on the draft, so keyboard focus would fall to the body.
                        tagsFieldRef.current?.focus()
                      }}
                    >
                      {chunks}
                    </button>
                  </Tooltip>
                )
            })}
          </p>
        )}
      </EditorSection>

      {isWearable && (
        <EditorSection title={t('item_editor.options.title')} testId={`${testId}-options`}>
          {vrmFlag.enabled && (
            <div className={styles.toggle}>
              <span className={styles.labelText}>
                {t('item_editor.options.vrm_export')}
                <InfoTooltip content={t('item_editor.options.vrm_export_hint')} testId={`${testId}-vrm-hint`} />
              </span>
              <Switch
                checked={!draft.blockVrmExport}
                disabled={disabled}
                label={t('item_editor.options.vrm_export')}
                testId={`${testId}-vrm-export`}
                onChange={checked => dispatch({ type: 'setFlag', field: 'blockVrmExport', value: !checked })}
              />
            </div>
          )}
          <div className={styles.toggle}>
            <span className={styles.labelText}>
              {t('item_editor.options.outline')}
              <InfoTooltip content={t('item_editor.options.outline_hint')} testId={`${testId}-outline-hint`} />
            </span>
            <Switch
              checked={draft.outlineCompatible}
              disabled={disabled}
              label={t('item_editor.options.outline')}
              testId={`${testId}-outline`}
              onChange={checked => dispatch({ type: 'setFlag', field: 'outlineCompatible', value: checked })}
            />
          </div>
        </EditorSection>
      )}

      <div className={styles.intercomClearance} aria-hidden />
      {/* An invalid mapping draft never reaches the item, so it doesn't make it dirty: Revert must still undo it. */}
      {editable && (isDirty || mappingInvalid) && (
        <div className={styles.footer} data-testid={`${testId}-footer`}>
          <Button
            type="button"
            variant="secondary"
            size="sm"
            disabled={!(isDirty || mappingInvalid) || isSaving}
            data-testid={`${testId}-revert`}
            onClick={() => {
              setMappingInvalid(false)
              setRevision(current => current + 1)
              onRevert()
            }}
          >
            {t('item_editor.revert')}
          </Button>
          <Button
            type="button"
            variant="primary"
            size="sm"
            disabled={!isDirty || nameInvalid || descriptionInvalid || mappingInvalid || draft.name.trim() === ''}
            loading={isSaving}
            data-testid={`${testId}-save`}
            onClick={onSave}
          >
            {t('item_editor.save')}
          </Button>
        </div>
      )}

      {thumbnailEditor.modal}
      {isVideoOpen && (
        <VideoModal
          video={draft.video ?? storedVideo.data ?? null}
          viewOnly={!canEditVideo}
          onChange={video => dispatch({ type: 'setVideo', video })}
          onClose={() => setVideoOpen(false)}
        />
      )}
      {changeFile && (
        <ChangeFileModal
          item={item}
          lockedBodyShape={changeFile.lockedBodyShape}
          linked={isLinked}
          onSave={onSaveFile}
          onClose={() => setChangeFile(null)}
        />
      )}
      {isDeleteOpen && (
        <DeleteItemModal
          item={item}
          isDeleting={deleteItem.isPending}
          onCancel={() => {
            setDeleteOpen(false)
            deleteItem.reset()
          }}
          onConfirm={confirmDelete}
        />
      )}
    </div>
  )
}
