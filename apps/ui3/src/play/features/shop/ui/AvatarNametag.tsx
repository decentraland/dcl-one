// AvatarNametag — the Explorer design system's nametag (NametagDefault / NametagTyping / NametagClaimed) as
// the Shop's NAMEs flow draws it over the preview avatar: the NAME being picked, in its colour and with the
// claimed badge, as it will read once it is the player's; the caret while it is being typed; the design's
// "YourName" placeholder while nothing is. The page places it and says what it reads.

import verifiedIcon from '../../../assets/shop/names/verified.svg'
import styles from './AvatarNametag.module.css'

/** What an empty nametag reads. */
export const NAMETAG_PLACEHOLDER = 'YourName'

export function AvatarNametag({
  name,
  color,
  typing = false,
  claimed = true,
  suffix = '',
  className
}: {
  /** The NAME it reads; empty, the placeholder. */
  name: string
  /** The NAME's colour (the reference client's, for a claimed NAME). */
  color: string
  /** Shows the caret after the NAME, while its field has the focus. */
  typing?: boolean
  /** A claimed NAME carries the badge; an unclaimed name (the design system's NametagUnclaimed) its suffix. */
  claimed?: boolean
  /** What follows an unclaimed name: `#` and the last four of its address. */
  suffix?: string
  className?: string
}): React.JSX.Element {
  const empty = name === ''
  return (
    // A mirror of the NAME field, which is what a screen reader reads.
    <div className={`${styles.tag} ${className ?? ''}`.trim()} aria-hidden="true" data-nametag="">
      {empty ? (
        <span className={styles.placeholder}>{NAMETAG_PLACEHOLDER}</span>
      ) : (
        <span className={styles.text} style={{ color }}>
          {name}
          {typing && <span className={styles.caret} data-testid="nametag-caret" />}
        </span>
      )}
      {!empty && !claimed && suffix !== '' && <span className={styles.suffix}>{suffix}</span>}
      {(empty || claimed) && <img className={styles.badge} src={verifiedIcon} alt="" />}
    </div>
  )
}
