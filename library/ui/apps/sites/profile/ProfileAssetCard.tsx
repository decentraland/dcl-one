import { Button, ShopCard, ShopCardPrice, type ShopCardProps } from '../../../index'
import { GeneralName } from '../../../foundations/icons'

import type { MouseEvent } from 'react'
import { categoryGlyph, collectibleBadges } from './collectibles'

type Navigate = (href: string) => (event: MouseEvent<HTMLAnchorElement>) => void

type Asset = {
  name: string
  image?: string
  category: string
  rarity?: ShopCardProps['rarity']
  price?: string
  network: 'ethereum' | 'polygon'
  traits: { category?: string; bodyShapes: readonly string[]; isSmart: boolean }
}

/** A NAME's View (its token page) and Transfer (its token's transfer page). */
export function NameActions({
  name,
  view,
  navigate
}: {
  name: string
  view: string
  navigate: Navigate
}) {
  const transfer = `${view}/transfer`
  return (
    <>
      <Button
        href={view}
        onClick={navigate(view)}
        variant="secondary"
        size="md"
        aria-label={`View ${name}`}
      >
        View
      </Button>
      <Button
        href={transfer}
        onClick={navigate(transfer)}
        size="md"
        aria-label={`Transfer ${name}`}
      >
        Transfer
      </Button>
    </>
  )
}

export function AssetCard({
  asset,
  href,
  navigate,
  linkComponent
}: {
  asset: Asset
  href: string
  navigate: Navigate
  linkComponent: ShopCardProps['linkComponent']
}) {
  return (
    <ShopCard
      name={asset.name}
      image={asset.image}
      // The catalogue has no image for a NAME.
      placeholder={asset.category === 'ens' ? <GeneralName size={64} /> : undefined}
      rarity={asset.rarity}
      price={
        asset.price ? (
          <ShopCardPrice kind="mana" amount={asset.price} network={asset.network} />
        ) : undefined
      }
      categoryIcon={categoryGlyph(asset.traits.category)}
      badges={collectibleBadges(asset.traits)}
      href={href}
      linkComponent={linkComponent}
      variant="catalogue"
      headingLevel={3}
      actions={
        asset.category === 'ens' ? (
          <NameActions name={asset.name} view={href} navigate={navigate} />
        ) : (
          <Button href={href} onClick={navigate(href)} size="md" aria-label={`View ${asset.name}`}>
            View
          </Button>
        )
      }
    />
  )
}
