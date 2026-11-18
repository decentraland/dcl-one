import { useEffect, useMemo, useRef, useState } from 'react'
import { PreviewProjection, PreviewRenderer } from '@dcl/schemas'
import { Button } from '@/features/creators/components/Button'
import { Modal } from '@/features/creators/components/Modal'
import {
  EmoteControls,
  WearablePreview,
  TranslationControls,
  createWearablePreviewController,
  type WearablePreviewController
} from '@/features/creators/components/WearablePreview'
import { ZoomControls } from '@/features/creators/components/ZoomControls'
import { usePreviewRenderer } from '@/features/creators/hooks/usePreviewRenderer'
import { useTranslation } from '@/data/lib/creators/intl'
import { BuilderServerError, COLLECTION_LOCKED_STATUS } from '@/data/lib/creators/builder'
import { MAX_THUMBNAIL_FILE_SIZE, THUMBNAIL_PATH, toMB } from '@/data/lib/creators/itemFiles'
import { isItemTooBigError } from '@/data/lib/creators/itemSize'
import { ItemType } from '@/data/lib/creators/items'
import { ImageType, dataURLToBlob, getImageType, resizeImage, blobToDataURL } from '@/data/lib/creators/media'
import { retryWhilePreviewReloads, toEmoteWithBlobs, toWearableWithBlobs } from '@/data/lib/creators/preview'
import styles from './ThumbnailModal.module.css'

const PREVIEW_ID = 'thumbnail-editor'
const THUMBNAIL_SIZE = 1024

export type ThumbnailPatch = {
  /** Data URL of the new thumbnail; also stored as contents[THUMBNAIL_PATH]. */
  thumbnail: string
  /** The item files plus the new thumbnail; only the thumbnail when it was uploaded before the files loaded. */
  contents: Record<string, Blob>
  isAutoThumbnail: boolean
}

export class ThumbnailFormatError extends Error {}
export class ThumbnailTooBigError extends Error {}

/** The i18n key (with params) for a failed capture, upload or save. */
export function getThumbnailErrorMessage(err: unknown): { key: string; params?: Record<string, string | number> } {
  if (err instanceof ThumbnailFormatError) return { key: 'thumbnail_modal.wrong_format' }
  if (err instanceof ThumbnailTooBigError)
    return { key: 'thumbnail_modal.too_big', params: { size: toMB(MAX_THUMBNAIL_FILE_SIZE) } }
  if (isItemTooBigError(err)) return { key: 'add_items_modal.file_error.size_exceeded', params: err.messageParams }
  if (err instanceof BuilderServerError && err.status === COLLECTION_LOCKED_STATUS)
    return { key: 'item_editor.save_error_locked' }
  return { key: 'thumbnail_modal.capture_failed' }
}

export function thumbnailPatchFromDataURL(contents: Record<string, Blob>, thumbnail: string): ThumbnailPatch {
  const thumbnailBlob = dataURLToBlob(thumbnail)
  if (!thumbnailBlob) throw new Error('Could not decode the thumbnail')
  if (thumbnailBlob.size > MAX_THUMBNAIL_FILE_SIZE) throw new ThumbnailTooBigError()
  return {
    thumbnail,
    contents: { ...contents, [THUMBNAIL_PATH]: thumbnailBlob },
    isAutoThumbnail: false
  }
}

/** Builds the patch for a user-picked PNG, resized to the thumbnail size. Throws ThumbnailFormatError for non-PNGs. */
export async function thumbnailPatchFromFile(contents: Record<string, Blob>, file: File): Promise<ThumbnailPatch> {
  if ((await getImageType(file)) !== ImageType.PNG) throw new ThumbnailFormatError()
  const resized = await resizeImage(file, THUMBNAIL_SIZE, THUMBNAIL_SIZE)
  return thumbnailPatchFromDataURL(contents, await blobToDataURL(resized))
}

type Props = {
  type: ItemType | null
  /** The item files to render; null while they are still being fetched. */
  contents: Record<string, Blob> | null
  /** Shown in place of the preview when the files could not be fetched. */
  loadError?: boolean
  /** A rejection keeps the modal open with the error. */
  onSave: (patch: ThumbnailPatch) => void | Promise<void>
  onClose: () => void
}

/**
 * Interactive thumbnail editor: pose/zoom the item in a live preview and capture, or
 * (wearables only) upload a PNG instead. Image-only wearables never reach this modal.
 */
export function ThumbnailModal({ type, contents, loadError = false, onSave, onClose }: Props) {
  const { t } = useTranslation()
  const fileInputRef = useRef<HTMLInputElement>(null)
  const [controller, setController] = useState<WearablePreviewController | null>(null)
  const [isLoaded, setLoaded] = useState(false)
  const isReady = controller !== null && isLoaded
  // Which button started the save, so its spinner shows while the caller's save settles.
  const [saving, setSaving] = useState<'capture' | 'upload' | null>(null)
  const isSaving = saving !== null
  const [error, setError] = useState<string | null>(null)

  const renderer = usePreviewRenderer()
  const isEmote = type === ItemType.EMOTE
  // A new blob reference remounts the preview iframe, so keep it stable across local state changes.
  const blob = useMemo(
    () => (contents ? (isEmote ? toEmoteWithBlobs(contents) : toWearableWithBlobs(contents)) : null),
    [isEmote, contents]
  )

  // One controller, created as soon as the iframe exists and shared with the preview controls: every
  // createController call drops the requests in flight, and a late one (in onLoad, or a control
  // mounting after it) swallows the length EmoteControls asks for on the autoplay PLAY, leaving its
  // play/pause stuck until the emote ends.
  const isMounted = blob !== null && renderer !== undefined
  useEffect(() => {
    setController(isMounted ? createWearablePreviewController(PREVIEW_ID) : null)
    if (!isMounted) setLoaded(false)
  }, [isMounted])

  function errorText(err: unknown): string {
    const { key, params } = getThumbnailErrorMessage(err)
    return t(key, params)
  }

  function handleCapture() {
    if (!controller) return
    setSaving('capture')
    setError(null)
    void (async () => {
      try {
        if (isEmote) await controller.emote.pause()
        const screenshot = await retryWhilePreviewReloads(() =>
          controller.scene.getScreenshot(THUMBNAIL_SIZE, THUMBNAIL_SIZE)
        )
        await onSave(thumbnailPatchFromDataURL(contents!, screenshot))
      } catch (err) {
        setError(errorText(err))
        setSaving(null)
      }
    })()
  }

  function handleUpload(files: FileList | null) {
    const file = files?.[0]
    if (!file) return
    setSaving('upload')
    setError(null)
    void (async () => {
      try {
        // A picked file needs neither the item files nor the preview.
        await onSave(await thumbnailPatchFromFile(contents ?? {}, file))
      } catch (err) {
        setError(errorText(err))
        setSaving(null)
      }
    })()
  }

  return (
    <Modal
      title={t('thumbnail_modal.title')}
      onClose={onClose}
      closeDisabled={isSaving}
      size="wide"
      testId="thumbnail-modal"
    >
      <div className={styles.wrap}>
        <div className={styles.previewArea} data-testid="thumbnail-preview">
          {!isMounted && !loadError && <span className={styles.spinner} aria-hidden data-testid="thumbnail-loading" />}
          {loadError && (
            <p className={styles.loadError} data-testid="thumbnail-load-error">
              {t('thumbnail_modal.load_failed')}
            </p>
          )}
          {isMounted && (
            <WearablePreview
              id={PREVIEW_ID}
              blob={blob}
              unity={renderer === PreviewRenderer.UNITY}
              disableBackground
              disableAutoRotate
              projection={PreviewProjection.PERSPECTIVE}
              zoom={50}
              wheelZoom={2}
              {...(isEmote
                ? {
                    profile: 'default',
                    disableFace: true,
                    disableDefaultWearables: true,
                    disableDefaultEmotes: true,
                    skin: '000000'
                  }
                : {})}
              onLoad={() => setLoaded(true)}
            />
          )}
          <div className={styles.frame} aria-hidden data-testid="thumbnail-frame" />
          {isReady && (
            <>
              <ZoomControls className={styles.zoomControls} controller={controller} />
              <TranslationControls
                className={styles.translationControls}
                vertical
                verticalPosition="left"
                previewId={PREVIEW_ID}
                controller={controller}
              />
            </>
          )}
        </div>
        {/* Mounted before the iframe loads so the PLAY event fired at load time is delivered. */}
        {isEmote && controller && (
          <div className={styles.emoteBar} data-testid="thumbnail-emote-controls" data-ready={isReady}>
            <EmoteControls className={styles.emoteControls} previewId={PREVIEW_ID} controller={controller} />
          </div>
        )}
        {error && (
          <p className={styles.errorText} data-testid="thumbnail-error">
            {error}
          </p>
        )}
        <div className={styles.actions}>
          {!isEmote && (
            <>
              <input
                ref={fileInputRef}
                type="file"
                accept="image/png"
                hidden
                data-testid="thumbnail-file-input"
                onChange={event => {
                  handleUpload(event.target.files)
                  event.target.value = ''
                }}
              />
              <Button
                type="button"
                variant="secondary"
                data-testid="thumbnail-upload"
                disabled={isSaving}
                loading={saving === 'upload'}
                onClick={() => fileInputRef.current?.click()}
              >
                {t('thumbnail_modal.upload_picture')}
              </Button>
            </>
          )}
          <Button
            type="button"
            variant="primary"
            data-testid="thumbnail-capture"
            disabled={!isReady || isSaving}
            loading={saving === 'capture'}
            onClick={handleCapture}
          >
            {t('thumbnail_modal.capture')}
          </Button>
        </div>
      </div>
    </Modal>
  )
}
