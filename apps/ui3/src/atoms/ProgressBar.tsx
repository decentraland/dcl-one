import type { CSSProperties } from "react";
import styles from "./ProgressBar.module.css";

export type ProgressBarProps = {
  value?: number;
  // No known value: the bar moves and the element has no `aria-valuenow`.
  indeterminate?: boolean;
  danger?: boolean;
  className?: string;
  "aria-label"?: string;
  "aria-labelledby"?: string;
};

export default function ProgressBar({
  value = 0,
  indeterminate = false,
  danger = false,
  className,
  ...naming
}: ProgressBarProps) {
  const pct = Math.min(100, Math.max(0, value));
  const style: (CSSProperties & { "--progress": string }) | undefined = indeterminate
    ? undefined
    : { "--progress": pct + "%" };

  return (
    <span
      role="progressbar"
      className={className ? styles.root + " " + className : styles.root}
      data-indeterminate={indeterminate ? "" : undefined}
      data-danger={danger ? "" : undefined}
      aria-valuemin={indeterminate ? undefined : 0}
      aria-valuemax={indeterminate ? undefined : 100}
      aria-valuenow={indeterminate ? undefined : pct}
      {...naming}
    >
      <span className={styles.bar} style={style} />
    </span>
  );
}
