import styles from './SwapLabel.module.css'

export type SwapLabelProps = {
  /** What happened, shown at rest: "Request Sent". */
  idle: string
  /** What a click does, shown while the button is hovered or focused: "Cancel Request". */
  hover: string
}

/**
 * A button's label that says what happened, and what a click does while the button is hovered or
 * focused. Place it directly inside the button, and give the button an accessible name for the
 * action: both texts are in the document.
 */
export function SwapLabel({ idle, hover }: SwapLabelProps) {
  return (
    <>
      <span className={styles.idle}>{idle}</span>
      <span className={styles.hover}>{hover}</span>
    </>
  )
}
