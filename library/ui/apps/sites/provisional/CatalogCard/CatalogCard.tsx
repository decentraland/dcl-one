// TODO(figma-ds): replace with the Figma-generated catalog card from library/ui/components.
import type { ComponentType, ReactNode } from 'react'
import {
  BackpackBody,
  BackpackEarrings,
  BackpackEyebrows,
  BackpackEyes,
  BackpackEyewear,
  BackpackFacialHair,
  BackpackFeet,
  BackpackGenderFemale,
  BackpackGenderMale,
  BackpackGenderUnisex,
  BackpackHair,
  BackpackHandwear,
  BackpackHat,
  BackpackHead,
  BackpackHelmet,
  BackpackLowerBody,
  BackpackMask,
  BackpackMouth,
  BackpackSkins,
  BackpackTiara,
  BackpackUpperBody,
  GeneralMana,
  GeneralPolygonMana,
  GeneralSmartItem
} from '../../../../foundations/icons'
import type { IconProps } from '../../../../foundations/icons/types'
import { useBrokenImage } from '../useBrokenImage'
import styles from './CatalogCard.module.css'

type CatalogCardRarity =
  'common' | 'uncommon' | 'rare' | 'epic' | 'legendary' | 'exotic' | 'mythic' | 'unique'

/** The app's router link, so opening the item is a client navigation; a plain anchor otherwise. */
type CatalogCardLinkProps = { to: string; className: string; children: ReactNode }

type CatalogCardProps = {
  title: string
  imageUrl?: string
  /** A decorative glyph shown instead of an image that is missing or fails, e.g. a NAME's. */
  placeholder?: ReactNode
  /** The item's or token's own page; the whole card opens it. */
  href: string
  /** Paints the image surface and the rarity tag. Without it the card has neither. */
  rarity?: CatalogCardRarity
  /** The line under the title, e.g. "By Alice". */
  byline?: ReactNode
  price?: { amount: string; network: 'ethereum' | 'polygon' }
  /** Shown where the price goes when there is none, e.g. "Not for sale". */
  priceFallback?: string
  /** Glyphs next to the rarity tag; see `CollectibleBadges`. */
  badges?: ReactNode
  /** Revealed over the badge row on hover or keyboard focus, e.g. Buy. */
  action?: ReactNode
  LinkComponent?: ComponentType<CatalogCardLinkProps>
}

const RARITY_LABEL: Record<CatalogCardRarity, string> = {
  common: 'Common',
  uncommon: 'Uncommon',
  rare: 'Rare',
  epic: 'Epic',
  legendary: 'Legendary',
  exotic: 'Exotic',
  mythic: 'Mythic',
  unique: 'Unique'
}

function AnchorLink({ to, ...props }: CatalogCardLinkProps) {
  return <a href={to} {...props} />
}

/**
 * A wearable, emote, NAME or LAND as the catalogue lists it: its image on the rarity's colour,
 * title, byline, price and badges. On hover or focus it lifts and slides its action in over
 * the badges, as the source does.
 */
function CatalogCard({
  title,
  imageUrl,
  placeholder,
  href,
  rarity,
  byline,
  price,
  priceFallback,
  badges,
  action,
  LinkComponent = AnchorLink
}: CatalogCardProps) {
  const { broken, ref, onError } = useBrokenImage(imageUrl)
  const Mana = price?.network === 'ethereum' ? GeneralMana : GeneralPolygonMana
  const rootClass = [styles.root, rarity && styles[rarity], action && styles.withAction]
    .filter(Boolean)
    .join(' ')

  return (
    <article className={rootClass}>
      <div className={styles.media}>
        {imageUrl && !broken ? (
          <img
            ref={ref}
            className={styles.image}
            src={imageUrl}
            alt=""
            loading="lazy"
            onError={onError}
          />
        ) : placeholder ? (
          <span className={styles.placeholder} aria-hidden="true">
            {placeholder}
          </span>
        ) : null}
      </div>
      <div className={styles.content}>
        <h3 className={styles.title}>
          {/* The title's link covers the card, so the whole card opens the item. */}
          <LinkComponent to={href} className={styles.cardLink}>
            {title}
          </LinkComponent>
        </h3>
        {byline ? <div className={styles.byline}>{byline}</div> : null}
        {price ? (
          <p className={styles.price}>
            <Mana size={32} aria-hidden="true" />
            <span>
              <span className={styles.visuallyHidden}>Price: </span>
              {price.amount}
              <span className={styles.visuallyHidden}> MANA</span>
            </span>
          </p>
        ) : priceFallback ? (
          <p className={styles.fallback}>{priceFallback}</p>
        ) : null}
        <div className={styles.badges}>
          {rarity ? <span className={styles.rarity}>{RARITY_LABEL[rarity]}</span> : null}
          {badges ? <span className={styles.info}>{badges}</span> : null}
        </div>
      </div>
      {action ? <div className={styles.action}>{action}</div> : null}
    </article>
  )
}

type IconComponent = ComponentType<IconProps>

/** The catalogue's wearable slots, keyed as it spells them. */
const CATEGORY_ICON: Record<string, IconComponent> = {
  body_shape: BackpackBody,
  earring: BackpackEarrings,
  eyebrows: BackpackEyebrows,
  eyes: BackpackEyes,
  eyewear: BackpackEyewear,
  facial_hair: BackpackFacialHair,
  feet: BackpackFeet,
  hair: BackpackHair,
  hands_wear: BackpackHandwear,
  hat: BackpackHat,
  helmet: BackpackHelmet,
  lower_body: BackpackLowerBody,
  mask: BackpackMask,
  mouth: BackpackMouth,
  skin: BackpackSkins,
  tiara: BackpackTiara,
  top_head: BackpackHead,
  upper_body: BackpackUpperBody
}

function categoryLabel(category: string): string {
  return category
    .split('_')
    .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
    .join(' ')
}

function Badge({ icon: Icon, label }: { icon: IconComponent; label: string }) {
  return (
    <span className={styles.badge} role="img" aria-label={label} title={label}>
      <Icon size={20} aria-hidden="true" />
    </span>
  )
}

type CollectibleBadgesProps = {
  /** The wearable slot; a slot without a glyph (an emote's kind) shows none. */
  category?: string
  /** `BaseMale` and `BaseFemale` together are unisex. */
  bodyShapes?: readonly string[]
  isSmart?: boolean
}

/** A wearable's slot, body shape and smart badges, each named for assistive technology. */
function CollectibleBadges({ category, bodyShapes = [], isSmart = false }: CollectibleBadgesProps) {
  const slot = category ? CATEGORY_ICON[category] : undefined
  const male = bodyShapes.some((shape) => shape.toLowerCase().includes('basemale'))
  const female = bodyShapes.some((shape) => shape.toLowerCase().includes('basefemale'))
  const body =
    male && female
      ? { icon: BackpackGenderUnisex, label: 'Unisex' }
      : male
        ? { icon: BackpackGenderMale, label: 'For male' }
        : female
          ? { icon: BackpackGenderFemale, label: 'For female' }
          : null

  return (
    <>
      {slot && category ? <Badge icon={slot} label={categoryLabel(category)} /> : null}
      {body ? <Badge icon={body.icon} label={body.label} /> : null}
      {isSmart ? <Badge icon={GeneralSmartItem} label="Smart wearable" /> : null}
    </>
  )
}

export { CatalogCard, CollectibleBadges }
export type { CatalogCardLinkProps, CatalogCardProps, CatalogCardRarity, CollectibleBadgesProps }
