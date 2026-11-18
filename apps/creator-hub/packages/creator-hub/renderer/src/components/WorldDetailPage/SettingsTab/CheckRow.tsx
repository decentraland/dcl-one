import type { LabelHTMLAttributes } from 'react';
import { cx } from '/@/lib/cx';

import styles from './CheckRow.module.css';

/** A checkbox and its text on one row; the label is the click target of both. */
export function CheckRow({ className, ...props }: LabelHTMLAttributes<HTMLLabelElement>) {
  return (
    <label
      {...props}
      className={cx(styles.checkRow, className)}
    />
  );
}
