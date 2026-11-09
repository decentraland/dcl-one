// How a button was pressed: a pointer click counts its clicks (detail ≥ 1); Enter or Space on a focused
// button, which clicks it too, counts none. The purchase dialog rings its first focus only for the keyboard.

export type PressedWith = 'pointer' | 'keyboard'

export function pressedWith(e: { detail: number }): PressedWith {
  return e.detail === 0 ? 'keyboard' : 'pointer'
}
