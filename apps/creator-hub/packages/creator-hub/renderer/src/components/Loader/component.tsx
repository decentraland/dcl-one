import type { ComponentPropsWithoutRef } from 'react';
import Spinner from '@ui/atoms/HubSpinner';

import styles from './component.module.css';

import './styles.css';

type Props = Omit<ComponentPropsWithoutRef<'span'>, 'color'> & {
  size?: number | string;
  color?: string;
  overlay?: boolean;
};

const DEFAULT_SIZE = 40;

function toPixels(size: number | string | undefined): number {
  if (size === undefined) return DEFAULT_SIZE;
  const pixels = typeof size === 'number' ? size : Number.parseFloat(size);
  return Number.isFinite(pixels) ? pixels : DEFAULT_SIZE;
}

export function Loader({ overlay, size, color = 'primary', ...props }: Props) {
  const spinner = (
    <Spinner
      role="progressbar"
      size={toPixels(size)}
      color={color}
      {...props}
    />
  );

  if (overlay) return <div className={styles.overlay}>{spinner}</div>;

  return <div className="Loader">{spinner}</div>;
}
