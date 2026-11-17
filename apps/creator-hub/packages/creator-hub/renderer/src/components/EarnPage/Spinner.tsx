import UiSpinner from '@ui/atoms/Spinner';

import styles from './EarnModal.module.css';

const CHECKING_RING = 59.33;
const CHECKING_THICKNESS = 4.45;
const RING_BOX = 44;

/** A primary spinner announced as one named progressbar; the drawn one is hidden inside it. */
export function Spinner({ label, size = 40 }: { label: string; size?: number }) {
  return (
    <span
      className={styles.spinnerBox}
      role="progressbar"
      aria-label={label}
    >
      <UiSpinner
        role={undefined}
        aria-label={undefined}
        aria-hidden
        size={size}
        color="var(--primary)"
      />
    </span>
  );
}

function Ring({ className }: { className: string }) {
  return (
    <svg
      className={className}
      viewBox={`${RING_BOX / 2} ${RING_BOX / 2} ${RING_BOX} ${RING_BOX}`}
      width={CHECKING_RING}
      height={CHECKING_RING}
      aria-hidden
      focusable={false}
    >
      <circle
        cx={RING_BOX}
        cy={RING_BOX}
        r={(RING_BOX - CHECKING_THICKNESS) / 2}
        fill="none"
        strokeWidth={CHECKING_THICKNESS}
      />
    </svg>
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
      <Ring className={styles.spinnerTrack} />
      <Ring className={styles.spinnerArc} />
    </span>
  );
}
