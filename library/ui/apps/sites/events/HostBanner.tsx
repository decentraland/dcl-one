import { useId } from 'react'
import { GeneralAdd, GeneralCheck } from '../../../foundations/icons'
import hostAvatar from '../assets/events/host_avatar.webp'
import hostScene from '../assets/events/host_scene.webp'
import { Button } from '../shared/Button/Button'
import styles from './HostBanner.module.css'

const CHECKS = [
  'Choose a vibe people can drop into.',
  'Add the event details.',
  'Publish it and send the invites.'
] as const

/** Source's invitation to host: the checklist, Create an Event and Learn More. */
function HostBanner({ createHref, learnMoreHref }: { createHref: string; learnMoreHref: string }) {
  const titleId = useId()
  return (
    <section className={styles.root} aria-labelledby={titleId}>
      <img className={styles.avatar} src={hostAvatar} alt="" loading="lazy" />
      <div className={styles.content}>
        <h2 id={titleId} className={styles.title}>
          Host an Event
        </h2>
        <p className={styles.subtitle}>Bring people together, your way.</p>
        <ul className={styles.checklist}>
          {CHECKS.map((check) => (
            <li key={check} className={styles.check}>
              <span className={styles.box} aria-hidden="true">
                <GeneralCheck size={16} />
              </span>
              {check}
            </li>
          ))}
        </ul>
        <div className={styles.buttons}>
          <Button href={createHref} size="sm">
            <GeneralAdd size={16} aria-hidden="true" />
            Create an Event
          </Button>
          <Button href={learnMoreHref} target="_blank" variant="secondary" size="sm">
            Learn More
          </Button>
        </div>
      </div>
      <div className={styles.scene} aria-hidden="true">
        <img src={hostScene} alt="" loading="lazy" />
      </div>
    </section>
  )
}

export { HostBanner }
