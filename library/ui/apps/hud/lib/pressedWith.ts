// How a control was pressed, and what the HUD does with its focus afterwards. The HUD sits over the game: a key
// press after a click is the game's, and Chrome would ring a control a click left focused (and Space or Enter would
// press it again). A pointer press therefore keeps no focus; a keyboard press keeps it and moves it into what opens.

export type PressedWith = 'pointer' | 'keyboard'

/** A pointer click counts its clicks (`detail` 1 or more); Enter or Space on a focused control, and `click()`, none. */
export function pressedWith(event: { detail: number }): PressedWith {
  return event.detail === 0 ? 'keyboard' : 'pointer'
}

/** After a pointer click, drops the focus the click gave the control. */
export function releasePointerFocus(event: React.MouseEvent<HTMLElement>): void {
  if (pressedWith(event) === 'pointer') event.currentTarget.blur()
}
