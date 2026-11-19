import {
  useEffect,
  useRef,
  useState,
  type AnchorHTMLAttributes,
  type ComponentType,
  type CSSProperties,
  type ReactNode
} from 'react'
import { GeneralMana, GeneralPolygonMana } from '../../foundations/icons/general'
import { ShopCredits } from '../../foundations/icons/shop'
import { imageSource } from '../../link.helpers'
import styles from './ShopCard.module.css'

/** The router's link, passed in so the card opens its item under any router. */
export type ShopCardLinkProps = AnchorHTMLAttributes<HTMLAnchorElement> & { href: string }
export type ShopCardLinkComponent = ComponentType<ShopCardLinkProps>

const TINTED = ['common', 'uncommon', 'rare', 'epic', 'legendary', 'exotic', 'mythic', 'unique'] as const

/** The rarities the foundations colour; anything else is drawn as `base`. */
export type ShopCardRarity = 'base' | (typeof TINTED)[number]

export type ShopCardCreator = {
  name: string
  /** The creator's face; a blank disc without it. */
  picture?: string
  /** The name's colour, e.g. the creator's name colour. */
  color?: string
}

export type ShopCardProps = {
  name: string
  image?: string
  /** A decorative glyph shown when there is no image or it fails to load, e.g. a NAME's. */
  placeholder?: ReactNode
  /**
   * Tints the thumbnail, the flap and the label. Without it the card is drawn as `base` and shows no
   * label; another value (a feed's own rarity) is drawn as `base` and shown as given.
   */
  rarity?: ShopCardRarity | (string & {})
  /** The glyph in the rarity's corner flap, e.g. the wearable category's. */
  categoryIcon?: ReactNode
  /** A tag over the thumbnail, e.g. "-20%". */
  discount?: string
  creator?: ShopCardCreator
  /** Glyphs after the rarity label, e.g. body shape or smart wearable badges. */
  badges?: ReactNode
  /** The price as the caller decided it: a `ShopCardPrice`, or the caller's own price line. */
  price?: ReactNode
  /** Shown in place of the creator, the label and the price while the card is hovered or holds focus. */
  actions?: ReactNode
  /** The item's page: the whole card links to it. */
  href?: string
  /** Renders `href` (a router link); a plain anchor without it. */
  linkComponent?: ShopCardLinkComponent
  /** Without `href`, the whole card is a toggle button that calls this. */
  onSelect?: () => void
  selected?: boolean
  /**
   * `shop` is the in-world Shop's card: one meta row, actions on hover or focus. `catalogue` is a web
   * grid's: a long price wraps under the label, and on a touch screen the actions always show.
   */
  variant?: 'shop' | 'catalogue'
  /** Renders the name as a heading of this level, so a grid of cards can be navigated by heading. */
  headingLevel?: 2 | 3 | 4 | 5 | 6
}

const RARITIES = new Set<string>(TINTED)

/**
 * Whether the image failed. A server-rendered page can fail it before React attaches `onError`, so
 * the mounted element is checked too.
 */
function useFailedImage(src: string | undefined) {
  const [failed, setFailed] = useState(false)
  const ref = useRef<HTMLImageElement>(null)
  useEffect(() => {
    const img = ref.current
    setFailed(Boolean(src && img && img.complete && img.naturalWidth === 0))
  }, [src])
  return { failed, ref, onError: () => setFailed(true) }
}

/**
 * A catalogue item (a wearable, emote, NAME or LAND) in a grid: the thumbnail on its rarity's glow with
 * the category flap, then the name, the creator and a row with the rarity label, badges and price.
 * Hover or keyboard focus swaps the creator and that row for the actions and draws the ring, as does
 * `selected`. Presentational: the caller decides the price, the actions and where the card goes.
 */
export function ShopCard({
  name,
  image,
  placeholder,
  rarity,
  categoryIcon,
  discount,
  creator,
  badges,
  price,
  actions,
  href,
  linkComponent: LinkComponent,
  onSelect,
  selected = false,
  variant = 'shop',
  headingLevel
}: ShopCardProps) {
  const thumbnail = imageSource(image)
  const face = imageSource(creator?.picture)
  const { failed, ref, onError } = useFailedImage(thumbnail)
  const faceImage = useFailedImage(face)
  const tint = rarity && RARITIES.has(rarity) ? rarity : 'base'
  const classes = [
    styles.card,
    variant === 'catalogue' && styles.catalogue,
    selected && styles.selected,
    actions != null && styles.hasActions
  ]
    .filter(Boolean)
    .join(' ')
  // The name is the card's control, so a heading leads to it. The full-card target behind the info is
  // the same link or toggle for the pointer only: out of the tab order and the accessibility tree.
  const Anchor = LinkComponent ?? 'a'
  const control = href ? (
    <Anchor href={href} className={styles.control}>
      {name}
    </Anchor>
  ) : onSelect ? (
    <button type="button" className={styles.control} aria-pressed={selected} onClick={onSelect}>
      {name}
    </button>
  ) : (
    name
  )
  const hit = href ? (
    <Anchor href={href} className={styles.hit} tabIndex={-1} aria-hidden="true" />
  ) : onSelect ? (
    <button type="button" className={styles.hit} tabIndex={-1} aria-hidden="true" onClick={onSelect} />
  ) : null
  return (
    <article className={classes} data-rarity={tint}>
      {hit}
      <span className={styles.thumb}>
        {thumbnail && !failed ? (
          <img
            ref={ref}
            className={styles.image}
            src={thumbnail}
            alt=""
            loading="lazy"
            onError={onError}
          />
        ) : placeholder != null ? (
          <span className={styles.placeholder} aria-hidden="true">
            {placeholder}
          </span>
        ) : null}
        {categoryIcon != null && (
          <span className={styles.flap}>
            <FlapGlyph />
            <span className={styles.flapIcon}>{categoryIcon}</span>
          </span>
        )}
        {discount != null && <span className={styles.discount}>{discount}</span>}
      </span>
      <div className={styles.info}>
        {headingLevel ? (
          <Heading level={headingLevel} className={styles.name} title={name}>
            {control}
          </Heading>
        ) : (
          <span className={styles.name} title={name}>
            {control}
          </span>
        )}
        {creator != null && (
          <span className={styles.creator}>
            {face && !faceImage.failed ? (
              <img
                ref={faceImage.ref}
                className={styles.face}
                src={face}
                alt=""
                onError={faceImage.onError}
              />
            ) : (
              <span className={styles.face} aria-hidden="true" />
            )}
            <span
              className={styles.creatorName}
              style={creator.color ? ({ '--creator-color': creator.color } as CSSProperties) : undefined}
            >
              {creator.name}
            </span>
          </span>
        )}
        <span className={styles.meta}>
          <span className={styles.tags}>
            {rarity != null && <span className={styles.rarity}>{rarity}</span>}
            {badges != null && <span className={styles.badges}>{badges}</span>}
          </span>
          {price != null && <span className={styles.price}>{price}</span>}
        </span>
        {actions != null && <span className={styles.actions}>{actions}</span>}
      </div>
    </article>
  )
}

function Heading({
  level,
  ...props
}: { level: 2 | 3 | 4 | 5 | 6; className: string | undefined; title: string; children: ReactNode }) {
  const Tag = `h${level}` as const
  return <Tag {...props} />
}

/** The rarity's corner (its diagonal rounded at both ends) and its 20% shadow along that edge. */
function FlapGlyph() {
  return (
    <svg className={styles.flapShape} viewBox="0 0 45.283 45.1207" aria-hidden="true" focusable="false">
      <path
        className={styles.flapSolid}
        d="M45.283 0C42.3351 0 39.5085 1.16715 37.428 3.24349L3.23252 37.3411C1.16235 39.4071 0 42.2045 0 45.1207V0C0 0 35.1599 0 45.283 0Z"
      />
      <path
        className={styles.flapShadow}
        d="M0 36.3979V44.4899C0.0196031 41.6005 1.17997 38.8328 3.23252 36.7844L29.7931 10.2773C29.8292 10.2412 29.8655 10.2055 29.9021 10.17L0 36.3979Z"
      />
    </svg>
  )
}

export type ShopCardPriceProps =
  | { kind: 'credits'; amount: string; title?: string }
  | { kind: 'mana'; amount: string; network?: 'ethereum' | 'polygon'; title?: string }
  /** No price: why, e.g. "Not for sale" or "Credits price unavailable". */
  | { kind: 'none'; label: string }

/**
 * A price as the caller decided it, for `ShopCard`'s `price`: credits with their coin, MANA with its
 * network's glyph, or the reason there is none. The currency is named for assistive technology.
 */
export function ShopCardPrice(props: ShopCardPriceProps) {
  if (props.kind === 'none') return <span>{props.label}</span>
  const Mark =
    props.kind === 'credits' ? ShopCredits : props.network === 'ethereum' ? GeneralMana : GeneralPolygonMana
  return (
    <span title={props.title}>
      <Mark size={16} aria-hidden="true" />
      <span className={styles.visuallyHidden}>Price: </span>
      {props.amount}
      <span className={styles.visuallyHidden}>{props.kind === 'credits' ? ' credits' : ' MANA'}</span>
    </span>
  )
}

/** A ShopCard's place while the grid loads: the thumbnail and two bars under the purple sweep. */
export function ShopCardSkeleton() {
  return (
    <span className={styles.skeleton} aria-hidden="true">
      <span className={`${styles.sheen} ${styles.skeletonTile}`} />
      <span className={styles.skeletonInfo}>
        <span className={`${styles.sheen} ${styles.skeletonBar}`} />
        <span className={`${styles.sheen} ${styles.skeletonBar} ${styles.skeletonBarShort}`} />
      </span>
    </span>
  )
}
