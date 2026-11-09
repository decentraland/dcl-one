// The Shop's FILTERS popover: a credits price range (typed, or dragged on the web Shop's price scale),
// rarities, the sale status and the "show only" switches. Each change applies at once; an outside click
// closes it. Escape is the host's (the HUD's cancel stack, as its other popovers take it).

import { useEffect, useRef, useState } from 'react'
import { PRICE_SLIDER_MAX, PRICE_SLIDER_STEPS, priceToSliderPos, sliderPosToPrice } from '../../../../shop/discovery/lib/price-slider'
import { formatCreditsFull } from '../../../../shop/checkout/lib/currency'
import CreditMark from '../../../../shop/credits/components/CreditMark'
import styles from './ShopFilters.module.css'

/** In the order the design lists them. */
const RARITIES = ['common', 'uncommon', 'rare', 'epic', 'legendary', 'exotic', 'mythic', 'unique'] as const
export type ShopFilterStatus = 'all' | 'on_sale' | 'not_for_sale'
/** What the popover sets: a price range in credits (null is no bound), rarities, status and switches. */
export interface ShopFilterValues {
  rarities: readonly string[]
  minCredits: number | null
  maxCredits: number | null
  status: ShopFilterStatus
  deals: boolean
  smart: boolean
}

const STATUSES: readonly { id: ShopFilterStatus; label: string }[] = [
  { id: 'all', label: 'All' },
  { id: 'on_sale', label: 'On sale' },
  { id: 'not_for_sale', label: 'Not for sale' }
]

export type ShopFilterPatch = Partial<Omit<ShopFilterValues, 'rarities'> & { rarities: string[] }>

export function ShopFilters({
  filters,
  onChange,
  onClose,
  showDeals,
  showSmart,
  className
}: {
  filters: ShopFilterValues
  onChange: (patch: ShopFilterPatch) => void
  onClose: () => void
  /** Deals exist only while creator sales are on. */
  showDeals: boolean
  /** Smart wearables only: emotes have none. */
  showSmart: boolean
  className?: string
}): React.JSX.Element {
  const ref = useRef<HTMLDivElement>(null)
  useEffect(() => {
    const onDown = (e: MouseEvent): void => {
      const t = e.target as Node
      if (ref.current != null && !ref.current.contains(t) && !(ref.current.parentElement?.contains(t) ?? false)) onClose()
    }
    document.addEventListener('mousedown', onDown)
    return () => document.removeEventListener('mousedown', onDown)
  }, [onClose])

  const toggleRarity = (r: string): void => {
    const next = filters.rarities.includes(r) ? filters.rarities.filter((x) => x !== r) : [...filters.rarities, r]
    onChange({ rarities: [...next].sort() })
  }
  const priced = filters.status !== 'not_for_sale'
  return (
    <div ref={ref} className={`${styles.menu} ${className ?? ''}`.trim()} role="dialog" aria-label="Filters">
      {priced && (
        <section className={styles.section}>
          <span className={styles.label}>Price</span>
          <PriceRange min={filters.minCredits} max={filters.maxCredits} onChange={(minCredits, maxCredits) => onChange({ minCredits, maxCredits })} />
        </section>
      )}
      <section className={styles.section}>
        <span className={styles.label}>Rarity</span>
        <div className={styles.rarities}>
          {RARITIES.map((r) => (
            <button key={r} type="button" className={styles.rarity} data-rarity={r} aria-pressed={filters.rarities.includes(r)} onClick={() => toggleRarity(r)}>
              {r}
            </button>
          ))}
        </div>
      </section>
      <section className={styles.section} role="radiogroup" aria-label="Status">
        <span className={styles.label}>Status</span>
        {STATUSES.map((s) => (
          <button key={s.id} type="button" role="radio" aria-checked={filters.status === s.id} className={styles.choice} onClick={() => onChange({ status: s.id })}>
            <span className={styles.radio} aria-hidden="true" />
            {s.label}
          </button>
        ))}
      </section>
      {(showDeals || showSmart) && (
        <section className={styles.section}>
          <span className={styles.label}>Show only</span>
          {showDeals && (
            <button type="button" role="checkbox" aria-checked={filters.deals} className={styles.choice} onClick={() => onChange({ deals: !filters.deals })}>
              <span className={styles.box} aria-hidden="true" />
              Deals
            </button>
          )}
          {showSmart && (
            <button type="button" role="checkbox" aria-checked={filters.smart} className={styles.choice} onClick={() => onChange({ smart: !filters.smart })}>
              <span className={styles.box} aria-hidden="true" />
              Smart
            </button>
          )}
        </section>
      )}
    </div>
  )
}

/** Min and Max, typed or dragged; an empty or zero bound is "no bound". A drag applies on release. */
function PriceRange({ min, max, onChange }: { min: number | null; max: number | null; onChange: (min: number | null, max: number | null) => void }): React.JSX.Element {
  const [lo, setLo] = useState(min == null ? '' : String(min))
  const [hi, setHi] = useState(max == null ? '' : String(max))
  useEffect(() => setLo(min == null ? '' : String(min)), [min])
  useEffect(() => setHi(max == null ? '' : String(max)), [max])
  const bound = (text: string): number | null => {
    const n = Number(text)
    return text.trim() === '' || !Number.isFinite(n) || n <= 0 ? null : Math.floor(n)
  }
  const commit = (nextLo = lo, nextHi = hi): void => {
    const a = bound(nextLo)
    const b = bound(nextHi)
    if (a !== min || b !== max) onChange(a, b)
  }
  const loPos = priceToSliderPos(bound(lo) ?? 0)
  const hiPos = priceToSliderPos(bound(hi) ?? PRICE_SLIDER_MAX)
  return (
    <div className={styles.price}>
      <div className={styles.bounds}>
        <input className={styles.input} inputMode="numeric" placeholder="Min" aria-label="Minimum price" value={lo} onChange={(e) => setLo(e.target.value.replace(/[^\d]/g, ''))} onBlur={() => commit()} onKeyDown={(e) => e.key === 'Enter' && commit()} />
        <span className={styles.to}>to</span>
        <input className={styles.input} inputMode="numeric" placeholder="Max" aria-label="Maximum price" value={hi} onChange={(e) => setHi(e.target.value.replace(/[^\d]/g, ''))} onBlur={() => commit()} onKeyDown={(e) => e.key === 'Enter' && commit()} />
      </div>
      <div className={styles.track} style={{ '--lo': `${(loPos / PRICE_SLIDER_STEPS) * 100}%`, '--hi': `${(hiPos / PRICE_SLIDER_STEPS) * 100}%` } as React.CSSProperties}>
        <input
          type="range"
          className={styles.thumb}
          min={0}
          max={PRICE_SLIDER_STEPS}
          value={loPos}
          aria-label="Minimum price slider"
          onChange={(e) => setLo(String(Math.min(sliderPosToPrice(Number(e.target.value)), bound(hi) ?? PRICE_SLIDER_MAX) || ''))}
          onPointerUp={() => commit()}
          onKeyUp={() => commit()}
        />
        <input
          type="range"
          className={styles.thumb}
          min={0}
          max={PRICE_SLIDER_STEPS}
          value={hiPos}
          aria-label="Maximum price slider"
          onChange={(e) => {
            const p = sliderPosToPrice(Number(e.target.value))
            setHi(p >= PRICE_SLIDER_MAX ? '' : String(Math.max(p, bound(lo) ?? 0)))
          }}
          onPointerUp={() => commit()}
          onKeyUp={() => commit()}
        />
      </div>
      <div className={styles.ends}>
        <span>
          <CreditMark size={12} />1
        </span>
        <span>
          <CreditMark size={12} />
          {formatCreditsFull(PRICE_SLIDER_MAX)}
        </span>
      </div>
    </div>
  )
}
