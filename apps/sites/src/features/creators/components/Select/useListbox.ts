import { useEffect, useId, useLayoutEffect, useRef, useState, type CSSProperties, type KeyboardEvent } from 'react'

const VIEWPORT_MARGIN = 8
const TRIGGER_GAP = 6

type Params<T extends string> = {
  options: { value: T }[]
  /** The option the trigger points at while closed; where keyboard navigation starts. */
  current: T | null
  onPick: (value: T) => void
  /** Multi-select: picking keeps the list open so more can be ticked. */
  keepOpen?: boolean
  minWidth: number
}

/**
 * The behavior every listbox here shares: open state, the keyboard-active option, closing on an
 * outside click or Escape, and anchoring the portaled list to its trigger.
 */
export function useListbox<T extends string>({ options, current, onPick, keepOpen = false, minWidth }: Params<T>) {
  const [open, setOpen] = useState(false)
  const [active, setActive] = useState<T | null>(current)
  const wrapRef = useRef<HTMLDivElement>(null)
  const listRef = useRef<HTMLUListElement>(null)
  const [listStyle, setListStyle] = useState<CSSProperties>()
  const listId = useId()

  useEffect(() => {
    if (!open) return
    function onMouseDown(event: MouseEvent) {
      const target = event.target as Node
      if (!wrapRef.current?.contains(target) && !listRef.current?.contains(target)) setOpen(false)
    }
    document.addEventListener('mousedown', onMouseDown)
    return () => document.removeEventListener('mousedown', onMouseDown)
  }, [open])

  // Anchor the list to the trigger; open upward when the viewport has no room below.
  useLayoutEffect(() => {
    if (!open) return
    function position() {
      const trigger = wrapRef.current?.getBoundingClientRect()
      const listHeight = listRef.current?.offsetHeight ?? 0
      if (!trigger) return
      const fitsBelow = trigger.bottom + TRIGGER_GAP + listHeight <= window.innerHeight
      // A list wider than its trigger keeps its left edge but never spills past the viewport.
      const width = Math.max(trigger.width, minWidth)
      const left = Math.max(VIEWPORT_MARGIN, Math.min(trigger.left, window.innerWidth - width - VIEWPORT_MARGIN))
      setListStyle({
        left,
        width,
        ...(fitsBelow
          ? { top: trigger.bottom + TRIGGER_GAP }
          : { top: Math.max(VIEWPORT_MARGIN, trigger.top - TRIGGER_GAP - listHeight) })
      })
    }
    position()
    window.addEventListener('resize', position)
    window.addEventListener('scroll', position, true)
    return () => {
      window.removeEventListener('resize', position)
      window.removeEventListener('scroll', position, true)
    }
  }, [open, minWidth])

  function toggle() {
    if (!open) setActive(current ?? options[0]?.value ?? null)
    setOpen(!open)
  }

  function pick(value: T) {
    onPick(value)
    if (!keepOpen) setOpen(false)
  }

  function onKeyDown(event: KeyboardEvent) {
    if (!open) {
      if (event.key === 'ArrowDown' || event.key === 'ArrowUp' || event.key === 'Enter' || event.key === ' ') {
        event.preventDefault()
        toggle()
      }
      return
    }
    const index = options.findIndex(option => option.value === active)
    switch (event.key) {
      case 'ArrowDown':
        event.preventDefault()
        setActive(options[Math.min(index + 1, options.length - 1)]?.value ?? null)
        break
      case 'ArrowUp':
        event.preventDefault()
        setActive(options[Math.max(index - 1, 0)]?.value ?? null)
        break
      case 'Enter':
      case ' ':
        event.preventDefault()
        if (active !== null) pick(active)
        break
      case 'Escape':
        // Stop the surrounding modal from closing on the same keypress.
        event.stopPropagation()
        setOpen(false)
        break
    }
  }

  return { open, active, setActive, wrapRef, listRef, listStyle, listId, toggle, pick, onKeyDown }
}
