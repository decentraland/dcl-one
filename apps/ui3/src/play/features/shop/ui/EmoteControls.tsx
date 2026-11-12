// The design's EMOTE controls, over the avatar while an emote plays: STOP EMOTE and the emote's sound.

import { MaskIcon } from '@hud/design'
import stopIcon from '../../../assets/shop/stop.svg'
import soundIcon from '../../../assets/shop/sound-on.svg'
import styles from './EmoteControls.module.css'

export function EmoteControls({
  onStop,
  onToggleSound,
  className
}: {
  onStop: () => void
  /** Mutes or unmutes the emote; without it the button is drawn as designed but off. */
  onToggleSound?: () => void
  className?: string
}): React.JSX.Element {
  return (
    <div className={`${styles.controls} ${className ?? ''}`.trim()}>
      <button type="button" className={styles.stop} onClick={onStop}>
        <MaskIcon src={stopIcon} size={24} />
        STOP EMOTE
      </button>
      <button
        type="button"
        className={styles.sound}
        aria-label="Emote sound"
        disabled={onToggleSound == null}
        title={onToggleSound == null ? 'Emote sound is coming soon' : undefined}
        onClick={onToggleSound}
      >
        <MaskIcon src={soundIcon} size={24} />
      </button>
    </div>
  )
}
