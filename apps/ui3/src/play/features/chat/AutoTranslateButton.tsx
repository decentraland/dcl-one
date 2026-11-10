// Switches a conversation's auto-translation on or off, on its title bar.

import { ControlButton, Tooltip, Translate } from '@hud/design'
import { autoTranslates, setAutoTranslate, useTranslationPrefs } from './translation'
import styles from '@hud/features/chat/Chat.module.css'

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
