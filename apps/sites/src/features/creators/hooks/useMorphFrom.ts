import { useCallback, useLayoutEffect, useRef, type RefObject } from 'react'

const OPEN_MS = 380
const CLOSE_MS = 220
const EASE_OUT = 'cubic-bezier(0.32, 0.72, 0, 1)'
const EASE_IN = 'cubic-bezier(0.55, 0, 0.75, 0.2)'
const NO_SHADOW = '0 0 0 0 rgba(0, 0, 0, 0)'

function reducedMotion(): boolean {
  return typeof window.matchMedia === 'function' && window.matchMedia('(prefers-reduced-motion: reduce)').matches
}

/** The panel's box shrunk onto the trigger's: a shift that tucks it behind the trigger, and a clip cut to its shape. */
function collapsed(panel: HTMLElement, trigger: HTMLElement): Keyframe {
  const from = trigger.getBoundingClientRect()
  const to = panel.getBoundingClientRect()
  const radius = Math.min(parseFloat(getComputedStyle(trigger).borderTopLeftRadius) || 0, from.height / 2)
  return {
    transform: `translateY(${from.top - to.top}px)`,
    clipPath: `inset(0px ${to.right - from.right}px ${to.height - from.height}px ${from.left - to.left}px round ${radius}px)`
  }
}

function expanded(panel: HTMLElement): Keyframe {
  return { transform: 'none', clipPath: `inset(0px round ${getComputedStyle(panel).borderTopLeftRadius})` }
}

/**
 * Grows a popup out from behind the button that opened it, and folds it back there on the way out.
 *
 * The trigger must stack above the panel, so the panel reads as sliding out from under it, and must be the
 * only element matching `triggerSelector` while the panel is open. Returns `leave`, which plays the fold and
 * then runs its callback; that callback must unmount the panel, which stays folded and inert once it has left.
 */
export function useMorphFrom(panel: RefObject<HTMLElement | null>, triggerSelector: string) {
  const leaving = useRef(false)
  const opening = useRef<Animation[]>([])
  const closing = useRef<{ fold: Animation; timer: ReturnType<typeof setTimeout> } | null>(null)

  useLayoutEffect(() => {
    const el = panel.current
    const trigger = document.querySelector<HTMLElement>(triggerSelector)
    if (!el || !trigger || typeof el.animate !== 'function' || reducedMotion()) return
    const shadow = getComputedStyle(el).boxShadow
    const grow = el.animate([collapsed(el, trigger), expanded(el)], { duration: OPEN_MS, easing: EASE_OUT })
    // The clip hides the shadow while the panel grows; fading it in afterwards keeps it from snapping on.
    const lift = el.animate([{ boxShadow: NO_SHADOW }, { boxShadow: shadow }], {
      duration: 200,
      delay: OPEN_MS,
      easing: 'ease-out',
      fill: 'backwards'
    })
    // Content waits until the panel has mostly grown, so it never shows squeezed, then settles a few px down.
    const fades = [...el.children].map((child, i) =>
      child.animate(
        [
          { opacity: 0, transform: 'translateY(-6px)' },
          { opacity: 1, transform: 'none' }
        ],
        {
          duration: OPEN_MS * 0.6,
          delay: OPEN_MS * 0.18 + i * 30,
          easing: EASE_OUT,
          fill: 'backwards'
        }
      )
    )
    opening.current = [grow, lift, ...fades]
    // A rerun measures the panel's resting box, which a still-running grow would have shifted.
    return () => [grow, lift, ...fades].forEach(animation => animation.cancel())
  }, [panel, triggerSelector])

  // Unmounted some other way mid-fold: drop the fold without running its callback a second time.
  useLayoutEffect(
    () => () => {
      const current = closing.current
      if (!current) return
      clearTimeout(current.timer)
      current.fold.onfinish = null
      current.fold.cancel()
    },
    []
  )

  return useCallback(
    (then: () => void) => {
      const el = panel.current
      const trigger = document.querySelector<HTMLElement>(triggerSelector)
      if (leaving.current) return
      if (!el || !trigger || typeof el.animate !== 'function' || reducedMotion()) return then()
      leaving.current = true
      // Swallowed rather than let through, so a second tap on the folding panel never reaches the page under it.
      const swallow = (event: Event) => {
        event.preventDefault()
        event.stopPropagation()
      }
      el.addEventListener('pointerdown', swallow, true)
      el.addEventListener('click', swallow, true)
      // Closed before it finished opening: fold from where it is now, measured against the resting box.
      const now = getComputedStyle(el)
      const from: Keyframe = { transform: now.transform, clipPath: now.clipPath, boxShadow: now.boxShadow }
      const shown = [...el.children].map(child => {
        const style = getComputedStyle(child)
        return { opacity: style.opacity, transform: style.transform }
      })
      opening.current.forEach(animation => animation.cancel())
      if (from.clipPath === 'none') from.clipPath = expanded(el).clipPath
      el.animate([{ boxShadow: from.boxShadow }, { boxShadow: NO_SHADOW }], {
        duration: CLOSE_MS * 0.4,
        fill: 'forwards'
      })
      ;[...el.children].forEach((child, i) =>
        child.animate([shown[i], { opacity: 0, transform: 'translateY(-4px)' }], {
          duration: CLOSE_MS * 0.45,
          easing: 'ease-in',
          fill: 'forwards'
        })
      )
      const fold = el.animate([{ transform: from.transform, clipPath: from.clipPath }, collapsed(el, trigger)], {
        duration: CLOSE_MS,
        delay: CLOSE_MS * 0.15,
        easing: EASE_IN,
        fill: 'both'
      })
      let done = false
      const finish = () => {
        if (done) return
        done = true
        clearTimeout(timer)
        then()
      }
      // A hidden tab can hold the fold's finish back; the panel must still go away.
      const timer = setTimeout(finish, CLOSE_MS * 1.15 + 150)
      closing.current = { fold, timer }
      fold.onfinish = finish
    },
    [panel, triggerSelector]
  )
}
