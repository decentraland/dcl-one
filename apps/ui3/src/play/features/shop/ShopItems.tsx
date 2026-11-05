// The page's cards and item panel: the Shop UI (./ui) fed from the HUD — the creator's profile, the item's
// entity (description, hides), its price as the web Shop's card rule sets it and its category glyph.

import { cardPrice } from '../../../shop/discovery/cardPrice'
import type { CatalogItem } from '../../../shop/discovery/lib/catalogItem'
import { MAX_SUPPLY } from '../../../shop/discovery/lib/rarity'
import { saleDiscountPct } from '../../../shop/discovery/lib/sale'
import { SIGNED_OUT } from '../../../shop/model/settlement'
import { MaskIcon } from '../../design'
import { shortAddr, userColor } from '../../lib/identity'
import emotesIcon from '../../assets/backpack/icon-emotes.webp'
import { CategoryIcon } from '../backpack/categoryIcons'
import { useProfile } from '../session/profileStore'
import { useItemEntity } from './itemEntity'
import { tryOnUrn } from './itemLinks'
import { buyableInWorld, PriceTag, ShopCardActions, ShopItemCard, ShopItemPanel } from './ui'

const CATEGORY_NAMES: Record<string, string> = { hands_wear: 'Handwear', body_shape: 'Body shape', upper_body: 'Upper body', lower_body: 'Lower body', top_head: 'Top head', facial_hair: 'Facial hair' }
const categoryName = (c: string): string => CATEGORY_NAMES[c] ?? c.replace(/[_-]+/g, ' ')

/** An item's category glyph: the emote mark, or its wearable category's. */
function itemIcon(item: CatalogItem, size: number): React.ReactNode {
  if (item.category === 'emote') return <MaskIcon src={emotesIcon} size={size} />
  const category = item.wearableCategory ?? item.category
  return category ? <CategoryIcon category={category} size={size} /> : undefined
}

export function ShopCard({
  item,
  selected,
  triedOn,
  lookKnown,
  onSelect,
  onTryOn,
  onOpenOnWeb
}: {
  item: CatalogItem
  selected: boolean
  triedOn: boolean
  lookKnown: boolean
  onSelect: () => void
  onTryOn: () => void
  onOpenOnWeb: () => void
}): React.JSX.Element {
  const creator = useProfile(item.creator)
  // Priced signed out: credits for primary sales, MANA for resales. The HUD's identity doesn't say how the
  // buyer signed in (features/auth/sso.ts forwards it whatever the method), so a web3 buyer's extra MANA
  // price waits for the Buy flow's wallet bridge, which knows.
  const price = cardPrice(item, SIGNED_OUT)
  const discount = price.kind === 'credits' && price.compareAt != null ? `-${saleDiscountPct(price.compareAt, price.credits)}%` : undefined
  const emote = item.category === 'emote'
  return (
    <ShopItemCard
      name={item.name}
      thumbnail={item.thumbnail}
      rarity={item.rarity}
      creator={creator?.name || shortAddr(item.creator)}
      creatorPicture={creator?.picture}
      price={<PriceTag price={price} size={15} />}
      discount={discount}
      categoryIcon={item.category === 'emote' || item.wearableCategory != null ? itemIcon(item, 16) : undefined}
      selected={selected || triedOn}
      onSelect={onSelect}
      actions={
        <ShopCardActions
          tryOn={
            tryOnUrn(item) == null
              ? undefined
              : {
                  label: triedOn ? (emote ? 'STOP' : 'TAKE OFF') : 'TRY ON',
                  // An emote plays over any look; a wearable needs the look it swaps a piece of.
                  disabled: !emote && !lookKnown,
                  title: emote || lookKnown ? undefined : 'Loading your look…',
                  onClick: onTryOn
                }
          }
          buyInWorld={buyableInWorld(price)}
          onBuy={onSelect}
          onView={onOpenOnWeb}
        />
      }
    />
  )
}

export function SelectedItemPanel({ item, onClose, onOpenOnWeb }: { item: CatalogItem; onClose: () => void; onOpenOnWeb: () => void }): React.JSX.Element {
  const creator = useProfile(item.creator)
  const entity = useItemEntity(tryOnUrn(item))
  const rarity = item.rarity || 'base'
  const max = MAX_SUPPLY[rarity]
  const creatorName = creator?.name || shortAddr(item.creator)
  return (
    <ShopItemPanel
      name={item.name}
      thumbnail={item.thumbnail}
      rarity={rarity}
      icon={itemIcon(item, 28)}
      price={cardPrice(item, SIGNED_OUT)}
      stock={item.available != null && max != null ? `${item.available}/${max}` : undefined}
      creator={{ name: creatorName, picture: creator?.picture, color: userColor(item.creator, creatorName, creator?.hasClaimedName) }}
      description={entity.data?.description}
      hides={(entity.data?.hides ?? []).map((h) => ({ key: h, label: categoryName(h), icon: <CategoryIcon category={h} size={22} /> }))}
      onClose={onClose}
      onOpenOnWeb={onOpenOnWeb}
    />
  )
}
