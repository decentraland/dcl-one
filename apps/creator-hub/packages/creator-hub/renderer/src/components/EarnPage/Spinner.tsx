import { CircularProgress } from 'decentraland-ui2';

import styles from './EarnModal.module.css';

const CHECKING_RING = 59.33;
const CHECKING_THICKNESS = 4.45;

/** A primary spinner announced as one named progressbar; MUI's own one is hidden inside it. */
export function Spinner({ label, size }: { label: string; size?: number }) {
  return (
    <span
      className={styles.spinnerBox}
      role="progressbar"
      aria-label={label}
    >
      <CircularProgress
        aria-hidden
        size={size}
        color="primary"
      />
    </span>
  );
}

/** The eligibility check's spinner: a round-capped Primary arc over a Snow 30 % track in an 80 box. */
export function CheckingSpinner({ label }: { label: string }) {
  return (
    <span
      className={styles.checkingSpinnerBox}
      role="progressbar"
      aria-label={label}
    >
      <CircularProgress
        className={styles.spinnerTrack}
        aria-hidden
        variant="determinate"
        value={100}
        size={CHECKING_RING}
        thickness={CHECKING_THICKNESS}
      />
      <CircularProgress
        className={styles.spinnerArc}
        aria-hidden
        disableShrink
        size={CHECKING_RING}
        thickness={CHECKING_THICKNESS}
        color="primary"
      />
    </span>
  );
}
