import { useId } from 'react'
import { JumpInIcon } from '../../../components/JumpInCTA/JumpInCTA'
import { Button } from '../shared/Button/Button'
import { HomeIcon } from './JumpIcons'
import styles from './JumpInvalid.module.css'

type JumpInvalidProps = {
  kind: 'place' | 'event'
  jumpInHref: string
  eventsHref: string
  homeHref: string
}

/** The landing for a jump link that resolves to nothing. */
function JumpInvalid({ kind, jumpInHref, eventsHref, homeHref }: JumpInvalidProps) {
  const titleId = useId()
  return (
    <section className={styles.root} aria-labelledby={titleId}>
      <h1 className={styles.title} id={titleId}>
        Oops!
      </h1>
      <p className={styles.message}>
        <span className={styles.desktopOnly}>
          Looks like that link was invalid. If you still want to explore, jump into
          Decentraland&apos;s virtual world or browse community events.
        </span>
        <span className={styles.mobileOnly}>
          Looks like that link was invalid. If you still want to explore, browse the
          community&apos;s events then jump in-world from your computer.
        </span>
      </p>
      <div className={`${styles.jumpInto} ${styles.desktopOnly}`}>
        <Button href={jumpInHref} className={styles.jumpIn}>
          Jump into Decentraland
          <JumpInIcon />
        </Button>
      </div>
      {kind === 'event' ? (
        <Button href={eventsHref} variant="secondary" className={styles.explore}>
          EXPLORE EVENTS
        </Button>
      ) : null}
      <div className={styles.mobileBar}>
        <Button href={homeHref} className={styles.home}>
          <HomeIcon size={22} />
          Go to Homepage
        </Button>
      </div>
    </section>
  )
}

export { JumpInvalid }
export type { JumpInvalidProps }
