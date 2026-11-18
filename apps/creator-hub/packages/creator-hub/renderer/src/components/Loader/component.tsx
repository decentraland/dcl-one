import type { ComponentPropsWithoutRef } from 'react';
import Spinner from '@ui/atoms/Spinner';

import styles from './component.module.css';

import './styles.css';

type Props = Omit<ComponentPropsWithoutRef<'span'>, 'color'> & {
  size?: number;
  color?: string;
  overlay?: boolean;
};

/** A spinner centred in the box of its parent, or over it with `overlay`. */
export function Loader({ overlay, size = 40, color = 'primary', ...props }: Props) {
  const spinner = (
    <Spinner
      role="progressbar"
      size={size}
      color={color}
      {...props}
    />
  );

  if (overlay) return <div className={styles.overlay}>{spinner}</div>;

  return <div className="Loader">{spinner}</div>;
}
