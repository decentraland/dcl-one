const STEP: Record<string, number> = { ArrowLeft: -1, ArrowUp: -1, ArrowRight: 1, ArrowDown: 1 }

/** The index a key moves to from `index` among `count` items; null for any other key. */
export function arrowStep(key: string, index: number, count: number): number | null {
  if (count === 0) return null
  if (key in STEP) return (Math.max(index, 0) + STEP[key] + count) % count
  if (key === 'Home') return 0
  if (key === 'End') return count - 1
  return null
}

/** Moves focus between the elements with `role` under the key event's container, and returns the
 *  index it moved to (null when the key moves nothing). */
export function moveFocus(e: React.KeyboardEvent<HTMLElement>, role: 'radio' | 'menuitem'): number | null {
  const items = [...e.currentTarget.querySelectorAll<HTMLElement>(`[role="${role}"]`)]
  const next = arrowStep(e.key, items.indexOf(document.activeElement as HTMLElement), items.length)
  if (next == null) return null
  e.preventDefault()
  items[next].focus()
  return next
}
