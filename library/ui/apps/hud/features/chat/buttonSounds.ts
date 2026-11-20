// The sounds of the chat's buttons, handled once on the chat's root rather than on each button:
// a press when one is clicked, a highlight when the pointer comes onto one. A button says
// otherwise with `data-ui-sound`: "soft" for the softer press of a close or a toggle, "press" for a
// press with no highlight, as the buttons of a dense grid or of every chat line have. A disabled
// button, `aria-disabled` included, is silent.
import type { UiSound } from '@explorer-core/engine/protocol'

const SOUND_ATTR = 'data-ui-sound'

function buttonAt(target: EventTarget | null): HTMLButtonElement | null {
  const button = target instanceof Element ? target.closest('button') : null
  return button != null && !button.disabled && button.getAttribute('aria-disabled') !== 'true' ? button : null
}

export function buttonSoundHandlers(play: (sound: UiSound) => void): {
  onClickCapture: (e: React.MouseEvent) => void
  onPointerOver: (e: React.PointerEvent) => void
} {
  return {
    onClickCapture: (e) => {
      const button = buttonAt(e.target)
      if (button != null) play(button.getAttribute(SOUND_ATTR) === 'soft' ? 'buttonClose' : 'buttonPress')
    },
    onPointerOver: (e) => {
      const button = buttonAt(e.target)
      if (button == null || button.getAttribute(SOUND_ATTR) === 'press') return
      // moving between a button's own children is not coming onto it again
      if (e.relatedTarget instanceof Node && button.contains(e.relatedTarget)) return
      play('buttonHover')
    }
  }
}
