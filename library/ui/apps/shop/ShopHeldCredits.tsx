import { useEffect, useId, useRef, useState, type FocusEvent, type ReactNode } from 'react'

import styles from './ShopHeldCredits.module.css'

type ShopHeldCreditsProps = {
  credits: number
  purchaseCount: number
  releasesAtSeconds: number | null
  now?: number
  children: ReactNode
}

function formatRemaining(seconds: number): string {
  const safe = Math.max(0, seconds)
  return `${Math.floor(safe / 60)}:${String(safe % 60).padStart(2, '0')}`
}

export function ShopHeldCredits({
  credits,
  purchaseCount,
  releasesAtSeconds: releasesAt,
  now: fixedNow,
  children
}: ShopHeldCreditsProps) {
  const panelId = useId()
  const [open, setOpen] = useState(false)
  const [liveNow, setLiveNow] = useState(() => Math.floor(Date.now() / 1000))
  const now = fixedNow === undefined ? liveNow : Math.floor(fixedNow / 1000)
  const rootRef = useRef<HTMLDivElement>(null)
  const remaining = releasesAt === null ? 0 : releasesAt - now
  const unknown = releasesAt === null || remaining <= 0

  useEffect(() => {
    if (fixedNow !== undefined || releasesAt === null || credits <= 0) return
    const update = () => setLiveNow(Math.floor(Date.now() / 1000))
    update()
    const id = setInterval(update, 1000)
    return () => clearInterval(id)
  }, [credits, releasesAt, fixedNow])

  useEffect(() => {
    if (!open) return
    const onKey = (e: KeyboardEvent) => e.key === 'Escape' && setOpen(false)
    document.addEventListener('keydown', onKey)
    return () => document.removeEventListener('keydown', onKey)
  }, [open])

  if (credits <= 0) return null
  const hideUnlessInside = (e: FocusEvent<HTMLElement>) => {
    if (!rootRef.current?.contains(e.relatedTarget as Node | null)) setOpen(false)
  }

  return (
    <div
      className={styles.root}
      ref={rootRef}
      onMouseEnter={() => setOpen(true)}
      onMouseLeave={() => setOpen(false)}
      data-testid="held-credits"
    >
      <button
        type="button"
        className={styles.trigger}
        aria-describedby={open ? panelId : undefined}
        onFocus={() => setOpen(true)}
        onBlur={hideUnlessInside}
      >
        {children}
        <span>on hold</span>
      </button>
      {open ? (
        <div className={styles.panel} id={panelId} role="tooltip">
          <p className={styles.title}>
            {credits} {credits === 1 ? 'Credit is' : 'Credits are'} on hold
          </p>
          <p>
            {purchaseCount <= 1
              ? 'A purchase is still being confirmed.'
              : `${purchaseCount} purchases are still being confirmed.`}{' '}
            {unknown ? (
              <span data-testid="held-credits-unknown">
                They come back once the network confirms it; we can't say exactly when.
              </span>
            ) : (
              <span>
                Back in <b data-testid="held-credits-countdown">{formatRemaining(remaining)}</b>
              </span>
            )}
          </p>
          <p className={styles.caveat}>Nothing is lost and there is nothing you need to do.</p>
        </div>
      ) : null}
    </div>
  )
}
