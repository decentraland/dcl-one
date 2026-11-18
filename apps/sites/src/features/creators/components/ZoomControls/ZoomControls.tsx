import { AddIcon as ZoomInIcon, RemoveIcon as ZoomOutIcon } from '~/components/Icons'
import { useTranslation } from '~/intl'
import { cx } from '~/lib/cx'
import styles from './ZoomControls.module.css'
import { type WearablePreviewController } from '~/components/WearablePreview'

/**
 * Inertial camera-radius nudge per click. Babylon's inertia (0.9) carries it ~10× further, so this moves
 * the camera ~0.25 units: about five clicks from the start framing to either end of the range.
 */
export const ZOOM_STEP = 0.075

type Props = {
  controller: WearablePreviewController
  /** Layout-only override (offsets); the look is fixed. */
  className?: string
  testId?: string
}

/**
 * Zoom in / out over the preview: `scene.changeZoom` is a relative camera nudge on both renderers, and
 * Unity hides its own in-canvas buttons when the page frames the camera (`zoom` / `wheelZoom`).
 */
export function ZoomControls({ controller, className, testId = 'zoom-controls' }: Props) {
  const { t } = useTranslation()
  const step = (delta: number) => controller.scene.changeZoom(delta).catch(() => undefined)
  return (
    <div className={cx(styles.wrap, className)} data-testid={testId}>
      <button
        className={styles.zoomButton}
        type="button"
        aria-label={t('zoom_controls.in')}
        data-testid={`${testId}-in`}
        onClick={() => void step(ZOOM_STEP)}
      >
        <ZoomInIcon fontSize="small" />
      </button>
      <button
        className={styles.zoomButton}
        type="button"
        aria-label={t('zoom_controls.out')}
        data-testid={`${testId}-out`}
        onClick={() => void step(-ZOOM_STEP)}
      >
        <ZoomOutIcon fontSize="small" />
      </button>
    </div>
  )
}
