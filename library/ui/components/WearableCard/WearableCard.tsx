import { useState, type CSSProperties, type ReactNode } from 'react'
import rarityStyles from './rarity.module.css'
import styles from './WearableCard.module.css'

/** The rarities the foundations colour, and `base` for an item with none. */
export type WearableCardRarity =
  | 'base'
  | 'common'
  | 'uncommon'
  | 'rare'
  | 'epic'
  | 'legendary'
  | 'mythic'
  | 'unique'
  | 'exotic'

/**
 * The pictures a host draws the tile with, each a URL. Without one the card draws that part plainly:
 * a wash of the rarity's colour, a triangular flap, a plain outline and a dark panel.
 */
export type WearableCardArt = {
  /** The tile's background, for the card's rarity. */
  tile?: string
  /** The corner flap's shape, used as a mask over the rarity's colour. */
  flap?: string
  /** The equipped outline, a 9-slice used as a mask over the ring's orange. */
  ring?: string
  /** The panel that grows behind a hovered card, a 9-slice. */
  hoverPanel?: string
}

export type WearableCardLabels = {
  equip: string
  unequip: string
  isNew: string
  smart: string
  incompatible: string
}

export type WearableCardProps = {
  thumbnail?: string
  /** The item's name: the card's accessible name and its title. */
  name?: string
  /** Tints the corner flap. `base` without it. */
  rarity?: WearableCardRarity
  art?: WearableCardArt
  equipped?: boolean
  /** Open in a detail panel: the card's pressed state, with no ring unless `pick`. */
  selected?: boolean
  isNew?: boolean
  /** Shows the smart item badge. */
  isSmart?: boolean
  /** Covers the tile with a note shown on hover, and an unequipped card offers no action. */
  incompatible?: boolean
  /** False for an item that cannot be taken off: equipped, it offers no action. */
  unequippable?: boolean
  /** The glyph in the top-left flap, e.g. the wearable category's. */
  categoryIcon?: ReactNode
  /** A number shown top-right, e.g. the emote wheel slot the item is in. */
  slotNumber?: number
  /** A press on the card. Not called while `coverNote` covers it. */
  onClick?: () => void
  onDoubleClick?: () => void
  /** A press on the action shown under a hovered card. */
  onEquip?: () => void
  onHoverChange?: (hovered: boolean) => void
  /** A tile to pick, not to equip: no hover panel or action, and the ring marks `selected`. */
  pick?: boolean
  /** How many copies there are, shown as "xN" above one. */
  count?: number
  /** Covers the tile with this note, always shown: the card cannot be pressed and offers no action. */
  coverNote?: string
  /** The card's texts. English by default. */
  labels?: Partial<WearableCardLabels>
}

const LABELS: WearableCardLabels = {
  equip: 'EQUIP',
  unequip: 'UNEQUIP',
  isNew: 'NEW',
  smart: 'SMART',
  incompatible: 'Incompatible with body shape'
}

// The url is quoted: an inlined image is a data URI, which an unquoted url() can cut short.
const url = (src: string | undefined): string | undefined =>
  src == null ? undefined : `url("${src}")`

/**
 * An item tile of the Explorer HUD's backpack: the thumbnail on its rarity's tile, a flap in the
 * rarity's colour with the category glyph, an orange ring while equipped, and on hover a lift with
 * the EQUIP or UNEQUIP action under the tile. The card is a toggle button (`selected`); the action is
 * for the pointer only.
 */
export function WearableCard({
  thumbnail,
  name,
  rarity = 'base',
  art,
  equipped = false,
  selected = false,
  isNew = false,
  isSmart = false,
  incompatible = false,
  unequippable = true,
  categoryIcon,
  slotNumber,
  onClick,
  onDoubleClick,
  onEquip,
  onHoverChange,
  pick = false,
  count,
  coverNote,
  labels
}: WearableCardProps) {
  // The picture that failed to load: a new one is tried again.
  const [failedSrc, setFailedSrc] = useState<string>()
  const failed = failedSrc != null && failedSrc === thumbnail
  const covered = coverNote != null
  const text = { ...LABELS, ...labels }
  const action =
    pick || covered
    ? null
    : incompatible && !equipped
      ? null
      : equipped
        ? unequippable
          ? text.unequip
          : null
        : text.equip
  const ringed = pick ? selected : equipped
  const hostArt = {
    '--wearable-card-tile': url(art?.tile),
    '--wearable-card-flap': url(art?.flap),
    '--wearable-card-ring': url(art?.ring),
    '--wearable-card-hover-panel': url(art?.hoverPanel)
  } as CSSProperties
  return (
    <button
      type="button"
      className={[
        styles.card,
        rarityStyles.tone,
        ringed && styles.equipped,
        pick && styles.pick
      ]
        .filter(Boolean)
        .join(' ')}
      style={hostArt}
      data-rarity={rarity}
      title={name}
      aria-label={name}
      aria-pressed={selected}
      aria-disabled={covered ? true : undefined}
      onClick={covered ? undefined : onClick}
      onDoubleClick={covered ? undefined : onDoubleClick}
      onMouseEnter={onHoverChange && (() => onHoverChange(true))}
      onMouseLeave={onHoverChange && (() => onHoverChange(false))}
    >
      {!pick && (
        <span
          className={[styles.hoverPanel, art?.hoverPanel == null && styles.plainPanel]
            .filter(Boolean)
            .join(' ')}
          aria-hidden="true"
        />
      )}
      <span className={[styles.body, art?.tile == null && styles.plainTile].filter(Boolean).join(' ')}>
        {thumbnail && !failed && (
          <img className={styles.thumb} src={thumbnail} alt="" onError={() => setFailedSrc(thumbnail)} />
        )}
        {categoryIcon != null && (
          <span className={styles.flap}>
            <span className={styles.flapTint} aria-hidden="true" />
            <span className={styles.flapIcon}>{categoryIcon}</span>
          </span>
        )}
        {isNew && <span className={styles.new}>{text.isNew}</span>}
        {isSmart && <span className={styles.smart}>{text.smart}</span>}
        {slotNumber != null && <span className={styles.slotNumber}>{slotNumber}</span>}
        {count != null && count > 1 && <span className={styles.count}>{`x${count}`}</span>}
        {covered && (
          <span className={styles.incompatibleCover}>
            <span className={`${styles.incompatibleNote} ${styles.shownNote}`}>{coverNote}</span>
          </span>
        )}
        {incompatible && (
          <span className={styles.incompatibleCover}>
            <span className={styles.incompatibleNote}>{text.incompatible}</span>
          </span>
        )}
      </span>
      <span
        className={[styles.ring, art?.ring == null && styles.plainRing].filter(Boolean).join(' ')}
        aria-hidden="true"
      />
      {action != null && (
        <span
          className={`${styles.action} ${equipped ? styles.unequip : styles.equip}`}
          role="button"
          onClick={(event) => {
            event.stopPropagation()
            onEquip?.()
          }}
          // The card's own double press is not this action's.
          onDoubleClick={(event) => event.stopPropagation()}
        >
          {action}
        </span>
      )}
    </button>
  )
}
