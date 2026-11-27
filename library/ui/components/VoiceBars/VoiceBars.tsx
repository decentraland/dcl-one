import styles from './VoiceBars.module.css'

export type VoiceBarsProps = {
  /** The badge's accessible name. */
  label?: string
}

/** Three green bars that bounce while someone talks: the speaking badge of a nametag. */
export function VoiceBars({ label = 'Speaking' }: VoiceBarsProps) {
  return (
    <span className={styles.root} role="img" aria-label={label} data-speaking-bars>
      <span className={styles.bar} />
      <span className={`${styles.bar} ${styles.middle}`} />
      <span className={styles.bar} />
    </span>
  )
}
