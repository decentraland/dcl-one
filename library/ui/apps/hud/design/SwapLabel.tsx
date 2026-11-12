// SwapLabel — a button's label that says what happened, and what a click does while the button is
// hovered or focused: "Request Sent" becoming "Cancel Request". Place it directly inside the button;
// give the button an accessible name for the action.

import styles from './SwapLabel.module.css'

export function SwapLabel({ idle, hover }: { idle: string; hover: string }): React.JSX.Element {
  return (
    <>
      <span className={styles.idle}>{idle}</span>
      <span className={styles.hover}>{hover}</span>
    </>
  )
}
