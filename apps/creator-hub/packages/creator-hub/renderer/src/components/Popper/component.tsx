import { type PropsWithChildren } from 'react';
import { cx } from '/@/lib/cx';
import Popover from '@ui/atoms/Popover';

import type { Props } from './types';
import styles from './component.module.css';

import './styles.css';

export function Popper({
  open,
  onClose,
  anchorEl,
  placement = 'bottom',
  modifiers,
  children,
  className,
  ...props
}: PropsWithChildren<Props>) {
  const offset = modifiers?.find($ => $.name === 'offset')?.options?.offset?.[1] ?? 0;

  return (
    <Popover
      {...props}
      open={open}
      anchor={anchorEl}
      onClose={onClose}
      placement={placement}
      offset={offset}
      closeOnEscape={false}
      className={cx('Popper', styles.surface, className)}
    >
      {children}
    </Popover>
  );
}
