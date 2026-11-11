import { GeneralHome, GeneralJumpIn } from '../../../foundations/icons'
import { Button } from '../shared/Button/Button'
import styles from './JumpInvalid.module.css'

type JumpInvalidProps = {
  kind: 'place' | 'event'
  jumpInHref: string
  eventsHref: string
  homeHref: string
}

/** The landing for a jump link that resolves to nothing. */
function JumpInvalid({ kind, jumpInHref, eventsHref, homeHref }: JumpInvalidProps) {
  return (
    <section className={styles.root} aria-labelledby="jump-invalid-title">
      <h1 className={styles.title} id="jump-invalid-title">
        Oops!
      </h1>
      <p className={`${styles.message} ${styles.desktopOnly}`}>
        Looks like that link was invalid. If you still want to explore, jump into
        Decentraland&apos;s virtual world or browse community events.
      </p>
      <p className={`${styles.message} ${styles.mobileOnly}`}>
        Looks like that link was invalid. If you still want to explore, browse the community&apos;s
        events then jump in-world from your computer.
      </p>
      <div className={styles.actions}>
        <Button href={jumpInHref} className={`${styles.wide} ${styles.desktopOnly}`}>
          Jump into Decentraland
          <GeneralJumpIn size={20} aria-hidden="true" />
        </Button>
        {kind === 'event' ? (
          <Button href={eventsHref} variant="secondary" className={styles.wide}>
            Explore events
          </Button>
        ) : null}
      </div>
      <div className={`${styles.mobileBar} ${styles.mobileOnly}`}>
        <Button href={homeHref} className={styles.wide}>
          <GeneralHome size={20} aria-hidden="true" />
          Go to Homepage
        </Button>
      </div>
    </section>
  )
}

export { JumpInvalid }
export type { JumpInvalidProps }
