import { GeneralMapPin } from '../../../../foundations/icons'
import { Badges, ByLine, Cover, JumpIn, TitleLink } from './ProvisionalParts'
import type { PlaceCardSlotProps } from './types'
import styles from './PlaceCardSlot.module.css'

/**
 * The Explore grid card. TODO(shared-card): render the design system's PlaceCard here once the UI
 * Components migration ships it; until then a provisional card matching source's PlaceCard: cover
 * with badges, title, then the by-line and location, traded for Jump In on hover or focus.
 */
function PlaceCardSlot(props: PlaceCardSlotProps) {
  if (props.skeleton) {
    return (
      <li className={styles.cell} aria-hidden="true">
        <div className={`${styles.card} ${styles.skeleton}`}>
          <div className={styles.media} />
          <div className={styles.body}>
            <span className={styles.skeletonLine} />
            <span className={styles.skeletonLine} />
          </div>
        </div>
      </li>
    )
  }
  const { card, labels, onOpen, onJumpIn } = props
  return (
    <li className={styles.cell}>
      <div className={styles.card}>
        <div className={styles.media}>
          <Cover card={card} />
          <Badges card={card} labels={labels} />
        </div>
        <div className={styles.body}>
          <h3 className={styles.title}>
            <TitleLink card={card} className={styles.titleLink} onOpen={onOpen} />
          </h3>
          <div className={styles.swap}>
            <div className={styles.meta}>
              <ByLine card={card} labels={labels} />
              {card.coords ? (
                <span className={styles.location}>
                  <GeneralMapPin size={15} aria-hidden="true" />
                  {card.coords}
                </span>
              ) : null}
            </div>
            <JumpIn card={card} labels={labels} className={styles.jumpIn} onJumpIn={onJumpIn} />
          </div>
        </div>
      </div>
    </li>
  )
}

export { PlaceCardSlot }
export type { PlaceCardSlotProps }
