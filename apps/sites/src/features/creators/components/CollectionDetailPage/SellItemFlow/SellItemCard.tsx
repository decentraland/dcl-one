import { useTranslation } from '~/intl'
import { getContentsStorageUrl } from '~/lib/builder'
import { EmotePlayMode } from '~/lib/itemFactory'
import { ItemType, getItemBodyShapeType, getItemSales, isSmartWearable, type Item } from '~/lib/items'
import { SmartIcon } from '~/components/Icons'
import { BodyShapeIcon, CategoryIcon, PlayModeIcon } from '~/components/ItemIcons'
import { ItemThumbnail } from '~/components/ItemThumbnail'
import { RarityPill } from '~/components/RarityPill'
import styles from './SellItemModal.module.css'

type Props = {
  item: Item
  /** Adds the "70 / 100 available" line under the badges. */
  showAvailability?: boolean
}

/** The item being sold: artwork, name, and its rarity / category / body shape / play mode / smart badges. */
export function SellItemCard({ item, showAvailability = false }: Props) {
  const { t } = useTranslation()
  const sales = showAvailability ? getItemSales(item) : undefined
  const thumbnailHash = item.contents[item.thumbnail]
  const bodyShape = getItemBodyShapeType(item)
  const category = item.data.category
  const isEmote = item.type === ItemType.EMOTE
  const isSmart = isSmartWearable(item)

  return (
    <div className={styles.card} data-testid="sell-item-card">
      <div className={styles.cardThumb}>
        <ItemThumbnail src={thumbnailHash ? getContentsStorageUrl(thumbnailHash) : null} rarity={item.rarity} />
      </div>
      <div className={styles.cardBody}>
        <h3 className={styles.cardName} title={item.name}>
          {item.name}
        </h3>
        <div className={styles.badges}>
          {item.rarity && <RarityPill rarity={item.rarity} />}
          {category && (
            <span
              className={styles.badge}
              data-testid="sell-item-badge-category"
              title={t(`collection_detail_page.category.${category}`)}
            >
              <CategoryIcon category={category} />
            </span>
          )}
          {bodyShape && (
            <span
              className={styles.badge}
              data-testid="sell-item-badge-body-shape"
              title={t(`collection_detail_page.body_type.${bodyShape}`)}
            >
              <BodyShapeIcon bodyShape={bodyShape} />
            </span>
          )}
          {isEmote && (
            <span
              className={styles.badge}
              data-testid="sell-item-badge-play-mode"
              title={t(
                `collection_detail_page.play_mode.${item.data.loop ? EmotePlayMode.LOOP : EmotePlayMode.SIMPLE}`
              )}
            >
              <PlayModeIcon playMode={item.data.loop ? EmotePlayMode.LOOP : EmotePlayMode.SIMPLE} />
            </span>
          )}
          {isSmart && (
            <span
              className={styles.badge}
              data-testid="sell-item-badge-smart"
              title={t('sell_item_modal.smart_wearable')}
            >
              <SmartIcon />
            </span>
          )}
        </div>
        {sales && (
          <span className={styles.availability} data-testid="sell-item-availability">
            {t('sell_item_modal.update_price.available', {
              available: sales.maxSupply - sales.minted,
              total: sales.maxSupply
            })}
          </span>
        )}
      </div>
    </div>
  )
}
