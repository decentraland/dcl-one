import { cx } from '../../lib/utils/cx';
import Spinner from '@ui/atoms/HubSpinner';

import type { Props, SpinnerProps, SpinnerSize } from './types';

import styles from './Loading.module.css';

const SPINNER_PX: Record<SpinnerSize, number> = {
  mini: 14,
  tiny: 16,
  small: 24,
  medium: 32,
  large: 48,
  big: 52,
  huge: 58,
  massive: 64,
};

/** The shared spinner, centered over its nearest positioned ancestor. */
export function LoadingSpinner({ size = 'medium' }: SpinnerProps) {
  return (
    <span className={styles.spinner}>
      <Spinner size={SPINNER_PX[size]} />
    </span>
  );
}

export function Loading({ dimmer = true, size }: Props) {
  return (
    <div className="loading">
      <LoadingSpinner size={size} />
      {dimmer && (
        <div
          className={cx('loading-dimmer', styles.dimmer)}
          data-testid="loading-dimmer"
        />
      )}
    </div>
  );
}
