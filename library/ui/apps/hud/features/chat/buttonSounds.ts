// The sounds of the chat's buttons, handled once on the chat's root rather than on each button:
// a press when one is clicked, a highlight when the pointer comes onto one. A button says
// otherwise with `data-ui-sound`: "close" for the softer close press, "press" for a press with no
// highlight.
import type { UiSound } from '@explorer-core/engine/protocol'

export const SOUND_ATTR = 'data-ui-sound'

function buttonAt(target: EventTarget | null): HTMLButtonElement | null {
  const button = target instanceof Element ? target.closest('button') : null
  return button != null && !button.disabled ? button : null
}

export function buttonSoundHandlers(play: (sound: UiSound) => void): {
  onClickCapture: (e: React.MouseEvent) => void
  onPointerOver: (e: React.PointerEvent) => void
} {
  return {
    onClickCapture: (e) => {
      const button = buttonAt(e.target)
      if (button != null) play(button.getAttribute(SOUND_ATTR) === 'close' ? 'buttonClose' : 'buttonPress')
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
