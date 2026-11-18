import { SwapVertIcon } from '../Icons'
import { useTranslation } from '@/data/lib/creators/intl'
import { cx } from '@/data/lib/creators/cx'
import { usePreviewController } from './usePreviewController'
import styles from './PreviewControls.module.css'
import type { TranslationControlsProps } from './types'

/** A vertical slider that moves a preview's camera up and down. */
export function TranslationControls({ previewId, controller: shared, className, vertical, verticalPosition = 'right' }: TranslationControlsProps) {
  const { t } = useTranslation()
  const { controller } = usePreviewController(previewId, shared)
  return (
    <div className={cx(styles.pan, className)} data-preview-control="pan" data-side={verticalPosition}>
      {vertical && (
        <div className={styles.panTrack}>
          <SwapVertIcon />
          <input
            type="range"
            className={styles.panSlider}
            aria-label={t('preview_controls.move')}
            aria-orientation="vertical"
            defaultValue={0}
            min={-2}
            max={2}
            step={0.1}
            onChange={event => void controller?.scene.panCamera({ y: Number(event.target.value) * -1 })}
          />
        </div>
      )}
    </div>
  )
}
