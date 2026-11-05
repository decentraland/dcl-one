// The Shop's All home: the published outfits in a carousel (picking one dresses the avatar in it), then
// the Trending and New rows, each with a way to the full catalogue.

import type { CatalogItem } from '../../../shop/discovery/lib/catalogItem'
import { ChevronRight, Rail } from '../../design'
import { useNewest, useShopOutfits, useTrending, type OutfitSummary } from './homeQuery'
import styles from './ShopHome.module.css'

export function ShopHome({
  enabled,
  creatorSales,
  renderCard,
  onOutfit,
  onViewAll
}: {
  enabled: boolean
  creatorSales: boolean
  /** The grid's own card, so a row's item selects and tries on as it does there. */
  renderCard: (item: CatalogItem) => React.ReactNode
  onOutfit: (outfit: OutfitSummary) => void
  onViewAll: (row: 'trending' | 'newest') => void
}): React.JSX.Element {
  const outfits = useShopOutfits(enabled)
  const trending = useTrending(creatorSales, enabled)
  const newest = useNewest(creatorSales, enabled)
  return (
    <div className={styles.home}>
      {outfits.data != null && outfits.data.length > 0 && (
        <Rail perPage={4} gap={20} className={styles.outfits}>
          {outfits.data.map((o) => (
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
      <HomeRow title="Trending" items={trending.data} pending={trending.isPending} renderCard={renderCard} onViewAll={() => onViewAll('trending')} />
      <HomeRow title="New" items={newest.data} pending={newest.isPending} renderCard={renderCard} onViewAll={() => onViewAll('newest')} />
    </div>
  )
}

function HomeRow({
  title,
  items,
  pending,
  renderCard,
  onViewAll
}: {
  title: string
  items: CatalogItem[] | undefined
  pending: boolean
  renderCard: (item: CatalogItem) => React.ReactNode
  onViewAll: () => void
}): React.JSX.Element | null {
  if (!pending && (items == null || items.length === 0)) return null
  return (
    <section className={styles.row} aria-label={title}>
      <header className={styles.rowHead}>
        <h2 className={styles.rowTitle}>{title}</h2>
        <button type="button" className={styles.viewAll} onClick={onViewAll}>
          View all
          <ChevronRight size={16} />
        </button>
      </header>
      {pending ? (
        <div className={styles.skeletons} aria-busy="true">
          {Array.from({ length: 6 }, (_, i) => (
            <span key={i} className={styles.skeleton} />
          ))}
        </div>
      ) : (
        <Rail perPage={6} gap={20} className={styles.rail}>
          {(items ?? []).map((item) => (
            <div key={item.id} className={styles.slot}>
              {renderCard(item)}
            </div>
          ))}
        </Rail>
      )}
    </section>
  )
}
