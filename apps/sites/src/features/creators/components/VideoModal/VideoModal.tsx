import { useState } from 'react'
import { Button } from '../Button'
import { Modal } from '../Modal'
import { useObjectURL } from '../../hooks/useObjectURL'
import { useTranslation } from '@/data/lib/creators/intl'
import { VideoDropzone } from './VideoDropzone'
import styles from './VideoModal.module.css'

type Props = {
  /** The current preview video, or null when the item has none yet. */
  video: Blob | null
  /** Watch-only: the video is frozen (a published, approved item) or the viewer has no edit rights. */
  viewOnly?: boolean
  /** A picked file replaces the video. */
  onChange: (video: File) => void
  onClose: () => void
}

/**
 * Smart wearable preview video editor, stacked over the add-items modal (legacy UploadVideoStep +
 * EditVideoModal): drop or browse an MP4, watch it, replace it. Holds no draft state.
 */
export function VideoModal({ video, viewOnly = false, onChange, onClose }: Props) {
  const { t } = useTranslation()
  const [isReplacing, setReplacing] = useState(false)
  const videoUrl = useObjectURL(video)
  const showPlayer = !!video && (viewOnly || !isReplacing)

  return (
    <Modal title={t('video_modal.title')} onClose={onClose} testId="video-modal">
      <div className={styles.wrap}>
        {showPlayer && videoUrl ? (
          <>
            <video
              className={styles.player}
              src={videoUrl}
              controls
              controlsList="nodownload noremoteplayback"
              disablePictureInPicture
              playsInline
              data-testid="video-modal-player"
            />
            <div className={styles.actions}>
              {!viewOnly && (
                <Button
                  type="button"
                  variant="secondary"
                  data-testid="video-modal-replace"
                  onClick={() => setReplacing(true)}
                >
                  {t('video_modal.replace')}
                </Button>
              )}
              <Button type="button" variant="primary" data-testid="video-modal-done" onClick={onClose}>
                {t('video_modal.done')}
              </Button>
            </div>
          </>
        ) : viewOnly ? (
          // The stored video is still being fetched: a watcher never gets the dropzone.
          <div
            className={styles.loading}
            aria-busy="true"
            aria-label={t('video_modal.loading')}
            data-testid="video-modal-loading"
          >
            <span className="spinner" aria-hidden />
          </div>
        ) : (
          <>
            <p className={styles.description}>{t('video_modal.description')}</p>
            <VideoDropzone
              testId="video-modal-dropzone"
              onPick={file => {
                onChange(file)
                setReplacing(false)
              }}
            />
            {isReplacing && (
              <div className={styles.actions}>
                <Button
                  type="button"
                  variant="secondary"
                  data-testid="video-modal-cancel"
                  onClick={() => setReplacing(false)}
                >
                  {t('video_modal.cancel')}
                </Button>
              </div>
            )}
          </>
        )}
      </div>
    </Modal>
  )
}
