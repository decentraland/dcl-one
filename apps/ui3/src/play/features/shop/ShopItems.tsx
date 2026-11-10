// The page's cards and item panel: the Shop UI (./ui) fed from the HUD — the creator's profile, the item's
// entity (description, hides), its price as the web Shop's card rule sets it and its category glyph.

import { cardPrice } from '@shop/discovery/cardPrice'
import type { CatalogItem } from '@shop/discovery/lib/catalogItem'
import { MAX_SUPPLY } from '@shop/discovery/lib/rarity'
import { saleDiscountPct } from '@shop/discovery/lib/sale'
import { SIGNED_OUT } from '@shop/model/settlement'
import { MaskIcon } from '@hud/design'
import { shortAddr, userColor } from '../../lib/identity'
import emotesIcon from '@hud/assets/backpack/icon-emotes.webp'
import { CategoryIcon } from '@hud/features/backpack/categoryIcons'
import { useProfile } from '../session/profileStore'
import { useItemEntity } from './itemEntity'
import { tryOnUrn, webShopUrl } from './itemLinks'
import { buyableInWorld, PriceTag, ShopCardActions, ShopItemCard, ShopItemPanel, type PressedWith } from './ui'

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
  onOpenOnWeb,
  onBuy,
  emoteTryOn = false
}: {
  item: CatalogItem
  selected: boolean
  triedOn: boolean
  lookKnown: boolean
  onSelect: () => void
  onTryOn: () => void
  onOpenOnWeb: () => void
  /** BUY: buys the item (the shell's purchase dialog). */
  onBuy: (input: PressedWith) => void
  /** Whether an emote can be tried on (InWorldShop's emoteTryOn). */
  emoteTryOn?: boolean
}): React.JSX.Element {
  const creator = useProfile(item.creator)
  const creatorName = creator?.name || shortAddr(item.creator)
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
      creator={creatorName}
      creatorPicture={creator?.picture}
      price={<PriceTag price={price} size={16} />}
      discount={discount}
      categoryIcon={item.category === 'emote' || item.wearableCategory != null ? itemIcon(item, 16) : undefined}
      selected={selected || triedOn}
      onSelect={onSelect}
      actions={
        <ShopCardActions
          tryOn={
            tryOnUrn(item) == null || (emote && !emoteTryOn)
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
          onBuy={onBuy}
          onView={webShopUrl(item) != null ? onOpenOnWeb : undefined}
        />
      }
    />
  )
}

/** A creator's name, from their profile (their short address without one). */
export function CreatorName({ address }: { address: string }): React.JSX.Element {
  const creator = useProfile(address)
  return <>{creator?.name || shortAddr(address)}</>
}

/** An item's category glyph for a cart line (`category`: 'emote' or the wearable's). */
export function categoryGlyph(category: string | undefined, size: number): React.ReactNode {
  if (category === 'emote') return <MaskIcon src={emotesIcon} size={size} />
  return category ? <CategoryIcon category={category} size={size} /> : undefined
}

export function SelectedItemPanel({
  item,
  onClose,
  onOpenOnWeb,
  onBuy,
  onAddToCart
}: {
  item: CatalogItem
  onClose: () => void
  onOpenOnWeb: () => void
  /** BUY ITEM. */
  onBuy: (input: PressedWith) => void
  /** ADD TO CART; absent while the cart can't take it (or nothing is bought here). */
  onAddToCart?: () => void
}): React.JSX.Element {
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
      onOpenOnWeb={webShopUrl(item) != null ? onOpenOnWeb : undefined}
      onBuy={onBuy}
      onAddToCart={onAddToCart}
    />
  )
}
