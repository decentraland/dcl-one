// Switches a conversation's auto-translation on or off, on its title bar.

import { ControlButton, Tooltip, Translate } from '../../design'
import { autoTranslates, setAutoTranslate } from '@explorer-core/features/chat/translation'
import { useTranslationPrefs } from './useTranslation'
import styles from './Chat.module.css'

export function AutoTranslateButton({ channel }: { channel: string }): React.JSX.Element {
  const on = autoTranslates(useTranslationPrefs(), channel)
  return (
    <Tooltip label={on ? 'Auto-translate: on' : 'Auto-translate: off'} side="bottom">
      <ControlButton
        variant="faint"
        active={on}
        className={on ? undefined : styles.autoTranslateOff}
        aria-label="Auto-translate"
        onClick={() => setAutoTranslate(channel, !on)}
      >
        <Translate size={18} />
      </ControlButton>
    </Tooltip>
  )
}
