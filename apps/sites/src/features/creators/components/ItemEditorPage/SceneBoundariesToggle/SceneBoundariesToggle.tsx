import { CylinderIcon } from '../../Icons'
import { Tooltip } from '../../Tooltip'
import { useTranslation } from '@/data/lib/creators/intl'
import { track } from '@/data/lib/creators/analytics'
import { useAvatarPreview } from '../../../store/avatarPreview'
import styles from './SceneBoundariesToggle.module.css'

type Props = {
  trigger: 'editor' | 'live_preview'
  testId?: string
}

/** Shows or hides the scene boundaries (the cylinder an item has to fit in) around the avatar. */
export function SceneBoundariesToggle({ trigger, testId = 'scene-boundaries-toggle' }: Props) {
  const { t } = useTranslation()
  const visible = useAvatarPreview(state => state.showSceneBoundaries)
  const toggle = useAvatarPreview(state => state.toggleSceneBoundaries)
  const label = t(visible ? 'item_editor.scene_boundaries.hide' : 'item_editor.scene_boundaries.show')

  function onClick() {
    track('Toggle scene boundaries', { visible: !visible, trigger })
    toggle()
  }

  return (
    <Tooltip content={label} asChild testId={`${testId}-tooltip`}>
      <button
        className={styles.button}
        type="button"
        aria-label={label}
        aria-pressed={visible}
        data-active={visible || undefined}
        data-testid={testId}
        onClick={onClick}
      >
        <CylinderIcon />
      </button>
    </Tooltip>
  )
}
