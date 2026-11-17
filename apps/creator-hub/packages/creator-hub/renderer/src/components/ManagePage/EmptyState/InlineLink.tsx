import type { ComponentProps } from 'react';
import { cx } from '/@/lib/cx';

import { ExternalLink } from '../../ExternalLink';

import styles from './EmptyState.module.css';

/** A link inside the text of an empty state. */
export function InlineLink({ className, ...props }: ComponentProps<typeof ExternalLink>) {
  return (
    <ExternalLink
      {...props}
      className={cx(styles.inlineLink, className)}
    />
  );
}
