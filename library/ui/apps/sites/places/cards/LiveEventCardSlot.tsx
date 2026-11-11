import { Badges, ByLine, Cover, JumpIn, TitleLink } from './ProvisionalParts'
import type { PlaceCardSlotProps } from './types'
import styles from './LiveEventCardSlot.module.css'

/**
 * The Live Now card. TODO(shared-card): render the design system's EventCard (Live) here once the UI
 * Components migration ships it; until then a provisional card matching source's LiveEventCard:
 * cover with badges over a dark content bar whose by-line trades places with Jump In on hover
 * (phones show both).
 */
function LiveEventCardSlot(props: Extract<PlaceCardSlotProps, { skeleton?: false }>) {
  const { card, labels, onOpen, onJumpIn } = props
  return (
    <div className={styles.cell}>
      <article className={styles.card} aria-label={card.title}>
        <div className={styles.media}>
          <Cover card={card} priority />
          <Badges card={card} labels={labels} />
        </div>
        <div className={styles.bar}>
          <h3 className={styles.title}>
            <TitleLink card={card} className={styles.titleLink} onOpen={onOpen} />
          </h3>
          <div className={styles.swap}>
            <div className={styles.byRow}>
              <ByLine card={card} labels={labels} />
            </div>
            <JumpIn card={card} labels={labels} className={styles.jumpIn} onJumpIn={onJumpIn} />
          </div>
        </div>
      </article>
    </div>
  )
}

export { LiveEventCardSlot }
