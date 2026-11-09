// The Shop's All home: the published outfits in a carousel (picking one dresses the avatar in it), then
// the Trending and New rows, each with a way to the full catalogue. What it shows comes in as props; the
// page reads it (homeQuery) and draws each row's card.

import { Button, ChevronRight, EmptyState, Rail } from '../../../design'
import { pressedWith, type PressedWith } from './pressedWith'
import { ShopItemSkeleton } from './ShopItemCard'
import styles from './ShopHome.module.css'

/** A row shows five whole cards and the sixth's edge, so it pages five at a time. */
const ROW_PAGE = 5
/** A hovered or selected card's ring sits 8px outside it. */
const HOVER_ROOM = 12
/** A hovered outfit's render rises 56px over its card (46px at rest), as the design draws it. */
const OUTFIT_RISE = 56

/** An outfit's tile: its picture on its gradient. */
export interface ShopHomeOutfit {
  id: string
  name: string
  image?: string | null
  gradientFrom?: string
  gradientTo?: string
}

/** What a hovered outfit offers: how many items it has, their price when it has one, and BUY (without it, none). */
export interface ShopHomeOutfitOffer {
  items: number
  price?: React.ReactNode
  onBuy?: (input: PressedWith) => void
}

/** A row's items, that they are still coming, or that they couldn't be read. */
export interface ShopHomeRow<T> {
  items: readonly T[] | undefined
  pending: boolean
  error?: boolean
}

export function ShopHome<T extends { id: string }, O extends ShopHomeOutfit>({
  outfits,
  trending,
  newest,
  renderCard,
  onOutfit,
  outfitOffer,
  onViewAll,
  onRetry
}: {
  outfits: readonly O[] | undefined
  trending: ShopHomeRow<T>
  newest: ShopHomeRow<T>
  /** The grid's own card, so a row's item selects and tries on as it does there. */
  renderCard: (item: T) => React.ReactNode
  onOutfit: (outfit: O) => void
  /** The hovered outfit's name row: its items, their price and BUY. */
  outfitOffer?: (outfit: O) => ShopHomeOutfitOffer
  onViewAll: (row: 'trending' | 'newest') => void
  /** Reads a row again after it failed. */
  onRetry: (row: 'trending' | 'newest') => void
}): React.JSX.Element {
  const shows = (row: ShopHomeRow<T>): boolean => row.pending || row.error === true || (row.items?.length ?? 0) > 0
  if ((outfits == null || outfits.length === 0) && !shows(trending) && !shows(newest)) {
    return <EmptyState variant="inline" title="Nothing on sale right now" subtitle="Check back soon." />
  }
  return (
    <div className={styles.home}>
      {outfits != null && outfits.length > 0 && (
        <Rail variant="shop" perPage={4} gap={20} bleed={HOVER_ROOM} bleedTop={OUTFIT_RISE + HOVER_ROOM} className={styles.outfits}>
          {outfits.map((o) => {
            const offer = outfitOffer?.(o)
            return (
              <div
                key={o.id}
                className={styles.outfit}
                style={{ '--from': o.gradientFrom || undefined, '--to': o.gradientTo || undefined } as React.CSSProperties}
              >
                <button type="button" className={styles.outfitHit} aria-label={`Try on ${o.name}`} onClick={() => onOutfit(o)} />
                {o.image != null && (
                  <span className={styles.outfitArt}>
                    <img src={o.image} alt="" />
                  </span>
                )}
                <span className={styles.outfitFade} aria-hidden="true" />
                <span className={styles.outfitInfo}>
                  <span className={styles.outfitName} title={o.name}>
                    {o.name}
                  </span>
                  {offer != null && (
                    <span className={styles.outfitMeta}>
                      <span className={styles.outfitCount}>
                        {offer.items} {offer.items === 1 ? 'Item' : 'Items'}
                      </span>
                      {offer.price != null && <span className={styles.outfitPrice}>{offer.price}</span>}
                    </span>
                  )}
                  {offer?.onBuy != null && (
                    <Button size="card" className={styles.outfitBuy} aria-label={`Buy ${o.name}`} onClick={(e) => offer.onBuy?.(pressedWith(e))}>
                      BUY
                    </Button>
                  )}
                </span>
              </div>
            )
          })}
        </Rail>
      )}
      <HomeRow title="Trending" row={trending} renderCard={renderCard} onViewAll={() => onViewAll('trending')} onRetry={() => onRetry('trending')} />
      <HomeRow title="New" row={newest} renderCard={renderCard} onViewAll={() => onViewAll('newest')} onRetry={() => onRetry('newest')} />
    </div>
  )
}

function HomeRow<T extends { id: string }>({
  title,
  row,
  renderCard,
  onViewAll,
  onRetry
}: {
  title: string
  row: ShopHomeRow<T>
  renderCard: (item: T) => React.ReactNode
  onViewAll: () => void
  onRetry: () => void
}): React.JSX.Element | null {
  if (row.error === true && !row.pending && (row.items == null || row.items.length === 0)) {
    // A row that failed keeps its place, with a way to read it again; the others stay as they are.
    return (
      <section className={styles.row} aria-label={title}>
        <header className={styles.rowHead}>
          <h2 className={styles.rowTitle}>{title}</h2>
        </header>
        <div className={styles.rowError} role="alert">
          <span>Couldn’t load these items.</span>
          <Button variant="secondary" size="sm" onClick={onRetry}>
            Retry
          </Button>
        </div>
      </section>
    )
  }
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
        // The design's rows page with the arrows only: no dots under them.
        <Rail perPage={ROW_PAGE} gap={20} bleed={HOVER_ROOM} dots={false}>
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
