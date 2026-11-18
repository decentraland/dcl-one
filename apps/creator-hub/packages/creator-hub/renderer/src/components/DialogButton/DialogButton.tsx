import { forwardRef, type ButtonHTMLAttributes } from 'react';
import { cx } from '/@/lib/cx';

import styles from './DialogButton.module.css';

type Props = ButtonHTMLAttributes<HTMLButtonElement> & {
  tone: 'primary' | 'secondary';
};

export const DialogButton = forwardRef<HTMLButtonElement, Props>(function DialogButton(
  { tone, className, ...props },
  ref,
) {
  return (
    <button
      {...props}
      ref={ref}
      className={cx(styles.root, className)}
      data-tone={tone}
    />
  );
});
