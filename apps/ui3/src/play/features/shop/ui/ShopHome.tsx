// The Shop's All home: the published outfits in a carousel (picking one dresses the avatar in it), then
// the Trending and New rows, each with a way to the full catalogue. What it shows comes in as props; the
// page reads it (homeQuery) and draws each row's card.

import { ChevronRight, Rail } from '../../../design'
import { ShopItemSkeleton } from './ShopItemCard'
import styles from './ShopHome.module.css'

/** A row shows five whole cards and the sixth's edge, so it pages five at a time. */
const ROW_PAGE = 5
/** What a hovered card grows into: 5% and a 3px ring, ~8px a side and ~11px top and bottom. */
const HOVER_ROOM = 12

/** An outfit's tile: its picture on its gradient. */
export interface ShopHomeOutfit {
  id: string
  name: string
  image?: string | null
  gradientFrom?: string
  gradientTo?: string
}

/** A row's items, or that they are still coming. */
export interface ShopHomeRow<T> {
  items: readonly T[] | undefined
  pending: boolean
}

export function ShopHome<T extends { id: string }, O extends ShopHomeOutfit>({
  outfits,
  trending,
  newest,
  renderCard,
  onOutfit,
  onViewAll
}: {
  outfits: readonly O[] | undefined
  trending: ShopHomeRow<T>
  newest: ShopHomeRow<T>
  /** The grid's own card, so a row's item selects and tries on as it does there. */
  renderCard: (item: T) => React.ReactNode
  onOutfit: (outfit: O) => void
  onViewAll: (row: 'trending' | 'newest') => void
}): React.JSX.Element {
  return (
    <div className={styles.home}>
      {outfits != null && outfits.length > 0 && (
        <Rail variant="shop" perPage={4} gap={20} bleed={HOVER_ROOM} className={styles.outfits}>
          {outfits.map((o) => (
            <button
              key={o.id}
              type="button"
              className={styles.outfit}
              aria-label={`Try on ${o.name}`}
              style={{ '--from': o.gradientFrom || undefined, '--to': o.gradientTo || undefined } as React.CSSProperties}
              onClick={() => onOutfit(o)}
            >
              {o.image != null && <img src={o.image} alt="" />}
            </button>
          ))}
        </Rail>
      )}
      <HomeRow title="Trending" row={trending} renderCard={renderCard} onViewAll={() => onViewAll('trending')} />
      <HomeRow title="New" row={newest} renderCard={renderCard} onViewAll={() => onViewAll('newest')} />
    </div>
  )
}

function HomeRow<T extends { id: string }>({
  title,
  row,
  renderCard,
  onViewAll
}: {
  title: string
  row: ShopHomeRow<T>
  renderCard: (item: T) => React.ReactNode
  onViewAll: () => void
}): React.JSX.Element | null {
  if (!row.pending && (row.items == null || row.items.length === 0)) return null
  return (
    <section className={styles.row} aria-label={title}>
      <header className={styles.rowHead}>
        <h2 className={styles.rowTitle}>{title}</h2>
        <button type="button" className={styles.viewAll} onClick={onViewAll}>
          View all
          <ChevronRight size={16} />
        </button>
      </header>
      {row.pending ? (
        <div className={styles.skeletons} aria-busy="true">
          {Array.from({ length: ROW_PAGE + 1 }, (_, i) => (
            <ShopItemSkeleton key={i} />
          ))}
        </div>
      ) : (
        <Rail perPage={ROW_PAGE} gap={20} bleed={HOVER_ROOM}>
          {(row.items ?? []).map((item) => (
            <div key={item.id} className={styles.slot}>
              {renderCard(item)}
            </div>
          ))}
        </Rail>
      )}
    </section>
  )
}
