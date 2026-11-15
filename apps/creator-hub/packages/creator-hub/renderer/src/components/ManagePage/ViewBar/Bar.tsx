import type { ComponentProps } from 'react';
import { cx } from '/@/lib/cx';

import { FiltersBar } from '../../FiltersBar';

import styles from './ViewBar.module.css';

/** The pill row of the Manage filters. */
export function Bar({ className, ...props }: ComponentProps<typeof FiltersBar>) {
  return (
    <FiltersBar
      {...props}
      className={cx(styles.bar, className)}
    />
  );
}
