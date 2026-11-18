import { useRef, useState, type DragEvent, type ReactNode } from 'react'
import { VideocamOutlinedIcon as VideoIcon } from '../Icons'
import { useTranslation } from '@/data/lib/creators/intl'
import { ItemFileError, MAX_VIDEO_FILE_SIZE, VIDEO_EXTENSIONS, toMB, validateVideoFile } from '@/data/lib/creators/itemFiles'
import { loadVideoMetadata } from '@/data/lib/creators/media'
import styles from './VideoDropzone.module.css'

/**
 * Validates a picked preview video (mp4, ≤ 250MB, decodable) and returns it; throws ItemFileError
 * with the `add_items_modal.file_error.*` key to show.
 */
async function pickVideoFile(file: File): Promise<File> {
  validateVideoFile(file)
  try {
    // Only decodability matters here; the duration (null on a stalled decode) is read by the poster itself.
    await loadVideoMetadata(file)
  } catch {
    throw new ItemFileError('invalid_video')
  }
  return file
}

type Props = {
  /** Called with a validated file; the caller stores it. */
  onPick: (file: File) => void
  /** Rendered instead of the empty CTA when a video exists; the zone still accepts a replacement drop. */
  children?: ReactNode
  /** Shorter zone for use as a form field. */
  compact?: boolean
  testId?: string
}

/** Drag-and-drop / browse target for a smart wearable preview video, with validation and inline errors. */
export function VideoDropzone({ onPick, children, compact = false, testId = 'video-dropzone' }: Props) {
  const { t } = useTranslation()
  const inputRef = useRef<HTMLInputElement>(null)
  const [isDragging, setDragging] = useState(false)
  const [isLoading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  function handleFile(file: File | undefined) {
    if (!file) return
    setDragging(false)
    setError(null)
    setLoading(true)
    void (async () => {
      try {
        onPick(await pickVideoFile(file))
      } catch (err) {
        const key = err instanceof ItemFileError ? err.messageKey : 'invalid_video'
        const params = err instanceof ItemFileError ? err.messageParams : undefined
        setError(t(`add_items_modal.file_error.${key}`, params))
      } finally {
        setLoading(false)
      }
    })()
  }

  function handleDrop(event: DragEvent) {
    event.preventDefault()
    handleFile(event.dataTransfer.files[0])
  }

  return (
    <>
      <div
        className={styles.zone}
        data-testid={testId}
        data-empty={!children || undefined}
        data-compact={compact || undefined}
        data-dragging={isDragging || undefined}
        data-busy={isLoading || undefined}
        onDragOver={event => {
          event.preventDefault()
          setDragging(true)
        }}
        onDragLeave={() => setDragging(false)}
        onDrop={handleDrop}
      >
        {isLoading && <span className={styles.spinner} aria-hidden data-spinner data-testid={`${testId}-loading`} />}
        {children ?? (
          <>
            <VideoIcon />
            <span className={styles.dropText}>
              <span data-desktop>{t('video_modal.drop_cta')}</span>
              <button
                className={styles.browseLink}
                type="button"
                data-testid={`${testId}-browse`}
                disabled={isLoading}
                onClick={() => inputRef.current?.click()}
              >
                {t('video_modal.browse')}
              </button>
            </span>
            <span className={styles.hint}>
              {t('video_modal.hint', {
                extensions: VIDEO_EXTENSIONS.join(', ').toUpperCase(),
                size: toMB(MAX_VIDEO_FILE_SIZE)
              })}
            </span>
          </>
        )}
        <input
          ref={inputRef}
          type="file"
          accept="video/mp4"
          hidden
          data-testid={`${testId}-input`}
          onChange={event => {
            handleFile(event.target.files?.[0])
            event.target.value = ''
          }}
        />
      </div>
      {error && (
        <p className={styles.errorText} data-testid={`${testId}-error`}>
          {error}
        </p>
      )}
    </>
  )
}
